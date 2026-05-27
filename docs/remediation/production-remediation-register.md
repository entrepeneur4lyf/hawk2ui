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

Hawk2UI is not production-ready. The repository contains useful crate boundaries, records, tests, and a real Winit smoke window, but several crates are currently contract/scaffold implementations while their descriptions imply production completeness.

The largest confirmed drift is that the current code rebuilt core systems with simplified internal models instead of implementing the selected production foundations:

- style parsing uses handwritten string splitting instead of Lightning CSS,
- layout uses a custom partial layout engine instead of Taffy,
- script execution uses a toy expression evaluator instead of Boa or another real JavaScript runtime,
- framework adapters parse source by string scanning instead of real framework/compiler integration,
- Winit presents a hard-coded Skia frame instead of rendered runtime scene output,
- Baseview/plugin support is lifecycle-record scaffolding instead of real embedded editor surfaces,
- accessibility and schema crates are typed records without production backend integration.

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
- `crates/hawk2ui-style/Cargo.toml` has no `lightningcss` dependency.
- `crates/hawk2ui-style/src/compile.rs` parses source with `split('}')`, `split(';')`, and `split_once(':')`.

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

### REM-CRATE-002: Adopt Taffy For Layout

Evidence:

- `docs/technical/crate-selection.md` marks `taffy` as preferred stable.
- `docs/specs/0001-product-direction.md` says Taffy should be the preferred primary layout engine.
- `crates/hawk2ui-layout/Cargo.toml` has no `taffy` dependency.
- `crates/hawk2ui-layout/src/compute.rs` implements a custom column/row layout loop.

Required remediation:

- Add Taffy behind Hawk2UI-owned layout structs.
- Map Hawk2UI layout style, constraints, flex, scroll, absolute positioning, percentage sizing, and measurement into Taffy.
- Integrate text measurement as an intrinsic sizing input.
- Preserve deterministic output and diagnostics.

Acceptance:

- Layout computation flows through Taffy.
- Nested flex, constrained plugin sizes, scroll clips, text measurement, and absolute children are covered by tests.

### REM-CRATE-003: Implement Real Embedded JavaScript Runtime

Evidence:

- `docs/technical/crate-selection.md` says `boa_engine` is the first spike.
- `docs/specs/0001-product-direction.md` separates Bun tooling from embedded runtime and names Boa as first runtime spike.
- `crates/hawk2ui-script/Cargo.toml` has no JS runtime dependency.
- `crates/hawk2ui-script/src/lib.rs` uses `compile_typescript` string replacement and `evaluate_expression_module` with `+` expression evaluation.

Required remediation:

- Implement the Boa runtime spike unless a decision record changes runtime choice.
- Support modules, promises, host bindings, interruption, memory/resource limits, deterministic timers, teardown, and diagnostics.
- Remove the toy evaluator from production paths.
- Keep Bun, if used, as external tooling only.

Acceptance:

- Real JavaScript executes through the selected runtime.
- TypeScript is compiled through a real transform path before runtime execution.
- Runtime policy tests cover denied globals, host binding permissions, interruption, teardown, and promise/timer semantics.

### REM-CRATE-004: Add AccessKit Host Bridge

Evidence:

- `docs/technical/crate-selection.md` marks `accesskit` as preferred.
- `docs/specs/rendering-architecture.md` requires accessibility geometry references.
- `crates/hawk2ui-a11y/Cargo.toml` has no `accesskit` dependency.
- `crates/hawk2ui-a11y/src/lib.rs` exports typed accessibility records only.

Required remediation:

- Add AccessKit integration behind Hawk2UI accessibility host traits.
- Map roles, labels, focus, actions, bounds, checked state, dynamic updates, and host surface kinds.
- Integrate accessibility geometry from scene/layout output.

Acceptance:

- Desktop host can export an AccessKit tree.
- Plugin host behavior is explicit for supported and unsupported accessibility cells.

### REM-CRATE-005: Add Schema Generation And Validation

Evidence:

- `docs/technical/crate-selection.md` marks `schemars` and `jsonschema` as preferred.
- `crates/hawk2ui-schema/Cargo.toml` has no schema dependencies.
- `crates/hawk2ui-schema/src/lib.rs` exports typed records only.

