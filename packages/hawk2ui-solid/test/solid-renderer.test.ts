import { expect, test } from "bun:test";
import { compileHawkSolid, renderHawkSolid } from "../src/index.ts";

test("Solid compiler emits versioned native compiler artifacts from TSX", () => {
  const output = compileHawkSolid({
    filename: "App.tsx",
    source:
      'export function App() { const [items] = createSignal([{ id: "title" }, { id: "cta" }]); return <hawk-view id="root" ref={root_ref} class="surface.card" data-asset="assets/logo.svg" onPointerDown={handlePress} onMount={onMount} onCleanup={onCleanup}><For each={items()}>{(item) => <hawk-text id={item.id}>{item.id}</hawk-text>}</For></hawk-view>; }',
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
  expect(output.compilerArtifact.root.children.map((child) => child.key)).toEqual(["title", "cta"]);
  expect(output.compilerArtifact.reactivity).toEqual([
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

test("Solid renderer records fine-grained updates, removals, and dispose", () => {
  let component = {
    id: "root",
    ref: "root_ref",
    class: "surface.card",
    asset: "assets/logo.svg",
    on: ["pointer.press"],
    children: [
      { id: "title", key: "title", text: "Title" },
      { id: "cta", key: "cta", text: "Go" },
    ],
  };
  const disposer = renderHawkSolid(() => component, { target: { id: "host" } });

  expect(disposer.records).toEqual([
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

  component = {
    id: "root",
    ref: "root_ref",
    class: "surface.card emphasis",
    asset: "assets/logo.svg",
    on: ["pointer.press"],
    children: [{ id: "title", key: "title", text: "Updated" }],
  };
  disposer.update();

  expect(disposer.records.slice(9)).toEqual([
    "style:root:surface.card emphasis",
    "prop:title:text=Updated",
    "remove-element:cta",
  ]);

  disposer();
  expect(disposer.records.at(-1)).toBe("unmount-element:root");
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
