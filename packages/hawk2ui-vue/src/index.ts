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
      const rootId = readComponentId(component) ?? target.id;
      records.push(`mount-element:${rootId}`);
      records.push(`patch-props:${rootId}`);
    },
    unmount: (target: { readonly id: string }) => {
      if (!target.id.trim()) {
        throw new Error("Hawk2UI Vue render targets require a stable id.");
      }
      records.push(`unmount-element:${target.id}`);
    },
  };
}

function readComponentId(component: unknown): string | undefined {
  if (!component || typeof component !== "object" || !("id" in component)) return undefined;
  const id = (component as { readonly id?: unknown }).id;
  return typeof id === "string" && id.trim() ? id : undefined;
}
