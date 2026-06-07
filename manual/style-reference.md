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

## Supported Units

The compiler accepts `px`, unitless `0` for lengths, unitless numbers for numeric properties, `ms`, and `s`.

## Supported Functions

The compiler accepts `rgb()`, `rgba()`, and `token()`. Use `token(color.surface)` style references for themeable values; CSS `var()` is rejected.

## Selector Diagnostics

The implemented selector validator reports these stable rules for unsupported selector forms:

- `selector.combinator.unsupported`
- `selector.state.unsupported`
- `selector.attribute.unsupported`
- `selector.list.unsupported`

## Rejected CSS

The implemented compiler reports these stable rules for unsupported style forms:

- `style.shorthand.unsupported`
- `style.unit.unsupported`
- `style.function.unsupported`
- `style.keyframes.unsupported`
- `style.at-rule.unsupported`

The full user-facing subset is maintained in `manual/css-subset-reference.md`.

## Authoring Guidance

Use the supported property subset for deterministic layout and rendering. Keep selectors simple, prefer explicit component classes or element identifiers, and move host-specific behavior into runtime or plugin contracts instead of style selectors.

## Runtime Cascade

Runtime style computation matches selectors against the `RuntimeStyleTree`, then applies specificity and source-order precedence. ID selectors outrank class and state selectors, class and `:hawk(...)` state selectors outrank element selectors, and later rules win when specificity ties.

Each computed node receives every production property. Explicit declarations win first, inherited properties use the parent computed value when no declaration matches, and the remaining properties use registry initial values.

## Tokens, Themes, And Preferences

`token(...)` declarations resolve through `TokenSet` at runtime. Theme variants override base tokens when `StyleRuntimeEnvironment::with_theme(...)` is active, and missing theme tokens fall back to base tokens.

Preference hook tokens can be redirected with `StyleRuntimeEnvironment::with_preference_override(...)`. A changed theme or preference environment is compared through `RuntimeStyleTable::diff_from(...)`, which returns affected node IDs for render invalidation.

Use `examples/style-gallery/hawk.json` as the repository-backed style fixture.
