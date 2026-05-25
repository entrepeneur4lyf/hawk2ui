# Desktop Host Runtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first real desktop vertical so `hawk2ui run-desktop` opens a native window, renders through Skia, handles lifecycle/input/resize/DPI, and exits cleanly.

**Architecture:** Keep fixture behavior available for tests, but add a production winit runtime path in `hawk2ui-host-winit`. Use a software presentation surface to bridge Skia-rendered CPU pixels into the native window so the first desktop path works on Wayland without relying on OpenGL setup.

**Tech Stack:** Rust, winit 0.30, softbuffer 0.4, skia-safe 0.97, existing Hawk2UI host/CLI crates.

---

## File Structure

- `specs/0023-desktop-host-runtime.md`: behavioral requirements and acceptance criteria only.
- `crates/hawk2ui-host-winit/Cargo.toml`: add software surface dependency.
- `crates/hawk2ui-host-winit/src/software_frame.rs`: Skia-backed software frame renderer that produces full-surface pixels.
- `crates/hawk2ui-host-winit/src/runtime.rs`: production winit event-loop runtime and presentation handling.
- `crates/hawk2ui-host-winit/src/lib.rs`: export the new runtime while preserving fixture adapter APIs.
- `crates/hawk2ui-host-winit/tests/desktop_runtime.rs`: focused tests for frame generation and runtime configuration.
- `crates/hawk2ui-cli/Cargo.toml`: depend on the production host runtime.
- `crates/hawk2ui-cli/src/executor.rs`: make `run-desktop` invoke the production runtime.

## Task 1: Skia Software Frame Renderer

**Files:**
- Create: `crates/hawk2ui-host-winit/src/software_frame.rs`
- Modify: `crates/hawk2ui-host-winit/src/lib.rs`
- Test: `crates/hawk2ui-host-winit/tests/desktop_runtime.rs`

- [ ] Write a failing test that renders a non-empty frame and proves all pixels are initialized.
- [ ] Run: `cargo test -p hawk2ui-host-winit software_frame_renders_visible_pixels`
- [ ] Implement `SoftwareFrameRenderer` using Skia raster rendering and a deterministic fallback scene.
- [ ] Run: `cargo test -p hawk2ui-host-winit software_frame_renders_visible_pixels`
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production-ready stability? If revision is needed, take corrective action.

## Task 2: Production Winit Runtime

**Files:**
- Create: `crates/hawk2ui-host-winit/src/runtime.rs`
- Modify: `crates/hawk2ui-host-winit/src/lib.rs`
- Test: `crates/hawk2ui-host-winit/tests/desktop_runtime.rs`

- [ ] Write failing tests for runtime config validation and first-frame smoke configuration.
- [ ] Run: `cargo test -p hawk2ui-host-winit runtime_config_rejects_zero_size runtime_config_accepts_first_frame_smoke_mode`
- [ ] Implement `WinitDesktopRuntime`, lifecycle summary records, and config validation.
- [ ] Run: `cargo test -p hawk2ui-host-winit runtime_config_rejects_zero_size runtime_config_accepts_first_frame_smoke_mode`
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production-ready stability? If revision is needed, take corrective action.

## Task 3: Event Loop, Resize, Input, DPI, And Present

**Files:**
- Modify: `crates/hawk2ui-host-winit/src/runtime.rs`
- Test: `crates/hawk2ui-host-winit/tests/desktop_runtime.rs`

- [ ] Write focused tests for non-window helpers that classify input, resize, and repaint requirements.
- [ ] Run: `cargo test -p hawk2ui-host-winit runtime_events_request_repaint_after_resize`
- [ ] Implement the `ApplicationHandler` path with close handling, resize/DPI repaint, input counting, and softbuffer presentation.
- [ ] Run: `cargo test -p hawk2ui-host-winit runtime_events_request_repaint_after_resize`
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production-ready stability? If revision is needed, take corrective action.

## Task 4: CLI Wiring

**Files:**
- Modify: `crates/hawk2ui-cli/Cargo.toml`
- Modify: `crates/hawk2ui-cli/src/executor.rs`
- Test: `crates/hawk2ui-cli/tests/commands.rs`

- [ ] Write or update CLI tests so `run-desktop` still validates manifests and can use automated first-frame mode.
- [ ] Run: `cargo test -p hawk2ui-cli run_desktop`
- [ ] Wire `WorkspaceCommandRunner::run_desktop` to `WinitDesktopRuntime::run_blocking`.
- [ ] Run: `cargo test -p hawk2ui-cli run_desktop`
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production-ready stability? If revision is needed, take corrective action.

## Task 5: Verification And Commit

**Files:**
- All changed files.

- [ ] Run: `cargo fmt --all`
- [ ] Run: `cargo test -p hawk2ui-host-winit`
- [ ] Run: `cargo test -p hawk2ui-cli`
- [ ] Run: `cargo check --workspace`
- [ ] Run: `cargo clippy --workspace -- -D warnings`
- [ ] Run GitNexus change detection before commit.
- [ ] Commit with an imperative subject.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production-ready stability? If revision is needed, take corrective action.

## Self-Review

Spec coverage: the plan covers command invocation, native lifecycle, surface metrics, input routing, rendering, resize/DPI repaint, and first-frame smoke verification.

Placeholder scan: no task depends on undefined later work or placeholder behavior.

Type consistency: runtime, frame renderer, CLI wiring, and tests are named consistently across tasks.
