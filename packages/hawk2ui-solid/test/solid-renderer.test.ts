import { expect, test } from "bun:test";
import { compileHawkSolid, renderHawkSolid } from "../src/index.ts";

test("Solid compiler emits versioned native compiler artifacts from TSX", () => {
  const output = compileHawkSolid({
    filename: "App.tsx",
    source:
      'const [items] = createSignal([{ id: "title" }, { id: "cta" }]); export function App() { return <hawk-view id="root" ref={root_ref} class="surface.card" data-asset="assets/logo.svg" onPointerDown={handlePress} onMount={onMount} onCleanup={onCleanup}><For each={items()}>{(item) => <hawk-text id={item.id}>{item.id}</hawk-text>}</For></hawk-view>; }',
  });

  expect(output.compilerArtifact.schema_version).toBe(1);
  expect(output.compilerArtifact.root.id).toBe("root");
  expect(output.compilerArtifact.root.style_refs).toEqual(["surface.card"]);
  expect(output.compilerArtifact.root.children.map((child) => child.key)).toEqual(["title", "cta"]);
  expect(output.compilerArtifact.reactivity).toEqual([
    { kind: "signal", name: "items" },
    { kind: "keyed-for-each", name: "items" },
    { kind: "effect", name: "root-props" },
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
