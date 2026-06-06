import { transformSync } from "@babel/core";
import generate from "@babel/generator";
import { parse } from "@babel/parser";
import {
  compilerArtifactForApp,
  recordsForApp,
  type HawkCompilerArtifact,
  type HawkCompilerDynamicBindingWire,
  type HawkCompilerDynamicValueWire,
  type HawkCompilerInitialDynamicValueWire,
  type HawkCompilerReactiveBindingWire,
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

export interface HawkSolidRenderOptions {
  readonly target: { readonly id: string };
}

export interface HawkSolidDisposer {
  (): void;
  readonly records: readonly string[];
  readonly update: () => void;
}

type AstNode = Record<string, unknown>;
type LiteralRecord = Readonly<Record<string, string | number | boolean>>;

interface SolidLoweringContext {
  readonly arrays: ReadonlyMap<string, readonly LiteralRecord[]>;
  readonly initialDynamicValues: ReadonlyMap<string, HawkCompilerInitialDynamicValueWire>;
  readonly locals: ReadonlyMap<string, LiteralRecord>;
  readonly reactivity: HawkCompilerReactiveBindingWire[];
  readonly dynamicBindings: HawkCompilerDynamicBindingWire[];
}

interface ReturnedJsxElement {
  readonly element: AstNode;
  readonly scope: AstNode | undefined;
  readonly entrypoint: string;
}

const VISUAL_PROP_NAMES = ["font_size", "color", "background"] as const;

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
    initialDynamicValues: signals.initialDynamicValues,
    locals: new Map(),
    reactivity: [...signals.reactivity],
    dynamicBindings: [],
  };
  const root = solidJsxElementToSpec(returned.element, context);
  validateUniqueChildKeys(root);
  context.reactivity.push({ kind: "effect", name: "root-props" });
  const app = { name: input.filename, root };
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
        },
      ),
    };
  }

function returnedJsxElement(program: AstNode): ReturnedJsxElement | undefined {
  for (const statement of arrayField(program, "body")) {
    const declaration = statement.declaration as AstNode | undefined;
    const candidate = statement.type === "ExportNamedDeclaration" && declaration ? declaration : statement;
    if (candidate.type === "FunctionDeclaration") {
        const returned = returnArgument(candidate.body as AstNode | undefined);
        if (returned && isHawkJsxElement(returned)) {
          return {
            element: returned,
            scope: candidate.body as AstNode | undefined,
            entrypoint: identifierName(candidate.id as AstNode | undefined) ?? "default",
          };
        }
      }
  }
  return undefined;
}

function returnArgument(block: AstNode | undefined): AstNode | undefined {
  for (const statement of arrayField(block, "body")) {
    if (statement.type === "ReturnStatement") return statement.argument as AstNode | undefined;
  }
  return undefined;
}

function solidJsxElementToSpec(node: AstNode, context: SolidLoweringContext): HawkElementSpec {
  const tag = jsxTagName(node);
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
    children: solidChildSpecs(node, context),
  };
  const props = runtimeProps(node, context, id, "solid");
  const text = solidTextContent(node, context, id);
  if (text) props.text = text;
  return Object.keys(props).length > 0 ? { ...spec, props } : spec;
}

function solidChildSpecs(node: AstNode, context: SolidLoweringContext): readonly HawkElementSpec[] {
  const children: HawkElementSpec[] = [];
  for (const child of arrayField(node, "children")) {
    if (isHawkJsxElement(child)) {
      children.push(solidJsxElementToSpec(child, context));
    } else if (child.type === "JSXElement" && jsxTagName(child) === "For") {
      children.push(...expandSolidFor(child, context));
    }
  }
  return children;
}

