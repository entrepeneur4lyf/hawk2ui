export type HawkElementKind = "view" | "text" | "button" | "custom-surface";

export interface HawkElementSpec {
  readonly id: string;
  readonly kind: HawkElementKind;
  readonly key?: string;
  readonly props?: Record<string, string | number | boolean>;
  readonly events?: readonly HawkEventSpec[];
  readonly lifecycle?: readonly HawkLifecycleSpec[];
  readonly styleRefs?: readonly string[];
  readonly assetRefs?: readonly { name: string; path: string }[];
  readonly refs?: readonly string[];
  readonly children?: readonly HawkElementSpec[];
}

export interface HawkAppSpec {
  readonly name: string;
  readonly root: HawkElementSpec;
}

export interface HawkEventSpec {
  readonly kind: "pointer.press";
  readonly handler: string;
}

export interface HawkLifecycleSpec {
  readonly phase: "mounted" | "unmounted";
  readonly handler: string;
}

export type HawkCompilerPropValueWire =
  | { readonly type: "string"; readonly value: string }
  | { readonly type: "bool"; readonly value: boolean }
  | { readonly type: "number"; readonly value: number };

export interface HawkCompilerPropWire {
  readonly name: string;
  readonly value: HawkCompilerPropValueWire;
}

export interface HawkCompilerAssetWire {
  readonly name: string;
  readonly path: string;
}

export interface HawkCompilerEventWire {
  readonly kind: "pointer.press";
  readonly handler: string;
  readonly payload_fields: readonly ("position" | "delta" | "value" | "key")[];
}

export interface HawkCompilerLifecycleWire {
  readonly event: "mounted" | "unmounted";
  readonly handler: string;
}

export interface HawkCompilerChildWire {
  readonly key?: string;
  readonly node: HawkCompilerNodeWire;
}

export interface HawkCompilerNodeWire {
  readonly id: string;
  readonly kind: HawkElementKind;
  readonly key?: string;
  readonly props: readonly HawkCompilerPropWire[];
  readonly refs: readonly string[];
  readonly style_refs: readonly string[];
  readonly asset_refs: readonly HawkCompilerAssetWire[];
  readonly events: readonly HawkCompilerEventWire[];
  readonly lifecycle: readonly HawkCompilerLifecycleWire[];
  readonly children: readonly HawkCompilerChildWire[];
}

export interface HawkCompilerReactiveBindingWire {
  readonly kind: "signal" | "keyed-for-each" | "effect";
  readonly name: string;
}

export type HawkCompilerDynamicBindingTargetWire =
  | { readonly type: "prop"; readonly name: string }
  | { readonly type: "text" };

export interface HawkCompilerDynamicBindingWire {
  readonly node_id: string;
  readonly target: HawkCompilerDynamicBindingTargetWire;
  readonly expression: string;
  readonly dependencies: readonly string[];
}

export type HawkCompilerDynamicValueWire =
  | { readonly type: "null" }
  | { readonly type: "bool"; readonly value: boolean }
  | { readonly type: "number"; readonly value: number }
  | { readonly type: "string"; readonly value: string }
  | { readonly type: "array"; readonly value: readonly HawkCompilerDynamicValueWire[] }
  | { readonly type: "object"; readonly value: Readonly<Record<string, HawkCompilerDynamicValueWire>> };

export type HawkCompilerEventHandlerActionWire =
  | {
    readonly type: "set_dynamic_value";
    readonly name: string;
    readonly value: HawkCompilerDynamicValueWire;
  }
  | {
    readonly type: "set_dynamic_expression";
    readonly name: string;
    readonly expression: string;
    readonly dependencies: readonly string[];
  };

export interface HawkCompilerEventHandlerWire {
  readonly name: string;
  readonly actions: readonly HawkCompilerEventHandlerActionWire[];
}

export interface HawkCompilerInitialDynamicValueWire {
  readonly name: string;
  readonly mode: "value" | "getter";
  readonly value: HawkCompilerDynamicValueWire;
}

