import { parse as parseScript, parseExpression } from "@babel/parser";
import generate from "@babel/generator";
import { parse as parseTemplate } from "@vue/compiler-dom";
import { compileTemplate, parse as parseSfc } from "@vue/compiler-sfc";
import {
    compilerArtifactForApp,
    recordsForApp,
      type HawkCompilerArtifact,
      type HawkCompilerDynamicBindingWire,
      type HawkCompilerDynamicValueWire,
      type HawkCompilerEventHandlerActionWire,
      type HawkCompilerEventHandlerWire,
      type HawkCompilerInitialDynamicValueWire,
      type HawkCompilerListTemplateNodeWire,
      type HawkCompilerListTemplateWire,
      type HawkCompilerReactiveBindingWire,
      type HawkCompilerTemplateScalarWire,
      type HawkElementSpec,
      type HawkEventSpec,
    type HawkLifecycleSpec,
  } from "../../hawk2ui-native/src/index.ts";

export interface HawkVueCompileInput {
  readonly filename: string;
  readonly source: string;
}

export interface HawkVueCompileOutput {
  readonly framework: "vue";
  readonly filename: string;
  readonly records: readonly string[];
  readonly compilerArtifact: HawkCompilerArtifact;
}

type AstNode = Record<string, unknown>;
type LiteralRecord = Readonly<Record<string, string | number | boolean>>;

interface VueLoweringContext {
  readonly arrays: ReadonlyMap<string, readonly LiteralRecord[]>;
  readonly components: ReadonlyMap<string, VueComponentDefinition>;
  readonly initialDynamicValues: ReadonlyMap<string, HawkCompilerInitialDynamicValueWire>;
  readonly locals: ReadonlyMap<string, LiteralRecord>;
  readonly scalars: ReadonlyMap<string, string | number | boolean>;
    readonly slots: ReadonlyMap<string, readonly AstNode[]>;
    readonly componentStack: readonly string[];
  readonly reactivity: HawkCompilerReactiveBindingWire[];
  readonly dynamicBindings: HawkCompilerDynamicBindingWire[];
  readonly listTemplates: HawkCompilerListTemplateWire[];
  readonly syntheticEventHandlers: HawkCompilerEventHandlerWire[];
  readonly pendingListTemplateAnchors: Map<string, string[]>;
}

interface VueComponentDefinition {
  readonly props: readonly string[];
  readonly root: AstNode;
}

const VISUAL_PROP_NAMES = ["font_size", "color", "background"] as const;
const VIEW_ELEMENT_TAGS = new Set(["div", "section", "main", "article", "header", "footer", "nav", "aside", "form", "label", "ul", "ol", "li"]);
const TEXT_ELEMENT_TAGS = new Set(["span", "p", "strong", "em", "small", "code", "h1", "h2", "h3", "h4", "h5", "h6"]);
const VUE_EVENT_DIRECTIVES = new Map<string, HawkEventSpec["kind"]>([
  ["click", "pointer.press"],
  ["pointerdown", "pointer.press"],
  ["pointerup", "pointer.release"],
  ["pointermove", "pointer.move"],
  ["pointerdrag", "pointer.drag"],
  ["pointerenter", "pointer.enter"],
  ["pointerleave", "pointer.leave"],
  ["wheel", "pointer.wheel"],
  ["keydown", "keyboard.key-down"],
  ["keyup", "keyboard.key-up"],
  ["textinput", "keyboard.text-input"],
  ["focus", "focus.focus-in"],
  ["blur", "focus.focus-out"],
  ["input", "input.value-changed"],
  ["change", "input.value-committed"],
  ["resize", "resize"],
]);
const VUE_LIFECYCLE_DIRECTIVES = new Map<string, HawkLifecycleSpec["phase"]>([
  ["mounted", "mounted"],
  ["suspended", "suspended"],
  ["resumed", "resumed"],
  ["hot-reloaded", "hot-reloaded"],
  ["error-boundary", "error-boundary"],
  ["shutdown", "shutdown"],
  ["unmounted", "unmounted"],
]);
const RESERVED_RUNTIME_PROP_NAMES = new Set<string>([
  "id",
  "key",
  "ref",
  "class",
  "data-asset",
  "width",
  "height",
  ...VISUAL_PROP_NAMES,
]);

export function compileHawkVue(input: HawkVueCompileInput): HawkVueCompileOutput {
  if (!input.filename.endsWith(".vue")) {
    throw new Error("Hawk2UI Vue compiler inputs must be .vue files.");
  }
  const parsed = parseSfc(input.source, { filename: input.filename });
  if (parsed.errors.length > 0) {
    throw new Error(`vue.sfc.invalid: ${parsed.errors.map(String).join("; ")}`);
  }
  const templateSource = parsed.descriptor.template?.content;
  if (!templateSource) {
    throw new Error("vue.template.missing: Vue SFC compiler input must contain a template.");
  }
  const compiled = compileTemplate({ source: templateSource, filename: input.filename, id: "hawk2ui" });
  if (compiled.errors.length > 0) {
    throw new Error(`vue.template.invalid: ${compiled.errors.map(String).join("; ")}`);
  }

  const ast = parseTemplate(templateSource) as unknown as AstNode;
  const rootNode = firstVueHawkElement(arrayField(ast, "children"));
  if (!rootNode) {
    throw new Error("vue.root.missing: Vue compiler output must contain one hawk root element.");
  }
  const script = parsed.descriptor.scriptSetup?.content ?? parsed.descriptor.script?.content ?? "";
  const context: VueLoweringContext = {
    arrays: literalArraysFromScript(script),
    components: componentDefinitionsFromScript(script),
    initialDynamicValues: initialDynamicValuesFromScript(script),
    locals: new Map(),
    scalars: scalarValuesFromScript(script),
      slots: new Map(),
      componentStack: [],
      reactivity: vueWatchReactivityBindings(script),
      dynamicBindings: [],
      listTemplates: [],
      syntheticEventHandlers: [],
      pendingListTemplateAnchors: new Map(),
    };
    const root = withRootLifecycle(
      vueElementToSpec(rootNode, context),
      vueLifecycleApiCalls(script),
    );
  validateUniqueChildKeys(root);
  const app = { name: input.filename, root };
  const eventHandlers = eventHandlerArtifactsForSpec(
    root,
    script,
    context.listTemplates,
    context.syntheticEventHandlers,
  );
  return {
    framework: "vue",
    filename: input.filename,
    records: recordsForApp(app),
      compilerArtifact: compilerArtifactForApp(
        app,
        context.reactivity,
        context.dynamicBindings,
        [...context.initialDynamicValues.values()],
          {
            compiler: {
              framework: "vue",
              compiler: "@hawk2ui/vue",
              source_path: input.filename,
              entrypoint: "default",
            },
            eventHandlers,
            listTemplates: context.listTemplates,
          },
        ),
      };
  }

function vueElementToSpec(node: AstNode, context: VueLoweringContext): HawkElementSpec {
  const tag = stringField(node, "tag");
  if (!isHawkTag(tag)) return vueComponentElementToSpec(node, context);
  const id = requiredString(vueAttributeValue(node, "id", context), tag, "id");
  const style = optionalString(vueAttributeValue(node, "class", context));
  const assetPath = optionalString(vueAttributeValue(node, "data-asset", context));
  if (assetPath && isUnsafeAssetPath(assetPath)) {
    throw new Error("vue.asset.path-invalid: Vue asset references must use workspace-relative paths.");
  }
  const key = optionalString(vueAttributeValue(node, "key", context)) ?? id;
  const spec: HawkElementSpec = {
    id,
    kind: kindForTag(tag),
    key,
    refs: optionalString(vueAttributeValue(node, "ref", context))
      ? [optionalString(vueAttributeValue(node, "ref", context)) as string]
      : [],
    styleRefs: style ? [style] : [],
    assetRefs: assetPath ? [{ name: "vue.asset", path: assetPath }] : [],
    events: vueEvents(node, context, id),
    lifecycle: vueLifecycle(node),
    children: vueChildSpecs(node, context, id),
  };
  const props = runtimeProps(node, context, id, "vue");
  const text = vueTextContent(node, context, id);
  if (text) props.text = text;
  return Object.keys(props).length > 0 ? { ...spec, props } : spec;
}

