export interface HawkReactRoot {
  readonly render: (element: unknown) => void;
  readonly unmount: () => void;
}

export function createHawkReactRoot(target: { readonly id: string }): HawkReactRoot {
  if (!target.id.trim()) {
    throw new Error("Hawk2UI React roots require a stable target id.");
  }
  return { render: () => undefined, unmount: () => undefined };
}
