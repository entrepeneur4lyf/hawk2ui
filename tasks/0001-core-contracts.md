# Task List 0001: Core Contracts

## Purpose

Track implementation work for the product model, schema records, diagnostics, artifacts, and conformance shell.

## Sources

- Spec: `specs/0001-product-and-scope.md`
- Spec: `specs/0008-build-artifacts.md`
- Spec: `specs/0014-testing.md`
- Spec: `specs/0015-api-contracts.md`
- Plan: `docs/superpowers/plans/2026-05-22-0001-product-and-scope-plan.md`
- Plan: `docs/superpowers/plans/2026-05-22-0008-build-artifacts-plan.md`
- Plan: `docs/superpowers/plans/2026-05-22-0014-testing-plan.md`
- Plan: `docs/superpowers/plans/2026-05-22-0015-api-contracts-plan.md`

## Tasks

### 0001.1 Product Model Records

- [ ] Deliverable: typed records for host targets, surface kinds, product capabilities, and product conformance model.
- [ ] Dependencies: `0000.1`, `0000.3`.
- [ ] Verify: `rtk cargo test -p hawk2ui-schema product`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0001.2 Manifest And Artifact Contracts

- [ ] Deliverable: manifest schema, sealed artifact schema, artifact hashing, schema compatibility checks, and artifact diagnostics.
- [ ] Dependencies: `0001.1`.
- [ ] Verify: `rtk cargo test -p hawk2ui-build manifest_validation sealed_artifact`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0001.3 Build Pipeline Records

- [ ] Deliverable: source discovery, validation, style/script/asset compilation orchestration, package target records, and verification report records.
- [ ] Dependencies: `0001.2`.
- [ ] Verify: `rtk cargo test -p hawk2ui-build build_pipeline verification_report`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0001.4 Shared Testkit

- [ ] Deliverable: fixture helpers, command runners, diagnostics assertions, artifact assertions, visual helpers, security helpers, and benchmark helpers.
- [ ] Dependencies: `0001.2`.
- [ ] Verify: `rtk cargo test -p hawk2ui-testkit`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0001.5 Product Conformance Tests

- [ ] Deliverable: conformance tests proving desktop and plugin product surfaces, manuals, examples, and artifact records exist.
- [ ] Dependencies: `0001.1`, `0001.4`.
- [ ] Verify: `rtk cargo test -p hawk2ui-conformance`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.
