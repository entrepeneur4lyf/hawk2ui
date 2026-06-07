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

test("createHawkApp rejects malformed runtime element records", () => {
  expect(() =>
    createHawkApp({
      name: "bad-kind",
      root: { id: "root", kind: "webview" } as never,
    }),
  ).toThrow("native.element.kind-invalid");

  expect(() =>
    createHawkApp({
      name: "bad-events",
      root: {
        id: "root",
        kind: "view",
        events: [{ kind: "click", handler: "handleClick" }] as never,
      },
    }),
  ).toThrow("native.event.kind-invalid");

  expect(() =>
    createHawkApp({
      name: "bad-lifecycle",
      root: {
        id: "root",
        kind: "view",
        lifecycle: [{ phase: "created", handler: "onCreated" }] as never,
      },
    }),
  ).toThrow("native.lifecycle.phase-invalid");

  expect(() =>
    createHawkApp({
      name: "bad-children",
      root: {
        id: "root",
        kind: "view",
        children: { id: "child", kind: "text" } as never,
      },
    }),
  ).toThrow("native.children.invalid");
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

test("compilerArtifactForApp rejects malformed runtime dynamic value records", () => {
  const spec = { name: "native-basic", root: { id: "root", kind: "view" as const } };

  expect(() =>
    compilerArtifactForApp(spec, [], [], [
      {
        name: "bad",
        mode: "value",
        value: { type: "date", value: "2026-06-06" } as never,
      },
    ]),
  ).toThrow("native.initial-dynamic-value.type-invalid");

  expect(() =>
    compilerArtifactForApp(spec, [], [], [
      {
        name: "nested",
        mode: "value",
        value: {
          type: "array",
          value: [{ type: "date", value: "2026-06-06" } as never],
        },
      },
    ]),
  ).toThrow("native.initial-dynamic-value.type-invalid");

  expect(() =>
    compilerArtifactForApp(spec, [], [], [
      {
        name: "bad-string",
        mode: "value",
        value: { type: "string", value: 12 } as never,
      },
    ]),
  ).toThrow("native.initial-dynamic-value.string-invalid");

  expect(() =>
    compilerArtifactForApp(spec, [], [], [
      {
        name: "bad-array",
        mode: "value",
        value: { type: "array", value: "not-an-array" } as never,
      },
    ]),
  ).toThrow("native.initial-dynamic-value.array-invalid");

  expect(() =>
    compilerArtifactForApp(spec, [], [], [
      {
        name: "bad-object",
        mode: "value",
        value: { type: "object", value: null } as never,
      },
    ]),
  ).toThrow("native.initial-dynamic-value.object-invalid");
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

test("compilerArtifactForApp preserves executable event handler artifacts", () => {
  const spec = {
    name: "native-events",
    root: {
      id: "root",
      kind: "view" as const,
      events: [{ kind: "pointer.press" as const, handler: "handlePress" }],
    },
  };

  const artifact = compilerArtifactForApp(spec, [], [], [], {
    eventHandlers: [
      {
        name: "handlePress",
        actions: [
          {
            type: "set_dynamic_value",
            name: "label",
            value: { type: "string", value: "Pressed" },
          },
        ],
      },
    ],
  });

  expect(artifact.event_handlers).toEqual([
    {
      name: "handlePress",
      actions: [
        {
          type: "set_dynamic_value",
          name: "label",
          value: { type: "string", value: "Pressed" },
        },
      ],
    },
  ]);

  expect(() =>
    compilerArtifactForApp(spec, [], [], [], {
      eventHandlers: [
        {
          name: "unreferenced",
          actions: [
            {
              type: "set_dynamic_value",
              name: "label",
              value: { type: "string", value: "Pressed" },
            },
          ],
        },
      ],
    }),
    ).toThrow("native.event-handler.unreferenced");
  });

  test("compilerArtifactForApp treats list template event handlers as referenced", () => {
    const spec = { name: "native-template-events", root: { id: "root", kind: "view" as const } };
    const artifact = compilerArtifactForApp(spec, [], [], [], {
      eventHandlers: [
        {
          name: "handleItemPress",
          actions: [
            {
              type: "set_dynamic_value",
              name: "selected",
              value: { type: "string", value: "selected" },
            },
          ],
        },
      ],
      listTemplates: [
        {
          id: "root:items",
          parent_id: "root",
          source: "items",
          item: "item",
          key: "item.id",
          node: {
            id: { type: "expression", expression: "item.id" },
            kind: "button",
            key: { type: "expression", expression: "item.id" },
            props: [],
            refs: [],
            style_refs: [],
            asset_refs: [],
            events: [{ kind: "pointer.press", handler: "handleItemPress", payload_fields: ["position"] }],
            lifecycle: [],
            children: [],
          },
        },
      ],
    });

    expect(artifact.event_handlers.map((handler) => handler.name)).toEqual(["handleItemPress"]);
    expect(artifact.list_templates[0].node.events.map((event) => event.handler)).toEqual(["handleItemPress"]);
  });

  test("compilerArtifactForApp accepts the complete stable native event and lifecycle contract", () => {
  const artifact = compilerArtifactForApp({
    name: "native-event-contract",
    root: {
      id: "root",
      kind: "view",
      events: [
        { kind: "pointer.press", handler: "handlePointerPress" },
        { kind: "pointer.release", handler: "handlePointerRelease" },
        { kind: "pointer.move", handler: "handlePointerMove" },
        { kind: "pointer.drag", handler: "handlePointerDrag" },
        { kind: "pointer.enter", handler: "handlePointerEnter" },
        { kind: "pointer.leave", handler: "handlePointerLeave" },
        { kind: "pointer.wheel", handler: "handlePointerWheel" },
        { kind: "keyboard.key-down", handler: "handleKeyDown" },
        { kind: "keyboard.key-up", handler: "handleKeyUp" },
        { kind: "keyboard.text-input", handler: "handleTextInput" },
        { kind: "focus.focus-in", handler: "handleFocusIn" },
        { kind: "focus.focus-out", handler: "handleFocusOut" },
        { kind: "input.value-changed", handler: "handleValueChanged" },
        { kind: "input.value-committed", handler: "handleValueCommitted" },
        { kind: "resize", handler: "handleResize" },
      ],
      lifecycle: [
        { phase: "mounted", handler: "handleMounted" },
        { phase: "suspended", handler: "handleSuspended" },
        { phase: "resumed", handler: "handleResumed" },
        { phase: "hot-reloaded", handler: "handleHotReloaded" },
        { phase: "error-boundary", handler: "handleErrorBoundary" },
        { phase: "shutdown", handler: "handleShutdown" },
        { phase: "unmounted", handler: "handleUnmounted" },
      ],
    },
  });

  expect(artifact.root.events.map((event) => event.kind)).toEqual([
    "pointer.press",
    "pointer.release",
    "pointer.move",
    "pointer.drag",
    "pointer.enter",
    "pointer.leave",
    "pointer.wheel",
    "keyboard.key-down",
    "keyboard.key-up",
    "keyboard.text-input",
    "focus.focus-in",
    "focus.focus-out",
    "input.value-changed",
    "input.value-committed",
    "resize",
  ]);
  expect(artifact.root.events.map((event) => event.payload_fields)).toEqual([
    ["position"],
    ["position"],
    ["position"],
    ["position", "delta"],
    ["position"],
    ["position"],
    ["position", "delta"],
    ["key"],
    ["key"],
    ["value"],
    [],
    [],
    ["value"],
    ["value"],
    [],
  ]);
  expect(artifact.root.lifecycle.map((lifecycle) => lifecycle.event)).toEqual([
    "mounted",
    "suspended",
    "resumed",
    "hot-reloaded",
    "error-boundary",
    "shutdown",
    "unmounted",
  ]);
});
