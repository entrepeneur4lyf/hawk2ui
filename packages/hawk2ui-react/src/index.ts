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
  return {
    get records() {
      return records;
    },
    render: (element: unknown) => {
      records.splice(0, records.length, ...recordsForApp({
        name: `react:${target.id}`,
        root: elementToNativeSpec(element, target.id),
      }));
    },
    unmount: () => {
      records.push(`unmount-element:${target.id}`);
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
    children: readChildren(element).map((child) => ({
      id: readString(child, "id") ?? "child",
      kind: "text",
      key: readString(child, "id"),
    })),
  };
}

function readProps(element: unknown): Record<string, unknown> | undefined {
  if (!element || typeof element !== "object") return undefined;
  const props = "props" in element ? (element as { readonly props?: unknown }).props : element;
  return props && typeof props === "object" ? (props as Record<string, unknown>) : undefined;
}

function readChildren(element: unknown): readonly Record<string, unknown>[] {
  if (!element || typeof element !== "object" || !("children" in element)) return [];
  const children = (element as { readonly children?: unknown }).children;
  return Array.isArray(children)
    ? children.filter((child): child is Record<string, unknown> => Boolean(child) && typeof child === "object")
    : [];
}

function readString(record: Record<string, unknown> | undefined, name: string): string | undefined {
  const value = record?.[name];
  return typeof value === "string" && value.trim() ? value : undefined;
}
