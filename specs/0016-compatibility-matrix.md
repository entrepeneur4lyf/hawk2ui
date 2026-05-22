# Spec 0016: Compatibility Matrix

## Status

Final baseline.

## Purpose

This spec defines compatibility matrix requirements for supported operating systems, host surfaces, renderer capabilities, plugin formats, package targets, and accessibility paths.

## Matrix Requirements

Compatibility data must be machine-readable and must include:

- target name,
- target version range,
- CPU architecture,
- host surface type,
- renderer capabilities,
- input capabilities,
- accessibility capabilities,
- packaging support,
- test coverage status,
- release support status.

Unsupported combinations must produce structured diagnostics.

## Operating System Requirements

The matrix must cover:

- Windows desktop surfaces,
- Windows embedded plugin surfaces,
- macOS desktop surfaces,
- macOS embedded plugin surfaces,
- Linux desktop surfaces,
- Linux embedded plugin surfaces,
- Wayland behavior,
- X11/XCB behavior,
- XWayland behavior.

## Renderer Compatibility Requirements

Renderer compatibility must track support for:

- high-DPI output,
- text shaping,
- font loading,
- image layers,
- vector layers,
- gradients,
- effects,
- custom draw surfaces,
- dirty-region submission,
- headless render export.

## Plugin Host Requirements

Plugin compatibility must track:

- format support,
- editor attachment,
- resize behavior,
- DPI behavior,
- keyboard routing,
- focus routing,
- parameter automation,
- state save and restore,
- preset handling,
- realtime visual data handling.

## Acceptance Criteria

- Supported and unsupported target combinations are explicit.
- Build tooling can validate target compatibility.
- Release gates can query compatibility data.
- Unsupported targets fail with actionable diagnostics.
