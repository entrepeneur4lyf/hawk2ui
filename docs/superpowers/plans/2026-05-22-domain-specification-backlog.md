# Domain Specification Backlog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert the domain index into a complete set of focused specs and keep dependency choices current with an explicit crate-selection baseline.

**Architecture:** Treat the repo as documentation-first until the critical architecture decisions are explicit. Domain specs are grouped into waves that reduce risk before code is accepted: decisions first, shared architecture second, runtime/render/host foundations third, product surface and manuals after the foundation is stable.

**Tech Stack:** Markdown specs, crates.io version checks through `cargo search` and `cargo info`, Rust dependency hygiene through `cargo outdated`, `cargo audit`, `cargo deny`, `cargo machete`, and `cargo nextest` once workspace code exists.

---

## Scope Check

`docs/specs/0002-domain-spec-index.md` intentionally covers many independent subsystems. This plan does not try to implement all subsystems in one branch. It turns the index into a sequence of spec-writing waves, with an explicit gate after each wave.

Implementation code starts only after the relevant domain spec, test strategy, security model, and dependency choices for that domain have been written and reviewed.

## Files

Create or modify these files during plan execution:

- Modify: `README.md` to link the domain index and crate-selection document.
- Modify: `docs/specs/0002-domain-spec-index.md` when domain status changes.
- Create: `docs/technical/crate-selection.md` for crate candidates, versions, risk notes, and freshness policy.
- Create: `docs/specs/deft-adoption-decision.md`.
- Create: `docs/specs/prototype-migration.md`.
- Create: `docs/specs/host-abstraction.md`.
- Create: `docs/specs/skia-renderer-abstraction.md`.
- Create: `docs/specs/scene-graph.md`.
- Create: `docs/specs/style-system.md`.
- Create: `docs/specs/layout-architecture.md`.
- Create: `docs/specs/javascript-runtime-choice.md`.
- Create: `docs/specs/manifest-schema.md`.
- Create: `docs/specs/plugin-format-strategy.md`.
- Create: `docs/specs/security-model.md`.
- Create: `docs/specs/test-strategy.md`.
- Create the remaining future spec files listed in `docs/specs/0002-domain-spec-index.md` after the first decision wave is accepted.
- Create user-facing manual files under `docs/manual/` after the corresponding technical specs exist.

## Spec Structure

Every new spec file must use these headings and fill them with domain-specific content before review:

- `# Spec: <domain title>`
- `## Purpose`
- `## Scope`
- `## Non-Goals`
- `## Inputs And Outputs`
- `## Public API`
- `## Internal Architecture`
- `## Lifecycle`
- `## Data Model`
- `## Validation And Errors`
- `## Security And Capability Boundaries`
- `## Compatibility Matrix`
- `## Testing Requirements`
- `## Examples`
- `## Open Questions`
- `## Acceptance Criteria`

Every spec must name concrete interfaces, data structures or schemas, lifecycle states, compatibility cells, and test requirements. A spec is not ready if it still contains generic scaffold prose, empty headings, or undecided dependency choices outside an explicit decision spec.

## Task 1: Crate Baseline Document

**Files:**

- Create: `docs/technical/crate-selection.md`
- Modify: `README.md`

- [ ] **Step 1: Verify current crate versions**

Run:

```bash
for c in skia-safe taffy lightningcss boa_engine deno_core v8 javascriptcore-rs rquickjs winit baseview sdl3 raw-window-handle accesskit parley fontdb swash image image-webp ravif oxipng usvg resvg serde serde_json toml toml_edit schemars jsonschema clap notify cargo_metadata tracing tracing-subscriber thiserror anyhow tokio smol crossbeam-channel ringbuf rtrb vst3 clap-sys nih_plug cpal symphonia midir directories camino camino-tempfile cargo-deny cargo-audit cargo-outdated cargo-machete cargo-nextest insta proptest arbitrary criterion; do
  echo "--- $c"
  cargo search "$c" --limit 3
 done
```