function vueChildSpecs(node: AstNode, context: VueLoweringContext, parentId: string): readonly HawkElementSpec[] {
  const children: HawkElementSpec[] = [];
  let activeConditionalChain: readonly string[] = [];
  for (const child of arrayField(node, "children")) {
    if (!isVueElement(child)) continue;
    const elseIfDirective = vueDirective(child, "else-if");
    const elseDirective = vueDirective(child, "else");
    if (elseIfDirective || elseDirective) {
      if (activeConditionalChain.length === 0) {
        throw new Error("vue.conditional.chain-invalid: v-else and v-else-if require a preceding v-if or v-else-if sibling.");
      }
      let branchSpecs = vueChildNodeSpecs(child, context, parentId);
      if (elseIfDirective) {
        const condition = requiredVueDirectiveExpression(elseIfDirective, "else-if");
        branchSpecs = withVueVisibilityBinding(
          branchSpecs,
          `(${negatedVueConditionChain(activeConditionalChain)}) && (${condition})`,
          context,
          false,
        );
        activeConditionalChain = [...activeConditionalChain, condition];
      } else {
        branchSpecs = withVueVisibilityBinding(
          branchSpecs,
          negatedVueConditionChain(activeConditionalChain),
          context,
          false,
        );
        activeConditionalChain = [];
      }
      for (const spec of branchSpecs) {
        anchorPendingListTemplates(context, parentId, spec.id);
        children.push(spec);
      }
      continue;
    }
    for (const spec of vueChildNodeSpecs(child, context, parentId)) {
      anchorPendingListTemplates(context, parentId, spec.id);
      children.push(spec);
    }
    const ifDirective = vueDirective(child, "if");
    activeConditionalChain = ifDirective ? [requiredVueDirectiveExpression(ifDirective, "if")] : [];
  }
  return children;
}

function vueChildNodeSpecs(child: AstNode, context: VueLoweringContext, parentId: string): readonly HawkElementSpec[] {
  if (isVueSlotElement(child)) {
    return (context.slots.get("default") ?? []).flatMap((slotChild) => vueChildNodeSpecs(slotChild, context, parentId));
  }
  if (!isVueElement(child)) return [];
  const forDirective = vueDirective(child, "for");
  if (forDirective) return expandVueFor(child, forDirective, context, parentId);
  if (isVueHawkElement(child) || isComponentTag(stringField(child, "tag"))) {
    let specs: readonly HawkElementSpec[] = [vueElementToSpec(child, context)];
    const ifDirective = vueDirective(child, "if");
    if (ifDirective) {
      specs = withVueVisibilityBinding(
        specs,
        stringField(ifDirective.exp as AstNode | undefined, "content"),
        context,
        false,
      );
    }
    const showDirective = vueDirective(child, "show");
    if (showDirective) {
      specs = withVueVisibilityBinding(
        specs,
        stringField(showDirective.exp as AstNode | undefined, "content"),
        context,
        false,
      );
    }
    return specs;
  }
  return [];
}

function vueComponentElementToSpec(node: AstNode, context: VueLoweringContext): HawkElementSpec {
  const name = stringField(node, "tag");
  const definition = context.components.get(name);
  if (!definition) {
    throw new Error(`vue.component.unresolved: local component \`${name}\` is not defined in this SFC script.`);
  }
  if (context.componentStack.includes(name)) {
    throw new Error(`vue.component.cycle: local component \`${name}\` recursively expands itself.`);
  }
  const scoped = scopedComponentContext(name, node, definition, context);
  return vueElementToSpec(definition.root, scoped);
}

function scopedComponentContext(
  name: string,
  node: AstNode,
  definition: VueComponentDefinition,
  context: VueLoweringContext,
): VueLoweringContext {
  const scalars = new Map(context.scalars);
  for (const prop of definition.props) {
    const value = vueAttributeValue(node, prop, context);
    if (value !== undefined) scalars.set(prop, value);
  }
  return {
    ...context,
    scalars,
    slots: new Map([...context.slots, ["default", arrayField(node, "children")]]),
    componentStack: [...context.componentStack, name],
  };
}

function expandVueFor(
  template: AstNode,
  directive: AstNode,
  context: VueLoweringContext,
  parentId: string,
): readonly HawkElementSpec[] {
  const parseResult = directive.forParseResult as AstNode | undefined;
  const source = stringField(parseResult?.source as AstNode | undefined, "content");
  const itemName = stringField(parseResult?.value as AstNode | undefined, "content");
  if (!source || !itemName) {
    throw new Error("vue.for.unsupported: Vue v-for must use `item in items` style bindings.");
  }
  const items = context.arrays.get(source);
  if (!items) {
    const initialValue = context.initialDynamicValues.get(source)?.value;
    if (initialValue?.type !== "array") {
      throw new Error(`vue.for.source-unresolved: Vue v-for source \`${source}\` must be a literal array or initial dynamic array.`);
    }
    pushVueReactivity(context, { kind: "keyed-for-each", name: source });
    context.listTemplates.push({
      id: `${parentId}:${source}`,
      parent_id: parentId,
      source,
      item: itemName,
      key: templateKeyExpression(template, itemName, "vue"),
      node: vueElementToListTemplateNode(template, context, itemName),
    });
    queuePendingListTemplateAnchor(context, parentId, `${parentId}:${source}`);
    return [];
  }
  pushVueReactivity(context, { kind: "keyed-for-each", name: source });
  return items.map((item) =>
    vueElementToSpec(template, {
      ...context,
      locals: new Map([...context.locals, [itemName, item]]),
    }),
  );
}

function queuePendingListTemplateAnchor(
  context: VueLoweringContext,
  parentId: string,
  templateId: string,
): void {
  const pending = context.pendingListTemplateAnchors.get(parentId) ?? [];
  pending.push(templateId);
  context.pendingListTemplateAnchors.set(parentId, pending);
}

function anchorPendingListTemplates(
  context: VueLoweringContext,
  parentId: string,
  anchorBefore: string,
): void {
  const pending = context.pendingListTemplateAnchors.get(parentId);
  if (!pending || pending.length === 0) return;
  for (const templateId of pending) {
    const index = context.listTemplates.findIndex((template) => template.id === templateId);
    const template = context.listTemplates[index];
    if (!template || template.anchor_before !== undefined) continue;
    context.listTemplates[index] = {
      ...template,
      anchor_before: anchorBefore,
    };
  }
  context.pendingListTemplateAnchors.delete(parentId);
}

function vueElementToListTemplateNode(
  node: AstNode,
  context: VueLoweringContext,
  itemName: string,
): HawkCompilerListTemplateNodeWire {
  const tag = stringField(node, "tag");
  if (!isHawkTag(tag)) {
    throw new Error(`vue.for.template-unsupported: Vue list templates must render native Hawk elements, found \`${tag}\`.`);
  }
  const id = templateScalarFromAttribute(node, "id", context, itemName, "vue");
  const key = optionalTemplateScalarFromAttribute(node, "key", context, itemName, "vue") ?? id;
  const classAttr = staticTemplateStringAttribute(node, "class", context, "vue");
  const assetPath = staticTemplateStringAttribute(node, "data-asset", context, "vue");
  if (assetPath && isUnsafeAssetPath(assetPath)) {
    throw new Error(`vue.asset.path-invalid: asset path \`${assetPath}\` must be workspace-relative.`);
  }
  const props = templateProps(node, context, itemName, "vue");
  const text = templateTextContent(node, context, itemName, "vue");
  if (text) props.push({ name: "text", value: text });
  return {
    id,
    kind: kindForTag(tag),
    key,
    props,
    refs: staticTemplateStringAttribute(node, "ref", context, "vue")
      ? [staticTemplateStringAttribute(node, "ref", context, "vue") as string]
      : [],
    style_refs: classAttr ? [classAttr] : [],
    asset_refs: assetPath ? [{ name: "vue.asset", path: assetPath }] : [],
    events: vueEvents(node, context, undefined).map((event) => ({
      kind: event.kind,
      handler: event.handler,
      payload_fields: [...payloadFieldsForEvent(event.kind)],
    })),
    lifecycle: vueLifecycle(node).map((lifecycle) => ({
      event: lifecycle.phase,
      handler: lifecycle.handler,
    })),
    children: arrayField(node, "children")
      .filter((child) => isVueElement(child) && isHawkTag(stringField(child, "tag")))
      .map((child) => vueElementToListTemplateNode(child, context, itemName)),
  };
}

