export interface HawkSolidRenderOptions {
  readonly target: { readonly id: string };
}

export function renderHawkSolid(component: () => unknown, options: HawkSolidRenderOptions): () => void {
  if (!options.target.id.trim()) {
    throw new Error("Hawk2UI Solid render targets require a stable id.");
  }
  component();
  return () => undefined;
}
