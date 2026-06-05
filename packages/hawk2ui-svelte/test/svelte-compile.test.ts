import { expect, test } from "bun:test";
import { compileHawkSvelte } from "../src/index.ts";

test("Svelte compiler emits lifecycle, child props, and deterministic records", () => {
  const output = compileHawkSvelte({
    filename: "App.svelte",
    source:
      '<hawk-view id="root" use:root_ref class="surface.card" data-asset="assets/logo.svg" on:press={handlePress} on:mount={onMount} on:destroy={onDestroy}><hawk-text id="title">Title</hawk-text><hawk-button id="cta">Go</hawk-button></hawk-view>',
  });

  expect(output.records).toEqual([
    "lifecycle:mounted:root:onMount",
    "mount-element:root",
    "ref:root:root_ref",
    "style:root:surface.card",
    "asset:root:assets/logo.svg",
    "bind-event:root:pointer.press",
    "mount-element:title",
    "prop:title:text=Title",
    "mount-element:cta",
    "prop:cta:text=Go",
    "lifecycle:unmounted:root:onDestroy",
  ]);
  expect(output.compilerArtifact.root.style_refs).toEqual(["surface.card"]);
  expect(output.compilerArtifact.root.children.map((child) => child.key)).toEqual(["title", "cta"]);
  expect(output.compilerArtifact.root.lifecycle).toEqual([
    { event: "mounted", handler: "onMount" },
    { event: "unmounted", handler: "onDestroy" },
  ]);
});

test("Svelte compiler rejects unsupported events", () => {
  expect(() =>
    compileHawkSvelte({
      filename: "App.svelte",
      source: '<hawk-view id="root" on:hover={handleHover}></hawk-view>',
    }),
  ).toThrow("svelte.event.unsupported");
});

test("Svelte compiler rejects duplicate child ids", () => {
  expect(() =>
    compileHawkSvelte({
      filename: "App.svelte",
      source: '<hawk-view id="root"><hawk-text id="title">A</hawk-text><hawk-text id="title">B</hawk-text></hawk-view>',
    }),
  ).toThrow("svelte.child-id.duplicate");
});