Expected: each selected crate prints a current crates.io result or a clear reason to remove it from the baseline.

- [ ] **Step 2: Write the crate baseline**

Create `docs/technical/crate-selection.md` with sections for dependency policy, rendering/UI pipeline, host/windowing, JavaScript runtimes, assets, data/schema, CLI/diagnostics, realtime/audio/plugin infrastructure, test tooling, freshness risks, and follow-up specs.

- [ ] **Step 3: Link the crate baseline**

Add this line under the README `See:` list:

```markdown
- `docs/technical/crate-selection.md`
```

- [ ] **Step 4: Verify the crate baseline**

Run:

```bash
perl -ne 'print "$ARGV:$.: trailing whitespace\n" if /[ \t]$/; print "$ARGV:$.: tab\n" if /\t/;' docs/technical/crate-selection.md README.md
bash -lc 'pattern="term""inal|Term""inal|T""UI|t""ui|OpenT""UI"; rg -n "$pattern" README.md docs || true'
```

Expected: no trailing whitespace, no tabs, and no excluded product-scope wording.

- [ ] **Step 5: Commit**

Run:

```bash
git add README.md docs/technical/crate-selection.md
git commit -m "Add crate selection baseline"
```

Expected: commit succeeds with only the crate baseline and README link staged.

## Task 2: Decision Specs Wave

**Files:**

- Create: `docs/specs/deft-adoption-decision.md`
- Create: `docs/specs/prototype-migration.md`
- Create: `docs/specs/javascript-runtime-choice.md`
- Modify: `docs/specs/0002-domain-spec-index.md`

- [ ] **Step 1: Write `deft-adoption-decision.md`**

Use the spec skeleton and replace the content with a decision analysis covering adopt, fork, and prior-art-only paths. Include explicit criteria for plugin embedding, framework support, renderer access, styling capability, project maintenance, licensing, and ability to preserve Hawk2UI product boundaries.

- [ ] **Step 2: Write `prototype-migration.md`**

Use the spec skeleton and cover each prototype area: style, layout, text, render, sealed artifacts, validation, host-window experiments, runtime host bindings, platform APIs, docs, tests, and assets. Classify every area as `port`, `redesign`, `discard`, or `research-only`.

- [ ] **Step 3: Write `javascript-runtime-choice.md`**

Use the spec skeleton and compare Boa, Deno/V8, JavaScriptCore, and QuickJS. Include proof criteria for modules, promises, timers, interruption, memory limits, host bindings, source maps, framework workloads, package size, build complexity, and host lifecycle integration.

- [ ] **Step 4: Update domain index statuses**

In `docs/specs/0002-domain-spec-index.md`, update D02, D03, and R01 from `decision-needed` or `prototype-backed` to `spec-written` only after the specs contain concrete acceptance criteria and no generic skeleton text remains.

- [ ] **Step 5: Verify decision specs**

Run:

```bash
perl -ne 'print "$ARGV:$.: trailing whitespace\n" if /[ \t]$/; print "$ARGV:$.: tab\n" if /\t/;' docs/specs/deft-adoption-decision.md docs/specs/prototype-migration.md docs/specs/javascript-runtime-choice.md docs/specs/0002-domain-spec-index.md
bash -lc 'pattern="One ""concrete|One ""explicit|Example ""input|Un""known|Replace ""with"; rg -n "$pattern" docs/specs/deft-adoption-decision.md docs/specs/prototype-migration.md docs/specs/javascript-runtime-choice.md'
```

Expected: no trailing whitespace, no tabs, and no unreplaced skeleton text.

- [ ] **Step 6: Commit**

Run:

```bash
git add docs/specs/deft-adoption-decision.md docs/specs/prototype-migration.md docs/specs/javascript-runtime-choice.md docs/specs/0002-domain-spec-index.md
git commit -m "Add core architecture decision specs"
```

