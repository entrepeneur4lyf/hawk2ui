# Spec 0004: Host And Windowing

## Status

Final baseline.

## Purpose

This spec defines host and windowing requirements for desktop applications and plugin editor surfaces.

## Host Surface Requirements

A host surface must provide:

- logical size,
- physical size,
- DPI scale,
- focus state,
- input event delivery,
- repaint requests,
- frame presentation,
- resize notifications,
- teardown notifications,
- backend capability reporting.

## Desktop Surface Requirements

Desktop surfaces must support:

- owned native window creation,
- close requests,
- minimize and maximize events,
- fullscreen requests,
- focus changes,
- keyboard input,
- pointer input,
- clipboard integration through declared capability,
- DPI changes,
- renderer target recreation.

## Plugin Surface Requirements

Plugin surfaces must support:

- DAW-owned parent attachment,
- editor create and destroy,
- host-driven resize,
- DPI changes,
- repaint scheduling,
- focus routing where available,
- keyboard routing where available,
- pointer routing,
- safe teardown without process-level quit behavior.

## Platform Requirements

Windows support must include owned HWND surfaces and child HWND attachment.

macOS support must include owned window surfaces and embedded NSView/NSWindow integration.

Linux support must account for Wayland, X11/XCB, and XWayland host behavior.

Windows and macOS are mandatory production targets. Public release is blocked until both platforms have verified desktop and plugin host coverage for open, close, resize, DPI changes, focus, keyboard, pointer input, repaint scheduling, software presentation, GPU presentation where declared, and safe teardown.

## Acceptance Criteria

- Desktop and plugin surfaces expose a common rendering surface contract.
- Plugin surfaces do not assume top-level window ownership.
- Resize and DPI changes are reported to rendering.
- Teardown is safe for app processes and plugin host processes.
- Windows and macOS native host behavior has recorded release evidence before announcement.
