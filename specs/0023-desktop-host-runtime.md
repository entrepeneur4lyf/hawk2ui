# Spec 0023: Desktop Host Runtime

## Status

Final baseline.

## Purpose

This spec defines the required behavior for running a Hawk2UI desktop application as a native owned window.

## Command Requirements

`hawk2ui run-desktop` must:

- load the application manifest from the selected workspace,
- validate that at least one desktop target is declared,
- create an owned native desktop window,
- enter the platform event loop,
- render visible content through the renderer backend,
- keep running until the user or platform requests close,
- return a failure diagnostic when setup, rendering, or presentation fails.

## Window Lifecycle Requirements

A desktop window must support:

- native creation with manifest-derived title and initial dimensions,
- close requests from the window manager,
- clean event-loop exit after close,
- focus gained and focus lost notifications,
- minimize, maximize, restore, and resize notifications where the platform reports them,
- renderer target recreation or resize when the drawable surface changes,
- teardown without leaked host resources.

## Surface Requirements

A desktop surface must expose:

- logical size,
- physical pixel size,
- DPI scale factor,
- frame buffer dimensions,
- repaint request scheduling,
- frame presentation result,
- diagnostics for failed resize, render, or present operations.

## Input Requirements

The desktop host must route platform input into Hawk2UI event records for:

- keyboard press and release,
- pointer motion,
- pointer button press and release,
- pointer wheel or scroll movement,
- focus changes,
- modifier state when reported by the platform.

## Rendering Requirements

A desktop frame must:

- be rendered by the renderer backend,
- cover the full current drawable surface after create, resize, maximize, restore, and DPI changes,
- present exactly one complete frame per successful presentation operation,
- avoid stale pixels outside the updated logical scene area,
- fail explicitly when the backing surface cannot be created, resized, rendered, or presented.

## DPI And Resize Requirements

The desktop host must:

- detect DPI scale changes reported by the platform,
- update logical-to-physical conversion after scale changes,
- resize the presentation surface before rendering the next frame,
- repaint the full surface after resize, maximize, restore, or DPI changes,
- keep text and vector rendering aligned to the current scale factor.

## Smoke Test Requirements

A local smoke test must prove:

- `hawk2ui run-desktop` opens a visible native window,
- the close button exits the process cleanly,
- resizing or maximizing repaints the full window,
- text remains visible after resize,
- keyboard and pointer events are accepted without panic,
- an automated first-frame mode can render and exit for CI or headless-compatible verification.

## Acceptance Criteria

- The production desktop run path uses a real native event loop, not a fixture or simulated window.
- The production desktop run path renders through Skia before presenting to the host surface.
- Close requests exit cleanly.
- Resize, maximize, restore, and DPI changes trigger full-surface repaint.
- Input and focus events are captured as runtime-visible host events.
- First-frame smoke mode renders one real frame and exits without requiring manual interaction.
