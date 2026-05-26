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

  const records = [`mount-element:${rootId}`];
  const ref = readAttribute(input.source, "use:ref");
  if (ref) records.push(`ref:${rootId}:${ref}`);
  const style = readAttribute(input.source, "class");
  if (style) records.push(`style:${rootId}:${style}`);
  if (assetPath) records.push(`asset:${rootId}:${assetPath}`);
  if (input.source.includes("on:press")) records.push(`bind-event:${rootId}:pointer.press`);
  if (input.source.includes("on:mount")) records.push(`lifecycle:mounted:${rootId}:onMount`);
  if (input.source.includes("on:destroy")) records.push(`lifecycle:unmounted:${rootId}:onDestroy`);
  for (const childId of keyedChildIds(input.source)) {
    records.push(`mount-element:${childId}`);
  }

  return { framework: "svelte", filename: input.filename, records };
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
