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

- `skia-cpu-raster`: supported; covers CPU raster, high DPI, text shaping, source-rect/sampled/tiled image draws, vector layers, structured effects, SVG clip paths, explicit blend-mode rect compositing, `SkRuntimeEffect` shader effects, runtime-scene replay, and dirty regions.
- `skia-gpu-candidate`: not currently marked supported for all hosts; non-promoted GPU paths must report `backend.capability.gpu-unavailable` instead of silently falling back when GPU is required.
- `skia-gpu-wayland-desktop`: supported for `winit` desktop windows on native Wayland; the host creates a Wayland EGL/Glutin surface, renders runtime scenes through Skia Ganesh GL, swaps native buffers, and records readback evidence in the desktop runtime summary.
- `skia-gpu-wayland-baseview-plugin`: supported for Baseview-backed plugin editor child windows on native Wayland; the host accepts Wayland parent handles, creates child EGL/OpenGL contexts, renders runtime scenes through Skia Ganesh GL, reports context creation failures as hard diagnostics, and covers resize-after-GL creation in the gated native smoke.

The stable diagnostic rules for backend capability gaps are `backend.capability.unsupported` and `backend.capability.gpu-unavailable`.

## Authoring Model

Authors should describe UI structure, style, layout, assets, and custom draw hooks. The renderer owns command ordering, layers, backend selection, and diagnostics. If a backend cannot support a requested feature, it must report a `BackendDiagnostic` instead of producing silent visual drift.

## Runtime Shader Effects

`hawk2ui-render-skia` supports Skia runtime shader effects through a bounded, typed API. Applications register a stable effect ID with `SkSL` source, then draw effect-filled rectangles with typed float/int uniform bindings and optional registered image child shaders. The backend validates source size, duplicate effect IDs, uniform arity/type, missing declarations, child shader bindings, image registration, geometry, and active-frame lifecycle before presenting pixels.

Runtime shader effects are also available through the framework/runtime path. Native authoring records can provide `shader_effect_id`, `shader_effect_source`, `shader_color`, and optional image child bindings; the bridge lowers them into `RuntimeShaderEffectVisual`, `RuntimeDrawCommand::ShaderEffect`, and Skia scene replay.

## Image Draw Controls

`SkiaImageDrawOptions` controls source rectangles, nearest or linear sampling, mipmap mode, and horizontal/vertical tile modes. Default `draw_image_rect(...)` remains the full-image clamp draw. Tiled source rectangles must be pixel-aligned because Skia creates integer source subsets before shader tiling.

## Text Draw Controls

`SkiaTextDrawOptions` controls highlight rectangles, stroke passes, underline, strikethrough, and Skia subpixel positioning for direct text draws. The existing `draw_text_at(...)` API remains the fill-only default, and `draw_text_at_with_options(...)` records explicit option evidence.

## Compositing Controls

The Skia backend supports SVG clip paths through `push_clip_path(...)` and explicit blend-mode rectangle compositing through `draw_blended_rect(...)`. Supported blend modes include source-over, source, destination, plus, multiply, screen, overlay, and difference.
