import * as React from "react";
import ReactReconciler from "react-reconciler";
import { ConcurrentRoot, DefaultEventPriority } from "react-reconciler/constants";

import type {
  HawkNativeProps,
  HawkNativeNodeHandle,
  HawkReactErrorHandler,
  HawkReactRootConfig,
  HawkSceneBridge,
  HawkSceneNodeKind,
  HawkSceneOp,
  HawkSceneOpBatch,
  HawkSceneValue,
} from "./nativeTypes.ts";
import { createGlobalSceneBridge } from "./sceneBridge.ts";

export type HawkHostType =
  | "hawk-view"
  | "hawk-text"
  | "hawk-button"
  | "hawk-input"
  | "hawk-image"
  | "hawk-vector"
  | "hawk-surface"
  | "hawk-custom-surface"
  | "hawk-scroll-view"
  | "hawk-list"
  | "view"
  | "text"
  | "button"
  | "input"
  | "image"
  | "vector"
  | "custom-surface"
  | "scroll-view"
  | "list"
  | string;

export interface HawkRootContainer {
  readonly id: string;
  readonly bridge: HawkSceneBridge;
  readonly committed: HawkSceneOpBatch[];
  readonly errors: Error[];
  roots: HawkChild[];
  pendingOps: HawkSceneOp[];
  nextTextId: number;
}

interface HawkNativeInstance {
  readonly nodeType: "instance";
  readonly type: HawkHostType;
  readonly id: string;
  readonly kind: HawkSceneNodeKind;
  props: HawkNativeProps;
  children: HawkChild[];
  parent: HawkParent | undefined;
  attached: boolean;
  hidden: boolean;
  handle: HawkNativeNodeHandle | undefined;
}

interface HawkTextInstance {
  readonly nodeType: "text";
  readonly id: string;
  text: string;
  parent: HawkParent | undefined;
  attached: boolean;
  hidden: boolean;
  handle: HawkNativeNodeHandle | undefined;
}

type HawkChild = HawkNativeInstance | HawkTextInstance;
type HawkParent = HawkRootContainer | HawkNativeInstance;
type TimeoutHandle = ReturnType<typeof setTimeout>;
type NoTimeout = -1;
type TransitionStatus = null;
interface HawkHostContext {
  readonly renderer: "hawk2ui-react";
}
type HawkHostConfig = ReactReconciler.HostConfig<
  HawkHostType,
  HawkNativeProps,
  HawkRootContainer,
  HawkNativeInstance,
  HawkTextInstance,
  never,
  never,
  HawkNativeInstance,
  HawkNativeNodeHandle,
  HawkHostContext,
  never,
  TimeoutHandle,
  NoTimeout,
  TransitionStatus
>;
type AccessibilitySemantics = {
  role?: string;
  label?: string;
  description?: string;
  value?: HawkSceneValue;
  disabled?: boolean;
  checked?: boolean;
  pressed?: boolean;
  focused?: boolean;
};
type HawkNativeEventTarget = {
  readonly id: string;
  readonly kind: HawkSceneNodeKind;
  readonly value?: unknown;
};
type HawkNativeEventPayload = Record<string, unknown> & {
  readonly target: HawkNativeEventTarget;
  readonly currentTarget: HawkNativeEventTarget;
};

const noTimeout = -1 as const;
const hostContext: HawkHostContext = { renderer: "hawk2ui-react" };
const transitionContext = React.createContext<TransitionStatus>(
  null,
) as unknown as ReactReconciler.ReactContext<TransitionStatus>;
let currentUpdatePriority = DefaultEventPriority;
let nextFunctionHandlerId = 1;
const functionHandlerIds = new WeakMap<Function, string>();

