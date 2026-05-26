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
      const rootId = readElementId(element) ?? target.id;
      records.push(`mount-element:${rootId}`);
      records.push(`commit-root:${rootId}`);
    },
    unmount: () => {
      records.push(`unmount-element:${target.id}`);
    },
  };
}

function readElementId(element: unknown): string | undefined {
  if (!element || typeof element !== "object") return undefined;
  const props = "props" in element ? (element as { readonly props?: unknown }).props : element;
  if (!props || typeof props !== "object" || !("id" in props)) return undefined;
  const id = (props as { readonly id?: unknown }).id;
  return typeof id === "string" && id.trim() ? id : undefined;
}
