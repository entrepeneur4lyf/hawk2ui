import { recordsForApp, type HawkElementSpec } from "../../hawk2ui-native/src/index.ts";

export interface HawkVueRenderer {
  readonly records: readonly string[];
  readonly render: (component: unknown, target: { readonly id: string }) => void;
  readonly unmount: (target: { readonly id: string }) => void;
}

export function createHawkVueRenderer(): HawkVueRenderer {
  const records: string[] = [];
  return {
    get records() {
      return records;
    },
    render: (component: unknown, target: { readonly id: string }) => {
      if (!target.id.trim()) {
        throw new Error("Hawk2UI Vue render targets require a stable id.");
      }
      records.splice(0, records.length, ...recordsForApp({
        name: `vue:${target.id}`,
        root: componentToNativeSpec(component, target.id),
      }));
    },
    unmount: (target: { readonly id: string }) => {
      if (!target.id.trim()) {
        throw new Error("Hawk2UI Vue render targets require a stable id.");
      }
      records.push(`unmount-element:${target.id}`);
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
