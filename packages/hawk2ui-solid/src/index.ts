import { transformSync } from "@babel/core";
import generate from "@babel/generator";
import { parse } from "@babel/parser";
import {
  compilerArtifactForApp,
  recordsForApp,
  type HawkCompilerArtifact,
    type HawkCompilerDynamicBindingWire,
    type HawkCompilerDynamicValueWire,
    type HawkCompilerEventHandlerActionWire,
      type HawkCompilerEventHandlerWire,
      type HawkCompilerInitialDynamicValueWire,
      type HawkCompilerReactiveBindingWire,
      type HawkCompilerListTemplateNodeWire,
      type HawkCompilerListTemplateWire,
      type HawkCompilerTemplateScalarWire,
    type HawkElementSpec,
  type HawkEventSpec,
  type HawkLifecycleSpec,
} from "../../hawk2ui-native/src/index.ts";

export interface HawkSolidCompileInput {
  readonly filename: string;
  readonly source: string;
}

export interface HawkSolidCompileOutput {
  readonly framework: "solid";
  readonly filename: string;
  readonly records: readonly string[];
  readonly compilerArtifact: HawkCompilerArtifact;
}

type AstNode = Record<string, unknown>;
type LiteralRecord = Readonly<Record<string, string | number | boolean>>;

interface SolidLoweringContext {
  readonly arrays: ReadonlyMap<string, readonly LiteralRecord[]>;
  readonly components: ReadonlyMap<string, SolidComponentDefinition>;
  readonly initialDynamicValues: ReadonlyMap<string, HawkCompilerInitialDynamicValueWire>;
  readonly locals: ReadonlyMap<string, LiteralRecord>;
  readonly scalars: ReadonlyMap<string, string | number | boolean>;
  readonly childSlots: ReadonlyMap<string, readonly AstNode[]>;
  readonly componentStack: readonly string[];
  readonly reactivity: HawkCompilerReactiveBindingWire[];
  readonly dynamicBindings: HawkCompilerDynamicBindingWire[];
  readonly listTemplates: HawkCompilerListTemplateWire[];
  readonly pendingListTemplateAnchors: Map<string, string[]>;
}

interface ReturnedJsxElement {
  readonly element: AstNode;
  readonly scope: AstNode | undefined;
  readonly entrypoint: string;
}

interface SolidComponentDefinition {
  readonly element: AstNode;
  readonly propsParam: AstNode | undefined;
}

const VISUAL_PROP_NAMES = ["font_size", "color", "background"] as const;
const VIEW_ELEMENT_TAGS = new Set(["div", "section", "main", "article", "header", "footer", "nav", "aside", "form", "label", "ul", "ol", "li"]);
const TEXT_ELEMENT_TAGS = new Set(["span", "p", "strong", "em", "small", "code", "h1", "h2", "h3", "h4", "h5", "h6"]);
const SOLID_EVENT_PROPS: ReadonlyArray<readonly [string, HawkEventSpec["kind"]]> = [
  ["onClick", "pointer.press"],
  ["onPointerDown", "pointer.press"],
  ["onPointerUp", "pointer.release"],
  ["onPointerMove", "pointer.move"],
  ["onPointerDrag", "pointer.drag"],
  ["onPointerEnter", "pointer.enter"],
  ["onPointerLeave", "pointer.leave"],
  ["onWheel", "pointer.wheel"],
  ["onKeyDown", "keyboard.key-down"],
  ["onKeyUp", "keyboard.key-up"],
  ["onTextInput", "keyboard.text-input"],
  ["onFocus", "focus.focus-in"],
  ["onBlur", "focus.focus-out"],
  ["onInput", "input.value-changed"],
  ["onChange", "input.value-committed"],
  ["onResize", "resize"],
];
const SOLID_LIFECYCLE_PROPS: ReadonlyArray<readonly [string, HawkLifecycleSpec["phase"]]> = [
  ["onMount", "mounted"],
  ["onSuspend", "suspended"],
  ["onResume", "resumed"],
  ["onHotReload", "hot-reloaded"],
  ["onErrorBoundary", "error-boundary"],
  ["onShutdown", "shutdown"],
  ["onCleanup", "unmounted"],
];
const RESERVED_RUNTIME_PROP_NAMES = new Set<string>([
  "id",
  "key",
  "ref",
  "class",
  "className",
  "data-asset",
  "children",
  "width",
  "height",
  ...VISUAL_PROP_NAMES,
  ...SOLID_EVENT_PROPS.map(([name]) => name),
  ...SOLID_LIFECYCLE_PROPS.map(([name]) => name),
]);

export function compileHawkSolid(input: HawkSolidCompileInput): HawkSolidCompileOutput {
  if (!/\.[jt]sx$/.test(input.filename)) {
    throw new Error("Hawk2UI Solid compiler inputs must be .jsx or .tsx files.");
  }
  transformSync(input.source, {
    filename: input.filename,
    babelrc: false,
    configFile: false,
    presets: [
      ["babel-preset-solid", { generate: "dom" }],
      ["@babel/preset-typescript", { allExtensions: true, isTSX: true }],
    ],
  });

  const ast = parse(input.source, {
    sourceType: "module",
    plugins: ["jsx", "typescript"],
  }) as unknown as AstNode;
  const program = ast.program as AstNode;
  const returned = returnedJsxElement(program);
  if (!returned) {
    throw new Error("solid.root.missing: Solid compiler input must return one hawk root element.");
  }
  const signals = solidSignalsFromProgram(program, returned.scope);
  const context: SolidLoweringContext = {
    arrays: signals.arrays,
    components: componentDefinitionsFromProgram(program),
    initialDynamicValues: signals.initialDynamicValues,
    locals: new Map(),
    scalars: new Map(),
    childSlots: new Map(),
    componentStack: [],
      reactivity: [...signals.reactivity],
      dynamicBindings: [],
      listTemplates: [],
      pendingListTemplateAnchors: new Map(),
    };
    const root = withRootLifecycle(
      solidJsxElementToSpec(returned.element, context),
      solidLifecycleApiCalls(returned.scope),
    );
  validateUniqueChildKeys(root);
  context.reactivity.push({ kind: "effect", name: "root-props" });
  const app = { name: input.filename, root };
  const eventHandlers = eventHandlersForSpec(
    root,
    program,
    returned.scope,
    solidSetterBindingsFromProgram(program, returned.scope),
    context.listTemplates,
  );
  return {
    framework: "solid",
    filename: input.filename,
    records: recordsForApp(app),
      compilerArtifact: compilerArtifactForApp(
        app,
        uniqueReactivity(context.reactivity),
        context.dynamicBindings,
        [...context.initialDynamicValues.values()],
          {
            compiler: {
              framework: "solid",
              compiler: "@hawk2ui/solid",
              source_path: input.filename,
              entrypoint: returned.entrypoint,
            },
              eventHandlers,
              listTemplates: context.listTemplates,
            },
          ),
      };
  }

