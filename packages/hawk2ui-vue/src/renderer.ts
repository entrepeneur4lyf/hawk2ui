import { createRenderer, nextTick, type Component } from "vue";

import type {
  HawkVueApp,
  HawkVueAppOptions,
  HawkVueMountOptions,
  HawkVueNodeKind,
  HawkVueRoot,
  HawkVueSceneBridge,
  HawkVueSceneOpBatch,
} from "./nativeTypes";
import { createGlobalVueSceneBridge } from "./sceneBridge";

type VueNativeNode = VueNativeElement | VueNativeText;
type VueParent = VueNativeElement;

interface VueNativeElement {
  readonly nodeType: "element";
  readonly type: string;
  readonly kind: HawkVueNodeKind;
  readonly children: VueNativeNode[];
  readonly pending: PendingElementOperation[];
  readonly bridge?: HawkVueSceneBridge;
  parent: VueParent | undefined;
  id: string | undefined;
  created: boolean;
}

type PendingElementOperation = (bridge: HawkVueSceneBridge, id: string) => void;

interface VueNativeText {
  readonly nodeType: "text";
  readonly id: string;
  text: string;
  parent: VueParent | undefined;
  created: boolean;
}

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
const RESERVED_PROPS = new Set([
  "id",
  "key",
  "ref",
  "class",
  "className",
  "style",
  "role",
  "label",
  "description",
  "ariaLabel",
  "aria-label",
  "disabled",
  "checked",
  "pressed",
  "focused",
  "value",
  "placeholder",
  "autoFocus",
  "measure",
]);
const EVENT_PROPS = new Map<string, string>([
  ["onClick", "pointer.press"],
  ["onPointerPress", "pointer.press"],
  ["onPointerdown", "pointer.press"],
  ["onPointerDown", "pointer.press"],
  ["onPointerup", "pointer.release"],
  ["onPointerUp", "pointer.release"],
  ["onPointermove", "pointer.move"],
  ["onPointerMove", "pointer.move"],
  ["onPointerenter", "pointer.enter"],
  ["onPointerEnter", "pointer.enter"],
  ["onPointerleave", "pointer.leave"],
  ["onPointerLeave", "pointer.leave"],
  ["onWheel", "pointer.wheel"],
  ["onKeydown", "keyboard.key-down"],
  ["onKeyDown", "keyboard.key-down"],
  ["onKeyup", "keyboard.key-up"],
  ["onKeyUp", "keyboard.key-up"],
  ["onInput", "input.value-changed"],
  ["onChange", "input.value-committed"],
  ["onFocus", "focus.focus-in"],
  ["onBlur", "focus.focus-out"],
  ["onResize", "resize"],
]);

let nextTextId = 1;
let nextHandlerId = 1;

const renderer = createRenderer<VueNativeNode, VueNativeElement>({
  patchProp(element, key, previousValue, nextValue) {
    patchNativeProp(element, key, previousValue, nextValue);
  },
  insert(child, parent, anchor) {
    attachChild(parent, child, anchor);
  },
  remove(child) {
    removeChild(child);
  },
  createElement(type) {
    return {
      nodeType: "element",
      type,
      kind: kindForType(type),
      children: [],
      pending: [],
      parent: undefined,
      id: undefined,
      created: false,
    };
  },
  createText(text) {
    return {
      nodeType: "text",
      id: `vue:text:${nextTextId++}`,
      text,
      parent: undefined,
      created: false,
    };
  },
  createComment(text) {
    return {
      nodeType: "text",
      id: `vue:comment:${nextTextId++}`,
      text,
      parent: undefined,
      created: false,
    };
    },
    setText(node, text) {
      if (node.nodeType !== "text") {
        throw new Error("vue.text.internal: setText expected a text node.");
      }
      ensureTextCreated(node);
      node.text = text;
      if (node.parent) bridgeForParent(node.parent).replaceText(node.id, text);
    },
  setElementText(element, text) {
    withCreatedElement(element, (bridge, id) => bridge.setProp(id, "text", text));
  },
  parentNode(node) {
    return node.parent?.nodeType === "element" ? node.parent : null;
  },
  nextSibling(node) {
    const parent = node.parent;
    if (!parent) return null;
    const index = parent.children.indexOf(node);
    return index >= 0 ? parent.children[index + 1] ?? null : null;
  },
  querySelector() {
    return null;
  },
  setScopeId() {},
  cloneNode(node) {
    return node.nodeType === "text"
      ? { ...node, parent: undefined }
      : { ...node, children: [...node.children], pending: [...node.pending], parent: undefined };
  },
  insertStaticContent(content, parent, anchor) {
    const node: VueNativeText = {
      nodeType: "text",
      id: `vue:static:${nextTextId++}`,
      text: content,
      parent: undefined,
      created: false,
    };
    attachChild(parent, node, anchor);
    return [node, node];
  },
});

