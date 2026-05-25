# API Stability Policy

This policy governs the public `hawk2ui-api` crate, sealed artifact contracts,
generated code expectations, and downstream host/plugin integrations.

## Source Compatibility

Public Rust APIs remain source-compatible within a major crate version. Additive
changes are allowed when existing constructors, methods, enum variant meanings,
and serialized field semantics remain valid for downstream code.

Breaking source changes require the breaking-change process. Examples include
removing a public type, renaming a public field, changing a constructor
signature, changing enum variant meaning, narrowing accepted input ranges, or
changing a method return contract.

## Artifact Compatibility

Sealed artifact schemas follow semantic compatibility:

- Matching major versions are required.
- Runtime readers may accept artifact minor versions up to the runtime-supported
  minor version.
- Patch versions are bug-fix only and must not change reader requirements.
- New optional records must be ignored safely by older compatible readers.

Artifact hash values, manifest snapshots, capability declarations, compiled
asset records, compiled style records, compiled script records, and target
metadata are compatibility commitments once emitted by production tooling.

## Feature Flags

Feature-gated public APIs must be listed in the API inventory. Experimental
features may change while gated, but they must not silently become stable without
inventory updates, module documentation, tests, and release notes.

Stable APIs must not gain a required feature flag in a compatible release.

## Deprecation Windows

Deprecated stable APIs remain available for at least one minor release before
removal. Deprecation notices must include the replacement API and migration
notes. Runtime and artifact compatibility deprecations must include validation
diagnostics before enforcement changes are made.

## Breaking-Change Process

Breaking changes require:

- A written migration note.
- Updated API inventory status.
- Updated public module stability documentation.
- Updated downstream compile coverage.
- Full workspace test and clippy verification.
- Explicit release notes calling out the incompatible change.

## Public Module Stability

### `artifact`

Artifact records define sealed package identity, schema compatibility, hashes,
capabilities, compiled assets, compiled styles, compiled scripts, and target
metadata. Existing schema compatibility behavior is stable within a major crate
version.

### `diagnostic`

Diagnostic records define stable rule identifiers, severities, source spans,
related context, suggested fixes, redaction status, and CLI-ready formatting.
Existing diagnostic meanings and output structure are stable within a major crate
version.

### `inventory`

Inventory records define the public API baseline used by tests and release
reviews. Existing public entries must not be removed or downgraded without the
breaking-change process.

### `plugin`

Plugin records define parameter metadata, automation gestures, editor metadata,
state, presets, and realtime data channels. Serialized state and automation
semantics are stable within a major crate version.

### `runtime`

Runtime records define capabilities, host bindings, lifecycle hooks, jobs,
phases, directions, and statuses. Existing phase/status meanings are stable
within a major crate version.

### `surface`

Surface records define host surface kinds, metrics, input events, repaint
reasons, and frame scheduling. Coordinate units, focus behavior, and repaint
scheduling meanings are stable within a major crate version.
