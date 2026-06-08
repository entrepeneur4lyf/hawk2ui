import { expect, test } from "bun:test";
import React, { useState } from "react";

import { createRecordingSceneBridge, createRoot, type HawkNativeNodeHandle } from "../src/index.ts";

function handlePress() {}

test("React reconciler renders native scene batches through the bridge", () => {
  const bridge = createRecordingSceneBridge();
  const root = createRoot("host", { bridge });

  root.render(
    React.createElement(
      "hawk-view",
      { id: "root", className: "surface.card", onPointerDown: handlePress },
      React.createElement("hawk-text", { id: "title" }, "Hello"),
      React.createElement("hawk-button", { id: "cta", onClick: "handleClick" }, "Go"),
    ),
  );

  expect(bridge.batches()).toEqual([
    {
      ops: [
        { type: "create-node", id: "root", kind: "view" },
        { type: "set-style", id: "root", name: "class", value: { kind: "string", value: "surface.card" } },
        { type: "register-event", id: "root", event: "pointer.press", handler: "handlePress" },
        { type: "create-node", id: "title", kind: "text" },
        { type: "set-prop", id: "title", name: "text", value: { kind: "string", value: "Hello" } },
        { type: "append-child", parent: "root", child: "title" },
        { type: "create-node", id: "cta", kind: "button" },
        { type: "set-prop", id: "cta", name: "text", value: { kind: "string", value: "Go" } },
        { type: "register-event", id: "cta", event: "pointer.press", handler: "handleClick" },
        { type: "append-child", parent: "root", child: "cta" },
        { type: "commit" },
      ],
    },
  ]);
  expect(root.committedBatches()).toEqual(bridge.batches());
});

test("React reconciler emits accessibility semantics separately from generic props", () => {
  const bridge = createRecordingSceneBridge();
  const root = createRoot("host", { bridge });

  root.render(
    React.createElement(
      "view",
      { id: "root" },
      React.createElement("button", {
        id: "render",
        role: "button",
        label: "Start render",
        description: "Starts the offline render",
        value: "ready",
        disabled: false,
        pressed: false,
      }),
    ),
  );

  expect(bridge.batches()).toEqual([
    {
      ops: [
        { type: "create-node", id: "root", kind: "view" },
        { type: "create-node", id: "render", kind: "button" },
        {
          type: "set-accessibility",
          id: "render",
          role: "button",
          label: "Start render",
          description: "Starts the offline render",
          value: { kind: "string", value: "ready" },
          disabled: false,
          pressed: false,
        },
        { type: "append-child", parent: "root", child: "render" },
        { type: "commit" },
      ],
    },
  ]);
});

test("React reconciler clears accessibility semantics when accessibility props are removed", () => {
  const bridge = createRecordingSceneBridge();
  const root = createRoot("host", { bridge });

  root.render(
    React.createElement(
      "view",
      { id: "root" },
      React.createElement("button", {
        id: "render",
        role: "button",
        label: "Start render",
        disabled: false,
      }),
    ),
  );
  bridge.drain();
  root.drainCommittedBatches();

  root.render(React.createElement("view", { id: "root" }, React.createElement("button", { id: "render" })));

  expect(bridge.batches()).toEqual([
    {
      ops: [
        { type: "set-accessibility", id: "render" },
        { type: "commit" },
      ],
    },
  ]);
});

test("React reconciler updates accessibility semantics after state changes", async () => {
  const bridge = createRecordingSceneBridge();
  const root = createRoot("host", { bridge });

  function AccessibleAction() {
    const [busy, setBusy] = useState(false);
    return React.createElement(
      "view",
      { id: "root" },
      React.createElement("button", {
        id: "render",
        label: busy ? "Rendering" : "Start render",
        disabled: busy,
        onPointerPress: () => setBusy(true),
      }),
    );
  }

  root.render(React.createElement(AccessibleAction));
  bridge.drain();
  root.drainCommittedBatches();

  bridge.dispatch("render", "pointer.press", {});
  await nextSchedulerTick();

  expect(bridge.batches()).toEqual([
    {
      ops: [
        {
          type: "set-accessibility",
          id: "render",
          label: "Rendering",
          disabled: true,
        },
        { type: "commit" },
      ],
    },
  ]);
});

