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
- `linux-x11-desktop`
- `windows-plugin`
- `macos-plugin`
- `linux-wayland-plugin`
- `linux-x11-plugin`

Windows, macOS, Linux Wayland, and Linux X11 are required production release platforms. Release packaging is not considered complete until `desktop-linux-wayland`, `desktop-linux-x11`, `desktop-windows`, `desktop-macos`, CLAP/VST3/AU plugin bundles, plugin-host embedding on Windows, macOS, Linux Wayland, and Linux X11, signing/notarization where applicable, and host-loadable plugin bundles are verified with recorded release evidence.

## Package Outputs

The package compatibility matrix currently names these outputs and verification commands:

- `desktop-linux-wayland`: `rtk cargo test -p hawk2ui-build package_desktop_linux`
- `desktop-linux-x11`: `rtk cargo test -p hawk2ui-build package_desktop_linux`
- `desktop-windows`: `rtk cargo test -p hawk2ui-build package_desktop_windows`
- `desktop-macos`: `rtk cargo test -p hawk2ui-build package_desktop_macos`
- `plugin-clap`: `rtk cargo test -p hawk2ui-plugin-adapters package_clap`
- `plugin-vst3`: `rtk cargo test -p hawk2ui-plugin-adapters package_vst3`
- `plugin-au`: `rtk cargo test -p hawk2ui-plugin-adapters package_au`
- `react-desktop-smoke`: `rtk cargo test -p hawk2ui-smoke react_desktop_basic_runs_sealed_deno_graph_through_winit_smoke`
- `react-plugin-smoke`: `rtk cargo test -p hawk2ui-smoke react_plugin_basic_runs_deno_ui_parameters_and_realtime_denial`
- `vue-desktop-smoke`: `rtk cargo test -p hawk2ui-smoke vue_desktop_basic_runs_sealed_deno_graph_through_winit_smoke`
- `vue-plugin-smoke`: `rtk cargo test -p hawk2ui-smoke vue_plugin_basic_runs_deno_ui_parameters_and_realtime_denial`
- `sealed-artifact`: `rtk cargo test -p hawk2ui-build sealed_artifact`
- `debug-package`: `rtk cargo test -p hawk2ui-build debug_package`
- `release-package`: `rtk cargo test -p hawk2ui-build release_package`

Every release-gated desktop and plugin bundle must verify these runtime bundle evidence records:

- `embedded-deno-runtime`: the package includes the embedded `hawk2ui-js-runtime` Deno/V8 runtime needed to execute React and Vue UI code.
- `rusty-v8-static-archive`: the package evidence records the verified upstream `rusty_v8` static archive used by the selected target/runtime build.
- `rusty-v8-source-binding`: the package evidence records the matching generated `src_binding_*` artifact for the same `rusty_v8` version, target, profile, and feature flavor.
- `sealed-js-module-graph`: the package includes the sealed JavaScript module graph manifest loaded by the runtime.
- `runtime-assets`: the package includes runtime-loadable asset records referenced by the sealed artifact.
- `package-manager-metadata`: the artifact records the selected package manager and lockfile hash used to produce the bundle.
- `lockfile-hash`: the artifact records the exact selected lockfile hash independently of package-manager identity.
- `dependency-graph-metadata`: the artifact records dependency graph metadata for the sealed JavaScript modules.
- `sealed-module-dependency-origin`: every sealed module records whether it came from workspace output, an installed package dependency, or generated build tooling.
- `sealed-module-source-map-hash`: every sealed module with a source map records the source-map hash used for runtime diagnostics.
- `sealed-module-entrypoint`: the sealed module graph records the exact graph entrypoint loaded by the runtime.
- `sealed-module-import-metadata`: static imports, dynamic imports, and chunk membership are recorded for the sealed module graph.
- `bundle-content-hash`: the artifact records the hash of each sealed JavaScript bundle/module payload.

React and Vue manifests declare the package-manager-produced bundle with `build.output`, for example `dist/main.js`. Release builds detect the example or project lockfile (`bun.lock`, `package-lock.json`, `pnpm-lock.yaml`, or `yarn.lock`), record the package-manager metadata, seal the declared bundle as a JS module graph, and make `hawk2ui verify-artifact` report `js-module-graphs`. If more than one supported lockfile is present, set `build.packageManager` to `bun`, `npm`, `pnpm`, or `yarn`.

Sealed JS module graph metadata records every module specifier, content hash, source-map hash, dependency origin, static import, dynamic import, chunk membership, package manager, lockfile hash, and graph entrypoint. `SealedJsDependencyOrigin` records whether a module came from workspace build output, an installed package dependency, or generated build tooling.

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

`hawk2ui package-plugin` materializes release-backed CLAP, VST3, and AU bundle layouts. CLAP, VST3, and AU plugin packaging is backed by the truce.audio plugin layer and verified through `hawk2ui package-plugin` evidence. The compatibility matrix records Windows, macOS, Linux Wayland, and Linux X11 plugin-host coverage; package-plugin compiles the generated `cdylib` crates, installs host-loadable shared libraries into the package binary slots, and refreshes package hashes before verification.

Use `hawk2ui diagnostics` whenever validation or verification returns a non-success exit code.
