# Spec 0003: Rendering

## Status

Final baseline.

## Purpose

This spec defines Hawk2UI's rendering architecture for desktop applications and embeddable plugin editors. The renderer must support premium, brand-heavy, high-DPI interfaces with dynamic creative surfaces while remaining independent from any single host backend or JavaScript framework.

## Scene Model

Hawk2UI uses a retained scene graph as the source of frame rendering.

The scene graph owns:

- node identity,
- parent and child hierarchy,
- layout attachment,
- z-order,
- clipping,
- transforms,
- opacity,
- effects,
- hit-test geometry,
- invalidation state,
- accessibility geometry references,
- export into backend draw commands.

The renderer consumes prepared scene data. It does not parse raw app source.

## Rendering Layers

A frame may combine:

- solid fills,
- strokes,
- rounded rectangles,
- arbitrary paths,
- gradients,
- shadows,
- glows,
- opacity groups,
- clips,
- transforms,
- text runs,
- image layers,
- vector asset layers,
- control visuals,
- custom draw surfaces,
- cached static layers,
- live dynamic layers.

Layer composition must be deterministic for visual regression testing.

## Skia Boundary

Skia is wrapped by Hawk2UI renderer traits. The renderer abstraction exposes:

- surface creation and teardown,
- resize and DPI changes,
- frame begin and end,
- clear, fill, stroke, and path commands,
- text draw commands,
- image draw commands,
- clip and transform stacks,
- layer and effect commands,
- cache handles,
- dirty-region submission,
- backend capability reporting,
- diagnostics capture.

The first implementation is Skia CPU raster. GPU support follows after CPU rendering, host surface lifecycle, and visual regression flow are stable.

## Text Rendering

The text system supports:

- font discovery,
- app-provided fonts,
- fallback fonts,
- shaping,
- line breaking,
- bidirectional text,
- glyph cache integration,
- text measurement for layout,
- high-DPI output.

## Assets

Images, vector assets, fonts, and design tokens are compiled into explicit asset manifests before runtime.

The asset pipeline records:

- stable asset IDs,
- source path metadata,
- content hashes,
- decoded dimensions where known,
- sanitization status,
- backend capability requirements,
- packaging metadata,
- cache invalidation metadata.

SVG and other vector inputs must pass through sanitization or compilation before runtime use.

## Controls And Custom Draw Surfaces

Controls are behavior-first. Visuals for controls come from styles, themes, assets, vector primitives, scene nodes, and custom draw hooks.

The rendering model must support:

- knobs,
- sliders,
- meters,
- switches,
- envelopes,
- scopes,
- analyzers,
- EQ curves,
- modulation displays,
- timelines,
- pads,
- keyboards,
- graph editors,
- inspector panels.

Custom draw surfaces integrate with hit testing, layout, invalidation, frame scheduling, and renderer capability reporting.

## Animation And Scheduling

Frame scheduling supports:

- explicit repaint requests,
- animation ticks,
- host-driven resize and DPI repaint,
- reduced-rate meter and analyzer updates,
- frame-rate caps,
- headless deterministic rendering,
- plugin-safe scheduling where the audio thread never blocks on the UI.

Animation state lives above the renderer. The renderer receives scene state for a frame and renders it deterministically.

## Paint And Export Boundary

Paint lists are generated from the retained scene for:

- visual regression tests,
- diagnostics,
- headless rendering,
- debugging scene output,
- fallback rendering paths,
- backend parity tests.

Paint lists are not the primary state model.

## Acceptance Criteria

- Public rendering APIs do not expose `skia-safe` types.
- A retained scene graph is the source of frame rendering.
- Paint lists can be generated for tests and diagnostics.
- Skia CPU raster can render styled scene output.
- Text measurement participates in layout.
- Image and vector assets render through compiled asset records.
- Custom draw surfaces support graph and plugin visual regions.
- Rendering works for owned desktop windows and embedded plugin surfaces.
- Rendering failure handling is safe for plugin hosts.