function returnedJsxElement(program: AstNode): ReturnedJsxElement | undefined {
  const exported = returnedJsxCandidates(program, true);
  if (exported.length > 0) return exported[0];
  const fallback = returnedJsxCandidates(program, false);
  return fallback[0];
}

function returnedJsxCandidates(program: AstNode, exportedOnly: boolean): ReturnedJsxElement[] {
  const candidates: ReturnedJsxElement[] = [];
  for (const statement of arrayField(program, "body")) {
    const declaration = statement.declaration as AstNode | undefined;
    const isExported = statement.type === "ExportNamedDeclaration" || statement.type === "ExportDefaultDeclaration";
    if (exportedOnly !== isExported) continue;
    const candidate = isExported && declaration ? declaration : statement;
    for (const returned of returnedJsxElementsFromCandidate(candidate)) candidates.push(returned);
  }
  return candidates;
}

function returnedJsxElementsFromCandidate(candidate: AstNode): ReturnedJsxElement[] {
  if (candidate.type === "FunctionDeclaration") {
    const returned = returnArgument(candidate.body as AstNode | undefined);
    return returned?.type === "JSXElement"
      ? [{
        element: returned,
        scope: candidate.body as AstNode | undefined,
        entrypoint: identifierName(candidate.id as AstNode | undefined) ?? "default",
      }]
      : [];
  }
  if (candidate.type !== "VariableDeclaration") return [];
  const returned: ReturnedJsxElement[] = [];
  for (const declaration of arrayField(candidate, "declarations")) {
    const name = identifierName(declaration.id as AstNode | undefined);
    const init = declaration.init as AstNode | undefined;
    const element = jsxElementFromFunctionLike(init);
    if (name && element) {
      returned.push({
        element,
        scope: functionBodyScope(init),
        entrypoint: name,
      });
    }
  }
  return returned;
}

function returnArgument(block: AstNode | undefined): AstNode | undefined {
  for (const statement of arrayField(block, "body")) {
    if (statement.type === "ReturnStatement") return statement.argument as AstNode | undefined;
  }
  return undefined;
}

function solidJsxElementToSpec(node: AstNode, context: SolidLoweringContext): HawkElementSpec {
  const tag = jsxTagName(node);
  if (!isHawkTag(tag)) return solidComponentElementToSpec(node, context);
  const id = requiredString(jsxAttributeValue(node, "id", context), tag, "id");
  const style = optionalString(jsxAttributeValue(node, "class", context));
  const assetPath = optionalString(jsxAttributeValue(node, "data-asset", context));
  if (assetPath && isUnsafeAssetPath(assetPath)) {
    throw new Error("solid.asset.path-invalid: Solid asset references must use workspace-relative paths.");
  }
  const key = optionalString(jsxAttributeValue(node, "key", context)) ?? id;
  const spec: HawkElementSpec = {
    id,
    kind: kindForTag(tag),
    key,
    refs: optionalString(jsxAttributeValue(node, "ref", context))
      ? [optionalString(jsxAttributeValue(node, "ref", context)) as string]
      : [],
    styleRefs: style ? [style] : [],
    assetRefs: assetPath ? [{ name: "solid.asset", path: assetPath }] : [],
    events: solidEvents(node),
    lifecycle: solidLifecycle(node),
    children: solidChildSpecs(node, context, id),
  };
  const props = runtimeProps(node, context, id, "solid");
  const text = solidTextContent(node, context, id);
  if (text) props.text = text;
  return Object.keys(props).length > 0 ? { ...spec, props } : spec;
}

function solidChildSpecs(node: AstNode, context: SolidLoweringContext, parentId: string): readonly HawkElementSpec[] {
  const children: HawkElementSpec[] = [];
  for (const child of arrayField(node, "children")) {
    for (const spec of solidChildNodeSpecs(child, context, parentId)) {
      anchorPendingListTemplates(context, parentId, spec.id);
      children.push(spec);
    }
  }
  return children;
}

function solidChildNodeSpecs(child: AstNode, context: SolidLoweringContext, parentId: string): readonly HawkElementSpec[] {
  if (child.type === "JSXElement" && jsxTagName(child) === "For") return expandSolidFor(child, context, parentId);
  if (child.type === "JSXElement" && jsxTagName(child) === "Index") return expandSolidIndex(child, context, parentId);
  if (child.type === "JSXElement" && jsxTagName(child) === "Show") return expandSolidShow(child, context, parentId);
  if (child.type === "JSXElement" && jsxTagName(child) === "Switch") return expandSolidSwitch(child, context, parentId);
  if (child.type === "JSXElement" && jsxTagName(child) === "Match") return expandSolidMatch(child, context, parentId);
  if (child.type === "JSXElement") return [solidJsxElementToSpec(child, context)];
  if (child.type !== "JSXExpressionContainer") return [];
  const slotName = componentChildSlotName(child.expression as AstNode | undefined, context);
  if (!slotName) return [];
  return (context.childSlots.get(slotName) ?? []).flatMap((slotChild) => solidChildNodeSpecs(slotChild, context, parentId));
}

function expandSolidShow(node: AstNode, context: SolidLoweringContext, parentId: string): readonly HawkElementSpec[] {
  const when = jsxRawAttributeValue(node, "when");
  if (when?.type !== "JSXExpressionContainer") {
    throw new Error("solid.show.unsupported: Solid Show must use `when={condition}`.");
  }
  return withSolidVisibilityBinding(
      solidChildSpecs(node, context, parentId),
    when.expression as AstNode | undefined,
    context,
    false,
  );
}

function expandSolidSwitch(node: AstNode, context: SolidLoweringContext, parentId: string): readonly HawkElementSpec[] {
  const branches: HawkElementSpec[] = [];
  for (const child of arrayField(node, "children")) {
    if (child.type !== "JSXElement" || jsxTagName(child) !== "Match") continue;
    branches.push(...expandSolidMatch(child, context, parentId));
  }
  return branches;
}