export function createHawkVueApp(rootComponent: Component, options: HawkVueAppOptions = {}): HawkVueApp {
  const bridge = options.bridge ?? (options.commit ? commitBridge(options.commit) : createGlobalVueSceneBridge());
  const app = renderer.createApp(rootComponent);
  let mountedRoot: HawkVueRootImpl | undefined;
  return {
    mount(target: string | HawkVueMountOptions = { rootId: "host" }): HawkVueRoot {
      const rootId = typeof target === "string" ? target : target.rootId ?? "host";
      const container = createRootContainer(rootId, bridge);
      app.mount(container);
      bridge.commit();
      mountedRoot = new HawkVueRootImpl(rootId, bridge);
      return mountedRoot;
    },
    unmount(): void {
      app.unmount();
      bridge.commit();
      mountedRoot = undefined;
    },
  };
}

export const createApp = createHawkVueApp;

class HawkVueRootImpl implements HawkVueRoot {
  constructor(
    readonly id: string,
    private readonly bridge: HawkVueSceneBridge,
  ) {}

  dispatch(nodeId: string, event: string, payload: unknown = {}): void {
    this.bridge.dispatch(nodeId, event, payload);
  }

  async flush(): Promise<void> {
    await nextTick();
    this.bridge.commit();
  }

  committedBatches(): readonly HawkVueSceneOpBatch[] {
    return this.bridge.batches();
  }

  drainCommittedBatches(): readonly HawkVueSceneOpBatch[] {
    return this.bridge.drain();
  }
}

function commitBridge(commit: NonNullable<HawkVueAppOptions["commit"]>): HawkVueSceneBridge {
  return createGlobalVueSceneBridge({
    __hawk2uiCommitScene(batch: { readonly ops: readonly unknown[] }) {
      commit(batch.ops as never);
    },
  });
}

function patchNativeProp(element: VueNativeElement, key: string, previousValue: unknown, nextValue: unknown): void {
  if (key === "id") {
    element.id = stringProp(nextValue);
    if (element.parent) ensureElementCreated(element);
    return;
  }
  if (key === "class" || key === "className") {
    if (nextValue !== undefined && nextValue !== null) {
      withCreatedElement(element, (bridge, id) => bridge.setStyle(id, "class", stringProp(nextValue)));
    }
    return;
  }
  if (key === "style") {
    patchStyle(element, nextValue);
    return;
  }
  const event = EVENT_PROPS.get(key);
  if (event) {
    patchEvent(element, event, previousValue, nextValue);
    return;
  }
  if (isAccessibilityProp(key)) {
    patchAccessibility(element);
    return;
  }
  if (RESERVED_PROPS.has(key)) {
    if (nextValue !== undefined && nextValue !== null) {
      withCreatedElement(element, (bridge, id) => bridge.setProp(id, key, nextValue));
    }
    return;
  }
  if (nextValue !== undefined && nextValue !== null) {
    withCreatedElement(element, (bridge, id) => bridge.setProp(id, key, nextValue));
  }
}

function patchStyle(element: VueNativeElement, nextValue: unknown): void {
  if (!nextValue || typeof nextValue !== "object" || Array.isArray(nextValue)) {
    withCreatedElement(element, (bridge, id) => bridge.setProp(id, "style", nextValue));
    return;
  }
  for (const [name, value] of Object.entries(nextValue)) {
    withCreatedElement(element, (bridge, id) => bridge.setStyle(id, name, value));
  }
}

function patchEvent(element: VueNativeElement, event: string, previousValue: unknown, nextValue: unknown): void {
  withCreatedElement(element, (bridge, id) => {
    if (previousValue) bridge.unregisterEventHandler(id, event);
    if (typeof nextValue === "function") {
      bridge.registerEventHandler(id, event, handlerId(nextValue), nextValue as (event: unknown) => void);
    } else if (typeof nextValue === "string") {
      bridge.registerEventHandler(id, event, nextValue, () => {});
    }
  });
}

