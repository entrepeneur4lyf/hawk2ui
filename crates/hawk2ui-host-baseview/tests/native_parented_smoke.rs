#![cfg(target_os = "linux")]

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use hawk2ui_host::{
    HostPlatformHandle, PluginEditorConfig, PluginHostAdapter, PluginParentHandle, SurfaceMetrics,
};
use hawk2ui_host_baseview::{
    BaseviewParentFixture, BaseviewPluginAdapter, BaseviewX11SkiaFrameHandler,
};
use hawk2ui_layout::{FlexDirection, LayoutSizing, LayoutStyle, Viewport};
use hawk2ui_render::Color;
use hawk2ui_runtime::{
    RuntimeSceneBridge, RuntimeViewId, RuntimeViewNode, RuntimeViewTree, RuntimeVisual,
};
use x11rb::{
    COPY_DEPTH_FROM_PARENT,
    connection::Connection,
    protocol::xproto::{ConnectionExt, CreateWindowAux, EventMask, WindowClass},
    rust_connection::RustConnection,
};

#[test]
fn baseview_opens_real_parented_x11_surface_when_native_smoke_enabled() {
    if std::env::var("HAWK2UI_NATIVE_BASEVIEW_SMOKE")
        .ok()
        .as_deref()
        != Some("1")
    {
        eprintln!("skipping native Baseview parented smoke; set HAWK2UI_NATIVE_BASEVIEW_SMOKE=1");
        return;
    }

    let parent = X11ParentWindow::open().expect("x11 parent window opens");
    let adapter = BaseviewPluginAdapter::attach(
        PluginEditorConfig::new(
            "native-baseview-smoke",
            PluginParentHandle::opaque("x11-parent"),
            SurfaceMetrics::new(320.0, 180.0, 1.0),
        ),
        BaseviewParentFixture::from_platform_handle(
            "x11-native-smoke-parent",
            HostPlatformHandle::linux_x11(1, u64::from(parent.window)),
        ),
    )
    .expect("baseview adapter attaches to x11 parent record");

    let handler_presented_frames = Arc::new(AtomicU64::new(0));
    let handler_error = Arc::new(Mutex::new(None));
    let scene = runtime_scene();
    let metrics = adapter.metrics();
    let presented_frames = Arc::clone(&handler_presented_frames);
    let presented_error = Arc::clone(&handler_error);
    let mut handle = adapter
        .open_parented_window(move |_| {
            BaseviewX11SkiaFrameHandler::new(scene, metrics, presented_frames, presented_error)
                .close_after_first_frame(true)
        })
        .expect("baseview opens a real parented child window and handler");

    let deadline = Instant::now() + Duration::from_secs(3);
    while handle.is_open() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }

    if handle.is_open() {
        handle.close();
    }

    assert_eq!(handler_presented_frames.load(Ordering::SeqCst), 1);
    assert_eq!(*handler_error.lock().expect("handler error lock"), None);
    assert!(!handle.is_open());
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

fn runtime_scene() -> hawk2ui_runtime::RuntimeSceneFrame {
    let tree = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("native-smoke-root"),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(320.0, 180.0)),
        RuntimeVisual::Fill(Color::rgba(32, 96, 180, 255)),
    ));
    RuntimeSceneBridge::new(Viewport::new(320.0, 180.0))
        .build(&tree)
        .expect("native smoke runtime scene builds")
}
