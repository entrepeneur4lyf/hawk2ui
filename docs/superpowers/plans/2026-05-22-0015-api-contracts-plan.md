# API Contracts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Define stable public API contracts for authoring, build, runtime, rendering, host surfaces, plugins, platform APIs, diagnostics, and artifacts.

**Architecture:** Public contracts live in small crates and are tested through downstream compile fixtures. Internal backend choices remain private behind traits and typed records.

**Tech Stack:** Rust, serde, thiserror, semver, trybuild, cargo test.

---

## File Structure

- Create: `crates/hawk2ui-api/src/lib.rs` public API root.
- Create: `crates/hawk2ui-api/src/diagnostic.rs` diagnostic contracts.
- Create: `crates/hawk2ui-api/src/artifact.rs` artifact contracts.
- Create: `crates/hawk2ui-api/src/surface.rs` host surface contracts.
- Create: `crates/hawk2ui-api/src/plugin.rs` plugin-facing contracts.
- Create: `crates/hawk2ui-api/src/runtime.rs` runtime contracts.
- Create: `tests/api-compile/` downstream compile fixtures.
- Create: `docs/development/api-stability.md` API stability policy.

## Tasks

### Task 1: Contract Inventory

- [ ] Create an API inventory listing all public types required by each domain plan.
- [ ] Mark each type as public, internal, feature-gated, or test-only.
- [ ] Add a test that public crates expose only documented root modules.
- [ ] Run: `rtk cargo test -p hawk2ui-api api_inventory`.
- [ ] Commit: `Add public API inventory`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 2: Diagnostic Contract

- [ ] Define `Diagnostic`, `DiagnosticSeverity`, `SourceSpan`, `RuleId`, `SuggestedFix`, and `RelatedContext`.
- [ ] Add serialization tests and snapshot tests for CLI-ready diagnostics.
- [ ] Run: `rtk cargo test -p hawk2ui-api diagnostic_contract`.
- [ ] Commit: `Add diagnostic API contract`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 3: Artifact Contract

- [ ] Define artifact versioning, manifest snapshot, hashes, capability declarations, compiled assets, compiled styles, compiled scripts, and target metadata records.
- [ ] Add semver compatibility tests for artifact schema versions.
- [ ] Run: `rtk cargo test -p hawk2ui-api artifact_contract`.
- [ ] Commit: `Add artifact API contract`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 4: Surface And Runtime Contracts

- [ ] Define host surface metrics, input events, repaint requests, frame scheduling, runtime jobs, lifecycle hooks, and host binding records.
- [ ] Add downstream compile tests that import and use these records without private modules.
- [ ] Run: `rtk cargo test -p hawk2ui-api surface_runtime_contracts`.
- [ ] Commit: `Add surface and runtime API contracts`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 5: Plugin Contract

- [ ] Define plugin parameter, automation, state, preset, editor, and realtime data records.
- [ ] Add downstream compile tests for generated editor and custom editor use cases.
- [ ] Run: `rtk cargo test -p hawk2ui-api plugin_contract`.
- [ ] Commit: `Add plugin API contract`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 6: Stability Policy

- [ ] Document source compatibility, artifact compatibility, feature flags, deprecation windows, and breaking-change process.
- [ ] Add a docs test that every public module has a stability section.
- [ ] Run: `rtk cargo test -p hawk2ui-api docs`.
- [ ] Commit: `Document API stability policy`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

## Verification

- [ ] Run: `rtk cargo test -p hawk2ui-api`.
- [ ] Run: `rtk cargo test --workspace api_contract`.
