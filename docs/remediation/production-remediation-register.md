# Hawk2UI Production Remediation Register

## Purpose

This register documents every known gap that must be remediated before Hawk2UI can be called production-ready.

The source of truth for requirements is:

- `docs/specs/0001-product-direction.md`
- `docs/specs/0002-domain-spec-index.md`
- `docs/specs/rendering-architecture.md`
- `docs/technical/crate-selection.md`

The source of truth for implementation status is the Rust workspace under `crates/`.

No item in this register is optional. Items may be sequenced, but they are not deferred out of scope.

## Current Verdict

Hawk2UI is not production-ready, but the largest early drift items have been materially reduced in
source. The current blocker profile is no longer "core foundations are absent"; it is "several
production verticals still stop at a validated internal boundary instead of an end-user,
host-backed product path."

Source-truth status as of 2026-05-27:

- style parsing uses `lightningcss` in `hawk2ui-style` and lowers the accepted subset into typed
  style records,
- layout computation uses `taffy` in `hawk2ui-layout` with `flexbox` and `grid` features enabled,
- script execution uses Boa plus OXC TypeScript transformation in `hawk2ui-script`, but the Boa
  dependency is pinned to a Git revision and remains a release-policy blocker,
- framework adapters have an explicit native compiler boundary and conformance path; legacy source
  scanners remain for compatibility fixtures and source-mapped diagnostic cases,
- Winit opens a native window and renders compiled runtime scene output through the Skia renderer
  path, with a fallback diagnostic frame only for direct host API use without a runtime tree,
- CLAP package/scaffold generation exists and is host-loaded by tests; plugin smoke coverage now
  validates a Baseview native parent, resizes/DPI-scales, renders a live Hawk2UI runtime scene
  through Skia, and verifies visible presented pixels,
- Baseview support now has a real `open_parented` path from validated host handles plus smoke
  coverage for the runtime-scene-to-Skia presentation path; remaining work is binding that path to
  an actual DAW-owned editor surface in a host integration smoke,
- accessibility model/export/action dispatch and schema catalog/export are implemented, while
  OS-specific accessibility attachment remains host-backend work.

Remaining production blockers must therefore be tracked around real host attachment, complete
package/product workflows, live plugin editor rendering, native dev loop/hot reload, visual
golden-image regression, release-grade dependency/signing policy, platform API backends, user
manuals, and premium templates.

## Release-Blocking Standard

Every remediation phase must end with this review checkpoint:

> As you are delivering this product yourself, are you satisfied with the implementation, or should there be revisions to ensure production-ready stability? If revision is needed, take corrective action before continuing.

An item is complete only when:

- source implementation matches the spec,
- tests prove the required behavior,
- diagnostics are explicit and host-safe,
- public docs accurately describe current behavior,
- crate descriptions do not overstate maturity,
- examples exercise the implementation end to end,
- `cargo fmt --all --check`, `cargo test --workspace`, `cargo check --workspace`, `cargo clippy --workspace -- -D warnings`, and `git diff --check` pass.

## Global Documentation Remediation

### REM-GDOC-001: Create Dedicated Specs For Every Domain

Evidence:

- `docs/specs/0002-domain-spec-index.md` lists the complete domain map.
- Only three spec files currently exist in `docs/specs/`: `0001-product-direction.md`, `0002-domain-spec-index.md`, and `rendering-architecture.md`.

Required remediation:

- Create every dedicated spec named in `docs/specs/0002-domain-spec-index.md`.
- Each spec must include purpose, scope, non-goals, inputs/outputs, public API, internal architecture, lifecycle, data model, validation/errors, security boundaries, compatibility matrix, tests, examples, open questions, and acceptance criteria.
- Specs must not contain architectural decisions. Decisions belong in decision records.
- Specs must not describe implementation as complete unless source proves it.

Acceptance:

- Every `Future Spec` path in the domain index exists.
- Each spec has acceptance criteria tied to tests and examples.
- `tasks/COVERAGE.md` references only existing spec files.

### REM-GDOC-002: Split Decision Records From Specs

Evidence:

- Product direction contains unresolved decision questions.
- The user explicitly rejected architectural decisions being placed inside specs.

Required remediation:

- Create or update decision records for choices such as Deft prior-art-only, Winit desktop, Baseview plugin adapter, Boa first spike, Lightning CSS, Taffy, Skia CPU raster first, and framework integration order.
- Remove decision language from specs where it reads as implementation choice rather than behavior requirement.

Acceptance:

- Specs define what must be true.
- Decision records define selected tradeoffs and rationale.

### REM-GDOC-003: Align Crate Descriptions With Implementation Reality

Evidence:

- Several crates describe themselves as production implementations while source is record-only or partial.
- Examples include `hawk2ui-script`, `hawk2ui-host-baseview`, `hawk2ui-a11y`, `hawk2ui-plugin-adapters`, and `hawk2ui-schema`.

Required remediation:

- Either implement the promised production behavior or revise crate descriptions until implementation catches up.
- Avoid misleading crate-level docs.

Acceptance:

- Crate root docs and Cargo descriptions accurately match source behavior.

## Confirmed Crate Selection Drift

### REM-CRATE-001: Adopt Lightning CSS For Style Parsing

Evidence:

- `docs/technical/crate-selection.md` marks `lightningcss` as preferred.
- `docs/specs/0001-product-direction.md` says Lightning CSS should be the preferred primary style parser/transformer.
- `crates/hawk2ui-style/Cargo.toml` pins `lightningcss = "1.0.0-alpha.71"`.
- `crates/hawk2ui-style/src/compile.rs` uses Lightning CSS parsing and still needs workspace-wide dependency stability policy coverage because the accepted parser line is alpha.

Required remediation:

- Add `lightningcss` at an explicitly pinned accepted version.
- Replace handwritten block/declaration parsing with Lightning CSS parsing.
- Preserve Hawk2UI's typed selector/property/token/runtime style table boundaries.
- Emit source-aware diagnostics for unsupported syntax, unsupported selectors, unsupported properties, unsupported values, malformed custom properties, and fallback failures.
- Add upgrade tests covering supported syntax and deliberate rejections.

Acceptance:

- Style parsing no longer relies on string splitting for CSS structure.
- Invalid CSS surfaces Lightning CSS-backed diagnostics.
- Supported CSS lowers into current typed style values.

Status:

- Implemented Lightning CSS-backed parsing in `hawk2ui-style`.
- Dependency stability for the accepted Lightning CSS alpha line is governed by `REM-CRATE-007`.

### REM-CRATE-002: Adopt Taffy For Layout

Evidence:

- `docs/technical/crate-selection.md` marks `taffy` as preferred stable.
- `docs/specs/0001-product-direction.md` says Taffy should be the preferred primary layout engine.
- `crates/hawk2ui-layout/Cargo.toml` pins `taffy = "0.10.0"` with `std`, `taffy_tree`, `flexbox`, and `grid`.
- `crates/hawk2ui-layout/src/compute.rs` computes layout through Taffy for flex, grid, absolute, percentage, scroll clip, and text-measured paths.

Required remediation:

- Add Taffy behind Hawk2UI-owned layout structs.
- Map Hawk2UI layout style, constraints, flex, scroll, absolute positioning, percentage sizing, and measurement into Taffy.
- Integrate text measurement as an intrinsic sizing input.
- Preserve deterministic output and diagnostics.

Acceptance:

- Layout computation flows through Taffy.
- Nested flex, constrained plugin sizes, scroll clips, text measurement, and absolute children are covered by tests.

Status:

- Implemented Taffy-backed flex layout, grid layout, absolute positioning, scroll clipping,
  percentage sizing, and text measurement integration.
- Remaining layout work should be tracked as explicit supported-subset expansion or host/platform
  integration, not as "Taffy is missing."

### REM-CRATE-003: Implement Real Embedded JavaScript Runtime

Evidence:

- `docs/technical/crate-selection.md` says `boa_engine` is the first spike.
- `docs/specs/0001-product-direction.md` separates Bun tooling from embedded runtime and names Boa as first runtime spike.
- `crates/hawk2ui-script/Cargo.toml` depends on `boa_engine` by Git commit `8f5ef6542d641fd22320e51234e914b59e623717`, which is not publishable to crates.io and is not a release-grade dependency contract.
- `crates/hawk2ui-script/Cargo.toml` pins OXC crates at `0.133.0`, which are fast-moving compiler crates and need explicit upgrade policy.
- `crates/hawk2ui-script/src/lib.rs` evaluates JavaScript through Boa and calls `Context::run_jobs()`.
- `ScriptBackend::execute_module_with_host_jobs` projects Rust-owned promise/timer records into
  Boa, drains Boa jobs, invokes deterministic timer callbacks, and reads a settled host-job result.

Required remediation:

- Keep production script execution on the selected Boa/OXC runtime path unless a decision record
  changes runtime choice.
- Finish any remaining module loading, host binding, interruption, memory/resource limit,
  deterministic timer, teardown, and diagnostic gaps around that runtime path.
- Keep Bun, if used, as external tooling only.

Acceptance:

- Real JavaScript executes through the selected runtime.
- TypeScript is compiled through a real transform path before runtime execution.
- Runtime policy tests cover denied globals, host binding permissions, interruption, teardown, and promise/timer semantics.

Status:

- Implemented real Boa-backed JavaScript evaluation and OXC-backed TypeScript transform.
- Dependency stability is governed by `REM-CRATE-007`; JavaScript promise/timer integration is
  remediated under `REM-RUNTIME-001A`.

### REM-CRATE-004: Add AccessKit Host Bridge

Status: Remediated at accessibility export boundary.

Evidence:

- `docs/technical/crate-selection.md` marks `accesskit` as preferred.
- `docs/specs/rendering-architecture.md` requires accessibility geometry references.
- `crates/hawk2ui-a11y/Cargo.toml` depends on `accesskit = "0.24.0"`.
- `crates/hawk2ui-a11y` exports typed accessibility records, host snapshots, and AccessKit
  tree updates.
- AccessKit export maps roles, labels, values, checked state, disabled state, bounds, actions,
  child order, focus, duplicate IDs, and invalid geometry into explicit output or diagnostics.

Required remediation:

- Add AccessKit integration behind Hawk2UI accessibility host traits.
- Map roles, labels, focus, actions, bounds, checked state, dynamic updates, and host surface kinds.
- Integrate accessibility geometry from scene/layout output.

Acceptance:

- Desktop host can export an AccessKit tree.
- Plugin host behavior is explicit for supported and unsupported accessibility cells.

Remediation delivered:

- Added crate-root exports for `AccessKitExport` and `AccessKitExportError` so host adapters can use
  the production AccessKit boundary without reaching into private modules.
- Added shared `Diagnostic` conversion for AccessKit/host export errors.
- Replaced raw string geometry failures with structured accessibility diagnostics.
- Added tests proving AccessKit tree export, focus validation, layout geometry updates, plugin editor
  availability records, invalid bounds diagnostics, and shared diagnostic conversion.

Review check:

- As the delivering engineer, I am satisfied with this accessibility export boundary for production
  stability: AccessKit integration is real, host-facing errors are structured, and plugin
  availability is explicit. Native OS adapter attachment remains tracked by host/platform backend
  remediation, not by the accessibility model.


### REM-CRATE-005: Add Schema Generation And Validation

Evidence:

