# Hawk2UI Rendering Reference

The rendering pipeline consumes layout geometry and produces backend-neutral paint work for native desktop windows and embedded plugin editors.

## Rendering Records

- `SceneGraph`: retained scene structure after layout attachment.
- `PaintCommandList`: ordered drawing commands emitted from the scene.
- `LayerStack`: composited layer ordering for visual output.
- `RendererBackend`: backend capability and execution contract.
- `CustomDrawSurface`: custom drawing integration surface.
- `BackendDiagnostic`: backend capability or runtime diagnostic.

## Graphics Backends

The graphics compatibility matrix names these backends:

- `skia-cpu-raster`: supported; covers CPU raster, high DPI, text shaping, image layers, vector layers, effects, and dirty regions.
- `skia-gpu-candidate`: not currently marked supported; candidate coverage includes high DPI, image layers, vector layers, and effects.

The stable diagnostic rules for backend capability gaps are `backend.capability.unsupported` and `backend.capability.gpu-unavailable`.

## Authoring Model

Authors should describe UI structure, style, layout, assets, and custom draw hooks. The renderer owns command ordering, layers, backend selection, and diagnostics. If a backend cannot support a requested feature, it must report a `BackendDiagnostic` instead of producing silent visual drift.
