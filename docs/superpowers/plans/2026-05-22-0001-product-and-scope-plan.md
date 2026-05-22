# Product And Scope Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create the project skeleton and product conformance harness that keeps desktop, plugin, authoring, rendering, assets, runtime, diagnostics, and manuals represented from the first commit.

**Architecture:** Start with a Rust workspace and shared schema crate so every domain can build against stable records. Add conformance tests that verify required product surfaces exist without implementing full behavior in this plan.

**Tech Stack:** Rust workspace, Cargo, serde, thiserror, insta, cargo test.

---

## File Structure

- Create: `Cargo.toml` workspace root.
- Create: `crates/hawk2ui-core/src/lib.rs` shared public surface exports.
- Create: `crates/hawk2ui-schema/src/lib.rs` shared typed records.
- Create: `crates/hawk2ui-conformance/tests/product_scope.rs` product-scope checks.
- Create: `examples/desktop-basic/manifest.hawk.toml` desktop example manifest.
- Create: `examples/plugin-basic/manifest.hawk.toml` plugin example manifest.
- Create: `manual/README.md` user-facing manual entrypoint.

## Tasks

### Task 1: Workspace Skeleton

- [ ] Create the root Cargo workspace with `hawk2ui-core`, `hawk2ui-schema`, and `hawk2ui-conformance` members.
- [ ] Add shared dependencies only where needed: `serde`, `thiserror`, `insta` for conformance snapshots.
- [ ] Run: `rtk cargo check --workspace`.
- [ ] Expected: all workspace crates compile with public modules and tests.
- [ ] Commit: `Create Hawk2UI workspace skeleton`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 2: Product Target Records

- [ ] Add `HostTarget`, `SurfaceKind`, `ProductCapability`, and `ProductModel` records in `crates/hawk2ui-schema/src/product.rs`.
- [ ] Export them from `crates/hawk2ui-schema/src/lib.rs`.
- [ ] Add tests that require both desktop and plugin surface kinds.
- [ ] Run: `rtk cargo test -p hawk2ui-schema product`.
- [ ] Commit: `Add product target schema`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 3: Public Core Facade

- [ ] Export schema types from `crates/hawk2ui-core/src/lib.rs` through a narrow facade.
- [ ] Add compile tests that a downstream crate can import `hawk2ui_core::ProductModel`.
- [ ] Run: `rtk cargo test -p hawk2ui-core`.
- [ ] Commit: `Expose core product facade`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 4: Example Manifests

- [ ] Add a complete desktop manifest with identity, source entry, capabilities, and target declaration.
- [ ] Add a complete plugin manifest with plugin identity, editor size, parameters array, and target declaration.
- [ ] Add conformance tests that load both files as raw TOML and verify required top-level sections exist.
- [ ] Run: `rtk cargo test -p hawk2ui-conformance product_scope`.
- [ ] Commit: `Add product target examples`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 5: Manual Entry Point

- [ ] Create `manual/README.md` with links for desktop apps, plugin editors, style, runtime APIs, packaging, and troubleshooting.
- [ ] Add a conformance test that verifies those manual headings exist.
- [ ] Run: `rtk cargo test -p hawk2ui-conformance manual_entrypoint`.
- [ ] Commit: `Add manual entrypoint conformance`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

## Verification

- [ ] Run: `rtk cargo fmt --all -- --check`.
- [ ] Run: `rtk cargo test --workspace`.
- [ ] Run: `rtk git diff --check`.
