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
  const rootId = readComponentId(component()) ?? options.target.id;
  records.push(`mount-element:${rootId}`);
  records.push(`effect:${rootId}`);
  const dispose = (() => {
    records.push(`unmount-element:${rootId}`);
  }) as HawkSolidDisposer;
  Object.defineProperty(dispose, "records", {
    enumerable: true,
    get: () => records,
  });
  return dispose;
}

function readComponentId(component: unknown): string | undefined {
  if (!component || typeof component !== "object" || !("id" in component)) return undefined;
  const id = (component as { readonly id?: unknown }).id;
  return typeof id === "string" && id.trim() ? id : undefined;
}