const EVENT_PROPS = new Map<string, string>([
  ["onClick", "pointer.press"],
  ["onPointerPress", "pointer.press"],
  ["onPointerDown", "pointer.press"],
  ["onPointerUp", "pointer.release"],
  ["onPointerMove", "pointer.move"],
  ["onPointerDrag", "pointer.drag"],
  ["onPointerEnter", "pointer.enter"],
  ["onPointerLeave", "pointer.leave"],
  ["onWheel", "pointer.wheel"],
  ["onKeyDown", "keyboard.key-down"],
  ["onKeyUp", "keyboard.key-up"],
  ["onTextInput", "keyboard.text-input"],
  ["onFocus", "focus.focus-in"],
  ["onBlur", "focus.focus-out"],
  ["onInput", "input.value-changed"],
  ["onChange", "input.value-committed"],
  ["onResize", "resize"],
]);

const RESERVED_PROPS = new Set<string>([
  "id",
  "children",
  "class",
  "className",
  "style",
  "ref",
  "key",
  "role",
  "label",
  "description",
  "ariaLabel",
  "aria-label",
  "disabled",
  "checked",
  "pressed",
  "selected",
  "value",
  "autoFocus",
  "measure",
  "focused",
  ...EVENT_PROPS.keys(),
]);

const UNSUPPORTED_DOM_PROPS = new Set<string>([
  "contentEditable",
  "dangerouslySetInnerHTML",
  "htmlFor",
  "innerHTML",
  "suppressContentEditableWarning",
  "suppressHydrationWarning",
]);

const VIEW_ALIASES = new Set([
  "div",
  "section",
  "main",
  "article",
  "header",
  "footer",
  "nav",
  "aside",
  "form",
  "label",
  "ul",
  "ol",
  "li",
]);
const TEXT_ALIASES = new Set(["span", "p", "strong", "em", "small", "code", "h1", "h2", "h3", "h4", "h5", "h6"]);
const TEXTUAL_KINDS = new Set<HawkSceneNodeKind>(["text", "button", "input"]);

