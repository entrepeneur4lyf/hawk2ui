import { expect, test } from "bun:test";
import { h, ref } from "vue";

import { createHawkVueApp } from "../src/index.ts";
import { createRecordingVueSceneBridge } from "../src/sceneBridge.ts";

test("Vue scene bridge records native scene operations in committed batches", () => {
  const bridge = createRecordingVueSceneBridge();

  bridge.createNode("view", "root");
  bridge.setProp("root", "label", "Main surface");
  bridge.appendChild("host", "root");
  bridge.commit();

  expect(bridge.operations.map((operation) => operation.type)).toEqual([
    "create-node",
    "set-prop",
    "append-child",
    "commit",
  ]);
  expect(bridge.batches()).toEqual([
    {
      ops: [
        { type: "create-node", id: "root", kind: "view" },
        { type: "set-prop", id: "root", name: "label", value: { kind: "string", value: "Main surface" } },
        { type: "append-child", parent: "host", child: "root" },
        { type: "commit" },
      ],
    },
  ]);
});

test("Vue scene bridge rejects DOM-only props before they reach the native runtime", () => {
  const bridge = createRecordingVueSceneBridge();

  bridge.createNode("view", "root");

  expect(() => bridge.setProp("root", "innerHTML", "<b>bad</b>")).toThrow("vue.prop.unsupported");
  expect(() => bridge.setProp("root", "dangerouslySetInnerHTML", { __html: "<b>bad</b>" })).toThrow(
    "vue.prop.unsupported",
  );
  expect(() => bridge.setProp("root", "style", "color: red")).toThrow("vue.prop.unsupported");
});

test("Vue runtime renderer commits native scene operations after reactive state updates", async () => {
  const bridge = createRecordingVueSceneBridge();
  const app = createHawkVueApp(
    {
      setup() {
        const label = ref("Ready");
        return () =>
          h("hawk-view", { id: "root" }, [
            h(
              "hawk-button",
              {
                id: "cta",
                onClick: () => {
                  label.value = "Pressed";
                },
              },
              label.value,
            ),
          ]);
      },
    },
    { bridge },
  );

  const root = app.mount({ rootId: "host" });
  expect(JSON.stringify(bridge.batches())).toContain("Ready");

  root.dispatch("cta", "pointer.press");
  await root.flush();

  expect(JSON.stringify(bridge.batches())).toContain("Pressed");
});

test("Vue runtime renderer maps supported native primitives and aliases", () => {
  const bridge = createRecordingVueSceneBridge();
  const app = createHawkVueApp(
    {
      setup() {
        return () =>
          h("div", { id: "root", class: "surface.card" }, [
            h("span", { id: "title" }, "Title"),
            h("button", { id: "cta" }, "Run"),
            h("input", { id: "name", value: "Ada", placeholder: "Name" }),
            h("hawk-image", { id: "logo" }),
            h("hawk-vector", { id: "shape" }),
            h("hawk-scroll-view", { id: "scroll" }),
            h("hawk-custom-surface", { id: "surface" }),
          ]);
      },
    },
    { bridge },
  );

  app.mount({ rootId: "host" });

  const created = bridge.operations
    .filter((operation) => operation.type === "create-node")
    .map((operation) => [operation.id, operation.kind]);
  expect(created).toEqual([
    ["root", "view"],
    ["title", "text"],
    ["cta", "button"],
    ["name", "input"],
    ["logo", "image"],
    ["shape", "vector"],
    ["scroll", "scroll-view"],
    ["surface", "custom-surface"],
  ]);
  expect(JSON.stringify(bridge.batches())).toContain('"placeholder"');
});

test("Vue runtime renderer rejects unsupported native elements and DOM-only props", () => {
  expect(() =>
    createHawkVueApp(
      {
        setup() {
          return () => h("canvas", { id: "paint" });
        },
      },
      { bridge: createRecordingVueSceneBridge() },
    ).mount({ rootId: "host" }),
  ).toThrow("vue.element.unsupported");

  expect(() =>
    createHawkVueApp(
      {
        setup() {
          return () => h("hawk-view", { id: "root", innerHTML: "<b>bad</b>" });
        },
      },
      { bridge: createRecordingVueSceneBridge() },
    ).mount({ rootId: "host" }),
  ).toThrow("vue.prop.unsupported");
});
