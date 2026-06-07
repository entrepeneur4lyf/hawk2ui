import { expect, test } from "bun:test";
import { compileHawkVue } from "../src/index.ts";
import { createHawkVueRenderer } from "../src/testkit.ts";

test("Vue compiler emits versioned native compiler artifacts from SFC templates", () => {
  const output = compileHawkVue({
    filename: "App.vue",
    source:
      '<script setup>const status = ref("idle"); function handlePress() { status.value = "pressed"; } function onMounted() { status.value = "mounted"; } function onUnmounted() { status.value = "unmounted"; } const items = [{ id: "title" }, { id: "cta" }];</script><template><hawk-view id="root" ref="root_ref" class="surface.card" data-asset="assets/logo.svg" @pointerdown="handlePress" @mounted="onMounted" @unmounted="onUnmounted"><hawk-text v-for="item in items" :id="item.id" :key="item.id">{{ item.id }}</hawk-text></hawk-view></template>',
  });

  expect(output.compilerArtifact.schema_version).toBe(1);
  expect(output.compilerArtifact.compiler).toEqual({
    framework: "vue",
    compiler: "@hawk2ui/vue",
    source_path: "App.vue",
    entrypoint: "default",
  });
  expect(output.compilerArtifact.root.id).toBe("root");
  expect(output.compilerArtifact.root.style_refs).toEqual(["surface.card"]);
  expect(output.compilerArtifact.root.children.map((child) => child.key)).toEqual(["title", "cta"]);
  expect(output.compilerArtifact.root.lifecycle).toEqual([
    { event: "mounted", handler: "onMounted" },
    { event: "unmounted", handler: "onUnmounted" },
  ]);
});

test("Vue compiler preserves dynamic text bindings from template interpolations", () => {
  const output = compileHawkVue({
    filename: "App.vue",
    source:
      '<script setup>const label = computed(() => "Title");</script><template><hawk-view id="root"><hawk-text id="title">{{ label }}</hawk-text></hawk-view></template>',
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
      value: { type: "string", value: "Title" },
    },
  ]);
});