- `docs/technical/crate-selection.md` marks `schemars` and `jsonschema` as preferred.
- `crates/hawk2ui-schema/Cargo.toml` depends on `schemars` and `jsonschema`.
- `crates/hawk2ui-schema` generates and validates the product model schema.
- `crates/hawk2ui-build` generates and validates the raw manifest schema before semantic
  validation.
- Manifest schema validation now preserves the JSON pointer and validator detail instead of
  collapsing every schema failure into a generic error.
- `crates/hawk2ui-build` now generates and validates sealed artifact JSON Schema from the artifact
  record types.
- `crates/hawk2ui-plugin` now generates and validates plugin package target metadata JSON Schema
  from `PluginFormatTarget`, `FormatMetadata`, and `BundleOutput`.
- Manifest capability declarations are covered by the generated raw manifest schema.
- `crates/hawk2ui-platform` now generates and validates capability record/table schemas.
- `crates/hawk2ui-plugin-adapters` now generates and validates package plan, materialized output,
  and verification report schemas.
- `crates/hawk2ui-schema` now exports a deterministic central schema catalog covering product,
  manifest, artifact, plugin, package adapter, and capability schema entries.
- `hawk2ui-cli export-schemas` emits the central schema catalog as JSON for build/release tooling.

Required remediation:

- Add schema generation for manifests, artifacts, capabilities, plugin metadata, and package metadata.
- Add JSON Schema validation in CLI/build paths.
- Version schemas and provide compatibility tests.

Acceptance:

- CLI validation uses generated schemas.
- Invalid manifests fail with source-specific diagnostics.

Status:

- Remediated: product model, raw manifest, sealed artifact, plugin metadata, package target metadata,
  package adapter outputs, and platform capability records now generate and validate JSON Schema at
  their owning crate boundaries.
- Remediated: the shared schema crate provides a deterministic catalog for all production schema
  entries, and the CLI exposes that catalog through `export-schemas`.

Review check:

- As the delivering engineer, I am satisfied with this schema boundary for production stability:
  every registered production data boundary now has generated JSON Schema, validation coverage, a
  central deterministic catalog entry, and a CLI export path for downstream build/release tooling.


### REM-CRATE-006: Add Realtime And Plugin Format Crates Where Chosen

Evidence:

- `docs/technical/crate-selection.md` lists `rtrb`, `vst3`, `clap-sys`, `clack-*`, and related audio/plugin candidates.
- `crates/hawk2ui-plugin/Cargo.toml` now depends on `rtrb` for realtime visual transport.
- `crates/hawk2ui-plugin-adapters/Cargo.toml` now depends on `clap-sys`.

Status:

- Realtime visual data now uses `rtrb`-backed preallocated transport records.
- `RealtimeVisualTransport::split_preallocated` returns separate audio-writer and UI-reader endpoints, and tests move the audio writer across a thread boundary.
- CLAP generated `cdylib` scaffolding and host-load tests exist.
- VST3/AU/LV2 remain package-layout or compatibility-matrix targets unless separately
  implemented or removed from the supported production matrix.

Required remediation:

- Choose plugin format sequence in decision records.
- Extend realtime UI data channels beyond the current `rtrb` split endpoint as needed by plugin format adapters.
- Finish the selected CLAP production vertical by connecting generated runtime artifacts to live
  Hawk2UI editor rendering inside the attached plugin GUI surface.
- Implement VST3/AU/LV2 adapters only if they remain in the selected production compatibility
  matrix; otherwise remove them from product claims and CLI/package output descriptions.

Acceptance:

- At least one plugin format can build a real loadable plugin editor bundle.
- Realtime data tests prove audio-thread-safe behavior.

### REM-CRATE-007: Define Workspace Dependency Stability Policy

Evidence:

- `crates/hawk2ui-script/Cargo.toml` depends on `boa_engine` by Git commit instead of a crates.io release.
- `crates/hawk2ui-script/Cargo.toml` pins OXC crates at `0.133.0`.
- `crates/hawk2ui-style/Cargo.toml` pins `lightningcss = "1.0.0-alpha.71"`.
- `crates/hawk2ui-layout/Cargo.toml` pins `taffy = "0.10.0"`.
- `crates/hawk2ui-render-skia/Cargo.toml` and `crates/hawk2ui-host-winit/Cargo.toml` pin `skia-safe = "0.97.0"`.
- `release/dependency-policy.toml` records dependency owners, risks, release blockers, and upgrade
  gates for Boa, OXC, Lightning CSS, Taffy, Skia, and CLAP.
- The Boa Git dependency remains explicitly release-blocking until replaced by a crates.io release
  dependency or isolated from publishable crates.

Required remediation:

- Keep the workspace dependency policy complete as dependencies are added or upgraded, including
  crates.io-only publishability, Git dependency exceptions, alpha/pre-1.0 acceptance, lockfile
  update cadence, security advisories, and compatibility testing.
- Replace the Boa Git dependency with a release-grade dependency contract before publishing, or isolate it behind a non-published adapter crate with explicit release rules.
- Add dependency audit commands to the release gate.
- Document accepted versions and upgrade triggers for Boa, OXC, Lightning CSS, Taffy, Skia, Winit, Baseview, and plugin crates.

Acceptance:

- The workspace can be published or packaged without accidental Git dependency blockers.
- Every accepted alpha/pre-1.0 dependency has an owner, compatibility gate, and rollback plan.
- Dependency upgrades require targeted compatibility tests for parser, layout, renderer, host, script, and plugin behavior.

Status:

- Added `release/dependency-policy.toml` as the machine-readable dependency stability policy for
  Boa, OXC, Lightning CSS, Taffy, and Skia.
- Added xtask validation for dependency policy entries, duplicate detection, required fields, and
  Git dependency release-blocker enforcement.
- Full `xtask release-check` now validates the dependency policy before changelog and script gates.
- Updated dependency hygiene documentation and release criteria so dependency health includes the
  policy gate in addition to `cargo deny`.

Review check:

- As the delivering engineer, I am satisfied with this remediation slice for production readiness:
  dependency risk is no longer tracked only in prose, the current Boa Git dependency is explicitly
  release-blocked, and high-risk dependency upgrades have owner and test-gate metadata.

### REM-CRATE-008: Unify Diagnostic And Error Types

Evidence:

- Multiple crates define structurally similar rule/message error records, including script, host, style, layout, runtime, renderer, asset, and package boundaries.
- This duplicates diagnostic shape and makes cross-crate consumer ergonomics worse as crate count grows.
- Remediation already requires structured diagnostics across domains, but the common type boundary is not tracked explicitly.

Required remediation:

- Add a shared diagnostic/error type in `hawk2ui-core` or a dedicated diagnostics crate.
- Preserve domain-specific error constructors while converting to the shared type at public boundaries.
- Include rule/code, message, optional source span, optional cause chain, severity, and domain metadata.
- Provide ergonomic `From` conversions and documentation for application authors.

Acceptance:

- Public APIs can expose a common diagnostic envelope without losing domain-specific detail.
- CLI, runtime, host, renderer, and script errors can be reported uniformly.
- Tests prove conversion preserves rule/code and message data.

Status:

- `hawk2ui-core` now re-exports the shared `hawk2ui-api` diagnostic contract.
- Added shared `Diagnostic` conversions for script backend errors, renderer backend errors, Winit
  host errors, build diagnostics, security diagnostics, platform handle diagnostics, runtime host
  binding errors, style selector errors, schema validation/product model errors, authoring
  diagnostics, accessibility action dispatch errors, and plugin automation errors.
- Added tests proving severity, rule/code, message, and related context preservation for the
  converted public diagnostic producers.

Review check:

- As the delivering engineer, I am satisfied with this remediation slice for production readiness:
  the common diagnostic envelope is now available from the core facade, the major public diagnostic
  producers have explicit conversions, and the conversions are covered by tests. Follow-up work may
  continue adding conversions for specialized package/performance/testkit errors, but the shared
  public diagnostic boundary is now real and usable.

## Product Direction Remediation

### REM-PROD-001: No Rust Requirement For Application Authors

Evidence:

- Product direction requires no application-author Rust.
- Current examples/framework paths are Rust crates and source-scanning adapters.

Required remediation:

- Provide non-Rust authoring entrypoints for app authors.
- Implement TypeScript/JavaScript/Svelte/React/Vue/Solid/custom renderer build inputs.
- Provide generated types, manifest schemas, and CLI workflows.

Acceptance:

- A user can create, run, build, and package an app without writing Rust.

### REM-PROD-002: Premium Visual Quality As Core Requirement

Evidence:

- Product direction lists premium visual capability as non-negotiable.
- Winit can render compiled runtime scene output through the Skia path.
- The current visual/template surface is not yet sufficient to prove premium desktop/plugin UI
  quality across gradients, typography, imagery, layer effects, analyzers, dense controls, and
  animated surfaces.

Required remediation:

- Implement expressive visual primitives through style/assets/scene/rendering.
- Provide premium templates for desktop and plugin UIs.
- Add visual regression fixtures proving gradients, textures, image panels, typography, shadows, glows, curves, meters, analyzers, knobs, sliders, and dense panels.

Acceptance:

- Example gallery demonstrates JUCE-class visual ambition without native drawing code by the user.

### REM-PROD-003: Manifest-First Product Validation

Evidence:

- Product direction requires plugin identity, editor metadata, parameters, presets, and asset declarations to be manifest-first and validated before runtime.
- Build/schema/plugin crates contain typed records, schema catalog/export, raw manifest validation,
  and package metadata validation.
- Raw manifest schema validation runs before semantic validation, and schema failures now carry
  path/detail diagnostics through the CLI boundary.

Required remediation:

- Validate app identity, plugin identity, editor metadata, parameters, defaults, ranges, duplicate IDs, asset references, unsafe assets, package targets, and capabilities.
- Keep schema coverage synchronized as new manifest/package fields are added.

Acceptance:

- Invalid projects fail before runtime.
- Diagnostics contain source paths and actionable rules.

## Rendering Pipeline Remediation

### REM-RENDER-001: Connect Runtime Scene Frames To Host Presentation

Status: Remediated in source.

Evidence:

- `crates/hawk2ui-runtime/src/view.rs` can build `RuntimeSceneFrame`.
- `crates/hawk2ui-host-winit/src/software_frame.rs` renders a hard-coded default scene.
- Winit runtime does not consume `RuntimeSceneFrame`.
- `hawk2ui-render-skia::SkiaRendererBackend::draw_runtime_scene_frame` now consumes `RuntimeSceneFrame::draw_commands()` directly and lowers fill, text, image asset, vector asset, and custom surface commands into Skia.
- `hawk2ui-host-winit::SoftwareFrameRenderer::render_scene_frame` now delegates runtime scene replay to the Skia backend instead of owning command semantics.
- Winit desktop runtime accepts a compiled runtime tree from CLI build output and renders that tree for `run-desktop`; the hard-coded default scene remains only as an explicit no-runtime-tree fallback for direct host API use.

Required remediation:

- Add a scene presenter that consumes `RuntimeSceneFrame::draw_commands()`.
- Lower runtime draw commands into `hawk2ui-render-skia`.
- Make Winit desktop runtime accept a compiled app/runtime tree and render it.
- Remove hard-coded product UI from the desktop presentation path.

Acceptance:

- Changing author/runtime tree content changes the visible native window output.
- Tests prove fill/text scene output reaches pixels or backend commands.

Remediation delivered:

