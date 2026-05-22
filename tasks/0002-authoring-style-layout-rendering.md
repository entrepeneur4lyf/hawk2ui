# Task List 0002: Authoring Style Layout Rendering

## Purpose

Track source-to-scene implementation work for authoring records, style compilation, layout calculation, retained scenes, and render export.

## Sources

- Spec: `specs/0002-authoring.md`
- Spec: `specs/0003-rendering.md`
- Spec: `specs/0005-style.md`
- Spec: `specs/0006-layout.md`
- Plan: `docs/superpowers/plans/2026-05-22-0002-authoring-plan.md`
- Plan: `docs/superpowers/plans/2026-05-22-0003-rendering-plan.md`
- Plan: `docs/superpowers/plans/2026-05-22-0005-style-plan.md`
- Plan: `docs/superpowers/plans/2026-05-22-0006-layout-plan.md`

## Tasks

### 0002.1 Authoring Records And Compiler

- [ ] Deliverable: element records, component records, custom draw surface records, event bindings, state records, source compiler, and framework adapter contract.
- [ ] Dependencies: `0001.4`.
- [ ] Verify: `rtk cargo test -p hawk2ui-authoring`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0002.2 Style Compiler

- [ ] Deliverable: typed property registry, selector subset, token records, style compiler, and runtime typed style table.
- [ ] Dependencies: `0002.1`.
- [ ] Verify: `rtk cargo test -p hawk2ui-style`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0002.3 Layout Engine

- [ ] Deliverable: layout tree records, flex/scroll calculation, text measurement bridge, plugin editor constraints, and scene geometry attachment.
- [ ] Dependencies: `0002.1`, `0002.2`.
- [ ] Verify: `rtk cargo test -p hawk2ui-layout`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0002.4 Retained Scene And Layers

- [ ] Deliverable: scene graph, layer records, invalidation, hit-test geometry, accessibility geometry references, and deterministic layer ordering.
- [ ] Dependencies: `0002.3`.
- [ ] Verify: `rtk cargo test -p hawk2ui-render scene_graph layer_records`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0002.5 Renderer Boundary And Paint Export

- [ ] Deliverable: backend boundary, recording backend, scene-to-paint export, text contracts, asset draw records, and custom draw surface integration.
- [ ] Dependencies: `0002.4`.
- [ ] Verify: `rtk cargo test -p hawk2ui-render`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0002.6 Source To Render Integration

- [ ] Deliverable: integration tests covering source to artifact, artifact to scene, scene to rendered output, and deterministic visual fixtures.
- [ ] Dependencies: `0001.3`, `0002.5`.
- [ ] Verify: `rtk cargo test --test source_to_render`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.
