import { expect, test } from "bun:test";
import { createHawkApp } from "../hawk2ui-native/src/index.ts";
import { compileHawkReact } from "../hawk2ui-react/src/index.ts";
import { compileHawkSolid } from "../hawk2ui-solid/src/index.ts";
import { compileHawkSvelte } from "../hawk2ui-svelte/src/index.ts";
import { compileHawkVue } from "../hawk2ui-vue/src/index.ts";

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
      '<script>let status = "idle"; function handlePress() { status = "pressed"; } const items = [{ id: "title" }, { id: "cta" }];</script><hawk-view id="root" use:root_ref class="surface.card" data-asset="assets/logo.svg" on:press={handlePress}>{#each items as item (item.id)}<hawk-text id={item.id}></hawk-text>{/each}</hawk-view>',
  });

  const react = compileHawkReact({
    filename: "App.tsx",
    source:
      'export function App() { let status = "idle"; function handlePress() { status = "pressed"; } return <hawk-view id="root" ref="root_ref" className="surface.card" data-asset="assets/logo.svg" onPointerDown={handlePress}><hawk-text id="title" /><hawk-button id="cta" /></hawk-view>; }',
  });

  const vue = compileHawkVue({
    filename: "App.vue",
    source:
      '<script setup>const status = ref("idle"); function handlePress() { status.value = "pressed"; }</script><template><hawk-view id="root" ref="root_ref" class="surface.card" data-asset="assets/logo.svg" @pointerdown="handlePress"><hawk-text id="title" /><hawk-button id="cta" /></hawk-view></template>',
  });

  const solid = compileHawkSolid({
    filename: "App.tsx",
    source:
      'export function App() { let status = "idle"; function handlePress() { status = "pressed"; } return <hawk-view id="root" ref="root_ref" class="surface.card" data-asset="assets/logo.svg" onPointerDown={handlePress}><hawk-text id="title" /><hawk-button id="cta" /></hawk-view>; }',
  });

  expect(native.records).toEqual(expectedRecords);
  expect(svelte.records).toEqual(expectedRecords);
  expect(react.records).toEqual(expectedRecords);
  expect(vue.records).toEqual(expectedRecords);
  expect(solid.records).toEqual(expectedRecords);
  expect(react.compilerArtifact.root.children.map((child) => child.node.kind)).toEqual([
    "text",
    "button",
  ]);
  expect(vue.compilerArtifact.root.children.map((child) => child.node.kind)).toEqual([
    "text",
    "button",
  ]);
  expect(solid.compilerArtifact.root.children.map((child) => child.node.kind)).toEqual([
    "text",
    "button",
  ]);
  expect(react.compilerArtifact.root.events.map((event) => event.handler)).toEqual([
    "handlePress",
  ]);
});
