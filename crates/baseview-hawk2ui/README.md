# baseview

A low-level windowing system geared towards making audio plugin UIs.

`baseview` abstracts the platform-specific windowing APIs (winapi, cocoa, xcb) into a platform-independent API, but otherwise gets out of your way so you can write plugin UIs.

This is a fork of [RustAudio/baseview](https://github.com/RustAudio/baseview) (published on crates.io as `baseview-truce`) carrying patches that haven't yet made it into the upstream crate:

- **Pro Tools (AAX) teardown crash fix** on macOS — see [Pro Tools (AAX) fix](#pro-tools-aax-fix).
- **Host-driven NSView resize → `Resized` events** on macOS — see [macOS frame-change Resized events](#macos-frame-change-resized-events).
- **`Window::set_mouse_cursor` for macOS** — upstream is `todo!()`, see [macOS cursor implementation](#macos-cursor-implementation).
- **`hit_test` gated behind `opengl` cfg** so CPU-only renderers (wgpu via `CAMetalLayer`, CoreGraphics blit) get AppKit's default hit-testing back — see [CPU-only hit-test gate](#cpu-only-hit-test-gate).
- **Crate renamed for `[patch]` ergonomics**: `package = "baseview-truce"`, `[lib].name = "baseview"`. Downstream `[patch.crates-io] baseview = { package = "baseview-truce" }` links cleanly because rustc still sees a crate named `baseview`.
- **Dependency refresh**: `keyboard-types` 0.7, migrated from the legacy `objc` / `cocoa` crates to the `objc2` family, added `objc2-quartz-core`.
- **Release scripts** under `scripts/` for bumping, publishing, verifying, and syncing with upstream — see [Release scripts](#release-scripts).

> **Note:** This package is a temporary fork intended to live only until these patches are merged upstream into [RustAudio/baseview](https://github.com/RustAudio/baseview). Once upstream carries the fixes, switch back to the canonical crate — there is nothing here that should outlive that merge.

## Pro Tools (AAX) fix

Upstream baseview on macOS crashes Pro Tools the moment a plugin editor is closed (or another plugin's editor is opened, which closes the first). This section describes the crash we actually observed, why it happens, and the minimal patch that fixes it.

### Crash signature

```
Thread 0 Crashed:: Main Thread Dispatch queue: com.apple.main-thread
0  libobjc.A.dylib                 objc_msgSend + 56
1  DFW                             -[DFW_NSContainer dealloc] + 56
2  libobjc.A.dylib                 AutoreleasePoolPage::releaseUntil
3  libobjc.A.dylib                 objc_autoreleasePoolPop + 244
4  DFW                             -[DFW_NSApplication sendEvent:] + 2016
5  AppKit                          -[NSApplication _handleEvent:]
6  AppKit                          -[NSApplication run]
7  DFW                             DFW_EventLoop::RunApplicationEventLoop
```

`EXC_BAD_ACCESS` with invalid addresses that varied between runs — `0x0`, `0x5`, `0x6`, `0x0f007fffffffffc0`. The address changes every time, which is the classic signature of dereferencing a freed object whose memory has been reused for unrelated data.

The crash happens deep inside Pro Tools' own event dispatch: `sendEvent:` holds an outer autorelease pool, plugin code runs inside it (our `close()` is called from that event), the pool drains after the event handler returns, and one of the autoreleased objects — always a `DFW_NSContainer` (Avid's private subclass of `NSView`) — tries to message one of its ivars during dealloc. The ivar's pointer is stale: it references an object we destroyed during plugin close.

### Root cause

When baseview's `WindowInner::close()` tears the view down it calls `removeFromSuperview` and then `release`. That is not enough.

While the view was attached, AppKit and the host embedded it in several back-referencing registries that **are not** cleared by `removeFromSuperview`:

- **Window first responder.** Our view can become first responder (it handles keyboard events and overrides `acceptsFirstResponder` to `YES`). `NSWindow.firstResponder` is a raw pointer; on the OS versions Pro Tools ships on it isn't always zeroing-weak, and even where it is, some hosts query or keep a parallel reference.
- **Tracking-area registry.** Baseview creates an `NSTrackingArea` with `owner: self` and adds it to the view. AppKit keeps the tracking area alive; its `owner` back-pointer to our view is not cleared by normal view teardown.
- **Layer contents.** If the view is layer-backed (wgpu via a `CAMetalLayer`, CoreGraphics blit via `setContents:`), the layer holds an image that in turn holds references into the view's rendering context.

Pro Tools' `DFW_NSContainer` — the container view Pro Tools wraps around the plugin embedding area — walks one of these registries during its own dealloc and messages what is now a freed pointer. Because the crash happens inside Pro Tools' outer pool drain, every host framework (CLAP, VST3 on other DAWs) that doesn't wrap plugin events in their own pools this way is unaffected. It's specific to the AAX + DFW shape.

### The fix

`src/macos/window.rs`, inside `WindowInner::close()`, before `removeFromSuperview` + `release`:

1. Wrap the body in a local `NSAutoreleasePool` and drain it at the end. Any ObjC object we autorelease during teardown gets released **here**, not in the host's outer pool.
2. `[window makeFirstResponder: nil]` if we're a responder.
3. Enumerate `view.trackingAreas` and `removeTrackingArea:` each one.
4. `[view.layer setContents: nil]` to drop the layer's image.

That's the whole patch — about 35 added lines in a single file. See the `git diff` vs upstream master.

## macOS frame-change Resized events

Upstream baseview on macOS only fires `WindowEvent::Resized` from `viewDidChangeBackingProperties:`, which AppKit only calls on **backing-scale** changes (the view moves to a different monitor). Every other path that mutates the NSView's frame size — the parent's `autoresizingMask` shrinking / growing the child during a host-window drag, the host directly calling `setFrameSize:` on the embedded view, even baseview's own `Window::resize` trampoline — runs **without** firing `Resized`.

For plugin editors that render through a `CAMetalLayer` (wgpu, anything blitting through `setContents:`) this is the difference between a clean resize and squashed knobs:

- The NSView's frame grows (autoresize works fine).
- The CAMetalLayer's frame implicitly tracks the NSView (good).
- The CAMetalLayer's `drawableSize` (the size of the texture wgpu / CoreGraphics actually paints into) **stays at the old logical size**.
- AppKit composites the small texture into the larger frame, stretching it across the new bounds.

The fix is a single `setFrameSize:` override on baseview's NSView subclass. It calls `super` first (so AppKit's bookkeeping — intrinsic content size, propagation to subview autoresize masks, etc. — runs unchanged), reads the new bounds, and emits `WindowEvent::Resized` with the same `WindowInfo` shape `viewDidChangeBackingProperties:` uses. Backends (egui / iced / slint / vizia) already react to `Resized` by reconfiguring their surfaces, so this single override fixes resize for every backend and every plugin format that embeds via `setFrameSize:`.

LV2 in particular relies on this path heavily: the LV2 UI spec gives the host a `ui:resize` extension to push size changes through, but in practice most hosts (Ardour, Reaper LV2, jalv-gtk on macOS) just resize the parent NSView and expect the child's autoresize mask to do the rest. Without this patch, the editor's wgpu surface stays at the original size and the user sees stretched / squashed content.

## macOS cursor implementation

Upstream `src/macos/cursor.rs` returns `todo!()` for every cursor variant — calling `Window::set_mouse_cursor` on macOS crashes. This fork ports the whole [`MouseCursor`](src/window.rs) enum to the corresponding AppKit `NSCursor`s via `objc2-app-kit`, with `NSCursor::arrowCursor`, `iBeamCursor`, `pointingHandCursor`, `crosshairCursor`, etc. mapped one-to-one where AppKit has a direct equivalent and falling back to the closest cursor for variants AppKit doesn't ship (e.g. `MouseCursor::Hand` → `pointingHandCursor`, `Help` → `arrowCursor`).

The implementation also handles dynamic cursor updates: AppKit's cursor only sticks while the cursor is over the view, so the fork installs the cursor inside the `cursorUpdate:` path so it persists across `NSTrackingArea` re-entries instead of reverting to whatever cursor the host's outer container last set.

## CPU-only hit-test gate

Upstream baseview's `-[NSView hitTest:]` override always runs, even when the consumer isn't using the OpenGL render path. That override exists to redirect hit-tests away from a GL render subview so input events land on baseview's main view; with no GL subview present it has nothing to redirect and can leave events unhandled (cursor reverts to the host's last-set cursor, drags don't start cleanly).

This fork gates the `hitTest:` method registration behind `#[cfg(feature = "opengl")]`. Consumers using baseview purely for windowing + CPU rendering (truce-gui via wgpu / CoreGraphics, anyone painting through `setContents:`) get AppKit's default hit-testing back, which behaves correctly for top-level NSViews.

## Release scripts

`scripts/` carries the publishing workflow for the fork:

- `scripts/bump.sh <version>` — bump the version in `Cargo.toml` + workspace examples, regenerate `Cargo.lock`, and commit.
- `scripts/release.sh` — drives the full release: verify → bump → publish, with confirmation prompts at each step.
- `scripts/publish.sh` — `cargo publish` to crates.io once a release commit is tagged.
- `scripts/verify.sh` — clippy + tests + docs pass before release.
- `scripts/sync-upstream.sh` — fetch RustAudio/baseview's `master` and either merge or rebase the truce-fork patches on top, used to pick up upstream changes before bumping. Keeps the patch set small by replaying onto the latest upstream rather than letting the fork drift.

## License

Licensed under either of <a href="LICENSE-APACHE">Apache License, Version 2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Baseview by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
