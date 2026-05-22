# Compatibility Matrix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Define and test the supported operating systems, graphics paths, plugin formats, host environments, DPI modes, accessibility paths, and packaging targets.

**Architecture:** Compatibility data is machine-readable and drives CI, smoke apps, manual test checklists, release notes, and unsupported-target diagnostics.

**Tech Stack:** TOML or JSON matrix files, Rust validation crate, CI matrix generation, cargo test.

---

## File Structure

- Create: `compatibility/matrix.toml` supported target matrix.
- Create: `compatibility/hosts.toml` plugin host and DAW matrix.
- Create: `compatibility/graphics.toml` rendering backend matrix.
- Create: `compatibility/packages.toml` package output matrix.
- Create: `crates/hawk2ui-compat/src/lib.rs` matrix loader and validator.
- Create: `crates/hawk2ui-compat/tests/matrix_validation.rs` matrix tests.
- Create: `docs/development/compatibility.md` compatibility manual.

## Tasks

### Task 1: OS Matrix

- [ ] Add supported Windows, macOS, and Linux versions with architecture, windowing, accessibility, packaging, and CI coverage fields.
- [ ] Add validation for duplicate targets and missing required fields.
- [ ] Run: `rtk cargo test -p hawk2ui-compat os_matrix`.
- [ ] Commit: `Add operating system compatibility matrix`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 2: Graphics Matrix

- [ ] Add rendering backend coverage for CPU raster, GPU surfaces, high-DPI, text shaping, image layers, vector layers, effects, and explicit capability diagnostics.
- [ ] Add tests that every rendering feature maps to at least one supported backend path.
- [ ] Run: `rtk cargo test -p hawk2ui-compat graphics_matrix`.
- [ ] Commit: `Add graphics compatibility matrix`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 3: Plugin Host Matrix

- [ ] Add CLAP, VST3, AU, and standalone target rows with host attachment, resize, DPI, keyboard focus, accessibility, state, automation, and realtime visual data coverage fields.
- [ ] Add tests that every format has editor lifecycle and state coverage declared.
- [ ] Run: `rtk cargo test -p hawk2ui-compat plugin_host_matrix`.
- [ ] Commit: `Add plugin host compatibility matrix`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 4: Packaging Matrix

- [ ] Add desktop app bundles, plugin bundles, sealed artifacts, debug bundles, release bundles, signing, notarization, and installer fields.
- [ ] Add tests that each supported target has a package output and verification command.
- [ ] Run: `rtk cargo test -p hawk2ui-compat packaging_matrix`.
- [ ] Commit: `Add package compatibility matrix`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 5: Unsupported Target Diagnostics

- [ ] Add validator output that explains unsupported target, unsupported capability, and missing package path failures.
- [ ] Add snapshot tests for diagnostics emitted from matrix validation.
- [ ] Run: `rtk cargo test -p hawk2ui-compat unsupported_diagnostics`.
- [ ] Commit: `Add compatibility diagnostics`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

## Verification

- [ ] Run: `rtk cargo test -p hawk2ui-compat`.
- [ ] Run: `rtk cargo test --workspace compatibility`.
