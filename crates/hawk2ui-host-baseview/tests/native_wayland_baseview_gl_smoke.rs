#![cfg(target_os = "linux")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use baseview::gl::GlConfig;
use baseview::{Event, EventStatus, WindowHandler};
use hawk2ui_host::{HostPlatformHandle, PluginEditorConfig, PluginParentHandle, SurfaceMetrics};
use hawk2ui_host_baseview::{BaseviewParentFixture, BaseviewPluginAdapter};
use raw_window_handle_06::{HasDisplayHandle, HasWindowHandle};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::wayland::EventLoopBuilderExtWayland;
use winit::window::{Window, WindowAttributes, WindowId};

#[test]
fn native_wayland_baseview_child_exposes_egl_gl_context_when_enabled() {
    if std::env::var("HAWK2UI_NATIVE_WAYLAND_BASEVIEW_GL_SMOKE")
        .ok()
        .as_deref()
        != Some("1")
    {
        eprintln!(
            "skipping native Wayland Baseview GL smoke; set HAWK2UI_NATIVE_WAYLAND_BASEVIEW_GL_SMOKE=1"
        );
        return;
    }
    assert!(
        std::env::var("WAYLAND_DISPLAY").is_ok(),
        "native Wayland Baseview GL smoke requires WAYLAND_DISPLAY"
    );

    let context_seen = Arc::new(AtomicBool::new(false));
    let resize_requested = Arc::new(AtomicBool::new(false));
    let frame_completed = Arc::new(AtomicBool::new(false));
    let last_error = Arc::new(Mutex::new(None));
    let mut event_loop = EventLoop::builder();
    event_loop.with_wayland().with_any_thread(true);
    let event_loop = event_loop.build().expect("Wayland event loop should build");
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = GlSmokeApp::new(
        Arc::clone(&context_seen),
        Arc::clone(&resize_requested),
        Arc::clone(&frame_completed),
        Arc::clone(&last_error),
    );
    event_loop
        .run_app(&mut app)
        .expect("Wayland event loop should run");

    if let Some(error) = last_error
        .lock()
        .expect("last_error lock should succeed")
        .clone()
    {
        panic!("Baseview Wayland GL handler failed: {error}");
    }
    assert!(app.window_created, "winit parent window should be created");
    assert!(app.child_created, "Baseview child window should be created");
    assert!(
        context_seen.load(Ordering::SeqCst),
        "Baseview Wayland child should expose an EGL/OpenGL context when gl_config is set"
    );
    assert!(
        resize_requested.load(Ordering::SeqCst),
        "Baseview Wayland child should accept a resize after GL context creation"
    );
    assert!(
        frame_completed.load(Ordering::SeqCst),
        "Baseview Wayland GL context should make current and swap buffers"
    );
}

struct GlSmokeApp {
    started_at: Instant,
    child: Option<baseview::WindowHandle>,
    parent: Option<Window>,
    context_seen: Arc<AtomicBool>,
    resize_requested: Arc<AtomicBool>,
    frame_completed: Arc<AtomicBool>,
    last_error: Arc<Mutex<Option<String>>>,
    window_created: bool,
    child_created: bool,
}

impl GlSmokeApp {
    fn new(
        context_seen: Arc<AtomicBool>,
        resize_requested: Arc<AtomicBool>,
        frame_completed: Arc<AtomicBool>,
        last_error: Arc<Mutex<Option<String>>>,
    ) -> Self {
        Self {
            started_at: Instant::now(),
            child: None,
            parent: None,
            context_seen,
            resize_requested,
            frame_completed,
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
            .with_title("hawk2ui-baseview-wayland-gl-smoke")
            .with_inner_size(LogicalSize::new(320.0, 180.0));
        let parent = event_loop
            .create_window(attributes)
            .expect("winit Wayland parent window should open");
        self.window_created = true;

        let parent_handle = wayland_parent_handle(&parent);
        let metrics = SurfaceMetrics::new(320.0, 180.0, 1.0);
        let editor_config = PluginEditorConfig::new(
            "native-wayland-baseview-gl-smoke",
            PluginParentHandle::opaque("native-wayland-parent"),
            metrics,
        );
        let adapter = BaseviewPluginAdapter::attach(
            editor_config,
            BaseviewParentFixture::from_platform_handle("native-wayland-parent", parent_handle),
        )
        .expect("Baseview adapter should accept a native Wayland parent");

        let mut options = adapter.open_options().clone();
        options = options.with_gl_config(GlConfig::default());

        let context_seen = Arc::clone(&self.context_seen);
        let resize_requested = Arc::clone(&self.resize_requested);
        let frame_completed = Arc::clone(&self.frame_completed);
        let last_error = Arc::clone(&self.last_error);
        let child = adapter
            .open_parented_window_with_options(options, move |window| GlSmokeHandler {
                context_available_at_build: window.gl_context().is_some(),
                context_seen,
                resize_requested,
                frame_completed,
                last_error,
            })
            .expect("Baseview should open a native Wayland GL child window");
        self.child = Some(child);
        self.child_created = true;
        self.parent = Some(parent);
    }
}

impl ApplicationHandler for GlSmokeApp {
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
        if self.frame_completed.load(Ordering::SeqCst) {
            event_loop.exit();
            return;
        }
        if self.started_at.elapsed() > Duration::from_secs(5) {
            event_loop.exit();
        }
    }
}

struct GlSmokeHandler {
    context_available_at_build: bool,
    context_seen: Arc<AtomicBool>,
    resize_requested: Arc<AtomicBool>,
    frame_completed: Arc<AtomicBool>,
    last_error: Arc<Mutex<Option<String>>>,
}

impl WindowHandler for GlSmokeHandler {
    fn on_frame(&mut self, window: &mut baseview::Window) {
        if !self.context_available_at_build {
            self.record_error("GL context was unavailable during Baseview build".to_owned());
            window.close();
            return;
        }
        if window.gl_context().is_none() {
            self.record_error("GL context was unavailable during frame rendering".to_owned());
            window.close();
            return;
        }
        self.context_seen.store(true, Ordering::SeqCst);

        window.resize(baseview::Size::new(400.0, 220.0));
        self.resize_requested.store(true, Ordering::SeqCst);

        let Some(context) = window.gl_context() else {
            self.record_error("GL context was unavailable after Wayland resize".to_owned());
            window.close();
            return;
        };
        context.swap_buffers();
        self.frame_completed.store(true, Ordering::SeqCst);
        window.close();
    }

    fn on_event(&mut self, _window: &mut baseview::Window, _event: Event) -> EventStatus {
        EventStatus::Ignored
    }
}

impl GlSmokeHandler {
    fn record_error(&self, error: String) {
        let mut last_error = self
            .last_error
            .lock()
            .expect("last_error lock should succeed");
        *last_error = Some(error);
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
