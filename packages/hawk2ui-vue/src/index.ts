export interface HawkVueRenderer {
  readonly render: (component: unknown, target: { readonly id: string }) => void;
  readonly unmount: (target: { readonly id: string }) => void;
}

export function createHawkVueRenderer(): HawkVueRenderer {
  return { render: () => undefined, unmount: () => undefined };
}
