# Svelte Runtime Bridge Design

## Goal
Make the Svelte 5 integration produce the same renderable runtime contract as native authoring. A valid Svelte Hawk component must compile into typed Svelte records, a bridged native runtime artifact, a `RuntimeViewTree`, and Skia-visible pixels through the existing runtime scene pipeline.

## Scope
This slice upgrades the existing Svelte integration path. It does not implement a full Svelte compiler. It supports the Hawk-native subset already represented by the fixtures: `hawk-view`, `hawk-text`, root attributes, keyed `{#each ... (item.id)}` children, refs, class/style refs, asset refs, pointer press, mount, and destroy lifecycle bindings.

## Rust API
Add a runtime-oriented API beside the existing record API:

- `SvelteIntegration::compile_to_runtime(source)` returns `SvelteRuntimeArtifact`.
- `SvelteRuntimeArtifact` contains the existing `SvelteCompiledArtifact` plus a `NativeRuntimeBridgeArtifact`.
- The runtime artifact exposes `runtime_tree()`, `metadata_for(id)`, `operation_keys()`, and `compiled()` accessors.

The existing `compile(source)` API remains stable for current callers.

## Mapping Rules
The Svelte integration builds a native authoring tree before invoking `NativeRuntimeBridge`.

Required mappings:

- Root `hawk-view` maps to `NativeAuthoringElement::new(root_id, ElementKind::View)`.
- `use:ref`, `class`, and `data-asset` map to native refs, style refs, and asset refs.
- `on:press`, `on:mount`, and `on:destroy` map to native event/lifecycle bindings.
- Keyed `hawk-text` fixture children map to `title` and `cta` text nodes in declaration order.
- Text children receive deterministic `text`, `font_size`, `color`, `width`, and `height` props so they are renderable by the runtime bridge.
- Root receives a deterministic dark background when no explicit background prop exists, so smoke fixtures render visible pixels.

## TypeScript Package
Update `packages/hawk2ui-svelte/src/index.ts` so `compileHawkSvelte` returns deterministic native record keys for supported source instead of `records: []`.

Required package behavior:

- Reject non-`.svelte` filenames.
- Reject unsafe asset paths using the same rule shape as Rust.
- Emit stable record strings for root mount, keyed children, refs, style refs, asset refs, pointer events, and lifecycle handlers.

## Error Handling
Rust diagnostics remain source-mapped to `SvelteSourceMap`.

Required error cases:

- Unsafe asset paths.
- Unresolved component marker `<Broken`.
- Bridge failures from invalid generated native props are converted into `svelte.runtime-bridge.failed` diagnostics.

## Pixel Proof
Tests must prove this complete path:

1. Svelte source fixture compiles with `compile_to_runtime`.
2. Runtime tree contains root and keyed text children.
3. Metadata preserves refs, styles, and assets.
4. Runtime scene bridge builds paint commands.
5. Skia snapshot contains visible background/accent/text pixels.

## Review Standard
After each task, ask: “As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability?” If revision is needed, take corrective action before continuing.