Required remediation:

- Add schema generation for manifests, artifacts, capabilities, plugin metadata, and package metadata.
- Add JSON Schema validation in CLI/build paths.
- Version schemas and provide compatibility tests.

Acceptance:

- CLI validation uses generated schemas.
- Invalid manifests fail with source-specific diagnostics.

### REM-CRATE-006: Add Realtime And Plugin Format Crates Where Chosen

Evidence:

- `docs/technical/crate-selection.md` lists `rtrb`, `vst3`, `clap-sys`, `clack-*`, and related audio/plugin candidates.
- `crates/hawk2ui-plugin/Cargo.toml` and `crates/hawk2ui-plugin-adapters/Cargo.toml` do not depend on realtime or plugin format crates.

Required remediation:

- Choose plugin format sequence in decision records.
- Implement realtime UI data channels with a preallocated lock-free primitive such as `rtrb`.
- Implement actual CLAP/VST3/AU/LV2 adapters according to selected compatibility matrix.

Acceptance:

- At least one plugin format can build a real loadable plugin editor bundle.
- Realtime data tests prove audio-thread-safe behavior.

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
- Current Winit output is a hard-coded diagnostic frame.
- Current runtime visuals only model fill and text.

Required remediation:

- Implement expressive visual primitives through style/assets/scene/rendering.
- Provide premium templates for desktop and plugin UIs.
- Add visual regression fixtures proving gradients, textures, image panels, typography, shadows, glows, curves, meters, analyzers, knobs, sliders, and dense panels.

Acceptance:

- Example gallery demonstrates JUCE-class visual ambition without native drawing code by the user.

### REM-PROD-003: Manifest-First Product Validation

Evidence:

- Product direction requires plugin identity, editor metadata, parameters, presets, and asset declarations to be manifest-first and validated before runtime.
- Build/schema/plugin crates contain partial typed records and package scaffolding, but schema validation is not complete.

Required remediation:

- Complete manifest schema, validation diagnostics, and CLI integration.
- Validate app identity, plugin identity, editor metadata, parameters, defaults, ranges, duplicate IDs, asset references, unsafe assets, package targets, and capabilities.

Acceptance:

- Invalid projects fail before runtime.
- Diagnostics contain source paths and actionable rules.

## Rendering Pipeline Remediation

### REM-RENDER-001: Connect Runtime Scene Frames To Host Presentation

Evidence:

- `crates/hawk2ui-runtime/src/view.rs` can build `RuntimeSceneFrame`.
- `crates/hawk2ui-host-winit/src/software_frame.rs` renders a hard-coded default scene.
- Winit runtime does not consume `RuntimeSceneFrame`.

Required remediation:

- Add a scene presenter that consumes `RuntimeSceneFrame::draw_commands()`.
- Lower runtime draw commands into `hawk2ui-render-skia`.
- Make Winit desktop runtime accept a compiled app/runtime tree and render it.
- Remove hard-coded product UI from the desktop presentation path.

Acceptance:

- Changing author/runtime tree content changes the visible native window output.
- Tests prove fill/text scene output reaches pixels or backend commands.

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

Status: In progress; full affine transforms and dirty-bounds invalidation are remediated in source.

Evidence:

- `SceneNode` has layout, clip, affine transform, opacity, hit-test, accessibility refs, and invalidation flags.
- Full effects, opacity group semantics, layer attachment, and scene diffing remain incomplete.
- Dirty bounds, invalidation reasons, and cache invalidation state are now recorded on scene nodes.

Required remediation:

- Add opacity group semantics, effect references, layer membership, scene diffing, and deterministic z-order traversal.
- Wire remaining invalidation consumers to renderer cache eviction and host repaint scheduling.

Acceptance:

- `crates/hawk2ui-render/tests/render_export.rs` covers full affine transform storage, point application, validation, and stable serialization.
- `crates/hawk2ui-render/tests/render_export.rs` covers invalidation reasons, transformed dirty bounds, ancestor dirty-bound propagation, and cache invalidation flags.
- `crates/hawk2ui-render-skia/tests/skia_backend.rs` proves affine transforms affect rendered pixels.
- Remaining scene graph tests must cover clips, z-order, opacity groups, effects, layer attachment, hit testing, renderer cache eviction, host repaint scheduling, and accessibility geometry.

