# Accessibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement accessibility tree records, headless component semantics, custom control semantics, host export hooks, plugin-safe accessibility behavior, and tests.

**Architecture:** Accessibility metadata attaches to scene nodes and layout geometry. Host adapters export the tree to platform services while plugin contexts preserve host safety.

**Tech Stack:** Rust, accesskit for desktop export, serde, cargo test, insta snapshots.

---

## File Structure

- Create: `crates/hawk2ui-a11y/src/lib.rs` accessibility API exports.
- Create: `crates/hawk2ui-a11y/src/tree.rs` accessibility tree records.
- Create: `crates/hawk2ui-a11y/src/component.rs` component semantics.
- Create: `crates/hawk2ui-a11y/src/actions.rs` accessibility actions.
- Create: `crates/hawk2ui-a11y/src/host.rs` host export records.
- Create: `crates/hawk2ui-a11y/tests/accessibility_tree.rs` accessibility tests.

## Tasks

### Task 1: Accessibility Tree Records

- [ ] Define records for role, name, description, value, checked, disabled, focus, bounds, actions, and hierarchy.
- [ ] Add tests for tree shape, node identity, bounds, and hierarchy serialization.
- [ ] Run: `rtk cargo test -p hawk2ui-a11y tree_records`.
- [ ] Commit: `Add accessibility tree records`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 2: Component Semantics

- [ ] Add semantics records for headless buttons, sliders, text inputs, checkboxes, lists, panels, and custom controls.
- [ ] Add tests that semantics exist independently of visual styles.
- [ ] Run: `rtk cargo test -p hawk2ui-a11y component_semantics`.
- [ ] Commit: `Add headless component semantics`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 3: Actions And Values

- [ ] Add action dispatch records for focus, press, increment, decrement, set value, and custom action.
- [ ] Add tests for role assignment, label assignment, focus changes, value updates, and action dispatch.
- [ ] Run: `rtk cargo test -p hawk2ui-a11y actions_values`.
- [ ] Commit: `Add accessibility actions and values`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 4: Host Export Hooks

- [ ] Add host export records for desktop accessibility services and plugin editor accessibility availability.
- [ ] Add tests that layout geometry updates accessibility bounds.
- [ ] Run: `rtk cargo test -p hawk2ui-a11y host_export`.
- [ ] Commit: `Add accessibility host export hooks`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 5: Plugin Safety

- [ ] Add plugin accessibility guard records that prohibit audio-thread work and unstable host calls.
- [ ] Add tests for safe plugin editor accessibility updates.
- [ ] Run: `rtk cargo test -p hawk2ui-a11y plugin_accessibility_safety`.
- [ ] Commit: `Add plugin accessibility safety checks`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

## Verification

- [ ] Run: `rtk cargo fmt --all -- --check`.
- [ ] Run: `rtk cargo test -p hawk2ui-a11y`.
- [ ] Run: `rtk cargo test --workspace a11y`.