export const hawkHostConfig: HawkHostConfig = {
  supportsMutation: true,
  supportsPersistence: false,
  supportsHydration: false,
  isPrimaryRenderer: true,
  supportsMicrotasks: true,
  noTimeout,
  NotPendingTransition: null,
  HostTransitionContext: transitionContext,

  createInstance(type, props) {
    const kind = kindForType(type);
    const id = requiredNodeId(type, props);
    return {
      nodeType: "instance",
      type,
      id,
      kind,
      props,
      children: [],
      parent: undefined,
      attached: false,
      hidden: false,
      handle: undefined,
    };
  },

  createTextInstance(text, rootContainer) {
    return {
      nodeType: "text",
      id: `${rootContainer.id}:text:${rootContainer.nextTextId++}`,
      text,
      parent: undefined,
      attached: false,
      hidden: false,
      handle: undefined,
    };
  },

  appendInitialChild(parentInstance, child) {
    attachLocal(parentInstance, child);
  },

  finalizeInitialChildren() {
    return false;
  },

  shouldSetTextContent(type, props) {
    return TEXTUAL_KINDS.has(kindForType(type)) && isTextScalar(props.children);
  },

  getRootHostContext() {
    return hostContext;
  },

  getChildHostContext() {
    return hostContext;
  },

  getPublicInstance(instance) {
    return publicNodeHandle(instance);
  },

  prepareForCommit() {
    return null;
  },

  resetAfterCommit(container) {
    flushPendingOps(container);
  },

  preparePortalMount() {},
  scheduleTimeout: setTimeout,
  cancelTimeout: clearTimeout,
  scheduleMicrotask: queueMicrotask,
  getInstanceFromNode() {
    return null;
  },
  beforeActiveInstanceBlur() {},
  afterActiveInstanceBlur() {},
  prepareScopeUpdate() {},
  getInstanceFromScope() {
    return null;
  },
  detachDeletedInstance(node) {
    node.parent = undefined;
    node.attached = false;
  },

  appendChild(parentInstance, child) {
    const oldParent = child.parent;
    attachLocal(parentInstance, child);
    if (parentInstance.attached) {
      if (oldParent && oldParent !== parentInstance && child.attached) emitRemove(oldParent, child);
      emitAttach(parentInstance, child, undefined);
    }
  },

  appendChildToContainer(container, child) {
    const oldParent = child.parent;
    attachLocal(container, child);
    if (oldParent && oldParent !== container && child.attached) emitRemove(oldParent, child);
    emitAttach(container, child, undefined);
  },

  insertBefore(parentInstance, child, beforeChild) {
    const oldParent = child.parent;
    attachLocal(parentInstance, child, beforeChild);
    if (parentInstance.attached) {
      if (oldParent && oldParent !== parentInstance && child.attached) emitRemove(oldParent, child);
      emitAttach(parentInstance, child, beforeChild);
    }
  },

  insertInContainerBefore(container, child, beforeChild) {
    const oldParent = child.parent;
    attachLocal(container, child, beforeChild);
    if (oldParent && oldParent !== container && child.attached) emitRemove(oldParent, child);
    emitAttach(container, child, beforeChild);
  },

  removeChild(parentInstance, child) {
    detachLocal(parentInstance, child);
      if (child.attached) {
        emitRemove(parentInstance, child);
        emitOp(parentInstance, { type: "dispose-subtree", id: child.id });
        unregisterSubtreeHandlers(rootContainerFor(parentInstance), child);
        markDetached(child);
      }
  },

  removeChildFromContainer(container, child) {
    detachLocal(container, child);
      if (child.attached) {
        emitRemove(container, child);
        emitOp(container, { type: "dispose-subtree", id: child.id });
        unregisterSubtreeHandlers(container, child);
        markDetached(child);
      }
  },

  resetTextContent(instance) {
    updateTextProp(instance, "");
  },

  commitTextUpdate(textInstance, _oldText, newText) {
    textInstance.text = newText;
    if (textInstance.attached) emitOp(textInstance, { type: "replace-text", id: textInstance.id, text: newText });
  },

  commitUpdate(instance, _type, prevProps, nextProps) {
    const previousEvents = eventRegistrations(instance.id, prevProps);
    const nextEvents = eventRegistrations(instance.id, nextProps);
    instance.props = nextProps;
    emitPropUpdates(instance, prevProps, nextProps);
    emitStyleUpdates(instance, prevProps, nextProps);
    emitEventUpdates(instance, previousEvents, nextEvents);
  },

  hideInstance(instance) {
    instance.hidden = true;
    emitOp(instance, { type: "set-prop", id: instance.id, name: "visible", value: { kind: "bool", value: false } });
  },

  hideTextInstance(textInstance) {
    textInstance.hidden = true;
  },

  unhideInstance(instance) {
    instance.hidden = false;
    emitOp(instance, { type: "set-prop", id: instance.id, name: "visible", value: { kind: "bool", value: true } });
  },

  unhideTextInstance(textInstance) {
    textInstance.hidden = false;
  },

    clearContainer(container) {
      for (const child of [...container.roots]) {
        if (child.attached) {
          emitOp(container, { type: "dispose-subtree", id: child.id });
          unregisterSubtreeHandlers(container, child);
          markDetached(child);
        }
      }
      container.roots = [];
  },

  setCurrentUpdatePriority(newPriority) {
    currentUpdatePriority = newPriority;
  },
  getCurrentUpdatePriority() {
    return currentUpdatePriority;
  },
  resolveUpdatePriority() {
    return DefaultEventPriority;
  },
  resetFormInstance() {},
  requestPostPaintCallback(callback) {
    queueMicrotask(() => callback(performance.now()));
  },
  shouldAttemptEagerTransition() {
    return false;
  },
  trackSchedulerEvent() {},
  resolveEventType() {
    return null;
  },
  resolveEventTimeStamp() {
    return performance.now();
  },
  maySuspendCommit() {
    return false;
  },
  preloadInstance() {
    return true;
  },
  startSuspendingCommit() {},
  suspendInstance() {},
  waitForCommitToBeReady() {
    return null;
  },
};