function templateProps(
  node: AstNode,
  context: VueLoweringContext,
  itemName: string,
  framework: string,
): { name: string; value: HawkCompilerTemplateScalarWire }[] {
  const props: { name: string; value: HawkCompilerTemplateScalarWire }[] = [];
  for (const name of ["width", "height", ...VISUAL_PROP_NAMES]) {
    const value = optionalTemplateScalarFromAttribute(node, name, context, itemName, framework);
    if (value) props.push({ name, value });
  }
  for (const attr of arrayField(node, "props").filter((prop) => prop.type === 6)) {
    const name = stringField(attr, "name");
    if (!name || RESERVED_RUNTIME_PROP_NAMES.has(name)) continue;
    props.push({ name, value: templateScalarFromStaticAttribute(attr, framework) });
  }
  for (const directive of vueDirectives(node, "bind")) {
    const name = stringField(directive.arg as AstNode | undefined, "content");
    if (!name || RESERVED_RUNTIME_PROP_NAMES.has(name)) continue;
    props.push({ name, value: templateScalarFromBinding(directive, context, itemName, framework) });
  }
  return props;
}

function templateTextContent(
  node: AstNode,
  context: VueLoweringContext,
  itemName: string,
  framework: string,
): HawkCompilerTemplateScalarWire | undefined {
  const children = arrayField(node, "children").filter((child) => child.type === 2 || child.type === 5);
  if (children.length === 0) return undefined;
  if (children.length === 1) {
    const child = children[0];
    if (!child) return undefined;
    if (child.type === 2) {
      const text = String(child.content ?? "").trim();
      return text ? literalTemplateScalar(text) : undefined;
    }
    return templateScalarFromExpression(
      stringField(child.content as AstNode | undefined, "content"),
      context,
      itemName,
      framework,
    );
  }
  return {
    type: "expression",
    expression: children
      .map((child) =>
        child.type === 2
          ? JSON.stringify(String(child.content ?? ""))
          : expressionSource(stringField(child.content as AstNode | undefined, "content"), framework),
      )
      .join(" + "),
  };
}

function templateKeyExpression(node: AstNode, itemName: string, framework: string): string {
  const key = boundTemplateExpression(node, "key");
  if (key) return expressionSource(key, framework);
  const staticKey = staticTemplateStringAttribute(node, "key", undefined, framework);
  if (staticKey) return JSON.stringify(staticKey);
  const id = boundTemplateExpression(node, "id");
  if (id) return expressionSource(id, framework);
  const staticId = staticTemplateStringAttribute(node, "id", undefined, framework);
  if (staticId) return JSON.stringify(staticId);
  return `${itemName}.id`;
}

function templateScalarFromAttribute(
  node: AstNode,
  name: string,
  context: VueLoweringContext,
  itemName: string,
  framework: string,
): HawkCompilerTemplateScalarWire {
  const value = optionalTemplateScalarFromAttribute(node, name, context, itemName, framework);
  if (!value) throw new Error(`${framework}.list-template.attribute-required: list template nodes require \`${name}\`.`);
  return value;
}

function optionalTemplateScalarFromAttribute(
  node: AstNode,
  name: string,
  context: VueLoweringContext,
  itemName: string,
  framework: string,
): HawkCompilerTemplateScalarWire | undefined {
  const staticAttr = arrayField(node, "props").find((prop) => prop.type === 6 && prop.name === name);
  if (staticAttr) return templateScalarFromStaticAttribute(staticAttr, framework);
  const bound = vueDirectives(node, "bind").find(
    (directive) => stringField(directive.arg as AstNode | undefined, "content") === name,
  );
  return bound ? templateScalarFromBinding(bound, context, itemName, framework) : undefined;
}

function templateScalarFromStaticAttribute(attr: AstNode, framework: string): HawkCompilerTemplateScalarWire {
  const staticValue = attr.value as AstNode | undefined;
  if (!staticValue) return literalTemplateScalar(true);
  if (typeof staticValue.content === "string") return literalTemplateScalar(staticValue.content);
  throw new Error(`${framework}.list-template.attribute-unsupported: list template static attributes must be scalar values.`);
}

function templateScalarFromBinding(
  directive: AstNode,
  context: VueLoweringContext,
  itemName: string,
  framework: string,
): HawkCompilerTemplateScalarWire {
  const expression = stringField(directive.exp as AstNode | undefined, "content");
  if (!expression) {
    throw new Error(`${framework}.list-template.attribute-unsupported: bound list template attributes require an expression.`);
  }
  return templateScalarFromExpression(expression, context, itemName, framework);
}

function templateScalarFromExpression(
  expression: string | undefined,
  context: VueLoweringContext,
  itemName: string,
  framework: string,
): HawkCompilerTemplateScalarWire {
  const staticValue = staticVueExpressionValue(expression, context);
  if (staticValue !== undefined) return literalTemplateScalar(staticValue);
  const source = expressionSource(expression, framework);
  if (!expressionDependencies(source).includes(itemName)) {
    throw new Error(`${framework}.list-template.expression-unsupported: list template expressions must depend on \`${itemName}\`.`);
  }
  return { type: "expression", expression: source };
}

function literalTemplateScalar(value: string | number | boolean): HawkCompilerTemplateScalarWire {
  if (typeof value === "string") return { type: "literal", value: { type: "string", value } };
  if (typeof value === "boolean") return { type: "literal", value: { type: "bool", value } };
  return { type: "literal", value: { type: "number", value } };
}

function boundTemplateExpression(node: AstNode, name: string): string | undefined {
  const bound = vueDirectives(node, "bind").find(
    (directive) => stringField(directive.arg as AstNode | undefined, "content") === name,
  );
  return stringField(bound?.exp as AstNode | undefined, "content") || undefined;
}

function staticTemplateStringAttribute(
  node: AstNode,
  name: string,
  context: VueLoweringContext | undefined,
  framework: string,
): string | undefined {
  const staticAttr = arrayField(node, "props").find((prop) => prop.type === 6 && prop.name === name);
  const staticValue = staticAttr?.value as AstNode | undefined;
  if (typeof staticValue?.content === "string") return staticValue.content;
  const expression = boundTemplateExpression(node, name);
  if (!expression) return undefined;
  if (!context) return undefined;
  const value = staticVueExpressionValue(expression, context);
  if (typeof value === "string") return value;
  throw new Error(`${framework}.list-template.attribute-unsupported: list template \`${name}\` must resolve to a static string.`);
}

function pushVueReactivity(context: VueLoweringContext, binding: HawkCompilerReactiveBindingWire): void {
  if (!context.reactivity.some((item) => item.kind === binding.kind && item.name === binding.name)) {
    context.reactivity.push(binding);
  }
}

function vueWatchReactivityBindings(source: string): HawkCompilerReactiveBindingWire[] {
  const program = parseVueScriptProgram(source);
  if (!program) return [];
  const bindings: HawkCompilerReactiveBindingWire[] = [];
  for (const statement of arrayField(program, "body")) {
    if (statement.type !== "ExpressionStatement") continue;
    const expression = statement.expression as AstNode | undefined;
    if (expression?.type !== "CallExpression") continue;
    const name = callName(expression.callee as AstNode | undefined);
    if (name !== "watch") continue;
    const args = arrayField(expression, "arguments");
    const sourceName = vueWatchSourceName(args[0]);
    const handlerName = identifierName(args[1]);
    if (!handlerName) {
      throw new Error("vue.watch.unsupported: watch handlers must be stable function identifiers.");
    }
    bindings.push({ kind: "effect", name: `watch:${sourceName}:${handlerName}` });
  }
  return bindings;
}

