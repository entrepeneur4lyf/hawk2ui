# Hawk2UI Manual

`Hawk2UI` is a native windowing and rendering framework for production desktop applications and embeddable plugin editors. This manual is the stable user-facing entrypoint for application authors, plugin authors, and integration maintainers.

## Desktop Applications

Desktop applications declare a `desktop` target in `manifest.hawk.toml` and run through a native host adapter. A desktop package must provide product identity, a source entrypoint, the capabilities it requires, and at least one target declaration.

Required desktop manifest sections:

- `[identity]`
- `[source]`
- `[capabilities]`
- `[[targets]]` with `kind = "desktop"`

## Plugin Editors

Plugin editors declare a `plugin` target and include editor metadata suitable for host embedding. Plugin packages must expose stable plugin identity, initial editor size, and parameter metadata that can be synchronized with the audio host.

Required plugin manifest sections:

- `[identity]`
- `[source]`
- `[capabilities]`
- `[[targets]]` with `kind = "plugin"`
- `[plugin]`
- `[editor]`
- `[[parameters]]`

## Style System

The style system accepts the supported `Hawk2UI` style subset and resolves it into typed style records before layout and rendering. Unsupported properties must fail with diagnostics instead of being silently ignored.

Production style inputs must be deterministic, validate selectors, and produce stable output for visual regression tests.

## Runtime APIs

Runtime APIs connect framework adapters, the scene model, host services, asset loading, security capabilities, and plugin state. Runtime access must be capability-gated and must not expose ambient authority to application code.

Runtime integrations are expected to use stable records from the core and schema crates rather than host-specific implementation details.

## Packaging

Packaging produces sealed artifacts from validated manifests, compiled source, compiled styles, resolved assets, and target metadata. A package is release-ready only when verification reports contain no release-blocking diagnostics.

Every package must preserve manifest identity, target declarations, schema version, and artifact hash metadata.

## Troubleshooting

Use diagnostics first. Every production-facing validation failure should include a stable rule, severity, and human-readable message.

Common checks:

- Confirm the manifest contains all required sections for its target.
- Confirm capability keys are non-empty and contain no spaces.
- Confirm plugin parameters are declared only when `[plugin]` metadata is present.
- Confirm sealed artifacts use a compatible major schema version.
