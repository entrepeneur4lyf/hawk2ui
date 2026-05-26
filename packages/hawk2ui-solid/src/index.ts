import { recordsForApp, type HawkElementSpec } from "../../hawk2ui-native/src/index.ts";

export interface HawkSolidRenderOptions {
  readonly target: { readonly id: string };
}

export interface HawkSolidDisposer {
  (): void;
  readonly records: readonly string[];
}

export function renderHawkSolid(component: () => unknown, options: HawkSolidRenderOptions): HawkSolidDisposer {
  if (!options.target.id.trim()) {
    throw new Error("Hawk2UI Solid render targets require a stable id.");
  }
  const records: string[] = [];
  const root = componentToNativeSpec(component(), options.target.id);
  records.push(...recordsForApp({ name: `solid:${options.target.id}`, root }));
  const dispose = (() => {
    records.push(`unmount-element:${root.id}`);
  }) as HawkSolidDisposer;
  Object.defineProperty(dispose, "records", {
    enumerable: true,
    get: () => records,
  });
  return dispose;
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
    assetRefs: asset ? [{ name: "solid.asset", path: asset }] : [],
    events: readStringArray(props, "on").includes("pointer.press")
      ? [{ kind: "pointer.press", handler: "handlePress" }]
      : [],
    children: readChildren(props).map((child) => ({
      id: readString(child, "id") ?? "child",
      kind: "text",
      key: readString(child, "id"),
    })),
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