function vueWatchSourceName(source: AstNode | undefined): string {
  const name = identifierName(source);
  if (name) return name;
  throw new Error("vue.watch.unsupported: watch sources must be stable identifiers.");
}

function payloadFieldsForEvent(kind: HawkEventSpec["kind"]) {
  switch (kind) {
    case "pointer.drag":
    case "pointer.wheel":
      return ["position", "delta"] as const;
    case "pointer.press":
    case "pointer.release":
    case "pointer.move":
    case "pointer.enter":
    case "pointer.leave":
      return ["position"] as const;
    case "keyboard.key-down":
    case "keyboard.key-up":
      return ["key"] as const;
    case "keyboard.text-input":
    case "input.value-changed":
    case "input.value-committed":
      return ["value"] as const;
    default:
      return [] as const;
  }
}

function vueEvents(
  node: AstNode,
  context: VueLoweringContext,
  nodeId: string | undefined,
): readonly HawkEventSpec[] {
  const events: HawkEventSpec[] = [];
  for (const directive of vueDirectives(node, "on")) {
    const event = stringField(directive.arg as AstNode | undefined, "content");
    const eventKind = VUE_EVENT_DIRECTIVES.get(event);
    if (eventKind) {
      events.push({ kind: eventKind, handler: vueHandlerName(directive) });
    } else if (!VUE_LIFECYCLE_DIRECTIVES.has(event)) {
      throw new Error(`vue.event.unsupported: Vue event \`${event}\` is not part of the native event contract.`);
    }
  }
  const model = vueDirective(node, "model");
  if (model) {
    if (!nodeId) {
      throw new Error("vue.model.list-template-unsupported: v-model in list templates requires a stable static node id.");
    }
    addVueModelBinding(model, context, nodeId, events);
  }
  return events;
}

function vueLifecycle(node: AstNode): readonly HawkLifecycleSpec[] {
  const lifecycle: HawkLifecycleSpec[] = [];
  for (const directive of vueDirectives(node, "on")) {
    const event = stringField(directive.arg as AstNode | undefined, "content");
    const phase = VUE_LIFECYCLE_DIRECTIVES.get(event);
    if (phase) lifecycle.push({ phase, handler: vueHandlerName(directive) });
  }
  return lifecycle;
}

function addVueModelBinding(
  directive: AstNode,
  context: VueLoweringContext,
  nodeId: string,
  events: HawkEventSpec[],
): void {
  const expression = stringField(directive.exp as AstNode | undefined, "content");
  if (!expression.trim()) {
    throw new Error("vue.model.unsupported: v-model requires a stable model expression.");
  }
  const source = expressionSource(expression, "vue");
  const targetName = vueModelTargetName(source);
  context.dynamicBindings.push({
    node_id: nodeId,
    target: { type: "prop", name: "value" },
    expression: source,
    dependencies: expressionDependencies(source),
  });
  const handler = `${nodeId}:v-model`;
  events.push({ kind: "input.value-changed", handler });
  if (!context.syntheticEventHandlers.some((item) => item.name === handler)) {
    context.syntheticEventHandlers.push({
      name: handler,
      actions: [
        {
          type: "set_dynamic_expression",
          name: targetName,
          expression: "event.value",
          dependencies: ["event"],
        },
      ],
    });
  }
}

function vueModelTargetName(expression: string): string {
  const parsed = parseExpression(expression, {
    sourceType: "module",
    plugins: ["typescript"],
  }) as unknown as AstNode;
  if (parsed.type === "Identifier") {
    const name = identifierName(parsed);
    if (name) return name;
  }
  if (parsed.type === "MemberExpression") {
    const object = identifierName(parsed.object as AstNode | undefined);
    const property = identifierName(parsed.property as AstNode | undefined);
    if (object && property === "value") return object;
  }
  throw new Error("vue.model.unsupported: v-model must target an identifier or ref `.value` expression.");
}

function vueLifecycleApiCalls(source: string): readonly HawkLifecycleSpec[] {
  const lifecycle: HawkLifecycleSpec[] = [];
  const program = parseVueScriptProgram(source);
  for (const statement of arrayField(program, "body")) {
    if (statement.type !== "ExpressionStatement") continue;
    const call = statement.expression as AstNode | undefined;
    if (call?.type !== "CallExpression") continue;
    const name = callName(call.callee as AstNode | undefined);
    const argument = arrayField(call, "arguments")[0];
    if (name === "onMounted") {
      for (const handler of lifecycleHandlerNamesFromArgument(argument, "vue", "onMounted")) {
        pushLifecycle(lifecycle, "mounted", handler);
      }
      continue;
    }
    if (name === "onUnmounted") {
      for (const handler of lifecycleHandlerNamesFromArgument(argument, "vue", "onUnmounted")) {
        pushLifecycle(lifecycle, "unmounted", handler);
      }
    }
  }
  return lifecycle;
}

function withRootLifecycle(
  root: HawkElementSpec,
  lifecycle: readonly HawkLifecycleSpec[],
): HawkElementSpec {
  if (lifecycle.length === 0) return root;
  const merged: HawkLifecycleSpec[] = [...(root.lifecycle ?? [])];
  for (const item of lifecycle) pushLifecycle(merged, item.phase, item.handler);
  return { ...root, lifecycle: merged };
}

function pushLifecycle(
  lifecycle: HawkLifecycleSpec[],
  phase: HawkLifecycleSpec["phase"],
  handler: string,
): void {
  if (!lifecycle.some((item) => item.phase === phase && item.handler === handler)) {
    lifecycle.push({ phase, handler });
  }
}

function lifecycleHandlerNamesFromArgument(
  argument: AstNode | undefined,
  framework: string,
  api: string,
): readonly string[] {
  const direct = identifierName(argument);
  if (direct) return [direct];
  const body = functionLikeBody(argument);
  if (!body) {
    throw new Error(`${framework}.lifecycle.unsupported: ${api} must reference a stable handler identifier or call one.`);
  }
  if (body.type === "CallExpression") {
    const name = callName(body.callee as AstNode | undefined);
    if (name) return [name];
  }
  if (body.type === "BlockStatement") {
    const names: string[] = [];
    for (const statement of arrayField(body, "body")) {
      if (statement.type === "ReturnStatement") continue;
      if (statement.type !== "ExpressionStatement") {
        throw new Error(`${framework}.lifecycle.unsupported: ${api} handlers may only call stable lifecycle functions.`);
      }
      const expression = statement.expression as AstNode | undefined;
      if (expression?.type !== "CallExpression") {
        throw new Error(`${framework}.lifecycle.unsupported: ${api} handlers may only call stable lifecycle functions.`);
      }
      const name = callName(expression.callee as AstNode | undefined);
      if (!name) {
        throw new Error(`${framework}.lifecycle.unsupported: ${api} handlers may only call stable lifecycle functions.`);
      }
      names.push(name);
    }
    if (names.length > 0) return names;
  }
  throw new Error(`${framework}.lifecycle.unsupported: ${api} must reference a stable handler identifier or call one.`);
}

function functionLikeBody(node: AstNode | undefined): AstNode | undefined {
  if (
    node?.type === "ArrowFunctionExpression"
    || node?.type === "FunctionExpression"
    || node?.type === "FunctionDeclaration"
  ) {
    return node.body as AstNode | undefined;
  }
  return undefined;
}

function parseVueScriptProgram(source: string): AstNode | undefined {
  if (!source.trim()) return undefined;
  const ast = parseScript(source, {
    sourceType: "module",
    plugins: ["typescript"],
  }) as unknown as AstNode;
  return ast.program as AstNode | undefined;
}

function callName(callee: AstNode | undefined): string | undefined {
  if (callee?.type === "MemberExpression") return identifierName(callee.property as AstNode | undefined);
  return identifierName(callee);
}