Expected: commit succeeds with only decision wave files staged.

## Task 3: Host And Renderer Foundation Specs

**Files:**

- Create: `docs/specs/host-abstraction.md`
- Create: `docs/specs/skia-renderer-abstraction.md`
- Create: `docs/specs/scene-graph.md`
- Create: `docs/specs/paint-list-boundary.md`
- Create: `docs/specs/renderer-backends.md`
- Modify: `docs/specs/0002-domain-spec-index.md`

- [ ] **Step 1: Write `host-abstraction.md`**

Define the common host-surface contract: creation, attachment, resize, DPI, focus, input, repaint requests, frame presentation, close requests, teardown, blocking and non-blocking pumps, and capability reporting.

- [ ] **Step 2: Write `skia-renderer-abstraction.md`**

Define the renderer trait boundary hiding Skia-specific APIs. Include surface creation, CPU raster, GPU candidate path, text rendering, images, vector primitives, dirty regions, frame timing, resource caches, and backend capabilities.

- [ ] **Step 3: Write `scene-graph.md`**

Define retained scene nodes, identity, layout attachment, z-order, transforms, clipping, opacity, effects, hit testing, invalidation, diffing, and export to paint lists or direct renderer commands.

- [ ] **Step 4: Write `paint-list-boundary.md`**

Define backend-neutral draw commands, command ordering, dirty bounds, text/image/vector commands, debug dumps, deterministic serialization, and visual regression integration.

- [ ] **Step 5: Write `renderer-backends.md`**

Define CPU raster, GPU desktop, plugin-safe surfaces, headless rendering, backend selection, fallback policy, and platform support cells.

- [ ] **Step 6: Verify host/renderer specs**

Run:

```bash
perl -ne 'print "$ARGV:$.: trailing whitespace\n" if /[ \t]$/; print "$ARGV:$.: tab\n" if /\t/;' docs/specs/host-abstraction.md docs/specs/skia-renderer-abstraction.md docs/specs/scene-graph.md docs/specs/paint-list-boundary.md docs/specs/renderer-backends.md docs/specs/0002-domain-spec-index.md
bash -lc 'pattern="One ""concrete|One ""explicit|Example ""input|Un""known|Replace ""with"; rg -n "$pattern" docs/specs/host-abstraction.md docs/specs/skia-renderer-abstraction.md docs/specs/scene-graph.md docs/specs/paint-list-boundary.md docs/specs/renderer-backends.md'
```

Expected: no trailing whitespace, no tabs, and no unreplaced skeleton text.

- [ ] **Step 7: Commit**

Run:

```bash
git add docs/specs/host-abstraction.md docs/specs/skia-renderer-abstraction.md docs/specs/scene-graph.md docs/specs/paint-list-boundary.md docs/specs/renderer-backends.md docs/specs/0002-domain-spec-index.md
git commit -m "Add host and renderer foundation specs"
```

Expected: commit succeeds with only host/renderer wave files staged.

## Task 4: Style And Layout Foundation Specs

**Files:**

- Create: `docs/specs/style-system.md`
- Create: `docs/specs/css-subset-reference.md`
- Create: `docs/specs/css-parsing-transform.md`
- Create: `docs/specs/selector-model.md`
- Create: `docs/specs/style-property-registry.md`
- Create: `docs/specs/layout-architecture.md`
- Create: `docs/specs/flexbox-support.md`
- Create: `docs/specs/text-measurement-layout.md`
- Modify: `docs/specs/0002-domain-spec-index.md`

- [ ] **Step 1: Write style architecture specs**

Write `style-system.md`, `css-subset-reference.md`, `css-parsing-transform.md`, `selector-model.md`, and `style-property-registry.md`. Each spec must reference the prototype audit and define where the prototype is reused versus replaced.

- [ ] **Step 2: Write layout architecture specs**