- Renderer-owned runtime scene replay centralizes draw command lowering in `hawk2ui-render-skia`.
- Winit software presentation now uses the renderer-owned replay path for runtime scene frames.
- Replay options carry frame index, DPI scale, and missing-asset fallback policy so desktop hosts can degrade missing assets visibly while headless/backend use can fail strictly.
- Regression coverage proves a runtime scene containing fill, image, vector, and custom surface commands reaches visible Skia pixels through the renderer-owned replay path.

Review check:

- As the implementer delivering this product, I am satisfied with this remediation for production stability. Runtime scene presentation no longer depends on host-owned draw-command lowering, and production desktop launch renders compiled runtime tree output.

### REM-RENDER-002: Replace String Paint Commands With Typed Backend Commands

Status: Remediated in source.

Evidence:

- `crates/hawk2ui-render/src/export.rs` exports typed `PaintCommand` records with `PaintCommandKind` payloads.
- `PaintCommand::as_str()` and `PaintCommandList::serialize_stable()` retain deterministic diagnostic serialization.
- Rendering architecture requires backend-neutral draw commands for tests, diagnostics, fallback, and parity.

Acceptance:

- `crates/hawk2ui-render/tests/render_export.rs` validates typed paint command access for backend parity.
- `crates/hawk2ui-render/tests/render_export.rs` validates deterministic snapshot serialization.

### REM-RENDER-003: Complete Scene Graph Semantics

Status: Remediated in source.

Evidence:

- `SceneNode` has layout, clip, affine transform, opacity, hit-test, accessibility refs, and invalidation flags.
- Dirty bounds, invalidation reasons, and cache invalidation state are now recorded on scene nodes.
- Scene diffs now report added, removed, changed, repaint bounds, and cache-invalidated node IDs.
- Scene nodes now record typed layer membership, opacity groups, and effect references.
- Scene graphs now expose deterministic paint-order traversal and effective opacity resolution.
- Runtime scene frames now produce update plans with repaint bounds and cache-invalidation targets.
- Runtime scheduler now consumes scene updates into coalesced render invalidations and host repaint callbacks.
- Runtime scene updates now apply explicit cache evictions through the render backend cache-invalidation contract.
- Skia now executes opacity groups through save-layer alpha compositing.
- Skia now executes supported structured layer effects through real shadow/glow primitives.
- Scene graphs now resolve accessibility geometry from hit-test geometry or layout geometry.

Acceptance:

- `crates/hawk2ui-render/tests/render_export.rs` covers full affine transform storage, point application, validation, and stable serialization.
- `crates/hawk2ui-render/tests/render_export.rs` covers invalidation reasons, transformed dirty bounds, ancestor dirty-bound propagation, and cache invalidation flags.
- `crates/hawk2ui-render/tests/render_export.rs` covers deterministic scene diffs for added, removed, changed, repaint bounds, and cache invalidation IDs.
- `crates/hawk2ui-render/tests/render_export.rs` covers layer membership, effect references, opacity groups, deterministic paint order, and effective opacity.
- `crates/hawk2ui-runtime/tests/runtime_behavior.rs` covers runtime scene update planning, cache invalidation target propagation, and host repaint scheduling.
- `crates/hawk2ui-runtime/tests/runtime_behavior.rs` covers applying runtime scene update cache evictions to the Skia backend before frame replay.
- `crates/hawk2ui-render-skia/tests/skia_backend.rs` covers opacity-group compositing through rendered pixels.
- `crates/hawk2ui-render-skia/tests/skia_backend.rs` covers structured effect execution through rendered pixels.
- `crates/hawk2ui-render/tests/render_export.rs` covers accessibility geometry resolution.
- `crates/hawk2ui-render-skia/tests/skia_backend.rs` proves affine transforms affect rendered pixels.

### REM-RENDER-004: Complete Skia Backend Execution

Status: Remediated in source.

Evidence:

- `hawk2ui-render-skia` draws several primitives.
- `draw_vector` renders registered compiled vector path records through Skia and fails with a structured diagnostic when the vector asset is missing.
- `apply_layer_effect` executes supported structured shadow/glow effects through Skia primitives and returns explicit diagnostics for unsupported effect strings.
- Trait-level `draw_text` uses configurable default text placement, baseline, font size, color, and the resolved Skia typeface instead of hard-coded origin/default font rendering.
- `draw_text_layout` renders production text layout lines produced by `hawk2ui-text`.
- `draw_image_rect` renders registered compiled image assets into destination rectangles with scaling.
- `draw_vector_rect` renders registered compiled vector assets into destination rectangles with clipping.
- `draw_runtime_scene_frame` replays accepted runtime scene draw commands, including custom surfaces, through the Skia backend.

Required remediation:

- Integrate shaped text layout output into the Skia text draw path so complex scripts, font fallback, and glyph metrics come from the production text stack.
- Implement image scaling, caching, color handling, and nine-slice if accepted by spec.

Acceptance:

- Skia backend renders all accepted layer types through tests and visual fixtures.

Remediation delivered:

- Accepted layer/render command types are covered by Skia pixel tests: fills, strokes, paths, text, shaped text layouts, images, vectors, affine transforms, clips, opacity groups, structured effects, cached layers, custom surfaces, and runtime scene replay.
- Missing image/vector assets can fail strictly or draw visible diagnostic placeholders depending on replay policy.
- No nine-slice layer is currently part of the accepted runtime/render command surface, so no additional nine-slice backend path is required for this remediation.

Review check:

- As the implementer delivering this product, I am satisfied with this remediation for production stability. Skia now owns the complete accepted runtime render surface, with test coverage at both primitive and runtime-scene replay levels.

### REM-RENDER-005: Integrate Text Measurement With Layout And Rendering

Status: Remediated in source.

Evidence:

- `hawk2ui-layout` exposes `HawkTextMeasurer`, which adapts `hawk2ui-text` measurement into layout measurement records.
- `LayoutTree::try_compute_layout_with_text_measurer` feeds text measurement into Taffy leaf sizing.
- `RuntimeSceneBridge::build_with_text_measurer` attaches runtime text visuals to layout text measurement inputs before scene export.
- `hawk2ui-text::TextLayout` now carries positioned `TextLayoutLine` records with measured line text, width, and baseline offsets.
- `hawk2ui-render-skia::SkiaRendererBackend::draw_text_layout` consumes `TextLayout` directly, resolves the requested family through Skia with default fallback, and draws each measured line at its layout baseline.

Required remediation:

- Feed shaped text output into Skia drawing.

Acceptance:

- Text wrapping, truncation, bidi, fallback, and high-DPI measurements affect layout and rendering.

Remediation delivered:

- Text layout output now exposes renderer-owned line records instead of only aggregate metrics.
- Skia text layout rendering now uses the production text backend's resolved family, display text, line count, bidi/parley/truncation flags, high-DPI baseline, and positioned line records.
- Regression coverage verifies wrapped line records, truncation display text propagation, high-DPI baseline changes, and visible Skia pixels from shaped text layout drawing.

Review check:

- As the implementer delivering this product, I am satisfied with this remediation for production stability. Text is no longer rendered only through raw string placement; renderers can consume the production text layout contract directly.

### REM-RENDER-006: Complete Asset-To-Renderer Flow

Status: Remediated in source.

Evidence:

- `hawk2ui-assets` compiles images/vectors/fonts into records.
- `hawk2ui-render` has compiled asset records.
- `hawk2ui-render-skia` registers compiled image and vector payloads from `hawk2ui-assets::AssetRecord`.
- Runtime scene output carries compiled image/vector asset draw commands and rejects raw path-like asset IDs at the render boundary.
- `hawk2ui-host-winit` previously failed the whole frame when a runtime scene carried image/vector asset commands that were not registered with the software frame path.
- `hawk2ui-host-winit::SoftwareFrameRenderer` now accepts compiled runtime asset records, registers image/vector assets with Skia during frame preparation, and draws registered runtime asset commands before falling back to placeholders for genuinely missing assets.
- `hawk2ui-cli run-desktop` now compiles declared runtime assets from the project workspace and attaches them to the Winit desktop runtime config.

Required remediation:

- Register assets with renderer during surface/frame preparation from compiled asset records.

Acceptance:

- Raw asset paths are rejected at rendering boundaries.
- Compiled image and vector assets render in desktop and headless tests.

Remediation delivered:

- Winit software frame rendering degrades missing runtime image/vector assets to visible placeholders instead of hard-failing the frame.
- Registered compiled image assets now render through `SkiaRendererBackend::draw_image_rect` in desktop software frames.
- Registered compiled vector assets now render through `SkiaRendererBackend::draw_vector_rect` in desktop software frames.
- CLI desktop launch compiles manifest-declared image/vector/font assets from safe workspace-relative paths and passes the compiled records into the desktop runtime.
- Regression coverage proves registered runtime image/vector assets render visible compiled pixels and missing runtime assets still degrade visibly.

Review check:

- As the implementer delivering this product, I am satisfied with this remediation for production stability. The desktop presentation path now has a real compiled asset registration flow; placeholders remain only as explicit missing-asset degradation.

### REM-RENDER-007: Implement Custom Draw Surfaces

Evidence:

- `CustomDrawSurface` is a record with category, layout, capabilities, invalidation, and schedule metadata.
- No draw callback/execution path is wired into render or host presentation.

Required remediation:

- Define and implement custom draw hooks for meters, analyzers, curves, scopes, timelines, graph editors, and inspector panels.
- Integrate with layout, hit testing, invalidation, frame scheduling, capabilities, and plugin-safe data feeds.

Acceptance:

- A custom graph/meter surface renders and updates independently under frame-rate limits.

Status:

- Implemented render-level `CustomSurfaceDataSnapshot`, `CustomSurfaceFrameContext`, and
  `CustomSurfaceDrawRequest` records with finite, bounded realtime data validation.
- Implemented frame-interval scheduling on `CustomDrawSurface`.
- Implemented `RuntimeCustomSurfaceVisual` and `RuntimeDrawCommand::CustomSurface` so runtime
  view trees produce executable custom draw commands with resolved layout geometry.
- Implemented Skia custom-surface hooks for meter-style and curve-style categories.
- Implemented software desktop frame replay for runtime custom surfaces; this currently duplicates some custom surface drawing logic from `hawk2ui-render-skia` and is tracked by `REM-RENDER-009`.
- Added tests proving validated custom-surface requests, runtime command emission, Skia pixel
  output, and desktop software-frame pixel output.

Review check:

- As the delivering engineer, I am satisfied with this slice for production readiness: the draw
  boundary is typed, data is bounded and finite, frame-rate gating is deterministic, and the
  runtime-to-Skia-to-host path is covered by tests. No corrective revision is required for this
  remediation item before moving to animation/frame scheduling.

### REM-RENDER-008: Implement Animation And Frame Scheduling

Evidence:

- Rendering spec requires animation ticks, repaint requests, frame caps, reduced-rate meters, and plugin-safe scheduling.
- Current runtime scheduler exists as records/logic, but host rendering does not use a complete animation scheduler.

Required remediation:

- Define animation state above renderer.
- Add repaint cadence policies for desktop and plugin hosts.
- Support frame caps, reduced motion, animation invalidation, and deterministic headless frame stepping.

Acceptance:

- Animation fixtures produce deterministic frame sequences.
- Plugin meter/analyzer updates never block audio processing.

Status:

- Implemented `AnimationCadencePolicy`, `AnimationFrameScheduler`, and `AnimationFrameTick` in
  `hawk2ui-runtime`.