const renderer = ReactReconciler(hawkHostConfig);

export function createHawkRootContainer(options: HawkReactRootConfig): HawkRootContainer {
  if (!options.id.trim()) {
    throw new Error("react.root.id-required: Hawk2UI React roots require a stable non-empty id.");
  }
  return {
    id: options.id,
    bridge: options.bridge ?? createGlobalSceneBridge(),
    committed: [],
    errors: [],
    roots: [],
    pendingOps: [],
    nextTextId: 0,
  };
}

export function createReconcilerRoot(container: HawkRootContainer, options: HawkReactRootConfig) {
  const onUncaughtError = options.onUncaughtError ?? recordRootError(container);
  const onCaughtError = options.onCaughtError ?? recordRootError(container);
  const onRecoverableError = options.onRecoverableError ?? recordRootError(container);
  return renderer.createContainer(
    container,
    ConcurrentRoot,
    null,
    options.strictMode ?? false,
    null,
    options.identifierPrefix ?? "",
    onUncaughtError,
    onCaughtError,
    onRecoverableError,
    () => {},
  );
}

export function updateReconcilerRoot(root: ReturnType<typeof createReconcilerRoot>, element: React.ReactNode): void {
  renderer.updateContainerSync(element, root, null, null);
  renderer.flushSyncWork();
  renderer.flushPassiveEffects();
}

function recordRootError(container: HawkRootContainer): HawkReactErrorHandler {
  return (error, info) => container.errors.push(errorWithComponentStack(error, info.componentStack));
}

function errorWithComponentStack(error: Error, componentStack: string | undefined): Error {
  if (!componentStack?.trim()) return error;
  const decorated = new Error(`${error.message}\nreact.error.component-stack:${componentStack}`);
  decorated.name = error.name;
  decorated.stack = error.stack ? `${error.stack}\nReact component stack:${componentStack}` : componentStack;
  return decorated;
}

function attachLocal(parent: HawkParent, child: HawkChild, before?: HawkChild): void {
  detachFromParent(child);
  const children = parentChildren(parent);
  const insertionIndex = before ? children.indexOf(before) : -1;
  child.parent = parent;
  if (insertionIndex >= 0) children.splice(insertionIndex, 0, child);
  else children.push(child);
}

function detachLocal(parent: HawkParent, child: HawkChild): void {
  const children = parentChildren(parent);
  const index = children.indexOf(child);
  if (index >= 0) children.splice(index, 1);
  if (child.parent === parent) child.parent = undefined;
}

function detachFromParent(child: HawkChild): void {
  if (child.parent) detachLocal(child.parent, child);
}

function parentChildren(parent: HawkParent): HawkChild[] {
  return "roots" in parent ? parent.roots : parent.children;
}

function emitAttach(parent: HawkParent, child: HawkChild, before: HawkChild | undefined): void {
  const parentId = parent.id;
  if (!child.attached) emitCreateSubtree(parent, child);
  if (before) emitOp(parent, { type: "insert-before", parent: parentId, child: child.id, before: before.id });
  else if (!("roots" in parent)) emitOp(parent, { type: "append-child", parent: parentId, child: child.id });
}

function emitCreateSubtree(parent: HawkParent, child: HawkChild): void {
  if (child.nodeType === "text") {
    emitOp(parent, { type: "create-text", id: child.id, text: child.text });
    child.attached = true;
    return;
  }
  emitOp(parent, { type: "create-node", id: child.id, kind: child.kind });
  emitInitialProps(child);
  emitInitialStyles(child);
  for (const event of eventRegistrations(child.id, child.props)) emitOp(child, event);
  child.attached = true;
  for (const nested of child.children) {
    emitCreateSubtree(child, nested);
    emitOp(child, { type: "append-child", parent: child.id, child: nested.id });
  }
}

