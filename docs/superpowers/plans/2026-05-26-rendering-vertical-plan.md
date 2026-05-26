# Rendering Vertical Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Skia renderer visibly testable and materially real for text placement, image/vector rendering, layer effects, cache behavior, invalidation, and frame presentation semantics.

**Architecture:** Keep the existing `RendererBackend` trait stable for current callers, and add production Skia-specific draw/readback APIs where the generic trait lacks geometry or material detail. Back the new APIs with real Skia raster operations and pixel readback so tests can assert actual rendered output rather than command logs.

**Tech Stack:** Rust, `hawk2ui-render`, `hawk2ui-render-skia`, `skia-safe` CPU raster surfaces, cargo test/clippy.

---

## File Structure

- `crates/hawk2ui-render-skia/src/lib.rs`: add frame snapshots, placed text/image/vector/effect/cache APIs, and stricter frame presentation state.
- `crates/hawk2ui-render-skia/tests/skia_backend.rs`: add red/green tests that inspect pixels and lifecycle state.
- `docs/superpowers/plans/2026-05-26-rendering-vertical-plan.md`: this implementation plan.

## Task 1: Frame Snapshot And Presentation Semantics

**Files:**
- Modify: `crates/hawk2ui-render-skia/src/lib.rs`
- Modify: `crates/hawk2ui-render-skia/tests/skia_backend.rs`

- [ ] Write failing tests for reading the presented frame pixels and rejecting readback while a frame is still active.
- [ ] Run: `cargo test -p hawk2ui-render-skia frame_snapshot_reads_presented_pixels_and_enforces_lifecycle`
- [ ] Add `SkiaFrameSnapshot`, `SkiaSurface::last_presented_frame`, and `SkiaRendererBackend::frame_snapshot` using Skia `read_pixels` after `end_frame`.
- [ ] Run: `cargo test -p hawk2ui-render-skia frame_snapshot_reads_presented_pixels_and_enforces_lifecycle`
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production-ready stability? If revision is needed, take corrective action.

## Task 2: Placed Text And Image Rendering

**Files:**
- Modify: `crates/hawk2ui-render-skia/src/lib.rs`
- Modify: `crates/hawk2ui-render-skia/tests/skia_backend.rs`

- [ ] Write failing tests that draw text at a non-zero point and draw a registered image into a target rectangle, then assert non-background pixels inside the target region.
- [ ] Run: `cargo test -p hawk2ui-render-skia placed_text_and_images_render_into_target_regions`
- [ ] Add `draw_text_at` and `draw_image_rect` APIs with color, font size, and geometry validation.
- [ ] Run: `cargo test -p hawk2ui-render-skia placed_text_and_images_render_into_target_regions`
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production-ready stability? If revision is needed, take corrective action.

## Task 3: Vector, Gradient, And Layer Effects

**Files:**
- Modify: `crates/hawk2ui-render-skia/src/lib.rs`
- Modify: `crates/hawk2ui-render-skia/tests/skia_backend.rs`

- [ ] Write failing tests that draw a filled vector path, rounded rectangle, gradient, shadow, and glow, then assert visible pixels in the expected regions.
- [ ] Run: `cargo test -p hawk2ui-render-skia vector_gradient_and_effects_render_pixels`
- [ ] Add Skia-backed APIs for filled paths, rounded rectangles, linear gradients, shadow rectangles, and glow rectangles.
- [ ] Run: `cargo test -p hawk2ui-render-skia vector_gradient_and_effects_render_pixels`
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production-ready stability? If revision is needed, take corrective action.

## Task 4: Cache And Invalidation Behavior

**Files:**
- Modify: `crates/hawk2ui-render-skia/src/lib.rs`
- Modify: `crates/hawk2ui-render-skia/tests/skia_backend.rs`

- [ ] Write failing tests for creating a cached raster layer, drawing it back to the surface, invalidating it, and proving invalidated caches cannot be reused.
- [ ] Run: `cargo test -p hawk2ui-render-skia cache_lifecycle_tracks_generation_and_invalidation`
- [ ] Add cache records with generation, size, validity, and real Skia image snapshots for cache replay.
- [ ] Run: `cargo test -p hawk2ui-render-skia cache_lifecycle_tracks_generation_and_invalidation`
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production-ready stability? If revision is needed, take corrective action.

## Task 5: Full Verification And Commit

**Files:**
- All changed files.

- [ ] Run: `cargo fmt --all --check`
- [ ] Run: `cargo test -p hawk2ui-render-skia`
- [ ] Run: `cargo test -p hawk2ui-render`
- [ ] Run: `cargo check --workspace`
- [ ] Run: `cargo clippy --workspace -- -D warnings`
- [ ] Run GitNexus change detection before commit.
- [ ] Commit with an imperative subject.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production-ready stability? If revision is needed, take corrective action.

## Self-Review

Spec coverage: this plan maps directly to rendering requirements for text placement, images, vector rendering, effects, caches, invalidation, and presentation semantics.

Placeholder scan: no task contains placeholder implementation steps.

Type consistency: all new APIs are rooted in `SkiaRendererBackend` and tested through `crates/hawk2ui-render-skia/tests/skia_backend.rs`.