export interface HawkCompilerSourceWire {
  readonly framework: string;
  readonly compiler: string;
  readonly source_path: string;
  readonly entrypoint: string;
}

export interface HawkCompilerArtifactOptions {
  readonly compiler?: HawkCompilerSourceWire;
  readonly eventHandlers?: readonly HawkCompilerEventHandlerWire[];
}

export interface HawkCompilerArtifact {
  readonly schema_version: 1;
  readonly compiler: HawkCompilerSourceWire;
  readonly root: HawkCompilerNodeWire;
  readonly reactivity: readonly HawkCompilerReactiveBindingWire[];
  readonly dynamic_bindings: readonly HawkCompilerDynamicBindingWire[];
  readonly initial_dynamic_values: readonly HawkCompilerInitialDynamicValueWire[];
  readonly event_handlers: readonly HawkCompilerEventHandlerWire[];
}

export interface HawkCompiledApp extends HawkAppSpec {
  readonly records: readonly string[];
  readonly compilerArtifact: HawkCompilerArtifact;
}

export function createHawkApp(spec: HawkAppSpec): HawkCompiledApp {
  if (!spec.name.trim()) {
    throw new Error("Hawk2UI native app requires a stable name.");
  }
  validateElement(spec.root);
  return {
    ...spec,
    records: recordsForApp(spec),
    compilerArtifact: compilerArtifactForApp(spec),
  };
}

export function compilerArtifactForApp(
  spec: HawkAppSpec,
  reactivity: readonly HawkCompilerReactiveBindingWire[] = [],
  dynamicBindings: readonly HawkCompilerDynamicBindingWire[] = [],
  initialDynamicValues: readonly HawkCompilerInitialDynamicValueWire[] = [],
  options: HawkCompilerArtifactOptions = {},
): HawkCompilerArtifact {
  if (!spec.name.trim()) {
    throw new Error("Hawk2UI native app requires a stable name.");
  }
  validateElement(spec.root);
  validateDynamicBindings(dynamicBindings, elementIds(spec.root));
  validateInitialDynamicValues(initialDynamicValues);
  validateEventHandlers(options.eventHandlers ?? [], referencedHandlerNames(spec.root));
  const compiler = cloneCompilerSourceWire(options.compiler ?? nativeCompilerSourceForApp(spec));
  validateCompilerSource(compiler);
  return {
    schema_version: 1,
    compiler,
    root: elementToWire(spec.root),
    reactivity: reactivity.map((binding) => ({ ...binding })),
    dynamic_bindings: dynamicBindings.map((binding) => ({
      node_id: binding.node_id,
      target: { ...binding.target },
      expression: binding.expression,
      dependencies: [...binding.dependencies],
      })),
      initial_dynamic_values: initialDynamicValues.map((value) => ({
        name: value.name,
        mode: value.mode,
        value: cloneDynamicValueWire(value.value),
      })),
      event_handlers: (options.eventHandlers ?? []).map(cloneEventHandlerWire),
    };
  }

export function recordsForApp(spec: HawkAppSpec): readonly string[] {
  const records: string[] = [];
  emitLifecycle(spec.root, "mounted", records);
  emitElement(spec.root, records);
  emitLifecycle(spec.root, "unmounted", records);
  return records;
}

function nativeCompilerSourceForApp(spec: HawkAppSpec): HawkCompilerSourceWire {
  return {
    framework: "native",
    compiler: "@hawk2ui/native",
    source_path: spec.name,
    entrypoint: "root",
  };
}

function cloneCompilerSourceWire(source: HawkCompilerSourceWire): HawkCompilerSourceWire {
  return {
    framework: source.framework,
    compiler: source.compiler,
    source_path: source.source_path,
    entrypoint: source.entrypoint,
  };
}