function emitRemove(parent: HawkParent, child: HawkChild): void {
  if ("roots" in parent) return;
  emitOp(parent, { type: "remove-child", parent: parent.id, child: child.id });
}

function markDetached(child: HawkChild): void {
  child.attached = false;
  child.parent = undefined;
  if (child.nodeType === "instance") {
    for (const nested of child.children) markDetached(nested);
  }
}

function emitInitialProps(instance: HawkNativeInstance): void {
  for (const op of propSetOps(instance.id, {}, instance.props)) emitOp(instance, op);
  for (const op of controlledValueSetOps(instance.id, instance.kind, {}, instance.props)) emitOp(instance, op);
  for (const op of accessibilitySetOps(instance.id, {}, instance.props)) emitOp(instance, op);
  for (const op of focusSetOps(instance.id, {}, instance.props)) emitOp(instance, op);
  for (const op of measureSetOps(instance.id, {}, instance.props)) emitOp(instance, op);
}

function emitInitialStyles(instance: HawkNativeInstance): void {
  for (const op of styleSetOps(instance.id, {}, instance.props)) emitOp(instance, op);
}

function emitPropUpdates(instance: HawkNativeInstance, previous: HawkNativeProps, next: HawkNativeProps): void {
  for (const op of propSetOps(instance.id, previous, next)) emitOp(instance, op);
  for (const op of controlledValueSetOps(instance.id, instance.kind, previous, next)) emitOp(instance, op);
  for (const op of accessibilitySetOps(instance.id, previous, next)) emitOp(instance, op);
  for (const op of focusSetOps(instance.id, previous, next)) emitOp(instance, op);
  for (const op of measureSetOps(instance.id, previous, next)) emitOp(instance, op);
}

function emitStyleUpdates(instance: HawkNativeInstance, previous: HawkNativeProps, next: HawkNativeProps): void {
  for (const op of styleSetOps(instance.id, previous, next)) emitOp(instance, op);
}

function emitEventUpdates(
  instance: HawkNativeInstance,
  previous: readonly HawkSceneOp[],
  next: readonly HawkSceneOp[],
): void {
  const previousKeys = new Set(previous.map(eventKey));
  const nextKeys = new Set(next.map(eventKey));
  for (const op of previous) {
    if (!nextKeys.has(eventKey(op)) && op.type === "register-event") {
      emitOp(instance, { type: "unregister-event", id: op.id, event: op.event });
    }
  }
  for (const op of next) {
    if (!previousKeys.has(eventKey(op))) emitOp(instance, op);
  }
}

function updateTextProp(instance: HawkNativeInstance, text: string): void {
  emitOp(instance, { type: "set-prop", id: instance.id, name: "text", value: { kind: "string", value: text } });
}

function propSetOps(id: string, previous: HawkNativeProps, next: HawkNativeProps): HawkSceneOp[] {
  const ops: HawkSceneOp[] = [];
  const names = new Set([...Object.keys(previous), ...Object.keys(next)]);
  for (const name of [...names].sort()) {
    if (RESERVED_PROPS.has(name) || name === "style" || name === "text") continue;
    const previousValue = propValue(previous, name);
    const nextValue = propValue(next, name);
    const nextHasValue = Object.prototype.hasOwnProperty.call(next, name);
    if (Object.is(previousValue, nextValue) && nextHasValue) continue;
    if (UNSUPPORTED_DOM_PROPS.has(name)) {
      throw new Error(
        `react.dom.unsupported: prop \`${name}\` is DOM-only; use Hawk2UI native props or capability APIs instead.`,
      );
    }
    const value: HawkSceneValue = nextHasValue ? sceneValue(nextValue, `react.prop.unsupported: prop \`${name}\``) : { kind: "null" };
    ops.push({ type: "set-prop", id, name, value });
  }
  const text = textContent(next);
  if (text !== undefined && text !== textContent(previous)) {
    ops.push({ type: "set-prop", id, name: "text", value: { kind: "string", value: text } });
  }
  return ops;
}