function eventHandlerArtifactsForSpec(
  root: HawkElementSpec,
  script: string,
  listTemplates: readonly HawkCompilerListTemplateWire[],
  syntheticEventHandlers: readonly HawkCompilerEventHandlerWire[],
): readonly HawkCompilerEventHandlerWire[] {
  const declarations = handlerDeclarationsFromScript(script);
  const lifecycleOnlyHandlers = lifecycleOnlyHandlerNames(root, listTemplates);
  const syntheticByName = new Map(syntheticEventHandlers.map((handler) => [handler.name, handler]));
  return referencedHandlerNames(root, listTemplates).flatMap((name) => {
    const synthetic = syntheticByName.get(name);
    if (synthetic) return synthetic;
    const declaration = declarations.get(name);
    if (!declaration) {
      throw new Error(`vue.handler.missing: event handler \`${name}\` must be declared in the component script.`);
    }
    const actions = handlerActions(name, declaration, lifecycleOnlyHandlers.has(name));
    if (actions.length === 0) return [];
    return {
      name,
      actions,
    };
  });
}

function lifecycleOnlyHandlerNames(
  root: HawkElementSpec,
  listTemplates: readonly HawkCompilerListTemplateWire[],
): ReadonlySet<string> {
  const eventHandlers = new Set<string>();
  const lifecycleHandlers = new Set<string>();
  const visit = (element: HawkElementSpec): void => {
    for (const event of element.events ?? []) eventHandlers.add(event.handler);
    for (const lifecycle of element.lifecycle ?? []) lifecycleHandlers.add(lifecycle.handler);
    for (const child of element.children ?? []) visit(child);
  };
  visit(root);
  for (const template of listTemplates) {
    visitListTemplateHandlerNames(template.node, eventHandlers, lifecycleHandlers);
  }
  for (const name of eventHandlers) lifecycleHandlers.delete(name);
  return lifecycleHandlers;
}

function referencedHandlerNames(
  root: HawkElementSpec,
  listTemplates: readonly HawkCompilerListTemplateWire[],
): readonly string[] {
  const names: string[] = [];
  const seen = new Set<string>();
  const push = (handler: string): void => {
    if (!seen.has(handler)) {
      seen.add(handler);
      names.push(handler);
    }
  };
  const visit = (element: HawkElementSpec): void => {
    for (const event of element.events ?? []) push(event.handler);
    for (const lifecycle of element.lifecycle ?? []) push(lifecycle.handler);
    for (const child of element.children ?? []) visit(child);
  };
  visit(root);
  for (const template of listTemplates) {
    visitListTemplateHandlers(template.node, push);
  }
  return names;
}

function visitListTemplateHandlerNames(
  node: HawkCompilerListTemplateNodeWire,
  eventHandlers: Set<string>,
  lifecycleHandlers: Set<string>,
): void {
  for (const event of node.events) eventHandlers.add(event.handler);
  for (const lifecycle of node.lifecycle) lifecycleHandlers.add(lifecycle.handler);
  for (const child of node.children) visitListTemplateHandlerNames(child, eventHandlers, lifecycleHandlers);
}

function visitListTemplateHandlers(
  node: HawkCompilerListTemplateNodeWire,
  push: (handler: string) => void,
): void {
  for (const event of node.events) push(event.handler);
  for (const lifecycle of node.lifecycle) push(lifecycle.handler);
  for (const child of node.children) visitListTemplateHandlers(child, push);
}

function handlerDeclarationsFromScript(source: string): ReadonlyMap<string, AstNode> {
  const declarations = new Map<string, AstNode>();
  const program = parseVueScriptProgram(source);
    for (const statement of arrayField(program, "body")) {
    if (statement.type === "FunctionDeclaration") {
      const name = identifierName(statement.id as AstNode | undefined);
      if (name) declarations.set(name, statement);
      continue;
    }
    if (statement.type !== "VariableDeclaration") continue;
    for (const declaration of arrayField(statement, "declarations")) {
      const name = identifierName(declaration.id as AstNode | undefined);
      const init = declaration.init as AstNode | undefined;
      if (name && (init?.type === "ArrowFunctionExpression" || init?.type === "FunctionExpression")) {
        declarations.set(name, init);
      }
    }
  }
  return declarations;
}

function handlerActions(
  handler: string,
  declaration: AstNode,
  allowEmpty: boolean,
): readonly HawkCompilerEventHandlerActionWire[] {
  const body = declaration.body as AstNode | undefined;
  if (!body) {
    throw new Error(`vue.handler.unsupported: event handler \`${handler}\` must have an executable body.`);
  }
  if (body.type !== "BlockStatement") {
    return [handlerActionFromExpression(handler, body)];
  }
  const actions = arrayField(body, "body").map((statement) => {
    if (statement.type !== "ExpressionStatement") {
      throw new Error(`vue.handler.unsupported: event handler \`${handler}\` contains unsupported statements.`);
    }
    return handlerActionFromExpression(handler, statement.expression as AstNode | undefined);
  });
    if (actions.length === 0 && !allowEmpty) {
      throw new Error(`vue.handler.unsupported: event handler \`${handler}\` must contain at least one action.`);
    }
  return actions;
}

function handlerActionFromExpression(handler: string, expression: AstNode | undefined): HawkCompilerEventHandlerActionWire {
  if (expression?.type === "AssignmentExpression" && expression.operator === "=") {
    const name = assignmentTargetName(expression.left as AstNode | undefined);
    if (!name) {
      throw new Error(`vue.handler.unsupported: event handler \`${handler}\` assignment target must be a dynamic value or ref value.`);
    }
    return dynamicUpdateAction(name, expression.right as AstNode | undefined);
  }
  throw new Error(`vue.handler.unsupported: event handler \`${handler}\` must assign a dynamic value.`);
}

function assignmentTargetName(target: AstNode | undefined): string | undefined {
  if (target?.type === "Identifier") return identifierName(target);
  if (target?.type !== "MemberExpression") return undefined;
  const object = identifierName(target.object as AstNode | undefined);
  const property = identifierName(target.property as AstNode | undefined);
  return object && property === "value" ? object : undefined;
}

function dynamicUpdateAction(
  name: string,
  expression: AstNode | undefined,
): HawkCompilerEventHandlerActionWire {
  const value = literalDynamicValue(expression);
  if (value) {
    return { type: "set_dynamic_value", name, value };
  }
  const source = expressionSourceFromAst(expression, "vue");
  return {
    type: "set_dynamic_expression",
    name,
    expression: source,
    dependencies: expressionDependencies(source),
  };
}

function expressionSourceFromAst(expression: AstNode | undefined, framework: string): string {
  if (!expression) {
    throw new Error(`${framework}.expression.unsupported: handler actions require an expression.`);
  }
  const source = generate(expression as never, { concise: true }).code.trim();
  if (!source) {
    throw new Error(`${framework}.expression.unsupported: handler actions require a printable expression.`);
  }
  return source;
}

function vueAttributeValue(
  node: AstNode,
  name: string,
  context: VueLoweringContext,
): string | number | boolean | undefined {
  const staticAttr = arrayField(node, "props").find((prop) => prop.type === 6 && prop.name === name);
  const staticValue = staticAttr?.value as AstNode | undefined;
  if (typeof staticValue?.content === "string") return staticValue.content;

  const bound = vueDirectives(node, "bind").find(
    (directive) => stringField(directive.arg as AstNode | undefined, "content") === name,
  );
  const expression = stringField(bound?.exp as AstNode | undefined, "content");
  return expression ? evaluateVueExpression(expression, context) : undefined;
}

function vueTextContent(
  node: AstNode,
  context: VueLoweringContext,
  nodeId: string,
): string | undefined {
  const staticValues: string[] = [];
  const dynamicExpressionParts: string[] = [];
  const dependencies = new Set<string>();
  let hasDynamicExpression = false;

  for (const child of arrayField(node, "children")) {
    if (child.type === 2) {
      const value = String(child.content ?? "").trim();
      if (value.length > 0) {
        staticValues.push(value);
        dynamicExpressionParts.push(JSON.stringify(value));
      }
      continue;
    }
    if (child.type !== 5) continue;
    const expression = stringField((child.content as AstNode | undefined), "content");
    const staticValue = staticVueExpressionValue(expression, context);
    if (staticValue !== undefined) {
      const value = String(staticValue);
      if (value.length > 0) {
        staticValues.push(value);
        dynamicExpressionParts.push(JSON.stringify(value));
      }
      continue;
    }
    hasDynamicExpression = true;
    dynamicExpressionParts.push(expressionSource(expression, "vue"));
    for (const dependency of expressionDependencies(expression)) {
      dependencies.add(dependency);
    }
  }

  if (hasDynamicExpression) {
    context.dynamicBindings.push({
      node_id: nodeId,
      target: { type: "prop", name: "text" },
      expression: dynamicExpressionParts.join(" + "),
      dependencies: [...dependencies],
    });
    return undefined;
  }
  return staticValues.length > 0 ? staticValues.join("") : undefined;
}

