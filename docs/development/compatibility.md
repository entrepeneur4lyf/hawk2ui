# Compatibility

Hawk2UI compatibility data is machine-readable and release-gated. Update the
matrix files in the same change as the code, fixture, or package behavior they
describe.

## Operating System Matrix

`compatibility/matrix.toml` declares supported desktop and plugin targets by
operating system family, OS version range, architecture, windowing path,
accessibility path, package output, release status, and CI coverage.

Supported release rows must have CI coverage.

## Graphics Matrix

`compatibility/graphics.toml` declares renderer backend coverage for CPU raster,
high-DPI, text shaping, image layers, vector layers, effects, and dirty-region
tracking. Every release-required rendering feature must map to at least one
supported backend.

## Plugin Host Matrix

`compatibility/hosts.toml` declares CLAP, VST3, AU, and standalone host coverage
for editor attachment, resize, DPI, keyboard focus, accessibility, state,
automation, and realtime visual data.

## Packaging Matrix

`compatibility/packages.toml` declares desktop bundles, plugin bundles, sealed
artifacts, debug packages, release packages, signing, notarization, installer
status, and verification commands.

## Unsupported Target Diagnostics

Unsupported target diagnostics must name the rejected target and list supported
targets so users can choose a valid release path.

## Local Verification

Run compatibility gates before changing supported targets:

```bash
rtk cargo test -p hawk2ui-compat
rtk cargo test --workspace compatibility
```
