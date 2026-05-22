# Spec 0019: Release Readiness

## Status

Final baseline.

## Purpose

This spec defines release readiness requirements for Hawk2UI versions, packages, artifacts, verification evidence, and release gates.

## Release Gate Requirements

A release must require passing evidence for:

- formatting,
- linting,
- unit tests,
- integration tests,
- visual regression tests,
- security rejection tests,
- compatibility validation,
- performance budgets,
- dependency hygiene,
- artifact verification,
- smoke applications,
- documentation checks,
- package verification.

## Version Requirements

Release metadata must track:

- crate versions,
- artifact schema versions,
- manifest schema versions,
- package versions,
- compatibility matrix version,
- manual version,
- migration notes.

Version mismatches must fail release validation.

## Package Requirements

Release packaging must verify:

- sealed artifacts,
- desktop bundles,
- plugin bundles,
- package metadata,
- target metadata,
- asset hashes,
- script hashes,
- style hashes,
- signing status where applicable,
- notarization status where applicable,
- verification report inclusion.

## Release Evidence Requirements

Release evidence must include:

- command name,
- command result,
- timestamp,
- target platform,
- artifact identifier,
- package identifier,
- verification report path,
- failing rule when blocked.

## Acceptance Criteria

- Release readiness is executable and auditable.
- Packages cannot ship without verification reports.
- Version mismatches block release.
- Release evidence identifies every required gate and result.
