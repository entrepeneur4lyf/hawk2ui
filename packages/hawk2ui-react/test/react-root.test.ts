import { expect, test } from "bun:test";
import { compileHawkReact } from "../src/index.ts";
import { createHawkReactRoot } from "../src/testkit.ts";

test("React compiler emits versioned native compiler artifacts from TSX", () => {
  const output = compileHawkReact({
    filename: "App.tsx",
    source:
      'export function App() { let status = "idle"; function handlePress() { status = "pressed"; } function onMount() { status = "mounted"; } function onUnmount() { status = "unmounted"; } const items = [{ id: "title" }, { id: "cta" }]; return <hawk-view id="root" ref="root_ref" className="surface.card" data-asset="assets/logo.svg" onPointerDown={handlePress} onMount={onMount} onUnmount={onUnmount}>{items.map((item) => <hawk-text id={item.id} key={item.id}>{item.id}</hawk-text>)}</hawk-view>; }',
  });

  expect(output.compilerArtifact.schema_version).toBe(1);
  expect(output.compilerArtifact.compiler).toEqual({
    framework: "react",
    compiler: "@hawk2ui/react",
    source_path: "App.tsx",
    entrypoint: "App",
  });
  expect(output.compilerArtifact.root.id).toBe("root");
  expect(output.compilerArtifact.root.style_refs).toEqual(["surface.card"]);
  expect(output.compilerArtifact.root.children.map((child) => child.key)).toEqual(["title", "cta"]);
  expect(output.compilerArtifact.root.lifecycle).toEqual([
    { event: "mounted", handler: "onMount" },
    { event: "unmounted", handler: "onUnmount" },
  ]);
});

test("React compiler preserves dynamic text bindings from TSX expressions", () => {
  const output = compileHawkReact({
    filename: "App.tsx",
    source:
      'const label = "Live"; export function App() { return <hawk-view id="root"><hawk-text id="title">{label}</hawk-text></hawk-view>; }',
  });

  expect(output.compilerArtifact.dynamic_bindings).toEqual([
    {
      node_id: "title",
      target: { type: "prop", name: "text" },
      expression: "label",
      dependencies: ["label"],
    },
  ]);
  expect(output.compilerArtifact.initial_dynamic_values).toEqual([
    {
      name: "label",
      mode: "value",
      value: { type: "string", value: "Live" },
    },
  ]);
});

