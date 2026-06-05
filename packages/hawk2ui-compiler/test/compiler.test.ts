import { expect, test } from "bun:test";
import { compileHawkSource, compilerArtifactJson } from "../src/index.ts";

test("framework compiler dispatch emits canonical compiler artifacts", () => {
  const fixtures = [
    {
      framework: "react" as const,
      filename: "App.tsx",
      source:
        'const label = getLabel(); export function App() { return <hawk-view id="root"><hawk-text id="title">{label}</hawk-text></hawk-view>; }',
      expression: "label",
    },
    {
      framework: "solid" as const,
      filename: "App.tsx",
      source:
        'const [label] = createSignal("Title"); export function App() { return <hawk-view id="root"><hawk-text id="title">{label()}</hawk-text></hawk-view>; }',
      expression: "label()",
    },
    {
      framework: "svelte" as const,
      filename: "App.svelte",
      source:
        '<script>let label = getLabel();</script><hawk-view id="root"><hawk-text id="title">{label}</hawk-text></hawk-view>',
      expression: "label",
    },
    {
      framework: "vue" as const,
      filename: "App.vue",
      source:
        '<script setup>const label = computed(() => "Title");</script><template><hawk-view id="root"><hawk-text id="title">{{ label }}</hawk-text></hawk-view></template>',
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
