import {
  recordsForApp,
  type HawkElementSpec,
  type HawkEventSpec,
} from "../../hawk2ui-native/src/index.ts";

export interface HawkSolidRenderOptions {
  readonly target: { readonly id: string };
}

export interface HawkSolidDisposer {
  (): void;
  readonly records: readonly string[];
  readonly snapshot: HawkElementSpec | undefined;
  readonly update: () => void;
}

const SOLID_RUNTIME_EVENT_KINDS = new Set<HawkEventSpec["kind"]>([
  "pointer.press",
  "pointer.release",
  "pointer.move",
  "pointer.drag",
  "pointer.enter",
  "pointer.leave",
  "pointer.wheel",
  "keyboard.key-down",
  "keyboard.key-up",
  "keyboard.text-input",
  "focus.focus-in",
  "focus.focus-out",
  "input.value-changed",
  "input.value-committed",
  "resize",
]);

export function renderHawkSolid(component: () => unknown, options: HawkSolidRenderOptions): HawkSolidDisposer {
  if (!options.target.id.trim()) {
    throw new Error("Hawk2UI Solid render targets require a stable id.");
  }
  const records: string[] = [];
  let root: HawkElementSpec | undefined = componentToNativeSpec(component(), options.target.id);
  validateUniqueChildKeys(root);
  records.push(...recordsForApp({ name: `solid:${options.target.id}`, root }));
  const dispose = (() => {
    if (root) {
      records.push(`unmount-element:${root.id}`);
      root = undefined;
    }
  }) as HawkSolidDisposer;
  Object.defineProperty(dispose, "records", {
    enumerable: true,
    get: () => records,
  });
  Object.defineProperty(dispose, "snapshot", {
    enumerable: true,
    get: () => root,
  });
  Object.defineProperty(dispose, "update", {
    enumerable: true,
    value: () => {
      if (!root) return;
      const next = componentToNativeSpec(component(), options.target.id);
      validateUniqueChildKeys(next);
      records.push(...diffRecords(root, next));
      root = next;
    },
  });
  return dispose;
}

function componentToNativeSpec(component: unknown, fallbackId: string): HawkElementSpec {
  return runtimeNodeSpec(component, fallbackId, "view");
}

function runtimeNodeSpec(
  component: unknown,
  fallbackId: string,
  fallbackKind: HawkElementSpec["kind"],
): HawkElementSpec {
  const props = readRecord(component);
  const id = readString(props, "id") ?? fallbackId;
  const asset = readString(props, "asset");
  const textProps = props ? readTextProp(props) : undefined;
  return {
    id,
    kind: runtimeKind(props, fallbackKind),
    refs: readString(props, "ref") ? [readString(props, "ref") as string] : [],
    styleRefs: readString(props, "class") ? [readString(props, "class") as string] : [],
    assetRefs: asset ? [{ name: "solid.asset", path: asset }] : [],
    events: runtimeEvents("solid", readStringArray(props, "on")),
    ...(textProps ? { props: textProps } : {}),
    children: readChildren(props).map(runtimeChildSpec),
  };
}

function runtimeChildSpec(child: Record<string, unknown>, index: number): HawkElementSpec {
  const id = readString(child, "id") ?? `child-${index}`;
  const key = readString(child, "key") ?? readString(child, "id");
  const spec = runtimeNodeSpec(child, id, "text");
  return key ? { ...spec, key } : spec;
}

function runtimeKind(
  record: Record<string, unknown> | undefined,
  fallback: HawkElementSpec["kind"],
): HawkElementSpec["kind"] {
  const kind = readString(record, "kind");
  return kind === "view" || kind === "text" || kind === "button" || kind === "custom-surface"
    ? kind
    : fallback;
}

function runtimeEvents(framework: string, eventNames: readonly string[]): readonly HawkEventSpec[] {
  return eventNames.map((kind) => {
    if (!isRuntimeEventKind(kind)) {
      throw new Error(`${framework}.event.unsupported: runtime event \`${kind}\` is not part of the native event contract.`);
    }
    return { kind, handler: kind };
  });
}

function isRuntimeEventKind(kind: string): kind is HawkEventSpec["kind"] {
  return SOLID_RUNTIME_EVENT_KINDS.has(kind as HawkEventSpec["kind"]);
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
