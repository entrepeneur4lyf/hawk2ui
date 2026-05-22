# Host And Windowing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a common host surface contract for owned desktop windows and DAW-owned plugin editor surfaces.

**Architecture:** Keep platform adapters behind a host trait that reports size, DPI, focus, input, repaint, presentation, resize, teardown, and capabilities. Desktop and plugin lifecycles use the same render surface contract with different ownership rules.

**Tech Stack:** Rust, winit for desktop adapter, baseview or platform attachment crates for plugin adapter, raw-window-handle, cargo test.

---

## File Structure

- Create: `crates/hawk2ui-host/src/lib.rs` host API exports.
- Create: `crates/hawk2ui-host/src/surface.rs` common surface contract.
- Create: `crates/hawk2ui-host/src/desktop.rs` desktop lifecycle records.
- Create: `crates/hawk2ui-host/src/plugin.rs` plugin lifecycle records.
- Create: `crates/hawk2ui-host/src/platform.rs` platform handle records.
- Create: `crates/hawk2ui-host/tests/surface_lifecycle.rs` lifecycle tests.

## Tasks

### Task 1: Common Surface Contract

- [ ] Define `HostSurface`, `SurfaceMetrics`, `SurfaceEvent`, `RepaintRequest`, `FramePresenter`, and `HostCapabilities`.
- [ ] Test logical size, physical size, DPI, focus, repaint, resize, and teardown event reporting.
- [ ] Run: `rtk cargo test -p hawk2ui-host surface_contract`.
- [ ] Commit: `Add common host surface contract`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 2: Desktop Lifecycle

- [ ] Add desktop records for owned window creation, close requests, minimize, maximize, fullscreen, focus, keyboard, pointer, clipboard capability, DPI changes, and renderer target recreation.
- [ ] Add tests using a recording desktop adapter event queue.
- [ ] Run: `rtk cargo test -p hawk2ui-host desktop_lifecycle`.
- [ ] Commit: `Add desktop host lifecycle`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 3: Plugin Lifecycle

- [ ] Add plugin records for parent attachment, editor create/destroy, host-driven resize, DPI changes, repaint scheduling, focus routing, keyboard routing, pointer routing, and safe teardown.
- [ ] Add tests proving plugin teardown does not request process quit.
- [ ] Run: `rtk cargo test -p hawk2ui-host plugin_lifecycle`.
- [ ] Commit: `Add plugin host lifecycle`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 4: Platform Handle Types

- [ ] Add Windows HWND, macOS NSView/NSWindow, Linux Wayland, X11/XCB, and XWayland handle records behind typed enums.
- [ ] Add compile tests that unsupported handle/surface combinations produce diagnostics.
- [ ] Run: `rtk cargo test -p hawk2ui-host platform_handles`.
- [ ] Commit: `Add platform host handle records`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 5: Renderer Resize Bridge

- [ ] Add bridge code that converts host resize and DPI events into renderer target recreation requests.
- [ ] Add tests for maximize and DPI changes forcing redraw and target recreation.
- [ ] Run: `rtk cargo test -p hawk2ui-host renderer_resize_bridge`.
- [ ] Commit: `Bridge host resize events to renderer`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

## Verification

- [ ] Run: `rtk cargo fmt --all -- --check`.
- [ ] Run: `rtk cargo test -p hawk2ui-host`.
- [ ] Run: `rtk cargo test --workspace host`.
