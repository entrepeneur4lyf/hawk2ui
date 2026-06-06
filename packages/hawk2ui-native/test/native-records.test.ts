import { expect, test } from "bun:test";
import { compilerArtifactForApp, createHawkApp } from "../src/index.ts";

test("createHawkApp emits deterministic records for native element trees", () => {
  const app = createHawkApp({
    name: "native-basic",
    root: {
      id: "root",
      kind: "view",
      refs: ["root_ref"],
      styleRefs: ["surface.card"],
      assetRefs: [{ name: "hawk.logo", path: "assets/logo.svg" }],
      events: [{ kind: "pointer.press", handler: "handlePress" }],
      lifecycle: [
        { phase: "mounted", handler: "onMount" },
        { phase: "unmounted", handler: "onUnmount" },
      ],
      children: [
        { id: "title", kind: "text", key: "title", props: { text: "title" } },
        { id: "cta", kind: "button", key: "cta", props: { text: "Go" } },
      ],
    },
  });

  expect(app.records).toEqual([
    "lifecycle:mounted:root:onMount",
    "mount-element:root",
    "ref:root:root_ref",
    "style:root:surface.card",
    "asset:root:assets/logo.svg",
    "bind-event:root:pointer.press",
    "mount-element:title",
    "prop:title:text=title",
    "mount-element:cta",
    "prop:cta:text=Go",
    "lifecycle:unmounted:root:onUnmount",
  ]);
});

test("createHawkApp rejects duplicate child keys", () => {
  expect(() =>
    createHawkApp({
      name: "bad-keys",
      root: {
        id: "root",
        kind: "view",
        children: [
          { id: "first", kind: "text", key: "title" },
          { id: "second", kind: "text", key: "title" },
        ],
      },
    }),
  ).toThrow("native.child-key.duplicate");
});

test("createHawkApp rejects unsafe asset paths", () => {
  expect(() =>
    createHawkApp({
      name: "bad-assets",
      root: {
        id: "root",
        kind: "view",
        assetRefs: [{ name: "secret", path: "../secret.svg" }],
      },
    }),
  ).toThrow("native.asset.path-invalid");
});

test("compilerArtifactForApp rejects invalid initial dynamic values", () => {
  const spec = { name: "native-basic", root: { id: "root", kind: "view" as const } };

  expect(() =>
    compilerArtifactForApp(spec, [], [], [
      { name: "label", mode: "value", value: { type: "string", value: "A" } },
      { name: "label", mode: "getter", value: { type: "string", value: "B" } },
    ]),
  ).toThrow("native.initial-dynamic-value.duplicate");

  expect(() =>
    compilerArtifactForApp(spec, [], [], [
      {
        name: "meter",
        mode: "value",
        value: {
          type: "object",
          value: {
            gain: { type: "number", value: Number.POSITIVE_INFINITY },
          },
        },
      },
    ]),
  ).toThrow("native.initial-dynamic-value.number-invalid");
});

test("compilerArtifactForApp preserves explicit compiler source metadata", () => {
  const spec = { name: "react-app", root: { id: "root", kind: "view" as const } };
  const artifact = compilerArtifactForApp(spec, [], [], [], {
    compiler: {
      framework: "react",
      compiler: "@hawk2ui/react",
      source_path: "src/App.tsx",
      entrypoint: "App",
    },
  });

  expect(artifact.compiler).toEqual({
    framework: "react",
    compiler: "@hawk2ui/react",
    source_path: "src/App.tsx",
    entrypoint: "App",
  });

  expect(() =>
    compilerArtifactForApp(spec, [], [], [], {
      compiler: {
        framework: "react",
        compiler: "@hawk2ui/react",
        source_path: "../App.tsx",
        entrypoint: "App",
      },
    }),
  ).toThrow("native.compiler.source-path-invalid");
});
