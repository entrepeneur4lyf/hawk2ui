# Native Renderer Contract Bridge Design

## Goal
Build a production bridge from Hawk2UI authoring/framework records into the retained runtime scene pipeline. The bridge must make native authoring output renderable without framework-specific shortcuts: authoring records become `RuntimeViewTree`, `RuntimeSceneBridge` computes layout/scene/paint data, and Skia proves visible pixels.

## Scope
This slice covers the common native contract used by direct native authoring and framework integrations. It does not implement Svelte, React, Vue, or Solid compilers; those integrations will target this bridge after the contract is real.

## Inputs
The bridge accepts finalized authoring records from `hawk2ui-authoring`:

- `NativeAuthoringArtifact` as the first supported full artifact input.
- `NativeAuthoringElement` trees for direct unit conversion.
- Element IDs, element kinds, props, child order, style refs, asset refs, refs, and events already validated by authoring.

## Runtime Output
The bridge produces `hawk2ui_runtime::RuntimeViewTree`.

Mapping rules:

- `ElementKind::View`, `Button`, and generic structural controls become runtime nodes with `RuntimeVisual::Fill` when a background color is available, otherwise `RuntimeVisual::None`.
- `ElementKind::Text` becomes `RuntimeVisual::Text` using the `text` string prop when present.
- Layout defaults are deterministic and renderable: root fills the viewport, children use fixed measured defaults unless explicit width/height props are present.
- Children preserve author declaration order.
- Duplicate or invalid runtime IDs fail with structured diagnostics rather than silently overwriting nodes.

## Metadata
The first implementation keeps renderer-critical data in runtime nodes and returns metadata beside the runtime tree:

- Stable operation keys from authoring remain available for framework conformance.
- Style refs, asset refs, refs, and event bindings are preserved in a `NativeRuntimeBridgeArtifact` so framework adapters can consume the same contract later.
- Metadata does not affect layout or pixels until the style/assets/event verticals are wired into runtime execution.

## Error Handling
Bridge errors use a dedicated structured error type with stable rule strings.

Required error cases:

- Missing root artifact or element.
- Duplicate runtime node IDs.
- Unsupported element kind when no safe fallback exists.
- Invalid numeric layout prop values.
- Text node without usable text content should render as an empty text node, not fail.

## Pixel Proof
Tests must prove the whole path:

1. Build a native authoring artifact with a root view and text child.
2. Convert it to a runtime view tree.
3. Build a `RuntimeSceneFrame` from the tree.
4. Execute draw commands through `SkiaRendererBackend`.
5. Assert visible fill and text pixels in the snapshot.

## Boundaries
This bridge is not a framework compiler. Framework crates continue to parse their framework-specific source into normalized authoring records. Once this bridge is in place, each framework integration can be upgraded to emit or consume the shared native runtime contract.

## Review Standard
After each task, ask: “As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability?” If revision is needed, take corrective action before continuing.