Write `layout-architecture.md`, `flexbox-support.md`, and `text-measurement-layout.md`. Define Taffy mapping, intrinsic measurement, plugin editor constraints, dense panels, scroll preparation, graph surfaces, layout invalidation, and accessibility geometry.

- [ ] **Step 3: Verify style/layout specs**

Run:

```bash
perl -ne 'print "$ARGV:$.: trailing whitespace\n" if /[ \t]$/; print "$ARGV:$.: tab\n" if /\t/;' docs/specs/style-system.md docs/specs/css-subset-reference.md docs/specs/css-parsing-transform.md docs/specs/selector-model.md docs/specs/style-property-registry.md docs/specs/layout-architecture.md docs/specs/flexbox-support.md docs/specs/text-measurement-layout.md docs/specs/0002-domain-spec-index.md
bash -lc 'pattern="One ""concrete|One ""explicit|Example ""input|Un""known|Replace ""with"; rg -n "$pattern" docs/specs/style-system.md docs/specs/css-subset-reference.md docs/specs/css-parsing-transform.md docs/specs/selector-model.md docs/specs/style-property-registry.md docs/specs/layout-architecture.md docs/specs/flexbox-support.md docs/specs/text-measurement-layout.md'
```

Expected: no trailing whitespace, no tabs, and no unreplaced skeleton text.

- [ ] **Step 4: Commit**

Run:

```bash
git add docs/specs/style-system.md docs/specs/css-subset-reference.md docs/specs/css-parsing-transform.md docs/specs/selector-model.md docs/specs/style-property-registry.md docs/specs/layout-architecture.md docs/specs/flexbox-support.md docs/specs/text-measurement-layout.md docs/specs/0002-domain-spec-index.md
git commit -m "Add style and layout foundation specs"
```

Expected: commit succeeds with only style/layout wave files staged.

## Task 5: Manifest, Security, And Test Foundation Specs

**Files:**

- Create: `docs/specs/manifest-schema.md`
- Create: `docs/specs/security-model.md`
- Create: `docs/specs/source-validation.md`
- Create: `docs/specs/asset-sanitization.md`
- Create: `docs/specs/capability-manifest.md`
- Create: `docs/specs/test-strategy.md`
- Modify: `docs/specs/0002-domain-spec-index.md`

- [ ] **Step 1: Write manifest and capability specs**

Write `manifest-schema.md` and `capability-manifest.md`. Define app identity, plugin identity, capabilities, assets, package metadata, validation timing, schema generation, and compatibility strategy.

- [ ] **Step 2: Write security specs**

Write `security-model.md`, `source-validation.md`, and `asset-sanitization.md`. Define trust boundaries, denied source features, script restrictions, image/vector sanitization, secrets handling, and package trust.

- [ ] **Step 3: Write test strategy**

Write `test-strategy.md`. Define unit, integration, visual, compatibility, fuzz, package, security, performance, and manual test gates. Include how each future implementation domain proves readiness.

- [ ] **Step 4: Verify manifest/security/test specs**

Run:

```bash
perl -ne 'print "$ARGV:$.: trailing whitespace\n" if /[ \t]$/; print "$ARGV:$.: tab\n" if /\t/;' docs/specs/manifest-schema.md docs/specs/security-model.md docs/specs/source-validation.md docs/specs/asset-sanitization.md docs/specs/capability-manifest.md docs/specs/test-strategy.md docs/specs/0002-domain-spec-index.md
bash -lc 'pattern="One ""concrete|One ""explicit|Example ""input|Un""known|Replace ""with"; rg -n "$pattern" docs/specs/manifest-schema.md docs/specs/security-model.md docs/specs/source-validation.md docs/specs/asset-sanitization.md docs/specs/capability-manifest.md docs/specs/test-strategy.md'
```

Expected: no trailing whitespace, no tabs, and no unreplaced skeleton text.

- [ ] **Step 5: Commit**

Run:

