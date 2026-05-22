# Task List 0000: Production Gates

## Purpose

Track implementation work that establishes production gates before broad feature development.

## Sources

- Spec: `specs/0015-api-contracts.md`
- Spec: `specs/0016-compatibility-matrix.md`
- Spec: `specs/0017-performance-and-stability.md`
- Spec: `specs/0019-release-readiness.md`
- Plan: `docs/superpowers/plans/2026-05-22-0000-workspace-and-ci-gates-plan.md`
- Plan: `docs/superpowers/plans/2026-05-22-0015-api-contracts-plan.md`
- Plan: `docs/superpowers/plans/2026-05-22-0016-compatibility-matrix-plan.md`
- Plan: `docs/superpowers/plans/2026-05-22-0017-production-performance-gates-plan.md`
- Plan: `docs/superpowers/plans/2026-05-22-0020-release-readiness-plan.md`

## Tasks

### 0000.1 Workspace And Local Gates

- [ ] Deliverable: Rust workspace, shared package metadata, local fast/full check scripts, and workspace maintenance command.
- [ ] Dependencies: none.
- [ ] Verify: `rtk bash scripts/check-fast.sh`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0000.2 CI Gates

- [ ] Deliverable: CI jobs for format, lint, tests, dependency policy, docs checks, examples, and smoke fixtures across supported platforms.
- [ ] Dependencies: `0000.1`.
- [ ] Verify: `rtk bash scripts/check.sh`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0000.3 Public API Contract Gate

- [ ] Deliverable: public API crate, diagnostic contracts, artifact contracts, surface/runtime contracts, plugin contracts, and compile fixtures.
- [ ] Dependencies: `0000.1`.
- [ ] Verify: `rtk cargo test -p hawk2ui-api`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0000.4 Compatibility Matrix Gate

- [ ] Deliverable: machine-readable OS, graphics, plugin host, and package compatibility matrices with unsupported-target diagnostics.
- [ ] Dependencies: `0000.3`.
- [ ] Verify: `rtk cargo test -p hawk2ui-compat`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0000.5 Performance And Stability Gate

- [ ] Deliverable: performance budgets, benchmark harnesses, runtime stability fixtures, and realtime safety guards.
- [ ] Dependencies: `0000.3`.
- [ ] Verify: `rtk cargo test --workspace performance`.
- [ ] Verify: `rtk cargo bench --workspace`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0000.6 Release Readiness Gate

- [ ] Deliverable: release criteria, version policy, package verification, changelog gate, and full release check command.
- [ ] Dependencies: `0000.2`, `0000.4`, `0000.5`.
- [ ] Verify: `rtk bash scripts/release-check.sh`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.
