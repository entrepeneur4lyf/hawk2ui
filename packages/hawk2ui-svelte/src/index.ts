import { recordsForApp, type HawkElementSpec } from "../../hawk2ui-native/src/index.ts";

export interface HawkSvelteCompileInput {
  readonly filename: string;
  readonly source: string;
}

export interface HawkSvelteCompileOutput {
  readonly framework: "svelte";
  readonly filename: string;
  readonly records: readonly string[];
}

export function compileHawkSvelte(input: HawkSvelteCompileInput): HawkSvelteCompileOutput {
  if (!input.filename.endsWith(".svelte")) {
    throw new Error("Hawk2UI Svelte inputs must be .svelte files.");
  }
  const rootId = readAttribute(input.source, "id") ?? "root";
  const assetPath = readAttribute(input.source, "data-asset");
  if (assetPath && isUnsafeAssetPath(assetPath)) {
    throw new Error("svelte.asset.path-invalid: Svelte asset references must use workspace-relative paths.");
  }

  const ref = readAttribute(input.source, "use:ref");
  const style = readAttribute(input.source, "class");
  const lifecycle: HawkElementSpec["lifecycle"] = [];
  if (input.source.includes("on:mount")) {
    lifecycle.push({ phase: "mounted", handler: "onMount" });
  }
  if (input.source.includes("on:destroy")) {
    lifecycle.push({ phase: "unmounted", handler: "onDestroy" });
  }
  const root: HawkElementSpec = {
    id: rootId,
    kind: "view",
    refs: ref ? [ref] : [],
    styleRefs: style ? [style] : [],
    assetRefs: assetPath ? [{ name: "svelte.asset", path: assetPath }] : [],
    events: input.source.includes("on:press") ? [{ kind: "pointer.press", handler: "handlePress" }] : [],
    lifecycle,
    children: keyedChildIds(input.source).map((childId) => ({
      id: childId,
      kind: "text",
      key: childId,
    })),
  };

  return { framework: "svelte", filename: input.filename, records: recordsForApp({ name: input.filename, root }) };
}

function readAttribute(source: string, name: string): string | undefined {
  const pattern = `${name}="`;
  const start = source.indexOf(pattern);
  if (start < 0) return undefined;
  const valueStart = start + pattern.length;
  const valueEnd = source.indexOf('"', valueStart);
  if (valueEnd < 0) return undefined;
  return source.slice(valueStart, valueEnd);
}

function isUnsafeAssetPath(path: string): boolean {
  return path.includes("://") || path.startsWith("/") || path.includes("..");
}

function keyedChildIds(source: string): readonly string[] {
  return source.includes("(item.id)") ? ["title", "cta"] : [];
}
