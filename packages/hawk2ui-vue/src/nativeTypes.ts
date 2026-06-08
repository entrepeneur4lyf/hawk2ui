export type HawkVueNodeKind =
  | "view"
  | "text"
  | "button"
  | "input"
  | "image"
  | "vector"
  | "custom-surface"
  | "scroll-view"
  | "list";

export interface HawkVueRoot {
  readonly id: string;
  dispatch(nodeId: string, event: string, payload?: unknown): void;
  flush(): Promise<void>;
  committedBatches(): readonly HawkVueSceneOpBatch[];
  drainCommittedBatches(): readonly HawkVueSceneOpBatch[];
}

export interface HawkVueMountOptions {
  readonly rootId?: string;
}

export interface HawkVueApp {
  mount(target?: string | HawkVueMountOptions): HawkVueRoot;
  unmount(): void;
}

export type HawkVueSceneValue =
  | { readonly kind: "null" }
  | { readonly kind: "bool"; readonly value: boolean }
  | { readonly kind: "number"; readonly value: number }
  | { readonly kind: "string"; readonly value: string };

export type HawkVueSceneOp =
  | { readonly type: "create-node"; readonly id: string; readonly kind: HawkVueNodeKind }
  | { readonly type: "create-text"; readonly id: string; readonly text: string }
  | { readonly type: "set-prop"; readonly id: string; readonly name: string; readonly value: HawkVueSceneValue }
  | { readonly type: "set-style"; readonly id: string; readonly name: string; readonly value: HawkVueSceneValue }
  | {
      readonly type: "set-accessibility";
      readonly id: string;
      readonly role?: string;
      readonly label?: string;
      readonly description?: string;
      readonly value?: HawkVueSceneValue;
      readonly disabled?: boolean;
      readonly checked?: boolean;
      readonly pressed?: boolean;
      readonly focused?: boolean;
    }
  | { readonly type: "focus-node"; readonly id: string }
  | { readonly type: "measure-node"; readonly id: string; readonly request: string }
  | { readonly type: "append-child"; readonly parent: string; readonly child: string }
  | { readonly type: "insert-before"; readonly parent: string; readonly child: string; readonly before: string }
  | { readonly type: "remove-child"; readonly parent: string; readonly child: string }
  | { readonly type: "replace-text"; readonly id: string; readonly text: string }
  | { readonly type: "register-event"; readonly id: string; readonly event: string; readonly handler: string }
  | { readonly type: "unregister-event"; readonly id: string; readonly event: string }
  | { readonly type: "commit" }
  | { readonly type: "dispose-subtree"; readonly id: string };

export interface HawkVueSceneOpBatch {
  readonly ops: readonly HawkVueSceneOp[];
}

export interface HawkVueSceneBridge {
  createNode(kind: HawkVueNodeKind, id: string): void;
  createText(id: string, text: string): void;
  setProp(id: string, name: string, value: unknown): void;
  setStyle(id: string, name: string, value: unknown): void;
  setAccessibility(id: string, semantics: HawkVueAccessibilitySemantics): void;
  appendChild(parent: string, child: string): void;
  insertBefore(parent: string, child: string, before: string): void;
  removeChild(parent: string, child: string): void;
  replaceText(id: string, text: string): void;
  registerEventHandler(nodeId: string, event: string, handlerId: string, handler: (event: unknown) => void): void;
  unregisterEventHandler(nodeId: string, event: string): void;
  dispatch(nodeId: string, event: string, payload?: unknown): void;
  commit(): void;
  batches(): readonly HawkVueSceneOpBatch[];
  drain(): readonly HawkVueSceneOpBatch[];
}

export interface HawkVueAccessibilitySemantics {
  readonly role?: string;
  readonly label?: string;
  readonly description?: string;
  readonly value?: unknown;
  readonly disabled?: boolean;
  readonly checked?: boolean;
  readonly pressed?: boolean;
  readonly focused?: boolean;
}

export interface HawkVueAppOptions {
  readonly bridge?: HawkVueSceneBridge;
  readonly commit?: (ops: readonly HawkVueSceneOp[]) => void;
}