function staticVueExpressionValue(
  expression: string | undefined,
  context: VueLoweringContext,
): string | number | boolean | undefined {
  try {
    return evaluateVueExpression(expression, context);
  } catch (error) {
    if (error instanceof Error && error.message.startsWith("vue.expression.unsupported")) {
      return undefined;
    }
    throw error;
  }
}

function layoutProps(
  node: AstNode,
  context: VueLoweringContext,
  nodeId: string,
  framework: string,
): Record<string, string | number | boolean> {
  const props: Record<string, string | number | boolean> = {};
  for (const name of ["width", "height"]) {
    const value = dynamicLayoutAttributeValue(node, name, context, nodeId, framework);
    if (value !== undefined) props[name] = value;
  }
  return props;
}

function runtimeProps(
  node: AstNode,
  context: VueLoweringContext,
  nodeId: string,
  framework: string,
): Record<string, string | number | boolean> {
  const props = layoutProps(node, context, nodeId, framework);
  for (const name of VISUAL_PROP_NAMES) {
    const value = dynamicVisualAttributeValue(node, name, context, nodeId, framework);
    if (value !== undefined) props[name] = value;
  }
  for (const attr of arrayField(node, "props").filter((prop) => prop.type === 6)) {
    const name = stringField(attr, "name");
    if (!name || RESERVED_RUNTIME_PROP_NAMES.has(name)) continue;
    const value = dynamicRuntimeVueStaticAttributeValue(attr, name, nodeId, framework);
    if (value !== undefined) props[name] = value;
  }
  for (const directive of vueDirectives(node, "bind")) {
    const name = stringField(directive.arg as AstNode | undefined, "content");
    if (!name || RESERVED_RUNTIME_PROP_NAMES.has(name)) continue;
    const value = dynamicRuntimeVueBindingValue(directive, name, context, nodeId, framework);
    if (value !== undefined) props[name] = value;
  }
  return props;
}

function withVueVisibilityBinding(
  specs: readonly HawkElementSpec[],
  expression: string | undefined,
  context: VueLoweringContext,
  negate: boolean,
): readonly HawkElementSpec[] {
  const dependencies = expressionDependencies(expression);
  if (dependencies.length > 0) {
    const source = expressionSource(expression, "vue");
    const visibleExpression = negate ? `!(${source})` : source;
    for (const spec of specs) {
      mergeDynamicVisibilityBinding(context, spec.id, visibleExpression, dependencies);
    }
    return specs;
  }

  const staticValue = staticVueExpressionValue(expression, context);
  if (typeof staticValue !== "boolean") {
    throw new Error("vue.conditional.unsupported: Vue visibility directives must use boolean expressions.");
  }
  return specs.map((spec) => withStaticVisibility(spec, negate ? !staticValue : staticValue));
}

function requiredVueDirectiveExpression(directive: AstNode, name: string): string {
  const expression = stringField(directive.exp as AstNode | undefined, "content");
  if (!expression.trim()) {
    throw new Error(`vue.conditional.unsupported: v-${name} requires a boolean expression.`);
  }
  return expressionSource(expression, "vue");
}

function negatedVueConditionChain(conditions: readonly string[]): string {
  return conditions.map((condition) => `!(${condition})`).join(" && ");
}

function mergeDynamicVisibilityBinding(
  context: VueLoweringContext,
  nodeId: string,
  expression: string,
  dependencies: readonly string[],
): void {
  const index = context.dynamicBindings.findIndex(
    (binding) =>
      binding.node_id === nodeId
      && binding.target.type === "prop"
      && binding.target.name === "visible",
  );
  if (index < 0) {
    context.dynamicBindings.push({
      node_id: nodeId,
      target: { type: "prop", name: "visible" },
      expression,
      dependencies,
    });
    return;
  }
  const existing = context.dynamicBindings[index];
  if (!existing) {
    throw new Error("vue.visibility.internal: visible binding index disappeared during merge.");
  }
  context.dynamicBindings[index] = {
    node_id: existing.node_id,
    target: existing.target,
    expression: `(${existing.expression}) && (${expression})`,
    dependencies: uniqueStrings([...existing.dependencies, ...dependencies]),
  };
}

function withStaticVisibility(spec: HawkElementSpec, visible: boolean): HawkElementSpec {
  const current = spec.props?.visible;
  const combined = typeof current === "boolean" ? current && visible : visible;
  return {
    ...spec,
    props: { ...(spec.props ?? {}), visible: combined },
  };
}

function uniqueStrings(values: readonly string[]): readonly string[] {
  return [...new Set(values)];
}

function dynamicRuntimeVueStaticAttributeValue(
  attr: AstNode,
  name: string,
  nodeId: string,
  framework: string,
): string | number | boolean | undefined {
  const staticValue = attr.value as AstNode | undefined;
  if (!staticValue) return true;
  if (typeof staticValue.content === "string") return staticValue.content;
  throw new Error(`${framework}.attribute.unsupported: prop \`${name}\` on \`${nodeId}\` must be a static scalar or expression.`);
}

function dynamicRuntimeVueBindingValue(
  directive: AstNode,
  name: string,
  context: VueLoweringContext,
  nodeId: string,
  framework: string,
): string | number | boolean | undefined {
  const expression = stringField(directive.exp as AstNode | undefined, "content");
  if (!expression) {
    throw new Error(`${framework}.attribute.unsupported: prop \`${name}\` on \`${nodeId}\` requires a binding expression.`);
  }
  const staticExpression = staticVueExpressionValue(expression, context);
  if (staticExpression !== undefined) return staticExpression;
  context.dynamicBindings.push({
    node_id: nodeId,
    target: { type: "prop", name },
    expression: expressionSource(expression, framework),
    dependencies: expressionDependencies(expression),
  });
  return undefined;
}

function dynamicVisualAttributeValue(
  node: AstNode,
  name: string,
  context: VueLoweringContext,
  nodeId: string,
  framework: string,
): string | number | boolean | undefined {
  const staticAttr = arrayField(node, "props").find((prop) => prop.type === 6 && prop.name === name);
  const staticValue = staticAttr?.value as AstNode | undefined;
  if (typeof staticValue?.content === "string") return staticValue.content;
  if (staticAttr) {
    throw new Error(`${framework}.attribute.unsupported: visual prop \`${name}\` on \`${nodeId}\` must be a static scalar or expression.`);
  }

  const bound = vueDirectives(node, "bind").find(
    (directive) => stringField(directive.arg as AstNode | undefined, "content") === name,
  );
  const expression = stringField(bound?.exp as AstNode | undefined, "content");
  if (bound && !expression) {
    throw new Error(`${framework}.attribute.unsupported: visual prop \`${name}\` on \`${nodeId}\` requires a binding expression.`);
  }
  if (!expression) return undefined;
  const staticExpression = staticVueExpressionValue(expression, context);
  if (staticExpression !== undefined) return staticExpression;
  context.dynamicBindings.push({
    node_id: nodeId,
    target: { type: "prop", name },
    expression: expressionSource(expression, framework),
    dependencies: expressionDependencies(expression),
  });
  return undefined;
}

function dynamicLayoutAttributeValue(
  node: AstNode,
  name: string,
  context: VueLoweringContext,
  nodeId: string,
  framework: string,
): number | undefined {
  const staticAttr = arrayField(node, "props").find((prop) => prop.type === 6 && prop.name === name);
  const staticValue = staticAttr?.value as AstNode | undefined;
  if (typeof staticValue?.content === "string") return layoutNumber(staticValue.content, nodeId, name, framework);

  const bound = vueDirectives(node, "bind").find(
    (directive) => stringField(directive.arg as AstNode | undefined, "content") === name,
  );
  const expression = stringField(bound?.exp as AstNode | undefined, "content");
  if (!expression) return undefined;
  const staticExpression = staticVueExpressionValue(expression, context);
  if (staticExpression !== undefined) return layoutNumber(staticExpression, nodeId, name, framework);
  context.dynamicBindings.push({
    node_id: nodeId,
    target: { type: "prop", name },
    expression: expressionSource(expression, framework),
    dependencies: expressionDependencies(expression),
  });
  return undefined;
}