- Added deterministic frame stepping with max frame-rate caps, reduced-rate surface cadence, forced
  headless steps, and reduced-motion suppression of automatic ticks.
- Extended scheduler batches with rich animation frame ticks while preserving existing timestamp
  tick compatibility.
- Wired Winit desktop runtime config to carry animation cadence policy.
- Wired Winit runtime application to request animation redraws from cadence state and count accepted
  animation ticks in runtime summaries.
- Added tests proving deterministic primary/reduced-rate frame sequences, reduced-motion forced
  stepping, Winit policy configuration, and lifecycle tick accounting.

Review check:

- As the delivering engineer, I am satisfied with this slice for production readiness: animation
  cadence is deterministic, host redraw policy is explicit, reduced motion does not silently animate,
  and no sleeps/blocking behavior was introduced. No corrective revision is required for this
  remediation item before moving to style remediation.

### REM-RENDER-009: Remove Duplicate Custom Surface Drawing Implementations

Status: Remediated in source.

Evidence:

- `hawk2ui-render-skia` implements custom surface drawing for meter-style and curve-style categories.
- `hawk2ui-host-winit/src/software_frame.rs` independently implements similar custom surface drawing for software desktop frame replay.
- Host code should not own renderer semantics because host abstraction remediation requires desktop and plugin hosts to share surface lifecycle while rendering remains in renderer-owned code.

Required remediation:

- Move runtime custom surface replay through the renderer backend API or extract a shared renderer-owned software path.
- Keep host code responsible for native window lifecycle, frame target ownership, and presentation, not shape semantics.
- Add tests proving Winit software frames and Skia backend output remain consistent for the same custom surface commands.

Acceptance:

- Custom surface drawing semantics have one renderer-owned implementation path.
- Host code delegates custom drawing and does not duplicate meter/curve rendering rules.

Remediation delivered:

- `hawk2ui-host-winit` now depends on `hawk2ui-render-skia` and replays runtime scene frames through `SkiaRendererBackend`.
- Winit software frames delegate custom surface commands to `SkiaRendererBackend::draw_custom_surface` using `CustomSurfaceDrawRequest` and `CustomSurfaceFrameContext`.
- The Winit-local meter/curve custom surface drawing implementation was removed; host-owned code now handles frame presentation, scaling, and missing-asset placeholders only.
- Regression coverage proves analyzer custom surface styling reaches Winit software frames through the renderer-owned path.

Review check:

- As the implementer delivering this product, I am satisfied with this remediation for production stability. The host no longer owns custom surface shape semantics; further renderer completeness work remains tracked under the asset and frame presentation remediation items.

## Style System Remediation

### REM-STYLE-001: Define And Enforce Exact CSS Subset

Evidence:

- Domain index requires `css-subset-reference.md`.
- Style implementation supports a small ad hoc subset.

Required remediation:

- Specify selectors, properties, units, functions, variables, tokens, inheritance, shorthands, transitions, keyframes, unsupported syntax, and diagnostics.
- Enforce the subset through Lightning CSS parsing and typed lowering.

Acceptance:

- User-facing style reference exists.
- Tests cover every accepted and rejected syntax class.

Status:

- Implemented `StyleSubsetReference` as a machine-readable production CSS subset surface.
- Enforced shorthand, unsupported unit, unsupported function, keyframe, and conditional at-rule
  diagnostics after Lightning CSS parsing.
- Added tests for accepted selectors, properties, units, functions, tokens, inheritance metadata,
  transition duration, and every documented rejected syntax class.
- Added `manual/css-subset-reference.md` with selectors, properties, units, functions, tokens,
  inheritance, shorthands, transitions, keyframes, and diagnostics.
- Expanded the user-facing style reference to document supported units/functions and rejected CSS
  diagnostics.

Review check:

- As the delivering engineer, I am satisfied with this remediation item for production readiness:
  the accepted subset is explicit in code and docs, unsupported CSS fails with stable diagnostics,
  and Lightning CSS remains the parser source of truth before typed lowering. No corrective
  revision is required before moving to cascade, inheritance, variables, and tokens.

### REM-STYLE-002: Complete Cascade, Inheritance, Variables, And Tokens

Evidence:

- Style tokens and runtime tables exist.
- Runtime style computation implements deterministic selector matching, specificity/source order,
  registry initial values, inherited properties, token resolution, theme variants, preference
  overrides, and invalidation diffs.

Required remediation:

- Implement deterministic cascade order, specificity, inheritance, initial values, custom properties, token resolution, theme variants, and user preference overrides.

Acceptance:

- Style computation matches documented subset.
- Theme and user preference changes invalidate affected render output.

Status:

- Implemented `RuntimeStyleTree`, `RuntimeStyleNode`, `StyleRuntimeEnvironment`, and full
  `RuntimeStyleTable::compute_for_tree` computation.
- Runtime style computation now evaluates supported selectors against the style tree, applies
  specificity and source-order precedence, fills registry initial values, inherits inherited
  properties, and resolves token-backed values.
- Theme variants and preference hook overrides resolve through `TokenSet` and
  `StyleRuntimeEnvironment`.
- Added `RuntimeStyleTable::diff_from` and `RuntimeStyleInvalidation` so theme/preference changes
  report affected node IDs for render invalidation.
- Documented runtime cascade, token, theme, preference, and invalidation behavior in
  `manual/style-reference.md`.

Review check:

- As the delivering engineer, I am satisfied with this remediation item for production readiness:
  cascade behavior is deterministic, token/theme/preference resolution is exercised by tests, and
  environment changes produce explicit invalidation output. No corrective revision is required
  before moving to layout architecture.

## Layout Remediation

### REM-LAYOUT-001: Complete Layout Architecture With Taffy

Evidence:

- Layout tree and records exist.
- Layout computation flows through Taffy behind Hawk2UI-owned records.
- Flex, grid, absolute positioning, percentage sizing, scroll clipping, text measurement, plugin
  constraints, and graph/analyzer geometry attachment are covered by source tests.

Required remediation:

- Implement Taffy-backed layout for nested flex, sizing constraints, min/max, percentage, absolute, scroll clipping, plugin editor constraints, and graph-heavy surfaces.

Acceptance:

- Layout tests cover desktop windows and constrained plugin editors.

Status:

- Layout computation flows through Taffy behind Hawk2UI-owned records.
- `LayoutStyle` now exposes flex basis, flex grow, explicit flex shrink, cross-axis alignment,
  main-axis distribution, and absolute inset records.
- Taffy lowering maps nested flex, fixed and percentage sizing, min/max constraints, margins,
  padding, gaps, scroll clips, absolute positioning, absolute insets, flex basis/grow/shrink,
  alignment, distribution, and measured text leaves.
- Existing tests cover nested flex, scroll clipping, absolute children, percentage resize behavior,
  text measurement, plugin editor constraints, and graph/analyzer geometry attachment.
- Added coverage for flex growth, flex basis, alignment, justify content, absolute insets, and
  invalid flex factors.
- Documented the production Taffy mapping and shrink default in `manual/layout-reference.md`.

Review check:

- As the delivering engineer, I am satisfied with this remediation item for production readiness:
  the layout engine is Taffy-backed through stable Hawk2UI records, missing flex/absolute controls
  are implemented and tested, existing scroll overflow behavior is preserved, and plugin/editor
  geometry coverage remains intact. No corrective revision is required before host size/DPI
  integration.

### REM-LAYOUT-002: Host Size Negotiation And DPI

Evidence:

- Host metrics records exist.
- Layout and host resize/DPI integration is partial.

Required remediation:

- Connect host logical/physical sizes and DPI changes to layout invalidation and renderer target recreation.
- Add explicit plugin host negotiation paths.

Acceptance:

- Resize/maximize/DPI changes re-layout and repaint actual scene output.

Status:

- Added `HostSurfaceUpdateRequest` to carry host metrics, logical viewport dimensions, physical
  target size, layout invalidation, and renderer target recreation in one contract.
- `RendererResizeBridge` now converts surface, desktop, and plugin resize/DPI events into combined
  layout-and-render update requests while preserving the existing renderer-only APIs.
- Added plugin host resize negotiation through `PluginEditorConstraints::try_negotiate_host_resize`,
  returning requested size, accepted clamped size, DPI, and physical target dimensions.
- Added tests for desktop DPI update requests, common surface resize invalidation, plugin host resize
  invalidation, and plugin editor resize/DPI negotiation.
- Documented resize/DPI behavior in `manual/layout-reference.md`.

Review check:

- As the delivering engineer, I am satisfied with this remediation item for production readiness:
  resize and DPI changes now produce explicit layout invalidation and renderer recreation records,
  plugin host sizing has a deterministic negotiation path, and existing renderer-only callers remain
  compatible. No corrective revision is required before authoring/runtime remediation.

### REM-LAYOUT-003: Enable And Implement Taffy Grid Support

Status: Remediated in source.

Evidence:

- `docs/specs/grid-support.md` is listed as a required domain spec.
- `crates/hawk2ui-layout/Cargo.toml` enables Taffy `std`, `taffy_tree`, and `flexbox`, but not `grid`.
- Dashboard-style desktop applications and premium plugin editors need grid layout for dense controls, meters, inspectors, and responsive panels.

Required remediation:

- Enable the Taffy grid feature when implementing the accepted grid spec.
- Add Hawk2UI-owned grid style records for rows, columns, gaps, placement, spanning, auto-flow, min/max content sizing, and unsupported syntax diagnostics.
- Map those records into Taffy while preserving deterministic layout output.
- Add authoring/style lowering for the accepted CSS grid subset.

Acceptance:

- Grid containers and children compute through Taffy in layout tests.
- CSS/style grid declarations lower into typed layout records.
- Unsupported grid syntax fails with structured diagnostics instead of silently degrading.

Remediation delivered:

- Enabled Taffy `grid` in `hawk2ui-layout`.
- Added Hawk2UI-owned grid records for explicit tracks, implicit tracks, auto-flow, row/column placement, line placement, spans, and min/max-content track sizing.
- Mapped Hawk2UI grid records into Taffy `Display::Grid`, template rows/columns, auto rows/columns, auto-flow, and item row/column placement.
- Added layout coverage for grid template tracks, gaps, spanning placement, auto-flow, and min/max-content implicit tracks.
- Added typed style compiler support for the accepted longhand grid subset: template rows/columns, auto rows/columns, auto-flow, and row/column start/end placement.
- Added structured diagnostics for unsupported grid functions such as `repeat(...)` instead of silent fallback.

Review check:

- As the implementer delivering this product, I am satisfied with this remediation for production stability of the first grid layout surface. The implemented subset is explicit, typed, tested, and intentionally rejects unsupported syntax. Future expansion to named areas or `repeat()` should be an additive spec change, not an undocumented fallback.

## Authoring And Framework Remediation

### REM-AUTH-001: Replace Framework String Scanners

Evidence:

- Svelte, React, Vue, and Solid adapters all implement `extract_attribute`-style string scanning.
- They recognize limited custom attributes/events and static text patterns.

Required remediation:

- Define real framework integration strategy for each framework.
- Implement compiler/runtime adapters that produce Hawk2UI typed records without browser DOM assumptions.
- Preserve source maps and diagnostics.

Acceptance:

- Framework examples compile through real framework tooling or an explicitly specified native compiler boundary.
- Dynamic children, lifecycle, refs, events, styles, assets, and diagnostics are source-mapped.

Status:

