# Manual Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete user-facing and developer-facing manuals for desktop apps, plugin editors, style, layout, rendering, runtime APIs, packaging, security, troubleshooting, and examples.

**Architecture:** Manuals are versioned with the product and validated by link checks, command checks, example checks, and API coverage checks. User docs explain workflows while developer docs explain extension and implementation contracts.

**Tech Stack:** Markdown, mdBook or equivalent static docs tool, link checker, command snippets tested by trycmd as executable checks.

---

## File Structure

- Create: `manual/SUMMARY.md` manual navigation.
- Create: `manual/getting-started.md` first app workflow.
- Create: `manual/desktop-apps.md` desktop app guide.
- Create: `manual/plugin-editors.md` plugin author guide.
- Create: `manual/style-reference.md` style reference.
- Create: `manual/layout-reference.md` layout reference.
- Create: `manual/runtime-apis.md` runtime API guide.
- Create: `manual/packaging.md` package and artifact guide.
- Create: `manual/security.md` capability and security guide.
- Create: `manual/troubleshooting.md` troubleshooting guide.
- Create: `manual/examples.md` example index.
- Create: `docs/development/architecture-contracts.md` developer contract guide.

## Tasks

### Task 1: Manual Navigation

- [ ] Create manual summary with sections for getting started, desktop apps, plugin editors, style, layout, rendering, runtime APIs, packaging, security, troubleshooting, and examples.
- [ ] Add a docs test that every summary link resolves.
- [ ] Run: `rtk cargo test --workspace manual_links`.
- [ ] Commit: `Add manual navigation`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 2: Getting Started And Desktop Guide

- [ ] Write first-app workflow covering project creation, manifest, UI source, styles, assets, validation, dev run, production build, and artifact verification.
- [ ] Write desktop app guide covering windows, resize, DPI, input, clipboard capability, packaging, and diagnostics.
- [ ] Add command snippet tests for documented CLI commands.
- [ ] Run: `rtk cargo test --workspace manual_desktop_commands`.
- [ ] Commit: `Add desktop user manual`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 3: Plugin Author Guide

- [ ] Write plugin editor guide covering manifest metadata, editor size, host attachment, parameters, automation, state, presets, realtime visual data, generated editor, custom editor, and packaging.
- [ ] Add docs tests that plugin guide references smoke plugin examples.
- [ ] Run: `rtk cargo test --workspace manual_plugin_examples`.
- [ ] Commit: `Add plugin author manual`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 4: Style Layout Rendering References

- [ ] Write style reference with supported properties, selector subset, tokens, unsupported syntax behavior, themes, and user preferences.
- [ ] Write layout reference with sizing, flex, scroll, custom measured nodes, text measurement, plugin constraints, and scene geometry.
- [ ] Write rendering reference with scene, layers, text, assets, effects, custom draw surfaces, dirty regions, and diagnostics.
- [ ] Run: `rtk cargo test --workspace manual_reference_links`.
- [ ] Commit: `Add style layout rendering references`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 5: Runtime Platform Security Packaging Guides

- [ ] Write runtime API guide covering modules, host bindings, events, state, scheduler, timers, async tasks, lifecycle, and teardown.
- [ ] Write platform and security guide covering capabilities, filesystem, network, clipboard, secrets, database, denied access, and diagnostics.
- [ ] Write packaging guide covering sealed artifacts, desktop bundles, plugin bundles, verification reports, package targets, signing, and release checks.
- [ ] Run: `rtk cargo test --workspace manual_runtime_security`.
- [ ] Commit: `Add runtime security packaging manuals`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 6: Troubleshooting And API Coverage

- [ ] Write troubleshooting guide for validation failures, style failures, runtime failures, rendering failures, plugin host failures, package failures, and security denials.
- [ ] Add API coverage checks that every public API module has a matching manual or developer guide section.
- [ ] Run: `rtk cargo test --workspace manual_api_coverage`.
- [ ] Commit: `Add troubleshooting and manual coverage checks`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

## Verification

- [ ] Run: `rtk cargo test --workspace manual`.
- [ ] Run: `rtk git diff --check`.
