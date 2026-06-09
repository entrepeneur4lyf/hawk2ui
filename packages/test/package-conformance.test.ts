import { readdirSync, readFileSync } from "node:fs";
import { expect, test } from "bun:test";
import { createHawkApp } from "@hawk2ui/native";
import { compileHawkReact } from "@hawk2ui/react/compiler";
import { compileHawkVue } from "@hawk2ui/vue/compiler";
import { compileHawkSolid } from "../hawk2ui-solid/src/index.ts";
import { compileHawkSvelte } from "../hawk2ui-svelte/src/index.ts";

type PackageJson = {
  dependencies?: Record<string, string>;
  exports?: Record<string, string>;
  version?: string;
};

const expectedRecords = [
  "mount-element:root",
  "ref:root:root_ref",
  "style:root:surface.card",
  "asset:root:assets/logo.svg",
  "bind-event:root:pointer.press",
  "mount-element:title",
  "mount-element:cta",
];

const readPackageJson = (packageName: string): PackageJson =>
  JSON.parse(
    readFileSync(new URL(`../${packageName}/package.json`, import.meta.url), "utf8"),
  ) as PackageJson;

const sourceFilesForPackage = (packageName: string): string[] => {
  const files: string[] = [];

  const visit = (directory: URL) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const entryUrl = new URL(entry.name + (entry.isDirectory() ? "/" : ""), directory);

      if (entry.isDirectory()) {
        visit(entryUrl);
        continue;
      }

      if (entry.name.endsWith(".ts") || entry.name.endsWith(".tsx")) {
        files.push(readFileSync(entryUrl, "utf8"));
      }
    }
  };

  visit(new URL(`../${packageName}/src/`, import.meta.url));
  return files;
};

const importSpecifierPattern = /\b(?:import|export)\s+(?:type\s+)?(?:[^"'()]+?\s+from\s+)?["']([^"']+)["']/g;

const collectImportSpecifiers = (sources: string[]): string[] =>
  sources.flatMap((source) =>
    [...source.matchAll(importSpecifierPattern)].map((match) => match[1] ?? ""),
  );

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

test("react and vue packages declare native as a package dependency", () => {
  const nativePackage = readPackageJson("hawk2ui-native");
  const reactPackage = readPackageJson("hawk2ui-react");
  const vuePackage = readPackageJson("hawk2ui-vue");

  expect(reactPackage.dependencies?.["@hawk2ui/native"]).toBe(nativePackage.version);
  expect(vuePackage.dependencies?.["@hawk2ui/native"]).toBe(nativePackage.version);
});

test("react and vue package source imports native through its package name", () => {
  for (const packageName of ["hawk2ui-react", "hawk2ui-vue"]) {
    const importSpecifiers = collectImportSpecifiers(sourceFilesForPackage(packageName));

    expect(importSpecifiers).toContain("@hawk2ui/native");
    expect(importSpecifiers).not.toContain("../../hawk2ui-native/src/index.ts");
  }
});

test("react and vue package source uses package-safe import specifiers", () => {
  for (const packageName of ["hawk2ui-react", "hawk2ui-vue"]) {
    const unsafeImportSpecifiers = collectImportSpecifiers(sourceFilesForPackage(packageName))
      .filter((specifier) => specifier.startsWith("."))
      .filter((specifier) => specifier.endsWith(".ts") || specifier.endsWith(".tsx"));

    expect(unsafeImportSpecifiers).toEqual([]);
  }
});