test("React reconciler emits focus node operations for autoFocus", () => {
  const bridge = createRecordingSceneBridge();
  const root = createRoot("host", { bridge });

  root.render(
    React.createElement("view", { id: "root" }, React.createElement("input", { id: "field", autoFocus: true })),
  );

  expect(bridge.batches()).toEqual([
    {
      ops: [
        { type: "create-node", id: "root", kind: "view" },
        { type: "create-node", id: "field", kind: "input" },
        { type: "focus-node", id: "field" },
        { type: "append-child", parent: "root", child: "field" },
        { type: "commit" },
      ],
    },
  ]);
});

test("React reconciler emits measure node operations for explicit measurement requests", () => {
  const bridge = createRecordingSceneBridge();
  const root = createRoot("host", { bridge });

  root.render(React.createElement("view", { id: "root", measure: "root-layout" }));

  expect(bridge.batches()).toEqual([
    {
      ops: [
        { type: "create-node", id: "root", kind: "view" },
        { type: "measure-node", id: "root", request: "root-layout" },
        { type: "commit" },
      ],
    },
  ]);
});

test("React reconciler emits controlled input value scene props", () => {
  const bridge = createRecordingSceneBridge();
  const root = createRoot("host", { bridge });

  root.render(
    React.createElement("view", { id: "root" }, React.createElement("input", { id: "field", value: "Ada" })),
  );

  expect(bridge.batches()).toEqual([
    {
      ops: [
        { type: "create-node", id: "root", kind: "view" },
        { type: "create-node", id: "field", kind: "input" },
        { type: "set-prop", id: "field", name: "value", value: { kind: "string", value: "Ada" } },
        { type: "set-accessibility", id: "field", value: { kind: "string", value: "Ada" } },
        { type: "append-child", parent: "root", child: "field" },
        { type: "commit" },
      ],
    },
  ]);

  bridge.drain();
  root.drainCommittedBatches();

  root.render(
    React.createElement("view", { id: "root" }, React.createElement("input", { id: "field", value: "Grace" })),
  );

  expect(bridge.batches()).toEqual([
    {
      ops: [
        { type: "set-prop", id: "field", name: "value", value: { kind: "string", value: "Grace" } },
        { type: "set-accessibility", id: "field", value: { kind: "string", value: "Grace" } },
        { type: "commit" },
      ],
    },
  ]);
});

test("React reconciler updates controlled input state from native input events", async () => {
  const bridge = createRecordingSceneBridge();
  const root = createRoot("host", { bridge });
  let observedTargetValue: unknown;
  let observedCurrentTargetValue: unknown;

  function Form() {
    const [value, setValue] = useState("Ada");
    return React.createElement(
      "view",
      { id: "root" },
      React.createElement("input", {
        id: "field",
        value,
        onInput: (event: unknown) => {
          const payload = event as {
            readonly target?: { readonly value?: unknown };
            readonly currentTarget?: { readonly value?: unknown };
          };
          observedTargetValue = payload.target?.value;
          observedCurrentTargetValue = payload.currentTarget?.value;
          setValue(String(payload.currentTarget?.value));
        },
      }),
    );
  }

  root.render(React.createElement(Form));
  bridge.drain();
  root.drainCommittedBatches();

  bridge.dispatch("field", "input.value-changed", { value: "Grace" });
  await nextSchedulerTick();

  expect(observedTargetValue).toBe("Grace");
  expect(observedCurrentTargetValue).toBe("Grace");
  expect(bridge.batches()).toEqual([
    {
      ops: [
        { type: "set-prop", id: "field", name: "value", value: { kind: "string", value: "Grace" } },
        { type: "set-accessibility", id: "field", value: { kind: "string", value: "Grace" } },
        { type: "commit" },
      ],
    },
  ]);
});

test("React reconciler exposes stable native node handles through refs and forwardRef", () => {
  const bridge = createRecordingSceneBridge();
  const root = createRoot("host", { bridge });
  const inputRef = React.createRef<HawkNativeNodeHandle>();

  const Field = React.forwardRef<HawkNativeNodeHandle, { readonly id: string }>(function Field(props, ref) {
    return React.createElement("input", { id: props.id, ref });
  });

  root.render(React.createElement("view", { id: "root" }, React.createElement(Field, { id: "field", ref: inputRef })));

  expect(inputRef.current).toEqual({
    id: "field",
    kind: "input",
    focus: expect.any(Function),
    measure: expect.any(Function),
  });
  expect(Object.isFrozen(inputRef.current)).toBe(true);
  expect("props" in (inputRef.current as object)).toBe(false);
  expect("children" in (inputRef.current as object)).toBe(false);

  bridge.drain();
  root.drainCommittedBatches();

  inputRef.current?.focus();

  expect(bridge.batches()).toEqual([
    {
      ops: [
        { type: "focus-node", id: "field" },
        { type: "commit" },
      ],
    },
  ]);
});

