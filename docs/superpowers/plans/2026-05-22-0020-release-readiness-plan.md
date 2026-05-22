# Release Readiness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Define release criteria, version policy, packaging verification, changelog process, artifact signing checks, and release-blocking gates.

**Architecture:** Release readiness is a checklist backed by executable validation commands. A release cannot proceed unless CI, compatibility, performance, security, smoke apps, manuals, and packaging checks pass.

**Tech Stack:** Rust, Cargo, cargo-release or custom xtask, git tags, changelog validation, package verification scripts.

---

## File Structure

- Create: `release/release-criteria.toml` release gates.
- Create: `release/checklist.md` human release checklist.
- Create: `release/package-targets.toml` package verification targets.
- Create: `scripts/release-check.sh` release gate runner.
- Create: `xtask/src/release.rs` release helper.
- Create: `CHANGELOG.md` changelog.
- Create: `docs/development/releasing.md` release manual.

## Tasks

### Task 1: Release Criteria

- [ ] Define criteria for API stability, artifact compatibility, CI pass, dependency health, compatibility matrix coverage, performance budgets, security gates, smoke apps, manuals, and packaging.
- [ ] Add validation that every criterion has owner, command, blocking level, and evidence path.
- [ ] Run: `rtk cargo test --workspace release_criteria`.
- [ ] Commit: `Add release criteria gates`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 2: Version Policy

- [ ] Define crate versioning, artifact schema versioning, package versioning, manual versioning, and compatibility notes.
- [ ] Add tests or scripts that reject mismatched crate and artifact schema metadata.
- [ ] Run: `rtk bash scripts/release-check.sh --version-only`.
- [ ] Commit: `Add release version policy`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 3: Package Verification

- [ ] Add package target verification for desktop bundles, plugin bundles, sealed artifacts, debug packages, release packages, signatures, and notarization status where applicable.
- [ ] Add package verification fixtures for pass and fail outcomes.
- [ ] Run: `rtk bash scripts/release-check.sh --packages-only`.
- [ ] Commit: `Add release package verification`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 4: Changelog Gate

- [ ] Add changelog format with added, changed, fixed, security, compatibility, migration, and known limitation sections.
- [ ] Add validation that a release candidate has changelog entries and linked verification evidence.
- [ ] Run: `rtk bash scripts/release-check.sh --changelog-only`.
- [ ] Commit: `Add changelog release gate`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 5: Full Release Check

- [ ] Wire full release command to local verification, CI-equivalent tests, dependency checks, compatibility checks, performance gates, security gates, smoke apps, docs checks, and package checks.
- [ ] Document exact command sequence for release candidates.
- [ ] Run: `rtk bash scripts/release-check.sh`.
- [ ] Commit: `Add full release readiness check`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

## Verification

- [ ] Run: `rtk bash scripts/release-check.sh`.
- [ ] Run: `rtk git diff --check`.