function validateCompilerSource(source: HawkCompilerSourceWire): void {
  if (!source.framework.trim()) {
    throw new Error("native.compiler.framework-invalid: compiler metadata requires a framework name.");
  }
  if (!source.compiler.trim()) {
    throw new Error("native.compiler.compiler-invalid: compiler metadata requires a compiler package name.");
  }
  if (!source.source_path.trim() || isUnsafeAssetPath(source.source_path)) {
    throw new Error("native.compiler.source-path-invalid: compiler metadata source path must be workspace-relative.");
  }
  if (!source.entrypoint.trim()) {
    throw new Error("native.compiler.entrypoint-invalid: compiler metadata requires an entrypoint.");
  }
}

function emitElement(element: HawkElementSpec, records: string[]): void {
  records.push(`mount-element:${element.id}`);
  for (const reference of element.refs ?? []) {
    records.push(`ref:${element.id}:${reference}`);
  }
  for (const style of element.styleRefs ?? []) {
    records.push(`style:${element.id}:${style}`);
  }
  for (const asset of element.assetRefs ?? []) {
    records.push(`asset:${element.id}:${asset.path}`);
  }
  for (const event of element.events ?? []) {
    records.push(`bind-event:${element.id}:${event.kind}`);
  }
  for (const [name, value] of Object.entries(element.props ?? {})) {
    records.push(`prop:${element.id}:${name}=${String(value)}`);
  }
  for (const child of element.children ?? []) {
    emitElement(child, records);
  }
}

function emitLifecycle(
  element: HawkElementSpec,
  phase: HawkLifecycleSpec["phase"],
  records: string[],
): void {
  for (const lifecycle of element.lifecycle ?? []) {
    if (lifecycle.phase === phase) {
      records.push(`lifecycle:${phase}:${element.id}:${lifecycle.handler}`);
    }
  }
  for (const child of element.children ?? []) {
    emitLifecycle(child, phase, records);
  }
}

function elementToWire(element: HawkElementSpec): HawkCompilerNodeWire {
  return {
    id: element.id,
    kind: element.kind,
    ...(element.key ? { key: element.key } : {}),
    props: Object.entries(element.props ?? {}).map(([name, value]) => ({
      name,
      value: propValueToWire(value, element.id, name),
    })),
    refs: [...(element.refs ?? [])],
    style_refs: [...(element.styleRefs ?? [])],
    asset_refs: (element.assetRefs ?? []).map((asset) => ({ ...asset })),
    events: (element.events ?? []).map((event) => ({
      kind: event.kind,
      handler: event.handler,
      payload_fields: ["position"],
    })),
    lifecycle: (element.lifecycle ?? []).map((lifecycle) => ({
      event: lifecycle.phase,
      handler: lifecycle.handler,
    })),
    children: (element.children ?? []).map((child) => ({
      ...(child.key ? { key: child.key } : {}),
      node: elementToWire(child),
    })),
  };
}

function propValueToWire(
  value: string | number | boolean,
  elementId: string,
  name: string,
): HawkCompilerPropValueWire {
  if (typeof value === "string") {
    return { type: "string", value };
  }
  if (typeof value === "boolean") {
    return { type: "bool", value };
  }
  if (!Number.isFinite(value)) {
    throw new Error(`native.prop.number-invalid: property \`${name}\` on \`${elementId}\` must be finite.`);
  }
  return { type: "number", value };
}

