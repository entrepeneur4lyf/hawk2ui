# Spec 0005: Style

## Status

Final baseline.

## Purpose

This spec defines styling requirements for Hawk2UI UI source, compiled style records, and runtime style data.

## Style Source Requirements

Style source must support a documented native subset for:

- layout properties,
- typography properties,
- color properties,
- borders,
- radii,
- shadows,
- transforms,
- opacity,
- overflow,
- custom properties,
- transitions,
- design-token references.

Unsupported style syntax must fail validation with actionable diagnostics.

## Selector Requirements

The selector model must support:

- element selectors,
- class selectors,
- id selectors,
- direct child selectors,
- descendant selectors,
- Hawk2UI-owned state selectors.

Unsupported selectors must fail at build time.

## Typed Property Requirements

Runtime style data must use typed properties.

Typed properties must define:

- property name,
- value type,
- default value,
- inheritance behavior,
- unit handling,
- validation rules,
- renderer capability requirements,
- layout capability requirements.

Runtime rendering must not depend on string maps for style values.

## Token Requirements

Design tokens must support:

- colors,
- spacing,
- radii,
- typography values,
- motion values,
- theme variants,
- user-editable preference hooks.

## Acceptance Criteria

- Style source compiles into typed records.
- Unsupported properties and selectors fail validation before runtime.
- Runtime style data is typed.
- Design tokens can feed themes, skins, and user preferences.
