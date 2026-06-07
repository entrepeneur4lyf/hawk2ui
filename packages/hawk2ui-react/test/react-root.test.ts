import { expect, test } from "bun:test";
import { compileHawkReact, createHawkReactRoot } from "../src/index.ts";

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
      children: [
        { type: "hawk-text", key: "title", props: { id: "title", text: "Title" } },
        { type: "hawk-button", key: "cta", props: { id: "cta", text: "Go" } },
      ],
    },
  });

  expect(root.records).toEqual([
    "mount-element:root",
    "ref:root:root_ref",
    "style:root:surface.card",
    "asset:root:assets/logo.svg",
    "bind-event:root:pointer.press",
    "mount-element:title",
    "prop:title:text=Title",
    "mount-element:cta",
    "prop:cta:text=Go",
  ]);

  root.render({
    type: "hawk-view",
    props: {
      id: "root",
      ref: "root_ref",
      className: "surface.card emphasis",
      "data-asset": "assets/logo.svg",
      onPointerDown: "handlePress",
      children: [
        { type: "hawk-text", key: "title", props: { id: "title", text: "Updated" } },
      ],
    },
  });

  expect(root.records.slice(9)).toEqual([
    "style:root:surface.card emphasis",
    "prop:title:text=Updated",
    "remove-element:cta",
  ]);

  root.unmount();
  expect(root.records.at(-1)).toBe("unmount-element:root");
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