function validateElement(element: HawkElementSpec): void {
  const record = element as unknown;
  if (!isObjectRecord(record)) {
    throw new Error("native.element.record-invalid: native elements must be object records.");
  }
  if (typeof record.id !== "string" || !record.id.trim()) {
    throw new Error("native.element.id-invalid: native elements require stable ids.");
  }
  if (!isElementKind(record.kind)) {
    throw new Error(`native.element.kind-invalid: native element \`${record.id}\` has unsupported kind \`${String(record.kind)}\`.`);
  }
  validateStringArray(record.refs, "native.refs.invalid", `refs on \`${record.id}\` must be an array of stable strings.`);
  validateStringArray(record.styleRefs, "native.style-refs.invalid", `style refs on \`${record.id}\` must be an array of stable strings.`);
  validateProps(record.props, record.id);

  for (const event of validateRecordArray(record.events, "native.events.invalid", `events on \`${record.id}\` must be an array of records.`)) {
    if (event.kind !== "pointer.press") {
      throw new Error(`native.event.kind-invalid: event on \`${record.id}\` has unsupported kind \`${String(event.kind)}\`.`);
    }
    if (typeof event.handler !== "string" || !event.handler.trim()) {
      throw new Error(`native.event.handler-invalid: event on \`${record.id}\` requires a stable handler name.`);
    }
  }

  for (const lifecycle of validateRecordArray(record.lifecycle, "native.lifecycle.invalid", `lifecycle on \`${record.id}\` must be an array of records.`)) {
    if (lifecycle.phase !== "mounted" && lifecycle.phase !== "unmounted") {
      throw new Error(`native.lifecycle.phase-invalid: lifecycle on \`${record.id}\` has unsupported phase \`${String(lifecycle.phase)}\`.`);
    }
    if (typeof lifecycle.handler !== "string" || !lifecycle.handler.trim()) {
      throw new Error(`native.lifecycle.handler-invalid: lifecycle on \`${record.id}\` requires a stable handler name.`);
    }
  }

  const keys = new Set<string>();
  for (const child of validateRecordArray(record.children, "native.children.invalid", `children on \`${record.id}\` must be an array of records.`)) {
    if (typeof child.key === "string" && child.key) {
      if (keys.has(child.key)) {
        throw new Error(`native.child-key.duplicate: duplicate native child key \`${child.key}\``);
      }
      keys.add(child.key);
    }
    validateElement(child as unknown as HawkElementSpec);
  }
  for (const asset of validateRecordArray(record.assetRefs, "native.assets.invalid", `asset refs on \`${record.id}\` must be an array of records.`)) {
    if (typeof asset.name !== "string" || !asset.name.trim()) {
      throw new Error(`native.asset.name-invalid: asset ref on \`${record.id}\` requires a stable name.`);
    }
    if (typeof asset.path !== "string" || !asset.path.trim() || isUnsafeAssetPath(asset.path)) {
      throw new Error(`native.asset.path-invalid: asset \`${String(asset.name)}\` must use a workspace-relative safe path`);
    }
  }
}

function isElementKind(value: unknown): value is HawkElementKind {
  return value === "view" || value === "text" || value === "button" || value === "custom-surface";
}

function validateStringArray(value: unknown, rule: string, message: string): void {
  if (value === undefined) return;
  if (!Array.isArray(value)) {
    throw new Error(`${rule}: ${message}`);
  }
  for (const item of value) {
    if (typeof item !== "string" || !item.trim()) {
      throw new Error(`${rule}: ${message}`);
    }
  }
}

function validateRecordArray(
  value: unknown,
  rule: string,
  message: string,
): readonly Readonly<Record<string, unknown>>[] {
  if (value === undefined) return [];
  if (!Array.isArray(value)) {
    throw new Error(`${rule}: ${message}`);
  }
  for (const item of value) {
    if (!isObjectRecord(item)) {
      throw new Error(`${rule}: ${message}`);
    }
  }
  return value;
}

function validateProps(value: unknown, elementId: string): void {
  if (value === undefined) return;
  if (!isObjectRecord(value)) {
    throw new Error(`native.props.invalid: props on \`${elementId}\` must be a record.`);
  }
  for (const [name, prop] of Object.entries(value)) {
    if (!name.trim()) {
      throw new Error(`native.prop.name-invalid: property names on \`${elementId}\` must be stable.`);
    }
    const valid = typeof prop === "string" || typeof prop === "boolean" || (typeof prop === "number" && Number.isFinite(prop));
    if (!valid) {
      throw new Error(`native.prop.value-invalid: property \`${name}\` on \`${elementId}\` must be a string, boolean, or finite number.`);
    }
  }
}

