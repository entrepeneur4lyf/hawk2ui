# Spec 0006: Layout

## Status

Final baseline.

## Purpose

This spec defines layout requirements for Hawk2UI scenes, components, text, and custom draw surfaces.

## Layout Tree Requirements

The layout system must support:

- layout nodes,
- parent and child hierarchy,
- fixed sizes,
- percentage sizes,
- min and max sizes,
- margins,
- padding,
- gaps,
- flex containers,
- scroll containers,
- absolute regions where required,
- custom measured nodes.

## Text Measurement Requirements

Text measurement must participate in layout.

The layout system must support:

- intrinsic text width,
- line wrapping,
- line height,
- truncation constraints,
- font-dependent measurement,
- layout invalidation when font or text changes.

## Plugin Editor Layout Requirements

Plugin editor layouts must support:

- fixed default editor sizes,
- host-driven resizing,
- constrained minimum and maximum sizes,
- dense parameter panels,
- graph and analyzer regions,
- generated parameter editor layouts.

## Scene Integration Requirements

Layout output must attach to retained scene nodes and provide geometry for:

- rendering,
- hit testing,
- accessibility mapping,
- scroll clipping,
- custom draw surfaces.

## Acceptance Criteria

- Layout output can drive scene geometry.
- Text measurement affects layout.
- Plugin editor constraints are represented.
- Custom draw regions can reserve layout space.
