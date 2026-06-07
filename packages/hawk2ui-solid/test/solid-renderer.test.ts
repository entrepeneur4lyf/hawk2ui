import { expect, test } from "bun:test";
import { compileHawkSolid } from "../src/index.ts";
import { renderHawkSolid } from "../src/testkit.ts";

test("Solid compiler emits versioned native compiler artifacts from TSX", () => {
  const output = compileHawkSolid({
    filename: "App.tsx",
    source:
      'export function App() { const [status, setStatus] = createSignal("idle"); function handlePress() { setStatus("pressed"); } function onMount() { setStatus("mounted"); } function onCleanup() { setStatus("unmounted"); } const [items] = createSignal([{ id: "title" }, { id: "cta" }]); return <hawk-view id="root" ref={root_ref} class="surface.card" data-asset="assets/logo.svg" onPointerDown={handlePress} onMount={onMount} onCleanup={onCleanup}><For each={items()}>{(item) => <hawk-text id={item.id}>{item.id}</hawk-text>}</For></hawk-view>; }',
  });

  expect(output.compilerArtifact.schema_version).toBe(1);
  expect(output.compilerArtifact.compiler).toEqual({
    framework: "solid",
    compiler: "@hawk2ui/solid",
    source_path: "App.tsx",
    entrypoint: "App",
  });
  expect(output.compilerArtifact.root.id).toBe("root");
  expect(output.compilerArtifact.root.style_refs).toEqual(["surface.card"]);
  expect(output.compilerArtifact.root.children).toEqual([]);
  expect(output.compilerArtifact.list_templates.map((template) => template.id)).toEqual(["root:items"]);
  expect(output.compilerArtifact.reactivity).toEqual([
    { kind: "signal", name: "status" },
    { kind: "signal", name: "items" },
    { kind: "keyed-for-each", name: "items" },
    { kind: "effect", name: "root-props" },
  ]);
});

test("Solid compiler preserves dynamic text bindings from signal expressions", () => {
  const output = compileHawkSolid({
    filename: "App.tsx",
    source:
      'const [label] = createSignal("Title"); export function App() { return <hawk-view id="root"><hawk-text id="title">{label()}</hawk-text></hawk-view>; }',
  });

  expect(output.compilerArtifact.dynamic_bindings).toEqual([
    {
      node_id: "title",
      target: { type: "prop", name: "text" },
      expression: "label()",
      dependencies: ["label"],
    },
  ]);
  expect(output.compilerArtifact.initial_dynamic_values).toEqual([
    {
      name: "label",
      mode: "getter",
      value: { type: "string", value: "Title" },
    },
  ]);
});

test("Solid compiler emits executable pointer handler actions", () => {
  const output = compileHawkSolid({
    filename: "App.tsx",
    source:
      'const [label, setLabel] = createSignal("Idle"); function handlePress() { setLabel("Pressed"); } export function App() { return <hawk-view id="root" onPointerDown={handlePress}><hawk-text id="title">{label()}</hawk-text></hawk-view>; }',
  });

  expect(output.compilerArtifact.event_handlers).toEqual([
    {
      name: "handlePress",
      actions: [
        {
          type: "set_dynamic_value",
          name: "label",
          value: { type: "string", value: "Pressed" },
        },
      ],
    },
  ]);
});

test("Solid compiler preserves event payload handler expressions", () => {
  const output = compileHawkSolid({
    filename: "App.tsx",
    source:
      'const [label, setLabel] = createSignal("Idle"); function handlePress(event: { x: number; y: number }) { setLabel(event.x + ":" + event.y); } export function App() { return <hawk-view id="root" onPointerDown={handlePress}><hawk-text id="title">{label()}</hawk-text></hawk-view>; }',
  });

  expect(output.compilerArtifact.event_handlers).toEqual([
    {
      name: "handlePress",
      actions: [
        {
          type: "set_dynamic_expression",
          name: "label",
          expression: "event.x + \":\" + event.y",
          dependencies: ["event"],
        },
      ],
    },
  ]);
});

