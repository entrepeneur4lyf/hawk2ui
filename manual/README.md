# Hawk2UI Manual

Hawk2UI is a native windowing and rendering framework for production desktop applications and embeddable plugin editors. This manual is for application authors, plugin authors, and framework integration authors. It is grounded in implemented code: CLI command definitions, manifest parser behavior, public API inventory, compatibility matrices, examples, and conformance tests.

## Project Manifest

Hawk2UI projects are manifest-driven. The canonical source manifest is `hawk.json`, which declares package identity, app entrypoints, desktop targets, plugin targets, assets, permissions, and package-manager build output. Legacy `manifest.hawk.toml` files are accepted as migration inputs, not as the long-term project format. Start with [Project Manifest](project-manifest.md).

## Desktop Applications

Desktop applications declare a `desktop` target in the project manifest and are modeled through the native host adapter API records. Start with [Desktop Apps](desktop-apps.md), [Project Manifest](project-manifest.md), and the fixtures `examples/desktop-basic/hawk.json` and `examples/desktop-dashboard/hawk.json`.

## Plugin Editors

Plugin editors declare a `plugin` target and include host-facing plugin metadata. Start with [Plugin Editors](plugin-editors.md), [Project Manifest](project-manifest.md), and the fixtures `examples/plugin-basic/hawk.json`, `examples/plugin-synth-editor/hawk.json`, and `examples/plugin-meter-analyzer/hawk.json`.

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
