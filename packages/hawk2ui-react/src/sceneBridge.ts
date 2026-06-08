import type { HawkSceneBridge, HawkSceneOpBatch } from "./nativeTypes.ts";

const GLOBAL_COMMIT_FUNCTION = "__hawk2uiCommitScene";

export class RecordingSceneBridge implements HawkSceneBridge {
  readonly #batches: HawkSceneOpBatch[] = [];
  readonly #handlers = new Map<string, (event: unknown) => void>();

  commit(batch: HawkSceneOpBatch): void {
    this.#batches.push(cloneBatch(batch));
  }

  registerEventHandler(nodeId: string, event: string, _handlerId: string, handler: (event: unknown) => void): void {
    this.#handlers.set(handlerKey(nodeId, event), handler);
  }

  unregisterEventHandler(nodeId: string, event: string): void {
    this.#handlers.delete(handlerKey(nodeId, event));
  }

  dispatch(nodeId: string, event: string, payload: unknown): void {
    const handler = this.#handlers.get(handlerKey(nodeId, event));
    if (!handler) {
      throw new Error(`react.event.handler-missing: no handler registered for ${nodeId}:${event}.`);
    }
    handler(payload);
  }

  batches(): readonly HawkSceneOpBatch[] {
    return this.#batches.map(cloneBatch);
  }

  drain(): readonly HawkSceneOpBatch[] {
    const drained = this.batches();
    this.#batches.length = 0;
    return drained;
  }
}

export function createRecordingSceneBridge(): RecordingSceneBridge {
  return new RecordingSceneBridge();
}

export function createGlobalSceneBridge(globalObject: object = globalThis): HawkSceneBridge {
  const commit = (globalObject as Record<string, unknown>)[GLOBAL_COMMIT_FUNCTION];
  if (typeof commit !== "function") {
    throw new Error(
      `react.scene-bridge.missing: globalThis.${GLOBAL_COMMIT_FUNCTION} is not available in this Hawk2UI runtime.`,
    );
  }
  return {
    commit(batch) {
      commit(cloneBatch(batch));
    },
  };
}

function cloneBatch(batch: HawkSceneOpBatch): HawkSceneOpBatch {
  return { ops: batch.ops.map((op) => ({ ...op })) };
}

function handlerKey(nodeId: string, event: string): string {
  return `${nodeId}:${event}`;
}
