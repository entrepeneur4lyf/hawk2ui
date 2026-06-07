#![cfg(target_os = "linux")]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use hawk2ui_host::{
    HostPlatformHandle, PluginEditorConfig, PluginHostEvent, PluginParentHandle, SurfaceMetrics,
};
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
    assert!(
        app.evidence.window_created(),
        "winit parent window should be created"
    );
    assert!(
        app.evidence.child_created(),
        "Baseview child window should be created"
    );
    assert!(
        app.evidence.parent_resize_requested(),
        "Wayland smoke should request a parent resize after child creation"
    );
    assert!(
        app.evidence.parent_resize_request_supported(),
        "Wayland parent should accept a resize request after the child is open"
    );
    assert!(
        app.evidence.child_close_requested(),
        "Wayland smoke should close the Baseview child explicitly"
    );
    assert!(
        app.evidence.child_destroy_observed(),
        "Baseview child close should emit an editor-destroyed event"
    );
    assert!(
        app.evidence.safe_teardown_observed(),
        "Baseview child close should emit safe teardown completion"
    );
    assert!(
        presented_frames.load(Ordering::SeqCst) >= 1,
        "Baseview Wayland child should present at least one software frame"
    );
}

struct SmokeApp {
    started_at: Instant,
    child: Option<baseview::WindowHandle>,
    parent: Option<Window>,
    event_sink: Arc<Mutex<Vec<PluginHostEvent>>>,
    presented_frames: Arc<AtomicU64>,
    last_error: Arc<Mutex<Option<hawk2ui_host_baseview::BaseviewHostError>>>,
    evidence: SmokeEvidence,
}

#[derive(Debug, Default)]
struct SmokeEvidence {
    bits: u16,
}

impl SmokeEvidence {
    const WINDOW_CREATED: u16 = 1 << 0;
    const CHILD_CREATED: u16 = 1 << 1;
    const PARENT_RESIZE_REQUESTED: u16 = 1 << 2;
    const PARENT_RESIZE_REQUEST_SUPPORTED: u16 = 1 << 3;
    const PARENT_RESIZE_OBSERVED: u16 = 1 << 4;
    const CHILD_CLOSE_REQUESTED: u16 = 1 << 5;
    const CHILD_DESTROY_OBSERVED: u16 = 1 << 6;
    const SAFE_TEARDOWN_OBSERVED: u16 = 1 << 7;

    fn mark(&mut self, bit: u16) {
        self.bits |= bit;
    }

    const fn contains(&self, bit: u16) -> bool {
        self.bits & bit != 0
    }

    const fn window_created(&self) -> bool {
        self.contains(Self::WINDOW_CREATED)
    }

    const fn child_created(&self) -> bool {
        self.contains(Self::CHILD_CREATED)
    }

    const fn parent_resize_requested(&self) -> bool {
        self.contains(Self::PARENT_RESIZE_REQUESTED)
    }

    const fn parent_resize_request_supported(&self) -> bool {
        self.contains(Self::PARENT_RESIZE_REQUEST_SUPPORTED)
    }

    const fn child_close_requested(&self) -> bool {
        self.contains(Self::CHILD_CLOSE_REQUESTED)
    }

    const fn child_destroy_observed(&self) -> bool {
        self.contains(Self::CHILD_DESTROY_OBSERVED)
    }

    const fn safe_teardown_observed(&self) -> bool {
        self.contains(Self::SAFE_TEARDOWN_OBSERVED)
    }
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
            event_sink: Arc::new(Mutex::new(Vec::new())),
            presented_frames,
            last_error,
            evidence: SmokeEvidence::default(),
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
        self.evidence.mark(SmokeEvidence::WINDOW_CREATED);

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
        let event_sink = Arc::clone(&self.event_sink);
        let child = adapter
            .open_parented_window(move |_window| {
                BaseviewX11SkiaFrameHandler::new(scene, metrics, presented_frames, last_error)
                    .with_event_sink(event_sink)
            })
            .expect("Baseview should open a native Wayland child window");
        self.child = Some(child);
        self.evidence.mark(SmokeEvidence::CHILD_CREATED);
        self.parent = Some(parent);
    }

    fn drain_child_events(&mut self) {
        let Ok(mut events) = self.event_sink.lock() else {
            return;
        };
        for event in events.drain(..) {
            match event {
                PluginHostEvent::EditorDestroyed(_) => {
                    self.evidence.mark(SmokeEvidence::CHILD_DESTROY_OBSERVED);
                }
                PluginHostEvent::SafeTeardownComplete => {
                    self.evidence.mark(SmokeEvidence::SAFE_TEARDOWN_OBSERVED);
                }
                _ => {}
            }
        }
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
        if self.evidence.parent_resize_requested() && matches!(event, WindowEvent::Resized(_)) {
            self.evidence.mark(SmokeEvidence::PARENT_RESIZE_OBSERVED);
        }
        if matches!(event, WindowEvent::CloseRequested) {
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.drain_child_events();

        if self.evidence.child_close_requested()
            && self.evidence.child_destroy_observed()
            && self.evidence.safe_teardown_observed()
        {
            event_loop.exit();
            return;
        }

        let presented_frames = self.presented_frames.load(Ordering::SeqCst);
        if presented_frames >= 1 && !self.evidence.parent_resize_requested() {
            if let Some(parent) = self.parent.as_ref() {
                if parent
                    .request_inner_size(LogicalSize::new(480.0, 240.0))
                    .is_some()
                {
                    self.evidence
                        .mark(SmokeEvidence::PARENT_RESIZE_REQUEST_SUPPORTED);
                }
                self.evidence.mark(SmokeEvidence::PARENT_RESIZE_REQUESTED);
            }
            return;
        }

        if presented_frames >= 2
            && self.evidence.parent_resize_requested()
            && !self.evidence.child_close_requested()
        {
            if let Some(child) = self.child.as_mut() {
                child.close();
                self.evidence.mark(SmokeEvidence::CHILD_CLOSE_REQUESTED);
            }
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