test("Solid compiler expands local components with props and forwarded children", () => {
  const output = compileHawkSolid({
    filename: "App.tsx",
    source:
      'function Card(props: { id: string; title: string; children?: unknown }) { return <hawk-view id={props.id} class="card"><hawk-text id="card-title">{props.title}</hawk-text>{props.children}</hawk-view>; } export function App() { return <hawk-view id="root"><Card id="panel" title="Panel"><hawk-button id="cta">Go</hawk-button></Card></hawk-view>; }',
  });

  expect(output.compilerArtifact.root.children).toHaveLength(1);
  const card = output.compilerArtifact.root.children[0].node;
  expect(card.id).toBe("panel");
  expect(card.kind).toBe("view");
  expect(card.style_refs).toEqual(["card"]);
  expect(card.children.map((child) => child.node.id)).toEqual(["card-title", "cta"]);
  expect(card.children[0].node.props).toEqual([{ name: "text", value: { type: "string", value: "Panel" } }]);
  expect(card.children[1].node.props).toEqual([{ name: "text", value: { type: "string", value: "Go" } }]);
});

test("Solid compiler lowers the complete native event and lifecycle contract from JSX props", () => {
  const output = compileHawkSolid({
    filename: "App.tsx",
    source:
      'let status = "idle"; function markPointerPress() { status = "pointer.press"; } function markPointerRelease() { status = "pointer.release"; } function markPointerMove() { status = "pointer.move"; } function markPointerDrag() { status = "pointer.drag"; } function markPointerEnter() { status = "pointer.enter"; } function markPointerLeave() { status = "pointer.leave"; } function markPointerWheel() { status = "pointer.wheel"; } function markKeyDown() { status = "keyboard.key-down"; } function markKeyUp() { status = "keyboard.key-up"; } function markTextInput() { status = "keyboard.text-input"; } function markFocusIn() { status = "focus.focus-in"; } function markFocusOut() { status = "focus.focus-out"; } function markValueChanged() { status = "input.value-changed"; } function markValueCommitted() { status = "input.value-committed"; } function markResize() { status = "resize"; } function markMounted() { status = "mounted"; } function markSuspended() { status = "suspended"; } function markResumed() { status = "resumed"; } function markHotReloaded() { status = "hot-reloaded"; } function markErrorBoundary() { status = "error-boundary"; } function markShutdown() { status = "shutdown"; } function markUnmounted() { status = "unmounted"; } export function App() { return <hawk-view id="root" onPointerDown={markPointerPress} onPointerUp={markPointerRelease} onPointerMove={markPointerMove} onPointerDrag={markPointerDrag} onPointerEnter={markPointerEnter} onPointerLeave={markPointerLeave} onWheel={markPointerWheel} onKeyDown={markKeyDown} onKeyUp={markKeyUp} onTextInput={markTextInput} onFocus={markFocusIn} onBlur={markFocusOut} onInput={markValueChanged} onChange={markValueCommitted} onResize={markResize} onMount={markMounted} onSuspend={markSuspended} onResume={markResumed} onHotReload={markHotReloaded} onErrorBoundary={markErrorBoundary} onShutdown={markShutdown} onCleanup={markUnmounted}></hawk-view>; }',
  });

  expect(output.compilerArtifact.root.events.map((event) => event.kind)).toEqual([
    "pointer.press",
    "pointer.release",
    "pointer.move",
    "pointer.drag",
    "pointer.enter",
    "pointer.leave",
    "pointer.wheel",
    "keyboard.key-down",
    "keyboard.key-up",
    "keyboard.text-input",
    "focus.focus-in",
    "focus.focus-out",
    "input.value-changed",
    "input.value-committed",
    "resize",
  ]);
  expect(output.compilerArtifact.root.lifecycle.map((lifecycle) => lifecycle.event)).toEqual([
    "mounted",
    "suspended",
    "resumed",
    "hot-reloaded",
    "error-boundary",
    "shutdown",
    "unmounted",
  ]);
});

test("Solid compiler lowers onMount and onCleanup API calls to root lifecycle", () => {
  const output = compileHawkSolid({
    filename: "App.tsx",
    source:
      'function mounted() {} function destroyed() {} export function App() { onMount(mounted); onCleanup(destroyed); return <hawk-view id="root"></hawk-view>; }',
  });

  expect(output.compilerArtifact.root.lifecycle).toContainEqual({ event: "mounted", handler: "mounted" });
  expect(output.compilerArtifact.root.lifecycle).toContainEqual({ event: "unmounted", handler: "destroyed" });
});

