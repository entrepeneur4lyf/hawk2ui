# Spec 0015: API Contracts

## Status

Final baseline.

## Purpose

This spec defines public API contract requirements for Hawk2UI crates, generated artifacts, host adapters, runtime bindings, plugin integration, diagnostics, and tooling.

## Public Contract Requirements

Public APIs must define:

- module ownership,
- exported types,
- trait boundaries,
- function signatures,
- error types,
- serialization behavior,
- feature flags,
- stability status,
- documentation links,
- test coverage links.

Internal backend types must not leak into author-facing APIs.

## Schema Requirements

Versioned schemas must exist for:

- project manifests,
- sealed artifacts,
- compiled UI records,
- compiled style records,
- compiled asset records,
- diagnostics,
- capability declarations,
- plugin metadata,
- plugin state envelopes,
- compatibility matrices,
- verification reports.

Schema versions must support compatibility checks and clear diagnostics for unsupported versions.

## Diagnostic Contract Requirements

All diagnostics must use a shared structure containing:

- severity,
- rule identifier,
- message,
- source path where available,
- source span where available,
- related target where applicable,
- related capability where applicable,
- suggested fix where practical,
- redaction status.

## Stability Requirements

Public contract changes must be classified as:

- additive,
- compatible behavior change,
- deprecation,
- breaking change,
- artifact schema change,
- runtime capability change.

Breaking changes must require version updates, migration notes, and compatibility diagnostics.

## Acceptance Criteria

- Public API surfaces are documented and testable.
- Runtime and build artifacts use versioned schemas.
- Diagnostics use one shared contract.
- Internal backend implementation types remain private.
- Contract compatibility is testable before release.
