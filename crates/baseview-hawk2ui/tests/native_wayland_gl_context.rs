#![cfg(all(target_os = "linux", feature = "opengl"))]

use std::ffi::{c_char, c_uchar, c_void, CStr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use baseview::gl::GlConfig;
use baseview::{Event, EventStatus, WindowHandler, WindowOpenOptions};
use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle,
    WaylandDisplayHandle, WaylandWindowHandle,
};
use raw_window_handle_06::{HasDisplayHandle, HasWindowHandle};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::wayland::EventLoopBuilderExtWayland;
use winit::window::{Window, WindowAttributes, WindowId};

const GL_VERSION: u32 = 0x1F02;

type GlGetString = unsafe extern "system" fn(u32) -> *const c_uchar;

#[test]
fn native_wayland_gl_context_can_be_made_current_when_enabled() {
    if std::env::var("BASEVIEW_NATIVE_WAYLAND_GL_CONTEXT_SMOKE").ok().as_deref() != Some("1") {
        eprintln!(
            "skipping native Baseview Wayland GL context smoke; set BASEVIEW_NATIVE_WAYLAND_GL_CONTEXT_SMOKE=1"
        );
        return;
    }
    assert!(
        std::env::var("WAYLAND_DISPLAY").is_ok(),
        "native Baseview Wayland GL context smoke requires WAYLAND_DISPLAY"
    );

    let build_context_seen = Arc::new(AtomicBool::new(false));
    let frame_completed = Arc::new(AtomicBool::new(false));
    let last_error = Arc::new(Mutex::new(None));
    let mut event_loop = EventLoop::builder();
    event_loop.with_wayland().with_any_thread(true);
    let event_loop = event_loop.build().expect("Wayland event loop should build");
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = BaseviewGlSmokeApp::new(
        Arc::clone(&build_context_seen),
        Arc::clone(&frame_completed),
        Arc::clone(&last_error),
    );
    event_loop.run_app(&mut app).expect("Wayland event loop should run");

    if let Some(error) = last_error.lock().expect("last_error lock should succeed").clone() {
        panic!("Baseview Wayland GL probe failed: {error}");
    }
    assert!(app.parent_created, "winit parent window should be created");
    assert!(app.child_created, "Baseview child window should be created");
    assert!(
        build_context_seen.load(Ordering::SeqCst),
        "Baseview Wayland build callback should see the requested GL context"
    );
    assert!(
        frame_completed.load(Ordering::SeqCst),
        "Baseview Wayland GL context should be currentable and presentable"
    );
}

struct BaseviewGlSmokeApp {
    started_at: Instant,
    child: Option<baseview::WindowHandle>,
    parent: Option<Window>,
    build_context_seen: Arc<AtomicBool>,
    frame_completed: Arc<AtomicBool>,
    last_error: Arc<Mutex<Option<String>>>,
    parent_created: bool,
    child_created: bool,
}

impl BaseviewGlSmokeApp {
    fn new(
        build_context_seen: Arc<AtomicBool>, frame_completed: Arc<AtomicBool>,
        last_error: Arc<Mutex<Option<String>>>,
    ) -> Self {
        Self {
            started_at: Instant::now(),
            child: None,
            parent: None,
            build_context_seen,
            frame_completed,
            last_error,
            parent_created: false,
            child_created: false,
        }
    }

    fn create_parent_and_child(&mut self, event_loop: &ActiveEventLoop) {
        if self.parent.is_some() {
            return;
        }
        let attributes = WindowAttributes::default()
            .with_title("baseview-wayland-gl-context-smoke")
            .with_inner_size(LogicalSize::new(320.0, 180.0));
        let parent =
            event_loop.create_window(attributes).expect("winit Wayland parent window should open");
        self.parent_created = true;

        let raw_parent = WaylandParent::from_window(&parent);
        let options = WindowOpenOptions::new()
            .with_title("baseview-wayland-gl-child")
            .with_size(320.0, 180.0)
            .with_gl_config(GlConfig::default());
        let build_context_seen = Arc::clone(&self.build_context_seen);
        let frame_completed = Arc::clone(&self.frame_completed);
        let last_error = Arc::clone(&self.last_error);
        let child = baseview::Window::open_parented(&raw_parent, options, move |window| {
            build_context_seen.store(window.gl_context().is_some(), Ordering::SeqCst);
            GlContextProbeHandler { frame_completed, last_error }
        });
        self.child = Some(child);
        self.child_created = true;
        self.parent = Some(parent);
    }
}

