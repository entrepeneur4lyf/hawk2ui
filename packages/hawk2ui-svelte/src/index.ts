import { compile as compileSvelte, parse as parseSvelte } from "svelte/compiler";
import {
  compilerArtifactForApp,
  recordsForApp,
  type HawkCompilerArtifact,
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
  readonly arrays: ReadonlyMap<string, readonly LiteralRecord[]>;
  readonly locals: ReadonlyMap<string, LiteralRecord>;
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
    arrays: literalArraysFromProgram((ast.instance as AstNode | undefined)?.content as AstNode | undefined),
    locals: new Map(),
  };
  const root = svelteElementToSpec(rootNode, context);
  validateUniqueChildIds(root.children ?? []);

  const app = { name: input.filename, root };
  return {
    framework: "svelte",
    filename: input.filename,
    records: recordsForApp(app),
    compilerArtifact: compilerArtifactForApp(app),
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

  const text = textContent(node, context);
  return text ? { ...spec, props: { text } } : spec;
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

function textContent(node: AstNode, context: SvelteLoweringContext): string | undefined {
  const values = childrenOf(node)
    .map((child) => {
      if (child.type === "Text") return stringField(child, "data").trim();
      if (child.type === "MustacheTag") return String(evaluateExpression(child.expression as AstNode | undefined, context));
      return "";
    })
    .filter((value) => value.length > 0);
  return values.length > 0 ? values.join("") : undefined;
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
