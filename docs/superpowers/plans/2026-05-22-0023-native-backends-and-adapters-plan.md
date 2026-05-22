# Native Backends And Adapters Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement production renderer, text, asset, host, script, and package adapters that satisfy Hawk2UI public contracts and smoke fixtures.

**Architecture:** Production adapters share behavioral tests with recording adapters and report capability gaps through diagnostics. Adapter implementations remain behind public contracts so author-facing APIs stay backend-neutral.

**Tech Stack:** Rust, skia-safe, fontdb, parley, swash, image codecs, SVG validation, winit, baseview, Boa or approved script backend, CLAP/VST3/AU adapter crates, cargo test.

---

## File Structure

- Create: `crates/hawk2ui-render-skia/src/lib.rs` Skia renderer backend.
- Create: `crates/hawk2ui-text/src/lib.rs` text backend.
- Create: `crates/hawk2ui-assets/src/lib.rs` asset backend.
- Create: `crates/hawk2ui-host-winit/src/lib.rs` desktop host adapter.
- Create: `crates/hawk2ui-host-baseview/src/lib.rs` plugin host adapter.
- Create: `crates/hawk2ui-script/src/lib.rs` script backend.
- Create: `crates/hawk2ui-plugin-adapters/src/lib.rs` plugin/package adapters.

## Tasks

### Task 1: Skia Renderer Backend

- [ ] Implement surfaces, resize, DPI, frame lifecycle, fills, strokes, paths, clips, transforms, text, images, vectors, effects, dirty regions, capabilities, and diagnostics.
- [ ] Add shared behavior tests against recording backend fixtures.
- [ ] Run: `rtk cargo test -p hawk2ui-render-skia`.
- [ ] Commit: `Add Skia renderer backend`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 2: Text Backend

- [ ] Implement font discovery, app font loading, fallback font selection, shaping, line breaking, bidirectional text, glyph cache integration, high-DPI metrics, and invalidation keys.
- [ ] Add text fixtures for Latin, emoji, combining marks, bidirectional text, wrapping, truncation, and high-DPI measurement.
- [ ] Run: `rtk cargo test -p hawk2ui-text`.
- [ ] Commit: `Add production text backend`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 3: Asset Backend

- [ ] Implement image decoding, metadata stripping, vector validation, vector lowering, font loading, hash verification, size limit enforcement, cache invalidation, and artifact integration.
- [ ] Add asset fixtures for valid images, oversized images, unsafe vectors, invalid hashes, fonts, and cache invalidation.
- [ ] Run: `rtk cargo test -p hawk2ui-assets`.
- [ ] Commit: `Add production asset backend`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 4: Winit Desktop Host Adapter

- [ ] Implement owned windows, close, minimize, maximize, fullscreen, focus, keyboard, pointer, clipboard capability, DPI, resize, repaint, and renderer target recreation.
- [ ] Add host lifecycle tests for Linux Wayland, Linux X11/XCB, Windows HWND, and macOS window behavior through platform fixtures.
- [ ] Run: `rtk cargo test -p hawk2ui-host-winit`.
- [ ] Commit: `Add winit desktop host adapter`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 5: Baseview Plugin Host Adapter

- [ ] Implement embedded parent attachment, editor create/destroy, host resize, DPI, repaint scheduling, focus routing, keyboard routing, pointer routing, renderer target recreation, and safe teardown.
- [ ] Add plugin host lifecycle fixtures for DAW-owned surfaces and teardown without process quit behavior.
- [ ] Run: `rtk cargo test -p hawk2ui-host-baseview`.
- [ ] Commit: `Add baseview plugin host adapter`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 6: Script Backend

- [ ] Implement JavaScript module loading, TypeScript-compiled module execution, promises, timers, typed host calls, structured data exchange, interruption, teardown, and sandbox enforcement.
- [ ] Add runtime fixtures for modules, promises, timers, denied host calls, interruption, teardown, and error diagnostics.
- [ ] Run: `rtk cargo test -p hawk2ui-script`.
- [ ] Commit: `Add production script backend`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 7: Plugin And Package Adapters

- [ ] Implement CLAP, VST3, AU, standalone, desktop bundle, sealed artifact, package metadata, target metadata, and verification report outputs.
- [ ] Add package fixtures for each supported package target and host lifecycle smoke tests for plugin bundles.
- [ ] Run: `rtk cargo test -p hawk2ui-plugin-adapters`.
- [ ] Commit: `Add production plugin and package adapters`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

## Verification

- [ ] Run: `rtk cargo test --workspace adapters`.
- [ ] Run: `rtk cargo test -p hawk2ui-smoke desktop_basic plugin_synth_editor`.
