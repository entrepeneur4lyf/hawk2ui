#![cfg(target_os = "linux")]
//! Gated native GPU smoke: opens a real X11 child window with an OpenGL context,
//! renders an asymmetric two-band scene through Skia's Ganesh GPU backend, and
//! verifies by pixel readback that real geometry was rendered — not a blank
//! frame, a channel-swapped frame, or only one band — before tearing the GPU
//! context down on close. Reaching the end after `close()` is itself the
//! teardown assertion: a `DirectContext`/`Surface` released against a dead GL
//! context would crash the process before we get there.
//!
//! Mirrors `native_parented_smoke`; gated off by default. Set
//! `HAWK2UI_NATIVE_BASEVIEW_SMOKE=1` with a live X server (e.g. `DISPLAY=:0`) to
//! actually run it — `cargo check-fast` never does, so run it by hand.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use baseview::gl::{GlConfig, Profile};
use hawk2ui_host::{HostPlatformHandle, PluginEditorConfig, PluginParentHandle, SurfaceMetrics};
use hawk2ui_host_baseview::{
    BaseviewGlSkiaFrameHandler, BaseviewParentFixture, BaseviewPluginAdapter,
};
use hawk2ui_layout::{FlexDirection, LayoutSizing, LayoutStyle, Viewport};
use hawk2ui_render::Color;
use hawk2ui_render_skia::SkiaFrameSnapshot;
use hawk2ui_runtime::{
    RuntimeSceneBridge, RuntimeSceneFrame, RuntimeViewId, RuntimeViewNode, RuntimeViewTree,
    RuntimeVisual,
};
use x11rb::{
    COPY_DEPTH_FROM_PARENT,
    connection::Connection,
    protocol::xproto::{ConnectionExt, CreateWindowAux, EventMask, WindowClass},
    rust_connection::RustConnection,
};

#[test]
fn gpu_editor_opens_gl_window_renders_and_tears_down_when_smoke_enabled() {
    if std::env::var("HAWK2UI_NATIVE_BASEVIEW_SMOKE")
        .ok()
        .as_deref()
        != Some("1")
    {
        eprintln!("skipping native GPU editor smoke; set HAWK2UI_NATIVE_BASEVIEW_SMOKE=1");
        return;
    }

    let metrics = SurfaceMetrics::new(320.0, 180.0, 1.0);
    let parent = X11ParentWindow::open().expect("x11 parent window opens");
    let adapter = BaseviewPluginAdapter::attach(
        PluginEditorConfig::new(
            "native-gpu-editor-smoke",
            PluginParentHandle::opaque("x11-parent"),
            metrics,
        ),
        BaseviewParentFixture::from_platform_handle(
            "x11-gpu-smoke-parent",
            HostPlatformHandle::linux_x11(1, u64::from(parent.window)),
        ),
    )
    .expect("baseview adapter attaches to x11 parent record");

    let presented_frames = Arc::new(AtomicU64::new(0));
    let last_error = Arc::new(Mutex::new(None));
    let snapshot_sink: Arc<Mutex<Option<SkiaFrameSnapshot>>> = Arc::new(Mutex::new(None));

    // Open with a non-sRGB compatibility-profile GL framebuffer, matching the
    // Ganesh surface the handler wraps over it (see `gpu_editor_gl_config`).
    let mut options = adapter.open_options().clone();
    options.gl_config = Some(GlConfig {
        srgb: false,
        profile: Profile::Core,
        ..GlConfig::default()
    });

    let scene = two_band_scene();
    let handler_frames = Arc::clone(&presented_frames);
    let handler_error = Arc::clone(&last_error);
    let handler_sink = Arc::clone(&snapshot_sink);

    let mut handle = adapter
        .open_parented_window_with_options(options, move |window| {
            BaseviewGlSkiaFrameHandler::new(window, scene, metrics, handler_frames, handler_error)
                .with_snapshot_sink(handler_sink)
                .close_after_first_frame(true)
        })
        .expect("baseview opens a real GL child window and handler");

    let deadline = Instant::now() + Duration::from_secs(5);
    while handle.is_open() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    if handle.is_open() {
        handle.close();
    }

    assert_eq!(
        *last_error.lock().expect("error lock"),
        None,
        "the GPU editor recorded a render error"
    );
    assert!(
        presented_frames.load(Ordering::SeqCst) >= 1,
        "expected the GPU editor to present at least one frame"
    );

    let snapshot = snapshot_sink
        .lock()
        .expect("snapshot lock")
        .take()
        .expect("the GPU editor read back a frame for verification");
    let top = pixel_rgb(&snapshot, 160, 45);
    let bottom = pixel_rgb(&snapshot, 160, 135);
    assert!(
        near(top, (200, 40, 40)),
        "top band rendered {top:?}, expected red ~(200,40,40) — blank or channel-swapped GPU render?"
    );
    assert!(
        near(bottom, (40, 40, 200)),
        "bottom band rendered {bottom:?}, expected blue ~(40,40,200) — blank or channel-swapped GPU render?"
    );

    // Reaching here, after `close()`, means GPU teardown on `WillClose` released
    // the Ganesh context without crashing against a dead GL context.
    eprintln!("native GPU editor smoke passed: rendered, verified, and torn down cleanly");
}

