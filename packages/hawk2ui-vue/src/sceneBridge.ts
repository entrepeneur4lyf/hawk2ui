import type {
  HawkVueAccessibilitySemantics,
  HawkVueNodeKind,
  HawkVueSceneBridge,
  HawkVueSceneOp,
  HawkVueSceneOpBatch,
  HawkVueSceneValue,
} from "./nativeTypes";

const GLOBAL_COMMIT_FUNCTION = "__hawk2uiCommitScene";
const UNSUPPORTED_DOM_PROPS = new Set([
  "contentEditable",
  "dangerouslySetInnerHTML",
  "htmlFor",
  "innerHTML",
  "outerHTML",
  "suppressContentEditableWarning",
  "suppressHydrationWarning",
]);

export class RecordingVueSceneBridge implements HawkVueSceneBridge {
  readonly #pending: HawkVueSceneOp[] = [];
  readonly #batches: HawkVueSceneOpBatch[] = [];
  readonly #handlers = new Map<string, (event: unknown) => void>();
  readonly #commit: ((ops: readonly HawkVueSceneOp[]) => void) | undefined;

  constructor(commit?: (ops: readonly HawkVueSceneOp[]) => void) {
    this.#commit = commit;
  }

  get operations(): readonly HawkVueSceneOp[] {
    return [...this.#batches.flatMap((batch) => batch.ops), ...this.#pending.map(cloneOp)];
  }

  createNode(kind: HawkVueNodeKind, id: string): void {
    assertStableId(id, "vue.node.id-required");
    this.#pending.push({ type: "create-node", id, kind });
  }

  createText(id: string, text: string): void {
    assertStableId(id, "vue.node.id-required");
    this.#pending.push({ type: "create-text", id, text });
  }

  setProp(id: string, name: string, value: unknown): void {
    assertSupportedProp(name, value);
    this.#pending.push({ type: "set-prop", id, name, value: sceneValue(value) });
  }

  setStyle(id: string, name: string, value: unknown): void {
    this.#pending.push({ type: "set-style", id, name, value: sceneValue(value) });
  }

  setAccessibility(id: string, semantics: HawkVueAccessibilitySemantics): void {
    const op: HawkVueSceneOp = {
      type: "set-accessibility",
      id,
    };
    const role = stringOrUndefined(semantics.role);
    const label = stringOrUndefined(semantics.label);
    const description = stringOrUndefined(semantics.description);
    const disabled = booleanOrUndefined(semantics.disabled);
    const checked = booleanOrUndefined(semantics.checked);
    const pressed = booleanOrUndefined(semantics.pressed);
    const focused = booleanOrUndefined(semantics.focused);
    this.#pending.push({
      ...op,
      ...(role === undefined ? {} : { role }),
      ...(label === undefined ? {} : { label }),
      ...(description === undefined ? {} : { description }),
      ...(semantics.value === undefined ? {} : { value: sceneValue(semantics.value) }),
      ...(disabled === undefined ? {} : { disabled }),
      ...(checked === undefined ? {} : { checked }),
      ...(pressed === undefined ? {} : { pressed }),
      ...(focused === undefined ? {} : { focused }),
    });
  }

  appendChild(parent: string, child: string): void {
    this.#pending.push({ type: "append-child", parent, child });
  }

  insertBefore(parent: string, child: string, before: string): void {
    this.#pending.push({ type: "insert-before", parent, child, before });
  }

  removeChild(parent: string, child: string): void {
    this.#pending.push({ type: "remove-child", parent, child });
  }

  replaceText(id: string, text: string): void {
    this.#pending.push({ type: "replace-text", id, text });
  }

  registerEventHandler(nodeId: string, event: string, handlerId: string, handler: (event: unknown) => void): void {
    this.#handlers.set(handlerKey(nodeId, event), handler);
    this.#pending.push({ type: "register-event", id: nodeId, event, handler: handlerId });
  }

  unregisterEventHandler(nodeId: string, event: string): void {
    this.#handlers.delete(handlerKey(nodeId, event));
    this.#pending.push({ type: "unregister-event", id: nodeId, event });
  }

  dispatch(nodeId: string, event: string, payload: unknown = {}): void {
    const handler = this.#handlers.get(handlerKey(nodeId, event));
    if (!handler) {
      throw new Error(`vue.event.handler-missing: no handler registered for ${nodeId}:${event}.`);
    }
    handler(payload);
  }

  commit(): void {
    if (this.#pending.length === 0) return;
    const ops = [...this.#pending.map(cloneOp), { type: "commit" } as const];
    this.#pending.length = 0;
    this.#batches.push({ ops });
    this.#commit?.(ops);
  }

  batches(): readonly HawkVueSceneOpBatch[] {
    return this.#batches.map((batch) => ({ ops: batch.ops.map(cloneOp) }));
  }

  drain(): readonly HawkVueSceneOpBatch[] {
    const drained = this.batches();
    this.#batches.length = 0;
    return drained;
  }
}

export function createRecordingVueSceneBridge(commit?: (ops: readonly HawkVueSceneOp[]) => void): RecordingVueSceneBridge {
  return new RecordingVueSceneBridge(commit);
}

export function createGlobalVueSceneBridge(globalObject: object = globalThis): HawkVueSceneBridge {
  const commit = (globalObject as Record<string, unknown>)[GLOBAL_COMMIT_FUNCTION];
  if (typeof commit !== "function") {
    throw new Error(
      `vue.scene-bridge.missing: globalThis.${GLOBAL_COMMIT_FUNCTION} is not available in this Hawk2UI runtime.`,
    );
  }
  return createRecordingVueSceneBridge((ops) => commit({ ops: ops.map(cloneOp) }));
}

function assertStableId(id: string, rule: string): void {
  if (!id.trim()) throw new Error(`${rule}: Hawk2UI Vue nodes require stable non-empty ids.`);
}

function assertSupportedProp(name: string, value: unknown): void {
  if (UNSUPPORTED_DOM_PROPS.has(name)) {
    throw new Error(`vue.prop.unsupported: DOM-only prop \`${name}\` is not supported by the native renderer.`);
  }
  if (name === "style" && typeof value === "string") {
    throw new Error("vue.prop.unsupported: style must be an object or class reference, not a raw CSS string.");
  }
}

function sceneValue(value: unknown): HawkVueSceneValue {
  if (value === null || value === undefined) return { kind: "null" };
  if (typeof value === "boolean") return { kind: "bool", value };
  if (typeof value === "number" && Number.isFinite(value)) return { kind: "number", value };
  if (typeof value === "string") return { kind: "string", value };
  throw new Error(`vue.prop.unsupported: scene values must be null, boolean, finite number, or string.`);
}

function stringOrUndefined(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

function booleanOrUndefined(value: unknown): boolean | undefined {
  return typeof value === "boolean" ? value : undefined;
}

function cloneOp(op: HawkVueSceneOp): HawkVueSceneOp {
  return { ...op };
}

function handlerKey(nodeId: string, event: string): string {
  return `${nodeId}:${event}`;
}