test("Vue compiler emits executable pointer handler actions", () => {
  const output = compileHawkVue({
    filename: "App.vue",
    source:
      '<script setup>const label = ref("Idle"); function handlePress() { label.value = "Pressed"; }</script><template><hawk-view id="root" @pointerdown="handlePress"><hawk-text id="title">{{ label }}</hawk-text></hawk-view></template>',
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

test("Vue compiler lowers v-model to value binding and value-changed handler", () => {
  const output = compileHawkVue({
    filename: "App.vue",
    source:
      '<script setup>const label = ref("Idle");</script><template><hawk-view id="root"><hawk-text id="title" v-model="label"></hawk-text></hawk-view></template>',
  });

  expect(output.compilerArtifact.dynamic_bindings).toContainEqual({
    node_id: "title",
    target: { type: "prop", name: "value" },
    expression: "label",
    dependencies: ["label"],
  });
  expect(output.compilerArtifact.root.children[0]?.node.events).toContainEqual({
    kind: "input.value-changed",
    handler: "title:v-model",
    payload_fields: ["value"],
  });
  expect(output.compilerArtifact.event_handlers).toContainEqual({
    name: "title:v-model",
    actions: [
      {
        type: "set_dynamic_expression",
        name: "label",
        expression: "event.value",
        dependencies: ["event"],
      },
    ],
  });
});

test("Vue compiler preserves event payload handler expressions", () => {
  const output = compileHawkVue({
    filename: "App.vue",
    source:
      '<script setup>const label = ref("Idle"); function handlePress(event: { x: number; y: number }) { label.value = event.x + ":" + event.y; }</script><template><hawk-view id="root" @pointerdown="handlePress"><hawk-text id="title">{{ label }}</hawk-text></hawk-view></template>',
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

test("Vue compiler expands local component templates with props and default slots", () => {
  const cardTemplate = '<hawk-view :id="id" class="card"><hawk-text id="card-title">{{ title }}</hawk-text><slot /></hawk-view>';
  const output = compileHawkVue({
    filename: "App.vue",
    source:
      `<script setup>const Card = { props: ["id", "title"], template: ${JSON.stringify(cardTemplate)} };</script><template><hawk-view id="root"><Card id="panel" title="Panel"><hawk-button id="cta">Go</hawk-button></Card></hawk-view></template>`,
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

test("Vue compiler lowers the complete native event and lifecycle contract from template directives", () => {
  const output = compileHawkVue({
    filename: "App.vue",
    source:
      '<script setup>const status = ref("idle"); function markPointerPress() { status.value = "pointer.press"; } function markPointerRelease() { status.value = "pointer.release"; } function markPointerMove() { status.value = "pointer.move"; } function markPointerDrag() { status.value = "pointer.drag"; } function markPointerEnter() { status.value = "pointer.enter"; } function markPointerLeave() { status.value = "pointer.leave"; } function markPointerWheel() { status.value = "pointer.wheel"; } function markKeyDown() { status.value = "keyboard.key-down"; } function markKeyUp() { status.value = "keyboard.key-up"; } function markTextInput() { status.value = "keyboard.text-input"; } function markFocusIn() { status.value = "focus.focus-in"; } function markFocusOut() { status.value = "focus.focus-out"; } function markValueChanged() { status.value = "input.value-changed"; } function markValueCommitted() { status.value = "input.value-committed"; } function markResize() { status.value = "resize"; } function markMounted() { status.value = "mounted"; } function markSuspended() { status.value = "suspended"; } function markResumed() { status.value = "resumed"; } function markHotReloaded() { status.value = "hot-reloaded"; } function markErrorBoundary() { status.value = "error-boundary"; } function markShutdown() { status.value = "shutdown"; } function markUnmounted() { status.value = "unmounted"; }</script><template><hawk-view id="root" @pointerdown="markPointerPress" @pointerup="markPointerRelease" @pointermove="markPointerMove" @pointerdrag="markPointerDrag" @pointerenter="markPointerEnter" @pointerleave="markPointerLeave" @wheel="markPointerWheel" @keydown="markKeyDown" @keyup="markKeyUp" @textinput="markTextInput" @focus="markFocusIn" @blur="markFocusOut" @input="markValueChanged" @change="markValueCommitted" @resize="markResize" @mounted="markMounted" @suspended="markSuspended" @resumed="markResumed" @hot-reloaded="markHotReloaded" @error-boundary="markErrorBoundary" @shutdown="markShutdown" @unmounted="markUnmounted"></hawk-view></template>',
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

test("Vue compiler lowers onMounted and onUnmounted API calls to root lifecycle", () => {
  const output = compileHawkVue({
    filename: "App.vue",
    source:
      '<script setup>function mounted() {} function destroyed() {} onMounted(mounted); onUnmounted(destroyed);</script><template><hawk-view id="root"></hawk-view></template>',
  });

  expect(output.compilerArtifact.root.lifecycle).toContainEqual({ event: "mounted", handler: "mounted" });
  expect(output.compilerArtifact.root.lifecycle).toContainEqual({ event: "unmounted", handler: "destroyed" });
});

test("Vue compiler preserves reactive object initial dynamic values", () => {
  const output = compileHawkVue({
    filename: "App.vue",
    source:
      '<script setup>const state = reactive({ label: "Ready", active: true });</script><template><hawk-text id="root">{{ state.label }}</hawk-text></template>',
  });

  expect(output.compilerArtifact.initial_dynamic_values).toContainEqual({
    name: "state",
    mode: "value",
    value: {
      type: "object",
      value: {
        label: { type: "string", value: "Ready" },
        active: { type: "bool", value: true },
      },
    },
  });
});

test("Vue compiler preserves watch calls as effect reactivity bindings", () => {
  const output = compileHawkVue({
    filename: "App.vue",
    source:
      '<script setup>const label = ref("Idle"); function syncLabel() { label.value = "Synced"; } watch(label, syncLabel);</script><template><hawk-text id="root">{{ label }}</hawk-text></template>',
  });

  expect(output.compilerArtifact.reactivity).toContainEqual({
    kind: "effect",
    name: "watch:label:syncLabel",
  });
});

test("Vue compiler emits runtime list templates for dynamic v-for sources", () => {
  const output = compileHawkVue({
    filename: "App.vue",
    source:
      '<script setup>const items = ref([{ id: "alpha", label: "Alpha" }, { id: "beta", label: "Beta" }]); function handleItemPress() { items.value = [{ id: "gamma", label: "Gamma" }]; }</script><template><hawk-view id="root"><hawk-text v-for="item in items" :id="item.id" :key="item.id" @pointerdown="handleItemPress">{{ item.label }}</hawk-text></hawk-view></template>',
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

test("Vue compiler anchors runtime list templates before the next static sibling", () => {
  const output = compileHawkVue({
    filename: "App.vue",
    source:
      '<script setup>const items = ref([{ id: "alpha", label: "Alpha" }]);</script><template><hawk-view id="root"><hawk-text id="header">Header</hawk-text><hawk-text v-for="item in items" :id="item.id + \'-row\'" :key="item.id + \'-row\'">{{ item.label + "!" }}</hawk-text><hawk-text id="footer">Footer</hawk-text></hawk-view></template>',
  });

  expect(output.compilerArtifact.root.children.map((child) => child.node.id)).toEqual(["header", "footer"]);
  expect(output.compilerArtifact.list_templates[0]?.anchor_before).toBe("footer");
  expect(output.compilerArtifact.list_templates[0]?.key).toBe("item.id + '-row'");
  expect(output.compilerArtifact.list_templates[0]?.node.id).toEqual({
    type: "expression",
    expression: "item.id + '-row'",
  });
  expect(output.compilerArtifact.list_templates[0]?.node.props).toEqual([
    { name: "text", value: { type: "expression", expression: 'item.label + "!"' } },
  ]);
});

test("Vue compiler preserves dynamic layout prop bindings from template bindings", () => {
  const output = compileHawkVue({
    filename: "App.vue",
    source:
      '<script setup>const panelWidth = computed(() => 240); const panelHeight = computed(() => 120);</script><template><hawk-view id="root"><hawk-view id="panel" :width="panelWidth" :height="panelHeight"></hawk-view></hawk-view></template>',
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

test("Vue compiler preserves dynamic visual prop bindings from template bindings", () => {
  const output = compileHawkVue({
    filename: "App.vue",
    source:
      '<script setup>const panelBackground = computed(() => "#111111"); const titleSize = computed(() => 18); const titleColor = computed(() => "#ffffff");</script><template><hawk-view id="root"><hawk-view id="panel" :background="panelBackground"></hawk-view><hawk-text id="title" :font_size="titleSize" :color="titleColor">Title</hawk-text></hawk-view></template>',
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

test("Vue compiler passes through arbitrary scalar props and dynamic prop bindings", () => {
  const output = compileHawkVue({
    filename: "App.vue",
    source:
      '<script setup>const role = getRole();</script><template><hawk-view id="root"><hawk-button id="cta" aria_label="Start" :tab_index="2" :selected="true" :data_role="role">Go</hawk-button></hawk-view></template>',
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

test("Vue compiler maps @click to pointer press", () => {
  const output = compileHawkVue({
    filename: "App.vue",
    source:
      '<script setup>const label = ref("Idle"); function handleClick() { label.value = "Clicked"; }</script><template><hawk-view id="root"><hawk-button id="cta" @click="handleClick">Go</hawk-button></hawk-view></template>',
  });

  expect(output.compilerArtifact.root.children[0].node.events).toEqual([
    { kind: "pointer.press", handler: "handleClick", payload_fields: ["position"] },
  ]);
});

test("Vue compiler lowers hawk-surface to a custom surface node", () => {
  const output = compileHawkVue({
    filename: "App.vue",
    source:
      '<template><hawk-view id="root"><hawk-surface id="meter" surface_id="level-meter" /></hawk-view></template>',
  });

  const surface = output.compilerArtifact.root.children[0].node;
  expect(surface.kind).toBe("custom-surface");
  expect(surface.props).toContainEqual({ name: "surface_id", value: { type: "string", value: "level-meter" } });
});

test("Vue compiler lowers v-if and v-show to visible dynamic bindings", () => {
  const output = compileHawkVue({
    filename: "App.vue",
    source:
      '<script setup>const showTitle = ref(true); const showStatus = computed(() => true);</script><template><hawk-view id="root"><hawk-text v-if="showTitle" id="title">Title</hawk-text><hawk-text v-show="showStatus" id="status">Status</hawk-text></hawk-view></template>',
  });

  expect(output.compilerArtifact.root.children.map((child) => child.node.id)).toEqual(["title", "status"]);
  expect(output.compilerArtifact.dynamic_bindings).toContainEqual({
    node_id: "title",
    target: { type: "prop", name: "visible" },
    expression: "showTitle",
    dependencies: ["showTitle"],
  });
  expect(output.compilerArtifact.dynamic_bindings).toContainEqual({
    node_id: "status",
    target: { type: "prop", name: "visible" },
    expression: "showStatus",
    dependencies: ["showStatus"],
  });
});

test("Vue compiler lowers v-else-if and v-else chains to combined visible bindings", () => {
  const output = compileHawkVue({
    filename: "App.vue",
    source:
      '<script setup>const showTitle = ref(true); const showAlt = ref(false);</script><template><hawk-view id="root"><hawk-text v-if="showTitle" id="title">Title</hawk-text><hawk-text v-else-if="showAlt" id="alt">Alt</hawk-text><hawk-text v-else id="fallback">Fallback</hawk-text></hawk-view></template>',
  });

  expect(output.compilerArtifact.root.children.map((child) => child.node.id)).toEqual(["title", "alt", "fallback"]);
  expect(output.compilerArtifact.dynamic_bindings.filter((binding) => binding.node_id === "alt")).toEqual([
    {
      node_id: "alt",
      target: { type: "prop", name: "visible" },
      expression: "(!(showTitle)) && (showAlt)",
      dependencies: ["showTitle", "showAlt"],
    },
  ]);
  expect(output.compilerArtifact.dynamic_bindings.filter((binding) => binding.node_id === "fallback")).toEqual([
    {
      node_id: "fallback",
      target: { type: "prop", name: "visible" },
      expression: "!(showTitle) && !(showAlt)",
      dependencies: ["showTitle", "showAlt"],
    },
  ]);
});

test("Vue compiler accepts standard element aliases for native nodes", () => {
  const output = compileHawkVue({
    filename: "App.vue",
    source:
      '<template><div id="root"><button id="cta">Go</button><p id="copy">Copy</p></div></template>',
  });

  expect(output.compilerArtifact.root.kind).toBe("view");
  expect(output.compilerArtifact.root.children.map((child) => [child.node.id, child.node.kind])).toEqual([
    ["cta", "button"],
    ["copy", "text"],
  ]);
});

test("Vue renderer renders, patches, removes children, and unmounts deterministically", () => {
  const renderer = createHawkVueRenderer();
  const target = { id: "host" };

  renderer.render(
    {
      id: "root",
      ref: "root_ref",
      class: "surface.card",
      asset: "assets/logo.svg",
      on: ["pointer.press", "keyboard.key-down"],
      children: [
        { id: "title", key: "title", text: "Title" },
        { id: "cta", kind: "button", key: "cta", text: "Go", on: ["pointer.release"] },
      ],
    },
    target,
  );

  expect(renderer.records).toEqual([
    "mount-element:root",
    "ref:root:root_ref",
    "style:root:surface.card",
    "asset:root:assets/logo.svg",
    "bind-event:root:pointer.press",
    "bind-event:root:keyboard.key-down",
    "mount-element:title",
    "prop:title:text=Title",
    "mount-element:cta",
    "bind-event:cta:pointer.release",
    "prop:cta:text=Go",
  ]);
  const initialSnapshot = renderer.snapshot(target);
  expect(initialSnapshot?.events?.map((event) => event.kind)).toEqual([
    "pointer.press",
    "keyboard.key-down",
  ]);
  expect(initialSnapshot?.children?.[1]?.kind).toBe("button");
  expect(initialSnapshot?.children?.[1]?.events?.[0]?.kind).toBe("pointer.release");

  renderer.render(
    {
      id: "root",
      ref: "root_ref",
      class: "surface.card emphasis",
      asset: "assets/logo.svg",
      on: ["pointer.press", "keyboard.key-down"],
      children: [{ id: "title", key: "title", text: "Updated" }],
    },
    target,
  );

  expect(renderer.records.slice(11)).toEqual([
    "style:root:surface.card emphasis",
    "prop:title:text=Updated",
    "remove-element:cta",
  ]);

  renderer.unmount(target);
  expect(renderer.records.at(-1)).toBe("unmount-element:root");
  expect(renderer.snapshot(target)).toBeUndefined();
});

test("Vue renderer rejects duplicate keyed children", () => {
  const renderer = createHawkVueRenderer();

  expect(() =>
    renderer.render(
      {
        id: "root",
        children: [
          { id: "first", key: "title" },
          { id: "second", key: "title" },
        ],
      },
      { id: "host" },
    ),
  ).toThrow("vue.child-key.duplicate");
});
