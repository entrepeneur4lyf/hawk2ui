# Developer Experience Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement CLI workflows, structured diagnostics, development loop behavior, documentation shell, examples, and meaningful exit codes.

**Architecture:** The CLI is a thin orchestration layer over build, runtime, host, and packaging crates. Diagnostics use one shared type so CLI output, dev overlay, verification reports, and tests match.

**Tech Stack:** Rust, clap, miette or codespan-reporting, notify, serde, cargo test, trycmd.

---

## File Structure

- Create: `crates/hawk2ui-cli/src/main.rs` CLI entrypoint.
- Create: `crates/hawk2ui-cli/src/commands.rs` command definitions.
- Create: `crates/hawk2ui-cli/src/diagnostics.rs` CLI diagnostic rendering.
- Create: `crates/hawk2ui-cli/src/dev_loop.rs` file watch and reload loop.
- Create: `crates/hawk2ui-cli/tests/cli_commands.rs` CLI behavior tests.
- Create: `manual/developer-guide.md` developer guide shell.
- Create: `manual/troubleshooting.md` troubleshooting shell.

## Tasks

### Task 1: CLI Command Surface

- [ ] Add commands for project creation, validation, development builds, production builds, artifact verification, desktop execution, plugin packaging, and diagnostics.
- [ ] Add tests for command help text and invalid command exit code.
- [ ] Run: `rtk cargo test -p hawk2ui-cli cli_commands`.
- [ ] Commit: `Add Hawk2UI CLI command surface`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 2: Diagnostics Type

- [ ] Define diagnostics with file path, source span, rule name, message, suggested fix, severity, related capability, and related target.
- [ ] Add rendering tests for warning, error, capability denial, and target incompatibility.
- [ ] Run: `rtk cargo test -p hawk2ui-cli diagnostics`.
- [ ] Commit: `Add shared CLI diagnostics`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 3: Validation And Build Commands

- [ ] Wire `validate`, `build-dev`, `build-release`, and `verify-artifact` commands to production build crate APIs.
- [ ] Add tests for success exit code, validation failure exit code, and verification failure exit code.
- [ ] Run: `rtk cargo test -p hawk2ui-cli build_commands`.
- [ ] Commit: `Wire validation and build commands`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 4: Development Loop

- [ ] Add file watching, incremental rebuild triggers, validation before runtime update, native surface reload event, state preservation flag, and visible error reporting record.
- [ ] Add tests with a recording watcher and recording runtime reload target.
- [ ] Run: `rtk cargo test -p hawk2ui-cli dev_loop`.
- [ ] Commit: `Add development loop orchestration`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 5: Documentation Shell

- [ ] Add manual pages for user manual, developer guide, style reference, plugin author guide, desktop app guide, troubleshooting, API reference, and examples index.
- [ ] Add tests that manual pages exist and contain required headings.
- [ ] Run: `rtk cargo test -p hawk2ui-cli manual_presence`.
- [ ] Commit: `Add developer documentation shell`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

## Verification

- [ ] Run: `rtk cargo fmt --all -- --check`.
- [ ] Run: `rtk cargo test -p hawk2ui-cli`.
- [ ] Run: `rtk cargo test --workspace cli`.
