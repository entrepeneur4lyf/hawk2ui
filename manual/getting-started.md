# Hawk2UI Getting Started

Hawk2UI models native desktop applications and embeddable plugin editors from a `manifest.hawk.toml` file. This page documents the implemented manifest surface, repository fixtures, and CLI command catalog.

## Project Shape

A Hawk2UI project starts with a manifest and an entry file:

```toml
[identity]
id = "com.example.app"
name = "Example App"
version = "0.1.0"

[source]
entry = "src/main.ts"

[capabilities]
keys = ["native-windowing", "sealed-artifacts"]

[[targets]]
kind = "desktop"
name = "linux-wayland"
```

The implemented manifest parser recognizes these sections:

- `[identity]` with `id`, `name`, and `version`.
- Optional `[package]` with `name` and `bundle_id`.
- `[source]` with `entry`, optional `style`, and optional `script`.
- Optional `[capabilities]` with `keys`.
- One or more `[[targets]]` with `kind` and `name`; implemented target kinds are `desktop` and `plugin`.
- Optional `[plugin]`, `[editor]`, `[[parameters]]`, `[[assets]]`, and `[[presets]]` for plugin and packaged artifact workflows.

## CLI Commands

The implemented CLI command catalog accepts these commands:

- `hawk2ui new` is the project creation command name.
- `hawk2ui validate` is the manifest/source/capability validation command name.
- `hawk2ui build-dev` is the development artifact command name.
- `hawk2ui build-release` is the release artifact command name.
- `hawk2ui verify-artifact` is the sealed artifact verification command name.
- `hawk2ui run-desktop` is the desktop native surface command name.
- `hawk2ui package-plugin` is the plugin package command name for CLAP, VST3, AU, and standalone targets.
- `hawk2ui diagnostics` is the structured diagnostics command name.

Implemented process exit codes are `0` for success, `2` for usage errors, `10` for validation failures, `11` for artifact verification failures, and `12` for runtime failures.

## First Checks

Run validation before any local launch or packaging flow:

```bash
hawk2ui validate
hawk2ui diagnostics
```

For desktop work, use the desktop flow:

```bash
hawk2ui build-dev
hawk2ui run-desktop
```

For release artifacts, use the release verification flow:

```bash
hawk2ui build-release
hawk2ui verify-artifact
```

For plugin editors, use the plugin packaging flow:

```bash
hawk2ui package-plugin
hawk2ui verify-artifact
```
