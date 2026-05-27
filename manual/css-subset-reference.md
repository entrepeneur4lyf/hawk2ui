# CSS Subset Reference

This page defines the production CSS subset accepted by `hawk2ui-style`. The compiler parses source with Lightning CSS, then lowers only the typed subset below. Unsupported syntax fails with structured diagnostics instead of being ignored.

## Selectors

Supported selector forms are element selectors, class selectors, ID selectors, direct-child combinators, descendant combinators, and Hawk2UI-owned `:hawk(state)` selectors.

Rejected selector forms include selector lists, attribute selectors, sibling combinators, and non-Hawk pseudo classes.

## Properties

The production property set is `display`, `font-size`, `color`, `border-width`, `border-radius`, `box-shadow`, `transform`, `opacity`, `overflow`, `--accent-color`, `transition-duration`, and `background-color`.

## Units

Supported units are `px` for lengths, unitless `0` for lengths, unitless numbers for numeric properties, `ms` for durations, and `s` for durations.

## Functions

Supported CSS functions are `rgb()`, `rgba()`, and `token()`. Renderer expressions for `box-shadow` and `transform` are preserved as typed renderer strings after Lightning CSS parsing.

## Tokens

Token-backed declarations use `token(path.name)`. CSS `var()` is not accepted in production styles because runtime values must be resolved through the typed token system.

## Inheritance

Inheritance is property metadata, not implicit string behavior. `font-size` and `color` inherit. Layout, border, radius, shadow, transform, compositing, overflow, transition, and token-backed surface properties do not inherit unless their registry metadata says otherwise.

## Shorthands

CSS shorthands are rejected. Authors must use explicit longhand properties such as `border-width`, `border-radius`, and `transition-duration`.

## Transitions

The supported transition surface is `transition-duration` only. Full CSS `transition` shorthand, easing functions, transition-property, and keyframe animation are outside the production subset until they are represented by typed animation records.

## Keyframes

`@keyframes` is rejected. Hawk2UI animation is driven by runtime animation frame scheduling and typed animation records, not CSS keyframe text.

## Diagnostics

The compiler emits these stable diagnostics for subset violations:

- `selector.combinator.unsupported`
- `selector.state.unsupported`
- `selector.attribute.unsupported`
- `selector.list.unsupported`
- `style.shorthand.unsupported`
- `style.unit.unsupported`
- `style.function.unsupported`
- `style.keyframes.unsupported`
- `style.at-rule.unsupported`
- `style.property.unknown`
- `style.value.unsupported`
- `style.value.range`