function expandSolidFor(node: AstNode, context: SolidLoweringContext): readonly HawkElementSpec[] {
  const each = jsxRawAttributeValue(node, "each");
  const source = solidSignalCallName((each?.expression as AstNode | undefined) ?? each);
  const callback = arrayField(node, "children")
    .find((child) => child.type === "JSXExpressionContainer")?.expression as AstNode | undefined;
  const itemName = identifierName((callback?.params as AstNode[] | undefined)?.[0]);
  const template = callback?.body as AstNode | undefined;
  if (!source || !itemName || !template || !isHawkJsxElement(template)) {
    throw new Error("solid.for.unsupported: Solid lists must use `<For each={items()}>{(item) => <hawk-* />}</For>`.");
  }
  const items = context.arrays.get(source);
  if (!items) {
    throw new Error(`solid.for.source-unresolved: Solid For source \`${source}\` must be a literal signal array.`);
  }
  context.reactivity.push({ kind: "keyed-for-each", name: source });
  return items.map((item) =>
    solidJsxElementToSpec(template, {
      ...context,
      locals: new Map([...context.locals, [itemName, item]]),
    }),
  );
}

function solidEvents(node: AstNode): readonly HawkEventSpec[] {
  return hasJsxAttribute(node, "onPointerDown")
    ? [{ kind: "pointer.press", handler: handlerName(jsxRawAttributeValue(node, "onPointerDown")) }]
    : [];
}

