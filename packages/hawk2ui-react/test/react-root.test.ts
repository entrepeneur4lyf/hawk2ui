import { expect, test } from "bun:test";
import { compileHawkReact, createHawkReactRoot } from "../src/index.ts";

test("React compiler emits versioned native compiler artifacts from TSX", () => {
  const output = compileHawkReact({
    filename: "App.tsx",
    source:
      'const items = [{ id: "title" }, { id: "cta" }]; export function App() { return <hawk-view id="root" ref="root_ref" className="surface.card" data-asset="assets/logo.svg" onPointerDown={handlePress} onMount={onMount} onUnmount={onUnmount}>{items.map((item) => <hawk-text id={item.id} key={item.id}>{item.id}</hawk-text>)}</hawk-view>; }',
  });

  expect(output.compilerArtifact.schema_version).toBe(1);
  expect(output.compilerArtifact.root.id).toBe("root");
  expect(output.compilerArtifact.root.style_refs).toEqual(["surface.card"]);
  expect(output.compilerArtifact.root.children.map((child) => child.key)).toEqual(["title", "cta"]);
  expect(output.compilerArtifact.root.lifecycle).toEqual([
    { event: "mounted", handler: "onMount" },
    { event: "unmounted", handler: "onUnmount" },
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
