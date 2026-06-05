import { compile as compileSvelte, parse as parseSvelte } from "svelte/compiler";
import {
  compilerArtifactForApp,
  recordsForApp,
  type HawkCompilerArtifact,
  type HawkCompilerDynamicBindingWire,
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
  readonly locals: ReadonlyMap<string, LiteralRecord>;
  readonly dynamicBindings: HawkCompilerDynamicBindingWire[];
}

export function compileHawkSvelte(input: HawkSvelteCompileInput): HawkSvelteCompileOutput {
  if (!input.filename.endsWith(".svelte")) {
    throw new Error("Hawk2UI Svelte inputs must be .svelte files.");
  }

  const ast = parseSvelte(input.source, { filename: input.filename }) as AstNode;
  compileSvelte(input.source, { filename: input.filename, generate: false });

  const rootNode = firstHawkElement(childrenOf(ast.html as AstNode | undefined));
  if (!rootNode) {
    throw new Error("svelte.root.missing: Svelte compiler output must contain one hawk root element.");
  }

  const context: SvelteLoweringContext = {
    source: input.source,
    arrays: literalArraysFromProgram((ast.instance as AstNode | undefined)?.content as AstNode | undefined),
    locals: new Map(),
    dynamicBindings: [],
  };
  const root = svelteElementToSpec(rootNode, context);
  validateUniqueChildIds(root.children ?? []);

  const app = { name: input.filename, root };
  return {
    framework: "svelte",
    filename: input.filename,
    records: recordsForApp(app),
    compilerArtifact: compilerArtifactForApp(app, [], context.dynamicBindings),
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
    children: childSpecs(node, context),
  };

  const props = layoutProps(node, context, id, "svelte");
  const text = textContent(node, context, id);
  if (text) props.text = text;
  return Object.keys(props).length > 0 ? { ...spec, props } : spec;
}

function childSpecs(node: AstNode, context: SvelteLoweringContext): readonly HawkElementSpec[] {
  const children: HawkElementSpec[] = [];
  for (const child of childrenOf(node)) {
    if (child.type === "Element" && isHawkTag(stringField(child, "name"))) {
      children.push(svelteElementToSpec(child, context));
    } else if (child.type === "EachBlock") {
      children.push(...expandEachBlock(child, context));
    }
  }
  return children;
}

function expandEachBlock(block: AstNode, context: SvelteLoweringContext): readonly HawkElementSpec[] {
  const source = identifierName(block.expression as AstNode | undefined);
  const itemName = identifierName(block.context as AstNode | undefined);
  if (!source || !itemName) {
    throw new Error("svelte.each.unsupported: keyed each blocks must use `items as item` style bindings.");
  }
  const items = context.arrays.get(source);
  if (!items) {
    throw new Error(`svelte.each.source-unresolved: Svelte each source \`${source}\` must be a literal array.`);
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

function eventHandlers(node: AstNode): {
  readonly events: readonly HawkEventSpec[];
  readonly lifecycle: readonly HawkLifecycleSpec[];
} {
  const events: HawkEventSpec[] = [];
  const lifecycle: HawkLifecycleSpec[] = [];
  for (const attribute of attributesOf(node)) {
    if (attribute.type !== "EventHandler") continue;
    const name = stringField(attribute, "name");
    const handler = handlerName(attribute.expression as AstNode | undefined);
    if (name === "press") {
      events.push({ kind: "pointer.press", handler });
    } else if (name === "mount") {
      lifecycle.push({ phase: "mounted", handler });
    } else if (name === "destroy") {
      lifecycle.push({ phase: "unmounted", handler });
    } else {
      throw new Error(`svelte.event.unsupported: Svelte event \`${name}\` is not part of the native event contract.`);
    }
  }
  return { events, lifecycle };
}

function actionRefs(node: AstNode): readonly string[] {
  return attributesOf(node)
    .filter((attribute) => attribute.type === "Action")
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
  const value = attribute.value;
  if (value === true) return true;
  if (!Array.isArray(value) || value.length === 0) return "";
  const first = value[0] as AstNode;
  if (first.type === "Text") return stringField(first, "data");
  if (first.type === "MustacheTag") return evaluateExpression(first.expression as AstNode | undefined, context);
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
    if (child.type !== "MustacheTag") continue;
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

function dynamicLayoutAttributeValue(
  node: AstNode,
  name: string,
  context: SvelteLoweringContext,
  nodeId: string,
  framework: string,
): number | undefined {
  const attribute = attributesOf(node).find((item) => item.type === "Attribute" && item.name === name);
  if (!attribute) return undefined;
  const value = attribute.value;
  if (!Array.isArray(value) || value.length === 0) {
    throw new Error(`${framework}.attribute.unsupported: layout prop \`${name}\` must be numeric.`);
  }
  const first = value[0] as AstNode;
  if (first.type === "Text") return layoutNumber(stringField(first, "data"), nodeId, name, framework);
  if (first.type !== "MustacheTag") {
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
    for (const declaration of Array.isArray(statement.declarations) ? (statement.declarations as AstNode[]) : []) {
      const name = identifierName(declaration.id as AstNode | undefined);
      const values = literalObjectArray(declaration.init as AstNode | undefined);
      if (name && values) arrays.set(name, values);
    }
  }
  return arrays;
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

function evaluateExpression(
  expression: AstNode | undefined,
  context: SvelteLoweringContext,
): string | number | boolean {
  const literal = literalValue(expression);
  if (literal !== undefined) return literal;
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
  return nodes.find((node) => node.type === "Element" && isHawkTag(stringField(node, "name")));
}

function childrenOf(node: AstNode | undefined): readonly AstNode[] {
  return Array.isArray(node?.children) ? (node.children as AstNode[]) : [];
}

function attributesOf(node: AstNode): readonly AstNode[] {
  return Array.isArray(node.attributes) ? (node.attributes as AstNode[]) : [];
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
  throw new Error(`svelte.element.unsupported: unsupported Hawk element \`${tag}\`.`);
}

function isHawkTag(tag: string): boolean {
  return tag.startsWith("hawk-");
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
