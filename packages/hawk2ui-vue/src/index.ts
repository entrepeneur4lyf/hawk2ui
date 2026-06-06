import { parse as parseScript, parseExpression } from "@babel/parser";
import { parse as parseTemplate } from "@vue/compiler-dom";
import { compileTemplate, parse as parseSfc } from "@vue/compiler-sfc";
import {
  compilerArtifactForApp,
  recordsForApp,
  type HawkCompilerArtifact,
  type HawkCompilerDynamicBindingWire,
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

export interface HawkVueRenderer {
  readonly records: readonly string[];
  readonly render: (component: unknown, target: { readonly id: string }) => void;
  readonly unmount: (target: { readonly id: string }) => void;
}

type AstNode = Record<string, unknown>;
type LiteralRecord = Readonly<Record<string, string | number | boolean>>;

interface VueLoweringContext {
  readonly arrays: ReadonlyMap<string, readonly LiteralRecord[]>;
  readonly locals: ReadonlyMap<string, LiteralRecord>;
  readonly dynamicBindings: HawkCompilerDynamicBindingWire[];
}

const VISUAL_PROP_NAMES = ["font_size", "color", "background"] as const;

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
    locals: new Map(),
    dynamicBindings: [],
  };
  const root = vueElementToSpec(rootNode, context);
  validateUniqueChildKeys(root);
  const app = { name: input.filename, root };
  return {
    framework: "vue",
    filename: input.filename,
    records: recordsForApp(app),
    compilerArtifact: compilerArtifactForApp(app, [], context.dynamicBindings),
  };
}

function vueElementToSpec(node: AstNode, context: VueLoweringContext): HawkElementSpec {
  const tag = stringField(node, "tag");
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
    events: vueEvents(node),
    lifecycle: vueLifecycle(node),
    children: vueChildSpecs(node, context),
  };
  const props = runtimeProps(node, context, id, "vue");
  const text = vueTextContent(node, context, id);
  if (text) props.text = text;
  return Object.keys(props).length > 0 ? { ...spec, props } : spec;
}

function vueChildSpecs(node: AstNode, context: VueLoweringContext): readonly HawkElementSpec[] {
  const children: HawkElementSpec[] = [];
  for (const child of arrayField(node, "children")) {
    if (!isVueHawkElement(child)) continue;
    const forDirective = vueDirective(child, "for");
    if (forDirective) {
      children.push(...expandVueFor(child, forDirective, context));
    } else {
      children.push(vueElementToSpec(child, context));
    }
  }
  return children;
}

function expandVueFor(
  template: AstNode,
  directive: AstNode,
  context: VueLoweringContext,
): readonly HawkElementSpec[] {
  const parseResult = directive.forParseResult as AstNode | undefined;
  const source = stringField(parseResult?.source as AstNode | undefined, "content");
  const itemName = stringField(parseResult?.value as AstNode | undefined, "content");
  if (!source || !itemName) {
    throw new Error("vue.for.unsupported: Vue v-for must use `item in items` style bindings.");
  }
  const items = context.arrays.get(source);
  if (!items) {
    throw new Error(`vue.for.source-unresolved: Vue v-for source \`${source}\` must be a literal array.`);
  }
  return items.map((item) =>
    vueElementToSpec(template, {
      ...context,
      locals: new Map([...context.locals, [itemName, item]]),
    }),
  );
}

function vueEvents(node: AstNode): readonly HawkEventSpec[] {
  const events: HawkEventSpec[] = [];
  for (const directive of vueDirectives(node, "on")) {
    const event = stringField(directive.arg as AstNode | undefined, "content");
    if (event === "pointerdown") {
      events.push({ kind: "pointer.press", handler: vueHandlerName(directive) });
    } else if (event !== "mounted" && event !== "unmounted") {
      throw new Error(`vue.event.unsupported: Vue event \`${event}\` is not part of the native event contract.`);
    }
  }
  return events;
}

function vueLifecycle(node: AstNode): readonly HawkLifecycleSpec[] {
  const lifecycle: HawkLifecycleSpec[] = [];
  for (const directive of vueDirectives(node, "on")) {
    const event = stringField(directive.arg as AstNode | undefined, "content");
    if (event === "mounted") lifecycle.push({ phase: "mounted", handler: vueHandlerName(directive) });
    if (event === "unmounted") lifecycle.push({ phase: "unmounted", handler: vueHandlerName(directive) });
  }
  return lifecycle;
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
  return props;
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

function evaluateVueExpression(
  expression: string | undefined,
  context: VueLoweringContext,
): string | number | boolean {
  if (!expression) {
    throw new Error("vue.expression.unsupported: empty expressions cannot be lowered into compiler artifacts.");
  }
  const literal = literalExpressionValue(expression);
  if (literal !== undefined) return literal;
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

function isVueHawkElement(node: AstNode): boolean {
  return node.type === 1 && isHawkTag(stringField(node, "tag"));
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
  throw new Error(`vue.element.unsupported: unsupported Hawk element \`${tag}\`.`);
}

function isHawkTag(tag: string): boolean {
  return tag.startsWith("hawk-");
}

function isUnsafeAssetPath(path: string): boolean {
  return path.includes("://") || path.startsWith("/") || path.includes("..");
}

export function createHawkVueRenderer(): HawkVueRenderer {
  const records: string[] = [];
  const roots = new Map<string, HawkElementSpec>();
  return {
    get records() {
      return records;
    },
    render: (component: unknown, target: { readonly id: string }) => {
      if (!target.id.trim()) {
        throw new Error("Hawk2UI Vue render targets require a stable id.");
      }
      const next = componentToNativeSpec(component, target.id);
      validateUniqueChildKeys(next);
      const previous = roots.get(target.id);
      if (!previous) {
        records.push(...recordsForApp({
          name: `vue:${target.id}`,
          root: next,
        }));
      } else {
        records.push(...diffRecords(previous, next));
      }
      roots.set(target.id, next);
    },
    unmount: (target: { readonly id: string }) => {
      if (!target.id.trim()) {
        throw new Error("Hawk2UI Vue render targets require a stable id.");
      }
      const root = roots.get(target.id);
      if (root) {
        records.push(`unmount-element:${root.id}`);
        roots.delete(target.id);
      }
    },
  };
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
    assetRefs: asset ? [{ name: "vue.asset", path: asset }] : [],
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
        throw new Error(`vue.child-key.duplicate: duplicate Vue child key \`${child.key}\``);
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
    records.push(...recordsForApp({ name: `vue:${next.id}`, root: next }));
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
      records.push(...recordsForApp({ name: `vue:${child.id}`, root: child }));
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
