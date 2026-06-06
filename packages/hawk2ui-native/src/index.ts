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
}

export interface HawkCompilerArtifact {
  readonly schema_version: 1;
  readonly compiler: HawkCompilerSourceWire;
  readonly root: HawkCompilerNodeWire;
  readonly reactivity: readonly HawkCompilerReactiveBindingWire[];
  readonly dynamic_bindings: readonly HawkCompilerDynamicBindingWire[];
  readonly initial_dynamic_values: readonly HawkCompilerInitialDynamicValueWire[];
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
  if (!element.id.trim()) {
    throw new Error("native.element.id-invalid: native elements require stable ids.");
  }
  const keys = new Set<string>();
  for (const child of element.children ?? []) {
    if (child.key) {
      if (keys.has(child.key)) {
        throw new Error(`native.child-key.duplicate: duplicate native child key \`${child.key}\``);
      }
      keys.add(child.key);
    }
    validateElement(child);
  }
  for (const asset of element.assetRefs ?? []) {
    if (isUnsafeAssetPath(asset.path)) {
      throw new Error(`native.asset.path-invalid: asset \`${asset.name}\` must use a workspace-relative safe path`);
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

function validateDynamicValue(value: HawkCompilerDynamicValueWire, name: string): void {
  switch (value.type) {
    case "null":
    case "bool":
    case "string":
      return;
    case "number":
      if (!Number.isFinite(value.value)) {
        throw new Error(`native.initial-dynamic-value.number-invalid: initial dynamic value \`${name}\` must be finite.`);
      }
      return;
    case "array":
      for (const item of value.value) validateDynamicValue(item, name);
      return;
    case "object":
      for (const [key, item] of Object.entries(value.value)) {
        if (!key.trim()) {
          throw new Error(`native.initial-dynamic-value.object-key-invalid: initial dynamic value \`${name}\` has an empty object key.`);
        }
        validateDynamicValue(item, name);
      }
      return;
  }
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

function isUnsafeAssetPath(path: string): boolean {
  return path.includes("://") || path.startsWith("/") || path.includes("..");
}
