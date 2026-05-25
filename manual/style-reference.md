# Hawk2UI Style Reference

The style reference documents the production style registry implemented by `hawk2ui-style`. Unsupported selectors or properties are diagnostics, not silent no-ops.

## Production Properties

The production property registry contains these property names:

- `display`
- `font-size`
- `color`
- `border-width`
- `border-radius`
- `box-shadow`
- `transform`
- `opacity`
- `overflow`
- `--accent-color`
- `transition-duration`
- `background-color`

## Selector Diagnostics

The implemented selector validator reports these stable rules for unsupported selector forms:

- `selector.combinator.unsupported`
- `selector.state.unsupported`
- `selector.attribute.unsupported`
- `selector.list.unsupported`

## Authoring Guidance

Use the supported property subset for deterministic layout and rendering. Keep selectors simple, prefer explicit component classes or element identifiers, and move host-specific behavior into runtime or plugin contracts instead of style selectors.

Use `examples/style-gallery/manifest.hawk.toml` as the repository-backed style fixture.
