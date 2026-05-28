# Hawk2UI Packaging

Packaging APIs model the records needed to turn a validated manifest, compiled source, compiled style, assets, and target metadata into sealed artifacts or platform bundles.

## Artifact API Records

- `ArtifactId`: artifact identifier.
- `ArtifactHash`: artifact content hash.
- `ArtifactSchemaVersion`: schema version used by sealed artifacts.
- `ArtifactVersionError`: runtime/artifact schema mismatch error.
- `ArtifactCapability`: packaged capability declaration.
- `ArtifactManifestSnapshot`: manifest snapshot embedded in the package.
- `CompiledAssetKind`: compiled asset kind.
- `CompiledAssetRecord`: compiled asset metadata.
- `CompiledStyleRecord`: compiled style metadata.
- `CompiledScriptRecord`: compiled script metadata.
- `TargetKind`: package target kind.
- `TargetMetadata`: package target metadata.

The packaging API belongs to the `Artifact` module. It also consumes `Diagnostic` records for validation and verification output.

## Compatibility Targets

The OS compatibility matrix currently names:

- `windows-desktop`
- `macos-desktop`
- `linux-wayland-desktop`
- `linux-x11-plugin`

## Package Outputs

The package compatibility matrix currently names these outputs and verification commands:

- `desktop-linux`: `rtk cargo test -p hawk2ui-build package_desktop_linux`
- `desktop-windows`: `rtk cargo test -p hawk2ui-build package_desktop_windows`
- `desktop-macos`: `rtk cargo test -p hawk2ui-build package_desktop_macos`
- `plugin-clap`: `rtk cargo test -p hawk2ui-plugin-adapters plugin_adapters_generate_all_supported_package_targets`
- `plugin-vst3`: tracked package metadata only; not a release-gated output
- `plugin-au`: `rtk cargo test -p hawk2ui-plugin-adapters plugin_adapters_generate_all_supported_package_targets`
- `sealed-artifact`: `rtk cargo test -p hawk2ui-build sealed_artifact`
- `debug-package`: `rtk cargo test -p hawk2ui-build debug_package`
- `release-package`: `rtk cargo test -p hawk2ui-build release_package`

## Packaging Workflow

For desktop applications:

```bash
hawk2ui validate
hawk2ui build-release
hawk2ui verify-artifact
```

For plugin editors:

```bash
hawk2ui validate
hawk2ui package-plugin
hawk2ui verify-artifact
```

Use `hawk2ui diagnostics` whenever validation or verification returns a non-success exit code.
