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
  type HawkElementSpec,
  type HawkEventSpec,
  type HawkLifecycleSpec,
} from "../../hawk2ui-native/src/index.ts";

export interface HawkReactCompileInput {
  readonly filename: string;
  readonly source: string;
}

export interface HawkReactCompileOutput {
  readonly framework: "react";
  readonly filename: string;
  readonly records: readonly string[];
  readonly compilerArtifact: HawkCompilerArtifact;
}

export interface HawkReactRoot {
  readonly records: readonly string[];
  readonly render: (element: unknown) => void;
  readonly unmount: () => void;
}

type AstNode = Record<string, unknown>;
type LiteralRecord = Readonly<Record<string, string | number | boolean>>;

interface ReactLoweringContext {
  readonly arrays: ReadonlyMap<string, readonly LiteralRecord[]>;
  readonly initialDynamicValues: ReadonlyMap<string, HawkCompilerInitialDynamicValueWire>;
  readonly locals: ReadonlyMap<string, LiteralRecord>;
  readonly dynamicBindings: HawkCompilerDynamicBindingWire[];
}

interface ReturnedJsxElement {
  readonly element: AstNode;
  readonly scope: AstNode | undefined;
}

const VISUAL_PROP_NAMES = ["font_size", "color", "background"] as const;