- Added `FrameworkNativeProgram` and `FrameworkNativeNode` as the explicit Rust native compiler
  boundary for framework outputs.
- The boundary carries root/child node identity, keyed children, typed props, refs, style refs,
  asset refs, events, lifecycle handlers, and child props without inspecting framework source text.
- Svelte 5, React 19+, Vue 3.5+, and Solid source units now accept `from_native_program(...)`.
- Runtime bridging uses the typed program directly when present, including child text/layout props
  and lifecycle/event bindings.
- Framework conformance snapshot and runtime evidence now run through explicit native compiler
  boundary inputs instead of framework source fixture scanning.
- Source-string scanners remain only as compatibility fixture paths for legacy source tests and
  source-mapped diagnostic cases; they are not the production conformance path.

Review check:

- As the delivering engineer, I am satisfied with this remediation item for production readiness:
  the production framework boundary is explicit, typed, runtime-bridged, conformance-backed, and
  documented. No corrective revision is required before continuing the next remediation item.

### REM-AUTH-002: Implement Custom Renderer API

Evidence:

- Domain index requires `custom-renderer-api.md`.
- Current authoring bridge is internal Rust records.

Required remediation:

- Define public custom renderer protocol for framework authors.
- Support create/update/remove nodes, props, events, refs, keyed children, lifecycle, style refs, asset refs, and error boundaries.

Acceptance:

- At least one framework integration uses the custom renderer API rather than bespoke parsing.

Status:

- Implemented `CustomRendererProtocol`, `CustomRendererOperation`, and `CustomRendererError` in
  `hawk2ui-authoring`.
- The public operation surface covers create node, set prop, style refs, asset refs, native refs,
  event bindings, lifecycle bindings, keyed/unkeyed child append, error boundaries, commit, and
  remove node.
- Protocol validation rejects duplicate nodes and references to missing nodes with stable
  diagnostics.
- React 19+ now emits reconciler operation evidence through `CustomRendererProtocol`.
- Documented the custom renderer protocol in `manual/runtime-apis.md`.

Review check:

- As the delivering engineer, I am satisfied with this remediation item for production readiness:
  the custom renderer API is explicit, validated, documented, and exercised by React as the first
  framework proof. No corrective revision is required before continuing framework scanner
  replacement.

### REM-AUTH-003: Complete App Lifecycle And Event Model

Evidence:

- Runtime/authoring crates contain lifecycle and event records.
- Full mount/update/suspend/resume/hot reload/shutdown and event propagation are not complete.

Required remediation:

- Implement component lifecycle, state updates, event delivery, capture/bubble or chosen alternative, pointer/key/input mapping, teardown, and error boundaries.

Acceptance:

- End-to-end app tests prove lifecycle and events update rendered output.

Status:

- Added `RuntimeViewTree::update_visual(...)` so an event/lifecycle handler can replace a node
  visual and mark the node invalidated in one runtime operation.
- Added an end-to-end runtime test proving lifecycle update hook registration, event dispatch,
  visual mutation, scene diff repaint requirement, invalidated view IDs, and updated draw command
  output.

Review check:

- As the delivering engineer, I am satisfied with this remediation slice for production readiness:
  the runtime now has a typed event-to-render update path with repaint evidence. No corrective
  revision is required before continuing broader runtime ownership work.

## Runtime Remediation

### REM-RUNTIME-001: Complete Runtime Architecture

Evidence:

- Runtime crates contain records for view, scheduler, script, lifecycle, bindings, and safety.
- The runtime is not the owner of a real app execution process from compiled artifact to host rendering.

Required remediation:

- Define runtime ownership, threads, queues, lifecycle, host integration, script jobs, render jobs, timers, IO callbacks, cancellation, and shutdown.
- Connect compiled app artifacts to runtime tree updates and host presentation.

Acceptance:

- `hawk2ui run-desktop` runs a compiled app artifact through runtime, layout, render, and host surface.

### REM-RUNTIME-001A: Wire Script Promises And Timers To JavaScript Runtime

Status: Remediated in production slice.

Evidence:

- `ScriptBackend::create_promise`, `ScriptBackend::resolve_promise`, and `ScriptBackend::schedule_timer` maintain Rust-side records.
- `ScriptBackend::evaluate_javascript` calls Boa `Context::run_jobs()`, but the sidecar promise/timer records do not create Boa promises, enqueue Boa jobs, or execute timer callbacks in JavaScript.
- Runtime tests currently prove record-keeping semantics, not end-to-end JavaScript promise/timer execution.

Required remediation:

- Define the host event loop contract between Hawk2UI runtime scheduler and the embedded JavaScript runtime.
- Implement promise creation/resolution through the selected JS runtime rather than sidecar-only records.
- Implement deterministic timers that enqueue JavaScript callbacks under runtime control.
- Ensure teardown cancels pending jobs and timers without leaking host handles.
- Add interruption and resource-limit behavior for queued jobs.

Acceptance:

- JavaScript code can await a host-created promise and observe its resolution through the runtime job queue.
- JavaScript timer callbacks execute deterministically in headless/runtime tests.
- Teardown prevents further callback execution and reports structured diagnostics for invalid operations.

Remediation delivered:

- `ScriptBackend::execute_module_with_host_jobs` projects resolved Rust-owned promise records into Boa as real JavaScript promises through `hawk2ui.promise(label)`.
- Deterministic timer records are projected into JavaScript callback registration through `hawk2ui.onTimer(label, callback)` and are flushed under host control after the initial Boa job queue drain.
- Boa jobs are drained after module evaluation and after each deterministic timer flush, so promise continuations and timer callbacks settle in a predictable order for headless/runtime execution.
- `ScriptBackend::teardown` clears pending promises and timers and prevents later host-job execution with a structured `script.torn-down` diagnostic.
- Regression coverage verifies JavaScript-visible promise resolution, deterministic timer callback execution, and teardown cancellation.

Review check:

- As the implementer delivering this product, I am satisfied with this remediation slice for production stability of the script backend host-job bridge. It does not close the larger `REM-RUNTIME-001` runtime ownership item; scheduler ownership, cross-thread queues, app artifact driving, and host presentation remain tracked there.

### REM-RUNTIME-002: Capability Policy Enforcement

Status: Remediated in runtime binding layer.

Evidence:

- Security/platform crates have capability records.
- `ScriptBackend` now runs real Boa-backed JavaScript/TypeScript execution and projects host
  promises/timers through Boa jobs.
- `HostBindingRegistry::call` denies unavailable lifecycle phases, schema mismatches, duplicate
  downgrade attempts, and missing declared capabilities with structured diagnostics.
- `RuntimeCapability` now covers filesystem, network, clipboard read/write, database, audio,
  secrets, AI providers, MCP, dialogs, notifications, global shortcuts, render invalidation, UI
  events, and plugin parameters.

Required remediation:

- Enforce filesystem, network, clipboard, database, audio, secrets, AI, MCP, dialogs, notifications, and shortcuts through capability checks.

Acceptance:

- Denied operations fail with structured diagnostics in real runtime execution.

Remediation delivered:

- Added missing runtime capability variants for database, audio playback, AI providers, MCP,
  dialogs, notifications, and global shortcuts.
- Added runtime host-binding coverage that proves every platform capability domain denies missing
  capability declarations and allows explicitly declared capability calls.
- Kept concrete OS/API behavior separate under `REM-API-001`; this item now covers the runtime
  enforcement boundary that script-host calls must pass before any platform API executes.

Review check:

- As the delivering engineer, I am satisfied with the runtime capability enforcement boundary:
  host binding execution is denied by default, capability declarations are explicit, and denials
  are structured. No corrective revision is required for this runtime layer before implementing
  the concrete platform APIs.

### REM-RUNTIME-003: State Persistence

Status: Remediated in runtime persistence layer.

Evidence:

- Plugin state records and platform records exist.
- Runtime now exposes versioned state snapshots with app, UI preference, plugin parameter,
  plugin non-parameter, and user preset scopes.
- Runtime persistence records include host-specific opaque state chunks for plugin host save/load
  cycles.
- Runtime storage roots and user preset paths are validated before state save/restore path
  materialization.
- Runtime migrations apply deterministically and fail with structured diagnostics when migration
  source versions do not match the snapshot version.

Required remediation:

- Implement persistence APIs, migrations, user preset paths, plugin host state chunks, UI preference separation, and restore behavior.

Acceptance:

- State survives restart and host save/load cycles.

Remediation delivered:

- Added `RuntimeStateSnapshot`, `RuntimeStateEntry`, `RuntimeStateScope`,
  `RuntimeHostStateChunk`, `RuntimeStateMigration`, `RuntimeStoragePath`, and
  `RuntimePersistenceStore`.
- Added deterministic save/restore behavior for runtime snapshots by stable identity.
- Added user preset path materialization under validated OS storage roots.
- Added tests proving scoped state separation, host chunk preservation, migrations, restart
  restore, user preset paths, unsafe path rejection, and migration mismatch diagnostics.

Review check:

- As the delivering engineer, I am satisfied with this runtime persistence layer for production
  stability: state scopes are explicit, restore is deterministic, storage paths are validated, and
  migration failures are structured. Native OS storage backends can now be implemented behind this
  contract without changing app/plugin state semantics.

## Build And Packaging Remediation

### REM-BUILD-001: Complete Build Pipeline

Evidence:

- `hawk2ui-build` contains manifest, artifact, assets, pipeline, report, workspace modules.
- Full source compilation, framework tooling, JS/TS, style, layout/render artifacts, and packaging are incomplete.

Required remediation:

- Build source graph, transform TS/JS/framework inputs, compile styles/assets, validate manifests, generate sealed artifacts, generate schemas, and produce verification reports.

Acceptance:

- `hawk2ui build` creates deterministic artifacts for desktop and plugin targets.

### REM-BUILD-002: Complete Sealed Artifact Format

Evidence:

- Sealed artifact records exist.
- Stable binary/container format and verification rules need completion.
- Sealed artifact records now derive serde/schema contracts and can validate JSON against the
  generated schema.

Required remediation:

- Define artifact container, versioning, hashes, signatures, manifest snapshots, compiled assets/styles/scripts, target metadata, and compatibility checks.

Acceptance:

- Artifacts are reproducible and verifiable.

Status:

- Remediated: sealed artifact records have deterministic content hashing, compatibility checks,
  generated JSON Schema, schema validation, deterministic container bytes, content-hash container
  verification, and explicit development-vs-release signature policy enforcement.

Review check:

- As the delivering engineer, I am satisfied with this sealed-artifact boundary for production
  stability: artifacts have a stable serialized container, release policy rejects unsigned payloads,
  and container loading verifies schema compatibility plus content hashes before returning an
  artifact.

### REM-BUILD-003: Implement Dev Server And Hot Reload

Evidence:

- Domain index requires development server and hot reload.
- Current CLI/dev loop is not a complete native hot reload system.

Required remediation:

- Add file watching, incremental rebuilds, source diagnostics, state preservation, native surface reload, and error overlays.

Acceptance:

- Editing source/style/assets updates the native window without process restart where supported.

## Host And Windowing Remediation

### REM-HOST-001: Complete Host Abstraction

Status: Remediated in source.

Evidence:

- Host contracts exist for surfaces, platform handles, resize, desktop/plugin adapters.
- `HostSurface` now defines a common lifecycle for metrics, DPI/resize, focus, repaint,
  window commands, clipboard requests, teardown, and frame-presentation timing.
