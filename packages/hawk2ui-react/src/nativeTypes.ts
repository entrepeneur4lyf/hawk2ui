import type { ReactNode } from "react";

export type HawkSceneNodeKind =
  | "view"
  | "text"
  | "button"
  | "input"
  | "image"
  | "vector"
  | "custom-surface"
  | "scroll-view"
  | "list";

export interface HawkNativeNodeHandle {
  readonly id: string;
  readonly kind: HawkSceneNodeKind;
  focus(): void;
  measure(request: string): void;
}

export type HawkSceneValue =
  | { readonly kind: "null" }
  | { readonly kind: "bool"; readonly value: boolean }
  | { readonly kind: "number"; readonly value: number }
  | { readonly kind: "string"; readonly value: string };

export type HawkSceneOp =
  | { readonly type: "create-node"; readonly id: string; readonly kind: HawkSceneNodeKind }
  | { readonly type: "create-text"; readonly id: string; readonly text: string }
  | { readonly type: "set-prop"; readonly id: string; readonly name: string; readonly value: HawkSceneValue }
  | { readonly type: "set-style"; readonly id: string; readonly name: string; readonly value: HawkSceneValue }
    | {
        readonly type: "set-accessibility";
      readonly id: string;
      readonly role?: string;
      readonly label?: string;
      readonly description?: string;
      readonly value?: HawkSceneValue;
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

export interface HawkSceneOpBatch {
  readonly ops: readonly HawkSceneOp[];
}

export interface HawkNativeProps {
  readonly id?: string;
  readonly ref?: string | ((node: unknown) => void) | { current: unknown } | null;
  readonly class?: string;
  readonly className?: string;
  readonly style?: Readonly<Record<string, unknown>>;
  readonly children?: ReactNode;
  readonly text?: string | number;
  readonly role?: string;
  readonly label?: string;
  readonly description?: string;
  readonly ariaLabel?: string;
  readonly "aria-label"?: string;
  readonly disabled?: boolean;
  readonly checked?: boolean;
  readonly pressed?: boolean;
  readonly selected?: boolean;
    readonly value?: string | number | boolean;
    readonly autoFocus?: boolean;
    readonly measure?: string;
    readonly onClick?: HawkEventHandler;
  readonly onPointerPress?: HawkEventHandler;
  readonly onPointerDown?: HawkEventHandler;
  readonly onPointerUp?: HawkEventHandler;
  readonly onPointerMove?: HawkEventHandler;
  readonly onPointerDrag?: HawkEventHandler;
  readonly onPointerEnter?: HawkEventHandler;
  readonly onPointerLeave?: HawkEventHandler;
  readonly onWheel?: HawkEventHandler;
  readonly onKeyDown?: HawkEventHandler;
  readonly onKeyUp?: HawkEventHandler;
  readonly onTextInput?: HawkEventHandler;
  readonly onFocus?: HawkEventHandler;
  readonly onBlur?: HawkEventHandler;
  readonly onInput?: HawkEventHandler;
  readonly onChange?: HawkEventHandler;
  readonly onResize?: HawkEventHandler;
  readonly [name: string]: unknown;
}

export type HawkEventHandler = string | ((event: unknown) => void);

export type HawkReactRootTarget = string | HawkReactRootContainer;

export interface HawkReactRootContainer {
  readonly id: string;
}

export interface HawkReactRootOptions {
  readonly bridge?: HawkSceneBridge;
  readonly strictMode?: boolean;
  readonly identifierPrefix?: string;
  readonly onUncaughtError?: HawkReactErrorHandler;
  readonly onCaughtError?: HawkReactErrorHandler;
  readonly onRecoverableError?: HawkReactErrorHandler;
}

export interface HawkReactRootConfig extends HawkReactRootOptions {
  readonly id: string;
}

export interface HawkReactRoot {
  readonly id: string;
  render(element: ReactNode): void;
  unmount(): void;
  committedBatches(): readonly HawkSceneOpBatch[];
  drainCommittedBatches(): readonly HawkSceneOpBatch[];
}

export interface HawkSceneBridge {
  commit(batch: HawkSceneOpBatch): void;
  registerEventHandler?(nodeId: string, event: string, handlerId: string, handler: (event: unknown) => void): void;
  unregisterEventHandler?(nodeId: string, event: string): void;
}

export type HawkReactErrorHandler = (error: Error, info: { readonly componentStack?: string }) => void;
