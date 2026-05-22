# Platform APIs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement capability-scoped platform APIs for filesystem, network, clipboard, secrets, and database access with structured denied-access diagnostics.

**Architecture:** All platform APIs register through a capability table. Desktop and plugin contexts share schemas while plugin contexts expose only host-safe subsets.

**Tech Stack:** Rust, serde, schemars, camino, reqwest behind feature gates, arboard behind feature gates, cargo test.

---

## File Structure

- Create: `crates/hawk2ui-platform/src/lib.rs` platform API exports.
- Create: `crates/hawk2ui-platform/src/capability.rs` capability records.
- Create: `crates/hawk2ui-platform/src/filesystem.rs` scoped filesystem API.
- Create: `crates/hawk2ui-platform/src/network.rs` network API records.
- Create: `crates/hawk2ui-platform/src/clipboard.rs` clipboard API records.
- Create: `crates/hawk2ui-platform/src/secrets.rs` secret API records.
- Create: `crates/hawk2ui-platform/src/database.rs` database API records.
- Create: `crates/hawk2ui-platform/tests/platform_capabilities.rs` platform tests.

## Tasks

### Task 1: Capability Table

- [ ] Define manifest capability key, allowed operations, denied operations, input schema, output schema, error schema, runtime availability, desktop applicability, and plugin applicability.
- [ ] Add tests for denied missing capability and plugin-incompatible capability.
- [ ] Run: `rtk cargo test -p hawk2ui-platform capability_table`.
- [ ] Commit: `Add platform capability table`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 2: Filesystem API

- [ ] Add scoped filesystem records for project assets, app data, cache data, user-selected files, plugin preset storage, and forbidden paths.
- [ ] Add tests for path escaping, forbidden paths, and user-selected file grants.
- [ ] Run: `rtk cargo test -p hawk2ui-platform filesystem_scope`.
- [ ] Commit: `Add scoped filesystem API`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 3: Network API

- [ ] Add network records with manifest declarations, allowlists, structured errors, and diagnostics for denied access.
- [ ] Add tests for allowed host, denied host, malformed URL, and missing capability.
- [ ] Run: `rtk cargo test -p hawk2ui-platform network_capabilities`.
- [ ] Commit: `Add capability-scoped network API`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 4: Clipboard API

- [ ] Add clipboard records with manifest declarations and explicit supported data types.
- [ ] Add tests for text, denied image type if unsupported, missing capability, and plugin-context denial.
- [ ] Run: `rtk cargo test -p hawk2ui-platform clipboard_capabilities`.
- [ ] Commit: `Add clipboard capability API`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 5: Secrets And Database APIs

- [ ] Add secret records with manifest declaration and diagnostic redaction.
- [ ] Add database records for migrations, transactions, and safe storage paths.
- [ ] Add tests for redaction, missing secret declaration, migration ordering, and unsafe storage path denial.
- [ ] Run: `rtk cargo test -p hawk2ui-platform secrets_database`.
- [ ] Commit: `Add secrets and database APIs`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

## Verification

- [ ] Run: `rtk cargo fmt --all -- --check`.
- [ ] Run: `rtk cargo test -p hawk2ui-platform`.
- [ ] Run: `rtk cargo test --workspace platform`.
