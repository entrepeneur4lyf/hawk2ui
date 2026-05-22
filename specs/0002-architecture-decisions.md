# Spec 0002: Architecture Decisions

## Status

Final baseline.

## External Projects

Deft is prior art only. Hawk2UI will not adopt or fork Deft as its foundation because Deft's app-owned window architecture conflicts with Hawk2UI's plugin embedding requirement.

Parcel is build-pipeline prior art only. Hawk2UI will not adopt Parcel as its build system. Hawk2UI may reuse concepts such as asset graphs, transformer stages, source maps, and Lightning CSS.

nice-plug is required plugin prior art. Hawk2UI will model plugin parameters, state, editor lifecycle, and CLAP/VST3 lessons from nice-plug where they fit Hawk2UI's authoring and rendering model.

The archived prototype repository is local prior art and migration material. Concepts and tests may be ported selectively. The prototype architecture is not the production architecture.

## Host Backends

Desktop host work starts with `winit`.

Plugin editor work starts with Baseview.

The desktop and plugin host paths share rendering code but do not share lifecycle ownership. Desktop owns native top-level windows. Plugin editors attach to host-owned parent surfaces and must never assume process-level window ownership.

## Plugin Format Order

Hawk2UI implements CLAP first, VST3 second, AU later, and LV2 only if the Linux plugin strategy requires it.

CLAP drives the first concrete plugin/editor implementation. VST3 follows because market support requires it. AU depends on macOS host adapter and packaging work.

## Runtime

Boa is the first embedded JavaScript runtime implementation target.

If Boa blocks module behavior, performance, framework compatibility, or host integration, Hawk2UI will move to Deno/V8. JavaScriptCore and QuickJS are comparison paths, not the default direction.

Bun is an external development/build tooling candidate. Bun is not the embedded runtime.

## Style And Layout

Lightning CSS is the style parser and transformer boundary.

Taffy stable is the first layout engine.

Hawk2UI owns typed style data and layout abstraction layers. External crate APIs do not define public Hawk2UI APIs.

## Rendering

Skia CPU raster is the first renderer implementation path.

Hawk2UI owns the renderer abstraction. Public APIs do not expose `skia-safe` types.

The rendering architecture is retained-scene-first with paint-list/export boundaries for tests, diagnostics, and backend parity.

## Text

The first text stack is Parley, fontdb, swash, and Skia.

Text measurement participates in layout. Text rendering is not a late paint-only step.

## Manifest And Artifacts

Human-authored manifests use TOML.

Validation schemas and generated machine artifacts use JSON.

Sealed artifacts are versioned containers containing compiled source records, capabilities, asset manifests, hashes, schema version, and package metadata.

## Realtime Data

Audio-thread-to-UI visual data uses `rtrb` first.

General-purpose channels are not used on the audio thread.

## Accessibility

AccessKit is the desktop accessibility direction.

Plugin accessibility is handled through format-specific and host-specific compatibility work.

## Dependency Policy

Crates that are marked alpha, experimental, or unstable may be used when they solve the product problem and are wrapped by Hawk2UI-owned boundaries. API churn is handled when it happens. Hawk2UI will not over-architect for hypothetical dependency failures.
