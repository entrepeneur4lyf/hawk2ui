import { recordsForApp, type HawkElementSpec } from "../../hawk2ui-native/src/index.ts";

export interface HawkReactRoot {
  readonly records: readonly string[];
  readonly render: (element: unknown) => void;
  readonly unmount: () => void;
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
    children: readChildren(element, props).map((child, index) => ({
      id: readString(readProps(child), "id") ?? readString(child, "id") ?? `child-${index}`,
      kind: "text",
      key: readString(child, "key") ?? readString(readProps(child), "id") ?? readString(child, "id"),
      props: readTextProp(child),
    })),
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