test("Solid compiler preserves createMemo literal initial dynamic values", () => {
  const output = compileHawkSolid({
    filename: "App.tsx",
    source:
      'const label = createMemo(() => "Ready"); export function App() { return <hawk-text id="root">{label()}</hawk-text>; }',
  });

  expect(output.compilerArtifact.initial_dynamic_values).toContainEqual({
    name: "label",
    mode: "getter",
    value: { type: "string", value: "Ready" },
  });
});

test("Solid compiler preserves createEffect calls as effect reactivity bindings", () => {
  const output = compileHawkSolid({
    filename: "App.tsx",
    source:
      'const [label] = createSignal("Ready"); function syncLabel() { label(); } export function App() { createEffect(syncLabel); return <hawk-text id="root">{label()}</hawk-text>; }',
  });

  expect(output.compilerArtifact.reactivity).toContainEqual({
    kind: "effect",
    name: "createEffect:syncLabel",
  });
});

test("Solid compiler emits runtime list templates for dynamic For sources", () => {
  const output = compileHawkSolid({
    filename: "App.tsx",
    source:
      'const [items, setItems] = createSignal([{ id: "alpha", label: "Alpha" }, { id: "beta", label: "Beta" }]); function handleItemPress() { setItems([{ id: "gamma", label: "Gamma" }]); } export function App() { return <hawk-view id="root"><For each={items()}>{(item) => <hawk-text id={item.id} onPointerDown={handleItemPress}>{item.label}</hawk-text>}</For></hawk-view>; }',
  });

  expect(output.compilerArtifact.root.children).toEqual([]);
  expect(output.compilerArtifact.list_templates).toEqual([
    {
      id: "root:items",
      parent_id: "root",
      source: "items",
      item: "item",
      key: "item.id",
      node: {
        id: { type: "expression", expression: "item.id" },
        kind: "text",
        key: { type: "expression", expression: "item.id" },
        props: [{ name: "text", value: { type: "expression", expression: "item.label" } }],
        refs: [],
        style_refs: [],
        asset_refs: [],
          events: [{ kind: "pointer.press", handler: "handleItemPress", payload_fields: ["position"] }],
          lifecycle: [],
          children: [],
        },
      },
    ]);
  expect(output.compilerArtifact.event_handlers.map((handler) => handler.name)).toEqual(["handleItemPress"]);
});

test("Solid compiler emits runtime list templates for dynamic Index sources", () => {
  const output = compileHawkSolid({
    filename: "App.tsx",
    source:
      'const [items] = createSignal([{ id: "alpha", label: "Alpha" }]); export function App() { return <hawk-view id="root"><Index each={items()}>{(item) => <hawk-text id={item().id}>{item().label}</hawk-text>}</Index></hawk-view>; }',
  });

  expect(output.compilerArtifact.root.children).toEqual([]);
  expect(output.compilerArtifact.list_templates).toEqual([
    {
      id: "root:items",
      parent_id: "root",
      source: "items",
      item: "item",
      key: "item.id",
      node: {
        id: { type: "expression", expression: "item.id" },
        kind: "text",
        key: { type: "expression", expression: "item.id" },
        props: [{ name: "text", value: { type: "expression", expression: "item.label" } }],
        refs: [],
        style_refs: [],
        asset_refs: [],
        events: [],
        lifecycle: [],
        children: [],
      },
    },
    ]);
  });

test("Solid compiler anchors runtime list templates before the next static sibling", () => {
  const output = compileHawkSolid({
    filename: "App.tsx",
    source:
      'const [items] = createSignal([{ id: "alpha", label: "Alpha" }]); export function App() { return <hawk-view id="root"><hawk-text id="header">Header</hawk-text><For each={items()}>{(item) => <hawk-text id={item.id + "-row"}>{item.label + "!"}</hawk-text>}</For><hawk-text id="footer">Footer</hawk-text></hawk-view>; }',
  });

  expect(output.compilerArtifact.root.children.map((child) => child.node.id)).toEqual(["header", "footer"]);
  expect(output.compilerArtifact.list_templates[0]?.anchor_before).toBe("footer");
  expect(output.compilerArtifact.list_templates[0]?.key).toBe('item.id + "-row"');
  expect(output.compilerArtifact.list_templates[0]?.node.id).toEqual({
    type: "expression",
    expression: 'item.id + "-row"',
  });
  expect(output.compilerArtifact.list_templates[0]?.node.props).toEqual([
    { name: "text", value: { type: "expression", expression: 'item.label + "!"' } },
  ]);
});