function validateDynamicBindings(
  bindings: readonly HawkCompilerDynamicBindingWire[],
  ids: ReadonlySet<string>,
): void {
  for (const binding of bindings) {
    if (!binding.node_id.trim()) {
      throw new Error("native.dynamic-binding.node-id-invalid: dynamic bindings require stable node ids.");
    }
    if (!ids.has(binding.node_id)) {
      throw new Error(`native.dynamic-binding.node-missing: dynamic binding references unknown node \`${binding.node_id}\`.`);
    }
    if (!binding.expression.trim()) {
      throw new Error("native.dynamic-binding.expression-invalid: dynamic bindings require non-empty expressions.");
    }
    if (binding.target.type === "prop" && !binding.target.name.trim()) {
      throw new Error("native.dynamic-binding.target-invalid: prop bindings require a property name.");
    }
    for (const dependency of binding.dependencies) {
      if (!dependency.trim()) {
        throw new Error("native.dynamic-binding.dependency-invalid: dynamic binding dependencies must be non-empty.");
      }
    }
  }
}

function validateInitialDynamicValues(values: readonly HawkCompilerInitialDynamicValueWire[]): void {
  const names = new Set<string>();
  for (const value of values) {
    if (!value.name.trim()) {
      throw new Error("native.initial-dynamic-value.name-invalid: initial dynamic values require stable names.");
    }
    if (names.has(value.name)) {
      throw new Error(`native.initial-dynamic-value.duplicate: initial dynamic value \`${value.name}\` is declared more than once.`);
    }
    names.add(value.name);
    if (value.mode !== "value" && value.mode !== "getter") {
      throw new Error(`native.initial-dynamic-value.mode-invalid: initial dynamic value \`${value.name}\` has an unsupported mode.`);
    }
    validateDynamicValue(value.value, value.name);
  }
}

function validateEventHandlers(value: unknown, referencedHandlers: ReadonlySet<string>): void {
  const handlers = validateRecordArray(
    value,
    "native.event-handlers.invalid",
    "event handler artifacts must be an array of records.",
  );
  const names = new Set<string>();
  for (const handler of handlers) {
    if (typeof handler.name !== "string" || !handler.name.trim()) {
      throw new Error("native.event-handler.name-invalid: event handler artifacts require stable names.");
    }
    if (names.has(handler.name)) {
      throw new Error(`native.event-handler.duplicate: event handler \`${handler.name}\` is declared more than once.`);
    }
    names.add(handler.name);
    if (!referencedHandlers.has(handler.name)) {
      throw new Error(`native.event-handler.unreferenced: event handler \`${handler.name}\` is not referenced by the element tree.`);
    }
    const actions = validateRecordArray(
      handler.actions,
      "native.event-handler.actions-invalid",
      `event handler \`${handler.name}\` requires an array of action records.`,
    );
    if (actions.length === 0) {
      throw new Error(`native.event-handler.actions-empty: event handler \`${handler.name}\` requires at least one executable action.`);
    }
    for (const action of actions) {
      if (typeof action.name !== "string" || !action.name.trim()) {
        throw new Error(`native.event-handler.action-name-invalid: event handler \`${handler.name}\` action requires a stable dynamic value name.`);
      }
      if (action.type === "set_dynamic_value") {
        validateDynamicValue(action.value, action.name);
      } else if (action.type === "set_dynamic_expression") {
        if (typeof action.expression !== "string" || !action.expression.trim()) {
          throw new Error(`native.event-handler.expression-invalid: event handler \`${handler.name}\` expression actions require a non-empty expression.`);
        }
        validateStringArray(
          action.dependencies,
          "native.event-handler.dependencies-invalid",
          `event handler \`${handler.name}\` expression dependencies must be an array of stable names.`,
        );
      } else {
        throw new Error(`native.event-handler.action-type-invalid: event handler \`${handler.name}\` has unsupported action type \`${String(action.type)}\`.`);
      }
    }
  }
}