function styleSetOps(id: string, previous: HawkNativeProps, next: HawkNativeProps): HawkSceneOp[] {
  const previousStyles = normalizedStyleProps(previous);
  const nextStyles = normalizedStyleProps(next);
  const ops: HawkSceneOp[] = [];
  const names = new Set([...Object.keys(previousStyles), ...Object.keys(nextStyles)]);
  for (const name of [...names].sort()) {
    const previousValue = previousStyles[name];
    const nextValue = nextStyles[name];
    const nextHasValue = Object.prototype.hasOwnProperty.call(nextStyles, name);
    if (Object.is(previousValue, nextValue) && nextHasValue) continue;
    const value: HawkSceneValue = nextHasValue
      ? sceneValue(nextValue, `react.style.unsupported: style \`${name}\``)
      : { kind: "null" };
    ops.push({ type: "set-style", id, name, value });
  }
  return ops;
}

function controlledValueSetOps(
  id: string,
  kind: HawkSceneNodeKind,
  previous: HawkNativeProps,
  next: HawkNativeProps,
): HawkSceneOp[] {
  if (kind !== "input") return [];
  const previousHasValue = Object.prototype.hasOwnProperty.call(previous, "value");
  const previousValue = previous.value;
  const nextHasValue = Object.prototype.hasOwnProperty.call(next, "value");
  const nextValue = next.value;
  if (!previousHasValue && !nextHasValue) return [];
  if (Object.is(previousValue, nextValue) && nextHasValue) return [];
  return [
    {
      type: "set-prop",
      id,
      name: "value",
      value: nextHasValue ? sceneValue(nextValue, "react.input.unsupported: value") : { kind: "null" },
    },
  ];
}

function accessibilitySetOps(id: string, previous: HawkNativeProps, next: HawkNativeProps): HawkSceneOp[] {
  const previousSemantics = accessibilitySemantics(previous);
  const nextSemantics = accessibilitySemantics(next);
  if (JSON.stringify(previousSemantics) === JSON.stringify(nextSemantics)) return [];
  return [{ type: "set-accessibility", id, ...nextSemantics }];
}

function focusSetOps(id: string, previous: HawkNativeProps, next: HawkNativeProps): HawkSceneOp[] {
  if (previous.autoFocus === true || next.autoFocus !== true) return [];
  return [{ type: "focus-node", id }];
}

function measureSetOps(id: string, previous: HawkNativeProps, next: HawkNativeProps): HawkSceneOp[] {
  const previousRequest = stringProp(previous.measure);
  const nextRequest = stringProp(next.measure);
  if (nextRequest === undefined || nextRequest === previousRequest) return [];
  return [{ type: "measure-node", id, request: nextRequest }];
}

function eventRegistrations(id: string, props: HawkNativeProps): HawkSceneOp[] {
  const ops: HawkSceneOp[] = [];
  for (const [propName, event] of EVENT_PROPS) {
    if (!(propName in props)) continue;
    ops.push({ type: "register-event", id, event, handler: handlerId(propName, props[propName]) });
  }
  return ops;
}

function eventKey(op: HawkSceneOp): string {
  return op.type === "register-event" ? `${op.id}:${op.event}:${op.handler}` : JSON.stringify(op);
}

function handlerId(propName: string, value: unknown): string {
  if (typeof value === "string" && value.trim()) return value;
  if (typeof value === "function" && value.name.trim()) return value.name;
  if (typeof value === "function") {
    const existing = functionHandlerIds.get(value);
    if (existing) return existing;
    const generated = `react.handler.${nextFunctionHandlerId++}`;
    functionHandlerIds.set(value, generated);
    return generated;
  }
  throw new Error(`react.event.handler-invalid: ${propName} requires a stable string handler id or named function.`);
}

