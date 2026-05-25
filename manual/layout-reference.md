# Hawk2UI Layout Reference

The layout API turns typed style records and content measurements into geometry that can be attached to the render scene. This page names the implemented public layout records that application and integration authors should expect to see at the boundary.

## Layout Records

- `LayoutTree`: retained layout tree input and output container.
- `LayoutStyle`: typed layout style applied to nodes.
- `FlexDirection`: flex axis direction record used by layout style.
- `PluginEditorConstraints`: plugin editor sizing constraints for host embedding.
- `SceneGeometryAttachment`: geometry bridge from layout output to scene rendering.
- `TextMeasureInput`: text measurement input used before final layout.

## Layout Flow

A typical flow is:

1. Parse source and style into typed records.
2. Build a `LayoutTree` with node styles and content.
3. Apply `LayoutStyle` and `FlexDirection` values.
4. Measure text with `TextMeasureInput`.
5. Apply desktop or plugin constraints, including `PluginEditorConstraints` for embedded editors.
6. Attach computed geometry to the scene through `SceneGeometryAttachment`.

The layout surface is intentionally data-oriented so desktop windows and plugin hosts can share the same geometry contract.