### REM-RENDER-004: Complete Skia Backend Execution

Evidence:

- `hawk2ui-render-skia` draws several primitives.
- `draw_vector` only records a command.
- `apply_layer_effect` only records a command.
- Trait-level `draw_text` uses `(0.0, 0.0)` and `Font::default()`.

Required remediation:

- Implement vector asset rendering from compiled vector records.
- Implement layer effects through Skia primitives where supported and explicit fallback diagnostics where unsupported.
- Implement text drawing with resolved font, position, baseline, color, DPI, and shaped layout.
- Implement image scaling, caching, color handling, and nine-slice if accepted by spec.

Acceptance:

- Skia backend renders all accepted layer types through tests and visual fixtures.

### REM-RENDER-005: Integrate Text Measurement With Layout And Rendering

Evidence:

- `hawk2ui-text` uses Parley/fontdb/swash.
- `hawk2ui-layout` does not use `hawk2ui-text`.
- Runtime text nodes use fixed/default sizing.

Required remediation:

- Add a text measurement provider from `hawk2ui-text` into layout.
- Feed measured text into Taffy.
- Feed shaped text output into Skia drawing.

Acceptance:

- Text wrapping, truncation, bidi, fallback, and high-DPI measurements affect layout and rendering.

### REM-RENDER-006: Complete Asset-To-Renderer Flow

Evidence:

- `hawk2ui-assets` compiles images/vectors/fonts into records.
- `hawk2ui-render` has compiled asset records.
- `hawk2ui-render-skia` can register image bytes but runtime scene output does not carry compiled asset draws end to end.

Required remediation:

- Connect asset manifest entries to runtime scene asset layers.
- Register assets with renderer during surface/frame preparation.
- Render image and vector layers from compiled asset IDs only.

Acceptance:

- Raw asset paths are rejected at rendering boundaries.
- Compiled image and vector assets render in desktop and headless tests.

### REM-RENDER-007: Implement Custom Draw Surfaces

Evidence:

- `CustomDrawSurface` is a record with category, layout, capabilities, invalidation, and schedule metadata.
- No draw callback/execution path is wired into render or host presentation.

Required remediation:

- Define and implement custom draw hooks for meters, analyzers, curves, scopes, timelines, graph editors, and inspector panels.
- Integrate with layout, hit testing, invalidation, frame scheduling, capabilities, and plugin-safe data feeds.

Acceptance:

- A custom graph/meter surface renders and updates independently under frame-rate limits.

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

### REM-STYLE-002: Complete Cascade, Inheritance, Variables, And Tokens

Evidence:

- Style tokens and runtime tables exist.
- Full cascade/inheritance/custom property behavior is not complete.

Required remediation:

- Implement deterministic cascade order, specificity, inheritance, initial values, custom properties, token resolution, theme variants, and user preference overrides.

Acceptance:

- Style computation matches documented subset.
- Theme and user preference changes invalidate affected render output.

## Layout Remediation

### REM-LAYOUT-001: Complete Layout Architecture With Taffy

Evidence:

- Layout tree and records exist.
- Custom layout is partial and not Taffy-backed.

Required remediation:

- Implement Taffy-backed layout for nested flex, sizing constraints, min/max, percentage, absolute, scroll clipping, plugin editor constraints, and graph-heavy surfaces.

Acceptance:

- Layout tests cover desktop windows and constrained plugin editors.

### REM-LAYOUT-002: Host Size Negotiation And DPI

Evidence:

- Host metrics records exist.
- Layout and host resize/DPI integration is partial.

Required remediation:

- Connect host logical/physical sizes and DPI changes to layout invalidation and renderer target recreation.
- Add explicit plugin host negotiation paths.

Acceptance:

- Resize/maximize/DPI changes re-layout and repaint actual scene output.

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

### REM-AUTH-002: Implement Custom Renderer API

Evidence:

- Domain index requires `custom-renderer-api.md`.
- Current authoring bridge is internal Rust records.

Required remediation:

- Define public custom renderer protocol for framework authors.
- Support create/update/remove nodes, props, events, refs, keyed children, lifecycle, style refs, asset refs, and error boundaries.

Acceptance:

- At least one framework integration uses the custom renderer API rather than bespoke parsing.

### REM-AUTH-003: Complete App Lifecycle And Event Model

Evidence:

- Runtime/authoring crates contain lifecycle and event records.
- Full mount/update/suspend/resume/hot reload/shutdown and event propagation are not complete.

Required remediation:

- Implement component lifecycle, state updates, event delivery, capture/bubble or chosen alternative, pointer/key/input mapping, teardown, and error boundaries.

Acceptance:

- End-to-end app tests prove lifecycle and events update rendered output.

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

### REM-RUNTIME-002: Capability Policy Enforcement

Evidence:

- Security/platform crates have capability records.
- Real script/runtime host API enforcement is incomplete because real JS runtime is absent.

Required remediation:

- Enforce filesystem, network, clipboard, database, audio, secrets, AI, MCP, dialogs, notifications, and shortcuts through capability checks.

Acceptance:

- Denied operations fail with structured diagnostics in real runtime execution.

### REM-RUNTIME-003: State Persistence

Evidence:

- Plugin state records and platform records exist.
- App state, plugin state, UI preferences, presets, migrations, and OS storage paths are not fully wired.

Required remediation:

- Implement persistence APIs, migrations, user preset paths, plugin host state chunks, UI preference separation, and restore behavior.

Acceptance:

- State survives restart and host save/load cycles.

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

Required remediation:

- Define artifact container, versioning, hashes, signatures, manifest snapshots, compiled assets/styles/scripts, target metadata, and compatibility checks.

Acceptance:

- Artifacts are reproducible and verifiable.

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

Evidence:

- Host contracts exist for surfaces, platform handles, resize, desktop/plugin adapters.
- Rendering and runtime ownership are not fully separated from host code.

Required remediation:

- Define common host surface lifecycle, metrics, DPI, input, focus, repaint, clipboard, close/minimize/maximize/fullscreen, teardown, and presentation timing.

Acceptance:

- Desktop and plugin hosts implement the same surface contract without owning rendering semantics.

### REM-HOST-002: Complete Winit Desktop Backend

Evidence:

- Winit opens a native window and handles resize/DPI/input/close.
- It renders a hard-coded frame and does not handle actual app scene rendering, maximize repaint validation, menus, tray, dialogs, drag/drop, IME, or complete clipboard.

Required remediation:

- Connect Winit to runtime scene renderer.
- Validate close, maximize, minimize, fullscreen, resize, DPI, focus, input, clipboard, and teardown across supported OSes.

Acceptance:

- Manual and automated desktop lifecycle tests pass on Linux Wayland and other supported targets.

### REM-HOST-003: Complete Baseview Plugin Backend

Evidence:

- Baseview dependency exists.
- `BaseviewPluginAdapter` uses parent fixtures and records events; it does not open/attach a real editor surface.

Required remediation:

- Implement real Baseview parented editor attachment.
- Route host resize, DPI, focus, pointer, keyboard, repaint, show/hide, and teardown.
- Ensure plugin editor teardown never exits process.

Acceptance:

- Baseview smoke app opens a real embedded/plugin-like surface and renders Hawk2UI scene output.

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

Required remediation:

- Define and implement plugin identity, editor metadata, parameters, presets, UI preferences, DSP/UI state boundary, generated parameter UI, host-safe diagnostics, and packaging outputs.

Acceptance:

- Plugin product manifests validate and build into selected plugin targets.

### REM-PLUGIN-002: Implement Plugin Format Adapters

Evidence:

- `hawk2ui-plugin-adapters` materializes package-like file layouts but does not use real CLAP/VST3/AU/LV2 APIs.

Required remediation:

- Implement selected format adapters with lifecycle callbacks, editor attachment, parameter binding, state, packaging, signing/notarization where applicable, and host tests.

Acceptance:

- At least one real plugin format loads in a test host with Hawk2UI editor rendering.

### REM-PLUGIN-003: Realtime Visual Data Channels

Evidence:

- Records exist for realtime visual packets/transports.
- No lock-free/preallocated implementation is wired.

