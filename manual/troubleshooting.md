# Hawk2UI Troubleshooting

Start with the implemented diagnostics and command exit codes. The CLI command catalog exposes `hawk2ui diagnostics` specifically for structured diagnostic output.

## Command Failures

- Exit code `2`: usage error or unknown command. Run `hawk2ui diagnostics` and confirm the command name is one of `hawk2ui init`, `hawk2ui new`, `hawk2ui run`, `hawk2ui dev`, `hawk2ui validate`, `hawk2ui build-dev`, `hawk2ui build-release`, `hawk2ui verify-artifact`, `hawk2ui run-desktop`, `hawk2ui package-desktop`, `hawk2ui package-plugin`, `hawk2ui export-schemas`, `hawk2ui export-params`, `hawk2ui pin-ids`, `hawk2ui migrate-manifest`, `hawk2ui diagnostics`, or `hawk2ui explain`.
- Exit code `10`: validation failure. Run `hawk2ui validate` and inspect manifest, source, style, and capability diagnostics.
- Exit code `11`: artifact verification failure. Run `hawk2ui verify-artifact` and check schema version, hashes, target metadata, and sealed assets.
- Exit code `12`: runtime failure. Run `hawk2ui diagnostics` and inspect host surface, rendering backend, or capability diagnostics.

## Manifest Checks

- Confirm canonical `hawk.json` has `schemaVersion`, `package`, `app`, and `targets`.
- Confirm `package` has `id`, `name`, and `version`.
- Confirm `app` has `entry`.
- Confirm at least one `targets.desktop[]` or `targets.plugin[]` entry exists.
- Confirm plugin parameters are only used with plugin metadata and plugin targets.
- Confirm capability keys are non-empty and contain no spaces.

## Style Checks

Unsupported selector forms report these diagnostic rules: `selector.combinator.unsupported`, `selector.state.unsupported`, `selector.attribute.unsupported`, and `selector.list.unsupported`.

## Rendering Checks

If rendering differs by host, confirm the requested backend is supported. The supported backends in the compatibility matrix are `skia-cpu-raster`, `skia-gpu-wayland-desktop`, and `skia-gpu-wayland-baseview-plugin`; `skia-gpu-candidate` is recorded as a candidate for GPU paths that are not promoted for the active host. GPU-required paths must report a backend diagnostic rather than silently falling back when context creation or presentation is unavailable.