/// A 320x180 column split into a red top band and a distinct blue bottom band.
/// The asymmetry makes a channel swizzle (red reads as blue) and a missing band
/// detectable by per-band pixel readback.
fn two_band_scene() -> RuntimeSceneFrame {
    let root_id = RuntimeViewId::new("gpu-smoke-root");
    let root = RuntimeViewNode::new(
        root_id.clone(),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(320.0, 180.0)),
        RuntimeVisual::Fill(Color::rgba(0, 0, 0, 255)),
    );
    let top = RuntimeViewNode::new(
        RuntimeViewId::new("gpu-smoke-top"),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(320.0, 90.0)),
        RuntimeVisual::Fill(Color::rgba(200, 40, 40, 255)),
    );
    let bottom = RuntimeViewNode::new(
        RuntimeViewId::new("gpu-smoke-bottom"),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(320.0, 90.0)),
        RuntimeVisual::Fill(Color::rgba(40, 40, 200, 255)),
    );
    let tree = RuntimeViewTree::new(root)
        .with_child(&root_id, top)
        .expect("top band attaches")
        .with_child(&root_id, bottom)
        .expect("bottom band attaches");
    RuntimeSceneBridge::new(Viewport::new(320.0, 180.0))
        .build(&tree)
        .expect("two-band GPU smoke scene builds")
}

/// Extracts an `(r, g, b)` triple from a `0x00RRGGBB` snapshot pixel.
fn pixel_rgb(snapshot: &SkiaFrameSnapshot, column: u32, row: u32) -> (u8, u8, u8) {
    let index = usize::try_from(row * snapshot.width() + column).expect("pixel index fits usize");
    let [_alpha, red, green, blue] = snapshot.pixels()[index].to_be_bytes();
    (red, green, blue)
}

/// Whether each channel is within a small tolerance, allowing for GPU rounding.
fn near(actual: (u8, u8, u8), expected: (u8, u8, u8)) -> bool {
    let delta = |a: u8, b: u8| a.abs_diff(b);
    delta(actual.0, expected.0) <= 6
        && delta(actual.1, expected.1) <= 6
        && delta(actual.2, expected.2) <= 6
}

struct X11ParentWindow {
    connection: RustConnection,
    window: u32,
}

impl X11ParentWindow {
    fn open() -> Result<Self, String> {
        let (connection, screen_number) =
            x11rb::connect(None).map_err(|error| format!("x11 connect failed: {error}"))?;
        let screen = &connection.setup().roots[screen_number];
        let window = connection
            .generate_id()
            .map_err(|error| format!("x11 window id allocation failed: {error}"))?;
        connection
            .create_window(
                COPY_DEPTH_FROM_PARENT,
                window,
                screen.root,
                0,
                0,
                320,
                180,
                0,
                WindowClass::INPUT_OUTPUT,
                0,
                &CreateWindowAux::new()
                    .background_pixel(screen.black_pixel)
                    .event_mask(EventMask::EXPOSURE | EventMask::STRUCTURE_NOTIFY),
            )
            .map_err(|error| format!("x11 parent window creation failed: {error}"))?;
        connection
            .map_window(window)
            .map_err(|error| format!("x11 parent window map failed: {error}"))?;
        connection
            .flush()
            .map_err(|error| format!("x11 parent window flush failed: {error}"))?;
        Ok(Self { connection, window })
    }
}

impl Drop for X11ParentWindow {
    fn drop(&mut self) {
        let _ = self.connection.destroy_window(self.window);
        let _ = self.connection.flush();
    }
}