test("Solid compiler preserves dynamic layout prop bindings from signal expressions", () => {
  const output = compileHawkSolid({
    filename: "App.tsx",
    source:
      'const [panelWidth] = createSignal(240); const [panelHeight] = createSignal(120); export function App() { return <hawk-view id="root"><hawk-view id="panel" width={panelWidth()} height={panelHeight()}></hawk-view></hawk-view>; }',
  });

  expect(output.compilerArtifact.dynamic_bindings).toEqual([
    {
      node_id: "panel",
      target: { type: "prop", name: "width" },
      expression: "panelWidth()",
      dependencies: ["panelWidth"],
    },
    {
      node_id: "panel",
      target: { type: "prop", name: "height" },
      expression: "panelHeight()",
      dependencies: ["panelHeight"],
    },
  ]);
});

test("Solid compiler preserves dynamic visual prop bindings from signal expressions", () => {
  const output = compileHawkSolid({
    filename: "App.tsx",
    source:
      'const [panelBackground] = createSignal("#111111"); const [titleSize] = createSignal(18); const [titleColor] = createSignal("#ffffff"); export function App() { return <hawk-view id="root"><hawk-view id="panel" background={panelBackground()}></hawk-view><hawk-text id="title" font_size={titleSize()} color={titleColor()}>Title</hawk-text></hawk-view>; }',
  });

  expect(output.compilerArtifact.dynamic_bindings).toEqual([
    {
      node_id: "panel",
      target: { type: "prop", name: "background" },
      expression: "panelBackground()",
      dependencies: ["panelBackground"],
    },
    {
      node_id: "title",
      target: { type: "prop", name: "font_size" },
      expression: "titleSize()",
      dependencies: ["titleSize"],
    },
    {
      node_id: "title",
      target: { type: "prop", name: "color" },
      expression: "titleColor()",
      dependencies: ["titleColor"],
    },
  ]);
});

test("Solid compiler passes through arbitrary scalar props and dynamic prop bindings", () => {
  const output = compileHawkSolid({
    filename: "App.tsx",
    source:
      'const [role] = createSignal(getRole()); export function App() { return <hawk-view id="root"><hawk-button id="cta" aria_label="Start" tab_index={2} selected={true} data_role={role()}>Go</hawk-button></hawk-view>; }',
  });
  const button = output.compilerArtifact.root.children[0].node;

  expect(button.props).toContainEqual({ name: "aria_label", value: { type: "string", value: "Start" } });
  expect(button.props).toContainEqual({ name: "tab_index", value: { type: "number", value: 2 } });
  expect(button.props).toContainEqual({ name: "selected", value: { type: "bool", value: true } });
  expect(output.compilerArtifact.dynamic_bindings).toContainEqual({
    node_id: "cta",
    target: { type: "prop", name: "data_role" },
    expression: "role()",
    dependencies: ["role"],
  });
});

test("Solid compiler maps onClick to pointer press", () => {
  const output = compileHawkSolid({
    filename: "App.tsx",
    source:
      'const [label, setLabel] = createSignal("Idle"); function handleClick() { setLabel("Clicked"); } export function App() { return <hawk-view id="root"><hawk-button id="cta" onClick={handleClick}>Go</hawk-button></hawk-view>; }',
  });

  expect(output.compilerArtifact.root.children[0].node.events).toEqual([
    { kind: "pointer.press", handler: "handleClick", payload_fields: ["position"] },
  ]);
});

test("Solid compiler lowers hawk-surface to a custom surface node", () => {
  const output = compileHawkSolid({
    filename: "App.tsx",
    source:
      'export function App() { return <hawk-view id="root"><hawk-surface id="meter" surface_id="level-meter" /></hawk-view>; }',
  });

  const surface = output.compilerArtifact.root.children[0].node;
  expect(surface.kind).toBe("custom-surface");
  expect(surface.props).toContainEqual({ name: "surface_id", value: { type: "string", value: "level-meter" } });
});