Required remediation:

- Implement meter/analyzer/waveform/scope/modulation channels with preallocated non-blocking behavior.
- Define drop policy and UI frame-rate reduction.

Acceptance:

- Tests prove audio-thread push cannot block or allocate.

### REM-PLUGIN-004: Audio Thread Safety Policy

Evidence:

- Product direction requires no audio-thread blocking.
- Enforcement is not complete.

Required remediation:

- Define forbidden operations, allocation checks, lock policy, telemetry, and tests.
- Add static/dynamic checks where possible.

Acceptance:

- Audio-thread safety tests gate plugin release.

## Platform API Remediation

### REM-API-001: Complete Capability-Gated Platform APIs

Evidence:

- `hawk2ui-platform` contains capability/filesystem/network/database/clipboard/secrets records.
- Real runtime binding and OS behavior are incomplete.

Required remediation:

- Implement filesystem scoped handles, network allowlists, database migrations, clipboard text/image, audio playback, secrets, AI provider wrappers, MCP integration, notifications, global shortcuts, localization, dialogs, and file pickers according to capability policy.

Acceptance:

- Every platform API has allow/deny tests and user-facing diagnostics.

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

Required remediation:

- Implement artifact signing or signature policy, checksum manifests, reproducible build checks, dependency policy, and release verification reports.

Acceptance:

- Tampered packages are rejected before execution.

## Accessibility Remediation

### REM-A11Y-001: Complete Accessibility Architecture

Evidence:

- A11y records exist for roles, actions, tree, host export snapshots, and plugin guards.
- AccessKit and OS bridge are absent.

Required remediation:

- Build accessibility tree from runtime scene/layout/component semantics.
- Export to AccessKit where supported.
- Route actions back into event/component systems.
- Define plugin host limitations explicitly.

Acceptance:

- Accessible names, roles, focus, bounds, and actions update with scene changes.

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

- Testkit visual snapshot metadata exists.
- No complete deterministic golden image renderer is wired to production scene output.

Required remediation:

- Implement headless Skia rendering of runtime scenes.
- Define golden image tolerances, font fixtures, CI behavior, and failure diagnostics.

Acceptance:

- Visual regressions fail with actionable diff artifacts.

### REM-TEST-003: Performance Benchmarks

Evidence:

- Performance records and crate exist.
- Production budgets for startup, style, layout, render, JS, asset load, and plugin realtime are incomplete.

Required remediation:

- Add Criterion or equivalent benchmarks and release budgets.

Acceptance:

- Performance regressions are visible and release-blocking where budgets are exceeded.

## Developer Experience And Manual Remediation

### REM-DX-001: Complete CLI Command Surface

Evidence:

- CLI can validate/run some paths.
- Full `new`, `dev`, `build`, `validate`, `run`, package, diagnostics, explain, and exit-code behavior is incomplete.

Required remediation:

- Implement full CLI commands with stable exit codes and source diagnostics.

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

## Implementation Priority Order

This order exists to avoid building on sand. It is sequencing, not scope reduction.

1. Documentation/spec alignment: create missing specs and decision records.
2. Style compiler: Lightning CSS-backed parser and typed lowering.
3. Layout engine: Taffy-backed layout and text measurement integration.
4. Runtime-to-render path: runtime scene frame to Skia backend to Winit presentation.
5. Typed paint commands and complete Skia layer execution.
6. Real JS runtime: Boa spike and capability enforcement.
7. Real framework compiler/runtime adapters, starting with custom renderer API and Svelte 5.
8. Build pipeline: manifests, schemas, sealed artifacts, asset/style/script compilation.
9. Host backends: Winit lifecycle completion, Baseview embedded plugin lifecycle, platform matrices.
10. Plugin vertical: realtime channels, generated UI, one real format adapter, package validation.
11. Accessibility: AccessKit bridge and action routing.
12. Security/trust: package integrity, sandbox, untrusted package handling, supply chain gates.
13. Platform APIs: capability-gated filesystem/network/database/clipboard/secrets/etc.
14. Visual quality: premium templates, graph/custom surfaces, animation, visual regression.
15. Release readiness: performance budgets, compatibility matrix, manual, troubleshooting, CI gates.

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
