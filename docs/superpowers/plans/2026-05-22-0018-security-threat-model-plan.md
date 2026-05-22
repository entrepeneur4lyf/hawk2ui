# Security Threat Model Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Define the threat model and required security rejection tests for source validation, artifacts, runtime authority, platform APIs, assets, secrets, plugin hosts, and package trust.

**Architecture:** Threats are tracked as machine-readable cases that map to validation rules, runtime denials, tests, diagnostics, and release gates. Every capability boundary must have at least one deny test.

**Tech Stack:** Rust, serde, TOML threat registry, insta snapshots, cargo test.

---

## File Structure

- Create: `security/threat-model.toml` threat registry.
- Create: `security/rejection-cases.toml` required denial cases.
- Create: `crates/hawk2ui-security-model/src/lib.rs` threat model validator.
- Create: `crates/hawk2ui-security-model/tests/threat_model.rs` threat model tests.
- Create: `fixtures/security/` malicious and denied input fixtures.
- Create: `docs/security/threat-model.md` human-readable threat model.

## Tasks

### Task 1: Threat Registry

- [ ] Add threat records for malicious source, malformed artifacts, unsafe assets, hostile package input, malicious plugin host data, untrusted user data, secret exposure, and denied platform authority.
- [ ] Add tests for unique threat IDs, severity, affected domain, mitigation, and required test link.
- [ ] Run: `rtk cargo test -p hawk2ui-security-model threat_registry`.
- [ ] Commit: `Add security threat registry`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 2: Capability Boundary Cases

- [ ] Add rejection cases for filesystem, network, clipboard, secrets, database, package targets, plugin metadata, host APIs, and runtime bindings.
- [ ] Add tests requiring every capability key to have an allow case and a deny case.
- [ ] Run: `rtk cargo test -p hawk2ui-security-model capability_rejections`.
- [ ] Commit: `Add capability rejection cases`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 3: Source And Asset Attack Fixtures

- [ ] Add fixtures for unsupported style syntax, unsupported script syntax, unsafe vector content, oversized assets, hash mismatch, missing assets, and malformed manifests.
- [ ] Add tests that each fixture maps to a specific diagnostic rule.
- [ ] Run: `rtk cargo test -p hawk2ui-security-model source_asset_fixtures`.
- [ ] Commit: `Add source and asset attack fixtures`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 4: Runtime Authority Tests

- [ ] Add tests for string-to-code execution denial, undeclared host API denial, direct filesystem denial, direct network denial, process spawn denial, and native module loading denial.
- [ ] Add snapshot checks that diagnostics do not leak secrets or source payloads beyond safe excerpts.
- [ ] Run: `rtk cargo test -p hawk2ui-security-model runtime_authority`.
- [ ] Commit: `Add runtime authority threat tests`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 5: Package Trust Checks

- [ ] Add checks for artifact schema version, manifest snapshot hash, compiled asset hashes, compiled script hashes, target metadata, package signature status, and verification report status.
- [ ] Add tests for tampered artifact and missing verification report.
- [ ] Run: `rtk cargo test -p hawk2ui-security-model package_trust`.
- [ ] Commit: `Add package trust checks`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

## Verification

- [ ] Run: `rtk cargo test -p hawk2ui-security-model`.
- [ ] Run: `rtk cargo test --workspace security`.
