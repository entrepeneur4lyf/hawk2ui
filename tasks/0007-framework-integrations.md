# Task List 0007: Framework Integrations

## Purpose

Track production framework integration work for Svelte 5, React 19 and later, Vue 3.5 and later, Solid, and direct Hawk2UI native element authoring.

## Sources

- Spec: `specs/0002-authoring.md`
- Spec: `specs/0007-runtime.md`
- Spec: `specs/0013-developer-experience.md`
- Spec: `specs/0021-framework-integrations.md`
- Plan: `docs/superpowers/plans/2026-05-22-0002-authoring-plan.md`
- Plan: `docs/superpowers/plans/2026-05-22-0007-runtime-plan.md`
- Plan: `docs/superpowers/plans/2026-05-22-0013-developer-experience-plan.md`
- Plan: `docs/superpowers/plans/2026-05-22-0022-framework-integrations-plan.md`

## Tasks

### 0007.1 Direct Native Authoring Runtime

- [ ] Deliverable: direct Hawk2UI native element authoring package, typed element operations, event bindings, lifecycle hooks, refs, keyed children, and diagnostics.
- [ ] Dependencies: `0002.1`, `0003.2`.
- [ ] Verify: `rtk cargo test -p hawk2ui-authoring native_authoring_runtime`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0007.2 Svelte 5 Integration

- [ ] Deliverable: Svelte 5 compiler integration, component lifecycle mapping, keyed child mapping, event mapping, style and asset references, source maps, diagnostics, and smoke example.
- [ ] Dependencies: `0007.1`.
- [ ] Verify: `rtk cargo test -p hawk2ui-framework-svelte`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0007.3 React 19 Integration

- [ ] Deliverable: React 19 custom renderer integration, reconciler bridge, component lifecycle mapping, event mapping, refs, keyed children, diagnostics, and smoke example.
- [ ] Dependencies: `0007.1`.
- [ ] Verify: `rtk cargo test -p hawk2ui-framework-react`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0007.4 Vue 3.5 Integration

- [ ] Deliverable: Vue 3.5 custom renderer integration, component lifecycle mapping, event mapping, refs, keyed children, style and asset references, diagnostics, and smoke example.
- [ ] Dependencies: `0007.1`.
- [ ] Verify: `rtk cargo test -p hawk2ui-framework-vue`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0007.5 Solid Integration

- [ ] Deliverable: Solid renderer integration, fine-grained update mapping, event mapping, refs, keyed children, style and asset references, diagnostics, and smoke example.
- [ ] Dependencies: `0007.1`.
- [ ] Verify: `rtk cargo test -p hawk2ui-framework-solid`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0007.6 Framework Conformance Suite

- [ ] Deliverable: shared conformance tests proving all supported frameworks emit equivalent Hawk2UI records for component lifecycle, events, state, refs, keyed children, style references, and asset references.
- [ ] Dependencies: `0007.2`, `0007.3`, `0007.4`, `0007.5`.
- [ ] Verify: `rtk cargo test --workspace framework_conformance`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.
