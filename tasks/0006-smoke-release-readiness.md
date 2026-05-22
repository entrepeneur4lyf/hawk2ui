# Task List 0006: Smoke Release Readiness

## Purpose

Track final production-readiness work for smoke apps, visual fixtures, security denial fixtures, package verification, release evidence, and full release gates.

## Sources

- Spec: `specs/0018-smoke-apps-and-fixtures.md`
- Spec: `specs/0019-release-readiness.md`
- Spec: `specs/0020-manual-completion.md`
- Plan: `docs/superpowers/plans/2026-05-22-0019-smoke-apps-and-fixtures-plan.md`
- Plan: `docs/superpowers/plans/2026-05-22-0020-release-readiness-plan.md`

## Tasks

### 0006.1 Desktop Smoke Apps

- [ ] Deliverable: complete desktop app, dense dashboard app, build verification, scene creation, first frame export, visual snapshots, focus, pointer, resize, and package verification.
- [ ] Dependencies: `0004.2`, `0005.3`.
- [ ] Verify: `rtk cargo test -p hawk2ui-smoke desktop_basic desktop_dashboard`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0006.2 Plugin Smoke Apps

- [ ] Deliverable: plugin synth editor, plugin realtime meter/analyzer fixture, editor lifecycle, attachment, resize, DPI, parameter updates, automation, state, presets, and realtime data.
- [ ] Dependencies: `0004.6`.
- [ ] Verify: `rtk cargo test -p hawk2ui-smoke plugin_synth_editor plugin_meter_analyzer`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0006.3 Style Asset Visual Gallery

- [ ] Deliverable: style gallery covering typography, color, borders, radii, shadows, transforms, opacity, overflow, transitions, tokens, image layers, vector layers, and custom draw surfaces.
- [ ] Dependencies: `0002.6`.
- [ ] Verify: `rtk cargo test -p hawk2ui-smoke style_gallery`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0006.4 Security Denial Smoke Fixtures

- [ ] Deliverable: denied fixtures for undeclared filesystem, denied network, denied clipboard, secret redaction, unsafe asset, unsupported style, invalid manifest, and denied runtime host API.
- [ ] Dependencies: `0003.6`.
- [ ] Verify: `rtk cargo test -p hawk2ui-smoke security_denials`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0006.5 Package Verification

- [ ] Deliverable: sealed artifact verification, desktop bundle verification, plugin bundle verification, metadata verification, hash verification, signing status, notarization status, and verification report inclusion.
- [ ] Dependencies: `0006.1`, `0006.2`.
- [ ] Verify: `rtk bash scripts/release-check.sh --packages-only`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0006.6 Full Release Readiness

- [ ] Deliverable: release evidence for formatting, linting, tests, visual regression, security rejection, compatibility, performance, dependency hygiene, artifacts, smoke apps, manuals, and packages.
- [ ] Dependencies: `0000.6`, `0005.6`, `0006.5`.
- [ ] Verify: `rtk bash scripts/release-check.sh`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.