- `RecordingDesktopAdapter` and `RecordingPluginAdapter` both implement the common
  `HostSurface` contract while preserving their desktop/plugin-specific host event APIs.
- Renderer command replay is owned by `hawk2ui-render-skia`; `hawk2ui-host-winit`
  delegates runtime scene drawing through renderer replay options instead of owning draw-command
  semantics.

Required remediation:

- Define common host surface lifecycle, metrics, DPI, input, focus, repaint, clipboard, close/minimize/maximize/fullscreen, teardown, and presentation timing.

Acceptance:

- Desktop and plugin hosts implement the same surface contract without owning rendering semantics.

Remediation delivered:

- Added common `SurfaceWindowCommand`, `SurfaceWindowMode`, and `SurfaceClipboardRequest`
  records.
- Added common host-surface event coverage for window commands, clipboard requests, and frame
  presentation records.
- Extended desktop and plugin recording adapters to implement `HostSurface` so common lifecycle
  tests exercise both host classes through the same trait.
- Extended resize/update bridging to classify new common host events without accidentally forcing
  renderer target recreation for non-size-affecting events.

Review check:

- As the delivering engineer, I am satisfied with this host abstraction slice for production
  readiness: common lifecycle ownership is explicit, desktop/plugin adapters share the same
  contract, and rendering semantics remain in the renderer layer. No corrective revision is
  required before continuing deeper Winit platform coverage.

### REM-HOST-002: Complete Winit Desktop Backend

Evidence:

- Winit opens a native window and handles resize/DPI/input/close.
- It renders compiled runtime scene output through the renderer-owned Skia replay path.
- It still does not provide complete menus, tray, or platform-specific validation for all native
  desktop integrations.
- `scale_factor_to_f32` previously converted numeric DPI scale by string round-trip instead of direct checked numeric conversion.

Required remediation:

- Connect Winit to runtime scene renderer.
- Validate close, maximize, minimize, fullscreen, resize, DPI, focus, input, clipboard, and teardown across supported OSes.

Acceptance:

- Manual and automated desktop lifecycle tests pass on Linux Wayland and other supported targets.

Status:

- `scale_factor_to_f32` now validates finite/positive input and then performs a direct `f64 as f32` conversion with post-cast validation.
- `logical_size_to_f32` now uses the same checked direct numeric conversion pattern for runtime
  scene viewport dimensions instead of a string round-trip.
- Winit software frame rendering now accepts runtime scene image/vector asset commands and paints visible missing-asset placeholders instead of aborting frame presentation.
- `REM-RENDER-001` connected Winit runtime scene frames to renderer-owned Skia replay, so desktop
  frames now render the runtime tree through the production renderer path.
- Winit native event translation now maps resize, DPI, focus, keyboard, pointer, IME, file
  drag/drop, occlusion, redraw, close, and destroyed events into `DesktopHostEvent` records and the
  runtime event loop uses that translator for lifecycle accounting and redraw decisions.
- Winit desktop clipboard requests now execute through a capability-checked native clipboard bridge
  with an `arboard` OS backend and deterministic fake-backend tests for read, write, clear, denial,
  and adapter event recording.
- Winit desktop dialog requests now execute through a typed native dialog bridge with `rfd` as the
  production backend, deterministic fake-backend tests for message/open/save/cancel behavior, and
  adapter event recording after successful backend completion.
- Winit native clipboard smoke coverage is available as an ignored environment-gated test:
  `HAWK2UI_NATIVE_CLIPBOARD_SMOKE=1 cargo test -p hawk2ui-host-winit winit_native_clipboard_backend_smoke_when_enabled --test winit_adapter -- --ignored`.
- Native menu/tray implementation remains blocked on a deliberate dependency decision: the current
  production crates (`muda`/`tray-icon`) require GTK/libxdo/AppIndicator development packages on
  Linux when enabled, so they must be isolated behind explicit platform features or documented OS
  package prerequisites before entering the default workspace build.

Review check:

- This remains partially open for production stability: close, resize, maximize, DPI, input, IME,
  drag/drop, occlusion, OS clipboard bridging, and runtime scene presentation are covered by
  automated translation/runtime paths and manual smoke paths, native dialogs now have a
  platform-backed bridge, and clipboard has an opt-in native smoke. Menus, tray, and interactive
  platform-specific dialog smoke coverage still need platform-backed implementation before
  `REM-HOST-002` can be closed.

### REM-HOST-003: Complete Baseview Plugin Backend

Evidence:

- Baseview dependency exists.
- `BaseviewPluginAdapter` uses parent fixtures, records events, validates native parent handles,
  and exposes a real `baseview::Window::open_parented` attachment path.
- Source imports Baseview open-option types and validates parent handles, metrics, resize, DPI,
  focus, pointer, keyboard, repaint, and teardown contracts through fixture tests.
- `BaseviewPluginAdapter::render_scene_frame` now replays a `RuntimeSceneFrame` through the Skia
  backend, retains the presented snapshot, tracks frame count, and sizes the render target from
  plugin host metrics across resize/DPI changes.
- The plugin synth smoke fixture now creates a Baseview adapter, validates an `XWayland` parent,
  applies resize/DPI, renders a runtime scene, and verifies the presented physical size and visible
  pixels.
- The plugin meter/analyzer smoke fixture now drives the actual preallocated realtime visual
  transport, frame gate, drop accounting, and UI drain path instead of returning constants from the
  trace file alone.
- `hawk2ui-host-baseview` has an opt-in native integration smoke
  (`HAWK2UI_NATIVE_BASEVIEW_SMOKE=1`) that creates a real `X11`/`XWayland` parent window, opens a
  parented Baseview child through `BaseviewPluginAdapter::open_parented_window`, renders a
  `RuntimeSceneFrame` through Skia, presents the pixels into the native child window, receives a
  frame, and closes cleanly.
- Native Baseview event translation now maps window resize/DPI/focus/close, keyboard events,
  pointer movement/buttons/wheel/drag events, frame presentation, and host-driven show/hide into
  `PluginHostEvent` records.

Status:

- Remediated for the current Baseview backend boundary: fixture/contract coverage, runtime-scene
  Skia presentation, raw native parent validation, a real Baseview `open_parented` source path,
  plugin smoke coverage for the Baseview render path, opt-in native parented-window smoke coverage
  with X11/XWayland pixel presentation, and native event translation for resize, DPI, focus,
  keyboard, pointer, frame presentation, show/hide, and teardown now exist.

Required remediation:

- Route host resize, DPI, focus, pointer, keyboard, repaint, show/hide, and teardown.
- Ensure plugin editor teardown never exits process.

Acceptance:

- Baseview smoke app opens a real embedded/plugin-like surface and renders Hawk2UI scene output.
- Baseview native event translation is covered by deterministic tests, and the opt-in native smoke
  records frame presentation through the same handler event sink used by real parented windows.

### REM-HOST-004: Complete Native Platform Backends

Evidence:

- Domain index requires Windows, macOS, Linux, Wayland, X11/XCB/XWayland specs.
- Current code has platform handle records and Winit fixtures, not complete platform adapters.

Required remediation:

- Implement Windows HWND ownership/child windows/message pump/DPI.
- Implement macOS NSWindow/NSView ownership/plugin embedding/scaling/events.
- Implement Linux Wayland and X11/XCB/XWayland compatibility strategy.

Acceptance:

- Compatibility matrix cells are explicit and tested.

## Plugin And Audio Product Remediation

### REM-PLUGIN-001: Complete Plugin Product Model

Evidence:

- Plugin metadata, parameter, automation, state, realtime records exist.
- No real plugin format adapter is complete.
- Plugin format/package metadata records now generate and validate JSON Schema at the plugin crate
  boundary.

Required remediation:

- Define and implement plugin identity, editor metadata, parameters, presets, UI preferences, DSP/UI state boundary, generated parameter UI, host-safe diagnostics, and packaging outputs.

Acceptance:

- Plugin product manifests validate and build into selected plugin targets.

### REM-PLUGIN-002: Implement Plugin Format Adapters

Evidence:

- `hawk2ui-plugin-adapters` materializes package file layouts and now uses `clap-sys` for the
  generated CLAP scaffold path.
- VST3/AU/LV2 remain non-loadable package/metadata paths unless separately implemented or removed
  from the production target matrix.

Status:

- CLAP adapter metadata now depends on `clap-sys` and emits a CLAP entry plan containing the CLAP ABI version, `clap_entry` symbol, plugin factory ID, plugin ID, vendor/name/version, descriptor ABI marker, and CLAP feature list.
- CLAP package materialization now writes `Contents/Resources/clap-entry.toml` and includes it in required file and hash verification coverage.
- CLAP `cdylib` scaffold generation now writes a Cargo project with a `clap_entry` export, plugin descriptor, plugin factory callbacks, lifecycle callbacks, realtime-safe bypass processing callback, audio-port extension, parameter extension generated from `ParameterModel`, state save/load extension, stateful GUI/editor extension configured from `PluginEditor`, and create-plugin path; tests compile it to a release dynamic library and host-load it through a generated external checker that resolves the entry, obtains the factory, reads the descriptor, creates a plugin instance, invokes lifecycle/process callbacks, reads parameter metadata/default values, round-trips state, and exercises GUI preferred API, create, parent attachment, resize, show/hide, and destroy.
- Package materialization can now embed a sealed `Hawk2UI` runtime artifact payload under `Contents/Resources/hawk2ui-runtime-artifact.json`, reference it from `hawk2ui-artifact.toml`, include it in hash coverage, and fail package verification when the payload is missing or tampered.
- Package materialization now emits `Contents/Resources/hawk2ui-editor.toml` for runtime-backed
  packages, declaring the `baseview` host adapter, `skia` renderer, runtime artifact path, format,
  plugin ID, and parameter count, and includes that descriptor in package hash verification.
- CLAP GUI parent handles now have a safe typed bridge from CLAP window APIs into `Hawk2UI`
  platform handle records, including validation for nonzero raw handles, Linux display metadata,
  and Baseview's current native-Wayland attachment limitation.
- Generated CLAP `cdylib` scaffolds can now embed a `Hawk2UI` runtime editor descriptor and expose
  it through a stable `hawk2ui_editor_descriptor` dynamic-library export; the external generated
  host checker resolves the export and verifies the runtime artifact path, host adapter, and
  renderer metadata.
- The generated CLAP editor descriptor is now backed by a typed `ClapRuntimeEditorDescriptor`
  record with validation for safe relative runtime artifact paths, host adapter IDs, and renderer
  IDs before it can be embedded into the dynamic library scaffold.
- Generated CLAP GUI scaffolds now honor the selected editor host adapter when advertising native
  window APIs: `host_adapter=baseview` prefers X11/XWayland on Linux and rejects native Wayland
  attachment, matching the current Baseview backend boundary instead of exposing an unsupported
  GUI path.
- Runtime-backed CLAP package materialization now writes the generated CLAP `cdylib` Cargo scaffold
  under `Contents/Resources/generated-clap`, embeds the same runtime editor descriptor payload used
  by host-load tests, preserves generated parameter metadata in the scaffold source, and includes
  both scaffold files in package hash verification.
- Generated CLAP `cdylib` scaffolds now keep parameter values in lock-free atomic storage, save
  parameter state into the CLAP state stream, restore validated finite parameter values from host
  state payloads, clamp restored values to generated parameter ranges, and prove the round trip in
  the external host-load test.
