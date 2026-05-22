# Build And Artifacts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement manifest validation, source/style/script/asset compilation orchestration, sealed artifact packaging, native target packaging hooks, and verification reports.

**Architecture:** The build pipeline produces versioned, hash-addressed artifacts that runtime code consumes without parsing raw source. Each build phase emits structured diagnostics with source locations on supported targets.

**Tech Stack:** Rust, serde, toml, blake3, camino, cargo test, insta snapshots.

---

## File Structure

- Create: `crates/hawk2ui-build/src/lib.rs` build API exports.
- Create: `crates/hawk2ui-build/src/manifest.rs` manifest schema and validation.
- Create: `crates/hawk2ui-build/src/pipeline.rs` build pipeline orchestration.
- Create: `crates/hawk2ui-build/src/artifact.rs` sealed artifact records.
- Create: `crates/hawk2ui-build/src/assets.rs` asset compilation records.
- Create: `crates/hawk2ui-build/src/report.rs` verification reports.
- Create: `crates/hawk2ui-build/tests/build_pipeline.rs` pipeline tests.

## Tasks

### Task 1: Manifest Schema

- [ ] Define manifest records for app identity, plugin identity, package metadata, capabilities, assets, entrypoints, editor metadata, plugin parameters, presets, and targets.
- [ ] Add validation tests for missing identity, duplicate target, invalid capability, and invalid plugin metadata.
- [ ] Run: `rtk cargo test -p hawk2ui-build manifest_validation`.
- [ ] Commit: `Add build manifest validation`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 2: Pipeline Phase Records

- [ ] Add phase records for source discovery, manifest validation, asset discovery, source validation, style compilation, script compilation, asset compilation, artifact generation, packaging, and verification.
- [ ] Add tests for phase ordering and diagnostic propagation.
- [ ] Run: `rtk cargo test -p hawk2ui-build pipeline_phases`.
- [ ] Commit: `Add build pipeline phases`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 3: Sealed Artifact Records

- [ ] Define artifact records for schema version, manifest snapshot, compiled scripts, compiled styles, asset manifest, compiled assets, capabilities, hashes, build metadata, and target metadata.
- [ ] Add tests for stable hashing and version compatibility checks.
- [ ] Run: `rtk cargo test -p hawk2ui-build sealed_artifact`.
- [ ] Commit: `Add sealed artifact records`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 4: Asset Compilation

- [ ] Add image, vector, font, and design token asset compilation records with source paths, hashes, dimensions, sanitization status, and package metadata.
- [ ] Add tests for missing asset, unsafe asset, and cache invalidation metadata.
- [ ] Run: `rtk cargo test -p hawk2ui-build asset_compilation`.
- [ ] Commit: `Add asset compilation records`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 5: Verification Reports

- [ ] Implement verification report output for invalid manifests, unsupported style, unsupported script, unsafe assets, missing assets, undeclared capabilities, and target incompatibility.
- [ ] Add snapshot tests for diagnostics including file path, span, rule, severity, and message.
- [ ] Run: `rtk cargo test -p hawk2ui-build verification_report`.
- [ ] Commit: `Add build verification reports`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

## Verification

- [ ] Run: `rtk cargo fmt --all -- --check`.
- [ ] Run: `rtk cargo test -p hawk2ui-build`.
- [ ] Run: `rtk cargo test --workspace build`.
