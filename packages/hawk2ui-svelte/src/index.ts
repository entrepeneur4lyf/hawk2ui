import { compile as compileSvelte, parse as parseSvelte } from "svelte/compiler";
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
      type HawkCompilerTemplateScalarWire,
      type HawkElementSpec,
      type HawkEventSpec,
  type HawkLifecycleSpec,
} from "../../hawk2ui-native/src/index.ts";

export interface HawkSvelteCompileInput {
  readonly filename: string;
  readonly source: string;
}

export interface HawkSvelteCompileOutput {
  readonly framework: "svelte";
  readonly filename: string;
  readonly records: readonly string[];
  readonly compilerArtifact: HawkCompilerArtifact;
}

type AstNode = Record<string, unknown>;
type LiteralRecord = Readonly<Record<string, string | number | boolean>>;

interface SvelteLoweringContext {
  readonly source: string;
  readonly arrays: ReadonlyMap<string, readonly LiteralRecord[]>;
  readonly snippets: ReadonlyMap<string, SvelteSnippetDefinition>;
  readonly initialDynamicValues: ReadonlyMap<string, HawkCompilerInitialDynamicValueWire>;
  readonly locals: ReadonlyMap<string, LiteralRecord>;
  readonly globals: ReadonlyMap<string, string | number | boolean>;
  readonly scalars: ReadonlyMap<string, string | number | boolean>;
  readonly snippetSlots: ReadonlyMap<string, readonly AstNode[]>;
  readonly snippetStack: readonly string[];
  readonly dynamicBindings: HawkCompilerDynamicBindingWire[];
  readonly listTemplates: HawkCompilerListTemplateWire[];
  readonly pendingListTemplateAnchors: Map<string, string[]>;
}

interface SvelteSnippetDefinition {
  readonly parameters: readonly AstNode[];
  readonly children: readonly AstNode[];
}

