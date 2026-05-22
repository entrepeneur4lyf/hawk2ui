# Smoke Apps And Fixtures Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build real smoke applications and plugin editor fixtures that exercise the production pipeline from source to artifact to native surface.

**Architecture:** Smoke apps are small, deterministic, and visual. They cover desktop, plugin editor, dense controls, graph surfaces, styling, assets, accessibility, platform capabilities, and security denials.

**Tech Stack:** Hawk2UI source format, TypeScript, style source, compiled artifacts, Rust smoke runner, screenshot/paint snapshots.

---

## File Structure

- Create: `examples/desktop-basic/` complete desktop app.
- Create: `examples/desktop-dashboard/` dense dashboard app.
- Create: `examples/plugin-synth-editor/` plugin editor fixture.
- Create: `examples/plugin-meter-analyzer/` realtime visual fixture.
- Create: `examples/style-gallery/` style and asset gallery.
- Create: `examples/security-denials/` denied capability fixtures.
- Create: `crates/hawk2ui-smoke/src/lib.rs` smoke runner.
- Create: `crates/hawk2ui-smoke/tests/smoke_apps.rs` smoke tests.

## Tasks

### Task 1: Basic Desktop App

- [ ] Add a desktop app with manifest, source entry, styles, tokens, image asset, vector asset, and runtime event handler.
- [ ] Add smoke test for build, artifact verification, scene creation, first frame export, and window lifecycle recording.
- [ ] Run: `rtk cargo test -p hawk2ui-smoke desktop_basic`.
- [ ] Commit: `Add basic desktop smoke app`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 2: Dense Dashboard App

- [ ] Add a dashboard with panels, lists, buttons, sliders, graphs, scroll containers, typography, themes, and resize behavior.
- [ ] Add smoke test for layout, style, scene export, visual snapshot, keyboard focus, and pointer events.
- [ ] Run: `rtk cargo test -p hawk2ui-smoke desktop_dashboard`.
- [ ] Commit: `Add dashboard smoke app`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 3: Plugin Synth Editor

- [ ] Add plugin editor fixture with parameter metadata, generated controls, custom controls, automation gestures, state save/load, and preset metadata.
- [ ] Add smoke test for editor create, attach, resize, DPI update, parameter update, automation gesture, and safe destroy.
- [ ] Run: `rtk cargo test -p hawk2ui-smoke plugin_synth_editor`.
- [ ] Commit: `Add plugin editor smoke fixture`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 4: Realtime Visual Fixture

- [ ] Add meter, analyzer, scope, modulation, and frame-drop tolerance fixture data.
- [ ] Add smoke test proving non-blocking audio-thread writes and UI-side frame consumption.
- [ ] Run: `rtk cargo test -p hawk2ui-smoke plugin_meter_analyzer`.
- [ ] Commit: `Add realtime visual smoke fixture`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 5: Style Gallery

- [ ] Add style gallery covering typography, color, borders, radii, shadows, transforms, opacity, overflow, transitions, tokens, image layers, vector layers, and custom draw surfaces.
- [ ] Add deterministic visual snapshots for each gallery section.
- [ ] Run: `rtk cargo test -p hawk2ui-smoke style_gallery`.
- [ ] Commit: `Add style gallery smoke app`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 6: Security Denial Fixtures

- [ ] Add denied fixtures for undeclared filesystem, denied network, denied clipboard, secret diagnostic redaction, unsafe asset, unsupported style, and invalid manifest.
- [ ] Add tests proving each fixture fails before runtime surface launch.
- [ ] Run: `rtk cargo test -p hawk2ui-smoke security_denials`.
- [ ] Commit: `Add security denial smoke fixtures`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

## Verification

- [ ] Run: `rtk cargo test -p hawk2ui-smoke`.
- [ ] Run: `rtk cargo test --workspace smoke`.