function layoutNumber(
  value: unknown,
  nodeId: string,
  name: string,
  framework: string,
): number {
  const parsed = typeof value === "number"
    ? value
    : typeof value === "string" && value.trim() !== ""
      ? Number(value)
      : Number.NaN;
  if (!Number.isFinite(parsed) || parsed < 0) {
    throw new Error(`${framework}.layout.invalid-number: layout prop \`${name}\` on \`${nodeId}\` must be finite and non-negative.`);
  }
  return parsed;
}

function vueHandlerName(directive: AstNode): string {
  const handler = stringField(directive.exp as AstNode | undefined, "content");
  if (!handler) throw new Error("vue.handler.unsupported: event handlers must be stable identifiers.");
  return handler;
}

function vueDirective(node: AstNode, name: string): AstNode | undefined {
  return vueDirectives(node, name)[0];
}

function vueDirectives(node: AstNode, name: string): readonly AstNode[] {
  return arrayField(node, "props").filter((prop) => prop.type === 7 && prop.name === name);
}

function literalArraysFromScript(source: string): ReadonlyMap<string, readonly LiteralRecord[]> {
  if (!source.trim()) return new Map();
  const ast = parseScript(source, {
    sourceType: "module",
    plugins: ["typescript"],
  }) as unknown as AstNode;
  return literalArraysFromProgram(ast.program as AstNode);
}

function componentDefinitionsFromScript(source: string): ReadonlyMap<string, VueComponentDefinition> {
  if (!source.trim()) return new Map();
  const ast = parseScript(source, {
    sourceType: "module",
    plugins: ["typescript"],
  }) as unknown as AstNode;
  return componentDefinitionsFromProgram(ast.program as AstNode);
}

function initialDynamicValuesFromScript(source: string): ReadonlyMap<string, HawkCompilerInitialDynamicValueWire> {
  if (!source.trim()) return new Map();
  const ast = parseScript(source, {
    sourceType: "module",
    plugins: ["typescript"],
  }) as unknown as AstNode;
  return initialDynamicValuesFromProgram(ast.program as AstNode);
}

function scalarValuesFromScript(source: string): ReadonlyMap<string, string | number | boolean> {
  if (!source.trim()) return new Map();
  const ast = parseScript(source, {
    sourceType: "module",
    plugins: ["typescript"],
  }) as unknown as AstNode;
  const values = new Map<string, string | number | boolean>();
  for (const statement of arrayField(ast.program as AstNode | undefined, "body")) {
    if (statement.type !== "VariableDeclaration") continue;
    for (const declaration of arrayField(statement, "declarations")) {
      const name = identifierName(declaration.id as AstNode | undefined);
      const value = literalValue(declaration.init as AstNode | undefined);
      if (name && value !== undefined) values.set(name, value);
    }
  }
  return values;
}

function componentDefinitionsFromProgram(program: AstNode): ReadonlyMap<string, VueComponentDefinition> {
  const components = new Map<string, VueComponentDefinition>();
  for (const statement of arrayField(program, "body")) {
    if (statement.type !== "VariableDeclaration") continue;
    for (const declaration of arrayField(statement, "declarations")) {
      const name = identifierName(declaration.id as AstNode | undefined);
      const definition = vueComponentDefinition(declaration.init as AstNode | undefined);
      if (name && isComponentTag(name) && definition) components.set(name, definition);
    }
  }
  return components;
}

function vueComponentDefinition(node: AstNode | undefined): VueComponentDefinition | undefined {
  if (node?.type !== "ObjectExpression") return undefined;
  let template: string | undefined;
  let props: readonly string[] = [];
  for (const property of arrayField(node, "properties")) {
    const key = identifierName(property.key as AstNode | undefined) ?? literalString(property.key as AstNode | undefined);
    if (key === "template") {
      const value = literalValue(property.value as AstNode | undefined);
      if (typeof value === "string") template = value;
    } else if (key === "props") {
      props = stringArrayLiteral(property.value as AstNode | undefined);
    }
  }
  if (!template) return undefined;
  const parsed = parseTemplate(template) as unknown as AstNode;
  const root = arrayField(parsed, "children").find(isVueHawkElement);
  if (!root) {
    throw new Error("vue.component.template-invalid: local component templates must render one hawk root element.");
  }
  return { props, root };
}

function stringArrayLiteral(node: AstNode | undefined): readonly string[] {
  if (node?.type !== "ArrayExpression") return [];
  return arrayField(node, "elements").map((element) => {
    const value = literalValue(element);
    if (typeof value !== "string") {
      throw new Error("vue.component.props-unsupported: local component props arrays must contain strings.");
    }
    return value;
  });
}

function literalArraysFromProgram(program: AstNode): ReadonlyMap<string, readonly LiteralRecord[]> {
  const arrays = new Map<string, readonly LiteralRecord[]>();
  for (const statement of arrayField(program, "body")) {
    if (statement.type !== "VariableDeclaration") continue;
    for (const declaration of arrayField(statement, "declarations")) {
      const name = identifierName(declaration.id as AstNode | undefined);
      const values = literalObjectArray(declaration.init as AstNode | undefined);
      if (name && values) arrays.set(name, values);
    }
  }
  return arrays;
}

function initialDynamicValuesFromProgram(program: AstNode): ReadonlyMap<string, HawkCompilerInitialDynamicValueWire> {
  const values = new Map<string, HawkCompilerInitialDynamicValueWire>();
  for (const statement of arrayField(program, "body")) {
    if (statement.type !== "VariableDeclaration") continue;
    for (const declaration of arrayField(statement, "declarations")) {
      const name = identifierName(declaration.id as AstNode | undefined);
      const value = vueInitialDynamicValue(declaration.init as AstNode | undefined);
      if (name && value) values.set(name, { name, mode: "value", value });
    }
  }
  return values;
}

function vueInitialDynamicValue(node: AstNode | undefined): HawkCompilerDynamicValueWire | undefined {
  const directValue = literalDynamicValue(node);
  if (directValue) return directValue;
  if (node?.type !== "CallExpression") return undefined;
    const callee = identifierName(node.callee as AstNode | undefined);
    const args = node.arguments as AstNode[] | undefined;
    if (callee === "ref") return literalDynamicValue(args?.[0]);
    if (callee === "reactive") return literalDynamicValue(args?.[0]);
    if (callee !== "computed") return undefined;
  const callback = args?.[0];
  if (callback?.type !== "ArrowFunctionExpression" && callback?.type !== "FunctionExpression") {
    return undefined;
  }
  if (callback.body && (callback.body as AstNode).type !== "BlockStatement") {
    return literalDynamicValue(callback.body as AstNode);
  }
  for (const statement of arrayField(callback.body as AstNode | undefined, "body")) {
    if (statement.type === "ReturnStatement") {
      return literalDynamicValue(statement.argument as AstNode | undefined);
    }
  }
  return undefined;
}

function literalObjectArray(node: AstNode | undefined): readonly LiteralRecord[] | undefined {
  if (!node || node.type !== "ArrayExpression") return undefined;
  return arrayField(node, "elements").map((item) => {
    if (item.type !== "ObjectExpression") {
      throw new Error("vue.literal-array.unsupported: v-for sources must be arrays of literal objects.");
    }
    const record: Record<string, string | number | boolean> = {};
    for (const property of arrayField(item, "properties")) {
      const key = identifierName(property.key as AstNode | undefined) ?? literalString(property.key as AstNode | undefined);
      const value = literalValue(property.value as AstNode | undefined);
      if (!key || value === undefined) {
        throw new Error("vue.literal-array.unsupported: literal object properties must be scalar values.");
      }
      record[key] = value;
    }
    return record;
  });
}

