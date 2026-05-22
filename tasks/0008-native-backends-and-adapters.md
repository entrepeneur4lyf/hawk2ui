# Task List 0008: Native Backends And Adapters

## Purpose

Track production implementation work for renderer backend, text backend, asset backend, desktop host adapter, embedded plugin host adapter, script backend, and package adapters.

## Sources

- Spec: `specs/0003-rendering.md`
- Spec: `specs/0004-host-windowing.md`
- Spec: `specs/0007-runtime.md`
- Spec: `specs/0008-build-artifacts.md`
- Spec: `specs/0009-plugin.md`
- Spec: `specs/0022-native-backends-and-adapters.md`
- Plan: `docs/superpowers/plans/2026-05-22-0003-rendering-plan.md`
- Plan: `docs/superpowers/plans/2026-05-22-0004-host-windowing-plan.md`
- Plan: `docs/superpowers/plans/2026-05-22-0007-runtime-plan.md`
- Plan: `docs/superpowers/plans/2026-05-22-0008-build-artifacts-plan.md`
- Plan: `docs/superpowers/plans/2026-05-22-0009-plugin-plan.md`
- Plan: `docs/superpowers/plans/2026-05-22-0023-native-backends-and-adapters-plan.md`

## Tasks

### 0008.1 Skia Renderer Backend

- [ ] Deliverable: Skia-backed renderer implementing surfaces, resize, DPI, frame lifecycle, fills, strokes, paths, clips, transforms, text, images, vectors, effects, dirty regions, capabilities, and diagnostics.
- [ ] Dependencies: `0002.5`, `0000.4`, `0000.5`.
- [ ] Verify: `rtk cargo test -p hawk2ui-render-skia`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0008.2 Production Text Backend

- [ ] Deliverable: font discovery, app font loading, fallback selection, shaping, line breaking, bidirectional text, glyph cache integration, high-DPI metrics, and layout invalidation keys.
- [ ] Dependencies: `0002.3`, `0008.1`.
- [ ] Verify: `rtk cargo test -p hawk2ui-text`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0008.3 Production Asset Backend

- [ ] Deliverable: image decoding, metadata stripping, vector validation, vector lowering, font loading, hash verification, size limit enforcement, cache invalidation, and artifact integration.
- [ ] Dependencies: `0001.3`, `0003.4`, `0008.1`.
- [ ] Verify: `rtk cargo test -p hawk2ui-assets`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0008.4 Winit Desktop Host Adapter

- [ ] Deliverable: winit desktop adapter with owned windows, close, minimize, maximize, fullscreen, focus, keyboard, pointer, clipboard capability, DPI, resize, repaint, and renderer target recreation.
- [ ] Dependencies: `0004.2`, `0008.1`.
- [ ] Verify: `rtk cargo test -p hawk2ui-host-winit`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0008.5 Baseview Plugin Host Adapter

- [ ] Deliverable: baseview embedded adapter with parent attachment, editor create/destroy, host resize, DPI, repaint scheduling, focus routing, keyboard routing, pointer routing, renderer target recreation, and safe teardown.
- [ ] Dependencies: `0004.3`, `0008.1`.
- [ ] Verify: `rtk cargo test -p hawk2ui-host-baseview`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0008.6 Production Script Backend

- [ ] Deliverable: JavaScript module loading, TypeScript-compiled module execution, promises, timers, typed host calls, structured data exchange, interruption, teardown, and sandbox enforcement.
- [ ] Dependencies: `0003.2`, `0003.4`.
- [ ] Verify: `rtk cargo test -p hawk2ui-script`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0008.7 Plugin Format Adapters

- [ ] Deliverable: CLAP, VST3, AU, and standalone adapters that map plugin metadata, editor embedding, parameters, automation, state, presets, realtime visual data, and package output to production bundles.
- [ ] Dependencies: `0004.6`, `0008.5`.
- [ ] Verify: `rtk cargo test -p hawk2ui-plugin-adapters`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.
