import { expect, test } from "bun:test";
import { compileHawkSvelte } from "../src/index.ts";

test("Svelte compiler emits lifecycle, child props, and deterministic records", () => {
  const output = compileHawkSvelte({
    filename: "App.svelte",
    source:
      '<script>let status = "idle"; function handlePress() { status = "pressed"; } function onMount() { status = "mounted"; } function onDestroy() { status = "unmounted"; }</script><hawk-view id="root" use:root_ref class="surface.card" data-asset="assets/logo.svg" on:press={handlePress} on:mount={onMount} on:destroy={onDestroy}><hawk-text id="title">Title</hawk-text><hawk-button id="cta">Go</hawk-button></hawk-view>',
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
  expect(output.compilerArtifact.compiler).toEqual({
    framework: "svelte",
    compiler: "@hawk2ui/svelte",
    source_path: "App.svelte",
    entrypoint: "default",
  });
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

test("Svelte compiler preserves dynamic text bindings from template expressions", () => {
  const output = compileHawkSvelte({
    filename: "App.svelte",
    source:
      '<script>let label = "Live";</script><hawk-view id="root"><hawk-text id="title">{label}</hawk-text></hawk-view>',
  });

  expect(output.compilerArtifact.dynamic_bindings).toEqual([
    {
      node_id: "title",
      target: { type: "prop", name: "text" },
      expression: "label",
      dependencies: ["label"],
    },
  ]);
  expect(output.compilerArtifact.initial_dynamic_values).toEqual([
    {
      name: "label",
      mode: "value",
      value: { type: "string", value: "Live" },
    },
  ]);
});

test("Svelte compiler emits executable pointer handler actions", () => {
  const output = compileHawkSvelte({
    filename: "App.svelte",
    source:
      '<script>let label = "Idle"; function handlePress() { label = "Pressed"; }</script><hawk-view id="root" on:press={handlePress}><hawk-text id="title">{label}</hawk-text></hawk-view>',
  });

  expect(output.compilerArtifact.event_handlers).toEqual([
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
});

test("Svelte compiler preserves event payload handler expressions", () => {
  const output = compileHawkSvelte({
    filename: "App.svelte",
    source:
      '<script>let label = "Idle"; function handlePress(event) { label = event.x + ":" + event.y; }</script><hawk-view id="root" on:pointerdown={handlePress}><hawk-text id="title">{label}</hawk-text></hawk-view>',
  });

  expect(output.compilerArtifact.event_handlers).toEqual([
    {
      name: "handlePress",
      actions: [
        {
          type: "set_dynamic_expression",
          name: "label",
          expression: 'event.x + ":" + event.y',
          dependencies: ["event"],
        },
      ],
    },
  ]);
});

test("Svelte compiler lowers the complete native event and lifecycle contract from directives", () => {
  const output = compileHawkSvelte({
    filename: "App.svelte",
    source:
      '<script>let status = "idle"; function markPointerPress() { status = "pointer.press"; } function markPointerRelease() { status = "pointer.release"; } function markPointerMove() { status = "pointer.move"; } function markPointerDrag() { status = "pointer.drag"; } function markPointerEnter() { status = "pointer.enter"; } function markPointerLeave() { status = "pointer.leave"; } function markPointerWheel() { status = "pointer.wheel"; } function markKeyDown() { status = "keyboard.key-down"; } function markKeyUp() { status = "keyboard.key-up"; } function markTextInput() { status = "keyboard.text-input"; } function markFocusIn() { status = "focus.focus-in"; } function markFocusOut() { status = "focus.focus-out"; } function markValueChanged() { status = "input.value-changed"; } function markValueCommitted() { status = "input.value-committed"; } function markResize() { status = "resize"; } function markMounted() { status = "mounted"; } function markSuspended() { status = "suspended"; } function markResumed() { status = "resumed"; } function markHotReloaded() { status = "hot-reloaded"; } function markErrorBoundary() { status = "error-boundary"; } function markShutdown() { status = "shutdown"; } function markUnmounted() { status = "unmounted"; }</script><hawk-view id="root" on:pointerdown={markPointerPress} on:pointerup={markPointerRelease} on:pointermove={markPointerMove} on:pointerdrag={markPointerDrag} on:pointerenter={markPointerEnter} on:pointerleave={markPointerLeave} on:wheel={markPointerWheel} on:keydown={markKeyDown} on:keyup={markKeyUp} on:textinput={markTextInput} on:focus={markFocusIn} on:blur={markFocusOut} on:input={markValueChanged} on:change={markValueCommitted} on:resize={markResize} on:mount={markMounted} on:suspend={markSuspended} on:resume={markResumed} on:hot-reloaded={markHotReloaded} on:error-boundary={markErrorBoundary} on:shutdown={markShutdown} on:destroy={markUnmounted}></hawk-view>',
  });

  expect(output.compilerArtifact.root.events.map((event) => event.kind)).toEqual([
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
  expect(output.compilerArtifact.root.lifecycle.map((lifecycle) => lifecycle.event)).toEqual([
    "mounted",
    "suspended",
    "resumed",
    "hot-reloaded",
    "error-boundary",
    "shutdown",
    "unmounted",
  ]);
});

test("Svelte compiler preserves dynamic layout prop bindings from template expressions", () => {
  const output = compileHawkSvelte({
    filename: "App.svelte",
    source:
      '<script>let panelWidth = getWidth(); let panelHeight = getHeight();</script><hawk-view id="root"><hawk-view id="panel" width={panelWidth} height={panelHeight}></hawk-view></hawk-view>',
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

test("Svelte compiler preserves dynamic visual prop bindings from template expressions", () => {
  const output = compileHawkSvelte({
    filename: "App.svelte",
    source:
      '<script>let panelBackground = getSurface(); let titleSize = getSize(); let titleColor = getColor();</script><hawk-view id="root"><hawk-view id="panel" background={panelBackground}></hawk-view><hawk-text id="title" font_size={titleSize} color={titleColor}>Title</hawk-text></hawk-view>',
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

test("Svelte compiler rejects duplicate child ids", () => {
  expect(() =>
    compileHawkSvelte({
      filename: "App.svelte",
      source: '<hawk-view id="root"><hawk-text id="title">A</hawk-text><hawk-text id="title">B</hawk-text></hawk-view>',
    }),
  ).toThrow("svelte.child-id.duplicate");
});