```bash
git add docs/specs/manifest-schema.md docs/specs/security-model.md docs/specs/source-validation.md docs/specs/asset-sanitization.md docs/specs/capability-manifest.md docs/specs/test-strategy.md docs/specs/0002-domain-spec-index.md
git commit -m "Add manifest security and test foundation specs"
```

Expected: commit succeeds with only manifest/security/test wave files staged.

## Task 6: Plugin Foundation Specs

**Files:**

- Create: `docs/specs/plugin-format-strategy.md`
- Create: `docs/specs/plugin-product-model.md`
- Create: `docs/specs/plugin-host-backend.md`
- Create: `docs/specs/parameter-model.md`
- Create: `docs/specs/automation-gestures.md`
- Create: `docs/specs/ui-dsp-state-boundary.md`
- Create: `docs/specs/realtime-visual-data.md`
- Create: `docs/specs/audio-thread-safety.md`
- Modify: `docs/specs/0002-domain-spec-index.md`

- [ ] **Step 1: Write plugin strategy specs**

Write `plugin-format-strategy.md`, `plugin-product-model.md`, and `plugin-host-backend.md`. Define VST3, CLAP, AU, LV2, standalone wrapper sequencing, DAW-owned surface lifecycle, packaging goals, and compatibility matrix.

- [ ] **Step 2: Write plugin data and realtime specs**

Write `parameter-model.md`, `automation-gestures.md`, `ui-dsp-state-boundary.md`, `realtime-visual-data.md`, and `audio-thread-safety.md`. Define stable parameter IDs, automation gestures, presets/state, UI-only preferences, realtime-safe channels, and forbidden audio-thread operations.

- [ ] **Step 3: Verify plugin specs**

Run:

```bash
perl -ne 'print "$ARGV:$.: trailing whitespace\n" if /[ \t]$/; print "$ARGV:$.: tab\n" if /\t/;' docs/specs/plugin-format-strategy.md docs/specs/plugin-product-model.md docs/specs/plugin-host-backend.md docs/specs/parameter-model.md docs/specs/automation-gestures.md docs/specs/ui-dsp-state-boundary.md docs/specs/realtime-visual-data.md docs/specs/audio-thread-safety.md docs/specs/0002-domain-spec-index.md
bash -lc 'pattern="One ""concrete|One ""explicit|Example ""input|Un""known|Replace ""with"; rg -n "$pattern" docs/specs/plugin-format-strategy.md docs/specs/plugin-product-model.md docs/specs/plugin-host-backend.md docs/specs/parameter-model.md docs/specs/automation-gestures.md docs/specs/ui-dsp-state-boundary.md docs/specs/realtime-visual-data.md docs/specs/audio-thread-safety.md'
```

Expected: no trailing whitespace, no tabs, and no unreplaced skeleton text.

- [ ] **Step 4: Commit**

Run:

```bash
git add docs/specs/plugin-format-strategy.md docs/specs/plugin-product-model.md docs/specs/plugin-host-backend.md docs/specs/parameter-model.md docs/specs/automation-gestures.md docs/specs/ui-dsp-state-boundary.md docs/specs/realtime-visual-data.md docs/specs/audio-thread-safety.md docs/specs/0002-domain-spec-index.md
git commit -m "Add plugin foundation specs"
```

Expected: commit succeeds with only plugin wave files staged.

## Task 7: Remaining Domain Specs Coverage Pass

**Files:**

- Create: every future spec path listed in `docs/specs/0002-domain-spec-index.md` that does not exist after Tasks 1-6.
- Create: every future manual path listed in `docs/specs/0002-domain-spec-index.md` after the corresponding technical spec exists.
- Modify: `docs/specs/0002-domain-spec-index.md`.

- [ ] **Step 1: Generate missing spec list**

Run:

