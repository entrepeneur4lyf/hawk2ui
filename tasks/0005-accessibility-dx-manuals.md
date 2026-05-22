# Task List 0005: Accessibility Developer Experience Manuals

## Purpose

Track implementation work for accessibility, CLI workflows, diagnostics, development loop, user manuals, developer manuals, examples, and documentation verification.

## Sources

- Spec: `specs/0012-accessibility.md`
- Spec: `specs/0013-developer-experience.md`
- Spec: `specs/0020-manual-completion.md`
- Plan: `docs/superpowers/plans/2026-05-22-0012-accessibility-plan.md`
- Plan: `docs/superpowers/plans/2026-05-22-0013-developer-experience-plan.md`
- Plan: `docs/superpowers/plans/2026-05-22-0021-manual-completion-plan.md`

## Tasks

### 0005.1 Accessibility Tree And Semantics

- [ ] Deliverable: accessibility tree records, component semantics, custom control semantics, action dispatch, values, labels, focus, and bounds.
- [ ] Dependencies: `0002.3`, `0002.4`.
- [ ] Verify: `rtk cargo test -p hawk2ui-a11y tree_records component_semantics actions_values`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0005.2 Accessibility Host Export

- [ ] Deliverable: desktop host export hooks, plugin editor accessibility safety, geometry updates, and host-safe accessibility tests.
- [ ] Dependencies: `0004.3`, `0005.1`.
- [ ] Verify: `rtk cargo test -p hawk2ui-a11y host_export plugin_accessibility_safety`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0005.3 CLI And Diagnostics

- [ ] Deliverable: CLI commands, diagnostics rendering, validation/build commands, artifact verification command, desktop execution command, plugin packaging command, and meaningful exit codes.
- [ ] Dependencies: `0001.3`, `0003.6`.
- [ ] Verify: `rtk cargo test -p hawk2ui-cli cli_commands diagnostics build_commands`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0005.4 Development Loop

- [ ] Deliverable: file watching, incremental rebuilds, validation before runtime update, native surface reload, safe state preservation, and visible error reporting.
- [ ] Dependencies: `0005.3`, `0004.2`.
- [ ] Verify: `rtk cargo test -p hawk2ui-cli dev_loop`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0005.5 User Manuals

- [ ] Deliverable: getting started, desktop app guide, plugin editor guide, style reference, layout reference, runtime API guide, platform capability reference, packaging guide, troubleshooting guide, and examples index.
- [ ] Dependencies: `0005.3`.
- [ ] Verify: `rtk cargo test --workspace manual_links manual_desktop_commands manual_plugin_examples`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0005.6 Developer Manuals

- [ ] Deliverable: crate map, API contracts, artifact schemas, renderer boundary, host boundary, runtime boundary, plugin boundary, security model, compatibility matrix, performance gates, release process, and contribution workflow.
- [ ] Dependencies: `0005.5`, `0000.6`.
- [ ] Verify: `rtk cargo test --workspace manual_api_coverage`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.