function normalizedStyleProps(props: HawkNativeProps): Record<string, unknown> {
  const styles: Record<string, unknown> = {};
  const className = props.className ?? props.class;
  if (typeof className === "string" && className.trim()) styles.class = className;
  if (props.style !== undefined) {
    if (!props.style || typeof props.style !== "object" || Array.isArray(props.style)) {
      throw new Error("react.style.unsupported: style must be an object of scalar values.");
    }
    for (const [name, value] of Object.entries(props.style)) styles[name] = value;
  }
  return styles;
}

function propValue(props: HawkNativeProps, name: string): unknown {
  if (name === "text") return textContent(props);
  return props[name];
}

function textContent(props: HawkNativeProps): string | undefined {
  if (typeof props.text === "string" || typeof props.text === "number") return String(props.text);
  return isTextScalar(props.children) ? String(props.children) : undefined;
}

function accessibilitySemantics(props: HawkNativeProps): AccessibilitySemantics {
  const semantics: AccessibilitySemantics = {};
  const role = stringProp(props.role);
  const label = stringProp(props.label ?? props.ariaLabel ?? props["aria-label"]);
  const description = stringProp(props.description);
  if (role !== undefined) semantics.role = role;
  if (label !== undefined) semantics.label = label;
  if (description !== undefined) semantics.description = description;
  const value = props.value;
  if (value !== undefined) semantics.value = sceneValue(value, "react.accessibility.unsupported: value");
  if (typeof props.disabled === "boolean") semantics.disabled = props.disabled;
  if (typeof props.checked === "boolean") semantics.checked = props.checked;
  if (typeof props.pressed === "boolean") semantics.pressed = props.pressed;
  if (typeof props.selected === "boolean") semantics.checked = props.selected;
  if (typeof props.focused === "boolean") semantics.focused = props.focused;
  return semantics;
}

function stringProp(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value : undefined;
}

function sceneValue(value: unknown, diagnosticPrefix: string): HawkSceneValue {
  if (value === null) return { kind: "null" };
  if (typeof value === "boolean") return { kind: "bool", value };
  if (typeof value === "number" && Number.isFinite(value)) return { kind: "number", value };
  if (typeof value === "string") return { kind: "string", value };
  throw new Error(`${diagnosticPrefix} must be null, boolean, finite number, or string.`);
}

function kindForType(type: HawkHostType): HawkSceneNodeKind {
  switch (type) {
    case "hawk-view":
    case "view":
      return "view";
    case "hawk-text":
    case "text":
      return "text";
    case "hawk-button":
    case "button":
      return "button";
    case "hawk-input":
    case "input":
      return "input";
    case "hawk-image":
    case "image":
      return "image";
    case "hawk-vector":
    case "vector":
      return "vector";
    case "hawk-surface":
    case "hawk-custom-surface":
    case "custom-surface":
      return "custom-surface";
    case "hawk-scroll-view":
    case "scroll-view":
      return "scroll-view";
    case "hawk-list":
    case "list":
      return "list";
    default:
      if (VIEW_ALIASES.has(type)) return "view";
      if (TEXT_ALIASES.has(type)) return "text";
      throw new Error(`react.element.unsupported: unsupported Hawk2UI host element \`${type}\`.`);
  }
}

function requiredNodeId(type: HawkHostType, props: HawkNativeProps): string {
  if (typeof props.id === "string" && props.id.trim()) return props.id;
  throw new Error(`react.node.id-required: host element \`${type}\` requires a stable non-empty id prop.`);
}

function isTextScalar(value: unknown): value is string | number {
  return typeof value === "string" || typeof value === "number";
}

function emitOp(anchor: HawkParent | HawkChild, op: HawkSceneOp): void {
  const container = rootContainerFor(anchor);
  if (op.type === "register-event" && isNativeInstance(anchor) && anchor.id === op.id) {
    registerBridgeHandler(container, anchor, op);
  } else if (op.type === "unregister-event") {
    container.bridge.unregisterEventHandler?.(op.id, op.event);
  }
  container.pendingOps.push(op);
}