function patchAccessibility(element: VueNativeElement): void {
  withCreatedElement(element, (bridge, id) => bridge.setAccessibility(id, {}));
}

function attachChild(parent: VueParent, child: VueNativeNode, anchor: VueNativeNode | null = null): void {
  const existing = parent.children.indexOf(child);
  if (existing >= 0) parent.children.splice(existing, 1);
  const anchorIndex = anchor ? parent.children.indexOf(anchor) : -1;
  if (anchorIndex >= 0) {
    parent.children.splice(anchorIndex, 0, child);
  } else {
    parent.children.push(child);
  }
  child.parent = parent;
  withCreatedParent(parent, (bridge, parentId) => {
    ensureNodeCreated(child);
    const childId = nodeId(child);
    if (anchor && anchorIndex >= 0) {
      bridge.insertBefore(parentId, childId, nodeId(anchor));
    } else {
      bridge.appendChild(parentId, childId);
    }
  });
}

function removeChild(child: VueNativeNode): void {
  const parent = child.parent;
  if (!parent) return;
  const index = parent.children.indexOf(child);
  if (index >= 0) parent.children.splice(index, 1);
  withCreatedParent(parent, (bridge, parentId) => bridge.removeChild(parentId, nodeId(child)));
  child.parent = undefined;
}

function ensureNodeCreated(node: VueNativeNode): void {
  if (node.nodeType === "text") {
    ensureTextCreated(node);
  } else {
    ensureElementCreated(node);
  }
}

function ensureElementCreated(element: VueNativeElement): void {
  if (element.created) return;
  const parent = element.parent;
  if (!parent) return;
  const bridge = bridgeForParent(parent);
  bridge.createNode(element.kind, requiredElementId(element));
  element.created = true;
  const pending = element.pending.splice(0);
  for (const operation of pending) operation(bridge, requiredElementId(element));
}

function ensureTextCreated(node: VueNativeText): void {
  if (node.created) return;
  const parent = node.parent;
  if (!parent) return;
  bridgeForParent(parent).createText(node.id, node.text);
  node.created = true;
}

function bridgeForElement(element: VueNativeElement): HawkVueSceneBridge {
  if (element.bridge) return element.bridge;
  const parent = element.parent;
  if (!parent) {
    throw new Error("vue.node.unmounted: Vue native element is not attached to a Hawk2UI root.");
  }
  return bridgeForParent(parent);
}

function withCreatedElement(element: VueNativeElement, operation: PendingElementOperation): void {
  if (element.bridge) {
    operation(element.bridge, requiredElementId(element));
    return;
  }
  if (!element.parent) {
    element.pending.push(operation);
    return;
  }
  ensureElementCreated(element);
  operation(bridgeForElement(element), requiredElementId(element));
}

function withCreatedParent(parent: VueParent, operation: PendingElementOperation): void {
  withCreatedElement(parent, operation);
}

function bridgeForParent(parent: VueParent): HawkVueSceneBridge {
  return bridgeForElement(parent);
}

function createRootContainer(id: string, bridge: HawkVueSceneBridge): VueNativeElement {
  return {
    nodeType: "element",
    type: "hawk-root",
    kind: "view",
    children: [],
    pending: [],
    bridge,
    parent: undefined,
    id,
    created: true,
  };
}

export type { HawkVueApp, HawkVueMountOptions, HawkVueRoot } from "./nativeTypes";

function requiredElementId(element: VueNativeElement): string {
  if (element.id?.trim()) return element.id;
  throw new Error(`vue.attribute.required: ${element.type} requires a stable id attribute.`);
}

function nodeId(node: VueNativeNode): string {
  return node.nodeType === "text" ? node.id : requiredElementId(node);
}

function kindForType(type: string): HawkVueNodeKind {
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
      throw new Error(`vue.element.unsupported: unsupported Hawk2UI host element \`${type}\`.`);
  }
}

function stringProp(value: unknown): string {
  return typeof value === "string" ? value : String(value ?? "");
}

function handlerId(handler: Function): string {
  return handler.name || `vue-handler-${nextHandlerId++}`;
}

function isAccessibilityProp(key: string): boolean {
  return key === "role" || key === "label" || key === "description" || key === "ariaLabel" || key === "aria-label";
}