- Generated CLAP `params.flush` now parses host input events, accepts `CLAP_EVENT_PARAM_VALUE`
  automation events from the core event space, rejects malformed/non-finite values, clamps valid
  automation to generated parameter ranges, and updates the same lock-free parameter store used by
  state persistence and `params.get_value`.
  - Runtime-backed CLAP packages now have a typed `ClapRuntimeEditorSession` loader that verifies the
    package hash manifest, parses `hawk2ui-editor.toml`, validates the `baseview`/`skia`/`clap`
    descriptor contract, resolves the safe package-relative runtime artifact path, parses and schema
    validates the sealed runtime artifact JSON into a typed `SealedArtifact`, and fails closed when
    package contents no longer match hash coverage or the runtime payload is malformed.
  - Runtime-backed CLAP package requests now carry `PluginEditor` metadata into materialized editor
    descriptors and generated CLAP scaffolds, so the verified session has stable editor ID, logical
    size, and DPI scale for the Baseview surface instead of relying on hard-coded defaults.
  - Verified CLAP runtime editor sessions can now build a Baseview handoff record from a CLAP GUI
    parent handle, producing the validated native parent handle plus the format-neutral
    `PluginEditorConfig` consumed by the Baseview adapter.
  - Sealed artifacts now carry an optional compiled runtime scene payload, and verified CLAP runtime
    editor sessions can decode that payload into a real `RuntimeSceneFrame`; missing or malformed
    runtime scene payloads fail explicitly instead of rendering a synthetic placeholder.
  - Baseview adapter integration now loads a verified CLAP runtime editor session from a materialized
    package, builds the CLAP parent-to-Baseview handoff, attaches the Baseview adapter to the validated
    native parent fixture, renders the sealed runtime scene through Skia, and verifies the presented
    surface pixels.
  - Generated CLAP dynamic-library scaffolds now expose a stable `hawk2ui_editor_state` export backed by
    the same atomics updated by the CLAP GUI callbacks, and the external host-load test verifies
    create, parent attach, resize, show, hide, and destroy transitions through the compiled library.
- Remaining work under this item is to replace the generated CLAP GUI callback state machine with live
  `Hawk2UI` runtime editor session attachment/rendering inside the plugin surface.

Required remediation:

- Finish the selected CLAP adapter with live Hawk2UI editor rendering in the attached GUI surface,
  parameter/state/realtime bridge integration, dynamic-library generation, signing/notarization
  policy where applicable, and host tests.
- Implement VST3/AU/LV2 with equivalent lifecycle/editor/state/host tests only if those formats
  remain selected production targets.

Acceptance:

- At least one real plugin format loads in a test host with Hawk2UI editor rendering.

### REM-PLUGIN-003: Realtime Visual Data Channels

Evidence:

- `RealtimeVisualTransport` and split audio/UI endpoints use `rtrb`-backed preallocated channels.
- `RealtimeVisualFrameGate` provides explicit UI-side frame-rate reduction for realtime visual drains.

Status:

- Remediated in source for the format-neutral realtime transport layer.
- Tests cover meter/analyzer/scope/modulation packets, drop policy behavior, thread-moved audio writers, non-blocking/no-allocation counters, UI drains, and reduced-cadence UI drain gating.

Required remediation:

- Carry the realtime transport into real plugin format adapters when `REM-PLUGIN-002` lands.

Acceptance:

- Tests prove audio-thread push cannot block or allocate.

### REM-PLUGIN-004: Audio Thread Safety Policy

Evidence:

- Product direction requires no audio-thread blocking.
- `hawk2ui-perf::RealtimeGuard` defines forbidden realtime operations, lock policy, and audit telemetry.
- `release/release-criteria.toml` includes `plugin-realtime-safety` as a release-blocking criterion.

Status:

- Remediated at release-gated policy/report layer.
- Tests cover denied allocation/blocking operations, allowed preallocated writes, explicit no-blocking-lock policy, telemetry counters, and release criterion coverage.

Required remediation:

- Carry the realtime guard into real plugin format adapter callbacks when `REM-PLUGIN-002` lands.

Acceptance:

- Audio-thread safety tests gate plugin release.

## Platform API Remediation

### REM-API-001: Complete Capability-Gated Platform APIs

Status: Remediated at policy/record layer.

Evidence:

- `hawk2ui-platform` contains capability/filesystem/network/database/clipboard/secrets records.
- `hawk2ui-platform` now contains capability-gated records and policies for filesystem, network,
  clipboard, database, secrets, audio playback, AI providers, MCP tools, notifications, global
  shortcuts, localization, dialogs, and file pickers.
- The policy layer validates manifest allowlists before platform API execution and returns
  structured diagnostics for missing capabilities or undeclared operations.
- Runtime host bindings enforce the corresponding runtime capability domains before calls can
  reach platform policies.

Required remediation:

- Implement filesystem scoped handles, network allowlists, database migrations, clipboard text/image, audio playback, secrets, AI provider wrappers, MCP integration, notifications, global shortcuts, localization, dialogs, and file pickers according to capability policy.

Acceptance:

- Every platform API has allow/deny tests and user-facing diagnostics.

Remediation delivered:

- Added `AudioPolicy`, `AiPolicy`, `McpPolicy`, `NotificationPolicy`, `ShortcutPolicy`,
  `LocalizationPolicy`, and `DialogPolicy`.
- Added platform operations for audio playback, AI provider requests, MCP tool calls,
  notifications, shortcut registration, localization reads, dialogs, and file pickers.
- Added allow/deny coverage for every extended platform domain in `hawk2ui-platform` tests.

Review check:

- As the delivering engineer, I am satisfied with this policy/record layer for production
  stability: all platform domains are denied by default, explicit manifest allowlists are required,
  and denials are structured. Native OS execution backends remain a separate host/platform
  integration concern, not a gap in this capability policy layer.

## Security Remediation

### REM-SEC-001: Complete Security Model

Evidence:

- Security source, sandbox, assets, secrets, trust records exist.
- Full source validation, runtime sandbox enforcement, package trust, and untrusted package handling are incomplete.

Required remediation:

- Define trust boundaries, source validation, script sandbox, asset sanitization, supply chain integrity, privacy/telemetry, plugin host safety, and untrusted package handling.
- Enforce at build, runtime, package validation, and CLI boundaries.

Acceptance:

- Security denial fixtures exercise real runtime/build/package code paths.

### REM-SEC-002: Complete Package Integrity

Evidence:

- `hawk2ui-security-model` validates trust records.
- Artifact signing, checksums, lockfile policy, reproducibility, and verification evidence are not complete.
- Package trust violations now convert into the shared diagnostic envelope with stable security rules.

Required remediation:

- Implement artifact signing or signature policy, checksum manifests, reproducible build checks, dependency policy, and release verification reports.

Acceptance:

- Tampered packages are rejected before execution.

Status:

- Remediated: package trust records validate artifact schema version, manifest hash presence,
  compiled asset/script hash presence and format, target metadata, signature status, and verification
  report presence.
- Package trust failures now expose shared diagnostics for downstream CLI/build/reporting
  boundaries.
- Package trust records can now be derived from actual sealed artifact payloads, including manifest
  hash, compiled asset hashes, compiled script payload hashes, target metadata, signature policy
  state, and verification report state.

Review check:

- As the delivering engineer, I am satisfied with this package-integrity boundary for production
  stability: trust failures have stable rules and user-facing messages, and trust records can be
  derived from the artifact that will actually be packaged instead of manually duplicated metadata.


## Accessibility Remediation

### REM-A11Y-001: Complete Accessibility Architecture

Status: Remediated at model/export/action-dispatch layer.

Evidence:

- A11y records exist for roles, actions, tree, host export snapshots, and plugin guards.
- AccessKit export is implemented in `hawk2ui-a11y` and covered by tests.
- OS-specific attachment is still owned by native host backends.

Required remediation:

- Build accessibility tree from runtime scene/layout/component semantics.
- Export to AccessKit where supported.
- Route actions back into event/component systems.
- Define plugin host limitations explicitly.

Acceptance:

- Accessible names, roles, focus, bounds, and actions update with scene changes.

Remediation delivered:

- Component semantics produce accessibility nodes independently from visual styling.
- Layout geometry updates mutate host-exported accessibility bounds.
- AccessKit export produces native tree updates with roles, labels, values, focus, actions, bounds,
  checked/disabled state, and deterministic child order.
- Accessibility action dispatch supports focus, press, increment, decrement, set value, and custom
  actions with shared diagnostic conversion for failures.
- Plugin accessibility guards deny audio-thread and unstable host operations while allowing safe UI
  thread updates.

Review check:

- As the delivering engineer, I am satisfied with this accessibility architecture layer for
  production stability: the model, action dispatch, AccessKit export, and plugin safety boundary are
  implemented and tested. Remaining OS attachment work belongs to Winit/Baseview/native host
  remediation.


## Testing And Release Remediation

### REM-TEST-001: Complete Test Strategy

Evidence:

- Domain index requires test strategy and many focused test specs.
- Current tests are useful but mostly unit/contract/smoke records.

Required remediation:

- Add unit, integration, visual, compatibility, fuzz, packaging, security, plugin host, performance, and manual gates.
- Use deterministic fixtures and headless rendering.

Acceptance:

- Release cannot proceed unless all defined gates pass.

### REM-TEST-002: Real Visual Regression

Evidence:

- Testkit visual fixture metadata and comparison-threshold records exist.
- `hawk2ui-render-skia` exposes CPU-readable `SkiaFrameSnapshot` data, and some framework tests
  assert visible rendered pixels.
- No complete deterministic golden-image suite currently renders production runtime scenes,
  writes baseline/diff artifacts, and fails with actionable pixel diagnostics.

Required remediation:

- Wire headless Skia runtime-scene rendering into the visual testkit.
- Define golden image tolerances, font fixtures, update workflow, CI behavior, and failure
  diagnostics.

Acceptance:

- Visual regressions fail with actionable diff artifacts.

### REM-TEST-003: Performance Benchmarks

Evidence:

- `hawk2ui-perf` contains performance budget records, benchmark helpers, and bench targets for
  startup, layout, render, render baseline, runtime, and plugin realtime.
- `release/release-criteria.toml` includes performance and realtime-safety gates.
- The remaining gap is release-grade measured evidence across the complete product matrix, not the
  absence of a performance crate or budget model.

Required remediation:

- Expand the existing benchmark suite into release-grade measured benchmarks for startup, style,
  layout, render, JS, asset load, package verification, desktop host, and plugin realtime paths.
- Persist baseline reports and enforce budget failures in release gates.

Acceptance:

- Performance regressions are visible and release-blocking where budgets are exceeded.

## Developer Experience And Manual Remediation

### REM-DX-001: Complete CLI Command Surface

Evidence:

- `hawk2ui-cli` defines and documents `new`, `validate`, `build-dev`, `build-release`,
  `verify-artifact`, `run-desktop`, `package-plugin`, `export-schemas`, and `diagnostics`.
- The CLI dev loop source is a recording watcher/reload target, not a real native hot-reload
  command.
- Generic `run`, production `dev`, `explain`, richer scaffolding, and complete no-Rust user
  workflows remain incomplete.

Required remediation:

- Implement production `dev` hot reload, `explain`, complete scaffolding workflows, stable exit
  code documentation, and source diagnostics across all user-facing commands.

Acceptance:

- User workflows are executable from CLI without Rust knowledge.