test("Solid compiler lowers Show conditionals to visible dynamic bindings", () => {
  const output = compileHawkSolid({
    filename: "App.tsx",
    source:
      'export function App() { const [showTitle] = createSignal(true); return <hawk-view id="root"><Show when={showTitle()}><hawk-text id="title">Title</hawk-text></Show></hawk-view>; }',
  });

  expect(output.compilerArtifact.root.children[0].node.id).toBe("title");
  expect(output.compilerArtifact.dynamic_bindings).toContainEqual({
    node_id: "title",
    target: { type: "prop", name: "visible" },
    expression: "showTitle()",
    dependencies: ["showTitle"],
  });
});

test("Solid compiler lowers Switch Match branches to visible dynamic bindings", () => {
  const output = compileHawkSolid({
    filename: "App.tsx",
    source:
      'export function App() { const [mode] = createSignal("ready"); return <hawk-view id="root"><Switch><Match when={mode() === "ready"}><hawk-text id="ready">Ready</hawk-text></Match><Match when={mode() === "idle"}><hawk-text id="idle">Idle</hawk-text></Match></Switch></hawk-view>; }',
  });

  expect(output.compilerArtifact.root.children.map((child) => child.node.id)).toEqual(["ready", "idle"]);
  expect(output.compilerArtifact.dynamic_bindings).toContainEqual({
    node_id: "ready",
    target: { type: "prop", name: "visible" },
    expression: 'mode() === "ready"',
    dependencies: ["mode"],
  });
  expect(output.compilerArtifact.dynamic_bindings).toContainEqual({
    node_id: "idle",
    target: { type: "prop", name: "visible" },
    expression: 'mode() === "idle"',
    dependencies: ["mode"],
  });
});

test("Solid compiler accepts standard element aliases for native nodes", () => {
  const output = compileHawkSolid({
    filename: "App.tsx",
    source:
      'export function App() { return <div id="root"><button id="cta">Go</button><span id="copy">Copy</span></div>; }',
  });

  expect(output.compilerArtifact.root.kind).toBe("view");
  expect(output.compilerArtifact.root.children.map((child) => [child.node.id, child.node.kind])).toEqual([
    ["cta", "button"],
    ["copy", "text"],
  ]);
});

test("Solid renderer records fine-grained updates, removals, and dispose", () => {
  let component = {
    id: "root",
    ref: "root_ref",
    class: "surface.card",
    asset: "assets/logo.svg",
    on: ["pointer.press", "keyboard.key-down"],
    children: [
      { id: "title", key: "title", text: "Title" },
      { id: "cta", kind: "button", key: "cta", text: "Go", on: ["pointer.release"] },
    ],
  };
  const disposer = renderHawkSolid(() => component, { target: { id: "host" } });

  expect(disposer.records).toEqual([
    "mount-element:root",
    "ref:root:root_ref",
    "style:root:surface.card",
    "asset:root:assets/logo.svg",
    "bind-event:root:pointer.press",
    "bind-event:root:keyboard.key-down",
    "mount-element:title",
    "prop:title:text=Title",
    "mount-element:cta",
    "bind-event:cta:pointer.release",
    "prop:cta:text=Go",
  ]);
  expect(disposer.snapshot.events?.map((event) => event.kind)).toEqual([
    "pointer.press",
    "keyboard.key-down",
  ]);
  expect(disposer.snapshot.children?.[1]?.kind).toBe("button");
  expect(disposer.snapshot.children?.[1]?.events?.[0]?.kind).toBe("pointer.release");

  component = {
    id: "root",
    ref: "root_ref",
    class: "surface.card emphasis",
    asset: "assets/logo.svg",
    on: ["pointer.press", "keyboard.key-down"],
    children: [{ id: "title", key: "title", text: "Updated" }],
  };
  disposer.update();

  expect(disposer.records.slice(11)).toEqual([
    "style:root:surface.card emphasis",
    "prop:title:text=Updated",
    "remove-element:cta",
  ]);

  disposer();
  expect(disposer.records.at(-1)).toBe("unmount-element:root");
  expect(disposer.snapshot).toBeUndefined();
});

test("Solid renderer rejects duplicate keyed children", () => {
  expect(() =>
    renderHawkSolid(
      () => ({
        id: "root",
        children: [
          { id: "first", key: "title" },
          { id: "second", key: "title" },
        ],
      }),
      { target: { id: "host" } },
    ),
  ).toThrow("solid.child-key.duplicate");
});