function solidLifecycle(node: AstNode): readonly HawkLifecycleSpec[] {
  const lifecycle: HawkLifecycleSpec[] = [];
  const mounted = jsxRawAttributeValue(node, "onMount");
  if (mounted) lifecycle.push({ phase: "mounted", handler: handlerName(mounted) });
  const cleanup = jsxRawAttributeValue(node, "onCleanup");
  if (cleanup) lifecycle.push({ phase: "unmounted", handler: handlerName(cleanup) });
  return lifecycle;
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
  return props;
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

function collectSolidSignalsFromBody(
  statements: readonly AstNode[],
  arrays: Map<string, readonly LiteralRecord[]>,
  initialDynamicValues: Map<string, HawkCompilerInitialDynamicValueWire>,
  reactivity: HawkCompilerReactiveBindingWire[],
): void {
  for (const statement of statements) {
    if (statement.type !== "VariableDeclaration") continue;
    for (const declaration of arrayField(statement, "declarations")) {
      const signalName = signalBindingName(declaration.id as AstNode | undefined);
      const init = declaration.init as AstNode | undefined;
      const signalValue = init?.type === "CallExpression" && identifierName(init.callee as AstNode | undefined) === "createSignal"
        ? literalDynamicValue((init.arguments as AstNode[] | undefined)?.[0])
        : undefined;
      const values = signalValue?.type === "array"
        ? literalObjectArray((init?.arguments as AstNode[] | undefined)?.[0])
        : literalObjectArray(init);
      if (signalName && values) {
        arrays.set(signalName, values);
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
      const value = literalDynamicValue(init);
      if (name && value) {
        initialDynamicValues.set(name, { name, mode: "value", value });
      }
    }
  }
}

function signalBindingName(node: AstNode | undefined): string | undefined {
  if (node?.type === "Identifier") return identifierName(node);
  if (node?.type !== "ArrayPattern") return undefined;
  return identifierName((node.elements as AstNode[] | undefined)?.[0]);
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
  if (expression?.type === "Identifier") return identifierName(expression) ?? "";
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

function hasJsxAttribute(node: AstNode, name: string): boolean {
  return Boolean(jsxRawAttributeValue(node, name));
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
  throw new Error(`solid.element.unsupported: unsupported Hawk element \`${tag}\`.`);
}

function isHawkTag(tag: string): boolean {
  return tag.startsWith("hawk-");
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

export function renderHawkSolid(component: () => unknown, options: HawkSolidRenderOptions): HawkSolidDisposer {
  if (!options.target.id.trim()) {
    throw new Error("Hawk2UI Solid render targets require a stable id.");
  }
  const records: string[] = [];
  let root = componentToNativeSpec(component(), options.target.id);
  validateUniqueChildKeys(root);
  records.push(...recordsForApp({ name: `solid:${options.target.id}`, root }));
  const dispose = (() => {
    records.push(`unmount-element:${root.id}`);
  }) as HawkSolidDisposer;
  Object.defineProperty(dispose, "records", {
    enumerable: true,
    get: () => records,
  });
  Object.defineProperty(dispose, "update", {
    enumerable: true,
    value: () => {
      const next = componentToNativeSpec(component(), options.target.id);
      validateUniqueChildKeys(next);
      records.push(...diffRecords(root, next));
      root = next;
    },
  });
  return dispose;
}

function componentToNativeSpec(component: unknown, fallbackId: string): HawkElementSpec {
  const props = readRecord(component);
  const id = readString(props, "id") ?? fallbackId;
  const asset = readString(props, "asset");
  return {
    id,
    kind: "view",
    refs: readString(props, "ref") ? [readString(props, "ref") as string] : [],
    styleRefs: readString(props, "class") ? [readString(props, "class") as string] : [],
    assetRefs: asset ? [{ name: "solid.asset", path: asset }] : [],
    events: readStringArray(props, "on").includes("pointer.press")
      ? [{ kind: "pointer.press", handler: "handlePress" }]
      : [],
    children: readChildren(props).map(runtimeChildSpec),
  };
}

function runtimeChildSpec(child: Record<string, unknown>, index: number): HawkElementSpec {
  const id = readString(child, "id") ?? `child-${index}`;
  const key = readString(child, "key") ?? readString(child, "id");
  const props = readTextProp(child);
  return {
    id,
    kind: "text",
    ...(key ? { key } : {}),
    ...(props ? { props } : {}),
  };
}

function readRecord(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === "object" ? (value as Record<string, unknown>) : undefined;
}

function readChildren(record: Record<string, unknown> | undefined): readonly Record<string, unknown>[] {
  const children = record?.children;
  return Array.isArray(children)
    ? children.filter((child): child is Record<string, unknown> => Boolean(child) && typeof child === "object")
    : [];
}

function readString(record: Record<string, unknown> | undefined, name: string): string | undefined {
  const value = record?.[name];
  return typeof value === "string" && value.trim() ? value : undefined;
}

function readStringArray(record: Record<string, unknown> | undefined, name: string): readonly string[] {
  const value = record?.[name];
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];
}

function readTextProp(record: Record<string, unknown>): Record<string, string> | undefined {
  const text = readString(record, "text");
  return text ? { text } : undefined;
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

function diffRecords(previous: HawkElementSpec, next: HawkElementSpec): readonly string[] {
  const records: string[] = [];
  if (previous.id !== next.id) {
    records.push(`remove-element:${previous.id}`);
    records.push(...recordsForApp({ name: `solid:${next.id}`, root: next }));
    return records;
  }
  if ((previous.styleRefs ?? []).join(" ") !== (next.styleRefs ?? []).join(" ")) {
    for (const style of next.styleRefs ?? []) {
      records.push(`style:${next.id}:${style}`);
    }
  }
  emitPropDiffs(previous, next, records);
  emitChildDiffs(previous, next, records);
  return records;
}

function emitPropDiffs(previous: HawkElementSpec, next: HawkElementSpec, records: string[]): void {
  const names = new Set([...Object.keys(previous.props ?? {}), ...Object.keys(next.props ?? {})]);
  for (const name of [...names].sort()) {
    const previousValue = previous.props?.[name];
    const nextValue = next.props?.[name];
    if (previousValue !== nextValue && nextValue !== undefined) {
      records.push(`prop:${next.id}:${name}=${String(nextValue)}`);
    }
  }
}

function emitChildDiffs(previous: HawkElementSpec, next: HawkElementSpec, records: string[]): void {
  const previousChildren = new Map((previous.children ?? []).map((child) => [child.key ?? child.id, child]));
  const nextChildren = new Map((next.children ?? []).map((child) => [child.key ?? child.id, child]));
  for (const [key, child] of nextChildren) {
    const previousChild = previousChildren.get(key);
    if (!previousChild) {
      records.push(...recordsForApp({ name: `solid:${child.id}`, root: child }));
    } else {
      records.push(...diffRecords(previousChild, child));
    }
  }
  for (const [key, child] of previousChildren) {
    if (!nextChildren.has(key)) {
      records.push(`remove-element:${child.id}`);
    }
  }
}
