# Spec 0003: Rendering

## Status

Final baseline.

## Purpose

This spec defines rendering requirements for Hawk2UI desktop surfaces and embedded plugin editor surfaces.

## Scene Requirements

Rendering must use a retained scene model that contains:

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
- backend draw export.

The renderer consumes prepared scene data. It does not parse raw app source.

## Layer Requirements

A rendered frame must support:

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

## Renderer Boundary Requirements

The rendering API must hide backend-specific types from public author-facing APIs.

The internal renderer boundary must expose:

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

## Text Rendering Requirements

The text system must support:

- font discovery,
- app-provided fonts,
- fallback fonts,
- shaping,
- line breaking,
- bidirectional text,
- glyph cache integration,
- text measurement for layout,
- high-DPI output.

## Asset Rendering Requirements

Images, vector assets, fonts, and design tokens must render through compiled asset records.

Asset records must include:

- stable asset IDs,
- source path metadata,
- content hashes,
- decoded dimensions where known,
- sanitization status,
- backend capability requirements,
- packaging metadata,
- cache invalidation metadata.

## Custom Draw Surface Requirements

Custom draw surfaces must support:

- knobs,
- sliders,
- meters,
- scopes,
- analyzers,
- EQ curves,
- modulation displays,
- timelines,
- graph editors,
- inspector panels.

Custom draw surfaces must integrate with hit testing, layout, invalidation, frame scheduling, and renderer capability reporting.

## Acceptance Criteria

- A retained scene model is the source of frame rendering.
- Paint/export records can be generated for tests and diagnostics.
- Text measurement participates in layout.
- Image and vector assets render through compiled asset records.
- Custom draw surfaces support graph and plugin visual regions.
- Rendering works for owned desktop windows and embedded plugin surfaces.