function expandSolidMatch(node: AstNode, context: SolidLoweringContext, parentId: string): readonly HawkElementSpec[] {
  const when = jsxRawAttributeValue(node, "when");
  if (when?.type !== "JSXExpressionContainer") {
    throw new Error("solid.match.unsupported: Solid Match must use `when={condition}`.");
  }
  return withSolidVisibilityBinding(
      solidChildSpecs(node, context, parentId),
    when.expression as AstNode | undefined,
    context,
    false,
  );
}

function expandSolidFor(node: AstNode, context: SolidLoweringContext, parentId: string): readonly HawkElementSpec[] {
  const each = jsxRawAttributeValue(node, "each");
  const source = solidSignalCallName((each?.expression as AstNode | undefined) ?? each);
  const callback = arrayField(node, "children")
    .find((child) => child.type === "JSXExpressionContainer")?.expression as AstNode | undefined;
  const itemName = identifierName((callback?.params as AstNode[] | undefined)?.[0]);
  const template = callback?.body as AstNode | undefined;
    if (!source || !itemName || !template || template.type !== "JSXElement") {
      throw new Error("solid.for.unsupported: Solid lists must use `<For each={items()}>{(item) => <hawk-* />}</For>`.");
    }
  const items = context.arrays.get(source);
  if (!items) {
    const initialValue = context.initialDynamicValues.get(source)?.value;
    if (initialValue?.type !== "array") {
      throw new Error(`solid.for.source-unresolved: Solid For source \`${source}\` must be a literal signal array or initial dynamic array.`);
    }
    context.reactivity.push({ kind: "keyed-for-each", name: source });
    context.listTemplates.push({
      id: `${parentId}:${source}`,
      parent_id: parentId,
      source,
      item: itemName,
      key: templateKeyExpression(template, itemName, "solid"),
      node: solidElementToListTemplateNode(template, context, itemName),
    });
    queuePendingListTemplateAnchor(context, parentId, `${parentId}:${source}`);
    return [];
  }
  context.reactivity.push({ kind: "keyed-for-each", name: source });
  return items.map((item) =>
    solidJsxElementToSpec(template, {
      ...context,
      locals: new Map([...context.locals, [itemName, item]]),
      }),
    );
}

function expandSolidIndex(node: AstNode, context: SolidLoweringContext, parentId: string): readonly HawkElementSpec[] {
  const each = jsxRawAttributeValue(node, "each");
  const source = solidSignalCallName((each?.expression as AstNode | undefined) ?? each);
  const callback = arrayField(node, "children")
    .find((child) => child.type === "JSXExpressionContainer")?.expression as AstNode | undefined;
  const itemName = identifierName((callback?.params as AstNode[] | undefined)?.[0]);
  const template = callback?.body as AstNode | undefined;
  if (!source || !itemName || !template || template.type !== "JSXElement") {
    throw new Error("solid.index.unsupported: Solid Index lists must use `<Index each={items()}>{(item) => <hawk-* />}</Index>`.");
  }
  const initialValue = context.initialDynamicValues.get(source)?.value;
  if (initialValue?.type !== "array") {
    throw new Error(`solid.index.source-unresolved: Solid Index source \`${source}\` must be an initial dynamic array.`);
  }
  context.reactivity.push({ kind: "keyed-for-each", name: source });
  context.listTemplates.push({
    id: `${parentId}:${source}`,
    parent_id: parentId,
    source,
    item: itemName,
    key: templateKeyExpression(template, itemName, "solid", true),
    node: solidElementToListTemplateNode(template, context, itemName, true),
  });
  queuePendingListTemplateAnchor(context, parentId, `${parentId}:${source}`);
  return [];
}

function queuePendingListTemplateAnchor(
  context: SolidLoweringContext,
  parentId: string,
  templateId: string,
): void {
  const pending = context.pendingListTemplateAnchors.get(parentId) ?? [];
  pending.push(templateId);
  context.pendingListTemplateAnchors.set(parentId, pending);
}

