export type HawkElementKind = "view" | "text" | "button";

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

export interface HawkCompiledApp extends HawkAppSpec {
  readonly records: readonly string[];
}

export function createHawkApp(spec: HawkAppSpec): HawkCompiledApp {
  if (!spec.name.trim()) {
    throw new Error("Hawk2UI native app requires a stable name.");
  }
  validateElement(spec.root);
  return { ...spec, records: recordsForApp(spec) };
}

export function recordsForApp(spec: HawkAppSpec): readonly string[] {
  const records: string[] = [];
  emitLifecycle(spec.root, "mounted", records);
  emitElement(spec.root, records);
  emitLifecycle(spec.root, "unmounted", records);
  return records;
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

function isUnsafeAssetPath(path: string): boolean {
  return path.includes("://") || path.startsWith("/") || path.includes("..");
}