```bash
perl -ne 'while (/`(docs\/(?:specs|manual)\/[^`]+\.md)`/g) { print "$1\n" }' docs/specs/0002-domain-spec-index.md | sort -u | while read path; do test -f "$path" || echo "$path"; done
```

Expected: prints every spec/manual path that still needs to be written.

- [ ] **Step 2: Write remaining specs by domain group**

For each missing path under `docs/specs/`, write a complete domain spec using the skeleton and replacing every generic line before review. Process groups in this order: authoring/frameworks, source compilation, runtime, remaining style/rendering, host platform adapters, remaining plugin adapters, platform APIs, developer experience, tests/release.

- [ ] **Step 3: Write manuals after specs exist**

For each missing path under `docs/manual/`, write user-facing documentation only after the corresponding technical spec exists. Manuals must explain usage, examples, and troubleshooting, not internal architecture.

- [ ] **Step 4: Verify full coverage**

Run:

```bash
perl -ne 'while (/`(docs\/(?:specs|manual)\/[^`]+\.md)`/g) { print "$1\n" }' docs/specs/0002-domain-spec-index.md | sort -u | while read path; do test -f "$path" || echo "missing: $path"; done
bash -lc 'pattern="One ""concrete|One ""explicit|Example ""input|Un""known|Replace ""with"; rg -n "$pattern" docs/specs docs/manual || true'
perl -ne 'print "$ARGV:$.: trailing whitespace\n" if /[ \t]$/; print "$ARGV:$.: tab\n" if /\t/;' $(find docs -name "*.md" | sort)
```

Expected: no missing paths, no unreplaced skeleton text, no trailing whitespace, and no tabs.

- [ ] **Step 5: Commit by domain group**

Run one commit per completed group:

```bash
git add docs/specs docs/manual
git commit -m "Add remaining <domain> specs"
```

Expected: each commit contains a coherent group of specs or manuals, not an unrelated mix.

## Task 8: Final Documentation Index And Readiness Gate

**Files:**

- Modify: `README.md`
- Modify: `docs/specs/0002-domain-spec-index.md`
- Create: `docs/specs/implementation-readiness.md`

- [ ] **Step 1: Write implementation readiness spec**

Create `docs/specs/implementation-readiness.md` with a checklist for starting code: decision specs accepted, dependency baseline current, security model accepted, test strategy accepted, host/render/style/runtime foundation specs accepted, and first implementation slice chosen.

- [ ] **Step 2: Link readiness spec**

Add `docs/specs/implementation-readiness.md` to the README `See:` list and cross-link it from `docs/specs/0002-domain-spec-index.md`.

- [ ] **Step 3: Verify final docs**

Run:

```bash
perl -ne 'print "$ARGV:$.: trailing whitespace\n" if /[ \t]$/; print "$ARGV:$.: tab\n" if /\t/;' $(find docs -name "*.md" | sort) README.md
bash -lc 'pattern="term""inal|Term""inal|T""UI|t""ui|OpenT""UI"; rg -n "$pattern" README.md docs || true'
perl -ne 'while (/`(docs\/(?:specs|manual|technical)\/[^`]+\.md)`/g) { print "$1\n" }' README.md docs/specs/0002-domain-spec-index.md | sort -u | while read path; do test -f "$path" || echo "missing: $path"; done
```

Expected: no whitespace failures, no excluded product-scope wording, and no missing linked docs.

- [ ] **Step 4: Commit**

Run:

```bash
git add README.md docs/specs/0002-domain-spec-index.md docs/specs/implementation-readiness.md
git commit -m "Add implementation readiness gate"
```

Expected: commit succeeds with only readiness gate files staged.

## Self-Review Checklist

- Every critical early decision has a first-wave spec task.
- Every implementation domain remains blocked until its spec and tests exist.
- The crate baseline is treated as time-sensitive and must be refreshed before dependencies are added.
- The plan avoids committing implementation code before architecture, security, and test foundations are reviewed.
- The final coverage gate mechanically checks that indexed spec/manual paths exist.
