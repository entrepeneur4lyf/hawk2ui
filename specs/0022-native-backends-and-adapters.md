# Spec 0022: Native Backends And Adapters

## Status

Final baseline.

## Purpose

This spec defines production backend and adapter requirements for rendering, text, assets, host windows, embedded plugin surfaces, script execution, and package formats.

## Renderer Backend Requirements

A production renderer backend must implement:

- surface creation,
- surface teardown,
- resize handling,
- DPI handling,
- frame begin and end,
- fills,
- strokes,
- paths,
- clips,
- transforms,
- text drawing,
- image drawing,
- vector drawing,
- effects,
- dirty-region submission,
- capability reporting,
- diagnostics.

## Text Backend Requirements

A production text backend must implement:

- font discovery,
- app font loading,
- fallback font selection,
- shaping,
- line breaking,
- bidirectional text,
- glyph cache integration,
- high-DPI measurement,
- layout invalidation keys.

## Asset Backend Requirements

A production asset backend must implement:

- image decoding,
- image metadata stripping,
- vector validation,
- vector lowering,
- font loading,
- hash verification,
- size limit enforcement,
- cache invalidation.

## Host Adapter Requirements

Production host adapters must implement:

- owned desktop windows,
- embedded plugin surfaces,
- resize events,
- DPI events,
- close requests,
- maximize and minimize events,
- focus routing,
- keyboard routing,
- pointer routing,
- repaint scheduling,
- safe teardown.

Production host adapters must cover Windows and macOS as first-class release targets. Windows adapters must support owned HWND windows and child HWND plugin attachment. macOS adapters must support owned native windows and embedded NSView/NSWindow plugin integration. Both platforms must produce release evidence for lifecycle, input, DPI, rendering, and teardown behavior.

## Script Backend Requirements

A production script backend must implement:

- JavaScript module loading,
- TypeScript-compiled module execution,
- promises,
- timers,
- host calls,
- structured data exchange,
- interruption,
- teardown,
- sandbox policy enforcement.

## Package Adapter Requirements

Production package adapters must implement:

- desktop app bundles,
- CLAP bundles,
- VST3 bundles,
- AU bundles,
- standalone wrappers,
- package metadata,
- target metadata,
- verification reports.

## Acceptance Criteria

- Production adapters satisfy the public API contracts.
- Recording adapters and production adapters share behavioral tests.
- Backend capability gaps produce structured diagnostics.
- Native surfaces and package outputs are covered by smoke applications.
- Windows and macOS adapters are verified before public release.