test("React reconciler preserves reducer context memo callback and effect cleanup semantics", async () => {
  const bridge = createRecordingSceneBridge();
  const root = createRoot("host", { bridge });
  const LabelContext = React.createContext("unset");
  let cleanupCount = 0;

  function Counter() {
    const label = React.useContext(LabelContext);
    const [count, dispatch] = React.useReducer((state: number, action: "increment") => {
      return action === "increment" ? state + 1 : state;
    }, 0);
    const text = React.useMemo(() => `${label}:${count}`, [label, count]);
    const increment = React.useCallback(() => dispatch("increment"), []);

    React.useEffect(() => {
      return () => {
        cleanupCount += 1;
      };
    }, []);

    return React.createElement(
      "view",
      { id: "root" },
      React.createElement("text", { id: "count" }, text),
      React.createElement("button", { id: "increment", onPointerPress: increment }, "Increment"),
    );
  }

  root.render(React.createElement(LabelContext.Provider, { value: "count" }, React.createElement(Counter)));
  bridge.drain();
  root.drainCommittedBatches();

  bridge.dispatch("increment", "pointer.press", {});
  await nextSchedulerTick();

  expect(bridge.batches()).toEqual([
    {
      ops: [
        { type: "set-prop", id: "count", name: "text", value: { kind: "string", value: "count:1" } },
        { type: "commit" },
      ],
    },
  ]);

  bridge.drain();
  root.unmount();
  await nextSchedulerTick();

  expect(cleanupCount).toBe(1);
});

test("React reconciler clears removed class and style values", () => {
  const bridge = createRecordingSceneBridge();
  const root = createRoot("host", { bridge });

  root.render(
    React.createElement("view", {
      id: "root",
      className: "panel",
      style: { color: "red", opacity: 1 },
    }),
  );
  bridge.drain();
  root.drainCommittedBatches();

  root.render(React.createElement("view", { id: "root", style: { color: "blue" } }));

  expect(bridge.batches()).toEqual([
    {
      ops: [
        { type: "set-style", id: "root", name: "class", value: { kind: "null" } },
        { type: "set-style", id: "root", name: "color", value: { kind: "string", value: "blue" } },
        { type: "set-style", id: "root", name: "opacity", value: { kind: "null" } },
        { type: "commit" },
      ],
    },
  ]);
});

test("React reconciler clears removed generic props", () => {
  const bridge = createRecordingSceneBridge();
  const root = createRoot("host", { bridge });

  root.render(React.createElement("view", { id: "root", dataState: "ready", priority: 5 }));
  bridge.drain();
  root.drainCommittedBatches();

  root.render(React.createElement("view", { id: "root", dataState: "done" }));

  expect(bridge.batches()).toEqual([
    {
      ops: [
        { type: "set-prop", id: "root", name: "dataState", value: { kind: "string", value: "done" } },
        { type: "set-prop", id: "root", name: "priority", value: { kind: "null" } },
        { type: "commit" },
      ],
    },
  ]);
});

test("React reconciler emits move operations for keyed child reorders", () => {
  const bridge = createRecordingSceneBridge();
  const root = createRoot("host", { bridge });

  root.render(
    React.createElement(
      "view",
      { id: "root" },
      React.createElement("text", { id: "alpha", key: "alpha" }, "Alpha"),
      React.createElement("text", { id: "beta", key: "beta" }, "Beta"),
      React.createElement("text", { id: "gamma", key: "gamma" }, "Gamma"),
    ),
  );
  bridge.drain();
  root.drainCommittedBatches();

  root.render(
    React.createElement(
      "view",
      { id: "root" },
      React.createElement("text", { id: "gamma", key: "gamma" }, "Gamma"),
      React.createElement("text", { id: "alpha", key: "alpha" }, "Alpha"),
      React.createElement("text", { id: "beta", key: "beta" }, "Beta"),
    ),
  );

  expect(bridge.batches()).toEqual([
    {
      ops: [
        { type: "append-child", parent: "root", child: "alpha" },
        { type: "append-child", parent: "root", child: "beta" },
        { type: "commit" },
      ],
    },
  ]);
});