function validateDynamicValue(value: unknown, name: string): asserts value is HawkCompilerDynamicValueWire {
  if (!isObjectRecord(value) || typeof value.type !== "string") {
    throw new Error(`native.initial-dynamic-value.type-invalid: initial dynamic value \`${name}\` has an unsupported dynamic value record.`);
  }
  switch (value.type) {
    case "null":
      return;
    case "bool":
      if (typeof value.value !== "boolean") {
        throw new Error(`native.initial-dynamic-value.bool-invalid: initial dynamic value \`${name}\` bool payload must be boolean.`);
      }
      return;
    case "string":
      if (typeof value.value !== "string") {
        throw new Error(`native.initial-dynamic-value.string-invalid: initial dynamic value \`${name}\` string payload must be text.`);
      }
      return;
    case "number":
      if (typeof value.value !== "number" || !Number.isFinite(value.value)) {
        throw new Error(`native.initial-dynamic-value.number-invalid: initial dynamic value \`${name}\` must be finite.`);
      }
      return;
    case "array":
      if (!Array.isArray(value.value)) {
        throw new Error(`native.initial-dynamic-value.array-invalid: initial dynamic value \`${name}\` array payload must be an array.`);
      }
      for (const item of value.value) validateDynamicValue(item, name);
      return;
    case "object":
      if (!isObjectRecord(value.value)) {
        throw new Error(`native.initial-dynamic-value.object-invalid: initial dynamic value \`${name}\` object payload must be a record.`);
      }
      for (const [key, item] of Object.entries(value.value)) {
        if (!key.trim()) {
          throw new Error(`native.initial-dynamic-value.object-key-invalid: initial dynamic value \`${name}\` has an empty object key.`);
        }
        validateDynamicValue(item, name);
      }
      return;
    default:
      throw new Error(`native.initial-dynamic-value.type-invalid: initial dynamic value \`${name}\` has unsupported type \`${value.type}\`.`);
  }
}

function isObjectRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function cloneDynamicValueWire(value: HawkCompilerDynamicValueWire): HawkCompilerDynamicValueWire {
  switch (value.type) {
    case "null":
      return { type: "null" };
    case "bool":
      return { type: "bool", value: value.value };
    case "number":
      return { type: "number", value: value.value };
    case "string":
      return { type: "string", value: value.value };
    case "array":
      return { type: "array", value: value.value.map(cloneDynamicValueWire) };
    case "object": {
      const cloned: Record<string, HawkCompilerDynamicValueWire> = {};
      for (const [key, item] of Object.entries(value.value)) {
        cloned[key] = cloneDynamicValueWire(item);
      }
      return { type: "object", value: cloned };
    }
  }
}

function cloneEventHandlerWire(handler: HawkCompilerEventHandlerWire): HawkCompilerEventHandlerWire {
  return {
    name: handler.name,
    actions: handler.actions.map((action) => {
      if (action.type === "set_dynamic_value") {
        return {
          type: "set_dynamic_value",
          name: action.name,
          value: cloneDynamicValueWire(action.value),
        };
      }
      return {
        type: "set_dynamic_expression",
        name: action.name,
        expression: action.expression,
        dependencies: [...action.dependencies],
      };
    }),
  };
}

function elementIds(root: HawkElementSpec): ReadonlySet<string> {
  const ids = new Set<string>();
  const visit = (element: HawkElementSpec): void => {
    ids.add(element.id);
    for (const child of element.children ?? []) {
      visit(child);
    }
  };
  visit(root);
  return ids;
}

function referencedHandlerNames(root: HawkElementSpec): ReadonlySet<string> {
  const names = new Set<string>();
  const visit = (element: HawkElementSpec): void => {
    for (const event of element.events ?? []) {
      names.add(event.handler);
    }
    for (const lifecycle of element.lifecycle ?? []) {
      names.add(lifecycle.handler);
    }
    for (const child of element.children ?? []) {
      visit(child);
    }
  };
  visit(root);
  return names;
}

function isUnsafeAssetPath(path: string): boolean {
  return path.includes("://") || path.startsWith("/") || path.includes("..");
}
