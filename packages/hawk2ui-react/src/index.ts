import type { ReactNode } from "react";

import {
  createHawkRootContainer,
  createReconcilerRoot,
  updateReconcilerRoot,
  type HawkRootContainer,
} from "./hostConfig";
import type {
  HawkReactRoot,
  HawkReactRootConfig,
  HawkReactRootOptions,
  HawkReactRootTarget,
  HawkSceneOpBatch,
} from "./nativeTypes";
export type {
  HawkEventHandler,
  HawkNativeNodeHandle,
  HawkNativeProps,
  HawkReactRoot,
  HawkReactRootConfig,
  HawkReactRootContainer,
  HawkReactRootOptions,
  HawkReactRootTarget,
  HawkSceneBridge,
  HawkSceneNodeKind,
  HawkSceneOp,
  HawkSceneOpBatch,
  HawkSceneValue,
} from "./nativeTypes";
export { createGlobalSceneBridge, createRecordingSceneBridge, RecordingSceneBridge } from "./sceneBridge";

export function createRoot(target: HawkReactRootTarget, options: HawkReactRootOptions = {}): HawkReactRoot {
  const config = rootConfig(target, options);
  const container = createHawkRootContainer(config);
  const root = createReconcilerRoot(container, config);
  return new HawkReactRootImpl(config.id, container, (element) => updateReconcilerRoot(root, element));
}

function rootConfig(target: HawkReactRootTarget, options: HawkReactRootOptions): HawkReactRootConfig {
  const id = typeof target === "string" ? target : target.id;
  return { ...options, id };
}

class HawkReactRootImpl implements HawkReactRoot {
  constructor(
    readonly id: string,
    private readonly container: HawkRootContainer,
    private readonly update: (element: ReactNode) => void,
  ) {}

  render(element: ReactNode): void {
    this.update(element);
    this.throwPendingRenderError();
  }

  unmount(): void {
    this.update(null);
    this.throwPendingRenderError();
  }

  committedBatches(): readonly HawkSceneOpBatch[] {
    return cloneBatches(this.container.committed);
  }

  drainCommittedBatches(): readonly HawkSceneOpBatch[] {
    const drained = this.committedBatches();
    this.container.committed.length = 0;
    return drained;
  }

  private throwPendingRenderError(): void {
    const error = this.container.errors.shift();
    if (error) throw error;
  }
}

function cloneBatches(batches: readonly HawkSceneOpBatch[]): readonly HawkSceneOpBatch[] {
  return batches.map((batch) => ({ ops: batch.ops.map((op) => ({ ...op })) }));
}
