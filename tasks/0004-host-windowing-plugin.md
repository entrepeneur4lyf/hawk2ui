# Task List 0004: Host Windowing Plugin

## Purpose

Track implementation work for desktop host surfaces, embedded plugin surfaces, platform handles, renderer resize bridging, plugin metadata, parameters, automation, state, presets, and realtime visual data.

## Sources

- Spec: `specs/0004-host-windowing.md`
- Spec: `specs/0009-plugin.md`
- Spec: `specs/0016-compatibility-matrix.md`
- Spec: `specs/0017-performance-and-stability.md`
- Plan: `docs/superpowers/plans/2026-05-22-0004-host-windowing-plan.md`
- Plan: `docs/superpowers/plans/2026-05-22-0009-plugin-plan.md`

## Tasks

### 0004.1 Common Host Surface Contract

- [ ] Deliverable: host surface metrics, surface events, repaint requests, frame presenter, host capabilities, and lifecycle tests.
- [ ] Dependencies: `0002.5`, `0003.1`.
- [ ] Verify: `rtk cargo test -p hawk2ui-host surface_contract`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0004.2 Desktop Lifecycle

- [ ] Deliverable: owned window creation, close/minimize/maximize/fullscreen events, focus, keyboard, pointer, clipboard capability, DPI, and renderer target recreation.
- [ ] Dependencies: `0004.1`.
- [ ] Verify: `rtk cargo test -p hawk2ui-host desktop_lifecycle renderer_resize_bridge`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0004.3 Plugin Surface Lifecycle

- [ ] Deliverable: parent attachment, editor create/destroy, host resize, DPI, repaint scheduling, focus routing, keyboard routing, pointer routing, and safe teardown.
- [ ] Dependencies: `0004.1`.
- [ ] Verify: `rtk cargo test -p hawk2ui-host plugin_lifecycle platform_handles`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0004.4 Plugin Format And Editor Records

- [ ] Deliverable: CLAP, VST3, AU, standalone targets, generated metadata, package bundle output, editor embedding records, generated editor records, and custom editor records.
- [ ] Dependencies: `0004.3`.
- [ ] Verify: `rtk cargo test -p hawk2ui-plugin format_records editor_embedding`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0004.5 Plugin Parameters Automation State

- [ ] Deliverable: parameter model, normalized conversion, gesture records, automation ordering, versioned state envelopes, migrations, host chunks, and preset separation.
- [ ] Dependencies: `0004.4`.
- [ ] Verify: `rtk cargo test -p hawk2ui-plugin parameter_model automation_events state_presets`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0004.6 Realtime Visual Data

- [ ] Deliverable: preallocated non-blocking transport for meters, analyzers, scopes, modulation, frame-drop tolerance, and audio-thread safety tests.
- [ ] Dependencies: `0004.5`, `0000.5`.
- [ ] Verify: `rtk cargo test -p hawk2ui-plugin realtime_visual_data`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.