test("React compiler emits executable pointer handler actions", () => {
  const output = compileHawkReact({
    filename: "App.tsx",
    source:
      'let label = "Idle"; function handlePress() { label = "Pressed"; } export function App() { return <hawk-view id="root" onPointerDown={handlePress}><hawk-text id="title">{label}</hawk-text></hawk-view>; }',
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

test("React compiler lowers useState setter handler actions", () => {
  const output = compileHawkReact({
    filename: "App.tsx",
    source:
      'import { useState } from "react"; export function App() { const [label, setLabel] = useState("Idle"); function handlePress() { setLabel("Pressed"); } return <hawk-view id="root" onPointerDown={handlePress}><hawk-text id="title">{label}</hawk-text></hawk-view>; }',
  });

  expect(output.compilerArtifact.initial_dynamic_values).toEqual([
    {
      name: "label",
      mode: "value",
      value: { type: "string", value: "Idle" },
    },
  ]);
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

test("React compiler preserves event payload handler expressions", () => {
  const output = compileHawkReact({
    filename: "App.tsx",
    source:
      'import { useState } from "react"; export function App() { const [label, setLabel] = useState("Idle"); function handlePress(event: { x: number; y: number }) { setLabel(event.x + ":" + event.y); } return <hawk-view id="root" onPointerDown={handlePress}><hawk-text id="title">{label}</hawk-text></hawk-view>; }',
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

test("React compiler expands local components with props and forwarded children", () => {
  const output = compileHawkReact({
    filename: "App.tsx",
    source:
      'function Card(props: { id: string; title: string; children?: unknown }) { return <hawk-view id={props.id} className="card"><hawk-text id="card-title">{props.title}</hawk-text>{props.children}</hawk-view>; } export function App() { return <hawk-view id="root"><Card id="panel" title="Panel"><hawk-button id="cta">Go</hawk-button></Card></hawk-view>; }',
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

test("React compiler lowers the complete native event and lifecycle contract from JSX props", () => {
  const output = compileHawkReact({
    filename: "App.tsx",
    source:
      'let status = "idle"; function markPointerPress() { status = "pointer.press"; } function markPointerRelease() { status = "pointer.release"; } function markPointerMove() { status = "pointer.move"; } function markPointerDrag() { status = "pointer.drag"; } function markPointerEnter() { status = "pointer.enter"; } function markPointerLeave() { status = "pointer.leave"; } function markPointerWheel() { status = "pointer.wheel"; } function markKeyDown() { status = "keyboard.key-down"; } function markKeyUp() { status = "keyboard.key-up"; } function markTextInput() { status = "keyboard.text-input"; } function markFocusIn() { status = "focus.focus-in"; } function markFocusOut() { status = "focus.focus-out"; } function markValueChanged() { status = "input.value-changed"; } function markValueCommitted() { status = "input.value-committed"; } function markResize() { status = "resize"; } function markMounted() { status = "mounted"; } function markSuspended() { status = "suspended"; } function markResumed() { status = "resumed"; } function markHotReloaded() { status = "hot-reloaded"; } function markErrorBoundary() { status = "error-boundary"; } function markShutdown() { status = "shutdown"; } function markUnmounted() { status = "unmounted"; } export function App() { return <hawk-view id="root" onPointerDown={markPointerPress} onPointerUp={markPointerRelease} onPointerMove={markPointerMove} onPointerDrag={markPointerDrag} onPointerEnter={markPointerEnter} onPointerLeave={markPointerLeave} onWheel={markPointerWheel} onKeyDown={markKeyDown} onKeyUp={markKeyUp} onTextInput={markTextInput} onFocus={markFocusIn} onBlur={markFocusOut} onInput={markValueChanged} onChange={markValueCommitted} onResize={markResize} onMount={markMounted} onSuspend={markSuspended} onResume={markResumed} onHotReload={markHotReloaded} onErrorBoundary={markErrorBoundary} onShutdown={markShutdown} onUnmount={markUnmounted}></hawk-view>; }',
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

test("React compiler lowers useEffect mount and cleanup to root lifecycle", () => {
  const output = compileHawkReact({
    filename: "App.tsx",
    source:
      'import { useEffect } from "react"; function mounted() {} function destroyed() {} export function App() { useEffect(() => { mounted(); return destroyed; }, []); return <hawk-view id="root"></hawk-view>; }',
  });

  expect(output.compilerArtifact.root.lifecycle).toContainEqual({ event: "mounted", handler: "mounted" });
  expect(output.compilerArtifact.root.lifecycle).toContainEqual({ event: "unmounted", handler: "destroyed" });
});

test("React compiler preserves dependency useEffect calls as effect reactivity bindings", () => {
  const output = compileHawkReact({
    filename: "App.tsx",
    source:
      'import { useEffect, useState } from "react"; export function App() { const [label] = useState("Ready"); function syncLabel() { label; } useEffect(syncLabel, [label]); return <hawk-text id="root">{label}</hawk-text>; }',
  });

  expect(output.compilerArtifact.reactivity).toContainEqual({
    kind: "effect",
    name: "useEffect:syncLabel:label",
  });
});

test("React compiler preserves useMemo literal initial dynamic values", () => {
  const output = compileHawkReact({
    filename: "App.tsx",
    source:
      'import { useMemo } from "react"; export function App() { const label = useMemo(() => "Ready", []); return <hawk-text id="root">{label}</hawk-text>; }',
  });

  expect(output.compilerArtifact.initial_dynamic_values).toContainEqual({
    name: "label",
    mode: "value",
    value: { type: "string", value: "Ready" },
  });
});

test("React compiler emits runtime list templates for dynamic keyed maps", () => {
  const output = compileHawkReact({
    filename: "App.tsx",
    source:
      'import { useState } from "react"; export function App() { const [items, setItems] = useState([{ id: "alpha", label: "Alpha" }, { id: "beta", label: "Beta" }]); function handleItemPress() { setItems([{ id: "gamma", label: "Gamma" }]); } return <hawk-view id="root">{items.map((item) => <hawk-text id={item.id} key={item.id} onPointerDown={handleItemPress}>{item.label}</hawk-text>)}</hawk-view>; }',
  });

  expect(output.compilerArtifact.root.children).toEqual([]);
  expect(output.compilerArtifact.initial_dynamic_values).toContainEqual({
    name: "items",
    mode: "value",
    value: {
      type: "array",
      value: [
        {
          type: "object",
          value: {
            id: { type: "string", value: "alpha" },
            label: { type: "string", value: "Alpha" },
          },
        },
        {
          type: "object",
          value: {
            id: { type: "string", value: "beta" },
            label: { type: "string", value: "Beta" },
          },
        },
      ],
    },
  });
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

test("React compiler anchors runtime list templates before the next static sibling", () => {
  const output = compileHawkReact({
    filename: "App.tsx",
    source:
      'import { useState } from "react"; export function App() { const [items] = useState([{ id: "alpha", label: "Alpha" }]); return <hawk-view id="root"><hawk-text id="header">Header</hawk-text>{items.map((item) => <hawk-text id={item.id + "-row"} key={item.id + "-row"}>{item.label + "!"}</hawk-text>)}<hawk-text id="footer">Footer</hawk-text></hawk-view>; }',
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

test("React compiler preserves dynamic layout prop bindings from TSX expressions", () => {
  const output = compileHawkReact({
    filename: "App.tsx",
    source:
      'const panelWidth = getWidth(); const panelHeight = getHeight(); export function App() { return <hawk-view id="root"><hawk-view id="panel" width={panelWidth} height={panelHeight}></hawk-view></hawk-view>; }',
  });

  expect(output.compilerArtifact.dynamic_bindings).toEqual([
    {
      node_id: "panel",
      target: { type: "prop", name: "width" },
      expression: "panelWidth",
      dependencies: ["panelWidth"],
    },
    {
      node_id: "panel",
      target: { type: "prop", name: "height" },
      expression: "panelHeight",
      dependencies: ["panelHeight"],
    },
  ]);
});

test("React compiler preserves dynamic visual prop bindings from TSX expressions", () => {
  const output = compileHawkReact({
    filename: "App.tsx",
    source:
      'const panelBackground = getSurface(); const titleSize = getSize(); const titleColor = getColor(); export function App() { return <hawk-view id="root"><hawk-view id="panel" background={panelBackground}></hawk-view><hawk-text id="title" font_size={titleSize} color={titleColor}>Title</hawk-text></hawk-view>; }',
  });

  expect(output.compilerArtifact.dynamic_bindings).toEqual([
    {
      node_id: "panel",
      target: { type: "prop", name: "background" },
      expression: "panelBackground",
      dependencies: ["panelBackground"],
    },
    {
      node_id: "title",
      target: { type: "prop", name: "font_size" },
      expression: "titleSize",
      dependencies: ["titleSize"],
    },
    {
      node_id: "title",
      target: { type: "prop", name: "color" },
      expression: "titleColor",
      dependencies: ["titleColor"],
    },
  ]);
});

test("React compiler passes through arbitrary scalar props and dynamic prop bindings", () => {
  const output = compileHawkReact({
    filename: "App.tsx",
    source:
      'const role = getRole(); export function App() { return <hawk-view id="root"><hawk-button id="cta" aria_label="Start" tab_index={2} selected={true} data_role={role}>Go</hawk-button></hawk-view>; }',
  });
  const button = output.compilerArtifact.root.children[0].node;

  expect(button.props).toContainEqual({ name: "aria_label", value: { type: "string", value: "Start" } });
  expect(button.props).toContainEqual({ name: "tab_index", value: { type: "number", value: 2 } });
  expect(button.props).toContainEqual({ name: "selected", value: { type: "bool", value: true } });
  expect(output.compilerArtifact.dynamic_bindings).toContainEqual({
    node_id: "cta",
    target: { type: "prop", name: "data_role" },
    expression: "role",
    dependencies: ["role"],
  });
});

test("React compiler maps onClick to pointer press", () => {
  const output = compileHawkReact({
    filename: "App.tsx",
    source:
      'let label = "Idle"; function handleClick() { label = "Clicked"; } export function App() { return <hawk-view id="root"><hawk-button id="cta" onClick={handleClick}>Go</hawk-button></hawk-view>; }',
  });

  expect(output.compilerArtifact.root.children[0].node.events).toEqual([
    { kind: "pointer.press", handler: "handleClick", payload_fields: ["position"] },
  ]);
});

test("React compiler lowers hawk-surface to a custom surface node", () => {
  const output = compileHawkReact({
    filename: "App.tsx",
    source:
      'export function App() { return <hawk-view id="root"><hawk-surface id="meter" surface_id="level-meter" /></hawk-view>; }',
  });

  const surface = output.compilerArtifact.root.children[0].node;
  expect(surface.kind).toBe("custom-surface");
  expect(surface.props).toContainEqual({ name: "surface_id", value: { type: "string", value: "level-meter" } });
});

test("React compiler lowers logical conditionals to visible dynamic bindings", () => {
  const output = compileHawkReact({
    filename: "App.tsx",
    source:
      'export function App() { const [showTitle] = useState(true); return <hawk-view id="root">{showTitle && <hawk-text id="title">Title</hawk-text>}</hawk-view>; }',
  });

  expect(output.compilerArtifact.root.children[0].node.id).toBe("title");
  expect(output.compilerArtifact.dynamic_bindings).toContainEqual({
    node_id: "title",
    target: { type: "prop", name: "visible" },
    expression: "showTitle",
    dependencies: ["showTitle"],
  });
});

test("React compiler lowers ternary branches to visible dynamic bindings", () => {
  const output = compileHawkReact({
    filename: "App.tsx",
    source:
      'export function App() { const [showTitle] = useState(true); return <hawk-view id="root">{showTitle ? <hawk-text id="title">Title</hawk-text> : <hawk-text id="fallback">Fallback</hawk-text>}</hawk-view>; }',
  });

  expect(output.compilerArtifact.root.children.map((child) => child.node.id)).toEqual(["title", "fallback"]);
  expect(output.compilerArtifact.dynamic_bindings).toContainEqual({
    node_id: "title",
    target: { type: "prop", name: "visible" },
    expression: "showTitle",
    dependencies: ["showTitle"],
  });
  expect(output.compilerArtifact.dynamic_bindings).toContainEqual({
    node_id: "fallback",
    target: { type: "prop", name: "visible" },
    expression: "!(showTitle)",
    dependencies: ["showTitle"],
  });
});

test("React compiler accepts standard element aliases for native nodes", () => {
  const output = compileHawkReact({
    filename: "App.tsx",
    source:
      'export function App() { return <div id="root"><button id="cta">Go</button><p id="copy">Copy</p></div>; }',
  });

  expect(output.compilerArtifact.root.kind).toBe("view");
  expect(output.compilerArtifact.root.children.map((child) => [child.node.id, child.node.kind])).toEqual([
    ["cta", "button"],
    ["copy", "text"],
  ]);
});

test("React root renders, updates, removes children, and unmounts deterministically", () => {
  const root = createHawkReactRoot({ id: "host" });

  root.render({
    type: "hawk-view",
    props: {
      id: "root",
      ref: "root_ref",
      className: "surface.card",
      "data-asset": "assets/logo.svg",
      onPointerDown: "handlePress",
      onKeyDown: "handleKeyDown",
      children: [
        { type: "hawk-text", key: "title", props: { id: "title", text: "Title" } },
        {
          type: "hawk-button",
          key: "cta",
          props: { id: "cta", text: "Go", onPointerUp: "handleRelease" },
        },
      ],
    },
  });

  expect(root.records).toEqual([
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
  expect(root.snapshot?.events?.map((event) => event.handler)).toEqual([
    "handlePress",
    "handleKeyDown",
  ]);
  expect(root.snapshot?.children?.[1]?.kind).toBe("button");
  expect(root.snapshot?.children?.[1]?.events?.[0]?.handler).toBe("handleRelease");

  root.render({
    type: "hawk-view",
    props: {
      id: "root",
      ref: "root_ref",
      className: "surface.card emphasis",
      "data-asset": "assets/logo.svg",
      onPointerDown: "handlePress",
      onKeyDown: "handleKeyDown",
      children: [
        { type: "hawk-text", key: "title", props: { id: "title", text: "Updated" } },
      ],
    },
  });

  expect(root.records.slice(11)).toEqual([
    "style:root:surface.card emphasis",
    "prop:title:text=Updated",
    "remove-element:cta",
  ]);

  root.unmount();
  expect(root.records.at(-1)).toBe("unmount-element:root");
  expect(root.snapshot).toBeUndefined();
});

test("React root rejects duplicate keyed children", () => {
  const root = createHawkReactRoot({ id: "host" });

  expect(() =>
    root.render({
      props: {
        id: "root",
        children: [
          { key: "title", props: { id: "first" } },
          { key: "title", props: { id: "second" } },
        ],
      },
    }),
  ).toThrow("react.child-key.duplicate");
});
