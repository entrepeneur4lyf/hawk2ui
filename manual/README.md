# Hawk2UI Manual

Hawk2UI is a native windowing and rendering framework for production desktop applications and embeddable plugin editors. This manual is for application authors, plugin authors, and framework integration authors. It is grounded in implemented code: CLI command definitions, manifest parser behavior, public API inventory, compatibility matrices, examples, and conformance tests.

## Desktop Applications

Desktop applications declare a `desktop` target in `manifest.hawk.toml` and are modeled through the native host adapter API records. Start with [Desktop Apps](desktop-apps.md) and the fixtures `examples/desktop-basic/manifest.hawk.toml` and `examples/desktop-dashboard/manifest.hawk.toml`.

## Plugin Editors

Plugin editors declare a `plugin` target and include host-facing plugin metadata. Start with [Plugin Editors](plugin-editors.md) and the fixtures `examples/plugin-basic/manifest.hawk.toml`, `examples/plugin-synth-editor/manifest.hawk.toml`, and `examples/plugin-meter-analyzer/manifest.hawk.toml`.

## Style System

The style system accepts the production property registry and rejects unsupported selector forms with stable diagnostics. See [Style Reference](style-reference.md).

## Runtime APIs

Runtime APIs connect framework adapters, host services, surfaces, diagnostics, scheduling, and plugin state through public records. See [Runtime APIs](runtime-apis.md).

## Packaging

Packaging APIs model sealed artifacts, target metadata, compatibility checks, and package outputs from validated manifests and compiled assets. See [Packaging](packaging.md).

## Troubleshooting

Use diagnostics and exit codes first. See [Troubleshooting](troubleshooting.md).

## Full Index

See [SUMMARY.md](SUMMARY.md) for the complete manual index.