export function compileHawkReact(input: HawkReactCompileInput): HawkReactCompileOutput {
  if (!/\.[jt]sx$/.test(input.filename)) {
    throw new Error("Hawk2UI React compiler inputs must be .jsx or .tsx files.");
  }
  transformSync(input.source, {
    filename: input.filename,
    babelrc: false,
    configFile: false,
    presets: [
      ["@babel/preset-react", { runtime: "automatic" }],
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
    throw new Error("react.root.missing: React compiler input must return one hawk root element.");
  }

  const context: ReactLoweringContext = {
    arrays: literalArraysFromProgram(program, returned.scope),
    initialDynamicValues: initialDynamicValuesFromProgram(program, returned.scope),
    locals: new Map(),
    dynamicBindings: [],
  };
  const root = jsxElementToSpec(returned.element, context);
  validateUniqueChildKeys(root);
  const app = { name: input.filename, root };
  return {
    framework: "react",
    filename: input.filename,
    records: recordsForApp(app),
    compilerArtifact: compilerArtifactForApp(
      app,
      [],
      context.dynamicBindings,
      [...context.initialDynamicValues.values()],
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
        return { element: returned, scope: candidate.body as AstNode | undefined };
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

function jsxElementToSpec(node: AstNode, context: ReactLoweringContext): HawkElementSpec {
  const tag = jsxTagName(node);
  const id = requiredString(jsxAttributeValue(node, "id", context), tag, "id");
  const style = optionalString(jsxAttributeValue(node, "className", context));
  const assetPath = optionalString(jsxAttributeValue(node, "data-asset", context));
  if (assetPath && isUnsafeAssetPath(assetPath)) {
    throw new Error("react.asset.path-invalid: React asset references must use workspace-relative paths.");
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
    assetRefs: assetPath ? [{ name: "react.asset", path: assetPath }] : [],
    events: reactEvents(node),
    lifecycle: reactLifecycle(node),
    children: reactChildSpecs(node, context),
  };
  const props = runtimeProps(node, context, id, "react");
  const text = reactTextContent(node, context, id);
  if (text) props.text = text;
  return Object.keys(props).length > 0 ? { ...spec, props } : spec;
}

function reactChildSpecs(node: AstNode, context: ReactLoweringContext): readonly HawkElementSpec[] {
  const children: HawkElementSpec[] = [];
  for (const child of arrayField(node, "children")) {
    if (isHawkJsxElement(child)) {
      children.push(jsxElementToSpec(child, context));
    } else if (child.type === "JSXExpressionContainer") {
      const expression = child.expression as AstNode | undefined;
      if (expression && isMapCall(expression)) {
        children.push(...expandMapCall(expression, context));
      }
    }
  }
  return children;
}

function expandMapCall(expression: AstNode, context: ReactLoweringContext): readonly HawkElementSpec[] {
  const callee = expression.callee as AstNode;
  const source = identifierName(callee.object as AstNode | undefined);
  const itemName = identifierName(((expression.arguments as AstNode[])?.[0]?.params as AstNode[] | undefined)?.[0]);
  const template = mapCallbackBody((expression.arguments as AstNode[])?.[0]);
  if (!source || !itemName || !template) {
    throw new Error("react.map.unsupported: React keyed lists must use `items.map((item) => <hawk-* />)`.");
  }
  const items = context.arrays.get(source);
  if (!items) {
    throw new Error(`react.map.source-unresolved: React map source \`${source}\` must be a literal array.`);
  }
  return items.map((item) =>
    jsxElementToSpec(template, {
      ...context,
      locals: new Map([...context.locals, [itemName, item]]),
    }),
  );
}

function isMapCall(expression: AstNode | undefined): boolean {
  const callee = expression?.callee as AstNode | undefined;
  return expression?.type === "CallExpression"
    && callee?.type === "MemberExpression"
    && identifierName(callee.property as AstNode | undefined) === "map";
}

function mapCallbackBody(callback: AstNode | undefined): AstNode | undefined {
  if (!callback) return undefined;
  if (isHawkJsxElement(callback.body as AstNode | undefined)) return callback.body as AstNode;
  const returned = returnArgument(callback.body as AstNode | undefined);
  return returned && isHawkJsxElement(returned) ? returned : undefined;
}

function reactEvents(node: AstNode): readonly HawkEventSpec[] {
  return hasJsxAttribute(node, "onPointerDown")
    ? [{ kind: "pointer.press", handler: handlerName(jsxRawAttributeValue(node, "onPointerDown")) }]
    : [];
}

function reactLifecycle(node: AstNode): readonly HawkLifecycleSpec[] {
  const lifecycle: HawkLifecycleSpec[] = [];
  const mounted = jsxRawAttributeValue(node, "onMount");
  if (mounted) lifecycle.push({ phase: "mounted", handler: handlerName(mounted) });
  const unmounted = jsxRawAttributeValue(node, "onUnmount");
  if (unmounted) lifecycle.push({ phase: "unmounted", handler: handlerName(unmounted) });
  return lifecycle;
}

function jsxAttributeValue(
  node: AstNode,
  name: string,
  context: ReactLoweringContext,
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

function reactTextContent(
  node: AstNode,
  context: ReactLoweringContext,
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
    if (child.type !== "JSXExpressionContainer" || isMapCall(child.expression as AstNode | undefined)) {
      continue;
    }
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
    dynamicExpressionParts.push(expressionSource(expression, "react"));
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
  context: ReactLoweringContext,
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
  context: ReactLoweringContext,
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
  context: ReactLoweringContext,
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
  context: ReactLoweringContext,
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
  context: ReactLoweringContext,
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

function literalArraysFromProgram(
  program: AstNode,
  componentScope: AstNode | undefined,
): ReadonlyMap<string, readonly LiteralRecord[]> {
  const arrays = new Map<string, readonly LiteralRecord[]>();
  collectLiteralArraysFromBody(arrayField(program, "body"), arrays);
  collectLiteralArraysFromBody(arrayField(componentScope, "body"), arrays);
  return arrays;
}

function initialDynamicValuesFromProgram(
  program: AstNode,
  componentScope: AstNode | undefined,
): ReadonlyMap<string, HawkCompilerInitialDynamicValueWire> {
  const values = new Map<string, HawkCompilerInitialDynamicValueWire>();
  collectInitialDynamicValuesFromBody(arrayField(program, "body"), values);
  collectInitialDynamicValuesFromBody(arrayField(componentScope, "body"), values);
  return values;
}

function collectInitialDynamicValuesFromBody(
  statements: readonly AstNode[],
  values: Map<string, HawkCompilerInitialDynamicValueWire>,
): void {
  for (const statement of statements) {
    if (statement.type !== "VariableDeclaration") continue;
    for (const declaration of arrayField(statement, "declarations")) {
      const name = identifierName(declaration.id as AstNode | undefined);
      const value = literalDynamicValue(declaration.init as AstNode | undefined);
      if (name && value) values.set(name, { name, mode: "value", value });
    }
  }
}

function collectLiteralArraysFromBody(
  statements: readonly AstNode[],
  arrays: Map<string, readonly LiteralRecord[]>,
): void {
  for (const statement of statements) {
    if (statement.type !== "VariableDeclaration") continue;
    for (const declaration of arrayField(statement, "declarations")) {
      const name = identifierName(declaration.id as AstNode | undefined);
      const values = literalObjectArray(declaration.init as AstNode | undefined);
      if (name && values) arrays.set(name, values);
    }
  }
}

function literalObjectArray(node: AstNode | undefined): readonly LiteralRecord[] | undefined {
  if (!node || node.type !== "ArrayExpression") return undefined;
  return arrayField(node, "elements").map((item) => {
    if (item.type !== "ObjectExpression") {
      throw new Error("react.literal-array.unsupported: map sources must be arrays of literal objects.");
    }
    const record: Record<string, string | number | boolean> = {};
    for (const property of arrayField(item, "properties")) {
      const key = identifierName(property.key as AstNode | undefined) ?? literalString(property.key as AstNode | undefined);
      const value = literalValue(property.value as AstNode | undefined);
      if (!key || value === undefined) {
        throw new Error("react.literal-array.unsupported: literal object properties must be scalar values.");
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
  context: ReactLoweringContext,
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
  throw new Error("react.expression.unsupported: compiler artifact expressions must resolve to literal values.");
}

function handlerName(value: AstNode | undefined): string {
  if (value?.type === "StringLiteral") return String(value.value);
  if (value?.type === "JSXExpressionContainer") {
    const expression = value.expression as AstNode | undefined;
    const name = identifierName(expression);
    if (name) return name;
  }
  throw new Error("react.handler.unsupported: event handlers must be stable identifiers.");
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
  throw new Error(`react.attribute.required: ${tag} requires a stable ${attribute} attribute.`);
}

function arrayField(node: AstNode | undefined, field: string): AstNode[] {
  const value = node?.[field];
  return Array.isArray(value) ? (value.filter(Boolean) as AstNode[]) : [];
}

function kindForTag(tag: string): HawkElementSpec["kind"] {
  if (tag === "hawk-view") return "view";
  if (tag === "hawk-text") return "text";
  if (tag === "hawk-button") return "button";
  throw new Error(`react.element.unsupported: unsupported Hawk element \`${tag}\`.`);
}

function isHawkTag(tag: string): boolean {
  return tag.startsWith("hawk-");
}

function isUnsafeAssetPath(path: string): boolean {
  return path.includes("://") || path.startsWith("/") || path.includes("..");
}

export function createHawkReactRoot(target: { readonly id: string }): HawkReactRoot {
  if (!target.id.trim()) {
    throw new Error("Hawk2UI React roots require a stable target id.");
  }
  const records: string[] = [];
  let current: HawkElementSpec | undefined;
  return {
    get records() {
      return records;
    },
    render: (element: unknown) => {
      const next = elementToNativeSpec(element, target.id);
      validateUniqueChildKeys(next);
      if (!current) {
        records.push(...recordsForApp({
        name: `react:${target.id}`,
          root: next,
        }));
      } else {
        records.push(...diffRecords(current, next));
      }
      current = next;
    },
    unmount: () => {
      if (current) {
        records.push(`unmount-element:${current.id}`);
        current = undefined;
      }
    },
  };
}

function elementToNativeSpec(element: unknown, fallbackId: string): HawkElementSpec {
  const props = readProps(element);
  const id = readString(props, "id") ?? fallbackId;
  return {
    id,
    kind: "view",
    refs: readString(props, "ref") ? [readString(props, "ref") as string] : [],
    styleRefs: readString(props, "className") ? [readString(props, "className") as string] : [],
    assetRefs: readString(props, "data-asset")
      ? [{ name: "react.asset", path: readString(props, "data-asset") as string }]
      : [],
    events: props && "onPointerDown" in props ? [{ kind: "pointer.press", handler: "handlePress" }] : [],
    children: readChildren(element, props).map(runtimeChildSpec),
  };
}

function runtimeChildSpec(child: Record<string, unknown>, index: number): HawkElementSpec {
  const props = readProps(child);
  const id = readString(props, "id") ?? readString(child, "id") ?? `child-${index}`;
  const key = readString(child, "key") ?? readString(props, "id") ?? readString(child, "id");
  const text = readTextProp(child);
  return {
    id,
    kind: "text",
    ...(key ? { key } : {}),
    ...(text ? { props: text } : {}),
  };
}

function readProps(element: unknown): Record<string, unknown> | undefined {
  if (!element || typeof element !== "object") return undefined;
  const props = "props" in element ? (element as { readonly props?: unknown }).props : element;
  return props && typeof props === "object" ? (props as Record<string, unknown>) : undefined;
}

function readChildren(
  element: unknown,
  props: Record<string, unknown> | undefined,
): readonly Record<string, unknown>[] {
  const children = props?.children ?? (element && typeof element === "object" && "children" in element
    ? (element as { readonly children?: unknown }).children
    : undefined);
  return Array.isArray(children)
    ? children.filter((child): child is Record<string, unknown> => Boolean(child) && typeof child === "object")
    : [];
}

function readString(record: Record<string, unknown> | undefined, name: string): string | undefined {
  const value = record?.[name];
  return typeof value === "string" && value.trim() ? value : undefined;
}

function readTextProp(element: unknown): Record<string, string> | undefined {
  const text = readString(readProps(element), "text");
  return text ? { text } : undefined;
}

function validateUniqueChildKeys(element: HawkElementSpec): void {
  const keys = new Set<string>();
  for (const child of element.children ?? []) {
    if (child.key) {
      if (keys.has(child.key)) {
        throw new Error(`react.child-key.duplicate: duplicate React child key \`${child.key}\``);
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
    records.push(...recordsForApp({ name: `react:${next.id}`, root: next }));
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
      records.push(...recordsForApp({ name: `react:${child.id}`, root: child }));
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