function rootContainerFor(anchor: HawkParent | HawkChild): HawkRootContainer {
  let cursor: HawkParent | HawkChild | undefined = anchor;
  while (cursor && !("roots" in cursor)) cursor = cursor.parent;
  if (!cursor) throw new Error("react.scene.root-detached: cannot emit scene ops for a detached node.");
  return cursor;
}

function flushPendingOps(container: HawkRootContainer): void {
  if (container.pendingOps.length === 0) return;
  const batch: HawkSceneOpBatch = { ops: [...container.pendingOps, { type: "commit" }] };
  container.pendingOps = [];
  container.bridge.commit(batch);
  container.committed.push({ ops: batch.ops.map((op) => ({ ...op })) });
}

function publicNodeHandle(instance: HawkChild): HawkNativeNodeHandle {
  if (instance.handle) return instance.handle;
  const kind = instance.nodeType === "instance" ? instance.kind : "text";
  const handle = Object.freeze({
    id: instance.id,
    kind,
    focus() {
      emitOp(instance, { type: "focus-node", id: instance.id });
      flushPendingOps(rootContainerFor(instance));
    },
    measure(request: string) {
      if (typeof request !== "string" || !request.trim()) {
        throw new Error("react.ref.measure.invalid: measure requires a stable non-empty request id.");
      }
      emitOp(instance, { type: "measure-node", id: instance.id, request });
      flushPendingOps(rootContainerFor(instance));
    },
  });
  instance.handle = handle;
  return handle;
}

function registerBridgeHandler(container: HawkRootContainer, instance: HawkNativeInstance, op: HawkSceneOp): void {
  if (op.type !== "register-event") return;
  const handler = eventHandlerForRegistration(instance, op);
  if (handler) {
    container.bridge.registerEventHandler?.(op.id, op.event, op.handler, (event) => {
      renderer.flushSyncFromReconciler(() => handler(eventPayloadFor(instance, event)));
      renderer.flushPassiveEffects();
    });
  }
}

function eventHandlerForRegistration(instance: HawkNativeInstance, op: Extract<HawkSceneOp, { type: "register-event" }>) {
  for (const [propName, event] of EVENT_PROPS) {
    if (event !== op.event || !(propName in instance.props)) continue;
    const value = instance.props[propName];
    if (handlerId(propName, value) === op.handler && typeof value === "function") {
      return value as (event: unknown) => void;
    }
  }
  return undefined;
}

function eventPayloadFor(instance: HawkNativeInstance, event: unknown): HawkNativeEventPayload {
  const payload = eventRecord(event);
  const target = eventTargetFor(instance, payload);
  return { ...payload, target, currentTarget: target };
}

function eventRecord(event: unknown): Record<string, unknown> {
  if (event && typeof event === "object" && !Array.isArray(event)) {
    return { ...(event as Record<string, unknown>) };
  }
  return { value: event };
}

function eventTargetFor(instance: HawkNativeInstance, payload: Record<string, unknown>): HawkNativeEventTarget {
  const target: Record<string, unknown> = { id: instance.id, kind: instance.kind };
  if (Object.prototype.hasOwnProperty.call(payload, "value")) target.value = payload.value;
  return target as HawkNativeEventTarget;
}

function unregisterSubtreeHandlers(container: HawkRootContainer, child: HawkChild): void {
  if (!isNativeInstance(child)) return;
  for (const [propName, event] of EVENT_PROPS) {
    if (propName in child.props) container.bridge.unregisterEventHandler?.(child.id, event);
  }
  for (const nested of child.children) unregisterSubtreeHandlers(container, nested);
}

function isNativeInstance(node: HawkParent | HawkChild): node is HawkNativeInstance {
  return "nodeType" in node && node.nodeType === "instance";
}
