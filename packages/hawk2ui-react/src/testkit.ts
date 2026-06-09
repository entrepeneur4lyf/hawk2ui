import {
  recordsForApp,
  type HawkElementSpec,
  type HawkEventSpec,
  type HawkLifecycleSpec,
} from "@hawk2ui/native";

export interface HawkReactRoot {
  readonly records: readonly string[];
  readonly snapshot: HawkElementSpec | undefined;
  readonly render: (element: unknown) => void;
  readonly unmount: () => void;
}

const REACT_RUNTIME_EVENTS: ReadonlyArray<readonly [string, HawkEventSpec["kind"]]> = [
  ["onClick", "pointer.press"],
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
];

const REACT_RUNTIME_LIFECYCLE: ReadonlyArray<readonly [string, HawkLifecycleSpec["phase"]]> = [
  ["onMount", "mounted"],
  ["onSuspend", "suspended"],
  ["onResume", "resumed"],
  ["onHotReload", "hot-reloaded"],
  ["onErrorBoundary", "error-boundary"],
  ["onShutdown", "shutdown"],
  ["onUnmount", "unmounted"],
];

const VIEW_ELEMENT_TAGS = new Set([
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
const TEXT_ELEMENT_TAGS = new Set([
  "span",
  "p",
  "strong",
  "em",
  "small",
  "code",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
]);

export function createHawkReactRoot(target: { readonly id: string }): HawkReactRoot {
  if (!target.id.trim()) {
    throw new Error("Hawk2UI React roots require a stable target id.");
  }
  const records: string[] = [];
  let current: HawkElementSpec | undefined;
  return {
    get records() {
      return records;
    },
    get snapshot() {
      return current;
    },
    render: (element: unknown) => {
      const next = elementToNativeSpec(element, target.id);
      validateUniqueChildKeys(next);
      if (!current) {
        records.push(...recordsForApp({ name: `react:${target.id}`, root: next }));
      } else {
        records.push(...diffRecords(current, next));
      }
      current = next;
    },
    unmount: () => {
      if (current) {
        records.push(`unmount-element:${current.id}`);
        current = undefined;
      }
    },
  };
}

function elementToNativeSpec(element: unknown, fallbackId: string): HawkElementSpec {
  return runtimeElementSpec(element, fallbackId, "view");
}

function runtimeElementSpec(
  element: unknown,
  fallbackId: string,
  fallbackKind: HawkElementSpec["kind"],
): HawkElementSpec {
  const record = readRecord(element);
  const props = readProps(element);
  const id = readString(props, "id") ?? readString(record, "id") ?? fallbackId;
  const text = readTextProp(element);
  return {
    id,
    kind: runtimeKind(record, fallbackKind),
    refs: readString(props, "ref") ? [readString(props, "ref") as string] : [],
    styleRefs: readString(props, "className") ? [readString(props, "className") as string] : [],
    assetRefs: readString(props, "data-asset")
      ? [{ name: "react.asset", path: readString(props, "data-asset") as string }]
      : [],
    events: runtimeReactEvents(props),
    lifecycle: runtimeReactLifecycle(props),
    ...(text ? { props: text } : {}),
    children: readChildren(element, props).map(runtimeChildSpec),
  };
}

function runtimeChildSpec(child: Record<string, unknown>, index: number): HawkElementSpec {
  const props = readProps(child);
  const id = readString(props, "id") ?? readString(child, "id") ?? `child-${index}`;
  const key = readString(child, "key") ?? readString(props, "id") ?? readString(child, "id");
  const spec = runtimeElementSpec(child, id, "text");
  return key ? { ...spec, key } : spec;
}

function runtimeKind(
  record: Record<string, unknown> | undefined,
  fallback: HawkElementSpec["kind"],
): HawkElementSpec["kind"] {
  const tag = readString(record, "type");
  return tag && isRuntimeTag(tag) ? kindForRuntimeTag(tag) : fallback;
}

function kindForRuntimeTag(tag: string): HawkElementSpec["kind"] {
  if (tag === "hawk-view") return "view";
  if (tag === "hawk-text") return "text";
  if (tag === "hawk-button") return "button";
  if (tag === "hawk-surface" || tag === "hawk-custom-surface") return "custom-surface";
  if (VIEW_ELEMENT_TAGS.has(tag)) return "view";
  if (TEXT_ELEMENT_TAGS.has(tag)) return "text";
  if (tag === "button") return "button";
  throw new Error(`react.element.unsupported: unsupported Hawk element \`${tag}\`.`);
}

function isRuntimeTag(tag: string): boolean {
  return tag.startsWith("hawk-") || VIEW_ELEMENT_TAGS.has(tag) || TEXT_ELEMENT_TAGS.has(tag) || tag === "button";
}

function runtimeReactEvents(props: Record<string, unknown> | undefined): readonly HawkEventSpec[] {
  if (!props) return [];
  const events: HawkEventSpec[] = [];
  for (const [attribute, kind] of REACT_RUNTIME_EVENTS) {
    if (attribute in props) {
      events.push({ kind, handler: runtimeHandlerName("react", attribute, props[attribute]) });
    }
  }
  return events;
}

function runtimeReactLifecycle(props: Record<string, unknown> | undefined): readonly HawkLifecycleSpec[] {
  if (!props) return [];
  const lifecycle: HawkLifecycleSpec[] = [];
  for (const [attribute, phase] of REACT_RUNTIME_LIFECYCLE) {
    if (attribute in props) {
      lifecycle.push({ phase, handler: runtimeHandlerName("react", attribute, props[attribute]) });
    }
  }
  return lifecycle;
}

function runtimeHandlerName(framework: string, attribute: string, value: unknown): string {
  if (typeof value === "string" && value.trim()) return value;
  if (typeof value === "function" && value.name.trim()) return value.name;
  throw new Error(`${framework}.handler.unsupported: runtime handler \`${attribute}\` must be a stable string or named function.`);
}

function readRecord(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === "object" ? (value as Record<string, unknown>) : undefined;
}

function readProps(element: unknown): Record<string, unknown> | undefined {
  if (!element || typeof element !== "object") return undefined;
  const props = "props" in element ? (element as { readonly props?: unknown }).props : element;
  return props && typeof props === "object" ? (props as Record<string, unknown>) : undefined;
}

function readChildren(
  element: unknown,
  props: Record<string, unknown> | undefined,
): readonly Record<string, unknown>[] {
  const children = props?.children ?? (element && typeof element === "object" && "children" in element
    ? (element as { readonly children?: unknown }).children
    : undefined);
  return Array.isArray(children)
    ? children.filter((child): child is Record<string, unknown> => Boolean(child) && typeof child === "object")
    : [];
}

function readString(record: Record<string, unknown> | undefined, name: string): string | undefined {
  const value = record?.[name];
  return typeof value === "string" && value.trim() ? value : undefined;
}

function readTextProp(element: unknown): Record<string, string> | undefined {
  const text = readString(readProps(element), "text");
  return text ? { text } : undefined;
}

function validateUniqueChildKeys(element: HawkElementSpec): void {
  const keys = new Set<string>();
  for (const child of element.children ?? []) {
    if (child.key) {
      if (keys.has(child.key)) {
        throw new Error(`react.child-key.duplicate: duplicate React child key \`${child.key}\``);
      }
      keys.add(child.key);
    }
    validateUniqueChildKeys(child);
  }
}

function diffRecords(previous: HawkElementSpec, next: HawkElementSpec): readonly string[] {
  const records: string[] = [];
  if (previous.id !== next.id) {
    records.push(`remove-element:${previous.id}`);
    records.push(...recordsForApp({ name: `react:${next.id}`, root: next }));
    return records;
  }
  if ((previous.styleRefs ?? []).join(" ") !== (next.styleRefs ?? []).join(" ")) {
    for (const style of next.styleRefs ?? []) {
      records.push(`style:${next.id}:${style}`);
    }
  }
  emitPropDiffs(previous, next, records);
  emitChildDiffs(previous, next, records);
  return records;
}

function emitPropDiffs(previous: HawkElementSpec, next: HawkElementSpec, records: string[]): void {
  const names = new Set([...Object.keys(previous.props ?? {}), ...Object.keys(next.props ?? {})]);
  for (const name of [...names].sort()) {
    const previousValue = previous.props?.[name];
    const nextValue = next.props?.[name];
    if (previousValue !== nextValue && nextValue !== undefined) {
      records.push(`prop:${next.id}:${name}=${String(nextValue)}`);
    }
  }
}

function emitChildDiffs(previous: HawkElementSpec, next: HawkElementSpec, records: string[]): void {
  const previousChildren = new Map((previous.children ?? []).map((child) => [child.key ?? child.id, child]));
  const nextChildren = new Map((next.children ?? []).map((child) => [child.key ?? child.id, child]));
  for (const [key, child] of nextChildren) {
    const previousChild = previousChildren.get(key);
    if (!previousChild) {
      records.push(...recordsForApp({ name: `react:${child.id}`, root: child }));
    } else {
      records.push(...diffRecords(previousChild, child));
    }
  }
  for (const [key, child] of previousChildren) {
    if (!nextChildren.has(key)) {
      records.push(`remove-element:${child.id}`);
    }
  }
}
