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

test("Svelte compiler expands Svelte 5 snippets with scalar parameters and child snippets", () => {
  const output = compileHawkSvelte({
    filename: "App.svelte",
    source:
      '<script>let title = "Panel";</script>{#snippet Card(id, title, children)}<hawk-view id={id} class="card"><hawk-text id="card-title">{title}</hawk-text>{@render children()}</hawk-view>{/snippet}{#snippet body()}<hawk-button id="cta">Go</hawk-button>{/snippet}<hawk-view id="root">{@render Card("panel", title, body)}</hawk-view>',
  });

  expect(output.compilerArtifact.root.children).toHaveLength(1);
  const card = output.compilerArtifact.root.children[0].node;
  expect(card.id).toBe("panel");
  expect(card.kind).toBe("view");
  expect(card.style_refs).toEqual(["card"]);
  expect(card.children.map((child) => child.node.id)).toEqual(["card-title", "cta"]);
  expect(card.children[0].node.props).toEqual([{ name: "text", value: { type: "string", value: "Panel" } }]);
  expect(card.children[1].node.props).toEqual([{ name: "text", value: { type: "string", value: "Go" } }]);
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

test("Svelte compiler lowers onMount and onDestroy API calls to root lifecycle", () => {
  const output = compileHawkSvelte({
    filename: "App.svelte",
    source:
      '<script>import { onMount, onDestroy } from "svelte"; function mounted() {} function destroyed() {} onMount(mounted); onDestroy(destroyed);</script><hawk-view id="root"></hawk-view>',
  });

  expect(output.compilerArtifact.root.lifecycle).toContainEqual({ event: "mounted", handler: "mounted" });
  expect(output.compilerArtifact.root.lifecycle).toContainEqual({ event: "unmounted", handler: "destroyed" });
});

test("Svelte compiler preserves reactive declaration initial dynamic values", () => {
  const output = compileHawkSvelte({
    filename: "App.svelte",
    source:
      '<script>let ready = true; $: label = "Ready";</script><hawk-text id="root">{label}</hawk-text>',
  });

  expect(output.compilerArtifact.initial_dynamic_values).toContainEqual({
    name: "label",
    mode: "value",
    value: { type: "string", value: "Ready" },
  });
});

test("Svelte compiler emits runtime list templates for dynamic keyed each blocks", () => {
  const output = compileHawkSvelte({
    filename: "App.svelte",
    source:
      '<script>let items = [{ id: "alpha", label: "Alpha" }, { id: "beta", label: "Beta" }]; function handleItemPress() { items = [{ id: "gamma", label: "Gamma" }]; }</script><hawk-view id="root">{#each items as item (item.id)}<hawk-text id={item.id} on:pointerdown={handleItemPress}>{item.label}</hawk-text>{/each}</hawk-view>',
  });

  expect(output.compilerArtifact.root.children).toEqual([]);
  expect(output.compilerArtifact.list_templates).toEqual([
    {
      id: "root:items",
      parent_id: "root",
      source: "items",
      item: "item",
      key: "item.id",
      node: {
        id: { type: "expression", expression: "item.id" },
        kind: "text",
        key: { type: "expression", expression: "item.id" },
        props: [{ name: "text", value: { type: "expression", expression: "item.label" } }],
        refs: [],
        style_refs: [],
        asset_refs: [],
          events: [{ kind: "pointer.press", handler: "handleItemPress", payload_fields: ["position"] }],
          lifecycle: [],
          children: [],
        },
      },
    ]);
    expect(output.compilerArtifact.event_handlers.map((handler) => handler.name)).toEqual(["handleItemPress"]);
  });

test("Svelte compiler anchors runtime list templates before the next static sibling", () => {
  const output = compileHawkSvelte({
    filename: "App.svelte",
    source:
      '<script>let items = [{ id: "alpha", label: "Alpha" }];</script><hawk-view id="root"><hawk-text id="header">Header</hawk-text>{#each items as item (item.id + "-row")}<hawk-text id={item.id + "-row"}>{item.label + "!"}</hawk-text>{/each}<hawk-text id="footer">Footer</hawk-text></hawk-view>',
  });

  expect(output.compilerArtifact.root.children.map((child) => child.node.id)).toEqual(["header", "footer"]);
  expect(output.compilerArtifact.list_templates[0]?.anchor_before).toBe("footer");
  expect(output.compilerArtifact.list_templates[0]?.key).toBe('item.id + "-row"');
  expect(output.compilerArtifact.list_templates[0]?.node.id).toEqual({
    type: "expression",
    expression: 'item.id + "-row"',
  });
  expect(output.compilerArtifact.list_templates[0]?.node.props).toEqual([
    { name: "text", value: { type: "expression", expression: 'item.label + "!"' } },
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

test("Svelte compiler passes through arbitrary scalar props and dynamic prop bindings", () => {
  const output = compileHawkSvelte({
    filename: "App.svelte",
    source:
      '<script>let role = getRole();</script><hawk-view id="root"><hawk-button id="cta" aria_label="Start" tab_index={2} selected={true} data_role={role}>Go</hawk-button></hawk-view>',
  });
  const button = output.compilerArtifact.root.children[0].node;

  expect(button.props).toContainEqual({ name: "aria_label", value: { type: "string", value: "Start" } });
  expect(button.props).toContainEqual({ name: "tab_index", value: { type: "number", value: 2 } });
  expect(button.props).toContainEqual({ name: "selected", value: { type: "bool", value: true } });
  expect(output.compilerArtifact.dynamic_bindings).toContainEqual({
    node_id: "cta",
    target: { type: "prop", name: "data_role" },
    expression: "role",
    dependencies: ["role"],
  });
});

test("Svelte compiler maps on:click to pointer press", () => {
  const output = compileHawkSvelte({
    filename: "App.svelte",
    source:
      '<script>let label = "Idle"; function handleClick() { label = "Clicked"; }</script><hawk-view id="root"><hawk-button id="cta" on:click={handleClick}>Go</hawk-button></hawk-view>',
  });

  expect(output.compilerArtifact.root.children[0].node.events).toEqual([
    { kind: "pointer.press", handler: "handleClick", payload_fields: ["position"] },
  ]);
});

test("Svelte compiler lowers hawk-surface to a custom surface node", () => {
  const output = compileHawkSvelte({
    filename: "App.svelte",
    source: '<hawk-view id="root"><hawk-surface id="meter" surface_id="level-meter" /></hawk-view>',
  });

  const surface = output.compilerArtifact.root.children[0].node;
  expect(surface.kind).toBe("custom-surface");
  expect(surface.props).toContainEqual({ name: "surface_id", value: { type: "string", value: "level-meter" } });
});

test("Svelte compiler lowers if blocks to visible dynamic bindings", () => {
  const output = compileHawkSvelte({
    filename: "App.svelte",
    source:
      '<script>let showTitle = true;</script><hawk-view id="root">{#if showTitle}<hawk-text id="title">Title</hawk-text>{/if}</hawk-view>',
  });

  expect(output.compilerArtifact.root.children[0].node.id).toBe("title");
  expect(output.compilerArtifact.dynamic_bindings).toContainEqual({
    node_id: "title",
    target: { type: "prop", name: "visible" },
    expression: "showTitle",
    dependencies: ["showTitle"],
  });
});

test("Svelte compiler lowers else blocks to negated visible bindings", () => {
  const output = compileHawkSvelte({
    filename: "App.svelte",
    source:
      '<script>let showTitle = true;</script><hawk-view id="root">{#if showTitle}<hawk-text id="title">Title</hawk-text>{:else}<hawk-text id="fallback">Fallback</hawk-text>{/if}</hawk-view>',
  });

  expect(output.compilerArtifact.root.children.map((child) => child.node.id)).toEqual(["title", "fallback"]);
  expect(output.compilerArtifact.dynamic_bindings).toContainEqual({
    node_id: "fallback",
    target: { type: "prop", name: "visible" },
    expression: "!(showTitle)",
    dependencies: ["showTitle"],
  });
});

test("Svelte compiler lowers else-if chains to combined visible bindings", () => {
  const output = compileHawkSvelte({
    filename: "App.svelte",
    source:
      '<script>let showTitle = getShowTitle(); let showAlt = getShowAlt();</script><hawk-view id="root">{#if showTitle}<hawk-text id="title">Title</hawk-text>{:else if showAlt}<hawk-text id="alt">Alt</hawk-text>{:else}<hawk-text id="fallback">Fallback</hawk-text>{/if}</hawk-view>',
  });

  expect(output.compilerArtifact.root.children.map((child) => child.node.id)).toEqual(["title", "alt", "fallback"]);
  expect(output.compilerArtifact.dynamic_bindings.filter((binding) => binding.node_id === "alt")).toEqual([
    {
      node_id: "alt",
      target: { type: "prop", name: "visible" },
      expression: "(showAlt) && (!(showTitle))",
      dependencies: ["showAlt", "showTitle"],
    },
  ]);
  expect(output.compilerArtifact.dynamic_bindings.filter((binding) => binding.node_id === "fallback")).toEqual([
    {
      node_id: "fallback",
      target: { type: "prop", name: "visible" },
      expression: "(!(showAlt)) && (!(showTitle))",
      dependencies: ["showAlt", "showTitle"],
    },
  ]);
});

test("Svelte compiler accepts standard element aliases for native nodes", () => {
  const output = compileHawkSvelte({
    filename: "App.svelte",
    source: '<div id="root"><button id="cta">Go</button><p id="copy">Copy</p></div>',
  });

  expect(output.compilerArtifact.root.kind).toBe("view");
  expect(output.compilerArtifact.root.children.map((child) => [child.node.id, child.node.kind])).toEqual([
    ["cta", "button"],
    ["copy", "text"],
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

test("Svelte compiler parses with the modern AST contract", async () => {
  const source = await Bun.file("packages/hawk2ui-svelte/src/index.ts").text();

  expect(source).toContain("modern: true");
});
