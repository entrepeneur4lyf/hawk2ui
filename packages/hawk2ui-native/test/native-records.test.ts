import { expect, test } from "bun:test";
import { createHawkApp } from "../src/index.ts";

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
