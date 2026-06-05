import { expect, test } from "bun:test";
import { compileHawkVue, createHawkVueRenderer } from "../src/index.ts";

test("Vue compiler emits versioned native compiler artifacts from SFC templates", () => {
  const output = compileHawkVue({
    filename: "App.vue",
    source:
      '<script setup>const items = [{ id: "title" }, { id: "cta" }];</script><template><hawk-view id="root" ref="root_ref" class="surface.card" data-asset="assets/logo.svg" @pointerdown="handlePress" @mounted="onMounted" @unmounted="onUnmounted"><hawk-text v-for="item in items" :id="item.id" :key="item.id">{{ item.id }}</hawk-text></hawk-view></template>',
  });

  expect(output.compilerArtifact.schema_version).toBe(1);
  expect(output.compilerArtifact.root.id).toBe("root");
  expect(output.compilerArtifact.root.style_refs).toEqual(["surface.card"]);
  expect(output.compilerArtifact.root.children.map((child) => child.key)).toEqual(["title", "cta"]);
  expect(output.compilerArtifact.root.lifecycle).toEqual([
    { event: "mounted", handler: "onMounted" },
    { event: "unmounted", handler: "onUnmounted" },
  ]);
});

test("Vue renderer renders, patches, removes children, and unmounts deterministically", () => {
  const renderer = createHawkVueRenderer();
  const target = { id: "host" };

  renderer.render(
    {
      id: "root",
      ref: "root_ref",
      class: "surface.card",
      asset: "assets/logo.svg",
      on: ["pointer.press"],
      children: [
        { id: "title", key: "title", text: "Title" },
        { id: "cta", key: "cta", text: "Go" },
      ],
    },
    target,
  );

  expect(renderer.records).toEqual([
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

  renderer.render(
    {
      id: "root",
      ref: "root_ref",
      class: "surface.card emphasis",
      asset: "assets/logo.svg",
      on: ["pointer.press"],
      children: [{ id: "title", key: "title", text: "Updated" }],
    },
    target,
  );

  expect(renderer.records.slice(9)).toEqual([
    "style:root:surface.card emphasis",
    "prop:title:text=Updated",
    "remove-element:cta",
  ]);

  renderer.unmount(target);
  expect(renderer.records.at(-1)).toBe("unmount-element:root");
});

test("Vue renderer rejects duplicate keyed children", () => {
  const renderer = createHawkVueRenderer();

  expect(() =>
    renderer.render(
      {
        id: "root",
        children: [
          { id: "first", key: "title" },
          { id: "second", key: "title" },
        ],
      },
      { id: "host" },
    ),
  ).toThrow("vue.child-key.duplicate");
});