function anchorPendingListTemplates(
  context: SolidLoweringContext,
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

function solidElementToListTemplateNode(
  node: AstNode,
  context: SolidLoweringContext,
  itemName: string,
  itemAccessor = false,
): HawkCompilerListTemplateNodeWire {
  const tag = jsxTagName(node);
  if (!isHawkTag(tag)) {
    throw new Error(`solid.for.template-unsupported: Solid list template must render a native Hawk element, found \`${tag}\`.`);
  }
  const id = templateScalarFromAttribute(node, "id", context, itemName, "solid", itemAccessor);
  const key = optionalTemplateScalarFromAttribute(node, "key", context, itemName, "solid", itemAccessor) ?? id;
  const classAttr = optionalString(jsxAttributeValue(node, "class", context));
  const assetPath = optionalString(jsxAttributeValue(node, "data-asset", context));
  if (assetPath && isUnsafeAssetPath(assetPath)) {
    throw new Error(`solid.asset.path-invalid: asset path \`${assetPath}\` must be workspace-relative.`);
  }
  const props = templateProps(node, context, itemName, "solid", itemAccessor);
  const text = templateTextContent(node, context, itemName, "solid", itemAccessor);
  if (text) props.push({ name: "text", value: text });
  return {
    id,
    kind: kindForTag(tag),
    key,
    props,
    refs: optionalString(jsxAttributeValue(node, "ref", context))
      ? [optionalString(jsxAttributeValue(node, "ref", context)) as string]
      : [],
    style_refs: classAttr ? [classAttr] : [],
    asset_refs: assetPath ? [{ name: "solid.asset", path: assetPath }] : [],
    events: solidEvents(node).map((event) => ({
      kind: event.kind,
      handler: event.handler,
      payload_fields: [...payloadFieldsForEvent(event.kind)],
    })),
    lifecycle: solidLifecycle(node).map((lifecycle) => ({
      event: lifecycle.phase,
      handler: lifecycle.handler,
    })),
    children: arrayField(node, "children")
      .filter((child) => child.type === "JSXElement" && isHawkTag(jsxTagName(child)))
      .map((child) => solidElementToListTemplateNode(child, context, itemName, itemAccessor)),
  };
}

function templateProps(
  node: AstNode,
  context: SolidLoweringContext,
  itemName: string,
  framework: string,
  itemAccessor = false,
): { name: string; value: HawkCompilerTemplateScalarWire }[] {
  const props: { name: string; value: HawkCompilerTemplateScalarWire }[] = [];
  for (const name of ["width", "height", ...VISUAL_PROP_NAMES]) {
    const value = optionalTemplateScalarFromAttribute(node, name, context, itemName, framework, itemAccessor);
    if (value) props.push({ name, value });
  }
  for (const attribute of arrayField(node.openingElement as AstNode | undefined, "attributes")) {
    if (attribute.type !== "JSXAttribute") continue;
    const name = jsxName(attribute.name as AstNode | undefined);
    if (!name || RESERVED_RUNTIME_PROP_NAMES.has(name)) continue;
    props.push({ name, value: templateScalarFromValue(attribute.value as AstNode | undefined, context, itemName, framework, itemAccessor) });
  }
  return props;
}

function templateTextContent(
  node: AstNode,
  context: SolidLoweringContext,
  itemName: string,
  framework: string,
  itemAccessor = false,
): HawkCompilerTemplateScalarWire | undefined {
  const children = arrayField(node, "children").filter((child) => child.type === "JSXText" || child.type === "JSXExpressionContainer");
  if (children.length === 0) return undefined;
  if (children.length === 1) {
    const child = children[0];
    if (!child) return undefined;
    if (child.type === "JSXText") {
      const text = String(child.value ?? "").trim();
      return text ? literalTemplateScalar(text) : undefined;
    }
    return templateScalarFromExpression(child.expression as AstNode | undefined, context, itemName, framework, itemAccessor);
  }
  return {
    type: "expression",
    expression: children.map((child) => child.type === "JSXText" ? JSON.stringify(String(child.value ?? "")) : listTemplateExpressionSource(child.expression as AstNode | undefined, framework, itemName, itemAccessor)).join(" + "),
  };
}

function templateKeyExpression(node: AstNode, itemName: string, framework: string, itemAccessor = false): string {
  const key = jsxRawAttributeValue(node, "key");
  if (key?.type === "JSXExpressionContainer") return listTemplateExpressionSource(key.expression as AstNode | undefined, framework, itemName, itemAccessor);
  if (key?.type === "StringLiteral") return JSON.stringify(key.value);
  const id = jsxRawAttributeValue(node, "id");
  if (id?.type === "JSXExpressionContainer") return listTemplateExpressionSource(id.expression as AstNode | undefined, framework, itemName, itemAccessor);
  if (id?.type === "StringLiteral") return JSON.stringify(id.value);
  return `${itemName}.id`;
}

function templateScalarFromAttribute(
  node: AstNode,
  name: string,
  context: SolidLoweringContext,
  itemName: string,
  framework: string,
  itemAccessor = false,
): HawkCompilerTemplateScalarWire {
  const value = optionalTemplateScalarFromAttribute(node, name, context, itemName, framework, itemAccessor);
  if (!value) throw new Error(`${framework}.list-template.attribute-required: list template nodes require \`${name}\`.`);
  return value;
}

function optionalTemplateScalarFromAttribute(
  node: AstNode,
  name: string,
  context: SolidLoweringContext,
  itemName: string,
  framework: string,
  itemAccessor = false,
): HawkCompilerTemplateScalarWire | undefined {
  const value = jsxRawAttributeValue(node, name);
  if (!value) return undefined;
  return templateScalarFromValue(value, context, itemName, framework, itemAccessor);
}

function templateScalarFromValue(
  value: AstNode | undefined,
  context: SolidLoweringContext,
  itemName: string,
  framework: string,
  itemAccessor = false,
): HawkCompilerTemplateScalarWire {
  if (!value) return literalTemplateScalar(true);
  if (value.type === "StringLiteral") return literalTemplateScalar(value.value as string);
  if (value.type !== "JSXExpressionContainer") return literalTemplateScalar(true);
  return templateScalarFromExpression(value.expression as AstNode | undefined, context, itemName, framework, itemAccessor);
}

function templateScalarFromExpression(
  expression: AstNode | undefined,
  context: SolidLoweringContext,
  itemName: string,
  framework: string,
  itemAccessor = false,
): HawkCompilerTemplateScalarWire {
  const staticValue = staticTextExpressionValue(expression, context);
  if (staticValue !== undefined) return literalTemplateScalar(staticValue);
  const source = listTemplateExpressionSource(expression, framework, itemName, itemAccessor);
  if (!expressionDependencies(expression).includes(itemName)) {
    throw new Error(`${framework}.list-template.expression-unsupported: list template expressions must depend on \`${itemName}\`.`);
  }
  return { type: "expression", expression: source };
}

function listTemplateExpressionSource(
  expression: AstNode | undefined,
  framework: string,
  itemName: string,
  itemAccessor: boolean,
): string {
  const source = expressionSource(expression, framework);
  return itemAccessor ? source.replace(new RegExp(`\\b${escapeRegExp(itemName)}\\s*\\(\\s*\\)`, "g"), itemName) : source;
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function literalTemplateScalar(value: string | number | boolean): HawkCompilerTemplateScalarWire {
  if (typeof value === "string") return { type: "literal", value: { type: "string", value } };
  if (typeof value === "boolean") return { type: "literal", value: { type: "bool", value } };
  return { type: "literal", value: { type: "number", value } };
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

function solidComponentElementToSpec(node: AstNode, context: SolidLoweringContext): HawkElementSpec {
  const name = jsxTagName(node);
  const definition = context.components.get(name);
  if (!definition) {
    throw new Error(`solid.component.unresolved: local component \`${name}\` is not defined in this source file.`);
  }
  if (context.componentStack.includes(name)) {
    throw new Error(`solid.component.cycle: local component \`${name}\` recursively expands itself.`);
  }
  const scoped = scopedComponentContext(node, definition, context);
  return solidJsxElementToSpec(definition.element, scoped);
}

function scopedComponentContext(
  node: AstNode,
  definition: SolidComponentDefinition,
  context: SolidLoweringContext,
): SolidLoweringContext {
  const props = componentPropsFromJsx(node, context);
  const children = arrayField(node, "children");
  const locals = new Map(context.locals);
  const scalars = new Map(context.scalars);
  const childSlots = new Map(context.childSlots);
  bindComponentProps(definition.propsParam, props, children, locals, scalars, childSlots);
  return {
    ...context,
    locals,
    scalars,
    childSlots,
    componentStack: [...context.componentStack, jsxTagName(node)],
  };
}

function bindComponentProps(
  propsParam: AstNode | undefined,
  props: LiteralRecord,
  children: readonly AstNode[],
  locals: Map<string, LiteralRecord>,
  scalars: Map<string, string | number | boolean>,
  childSlots: Map<string, readonly AstNode[]>,
): void {
  if (!propsParam) return;
  if (propsParam.type === "Identifier") {
    const name = identifierName(propsParam);
    if (!name) return;
    locals.set(name, props);
    childSlots.set(name, children);
    return;
  }
  if (propsParam.type !== "ObjectPattern") {
    throw new Error("solid.component.props-unsupported: component props must use an identifier or object destructuring.");
  }
  for (const property of arrayField(propsParam, "properties")) {
    const key = identifierName(property.key as AstNode | undefined) ?? literalString(property.key as AstNode | undefined);
    const binding = identifierName(property.value as AstNode | undefined) ?? key;
    if (!key || !binding) {
      throw new Error("solid.component.props-unsupported: destructured component props must bind stable identifiers.");
    }
    if (key === "children") {
      childSlots.set(binding, children);
      continue;
    }
    const value = props[key];
    if (value !== undefined) scalars.set(binding, value);
  }
}

function componentPropsFromJsx(node: AstNode, context: SolidLoweringContext): LiteralRecord {
  const props: Record<string, string | number | boolean> = {};
  for (const attribute of arrayField(node.openingElement as AstNode | undefined, "attributes")) {
    if (attribute.type !== "JSXAttribute") {
      throw new Error("solid.component.props-unsupported: component prop spreads are not supported in compiler artifacts.");
    }
    const name = jsxName(attribute.name as AstNode | undefined);
    if (!name || name === "key") continue;
    props[name] = jsxAttributeValue(node, name, context) ?? true;
  }
  return props;
}

function componentDefinitionsFromProgram(program: AstNode): ReadonlyMap<string, SolidComponentDefinition> {
  const components = new Map<string, SolidComponentDefinition>();
  for (const statement of arrayField(program, "body")) {
    const declaration = statement.declaration as AstNode | undefined;
    const candidate = declaration ?? statement;
    collectComponentDefinitionsFromCandidate(candidate, components);
  }
  return components;
}

function collectComponentDefinitionsFromCandidate(
  candidate: AstNode,
  components: Map<string, SolidComponentDefinition>,
): void {
  if (candidate.type === "FunctionDeclaration") {
    const name = identifierName(candidate.id as AstNode | undefined);
    const element = jsxElementFromFunctionLike(candidate);
    if (name && isComponentTag(name) && element) {
      components.set(name, { element, propsParam: arrayField(candidate, "params")[0] });
    }
    return;
  }
  if (candidate.type !== "VariableDeclaration") return;
  for (const declaration of arrayField(candidate, "declarations")) {
    const name = identifierName(declaration.id as AstNode | undefined);
    const init = declaration.init as AstNode | undefined;
    const element = jsxElementFromFunctionLike(init);
    if (name && isComponentTag(name) && element) {
      components.set(name, { element, propsParam: arrayField(init, "params")[0] });
    }
  }
}

function jsxElementFromFunctionLike(node: AstNode | undefined): AstNode | undefined {
  if (!node) return undefined;
  if (
    node.type !== "FunctionDeclaration"
    && node.type !== "FunctionExpression"
    && node.type !== "ArrowFunctionExpression"
  ) {
    return undefined;
  }
  const body = node.body as AstNode | undefined;
  if (body?.type === "JSXElement") return body;
  const returned = returnArgument(body);
  return returned?.type === "JSXElement" ? returned : undefined;
}

function functionBodyScope(node: AstNode | undefined): AstNode | undefined {
  const body = node?.body as AstNode | undefined;
  return body?.type === "BlockStatement" ? body : undefined;
}

function componentChildSlotName(expression: AstNode | undefined, context: SolidLoweringContext): string | undefined {
  if (expression?.type === "Identifier") {
    const name = identifierName(expression);
    return name && context.childSlots.has(name) ? name : undefined;
  }
  if (expression?.type !== "MemberExpression") return undefined;
  const object = identifierName(expression.object as AstNode | undefined);
  const property = identifierName(expression.property as AstNode | undefined);
  return object && property === "children" && context.childSlots.has(object) ? object : undefined;
}

function solidEvents(node: AstNode): readonly HawkEventSpec[] {
  const events: HawkEventSpec[] = [];
  for (const [attribute, kind] of SOLID_EVENT_PROPS) {
    const value = jsxRawAttributeValue(node, attribute);
    if (value) events.push({ kind, handler: handlerName(value) });
  }
  return events;
}

function solidLifecycle(node: AstNode): readonly HawkLifecycleSpec[] {
  const lifecycle: HawkLifecycleSpec[] = [];
  for (const [attribute, phase] of SOLID_LIFECYCLE_PROPS) {
    const value = jsxRawAttributeValue(node, attribute);
    if (value) lifecycle.push({ phase, handler: handlerName(value) });
  }
  return lifecycle;
}

function solidLifecycleApiCalls(componentScope: AstNode | undefined): readonly HawkLifecycleSpec[] {
  const lifecycle: HawkLifecycleSpec[] = [];
  for (const statement of arrayField(componentScope, "body")) {
    if (statement.type !== "ExpressionStatement") continue;
    const call = statement.expression as AstNode | undefined;
    if (call?.type !== "CallExpression") continue;
    const name = callName(call.callee as AstNode | undefined);
    const argument = arrayField(call, "arguments")[0];
    if (name === "onMount") {
      for (const handler of lifecycleHandlerNamesFromArgument(argument, "solid", "onMount")) {
        pushLifecycle(lifecycle, "mounted", handler);
      }
      continue;
    }
    if (name === "onCleanup") {
      for (const handler of lifecycleHandlerNamesFromArgument(argument, "solid", "onCleanup")) {
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

function callName(callee: AstNode | undefined): string | undefined {
  if (callee?.type === "MemberExpression") return identifierName(callee.property as AstNode | undefined);
  return identifierName(callee);
}

function eventHandlersForSpec(
  root: HawkElementSpec,
  program: AstNode,
  componentScope: AstNode | undefined,
  setterBindings: ReadonlyMap<string, string>,
  listTemplates: readonly HawkCompilerListTemplateWire[],
): readonly HawkCompilerEventHandlerWire[] {
  const declarations = handlerDeclarationsFromBody([
    ...arrayField(program, "body"),
    ...arrayField(componentScope, "body"),
  ]);
  const lifecycleOnlyHandlers = lifecycleOnlyHandlerNames(root, listTemplates);
  return referencedHandlerNames(root, listTemplates).flatMap((name) => {
    const declaration = declarations.get(name);
    if (!declaration) {
      throw new Error(`solid.handler.missing: event handler \`${name}\` must be declared in the module or component scope.`);
    }
    const actions = handlerActions(name, declaration, setterBindings, "solid", lifecycleOnlyHandlers.has(name));
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

function handlerDeclarationsFromBody(statements: readonly AstNode[]): ReadonlyMap<string, AstNode> {
  const declarations = new Map<string, AstNode>();
  for (const statement of statements) {
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
  setterBindings: ReadonlyMap<string, string>,
  framework: string,
  allowEmpty: boolean,
): readonly HawkCompilerEventHandlerActionWire[] {
  const body = declaration.body as AstNode | undefined;
  if (!body) {
    throw new Error(`${framework}.handler.unsupported: event handler \`${handler}\` must have an executable body.`);
  }
  if (body.type !== "BlockStatement") {
    return [handlerActionFromExpression(handler, body, setterBindings, framework)];
  }
  const actions = arrayField(body, "body").map((statement) => {
    if (statement.type !== "ExpressionStatement") {
      throw new Error(`${framework}.handler.unsupported: event handler \`${handler}\` contains unsupported statements.`);
    }
    return handlerActionFromExpression(handler, statement.expression as AstNode | undefined, setterBindings, framework);
  });
    if (actions.length === 0 && !allowEmpty) {
      throw new Error(`${framework}.handler.unsupported: event handler \`${handler}\` must contain at least one action.`);
    }
  return actions;
}

function handlerActionFromExpression(
  handler: string,
  expression: AstNode | undefined,
  setterBindings: ReadonlyMap<string, string>,
  framework: string,
): HawkCompilerEventHandlerActionWire {
  if (expression?.type === "AssignmentExpression" && expression.operator === "=") {
    const name = identifierName(expression.left as AstNode | undefined);
    if (!name) {
      throw new Error(`${framework}.handler.unsupported: event handler \`${handler}\` assignment target must be a dynamic value name.`);
    }
    return dynamicUpdateAction(name, expression.right as AstNode | undefined, framework);
  }
  if (expression?.type === "CallExpression") {
    const setter = identifierName(expression.callee as AstNode | undefined);
    const name = setter ? setterBindings.get(setter) : undefined;
    const argument = (expression.arguments as AstNode[] | undefined)?.[0];
    if (name && argument) {
      return dynamicUpdateAction(name, argument, framework);
    }
  }
  throw new Error(`${framework}.handler.unsupported: event handler \`${handler}\` must assign a dynamic value or call a signal setter.`);
}

function dynamicUpdateAction(
  name: string,
  expression: AstNode | undefined,
  framework: string,
): HawkCompilerEventHandlerActionWire {
  const value = literalDynamicValue(expression);
  if (value) {
    return { type: "set_dynamic_value", name, value };
  }
  return {
    type: "set_dynamic_expression",
    name,
    expression: expressionSource(expression, framework),
    dependencies: expressionDependencies(expression),
  };
}

function jsxAttributeValue(
  node: AstNode,
  name: string,
  context: SolidLoweringContext,
): string | number | boolean | undefined {
  const value = jsxRawAttributeValue(node, name);
  if (!value) return undefined;
  if (value.type === "StringLiteral") return value.value as string;
  if (value.type === "JSXExpressionContainer") return evaluateExpression(value.expression as AstNode | undefined, context);
  return true;
}

function jsxRawAttributeValue(node: AstNode, name: string): AstNode | undefined {
  const attribute = arrayField(node.openingElement as AstNode | undefined, "attributes").find(
    (item) => item.type === "JSXAttribute" && jsxName(item.name as AstNode | undefined) === name,
  );
  return attribute?.value as AstNode | undefined;
}

function solidTextContent(
  node: AstNode,
  context: SolidLoweringContext,
  nodeId: string,
): string | undefined {
  const staticValues: string[] = [];
  const dynamicExpressionParts: string[] = [];
  const dependencies = new Set<string>();
  let hasDynamicExpression = false;

  for (const child of arrayField(node, "children")) {
    if (child.type === "JSXText") {
      const value = String(child.value ?? "").trim();
      if (value.length > 0) {
        staticValues.push(value);
        dynamicExpressionParts.push(JSON.stringify(value));
      }
      continue;
    }
    if (child.type !== "JSXExpressionContainer") continue;
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
    dynamicExpressionParts.push(expressionSource(expression, "solid"));
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
  context: SolidLoweringContext,
): string | number | boolean | undefined {
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
  return undefined;
}

function layoutProps(
  node: AstNode,
  context: SolidLoweringContext,
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
  context: SolidLoweringContext,
  nodeId: string,
  framework: string,
): Record<string, string | number | boolean> {
  const props = layoutProps(node, context, nodeId, framework);
  for (const name of VISUAL_PROP_NAMES) {
    const value = dynamicVisualAttributeValue(node, name, context, nodeId, framework);
    if (value !== undefined) props[name] = value;
  }
  for (const attribute of arrayField(node.openingElement as AstNode | undefined, "attributes")) {
    if (attribute.type !== "JSXAttribute") continue;
    const name = jsxName(attribute.name as AstNode | undefined);
    if (!name || RESERVED_RUNTIME_PROP_NAMES.has(name)) continue;
    const value = dynamicRuntimeJsxAttributeValue(attribute.value as AstNode | undefined, name, context, nodeId, framework);
    if (value !== undefined) props[name] = value;
  }
  return props;
}

function withSolidVisibilityBinding(
  specs: readonly HawkElementSpec[],
  expression: AstNode | undefined,
  context: SolidLoweringContext,
  negate: boolean,
): readonly HawkElementSpec[] {
  const dependencies = expressionDependencies(expression);
  if (dependencies.length > 0) {
    const source = expressionSource(expression, "solid");
    const visibleExpression = negate ? `!(${source})` : source;
    for (const spec of specs) {
      context.dynamicBindings.push({
        node_id: spec.id,
        target: { type: "prop", name: "visible" },
        expression: visibleExpression,
        dependencies,
      });
    }
    return specs;
  }

  const staticValue = staticTextExpressionValue(expression, context);
  if (typeof staticValue !== "boolean") {
    throw new Error("solid.show.unsupported: Solid Show must use a boolean expression.");
  }
  return specs.map((spec) => ({
    ...spec,
    props: { ...(spec.props ?? {}), visible: negate ? !staticValue : staticValue },
  }));
}

function dynamicRuntimeJsxAttributeValue(
  value: AstNode | undefined,
  name: string,
  context: SolidLoweringContext,
  nodeId: string,
  framework: string,
): string | number | boolean | undefined {
  if (!value) return true;
  if (value.type === "StringLiteral") return value.value as string;
  if (value.type !== "JSXExpressionContainer") {
    throw new Error(`${framework}.attribute.unsupported: prop \`${name}\` on \`${nodeId}\` must be a static scalar or expression.`);
  }
  const expression = value.expression as AstNode | undefined;
  const staticValue = staticTextExpressionValue(expression, context);
  if (staticValue !== undefined) return staticValue;
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
  context: SolidLoweringContext,
  nodeId: string,
  framework: string,
): string | number | boolean | undefined {
  const value = jsxRawAttributeValue(node, name);
  if (!value) return undefined;
  if (value.type === "StringLiteral") return value.value as string;
  if (value.type !== "JSXExpressionContainer") {
    throw new Error(`${framework}.attribute.unsupported: visual prop \`${name}\` on \`${nodeId}\` must be a static scalar or expression.`);
  }
  const expression = value.expression as AstNode | undefined;
  const staticValue = staticTextExpressionValue(expression, context);
  if (staticValue !== undefined) return staticValue;
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
  context: SolidLoweringContext,
  nodeId: string,
  framework: string,
): number | undefined {
  const value = jsxRawAttributeValue(node, name);
  if (!value) return undefined;
  if (value.type === "StringLiteral") return layoutNumber(value.value, nodeId, name, framework);
  if (value.type !== "JSXExpressionContainer") {
    throw new Error(`${framework}.attribute.unsupported: layout prop \`${name}\` must be numeric.`);
  }
  const expression = value.expression as AstNode | undefined;
  const staticValue = staticTextExpressionValue(expression, context);
  if (staticValue !== undefined) return layoutNumber(staticValue, nodeId, name, framework);
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

function solidSignalsFromProgram(program: AstNode, componentScope: AstNode | undefined): {
  readonly arrays: ReadonlyMap<string, readonly LiteralRecord[]>;
  readonly initialDynamicValues: ReadonlyMap<string, HawkCompilerInitialDynamicValueWire>;
  readonly reactivity: readonly HawkCompilerReactiveBindingWire[];
} {
  const arrays = new Map<string, readonly LiteralRecord[]>();
  const initialDynamicValues = new Map<string, HawkCompilerInitialDynamicValueWire>();
  const reactivity: HawkCompilerReactiveBindingWire[] = [];
  collectSolidSignalsFromBody(arrayField(program, "body"), arrays, initialDynamicValues, reactivity);
  collectSolidSignalsFromBody(arrayField(componentScope, "body"), arrays, initialDynamicValues, reactivity);
  return { arrays, initialDynamicValues, reactivity };
}

function solidSetterBindingsFromProgram(program: AstNode, componentScope: AstNode | undefined): ReadonlyMap<string, string> {
  const bindings = new Map<string, string>();
  collectSolidSetterBindings(arrayField(program, "body"), bindings);
  collectSolidSetterBindings(arrayField(componentScope, "body"), bindings);
  return bindings;
}

function collectSolidSetterBindings(statements: readonly AstNode[], bindings: Map<string, string>): void {
  for (const statement of statements) {
    if (statement.type !== "VariableDeclaration") continue;
    for (const declaration of arrayField(statement, "declarations")) {
      const init = declaration.init as AstNode | undefined;
      if (init?.type !== "CallExpression" || identifierName(init.callee as AstNode | undefined) !== "createSignal") {
        continue;
      }
      const pair = signalBindingPair(declaration.id as AstNode | undefined);
      if (pair) bindings.set(pair.setter, pair.signal);
    }
  }
}

function collectSolidSignalsFromBody(
  statements: readonly AstNode[],
  arrays: Map<string, readonly LiteralRecord[]>,
  initialDynamicValues: Map<string, HawkCompilerInitialDynamicValueWire>,
  reactivity: HawkCompilerReactiveBindingWire[],
): void {
  for (const statement of statements) {
    if (statement.type === "ExpressionStatement") {
      const effect = solidCreateEffectBinding(statement.expression as AstNode | undefined);
      if (effect) reactivity.push(effect);
      continue;
    }
    if (statement.type !== "VariableDeclaration") continue;
    for (const declaration of arrayField(statement, "declarations")) {
      const signalName = signalBindingName(declaration.id as AstNode | undefined);
      const init = declaration.init as AstNode | undefined;
      const signalValue = init?.type === "CallExpression" && identifierName(init.callee as AstNode | undefined) === "createSignal"
        ? literalDynamicValue((init.arguments as AstNode[] | undefined)?.[0])
        : undefined;
      const values = signalValue ? undefined : literalObjectArray(init);
      if (signalName && values) {
        arrays.set(signalName, values);
        reactivity.push({ kind: "signal", name: signalName });
      }
      if (signalName && signalValue) {
        reactivity.push({ kind: "signal", name: signalName });
      }
        if (signalName && signalValue) {
          initialDynamicValues.set(signalName, {
            name: signalName,
            mode: "getter",
            value: signalValue,
          });
        }
        const name = identifierName(declaration.id as AstNode | undefined);
        const memoValue = isCreateMemoCall(init)
          ? literalDynamicValueFromFunction((init.arguments as AstNode[] | undefined)?.[0])
          : undefined;
        if (name && memoValue) {
          initialDynamicValues.set(name, { name, mode: "getter", value: memoValue });
          reactivity.push({ kind: "effect", name });
          continue;
        }
        const value = literalDynamicValue(init);
        if (name && value) {
          initialDynamicValues.set(name, { name, mode: "value", value });
        }
      }
  }
}

function solidCreateEffectBinding(
  expression: AstNode | undefined,
): HawkCompilerReactiveBindingWire | undefined {
  if (expression?.type !== "CallExpression") return undefined;
  if (identifierName(expression.callee as AstNode | undefined) !== "createEffect") return undefined;
  const callback = arrayField(expression, "arguments")[0];
  const callbackName = identifierName(callback);
  if (!callbackName) {
    throw new Error("solid.effect.unsupported: createEffect callbacks must be stable function identifiers.");
  }
  return { kind: "effect", name: `createEffect:${callbackName}` };
}

function signalBindingName(node: AstNode | undefined): string | undefined {
  if (node?.type === "Identifier") return identifierName(node);
  if (node?.type !== "ArrayPattern") return undefined;
  return identifierName((node.elements as AstNode[] | undefined)?.[0]);
}

function isCreateMemoCall(node: AstNode | undefined): node is AstNode & { arguments?: AstNode[] } {
  return node?.type === "CallExpression" && identifierName(node.callee as AstNode | undefined) === "createMemo";
}

function literalDynamicValueFromFunction(callback: AstNode | undefined): HawkCompilerDynamicValueWire | undefined {
  if (callback?.type !== "ArrowFunctionExpression" && callback?.type !== "FunctionExpression") {
    return undefined;
  }
  const body = callback.body as AstNode | undefined;
  if (body?.type !== "BlockStatement") return literalDynamicValue(body);
  for (const statement of arrayField(body, "body")) {
    if (statement.type === "ReturnStatement") {
      return literalDynamicValue(statement.argument as AstNode | undefined);
    }
  }
  return undefined;
}

function signalBindingPair(node: AstNode | undefined): { readonly signal: string; readonly setter: string } | undefined {
  if (node?.type !== "ArrayPattern") return undefined;
  const elements = node.elements as AstNode[] | undefined;
  const signal = identifierName(elements?.[0]);
  const setter = identifierName(elements?.[1]);
  return signal && setter ? { signal, setter } : undefined;
}

function literalObjectArray(node: AstNode | undefined): readonly LiteralRecord[] | undefined {
  if (!node || node.type !== "ArrayExpression") return undefined;
  return arrayField(node, "elements").map((item) => {
    if (item.type !== "ObjectExpression") {
      throw new Error("solid.literal-array.unsupported: For sources must be arrays of literal objects.");
    }
    const record: Record<string, string | number | boolean> = {};
    for (const property of arrayField(item, "properties")) {
      const key = identifierName(property.key as AstNode | undefined) ?? literalString(property.key as AstNode | undefined);
      const value = literalValue(property.value as AstNode | undefined);
      if (!key || value === undefined) {
        throw new Error("solid.literal-array.unsupported: literal object properties must be scalar values.");
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

function evaluateExpression(
  expression: AstNode | undefined,
  context: SolidLoweringContext,
): string | number | boolean {
  const literal = literalValue(expression);
  if (literal !== undefined) return literal;
  if (expression?.type === "Identifier") {
    const name = identifierName(expression);
    const value = name ? context.scalars.get(name) : undefined;
    return value !== undefined ? value : name ?? "";
  }
  if (expression?.type === "MemberExpression") {
    const object = identifierName(expression.object as AstNode | undefined);
    const property = identifierName(expression.property as AstNode | undefined);
    const record = object ? context.locals.get(object) : undefined;
    const value = property ? record?.[property] : undefined;
    if (value !== undefined) return value;
  }
  throw new Error("solid.expression.unsupported: compiler artifact expressions must resolve to literal values.");
}

function handlerName(value: AstNode | undefined): string {
  if (value?.type === "StringLiteral") return String(value.value);
  if (value?.type === "JSXExpressionContainer") {
    const expression = value.expression as AstNode | undefined;
    const name = identifierName(expression);
    if (name) return name;
  }
  throw new Error("solid.handler.unsupported: event handlers must be stable identifiers.");
}

function solidSignalCallName(node: AstNode | undefined): string | undefined {
  return node?.type === "CallExpression" ? identifierName(node.callee as AstNode | undefined) : undefined;
}

function literalValue(node: AstNode | undefined): string | number | boolean | undefined {
  if (!node) return undefined;
  if (node.type === "StringLiteral" || node.type === "NumericLiteral" || node.type === "BooleanLiteral") {
    const value = node.value;
    if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") return value;
  }
  return undefined;
}

function expressionSource(expression: AstNode | undefined, framework: string): string {
  if (!expression) {
    throw new Error(`${framework}.expression.unsupported: dynamic text bindings require an expression.`);
  }
  const source = generate(expression as never, { concise: true }).code.trim();
  if (!source) {
    throw new Error(`${framework}.expression.unsupported: dynamic text bindings require a printable expression.`);
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

function literalString(node: AstNode | undefined): string | undefined {
  const value = literalValue(node);
  return typeof value === "string" ? value : undefined;
}

function isHawkJsxElement(node: AstNode | undefined): boolean {
  return node?.type === "JSXElement" && isHawkTag(jsxTagName(node));
}

function jsxTagName(node: AstNode): string {
  return jsxName((node.openingElement as AstNode).name as AstNode);
}

function jsxName(node: AstNode | undefined): string {
  if (typeof node?.name === "string") return node.name;
  return "";
}

function identifierName(node: AstNode | undefined): string | undefined {
  return typeof node?.name === "string" ? node.name : undefined;
}

function optionalString(value: string | number | boolean | undefined): string | undefined {
  return typeof value === "string" && value.trim() ? value : undefined;
}

function requiredString(value: string | number | boolean | undefined, tag: string, attribute: string): string {
  if (typeof value === "string" && value.trim()) return value;
  throw new Error(`solid.attribute.required: ${tag} requires a stable ${attribute} attribute.`);
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
  throw new Error(`solid.element.unsupported: unsupported Hawk element \`${tag}\`.`);
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

function uniqueReactivity(
  bindings: readonly HawkCompilerReactiveBindingWire[],
): readonly HawkCompilerReactiveBindingWire[] {
  const seen = new Set<string>();
  return bindings.filter((binding) => {
    const key = `${binding.kind}:${binding.name}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

function validateUniqueChildKeys(element: HawkElementSpec): void {
  const keys = new Set<string>();
  for (const child of element.children ?? []) {
    if (child.key) {
      if (keys.has(child.key)) {
        throw new Error(`solid.child-key.duplicate: duplicate Solid child key \`${child.key}\``);
      }
      keys.add(child.key);
    }
    validateUniqueChildKeys(child);
  }
}