test("React reconciler emits deterministic update, removal, and unmount batches", () => {
  const bridge = createRecordingSceneBridge();
  const root = createRoot("host", { bridge });

  root.render(
    React.createElement(
      "hawk-view",
      { id: "root" },
      React.createElement("hawk-text", { id: "title" }, "Hello"),
      React.createElement("hawk-button", { id: "cta" }, "Go"),
    ),
  );
  bridge.drain();
  root.drainCommittedBatches();

  root.render(React.createElement("hawk-view", { id: "root" }, React.createElement("hawk-text", { id: "title" }, "Updated")));

  expect(bridge.batches()).toEqual([
    {
      ops: [
        { type: "remove-child", parent: "root", child: "cta" },
        { type: "dispose-subtree", id: "cta" },
        { type: "set-prop", id: "title", name: "text", value: { kind: "string", value: "Updated" } },
        { type: "commit" },
      ],
    },
  ]);

  bridge.drain();
  root.unmount();

  expect(bridge.batches()).toEqual([
    {
      ops: [
        { type: "dispose-subtree", id: "root" },
        { type: "commit" },
      ],
    },
  ]);
});

test("React reconciler state updates emit a second scene commit from an event handler", async () => {
  const bridge = createRecordingSceneBridge();
  const root = createRoot("host", { bridge });

  function Counter() {
    const [count, setCount] = useState(0);
    return React.createElement(
      "hawk-view",
      { id: "root" },
      React.createElement("hawk-text", { id: "count" }, String(count)),
      React.createElement(
        "hawk-button",
        { id: "increment", onPointerPress: () => setCount((value) => value + 1) },
        "Increment",
      ),
    );
  }

  root.render(React.createElement(Counter));

  bridge.drain();
  root.drainCommittedBatches();
  bridge.dispatch("increment", "pointer.press", {});
  await nextSchedulerTick();

  expect(bridge.batches()).toEqual([
    {
      ops: [
        { type: "set-prop", id: "count", name: "text", value: { kind: "string", value: "1" } },
        { type: "commit" },
      ],
    },
  ]);

  bridge.drain();
  root.unmount();

  expect(bridge.batches()).toEqual([
    {
      ops: [
        { type: "dispose-subtree", id: "root" },
        { type: "commit" },
      ],
    },
  ]);
  expect(() => bridge.dispatch("increment", "pointer.press", {})).toThrow("react.event.handler-missing");
});

test("React root render errors include component stack context", () => {
  const bridge = createRecordingSceneBridge();
  const root = createRoot("host", { bridge });

  function BrokenPanel() {
    throw new Error("broken panel");
  }

  let thrown: unknown;
  try {
    root.render(React.createElement(BrokenPanel));
  } catch (error) {
    thrown = error;
  }

  expect(thrown).toBeInstanceOf(Error);
  const error = thrown as Error;
  expect(error.message).toContain("broken panel");
  expect(error.message).toContain("react.error.component-stack");
  expect(error.message).toContain("BrokenPanel");
});

function nextSchedulerTick(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

test("React reconciler rejects unsupported host elements and missing ids", () => {
  const bridge = createRecordingSceneBridge();
  const root = createRoot("host", { bridge });

  expect(() => root.render(React.createElement("canvas", { id: "bad" }))).toThrow("react.element.unsupported");
  expect(() => root.render(React.createElement("hawk-view", null))).toThrow("react.node.id-required");
});

test("React reconciler rejects DOM-only props with actionable diagnostics", () => {
  const bridge = createRecordingSceneBridge();
  const root = createRoot("host", { bridge });

  expect(() =>
    root.render(
      React.createElement("view", {
        id: "root",
        dangerouslySetInnerHTML: { __html: "<span>browser</span>" },
      }),
    ),
  ).toThrow("react.dom.unsupported: prop `dangerouslySetInnerHTML` is DOM-only");
});

test("React root requires an explicit bridge outside the embedded Hawk2UI runtime", () => {
  expect(() => createRoot("host")).toThrow("react.scene-bridge.missing");
});