function literalDynamicValue(node: AstNode | undefined): HawkCompilerDynamicValueWire | undefined {
  if (!node) return undefined;
  if (node.type === "NullLiteral") return { type: "null" };
  const literal = literalValue(node);
  if (typeof literal === "string") return { type: "string", value: literal };
  if (typeof literal === "boolean") return { type: "bool", value: literal };
  if (typeof literal === "number" && Number.isFinite(literal)) return { type: "number", value: literal };
  if (node.type === "ArrayExpression") {
    const values: HawkCompilerDynamicValueWire[] = [];
    for (const element of arrayField(node, "elements")) {
      const value = literalDynamicValue(element);
      if (!value) return undefined;
      values.push(value);
    }
    return { type: "array", value: values };
  }
  if (node.type === "ObjectExpression") {
    const values: Record<string, HawkCompilerDynamicValueWire> = {};
    for (const property of arrayField(node, "properties")) {
      const key = identifierName(property.key as AstNode | undefined) ?? literalString(property.key as AstNode | undefined);
      const value = literalDynamicValue(property.value as AstNode | undefined);
      if (!key || !value) return undefined;
      values[key] = value;
    }
    return { type: "object", value: values };
  }
  return undefined;
}

function evaluateVueExpression(
  expression: string | undefined,
  context: VueLoweringContext,
): string | number | boolean {
  if (!expression) {
    throw new Error("vue.expression.unsupported: empty expressions cannot be lowered into compiler artifacts.");
  }
  const literal = literalExpressionValue(expression);
  if (literal !== undefined) return literal;
  const scalar = context.scalars.get(expression);
  if (scalar !== undefined) return scalar;
  const [object, property] = expression.split(".");
  const record = object ? context.locals.get(object) : undefined;
  const value = property ? record?.[property] : undefined;
  if (value !== undefined) return value;
  throw new Error("vue.expression.unsupported: compiler artifact expressions must resolve to literal values.");
}

function literalExpressionValue(expression: string): string | number | boolean | undefined {
  if ((expression.startsWith('"') && expression.endsWith('"')) || (expression.startsWith("'") && expression.endsWith("'"))) {
    return expression.slice(1, -1);
  }
  if (expression === "true") return true;
  if (expression === "false") return false;
  const number = Number(expression);
  return Number.isFinite(number) && expression.trim() !== "" ? number : undefined;
}

function expressionSource(expression: string | undefined, framework: string): string {
  const source = expression?.trim() ?? "";
  if (!source) {
    throw new Error(`${framework}.expression.unsupported: dynamic text bindings require a printable expression.`);
  }
  return source;
}

function expressionDependencies(expression: string | undefined): readonly string[] {
  const source = expressionSource(expression, "vue");
  const parsed = parseExpression(source, { plugins: ["typescript"] }) as unknown as AstNode;
  const dependencies = new Set<string>();
  collectExpressionDependencies(parsed, dependencies);
  return [...dependencies];
}

function collectExpressionDependencies(node: AstNode | undefined, dependencies: Set<string>): void {
  if (!node) return;
  switch (node.type) {
    case "Identifier": {
      const name = identifierName(node);
      if (name) dependencies.add(name);
      return;
    }
    case "MemberExpression":
    case "OptionalMemberExpression":
      collectExpressionDependencies(node.object as AstNode | undefined, dependencies);
      if (node.computed) collectExpressionDependencies(node.property as AstNode | undefined, dependencies);
      return;
    case "CallExpression":
    case "OptionalCallExpression":
      collectExpressionDependencies(node.callee as AstNode | undefined, dependencies);
      for (const argument of arrayField(node, "arguments")) collectExpressionDependencies(argument, dependencies);
      return;
    case "BinaryExpression":
    case "LogicalExpression":
      collectExpressionDependencies(node.left as AstNode | undefined, dependencies);
      collectExpressionDependencies(node.right as AstNode | undefined, dependencies);
      return;
    case "ConditionalExpression":
      collectExpressionDependencies(node.test as AstNode | undefined, dependencies);
      collectExpressionDependencies(node.consequent as AstNode | undefined, dependencies);
      collectExpressionDependencies(node.alternate as AstNode | undefined, dependencies);
      return;
    case "UnaryExpression":
    case "UpdateExpression":
    case "AwaitExpression":
      collectExpressionDependencies(node.argument as AstNode | undefined, dependencies);
      return;
    case "TemplateLiteral":
      for (const expression of arrayField(node, "expressions")) collectExpressionDependencies(expression, dependencies);
      return;
    case "ArrayExpression":
      for (const element of arrayField(node, "elements")) collectExpressionDependencies(element, dependencies);
      return;
    case "ObjectExpression":
      for (const property of arrayField(node, "properties")) {
        if (property.computed) collectExpressionDependencies(property.key as AstNode | undefined, dependencies);
        collectExpressionDependencies(property.value as AstNode | undefined, dependencies);
      }
      return;
    default:
      return;
  }
}

function literalValue(node: AstNode | undefined): string | number | boolean | undefined {
  if (!node) return undefined;
  if (node.type === "StringLiteral" || node.type === "NumericLiteral" || node.type === "BooleanLiteral") {
    const value = node.value;
    if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") return value;
  }
  return undefined;
}

function literalString(node: AstNode | undefined): string | undefined {
  const value = literalValue(node);
  return typeof value === "string" ? value : undefined;
}

function firstVueHawkElement(nodes: readonly AstNode[]): AstNode | undefined {
  return nodes.find(isVueHawkElement);
}

function isVueElement(node: AstNode): boolean {
  return node.type === 1;
}

function isVueHawkElement(node: AstNode): boolean {
  return node.type === 1 && isHawkTag(stringField(node, "tag"));
}

function isVueSlotElement(node: AstNode): boolean {
  return node.type === 1 && stringField(node, "tag") === "slot";
}

function identifierName(node: AstNode | undefined): string | undefined {
  return typeof node?.name === "string" ? node.name : undefined;
}

function optionalString(value: string | number | boolean | undefined): string | undefined {
  return typeof value === "string" && value.trim() ? value : undefined;
}

function requiredString(value: string | number | boolean | undefined, tag: string, attribute: string): string {
  if (typeof value === "string" && value.trim()) return value;
  throw new Error(`vue.attribute.required: ${tag} requires a stable ${attribute} attribute.`);
}

function stringField(node: AstNode | undefined, field: string): string {
  const value = node?.[field];
  return typeof value === "string" ? value : "";
}

function arrayField(node: AstNode | undefined, field: string): AstNode[] {
  const value = node?.[field];
  return Array.isArray(value) ? (value.filter(Boolean) as AstNode[]) : [];
}

function kindForTag(tag: string): HawkElementSpec["kind"] {
  if (tag === "hawk-view") return "view";
  if (tag === "hawk-text") return "text";
  if (tag === "hawk-button") return "button";
  if (tag === "hawk-surface" || tag === "hawk-custom-surface") return "custom-surface";
  if (VIEW_ELEMENT_TAGS.has(tag)) return "view";
  if (TEXT_ELEMENT_TAGS.has(tag)) return "text";
  if (tag === "button") return "button";
  throw new Error(`vue.element.unsupported: unsupported Hawk element \`${tag}\`.`);
}

function isHawkTag(tag: string): boolean {
  return tag.startsWith("hawk-") || VIEW_ELEMENT_TAGS.has(tag) || TEXT_ELEMENT_TAGS.has(tag) || tag === "button";
}

function isComponentTag(tag: string): boolean {
  return /^[A-Z]/.test(tag);
}

function isUnsafeAssetPath(path: string): boolean {
  return path.includes("://") || path.startsWith("/") || path.includes("..");
}

function validateUniqueChildKeys(element: HawkElementSpec): void {
  const keys = new Set<string>();
  for (const child of element.children ?? []) {
    if (child.key) {
      if (keys.has(child.key)) {
        throw new Error(`vue.child-key.duplicate: duplicate Vue child key \`${child.key}\``);
      }
      keys.add(child.key);
    }
    validateUniqueChildKeys(child);
  }
}
