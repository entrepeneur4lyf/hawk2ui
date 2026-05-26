import { expect, test } from "bun:test";
import { createHawkApp } from "../hawk2ui-native/src/index.ts";
import { createHawkReactRoot } from "../hawk2ui-react/src/index.ts";
import { renderHawkSolid } from "../hawk2ui-solid/src/index.ts";
import { compileHawkSvelte } from "../hawk2ui-svelte/src/index.ts";
import { createHawkVueRenderer } from "../hawk2ui-vue/src/index.ts";

const expectedRecords = [
  "mount-element:root",
  "ref:root:root_ref",
  "style:root:surface.card",
  "asset:root:assets/logo.svg",
  "bind-event:root:pointer.press",
  "mount-element:title",
  "mount-element:cta",
];

test("framework packages emit equivalent native records for the shared fixture", () => {
  const native = createHawkApp({
    name: "native-basic",
    root: {
      id: "root",
      kind: "view",
      refs: ["root_ref"],
      styleRefs: ["surface.card"],
      assetRefs: [{ name: "hawk.logo", path: "assets/logo.svg" }],
      events: [{ kind: "pointer.press", handler: "handlePress" }],
      children: [
        { id: "title", kind: "text", key: "title" },
        { id: "cta", kind: "button", key: "cta" },
      ],
    },
  });

  const svelte = compileHawkSvelte({
    filename: "App.svelte",
    source:
      '<hawk-view id="root" use:ref="root_ref" class="surface.card" data-asset="assets/logo.svg" on:press={handlePress}>{#each items as item (item.id)}<hawk-text id={item.id}>{item.id}</hawk-text>{/each}</hawk-view>',
  });

  const react = createHawkReactRoot({ id: "root" });
  react.render({
    props: {
      id: "root",
      ref: "root_ref",
      className: "surface.card",
      "data-asset": "assets/logo.svg",
      onPointerDown: "handlePress",
    },
    children: [{ id: "title" }, { id: "cta" }],
  });

  const vue = createHawkVueRenderer();
  vue.render(
    {
      id: "root",
      ref: "root_ref",
      class: "surface.card",
      asset: "assets/logo.svg",
      on: ["pointer.press"],
      children: [{ id: "title" }, { id: "cta" }],
    },
    { id: "root" },
  );

  const solid = renderHawkSolid(
    () => ({
      id: "root",
      ref: "root_ref",
      class: "surface.card",
      asset: "assets/logo.svg",
      on: ["pointer.press"],
      children: [{ id: "title" }, { id: "cta" }],
    }),
    { target: { id: "root" } },
  );

  expect(native.records).toEqual(expectedRecords);
  expect(svelte.records).toEqual(expectedRecords);
  expect(react.records).toEqual(expectedRecords);
  expect(vue.records).toEqual(expectedRecords);
  expect(solid.records).toEqual(expectedRecords);
});
