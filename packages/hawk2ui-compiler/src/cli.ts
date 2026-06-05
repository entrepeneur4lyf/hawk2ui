#!/usr/bin/env bun
import { compileHawkSource, compilerArtifactJson, type HawkFrameworkKind } from "./index.ts";

declare const Bun: {
  readonly file: (path: string) => { readonly text: () => Promise<string> };
  readonly write: (path: string, data: string) => Promise<number>;
};

declare const process: {
  readonly argv: readonly string[];
  readonly stdout: { readonly write: (data: string) => void };
  readonly stderr: { readonly write: (data: string) => void };
  exitCode?: number;
};

interface CliOptions {
  readonly framework?: HawkFrameworkKind;
  readonly input: string;
  readonly out?: string;
}

async function main(): Promise<void> {
  const options = parseArgs(process.argv.slice(2));
  const source = await Bun.file(options.input).text();
  const output = compileHawkSource(
    options.framework
      ? { framework: options.framework, filename: options.input, source }
      : { filename: options.input, source },
  );
  const json = compilerArtifactJson(output);
  if (options.out) {
    await Bun.write(options.out, json);
  } else {
    process.stdout.write(json);
  }
}

function parseArgs(args: readonly string[]): CliOptions {
  let framework: HawkFrameworkKind | undefined;
  let input = "";
  let out: string | undefined;
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--framework") {
      framework = parseFramework(requiredValue(args, index, "--framework"));
      index += 1;
    } else if (arg === "--input") {
      input = requiredValue(args, index, "--input");
      index += 1;
    } else if (arg === "--out") {
      out = requiredValue(args, index, "--out");
      index += 1;
    } else if (arg === "--help" || arg === "-h") {
      throw new UsageRequested();
    } else {
      throw new Error(`compiler.cli.arg-unknown: unknown argument \`${String(arg)}\`.`);
    }
  }
  if (!input.trim()) {
    throw new Error("compiler.cli.input-required: pass --input <source-file>.");
  }
  const base = framework ? { framework, input } : { input };
  return out ? { ...base, out } : base;
}

function requiredValue(args: readonly string[], index: number, name: string): string {
  const value = args[index + 1];
  if (!value?.trim()) {
    throw new Error(`compiler.cli.value-required: ${name} requires a value.`);
  }
  return value;
}

function parseFramework(value: string): HawkFrameworkKind {
  if (value === "react" || value === "solid" || value === "svelte" || value === "vue") {
    return value;
  }
  throw new Error(`compiler.framework.unsupported: unsupported framework \`${value}\`.`);
}

class UsageRequested extends Error {}

function usage(): string {
  return "Usage: hawk2ui-compile --input <file> [--framework react|solid|svelte|vue] [--out artifact.json]\n";
}

main().catch((error: unknown) => {
  if (error instanceof UsageRequested) {
    process.stdout.write(usage());
    return;
  }
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`${usage()}hawk2ui-compile: ${message}\n`);
  process.exitCode = 1;
});
