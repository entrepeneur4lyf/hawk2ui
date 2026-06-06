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

test("Vue compiler preserves dynamic text bindings from template interpolations", () => {
  const output = compileHawkVue({
    filename: "App.vue",
    source:
      '<script setup>const label = computed(() => "Title");</script><template><hawk-view id="root"><hawk-text id="title">{{ label }}</hawk-text></hawk-view></template>',
  });

  expect(output.compilerArtifact.dynamic_bindings).toEqual([
    {
      node_id: "title",
      target: { type: "prop", name: "text" },
      expression: "label",
      dependencies: ["label"],
    },
  ]);
});

test("Vue compiler preserves dynamic layout prop bindings from template bindings", () => {
  const output = compileHawkVue({
    filename: "App.vue",
    source:
      '<script setup>const panelWidth = computed(() => 240); const panelHeight = computed(() => 120);</script><template><hawk-view id="root"><hawk-view id="panel" :width="panelWidth" :height="panelHeight"></hawk-view></hawk-view></template>',
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

test("Vue compiler preserves dynamic visual prop bindings from template bindings", () => {
  const output = compileHawkVue({
    filename: "App.vue",
    source:
      '<script setup>const panelBackground = computed(() => "#111111"); const titleSize = computed(() => 18); const titleColor = computed(() => "#ffffff");</script><template><hawk-view id="root"><hawk-view id="panel" :background="panelBackground"></hawk-view><hawk-text id="title" :font_size="titleSize" :color="titleColor">Title</hawk-text></hawk-view></template>',
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
