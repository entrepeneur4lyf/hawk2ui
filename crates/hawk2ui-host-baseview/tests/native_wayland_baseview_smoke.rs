#![cfg(target_os = "linux")]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use hawk2ui_host::{HostPlatformHandle, PluginEditorConfig, PluginParentHandle, SurfaceMetrics};
use hawk2ui_host_baseview::{
    BaseviewParentFixture, BaseviewPluginAdapter, BaseviewX11SkiaFrameHandler,
};
use hawk2ui_layout::{FlexDirection, LayoutSizing, LayoutStyle};
use hawk2ui_render::Color;
use hawk2ui_runtime::{RuntimeViewId, RuntimeViewNode, RuntimeViewTree, RuntimeVisual};
use raw_window_handle_06::{HasDisplayHandle, HasWindowHandle};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::wayland::EventLoopBuilderExtWayland;
use winit::window::{Window, WindowAttributes, WindowId};

#[test]
fn native_wayland_baseview_child_renders_one_software_frame_when_enabled() {
    if std::env::var("HAWK2UI_NATIVE_WAYLAND_BASEVIEW_SMOKE")
        .ok()
        .as_deref()
        != Some("1")
    {
        eprintln!(
            "skipping native Wayland Baseview smoke; set HAWK2UI_NATIVE_WAYLAND_BASEVIEW_SMOKE=1"
        );
        return;
    }
    assert!(
        std::env::var("WAYLAND_DISPLAY").is_ok(),
        "native Wayland Baseview smoke requires WAYLAND_DISPLAY"
    );

    let presented_frames = Arc::new(AtomicU64::new(0));
    let last_error = Arc::new(Mutex::new(None));
    let mut event_loop = EventLoop::builder();
    event_loop.with_wayland().with_any_thread(true);
    let event_loop = event_loop.build().expect("Wayland event loop should build");
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = SmokeApp::new(Arc::clone(&presented_frames), Arc::clone(&last_error));
    event_loop
        .run_app(&mut app)
        .expect("Wayland event loop should run");

    if let Some(error) = last_error
        .lock()
        .expect("last_error lock should succeed")
        .clone()
    {
        panic!("Baseview Wayland frame handler failed: {error:?}");
    }
    assert!(app.window_created, "winit parent window should be created");
    assert!(app.child_created, "Baseview child window should be created");
    assert!(
        presented_frames.load(Ordering::SeqCst) >= 1,
        "Baseview Wayland child should present at least one software frame"
    );
}

struct SmokeApp {
    started_at: Instant,
    child: Option<baseview::WindowHandle>,
    parent: Option<Window>,
    presented_frames: Arc<AtomicU64>,
    last_error: Arc<Mutex<Option<hawk2ui_host_baseview::BaseviewHostError>>>,
    window_created: bool,
    child_created: bool,
}

impl SmokeApp {
    fn new(
        presented_frames: Arc<AtomicU64>,
        last_error: Arc<Mutex<Option<hawk2ui_host_baseview::BaseviewHostError>>>,
    ) -> Self {
        Self {
            started_at: Instant::now(),
            child: None,
            parent: None,
            presented_frames,
            last_error,
            window_created: false,
            child_created: false,
        }
    }

    fn create_parent_and_child(&mut self, event_loop: &ActiveEventLoop) {
        if self.parent.is_some() {
            return;
        }

        let attributes = WindowAttributes::default()
            .with_title("hawk2ui-baseview-wayland-smoke")
            .with_inner_size(LogicalSize::new(320.0, 180.0));
        let parent = event_loop
            .create_window(attributes)
            .expect("winit Wayland parent window should open");
        self.window_created = true;

        let parent_handle = wayland_parent_handle(&parent);
        let metrics = SurfaceMetrics::new(320.0, 180.0, 1.0);
        let editor_config = PluginEditorConfig::new(
            "native-wayland-baseview-smoke",
            PluginParentHandle::opaque("native-wayland-parent"),
            metrics,
        );
        let adapter = BaseviewPluginAdapter::attach(
            editor_config,
            BaseviewParentFixture::from_platform_handle("native-wayland-parent", parent_handle),
        )
        .expect("Baseview adapter should accept a native Wayland parent");

        let scene = smoke_scene();
        let presented_frames = Arc::clone(&self.presented_frames);
        let last_error = Arc::clone(&self.last_error);
        let child = adapter
            .open_parented_window(move |_window| {
                BaseviewX11SkiaFrameHandler::new(scene, metrics, presented_frames, last_error)
                    .close_after_first_frame(true)
            })
            .expect("Baseview should open a native Wayland child window");
        self.child = Some(child);
        self.child_created = true;
        self.parent = Some(parent);
    }
}

impl ApplicationHandler for SmokeApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.create_parent_and_child(event_loop);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if matches!(event, WindowEvent::CloseRequested) {
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.presented_frames.load(Ordering::SeqCst) >= 1 {
            event_loop.exit();
            return;
        }
        if self.started_at.elapsed() > Duration::from_secs(5) {
            event_loop.exit();
        }
    }
}

fn wayland_parent_handle(window: &Window) -> HostPlatformHandle {
    let display = match window
        .display_handle()
        .expect("winit display handle should be available")
        .as_raw()
    {
        raw_window_handle_06::RawDisplayHandle::Wayland(handle) => handle.display.as_ptr() as u64,
        handle => panic!("expected Wayland display handle, got {handle:?}"),
    };
    let surface = match window
        .window_handle()
        .expect("winit window handle should be available")
        .as_raw()
    {
        raw_window_handle_06::RawWindowHandle::Wayland(handle) => handle.surface.as_ptr() as u64,
        handle => panic!("expected Wayland window handle, got {handle:?}"),
    };
    HostPlatformHandle::linux_wayland(display, surface)
}

fn smoke_scene() -> hawk2ui_runtime::RuntimeSceneFrame {
    let root_id = RuntimeViewId::new("root");
    let accent_id = RuntimeViewId::new("accent");
    let root = RuntimeViewNode::new(
        root_id.clone(),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(320.0, 180.0)),
        RuntimeVisual::Fill(Color::rgba(12, 15, 20, 255)),
    );
    let accent = RuntimeViewNode::new(
        accent_id,
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(320.0, 180.0)),
        RuntimeVisual::Fill(Color::rgba(30, 180, 140, 255)),
    );
    let tree = RuntimeViewTree::new(root)
        .with_child(&root_id, accent)
        .expect("accent child should attach");
    hawk2ui_runtime::RuntimeSceneBridge::new(hawk2ui_layout::Viewport::new(320.0, 180.0))
        .build(&tree)
        .expect("smoke runtime scene should build")
}
