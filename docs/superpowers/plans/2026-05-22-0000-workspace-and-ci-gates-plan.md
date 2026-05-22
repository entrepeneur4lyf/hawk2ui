# Workspace And CI Gates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish the workspace, local verification commands, and CI gates required before production implementation proceeds.

**Architecture:** CI mirrors local commands exactly and treats formatting, linting, tests, security checks, docs checks, examples, and smoke fixtures as release gates. Workspace structure is created once and every later crate plugs into the same gates.

**Tech Stack:** Rust, Cargo, GitHub Actions or equivalent CI, cargo fmt, clippy, cargo test, cargo deny, cargo audit, cargo nextest on supported targets.

---

## File Structure

- Create: `Cargo.toml` workspace root.
- Create: `.github/workflows/ci.yml` CI workflow.
- Create: `deny.toml` dependency policy.
- Create: `scripts/check.sh` local full-check runner.
- Create: `scripts/check-fast.sh` local fast-check runner.
- Create: `xtask/src/main.rs` workspace maintenance commands.
- Create: `docs/development/verification.md` local verification manual.

## Tasks

### Task 1: Workspace Gate

- [ ] Create the root workspace and initial production crates required by the first domain plans.
- [ ] Add workspace-level package metadata, license, edition, lint policy, and dependency inheritance.
- [ ] Run: `rtk cargo metadata --format-version 1`.
- [ ] Expected: metadata lists all workspace members with no missing package fields.
- [ ] Commit: `Create production workspace gates`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 2: Local Check Scripts

- [ ] Add `scripts/check-fast.sh` for `cargo fmt --check`, `cargo check --workspace`, and targeted tests.
- [ ] Add `scripts/check.sh` for full formatting, clippy, tests, dependency checks, docs checks, and smoke fixtures.
- [ ] Run: `rtk bash scripts/check-fast.sh`.
- [ ] Expected: script exits `0` on the production workspace.
- [ ] Commit: `Add local verification scripts`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 3: CI Workflow

- [ ] Add CI jobs for Linux, Windows, and macOS.
- [ ] Add separate jobs for format, clippy, unit tests, integration tests, dependency policy, docs links, and examples.
- [ ] Ensure CI command names match local script sections.
- [ ] Run: `rtk bash scripts/check-fast.sh`.
- [ ] Commit: `Add workspace CI workflow`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 4: Dependency Policy

- [ ] Add `deny.toml` with license allowlist, duplicate dependency policy, advisory checks, and source allowlist.
- [ ] Add documentation for approving new dependencies.
- [ ] Run: `rtk cargo deny check`.
- [ ] Expected: no advisories or unapproved licenses.
- [ ] Commit: `Add dependency hygiene gate`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 5: Gate Documentation

- [ ] Document required commands before every commit and before every release.
- [ ] Document what failures block merge and what failures block release.
- [ ] Add a CI troubleshooting section with exact commands.
- [ ] Run: `rtk bash scripts/check-fast.sh`.
- [ ] Commit: `Document verification gates`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

## Verification

- [ ] Run: `rtk bash scripts/check.sh`.
- [ ] Run: `rtk git diff --check`.
