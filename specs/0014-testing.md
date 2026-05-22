# Spec 0014: Testing

## Status

Final baseline.

## Purpose

This spec defines test requirements for Hawk2UI implementation work.

## Test Categories

Hawk2UI requires:

- unit tests,
- integration tests,
- visual regression tests,
- host lifecycle tests,
- plugin lifecycle tests,
- security rejection tests,
- artifact verification tests,
- performance benchmarks,
- dependency hygiene checks.

## Unit Test Requirements

Unit tests must cover:

- parsers,
- validators,
- style properties,
- layout calculations,
- scene graph mutations,
- renderer command generation,
- runtime scheduling,
- manifest handling,
- plugin parameter behavior.

## Integration Test Requirements

Integration tests must cover:

- source to artifact,
- artifact to scene,
- scene to rendered output,
- host events to runtime updates,
- plugin parameter updates to UI updates,
- UI updates to automation gestures.

## Visual Regression Requirements

Visual tests must support deterministic fixtures for:

- text,
- shapes,
- gradients,
- image layers,
- vector assets,
- custom controls,
- graph surfaces,
- DPI scaling.

## Security Test Requirements

Security tests must verify rejection of:

- undeclared capabilities,
- unsupported source features,
- unsafe assets,
- invalid manifests,
- denied host APIs,
- secret leaks in diagnostics.

## Acceptance Criteria

- New implementation domains include tests in the same change set.
- Visual output can be compared deterministically for fixed fixtures.
- Security rejection tests exist for every capability boundary.
