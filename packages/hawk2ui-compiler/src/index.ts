import { compileHawkSolid, type HawkSolidCompileOutput } from "../../hawk2ui-solid/src/index.ts";
import { compileHawkSvelte, type HawkSvelteCompileOutput } from "../../hawk2ui-svelte/src/index.ts";
import { compileHawkVue, type HawkVueCompileOutput } from "@hawk2ui/vue/compiler";

export type HawkFrameworkKind = "react" | "solid" | "svelte" | "vue";

export interface HawkCompileSourceInput {
  readonly framework?: HawkFrameworkKind;
  readonly filename: string;
  readonly source: string;
}

export type HawkFrameworkCompileOutput =
  | HawkSolidCompileOutput
  | HawkSvelteCompileOutput
  | HawkVueCompileOutput;

export function compileHawkSource(input: HawkCompileSourceInput): HawkFrameworkCompileOutput {
  const framework = input.framework ?? inferFramework(input.filename);
  switch (framework) {
    case "react":
      throw new Error(
        "compiler.framework.react-runtime: React production uses @hawk2ui/react createRoot with the sealed Deno runtime; the legacy React compiler is not a production compiler path.",
      );
    case "solid":
      return compileHawkSolid(input);
    case "svelte":
      return compileHawkSvelte(input);
    case "vue":
      return compileHawkVue(input);
  }
}

export function compilerArtifactJson(output: HawkFrameworkCompileOutput): string {
  return `${JSON.stringify(output.compilerArtifact, null, 2)}\n`;
}

function inferFramework(filename: string): HawkFrameworkKind {
  if (filename.endsWith(".svelte")) return "svelte";
  if (filename.endsWith(".vue")) return "vue";
  if (/\.[jt]sx$/.test(filename)) {
    throw new Error("compiler.framework.required: TSX/JSX inputs require --framework react or --framework solid.");
  }
  throw new Error(`compiler.framework.unsupported: cannot infer framework for \`${filename}\`.`);
}