impl ApplicationHandler for BaseviewGlSmokeApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.create_parent_and_child(event_loop);
    }

    fn window_event(
        &mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent,
    ) {
        if matches!(event, WindowEvent::CloseRequested) {
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.frame_completed.load(Ordering::SeqCst)
            || self.last_error.lock().expect("last_error lock should succeed").is_some()
        {
            event_loop.exit();
            return;
        }
        if self.started_at.elapsed() > Duration::from_secs(5) {
            event_loop.exit();
        }
    }
}

struct GlContextProbeHandler {
    frame_completed: Arc<AtomicBool>,
    last_error: Arc<Mutex<Option<String>>>,
}

impl WindowHandler for GlContextProbeHandler {
    fn on_frame(&mut self, window: &mut baseview::Window) {
        match probe_context(window.gl_context()) {
            Ok(()) => self.frame_completed.store(true, Ordering::SeqCst),
            Err(error) => {
                let mut last_error =
                    self.last_error.lock().expect("last_error lock should succeed");
                *last_error = Some(error.to_owned());
            }
        }
        window.close();
    }

    fn on_event(&mut self, _window: &mut baseview::Window, _event: Event) -> EventStatus {
        EventStatus::Ignored
    }
}

fn probe_context(context: Option<&baseview::gl::GlContext>) -> Result<(), &'static str> {
    let Some(context) = context else {
        return Err("GL context was unavailable during frame rendering");
    };

    unsafe {
        context.make_current();
    }
    let result = probe_current_context(context);
    unsafe {
        context.make_not_current();
    }
    result
}

fn probe_current_context(context: &baseview::gl::GlContext) -> Result<(), &'static str> {
    let get_string = context.get_proc_address("glGetString");
    if get_string.is_null() {
        return Err("glGetString was not resolved from the Wayland GL context");
    }
    let get_string = unsafe { std::mem::transmute::<*const c_void, GlGetString>(get_string) };
    let version = unsafe { get_string(GL_VERSION) };
    if version.is_null() {
        return Err("glGetString(GL_VERSION) returned null");
    }
    let version = unsafe { CStr::from_ptr(version.cast::<c_char>()) };
    if version.to_str().is_err() {
        return Err("glGetString(GL_VERSION) returned invalid UTF-8");
    }
    context.swap_buffers();
    Ok(())
}

#[derive(Clone, Copy)]
struct WaylandParent {
    display: *mut c_void,
    surface: *mut c_void,
}

impl WaylandParent {
    fn from_window(window: &Window) -> Self {
        let display = match window
            .display_handle()
            .expect("winit display handle should be available")
            .as_raw()
        {
            raw_window_handle_06::RawDisplayHandle::Wayland(handle) => {
                handle.display.as_ptr().cast::<c_void>()
            }
            handle => panic!("expected Wayland display handle, got {handle:?}"),
        };
        let surface =
            match window.window_handle().expect("winit window handle should be available").as_raw()
            {
                raw_window_handle_06::RawWindowHandle::Wayland(handle) => {
                    handle.surface.as_ptr().cast::<c_void>()
                }
                handle => panic!("expected Wayland window handle, got {handle:?}"),
            };
        Self { display, surface }
    }
}

unsafe impl HasRawDisplayHandle for WaylandParent {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        let mut handle = WaylandDisplayHandle::empty();
        handle.display = self.display;
        RawDisplayHandle::Wayland(handle)
    }
}

unsafe impl HasRawWindowHandle for WaylandParent {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = WaylandWindowHandle::empty();
        handle.surface = self.surface;
        RawWindowHandle::Wayland(handle)
    }
}