const VISUAL_PROP_NAMES = ["font_size", "color", "background"] as const;
const VIEW_ELEMENT_TAGS = new Set(["div", "section", "main", "article", "header", "footer", "nav", "aside", "form", "label", "ul", "ol", "li"]);
const TEXT_ELEMENT_TAGS = new Set(["span", "p", "strong", "em", "small", "code", "h1", "h2", "h3", "h4", "h5", "h6"]);
const RESERVED_RUNTIME_ATTRIBUTE_NAMES = new Set<string>([
  "id",
  "class",
  "data-asset",
  "key",
  "width",
  "height",
  ...VISUAL_PROP_NAMES,
]);
const SVELTE_EVENT_DIRECTIVES = new Map<string, HawkEventSpec["kind"]>([
  ["click", "pointer.press"],
  ["press", "pointer.press"],
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
const SVELTE_LIFECYCLE_DIRECTIVES = new Map<string, HawkLifecycleSpec["phase"]>([
  ["mount", "mounted"],
  ["suspend", "suspended"],
  ["resume", "resumed"],
  ["hot-reloaded", "hot-reloaded"],
  ["error-boundary", "error-boundary"],
  ["shutdown", "shutdown"],
  ["destroy", "unmounted"],
]);

export function compileHawkSvelte(input: HawkSvelteCompileInput): HawkSvelteCompileOutput {
  if (!input.filename.endsWith(".svelte")) {
    throw new Error("Hawk2UI Svelte inputs must be .svelte files.");
  }

  const ast = parseSvelte(input.source, { filename: input.filename, modern: true }) as unknown as AstNode;
  compileSvelte(input.source, { filename: input.filename, generate: false });

  const rootChildren = childrenOf(ast);
  const rootNode = firstHawkElement(rootChildren);
  if (!rootNode) {
    throw new Error("svelte.root.missing: Svelte compiler output must contain one hawk root element.");
  }
  const instanceProgram = (ast.instance as AstNode | undefined)?.content as AstNode | undefined;

  const context: SvelteLoweringContext = {
    source: input.source,
    arrays: literalArraysFromProgram(instanceProgram),
    snippets: snippetDefinitionsFromNodes(rootChildren),
    initialDynamicValues: initialDynamicValuesFromProgram(instanceProgram),
    locals: new Map(),
    globals: scalarValuesFromProgram(instanceProgram),
    scalars: new Map(),
    snippetSlots: new Map(),
    snippetStack: [],
    dynamicBindings: [],
    listTemplates: [],
    pendingListTemplateAnchors: new Map(),
  };
  const root = withRootLifecycle(
    svelteElementToSpec(rootNode, context),
    svelteLifecycleApiCalls(instanceProgram),
  );
  validateUniqueChildIds(root.children ?? []);

  const app = { name: input.filename, root };
  const handlerArtifacts = eventHandlerArtifactsForSpec(root, instanceProgram, context);
  return {
    framework: "svelte",
    filename: input.filename,
    records: recordsForApp(app),
      compilerArtifact: compilerArtifactForApp(
        app,
        [],
        context.dynamicBindings,
        [...context.initialDynamicValues.values()],
          {
            compiler: {
              framework: "svelte",
              compiler: "@hawk2ui/svelte",
              source_path: input.filename,
              entrypoint: "default",
            },
              eventHandlers: handlerArtifacts,
              listTemplates: context.listTemplates,
            },
          ),
      };
  }

function svelteElementToSpec(node: AstNode, context: SvelteLoweringContext): HawkElementSpec {
  const tag = stringField(node, "name");
  const id = requiredString(attributeValue(node, "id", context), tag, "id");
  const style = optionalString(attributeValue(node, "class", context));
  const assetPath = optionalString(attributeValue(node, "data-asset", context));
  if (assetPath && isUnsafeAssetPath(assetPath)) {
    throw new Error("svelte.asset.path-invalid: Svelte asset references must use workspace-relative paths.");
  }

  const handlers = eventHandlers(node);
  const spec: HawkElementSpec = {
    id,
    kind: kindForTag(tag),
    key: id,
    refs: actionRefs(node),
    styleRefs: style ? [style] : [],
    assetRefs: assetPath ? [{ name: "svelte.asset", path: assetPath }] : [],
    events: handlers.events,
    lifecycle: handlers.lifecycle,
    children: childSpecs(node, context, id),
  };

  const props = runtimeProps(node, context, id, "svelte");
  const text = textContent(node, context, id);
  if (text) props.text = text;
  return Object.keys(props).length > 0 ? { ...spec, props } : spec;
}

function childSpecs(node: AstNode, context: SvelteLoweringContext, parentId: string): readonly HawkElementSpec[] {
  const children: HawkElementSpec[] = [];
  for (const child of childrenOf(node)) {
    let specs: readonly HawkElementSpec[] = [];
    if (isSvelteElement(child) && isHawkTag(stringField(child, "name"))) {
      specs = [svelteElementToSpec(child, context)];
    } else if (child.type === "EachBlock") {
      specs = expandEachBlock(child, context, parentId);
    } else if (child.type === "IfBlock") {
      specs = expandIfBlock(child, context, parentId);
    } else if (child.type === "RenderTag") {
      specs = expandRenderTag(child, context, parentId);
    }
    for (const spec of specs) {
      anchorPendingListTemplates(context, parentId, spec.id);
      children.push(spec);
    }
  }
  return children;
}

function expandIfBlock(block: AstNode, context: SvelteLoweringContext, parentId: string): readonly HawkElementSpec[] {
  const expression = (block.expression ?? block.test) as AstNode | undefined;
  const consequent = (block.consequent as AstNode | undefined) ?? block;
  const visibleChildren = withSvelteVisibilityBinding(
    childSpecs({ children: childrenOf(consequent) }, context, parentId),
    expression,
    context,
    false,
  );
  const elseBlock = (block.else ?? block.alternate) as AstNode | undefined;
  if (!elseBlock) return visibleChildren;
  return [
    ...visibleChildren,
      ...withSvelteVisibilityBinding(
        childSpecs({ children: childrenOf(elseBlock) }, context, parentId),
      expression,
      context,
      true,
    ),
  ];
}

function expandRenderTag(tag: AstNode, context: SvelteLoweringContext, parentId: string): readonly HawkElementSpec[] {
  const call = tag.expression as AstNode | undefined;
  if (call?.type !== "CallExpression") {
    throw new Error("svelte.render.unsupported: render tags must call a Svelte 5 snippet.");
  }
  const name = identifierName(call.callee as AstNode | undefined);
  if (!name) {
    throw new Error("svelte.render.unsupported: render tags must call a stable snippet identifier.");
  }
  const slot = context.snippetSlots.get(name);
  if (slot) return slot.flatMap((child) => childSpecs({ children: [child] }, context, parentId));
  const definition = context.snippets.get(name);
  if (!definition) {
    throw new Error(`svelte.render.unresolved: snippet \`${name}\` must be declared in this component.`);
  }
  if (context.snippetStack.includes(name)) {
    throw new Error(`svelte.render.cycle: snippet \`${name}\` recursively renders itself.`);
  }
  const scoped = scopedSnippetContext(name, definition, arrayField(call, "arguments"), context);
  return childSpecs({ children: definition.children }, scoped, parentId);
}

function scopedSnippetContext(
  name: string,
  definition: SvelteSnippetDefinition,
  args: readonly AstNode[],
  context: SvelteLoweringContext,
): SvelteLoweringContext {
  const scalars = new Map(context.scalars);
  const snippetSlots = new Map(context.snippetSlots);
  definition.parameters.forEach((parameter, index) => {
    const parameterName = identifierName(parameter);
    if (!parameterName) {
      throw new Error("svelte.snippet.parameter-unsupported: snippet parameters must be stable identifiers.");
    }
    const argument = args[index];
    const slotName = identifierName(argument);
    const slotDefinition = slotName ? context.snippets.get(slotName) : undefined;
    if (slotDefinition) {
      snippetSlots.set(parameterName, slotDefinition.children);
      return;
    }
    scalars.set(parameterName, evaluateSnippetArgument(argument, context));
  });
  return {
    ...context,
    scalars,
    snippetSlots,
    snippetStack: [...context.snippetStack, name],
  };
}

function evaluateSnippetArgument(
  expression: AstNode | undefined,
  context: SvelteLoweringContext,
): string | number | boolean {
  const literal = literalValue(expression);
  if (literal !== undefined) return literal;
  if (expression?.type === "Identifier") {
    const name = identifierName(expression);
    const local = name ? context.scalars.get(name) : undefined;
    if (local !== undefined) return local;
    const global = name ? context.globals.get(name) : undefined;
    if (global !== undefined) return global;
  }
  return evaluateExpression(expression, context);
}

function expandEachBlock(block: AstNode, context: SvelteLoweringContext, parentId: string): readonly HawkElementSpec[] {
  const source = identifierName(block.expression as AstNode | undefined);
  const itemName = identifierName(block.context as AstNode | undefined);
  if (!source || !itemName) {
    throw new Error("svelte.each.unsupported: keyed each blocks must use `items as item` style bindings.");
  }
  const items = context.arrays.get(source);
  if (!items) {
    const initialValue = context.initialDynamicValues.get(source)?.value;
    if (initialValue?.type !== "array") {
      throw new Error(`svelte.each.source-unresolved: Svelte each source \`${source}\` must be a literal array or initial dynamic array.`);
    }
    const template = firstHawkElement(childrenOf(block));
    if (!template) {
      throw new Error("svelte.each.template-missing: Svelte each blocks must render one hawk child element.");
    }
    const keyExpression = expressionSource(block.key as AstNode | undefined, context);
    context.listTemplates.push({
      id: `${parentId}:${source}`,
      parent_id: parentId,
      source,
      item: itemName,
      key: keyExpression,
      node: svelteElementToListTemplateNode(template, context, itemName),
    });
    queuePendingListTemplateAnchor(context, parentId, `${parentId}:${source}`);
    return [];
  }
  const template = firstHawkElement(childrenOf(block));
  if (!template) {
    throw new Error("svelte.each.template-missing: Svelte each blocks must render one hawk child element.");
  }
  return items.map((item) =>
    svelteElementToSpec(template, {
      ...context,
      locals: new Map([...context.locals, [itemName, item]]),
    }),
  );
}

function queuePendingListTemplateAnchor(
  context: SvelteLoweringContext,
  parentId: string,
  templateId: string,
): void {
  const pending = context.pendingListTemplateAnchors.get(parentId) ?? [];
  pending.push(templateId);
  context.pendingListTemplateAnchors.set(parentId, pending);
}

function anchorPendingListTemplates(
  context: SvelteLoweringContext,
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

function svelteElementToListTemplateNode(
  node: AstNode,
  context: SvelteLoweringContext,
  itemName: string,
): HawkCompilerListTemplateNodeWire {
  const tag = stringField(node, "name");
  const id = templateScalarFromAttribute(node, "id", context, itemName);
  const props = templateProps(node, context, itemName);
  const text = templateTextContent(node, context, itemName);
  if (text) props.push({ name: "text", value: text });
  const handlers = eventHandlers(node);
  return {
    id,
    kind: kindForTag(tag),
    key: id,
    props,
    refs: actionRefs(node),
    style_refs: optionalString(attributeValue(node, "class", context)) ? [optionalString(attributeValue(node, "class", context)) as string] : [],
    asset_refs: optionalString(attributeValue(node, "data-asset", context)) ? [{ name: "svelte.asset", path: optionalString(attributeValue(node, "data-asset", context)) as string }] : [],
    events: handlers.events.map((event) => ({ kind: event.kind, handler: event.handler, payload_fields: payloadFieldsForEvent(event.kind) })),
    lifecycle: handlers.lifecycle.map((lifecycle) => ({ event: lifecycle.phase, handler: lifecycle.handler })),
    children: childrenOf(node)
      .filter((child) => isSvelteElement(child) && isHawkTag(stringField(child, "name")))
      .map((child) => svelteElementToListTemplateNode(child, context, itemName)),
  };
}

function templateProps(
  node: AstNode,
  context: SvelteLoweringContext,
  itemName: string,
): { name: string; value: HawkCompilerTemplateScalarWire }[] {
  const props: { name: string; value: HawkCompilerTemplateScalarWire }[] = [];
  for (const name of ["width", "height", ...VISUAL_PROP_NAMES]) {
    const value = optionalTemplateScalarFromAttribute(node, name, context, itemName);
    if (value) props.push({ name, value });
  }
  for (const attribute of attributesOf(node)) {
    if (attribute.type !== "Attribute") continue;
    const name = stringField(attribute, "name");
    if (!name || RESERVED_RUNTIME_ATTRIBUTE_NAMES.has(name)) continue;
    props.push({ name, value: templateScalarFromAttribute(node, name, context, itemName) });
  }
  return props;
}

function templateTextContent(
  node: AstNode,
  context: SvelteLoweringContext,
  itemName: string,
): HawkCompilerTemplateScalarWire | undefined {
  const children = childrenOf(node).filter((child) => child.type === "Text" || isExpressionTag(child));
  if (children.length === 0) return undefined;
  if (children.length === 1) {
    const child = children[0];
    if (!child) return undefined;
    if (child.type === "Text") {
      const text = stringField(child, "data").trim();
      return text ? literalTemplateScalar(text) : undefined;
    }
    return templateScalarFromExpression(child.expression as AstNode | undefined, context, itemName);
  }
  return {
    type: "expression",
    expression: children.map((child) => child.type === "Text" ? JSON.stringify(stringField(child, "data")) : expressionSource(child.expression as AstNode | undefined, context)).join(" + "),
  };
}

function templateScalarFromAttribute(
  node: AstNode,
  name: string,
  context: SvelteLoweringContext,
  itemName: string,
): HawkCompilerTemplateScalarWire {
  const value = optionalTemplateScalarFromAttribute(node, name, context, itemName);
  if (!value) throw new Error(`svelte.list-template.attribute-required: list template nodes require \`${name}\`.`);
  return value;
}

function optionalTemplateScalarFromAttribute(
  node: AstNode,
  name: string,
  context: SvelteLoweringContext,
  itemName: string,
): HawkCompilerTemplateScalarWire | undefined {
  const attribute = attributesOf(node).find((candidate) => candidate.type === "Attribute" && stringField(candidate, "name") === name);
  if (!attribute) return undefined;
  const value = firstAttributeValueNode(attribute);
  if (value === true) return literalTemplateScalar(true);
  if (!value) return literalTemplateScalar(true);
  if (value.type === "Text") return literalTemplateScalar(stringField(value, "data"));
  if (isExpressionTag(value)) return templateScalarFromExpression(value.expression as AstNode | undefined, context, itemName);
  return undefined;
}

function templateScalarFromExpression(
  expression: AstNode | undefined,
  context: SvelteLoweringContext,
  itemName: string,
): HawkCompilerTemplateScalarWire {
  const staticValue = staticTextExpressionValue(expression, context);
  if (staticValue !== undefined) return literalTemplateScalar(staticValue);
  const source = expressionSource(expression, context);
  if (!expressionDependencies(expression).includes(itemName)) {
    throw new Error(`svelte.list-template.expression-unsupported: list template expressions must depend on \`${itemName}\`.`);
  }
  return { type: "expression", expression: source };
}

function literalTemplateScalar(value: string | number | boolean): HawkCompilerTemplateScalarWire {
  if (typeof value === "string") return { type: "literal", value: { type: "string", value } };
  if (typeof value === "boolean") return { type: "literal", value: { type: "bool", value } };
  return { type: "literal", value: { type: "number", value } };
}

function eventHandlers(node: AstNode): {
  readonly events: readonly HawkEventSpec[];
  readonly lifecycle: readonly HawkLifecycleSpec[];
} {
  const events: HawkEventSpec[] = [];
  const lifecycle: HawkLifecycleSpec[] = [];
  for (const attribute of attributesOf(node)) {
      if (attribute.type !== "EventHandler" && attribute.type !== "OnDirective") continue;
      const name = stringField(attribute, "name");
      const handler = handlerName(attribute.expression as AstNode | undefined);
      const eventKind = SVELTE_EVENT_DIRECTIVES.get(name);
      const lifecyclePhase = SVELTE_LIFECYCLE_DIRECTIVES.get(name);
      if (eventKind) {
        events.push({ kind: eventKind, handler });
      } else if (lifecyclePhase) {
        lifecycle.push({ phase: lifecyclePhase, handler });
      } else {
        throw new Error(`svelte.event.unsupported: Svelte event \`${name}\` is not part of the native event contract.`);
      }
  }
  return { events, lifecycle };
}

function svelteLifecycleApiCalls(program: AstNode | undefined): readonly HawkLifecycleSpec[] {
  const lifecycle: HawkLifecycleSpec[] = [];
  for (const statement of arrayField(program, "body")) {
    if (statement.type !== "ExpressionStatement") continue;
    const call = statement.expression as AstNode | undefined;
    if (call?.type !== "CallExpression") continue;
    const name = callName(call.callee as AstNode | undefined);
    const argument = arrayField(call, "arguments")[0];
    if (name === "onMount") {
      for (const handler of lifecycleHandlerNamesFromArgument(argument, "svelte", "onMount")) {
        pushLifecycle(lifecycle, "mounted", handler);
      }
      const cleanup = lifecycleCleanupHandlerNameFromArgument(argument, "svelte", "onMount");
      if (cleanup) pushLifecycle(lifecycle, "unmounted", cleanup);
      continue;
    }
    if (name === "onDestroy") {
      for (const handler of lifecycleHandlerNamesFromArgument(argument, "svelte", "onDestroy")) {
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

function lifecycleCleanupHandlerNameFromArgument(
  argument: AstNode | undefined,
  framework: string,
  api: string,
): string | undefined {
  const body = functionLikeBody(argument);
  if (body?.type !== "BlockStatement") return undefined;
  for (const statement of arrayField(body, "body")) {
    if (statement.type !== "ReturnStatement") continue;
    return lifecycleCleanupHandlerName(statement.argument as AstNode | undefined, framework, api);
  }
  return undefined;
}

function lifecycleCleanupHandlerName(
  expression: AstNode | undefined,
  framework: string,
  api: string,
): string {
  const direct = identifierName(expression);
  if (direct) return direct;
  const body = functionLikeBody(expression);
  if (body?.type === "CallExpression") {
    const name = callName(body.callee as AstNode | undefined);
    if (name) return name;
  }
  if (body?.type === "BlockStatement") {
    const handlers = lifecycleHandlerNamesFromArgument(expression, framework, api);
    const handler = handlers[0];
    if (handlers.length === 1 && handler) return handler;
  }
  throw new Error(`${framework}.lifecycle.unsupported: ${api} cleanup must reference or call one stable handler.`);
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

function callName(callee: AstNode | undefined): string | undefined {
  if (callee?.type === "MemberExpression") return identifierName(callee.property as AstNode | undefined);
  return identifierName(callee);
}

function eventHandlerArtifactsForSpec(
  root: HawkElementSpec,
  program: AstNode | undefined,
  context: SvelteLoweringContext,
): readonly HawkCompilerEventHandlerWire[] {
  const declarations = handlerDeclarationsFromProgram(program);
  const lifecycleOnlyHandlers = lifecycleOnlyHandlerNames(root, context.listTemplates);
  return referencedHandlerNames(root, context.listTemplates).flatMap((name) => {
    const declaration = declarations.get(name);
    if (!declaration) {
      throw new Error(`svelte.handler.missing: event handler \`${name}\` must be declared in the instance script.`);
    }
    const actions = handlerActions(name, declaration, context, lifecycleOnlyHandlers.has(name));
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

function handlerDeclarationsFromProgram(program: AstNode | undefined): ReadonlyMap<string, AstNode> {
  const declarations = new Map<string, AstNode>();
  for (const statement of Array.isArray(program?.body) ? (program.body as AstNode[]) : []) {
    if (statement.type === "FunctionDeclaration") {
      const name = identifierName(statement.id as AstNode | undefined);
      if (name) declarations.set(name, statement);
      continue;
    }
    if (statement.type !== "VariableDeclaration") continue;
    for (const declaration of Array.isArray(statement.declarations) ? (statement.declarations as AstNode[]) : []) {
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
  context: SvelteLoweringContext,
  allowEmpty: boolean,
): readonly HawkCompilerEventHandlerActionWire[] {
  const body = declaration.body as AstNode | undefined;
  if (!body) {
    throw new Error(`svelte.handler.unsupported: event handler \`${handler}\` must have an executable body.`);
  }
  if (body.type !== "BlockStatement") {
    return [handlerActionFromExpression(handler, body, context)];
  }
  const actions = (Array.isArray(body.body) ? (body.body as AstNode[]) : []).map((statement) => {
    if (statement.type !== "ExpressionStatement") {
      throw new Error(`svelte.handler.unsupported: event handler \`${handler}\` contains unsupported statements.`);
    }
    return handlerActionFromExpression(handler, statement.expression as AstNode | undefined, context);
    });
    if (actions.length === 0 && !allowEmpty) {
      throw new Error(`svelte.handler.unsupported: event handler \`${handler}\` must contain at least one action.`);
    }
    return actions;
  }

function handlerActionFromExpression(
  handler: string,
  expression: AstNode | undefined,
  context: SvelteLoweringContext,
): HawkCompilerEventHandlerActionWire {
  if (expression?.type === "AssignmentExpression" && expression.operator === "=") {
    const name = identifierName(expression.left as AstNode | undefined);
    if (!name) {
      throw new Error(`svelte.handler.unsupported: event handler \`${handler}\` assignment target must be a dynamic value name.`);
    }
    return dynamicUpdateAction(name, expression.right as AstNode | undefined, context);
  }
  throw new Error(`svelte.handler.unsupported: event handler \`${handler}\` must assign a dynamic value.`);
}

function dynamicUpdateAction(
  name: string,
  expression: AstNode | undefined,
  context: SvelteLoweringContext,
): HawkCompilerEventHandlerActionWire {
  const value = literalDynamicValue(expression);
  if (value) {
    return { type: "set_dynamic_value", name, value };
  }
  return {
    type: "set_dynamic_expression",
    name,
    expression: expressionSource(expression, context),
    dependencies: expressionDependencies(expression),
  };
}

function actionRefs(node: AstNode): readonly string[] {
  return attributesOf(node)
    .filter((attribute) => attribute.type === "Action" || attribute.type === "UseDirective")
    .map((attribute) => stringField(attribute, "name"))
    .filter((name) => name.trim().length > 0);
}

function attributeValue(
  node: AstNode,
  name: string,
  context: SvelteLoweringContext,
): string | number | boolean | undefined {
  const attribute = attributesOf(node).find((item) => item.type === "Attribute" && item.name === name);
  if (!attribute) return undefined;
  const first = firstAttributeValueNode(attribute);
  if (first === true) return true;
  if (!first) return "";
  if (first.type === "Text") return stringField(first, "data");
  if (isExpressionTag(first)) return evaluateExpression(first.expression as AstNode | undefined, context);
  throw new Error(`svelte.attribute.unsupported: attribute \`${name}\` must be static or a literal member expression.`);
}

function textContent(
  node: AstNode,
  context: SvelteLoweringContext,
  nodeId: string,
): string | undefined {
  const staticValues: string[] = [];
  const dynamicExpressionParts: string[] = [];
  const dependencies = new Set<string>();
  let hasDynamicExpression = false;

  for (const child of childrenOf(node)) {
    if (child.type === "Text") {
      const value = stringField(child, "data").trim();
      if (value.length > 0) {
        staticValues.push(value);
        dynamicExpressionParts.push(JSON.stringify(value));
      }
      continue;
    }
      if (!isExpressionTag(child)) continue;
    const expression = child.expression as AstNode | undefined;
    const staticValue = staticTextExpressionValue(expression, context);
    if (staticValue !== undefined) {
      const value = String(staticValue);
      if (value.length > 0) {
        staticValues.push(value);
        dynamicExpressionParts.push(JSON.stringify(value));
      }
      continue;
    }
    hasDynamicExpression = true;
    dynamicExpressionParts.push(expressionSource(expression, context));
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

function staticTextExpressionValue(
  expression: AstNode | undefined,
  context: SvelteLoweringContext,
): string | number | boolean | undefined {
  try {
    return evaluateExpression(expression, context);
  } catch (error) {
    if (error instanceof Error && error.message.startsWith("svelte.expression.unsupported")) {
      return undefined;
    }
    throw error;
  }
}

function layoutProps(
  node: AstNode,
  context: SvelteLoweringContext,
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
  context: SvelteLoweringContext,
  nodeId: string,
  framework: string,
): Record<string, string | number | boolean> {
  const props = layoutProps(node, context, nodeId, framework);
  for (const name of VISUAL_PROP_NAMES) {
    const value = dynamicVisualAttributeValue(node, name, context, nodeId, framework);
    if (value !== undefined) props[name] = value;
  }
  for (const attribute of attributesOf(node).filter((item) => item.type === "Attribute")) {
    const name = stringField(attribute, "name");
    if (!name || RESERVED_RUNTIME_ATTRIBUTE_NAMES.has(name)) continue;
    const value = dynamicRuntimeAttributeValue(attribute, name, context, nodeId, framework);
    if (value !== undefined) props[name] = value;
  }
  return props;
}

function withSvelteVisibilityBinding(
  specs: readonly HawkElementSpec[],
  expression: AstNode | undefined,
  context: SvelteLoweringContext,
  negate: boolean,
): readonly HawkElementSpec[] {
  const dependencies = expressionDependencies(expression);
  if (dependencies.length > 0) {
    const source = expressionSource(expression, context);
    const visibleExpression = negate ? `!(${source})` : source;
    for (const spec of specs) {
      mergeDynamicVisibilityBinding(context, spec.id, visibleExpression, dependencies);
    }
    return specs;
  }

  const staticValue = staticTextExpressionValue(expression, context);
  if (typeof staticValue !== "boolean") {
    throw new Error("svelte.if.unsupported: Svelte if blocks must use boolean expressions.");
  }
  return specs.map((spec) => withStaticVisibility(spec, negate ? !staticValue : staticValue));
}

function mergeDynamicVisibilityBinding(
  context: SvelteLoweringContext,
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
    throw new Error("svelte.visibility.internal: visible binding index disappeared during merge.");
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

function dynamicRuntimeAttributeValue(
  attribute: AstNode,
  name: string,
  context: SvelteLoweringContext,
  nodeId: string,
  framework: string,
): string | number | boolean | undefined {
  const value = attribute.value;
  const first = firstAttributeValueNode(attribute);
  if (first === true) return true;
  if (!first) return "";
  if (first.type === "Text") return stringField(first, "data");
  if (!isExpressionTag(first)) {
    throw new Error(`${framework}.attribute.unsupported: prop \`${name}\` on \`${nodeId}\` must be a static scalar or expression.`);
  }
  const expression = first.expression as AstNode | undefined;
  const staticValue = staticTextExpressionValue(expression, context);
  if (staticValue !== undefined) return staticValue;
  context.dynamicBindings.push({
    node_id: nodeId,
    target: { type: "prop", name },
    expression: expressionSource(expression, context),
    dependencies: expressionDependencies(expression),
  });
  return undefined;
}

function dynamicVisualAttributeValue(
  node: AstNode,
  name: string,
  context: SvelteLoweringContext,
  nodeId: string,
  framework: string,
): string | number | boolean | undefined {
  const attribute = attributesOf(node).find((item) => item.type === "Attribute" && item.name === name);
  if (!attribute) return undefined;
  const first = firstAttributeValueNode(attribute);
  if (first === true) return "";
  if (!first) return "";
  if (first.type === "Text") return stringField(first, "data");
  if (!isExpressionTag(first)) {
    throw new Error(`${framework}.attribute.unsupported: visual prop \`${name}\` on \`${nodeId}\` must be a static scalar or expression.`);
  }
  const expression = first.expression as AstNode | undefined;
  const staticValue = staticTextExpressionValue(expression, context);
  if (staticValue !== undefined) return staticValue;
  context.dynamicBindings.push({
    node_id: nodeId,
    target: { type: "prop", name },
    expression: expressionSource(expression, context),
    dependencies: expressionDependencies(expression),
  });
  return undefined;
}

function dynamicLayoutAttributeValue(
  node: AstNode,
  name: string,
  context: SvelteLoweringContext,
  nodeId: string,
  framework: string,
): number | undefined {
  const attribute = attributesOf(node).find((item) => item.type === "Attribute" && item.name === name);
  if (!attribute) return undefined;
  const first = firstAttributeValueNode(attribute);
  if (first === true || !first) {
    throw new Error(`${framework}.attribute.unsupported: layout prop \`${name}\` must be numeric.`);
  }
  if (first.type === "Text") return layoutNumber(stringField(first, "data"), nodeId, name, framework);
  if (!isExpressionTag(first)) {
    throw new Error(`${framework}.attribute.unsupported: layout prop \`${name}\` must be numeric.`);
  }
  const expression = first.expression as AstNode | undefined;
  const staticValue = staticTextExpressionValue(expression, context);
  if (staticValue !== undefined) return layoutNumber(staticValue, nodeId, name, framework);
  context.dynamicBindings.push({
    node_id: nodeId,
    target: { type: "prop", name },
    expression: expressionSource(expression, context),
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

function literalArraysFromProgram(program: AstNode | undefined): ReadonlyMap<string, readonly LiteralRecord[]> {
  const arrays = new Map<string, readonly LiteralRecord[]>();
  for (const statement of Array.isArray(program?.body) ? (program.body as AstNode[]) : []) {
    if (statement.type !== "VariableDeclaration") continue;
    if (statement.kind !== "const") continue;
    for (const declaration of Array.isArray(statement.declarations) ? (statement.declarations as AstNode[]) : []) {
      const name = identifierName(declaration.id as AstNode | undefined);
      const values = literalObjectArray(declaration.init as AstNode | undefined);
      if (name && values) arrays.set(name, values);
    }
  }
  return arrays;
}

function initialDynamicValuesFromProgram(
  program: AstNode | undefined,
): ReadonlyMap<string, HawkCompilerInitialDynamicValueWire> {
  const values = new Map<string, HawkCompilerInitialDynamicValueWire>();
  for (const statement of Array.isArray(program?.body) ? (program.body as AstNode[]) : []) {
    if (statement.type === "LabeledStatement" && identifierName(statement.label as AstNode | undefined) === "$") {
      const expression = (statement.body as AstNode | undefined)?.expression as AstNode | undefined;
      if (expression?.type === "AssignmentExpression" && expression.operator === "=") {
        const name = identifierName(expression.left as AstNode | undefined);
        const value = literalDynamicValue(expression.right as AstNode | undefined);
        if (name && value) values.set(name, { name, mode: "value", value });
      }
      continue;
    }
    if (statement.type !== "VariableDeclaration") continue;
    for (const declaration of Array.isArray(statement.declarations) ? (statement.declarations as AstNode[]) : []) {
      const name = identifierName(declaration.id as AstNode | undefined);
      const value = literalDynamicValue(declaration.init as AstNode | undefined);
      if (name && value) values.set(name, { name, mode: "value", value });
    }
  }
  return values;
}

function scalarValuesFromProgram(program: AstNode | undefined): ReadonlyMap<string, string | number | boolean> {
  const values = new Map<string, string | number | boolean>();
  for (const statement of Array.isArray(program?.body) ? (program.body as AstNode[]) : []) {
    if (statement.type !== "VariableDeclaration") continue;
    for (const declaration of Array.isArray(statement.declarations) ? (statement.declarations as AstNode[]) : []) {
      const name = identifierName(declaration.id as AstNode | undefined);
      const value = literalValue(declaration.init as AstNode | undefined);
      if (name && value !== undefined) values.set(name, value);
    }
  }
  return values;
}

function snippetDefinitionsFromNodes(nodes: readonly AstNode[]): ReadonlyMap<string, SvelteSnippetDefinition> {
  const snippets = new Map<string, SvelteSnippetDefinition>();
  for (const node of nodes) {
    if (node.type !== "SnippetBlock") continue;
    const name = identifierName(node.expression as AstNode | undefined);
    if (!name) {
      throw new Error("svelte.snippet.unsupported: snippets must use stable identifiers.");
    }
    snippets.set(name, {
      parameters: arrayField(node, "parameters"),
      children: childrenOf(node),
    });
  }
  return snippets;
}

function literalObjectArray(node: AstNode | undefined): readonly LiteralRecord[] | undefined {
  if (!node || node.type !== "ArrayExpression" || !Array.isArray(node.elements)) return undefined;
  return node.elements.map((item) => {
    if (!item || typeof item !== "object" || (item as AstNode).type !== "ObjectExpression") {
      throw new Error("svelte.literal-array.unsupported: each sources must be arrays of literal objects.");
    }
    const record: Record<string, string | number | boolean> = {};
    for (const property of ((item as AstNode).properties as AstNode[]) ?? []) {
      const key = identifierName(property.key as AstNode | undefined) ?? stringField(property.key as AstNode, "value");
      const value = literalValue(property.value as AstNode | undefined);
      if (!key || value === undefined) {
        throw new Error("svelte.literal-array.unsupported: literal object properties must be scalar values.");
      }
      record[key] = value;
    }
    return record;
  });
}

function literalDynamicValue(node: AstNode | undefined): HawkCompilerDynamicValueWire | undefined {
  if (!node) return undefined;
  if (node.type === "Literal") {
    const value = node.value;
    if (value === null) return { type: "null" };
    if (typeof value === "string") return { type: "string", value };
    if (typeof value === "boolean") return { type: "bool", value };
    if (typeof value === "number" && Number.isFinite(value)) return { type: "number", value };
  }
  if (node.type === "ArrayExpression") {
    const values: HawkCompilerDynamicValueWire[] = [];
    for (const element of Array.isArray(node.elements) ? (node.elements as AstNode[]) : []) {
      const value = literalDynamicValue(element);
      if (!value) return undefined;
      values.push(value);
    }
    return { type: "array", value: values };
  }
  if (node.type === "ObjectExpression") {
    const values: Record<string, HawkCompilerDynamicValueWire> = {};
    for (const property of (node.properties as AstNode[] | undefined) ?? []) {
      const key = identifierName(property.key as AstNode | undefined) ?? stringField(property.key as AstNode, "value");
      const value = literalDynamicValue(property.value as AstNode | undefined);
      if (!key || !value) return undefined;
      values[key] = value;
    }
    return { type: "object", value: values };
  }
  return undefined;
}

function evaluateExpression(
  expression: AstNode | undefined,
  context: SvelteLoweringContext,
): string | number | boolean {
  const literal = literalValue(expression);
  if (literal !== undefined) return literal;
  if (expression?.type === "Identifier") {
    const name = identifierName(expression);
    const value = name ? context.scalars.get(name) : undefined;
    if (value !== undefined) return value;
  }
  if (expression?.type === "MemberExpression") {
    const object = identifierName(expression.object as AstNode | undefined);
    const property = identifierName(expression.property as AstNode | undefined);
    const record = object ? context.locals.get(object) : undefined;
    const value = property ? record?.[property] : undefined;
    if (value !== undefined) return value;
  }
  throw new Error("svelte.expression.unsupported: compiler artifact expressions must resolve to literal values.");
}

function literalValue(node: AstNode | undefined): string | number | boolean | undefined {
  if (!node) return undefined;
  if (node.type === "Literal" || node.type === "StringLiteral" || node.type === "NumericLiteral" || node.type === "BooleanLiteral") {
    const value = node.value;
    if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") return value;
  }
  return undefined;
}

function expressionSource(expression: AstNode | undefined, context: SvelteLoweringContext): string {
  const start = numberField(expression, "start");
  const end = numberField(expression, "end");
  const source = start !== undefined && end !== undefined
    ? context.source.slice(start, end).trim()
    : identifierName(expression);
  if (!source) {
    throw new Error("svelte.expression.unsupported: dynamic text bindings require a printable expression.");
  }
  return source;
}

function expressionDependencies(expression: AstNode | undefined): readonly string[] {
  const dependencies = new Set<string>();
  collectExpressionDependencies(expression, dependencies);
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

function firstHawkElement(nodes: readonly AstNode[]): AstNode | undefined {
  return nodes.find((node) => isSvelteElement(node) && isHawkTag(stringField(node, "name")));
}

function childrenOf(node: AstNode | undefined): readonly AstNode[] {
  if (Array.isArray(node?.children)) return node.children as AstNode[];
  const fragment = node?.fragment as AstNode | undefined;
  if (Array.isArray(fragment?.nodes)) return fragment.nodes as AstNode[];
  const body = node?.body as AstNode | undefined;
  if (Array.isArray(body?.nodes)) return body.nodes as AstNode[];
  if (Array.isArray(node?.nodes)) return node.nodes as AstNode[];
  return [];
}

function attributesOf(node: AstNode): readonly AstNode[] {
  return Array.isArray(node.attributes) ? (node.attributes as AstNode[]) : [];
}

function firstAttributeValueNode(attribute: AstNode): AstNode | true | undefined {
  const value = attribute.value;
  if (value === true) return true;
  if (Array.isArray(value)) return value[0] as AstNode | undefined;
  if (value && typeof value === "object") return value as AstNode;
  return undefined;
}

function isSvelteElement(node: AstNode | undefined): boolean {
  return node?.type === "Element" || node?.type === "RegularElement";
}

function isExpressionTag(node: AstNode | undefined): boolean {
  return node?.type === "MustacheTag" || node?.type === "ExpressionTag";
}

function handlerName(expression: AstNode | undefined): string {
  const name = identifierName(expression);
  if (!name) throw new Error("svelte.handler.unsupported: event handlers must be stable identifiers.");
  return name;
}

function identifierName(node: AstNode | undefined): string | undefined {
  return typeof node?.name === "string" ? node.name : undefined;
}

function optionalString(value: string | number | boolean | undefined): string | undefined {
  return typeof value === "string" && value.trim() ? value : undefined;
}

function requiredString(value: string | number | boolean | undefined, tag: string, attribute: string): string {
  if (typeof value === "string" && value.trim()) return value;
  throw new Error(`svelte.attribute.required: ${tag} requires a stable ${attribute} attribute.`);
}

function stringField(node: AstNode | undefined, field: string): string {
  const value = node?.[field];
  return typeof value === "string" ? value : "";
}

function numberField(node: AstNode | undefined, field: string): number | undefined {
  const value = node?.[field];
  return typeof value === "number" ? value : undefined;
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
  throw new Error(`svelte.element.unsupported: unsupported Hawk element \`${tag}\`.`);
}

function isHawkTag(tag: string): boolean {
  return tag.startsWith("hawk-") || VIEW_ELEMENT_TAGS.has(tag) || TEXT_ELEMENT_TAGS.has(tag) || tag === "button";
}

function validateUniqueChildIds(children: readonly HawkElementSpec[]): void {
  const ids = new Set<string>();
  for (const child of children) {
    if (ids.has(child.id)) {
      throw new Error(`svelte.child-id.duplicate: duplicate Svelte child id \`${child.id}\``);
    }
    ids.add(child.id);
  }
}

function isUnsafeAssetPath(path: string): boolean {
  return path.includes("://") || path.startsWith("/") || path.includes("..");
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
