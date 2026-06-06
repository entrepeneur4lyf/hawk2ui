import { expect, test } from "bun:test";
import { compileHawkSource, compilerArtifactJson } from "../src/index.ts";

test("framework compiler dispatch emits canonical compiler artifacts", () => {
  const fixtures = [
    {
      framework: "react" as const,
      filename: "App.tsx",
      source:
        'let label = "Idle"; function handlePress() { label = "Pressed"; } export function App() { return <hawk-view id="root" onPointerDown={handlePress}><hawk-text id="title">{label}</hawk-text></hawk-view>; }',
      expression: "label",
    },
    {
      framework: "solid" as const,
      filename: "App.tsx",
      source:
        'const [label, setLabel] = createSignal("Idle"); function handlePress() { setLabel("Pressed"); } export function App() { return <hawk-view id="root" onPointerDown={handlePress}><hawk-text id="title">{label()}</hawk-text></hawk-view>; }',
      expression: "label()",
    },
    {
      framework: "svelte" as const,
      filename: "App.svelte",
      source:
        '<script>let label = "Idle"; function handlePress() { label = "Pressed"; }</script><hawk-view id="root" on:press={handlePress}><hawk-text id="title">{label}</hawk-text></hawk-view>',
      expression: "label",
    },
    {
      framework: "vue" as const,
      filename: "App.vue",
      source:
        '<script setup>const label = ref("Idle"); function handlePress() { label.value = "Pressed"; }</script><template><hawk-view id="root" @pointerdown="handlePress"><hawk-text id="title">{{ label }}</hawk-text></hawk-view></template>',
      expression: "label",
    },
  ];

  for (const fixture of fixtures) {
    const output = compileHawkSource(fixture);
    expect(output.compilerArtifact.schema_version).toBe(1);
    expect(output.compilerArtifact.root.id).toBe("root");
    expect(output.compilerArtifact.dynamic_bindings).toEqual([
      {
        node_id: "title",
        target: { type: "prop", name: "text" },
        expression: fixture.expression,
        dependencies: ["label"],
      },
    ]);
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
    expect(JSON.parse(compilerArtifactJson(output))).toEqual(output.compilerArtifact);
  }
});

test("framework compiler dispatch requires explicit framework for TSX", () => {
  expect(() =>
    compileHawkSource({
      filename: "App.tsx",
      source: 'export function App() { return <hawk-view id="root"></hawk-view>; }',
    }),
  ).toThrow("compiler.framework.required");
});
