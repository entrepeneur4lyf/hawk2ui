# Rendering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a retained scene model, renderer boundary, deterministic paint export, text hooks, asset draw records, and custom draw surface integration.

**Architecture:** Scene ownership lives above renderer backends. Backends consume command records and report capabilities while public author APIs never expose backend-specific types.

**Tech Stack:** Rust, serde, smallvec, kurbo or skia-safe behind private backend modules, fontdb and parley behind renderer text traits, insta snapshots.

---

## File Structure

- Create: `crates/hawk2ui-render/src/lib.rs` rendering API.
- Create: `crates/hawk2ui-render/src/scene.rs` retained scene records.
- Create: `crates/hawk2ui-render/src/layer.rs` paint layer records.
- Create: `crates/hawk2ui-render/src/backend.rs` renderer backend trait.
- Create: `crates/hawk2ui-render/src/text.rs` text measurement and draw records.
- Create: `crates/hawk2ui-render/src/assets.rs` compiled asset draw records.
- Create: `crates/hawk2ui-render/src/custom_surface.rs` custom draw surface records.
- Create: `crates/hawk2ui-render/tests/render_export.rs` deterministic export tests.

## Tasks

### Task 1: Scene Graph Records

- [ ] Define scene node identity, parent/child hierarchy, layout attachment, z-order, clipping, transforms, opacity, hit-test geometry, invalidation, and accessibility geometry references.
- [ ] Test parent/child mutation, z-order sorting, and invalidation propagation.
- [ ] Run: `rtk cargo test -p hawk2ui-render scene_graph`.
- [ ] Commit: `Add retained scene graph records`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 2: Layer Records

- [ ] Add layer records for fills, strokes, rounded rectangles, paths, gradients, shadows, glows, opacity groups, clips, transforms, text, image, vector, controls, custom surfaces, static cache, and live layers.
- [ ] Add snapshot tests for deterministic layer ordering and serialization.
- [ ] Run: `rtk cargo test -p hawk2ui-render layer_records`.
- [ ] Commit: `Add deterministic layer records`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 3: Renderer Backend Boundary

- [ ] Define a private backend trait for surface create/teardown, resize, DPI, frame begin/end, clear, fill, stroke, path, text, image, clip, transform, layer effects, cache handles, dirty regions, capability reports, and diagnostics.
- [ ] Implement a `RecordingBackend` used only by tests.
- [ ] Run: `rtk cargo test -p hawk2ui-render backend_boundary`.
- [ ] Commit: `Add renderer backend boundary`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 4: Paint Export

- [ ] Implement scene-to-paint export that produces stable command records from prepared scene data.
- [ ] Add fixtures for text, shapes, gradients, image layers, vector layers, and custom surfaces.
- [ ] Run: `rtk cargo test -p hawk2ui-render render_export`.
- [ ] Commit: `Export scene paint commands`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 5: Text Integration Contracts

- [ ] Add text measurement input/output records for font discovery, app fonts, shaping, line breaking, bidi, glyph cache keying, and high-DPI metrics.
- [ ] Add deterministic measurer harness tests for layout-facing width, height, baseline, and invalidation keys.
- [ ] Run: `rtk cargo test -p hawk2ui-render text_contracts`.
- [ ] Commit: `Add text rendering contracts`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 6: Asset Draw Records

- [ ] Add compiled asset records with stable IDs, source path metadata, hashes, dimensions, sanitization status, backend requirements, packaging metadata, and cache invalidation metadata.
- [ ] Add tests that image, vector, and font assets render only through compiled records.
- [ ] Run: `rtk cargo test -p hawk2ui-render asset_records`.
- [ ] Commit: `Add compiled asset render records`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 7: Custom Draw Surface Integration

- [ ] Add custom surface categories for knobs, sliders, meters, scopes, analyzers, EQ curves, modulation, timelines, graph editors, and inspector panels.
- [ ] Test hit-test, layout reservation, invalidation, frame scheduling, and capability reporting for custom surfaces.
- [ ] Run: `rtk cargo test -p hawk2ui-render custom_surface`.
- [ ] Commit: `Integrate custom draw surfaces`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

## Verification

- [ ] Run: `rtk cargo fmt --all -- --check`.
- [ ] Run: `rtk cargo test -p hawk2ui-render`.
- [ ] Run: `rtk cargo test --workspace render`.