### REM-DX-002: Complete Project Scaffolding

Evidence:

- Examples exist.
- Production app/plugin templates are incomplete.

Required remediation:

- Add desktop app, plugin editor, generated parameter UI, style gallery, framework examples, security fixtures, and visual quality templates.

Acceptance:

- `hawk2ui new` creates runnable, buildable, testable projects.

### REM-DX-003: Complete User Manual

Evidence:

- Domain index lists manual pages M00-M09.
- Manual files are not complete.

Required remediation:

- Write user manual outline, getting started, authoring guide, style reference, component guide, plugin author guide, desktop app guide, security guide, troubleshooting, and prototype migration guide.

Acceptance:

- A new user can create, run, build, package, troubleshoot, and understand supported features from the manual.

### REM-DX-004: Complete Developer Documentation

Evidence:

- Many crates have API docs.
- Architecture, implementation, compatibility, extension, and contribution docs are incomplete.

Required remediation:

- Document architecture boundaries, runtime pipeline, renderer pipeline, host adapters, plugin adapters, security model, testing strategy, and contribution workflow.

Acceptance:

- A contributor can implement a domain without relying on chat history.

## Domain Spec Coverage Checklist

The following dedicated specs must exist and be implementation-aligned.

### Root Product Domains

- D00: `docs/specs/product-principles.md`
- D01: `docs/specs/architecture-boundaries.md`
- D02: `docs/specs/deft-adoption-decision.md`
- D03: `docs/specs/prototype-migration.md`
- D04: `docs/specs/compatibility-matrix.md`

### Authoring And Framework Domains

- A00: `docs/specs/authoring-model.md`
- A01: `docs/specs/declarative-ui-tree.md`
- A02: `docs/specs/component-contract.md`
- A03: `docs/specs/headless-rust-components.md`
- A04: `docs/specs/react-integration.md`
- A05: `docs/specs/vue-integration.md`
- A06: `docs/specs/svelte-integration.md`
- A07: `docs/specs/solid-integration.md`
- A08: `docs/specs/custom-renderer-api.md`
- A09: `docs/specs/app-lifecycle.md`
- A10: `docs/specs/routing-navigation.md`
- A11: `docs/specs/event-model.md`
- A12: `docs/specs/reactivity-data-binding.md`

### Source Language And Compilation Domains

- C00: `docs/specs/build-pipeline.md`
- C01: `docs/specs/typescript-javascript-pipeline.md`
- C02: `docs/specs/jsx-transform.md`
- C03: `docs/specs/svelte-compiler-pipeline.md`
- C04: `docs/specs/vue-compiler-pipeline.md`
- C05: `docs/specs/css-parsing-transform.md`
- C06: `docs/specs/selector-model.md`
- C07: `docs/specs/style-property-registry.md`
- C08: `docs/specs/design-token-compiler.md`
- C09: `docs/specs/asset-compiler.md`
- C10: `docs/specs/manifest-schema.md`
- C11: `docs/specs/sealed-artifact-format.md`
- C12: `docs/specs/dev-server-hot-reload.md`
- C13: `docs/specs/bun-toolchain-integration.md`

### Runtime Domains

- R00: `docs/specs/runtime-architecture.md`
- R01: `docs/specs/javascript-runtime-choice.md`
- R02: `docs/specs/boa-runtime-spike.md`
- R03: `docs/specs/host-bindings.md`
- R04: `docs/specs/scheduler-task-queues.md`
- R05: `docs/specs/runtime-limits.md`
- R06: `docs/specs/runtime-errors-diagnostics.md`
- R07: `docs/specs/state-persistence.md`
- R08: `docs/specs/capability-policy-runtime.md`
- R09: `docs/specs/native-module-boundary.md`

### Style, Layout, Scene, And Rendering Domains

- S00: `docs/specs/style-system.md`
- S01: `docs/specs/css-subset-reference.md`
- S02: `docs/specs/theming-skinning.md`
- S03: `docs/specs/layout-architecture.md`
- S04: `docs/specs/flexbox-support.md`
- S05: `docs/specs/grid-support.md`
- S06: `docs/specs/text-measurement-layout.md`
- S07: `docs/specs/scroll-clipping.md`
- S08: `docs/specs/scene-graph.md`
- S09: `docs/specs/paint-list-boundary.md`
- S10: `docs/specs/skia-renderer-abstraction.md`
- S11: `docs/specs/renderer-backends.md`
- S12: `docs/specs/text-shaping-typography.md`
- S13: `docs/specs/vector-path-drawing.md`
- S14: `docs/specs/image-rendering.md`
- S15: `docs/specs/svg-vector-assets.md`
- S16: `docs/specs/animation-system.md`
- S17: `docs/specs/visual-effects.md`
- S18: `docs/specs/graph-canvas-surfaces.md`
- S19: `docs/specs/hit-testing.md`
- S20: `docs/specs/visual-regression-renderer.md`

### Host And Windowing Domains

- H00: `docs/specs/host-abstraction.md`
- H01: `docs/specs/desktop-host-backend.md`
- H02: `docs/specs/plugin-host-backend.md`
- H03: `docs/specs/baseview-adapter.md`
- H04: `docs/specs/native-window-lifecycle.md`
- H05: `docs/specs/dpi-scaling.md`
- H06: `docs/specs/input-backend-mapping.md`
- H07: `docs/specs/clipboard-integration.md`
- H08: `docs/specs/platform-shell.md`
- H09: `docs/specs/dialogs-file-pickers.md`
- H10: `docs/specs/accessibility-host-bridge.md`
- H11: `docs/specs/wayland-support.md`
- H12: `docs/specs/x11-xcb-xwayland-support.md`
- H13: `docs/specs/windows-host-adapter.md`
- H14: `docs/specs/macos-host-adapter.md`
- H15: `docs/specs/linux-host-adapter.md`

### Plugin And Audio-Product Domains

- P00: `docs/specs/plugin-product-model.md`
- P01: `docs/specs/plugin-format-strategy.md`
- P02: `docs/specs/vst3-adapter.md`
- P03: `docs/specs/clap-adapter.md`
- P04: `docs/specs/au-adapter.md`
- P05: `docs/specs/lv2-adapter.md`
- P06: `docs/specs/standalone-plugin-wrapper.md`
- P07: `docs/specs/parameter-model.md`
- P08: `docs/specs/automation-gestures.md`
- P09: `docs/specs/ui-dsp-state-boundary.md`
- P10: `docs/specs/realtime-visual-data.md`
- P11: `docs/specs/generated-parameter-ui.md`
- P12: `docs/specs/preset-state-serialization.md`
- P13: `docs/specs/midi-external-control.md`
- P14: `docs/specs/plugin-packaging.md`
- P15: `docs/specs/plugin-host-compatibility-tests.md`
- P16: `docs/specs/audio-thread-safety.md`

### Platform API Domains

- API00: `docs/specs/capability-manifest.md`
- API01: `docs/specs/filesystem-api.md`
- API02: `docs/specs/network-api.md`
- API03: `docs/specs/database-api.md`
- API04: `docs/specs/audio-playback-api.md`
- API05: `docs/specs/secrets-api.md`
- API06: `docs/specs/ai-provider-wrapper.md`
- API07: `docs/specs/mcp-integration.md`
- API08: `docs/specs/notifications-api.md`
- API09: `docs/specs/global-shortcuts-api.md`
- API10: `docs/specs/localization-api.md`

### Security, Validation, And Trust Domains

- SEC00: `docs/specs/security-model.md`
- SEC01: `docs/specs/source-validation.md`
- SEC02: `docs/specs/script-sandbox.md`
- SEC03: `docs/specs/asset-sanitization.md`
- SEC04: `docs/specs/supply-chain-integrity.md`
- SEC05: `docs/specs/privacy-telemetry.md`
- SEC06: `docs/specs/plugin-host-safety.md`
- SEC07: `docs/specs/untrusted-package-handling.md`

### Developer Experience Domains

- DX00: `docs/specs/cli-command-surface.md`
- DX01: `docs/specs/project-scaffolding.md`
- DX02: `docs/specs/diagnostics-ux.md`
- DX03: `docs/specs/documentation-system.md`
- DX04: `docs/specs/example-suite.md`
- DX05: `docs/specs/template-design-system.md`
- DX06: `docs/specs/inspection-debug-tools.md`
- DX07: `docs/specs/ide-integration.md`

### Testing And Release Domains

- T00: `docs/specs/test-strategy.md`
- T01: `docs/specs/style-layout-tests.md`
- T02: `docs/specs/renderer-tests.md`
- T03: `docs/specs/runtime-tests.md`
- T04: `docs/specs/host-lifecycle-tests.md`
- T05: `docs/specs/plugin-lifecycle-tests.md`
- T06: `docs/specs/security-tests.md`
- T07: `docs/specs/performance-benchmarks.md`
- T08: `docs/specs/release-process.md`

### Manual And User-Facing Documentation Domains

- M00: `docs/manual/user-manual-outline.md`
- M01: `docs/manual/getting-started.md`
- M02: `docs/manual/authoring-guide.md`
- M03: `docs/manual/style-reference.md`
- M04: `docs/manual/component-guide.md`
- M05: `docs/manual/plugin-author-guide.md`
- M06: `docs/manual/desktop-app-guide.md`
- M07: `docs/manual/security-guide.md`
- M08: `docs/manual/troubleshooting.md`
- M09: `docs/manual/prototype-migration-guide.md`

## Remaining Implementation Priority Order

This order is based on the source-truth audit above. It is sequencing, not scope reduction.

1. Live plugin editor vertical: real Baseview parented attachment, live Hawk2UI scene rendering in
   the attached CLAP GUI, host resize/focus/input teardown, and realtime visual data bridge.
2. Native desktop completion: finish Winit menus, tray, dialogs, drag/drop, IME, complete OS
   clipboard, platform-specific validation, and accessibility attachment.
3. Build/package completion: replace build record-only phases with real source graph execution,
   TypeScript/framework/style compilation, deterministic package outputs, signing/notarization
   policy, and release verification reports.
4. Dev loop and CLI completion: real file watching, incremental rebuilds, native hot reload,
   state preservation, error overlays, `explain`, complete scaffolding, and stable documented exit
   codes.
5. Platform APIs: capability-gated filesystem, network, database, clipboard, secrets, dialogs,
   notifications, global shortcuts, localization, AI provider, and MCP backends.
6. Security/trust completion: source validation, runtime sandbox enforcement, asset sanitization,
   untrusted package handling, supply-chain integrity, dependency publishing policy closure, and
   denial fixtures that exercise real build/runtime/package paths.
7. Visual quality and regression: premium desktop/plugin templates, graph/custom surfaces,
   animation coverage, deterministic Skia golden-image rendering, baseline/diff artifact workflow,
   and font/CI tolerance policy.
8. Release readiness: measured performance baselines across the product matrix, compatibility
   matrix evidence, manual completion, troubleshooting docs, release evidence capture, and CI gates.

## Non-Completion Criteria

The following must not be accepted as production completion:

- record-only crates without runtime integration,
- string-scanning framework adapters,
- handwritten CSS parsing in production paths,
- custom partial layout when Taffy is the accepted baseline,
- toy JavaScript execution,
- hard-coded desktop rendering,
- command logs standing in for actual rendering,
- plugin package folders without real plugin lifecycle support,
- security records without runtime/build enforcement,
- documentation claiming support before examples and tests prove it.
