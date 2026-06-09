# Hawk2UI Getting Started

Hawk2UI models native desktop applications and embeddable plugin editors from a project manifest. The canonical source manifest is `hawk.json`; legacy `manifest.hawk.toml` files are migration inputs only. See [Project Manifest](project-manifest.md) for the canonical JSON contract, repository fixtures, and CLI command catalog.

## Project Shape

A Hawk2UI project starts with a manifest and an entry file:

```json
{
  "$schema": "https://hawk2ui.dev/schemas/hawk.schema.json",
  "schemaVersion": 1,
  "package": {
    "id": "com.example.app",
    "name": "Example App",
    "version": "0.1.0",
    "bundleId": "com.example.app"
  },
  "app": {
    "entry": "src/main.ts",
    "framework": "native"
  },
  "targets": {
    "desktop": [
      {
        "name": "main",
        "platforms": ["windows", "macos", "linux-wayland", "linux-x11"]
      }
    ]
  },
  "permissions": {
    "capabilities": ["native-windowing", "sealed-artifacts"],
    "network": false,
    "filesystem": []
  }
}
```

The canonical manifest parser recognizes these JSON fields:

- `schemaVersion` with value `1`.
- `package` with `id`, `name`, `version`, and optional `bundleId`.
- `app` with `entry`, optional `framework`, optional `style`, and optional `script`.
- `targets.desktop[]` with `name`, `platforms`, and optional `window`.
- `targets.plugin[]` with `name`, `formats`, and optional `editor`.
- Optional `plugin` metadata with `id`, `name`, `parameters`, `meters`, and optional `state.version`.
- Optional `assets`, `presets`, `permissions`, and `build`.

Legacy `manifest.hawk.toml` is still accepted when `hawk.json` is absent so projects can run `hawk2ui migrate-manifest`.

## CLI Commands

The implemented CLI command catalog accepts these commands:

- `hawk2ui new` is the project creation command name.
- `hawk2ui init` is the project creation alias. It accepts `--template react-app`, `react-plugin`, `vue-app`, `vue-plugin`, or `native`, and `--package-manager bun`, `npm`, `pnpm`, or `yarn`.
- `hawk2ui run` builds and runs the default native target.
- `hawk2ui dev` is the React and Vue development loop command name.
- `hawk2ui validate` is the manifest/source/capability validation command name.
- `hawk2ui build-dev` is the development artifact command name.
- `hawk2ui build-release` is the release artifact command name.
- `hawk2ui verify-artifact` is the sealed artifact verification command name.
- `hawk2ui run-desktop` is the desktop native surface command name.
- `hawk2ui package-desktop` is the desktop package command name.
- `hawk2ui package-plugin` is the plugin package command name for release-backed CLAP, VST3, and AU targets.
- `hawk2ui export-schemas` writes the generated JSON Schema catalog.
- `hawk2ui export-params` writes the truce parameter source generated from the manifest.
- `hawk2ui pin-ids` pins stable numeric ids to unpinned manifest parameters.
- `hawk2ui migrate-manifest` converts legacy `manifest.hawk.toml` into canonical `hawk.json`.
- `hawk2ui diagnostics` is the structured diagnostics command name.
- `hawk2ui explain` explains project targets, capabilities, and next commands.

Implemented process exit codes are `0` for success, `2` for usage errors, `10` for validation failures, `11` for artifact verification failures, and `12` for runtime failures.

## First Checks

Run validation before any local launch or packaging flow:

```bash
hawk2ui validate
hawk2ui diagnostics
```

For desktop work, use the desktop flow:

```bash
hawk2ui init
hawk2ui dev
hawk2ui build-dev
hawk2ui run-desktop
```

React 19+ and Vue 3.5+ projects use package-manager output for the sealed Deno runtime. Set `app.framework` to `react` or `vue`, declare `build.output`, and keep the selected lockfile with the project so release artifacts can record package-manager metadata.

`hawk2ui-cli` remains the installable Rust CLI. React and Vue projects consume generated `@hawk2ui/react` and `@hawk2ui/vue` npm packages. Those packages are generated from the Hawk2UI repository during release; npm is the distribution channel, not a separate source of truth.

For release artifacts, use the release verification flow:

```bash
hawk2ui build-release
hawk2ui package-desktop
hawk2ui verify-artifact
```

For plugin editors, use the plugin packaging flow:

```bash
hawk2ui package-plugin
hawk2ui verify-artifact
```
