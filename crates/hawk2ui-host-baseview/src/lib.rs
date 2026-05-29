#![deny(unsafe_code)]
//! `Baseview`-backed embedded plugin host adapter for `Hawk2UI`.

use baseview::{
    EventStatus, Size, Window, WindowHandle, WindowHandler, WindowOpenOptions, WindowScalePolicy,
};
use hawk2ui_host::{
    HostPlatformHandle, KeyboardInput, PluginEditorConfig, PluginHostAdapter, PluginHostEvent,
    PointerInput, SurfaceMetrics, SurfaceOwnership,
};
use hawk2ui_plugin::{
    ParameterValue, PluginStateEnvelope, RealtimeVisualFrameGate, RealtimeVisualPacket,
    RealtimeVisualUiReader, StateValue,
};
use hawk2ui_plugin_adapters::{
    ClapGuiParentHandle, ClapGuiWindowApi, ClapRuntimeEditorSession, PackageDiagnostic,
    PackageMaterializationError,
};
use hawk2ui_render::{Color, RendererBackend};
use hawk2ui_render_skia::{SkiaFrameSnapshot, SkiaRendererBackend, SkiaSurfaceConfig};
use hawk2ui_runtime::RuntimeSceneFrame;
use keyboard_types::{Key, KeyState, KeyboardEvent};
use raw_window_handle::{
    AppKitDisplayHandle, AppKitWindowHandle, HasRawDisplayHandle, HasRawWindowHandle,
    RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowsDisplayHandle, XcbDisplayHandle,
    XcbWindowHandle, XlibDisplayHandle, XlibWindowHandle,
};
#[cfg(target_os = "linux")]
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::{collections::BTreeMap, ffi::c_void, fmt::Write as _, path::PathBuf};
#[cfg(target_os = "linux")]
use x11rb::{
    connection::Connection,
    protocol::xproto::{ConnectionExt, CreateGCAux, ImageFormat},
    rust_connection::RustConnection,
};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-host-baseview";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

/// Headless parent fixture for DAW-owned Baseview editor attachment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BaseviewParentFixture {
    id: &'static str,
    handle: HostPlatformHandle,
}

impl BaseviewParentFixture {
    /// Creates a parent record from a host-provided platform handle.
    #[must_use]
    pub const fn from_platform_handle(id: &'static str, handle: HostPlatformHandle) -> Self {
        Self { id, handle }
    }

    /// Creates a Linux X11 parent fixture.
    #[must_use]
    pub const fn linux_x11() -> Self {
        Self {
            id: "linux-x11-parent",
            handle: HostPlatformHandle::linux_x11(1, 2),
        }
    }

    /// Creates a Linux Wayland parent fixture.
    #[must_use]
    pub const fn wayland() -> Self {
        Self {
            id: "linux-wayland-parent",
            handle: HostPlatformHandle::linux_wayland(3, 4),
        }
    }

    /// Creates a Linux `XWayland` parent fixture.
    #[must_use]
    pub const fn linux_xwayland() -> Self {
        Self {
            id: "linux-xwayland-parent",
            handle: HostPlatformHandle::linux_xwayland(5, 6),
        }
    }

    /// Creates a macOS `NSView` parent fixture.
    #[must_use]
    pub const fn macos_ns_view() -> Self {
        Self {
            id: "macos-nsview-parent",
            handle: HostPlatformHandle::macos_ns_view(5),
        }
    }

    /// Creates a macOS `NSView` parent fixture with the owning `NSWindow`.
    #[must_use]
    pub const fn macos_ns_view_in_window() -> Self {
        Self {
            id: "macos-nsview-window-parent",
            handle: HostPlatformHandle::macos_ns_view_in_window(5, 6),
        }
    }

    /// Returns fixture ID.
    #[must_use]
    pub const fn id(&self) -> &'static str {
        self.id
    }

    /// Returns platform handle.
    #[must_use]
    pub const fn handle(&self) -> HostPlatformHandle {
        self.handle
    }
}

/// Baseview adapter capabilities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BaseviewCapabilities {
    flags: u16,
}

impl BaseviewCapabilities {
    /// Returns plugin editor capabilities.
    #[must_use]
    pub const fn plugin_editor() -> Self {
        Self {
            flags: BASEVIEW_CAP_PARENT_ATTACHMENT
                | BASEVIEW_CAP_CREATE_DESTROY
                | BASEVIEW_CAP_HOST_RESIZE
                | BASEVIEW_CAP_DPI
                | BASEVIEW_CAP_REPAINT
                | BASEVIEW_CAP_FOCUS
                | BASEVIEW_CAP_KEYBOARD
                | BASEVIEW_CAP_POINTER
                | BASEVIEW_CAP_SAFE_TEARDOWN,
        }
    }

    /// Returns whether embedded parent attachment is supported.
    #[must_use]
    pub const fn embedded_parent_attachment(&self) -> bool {
        self.flags & BASEVIEW_CAP_PARENT_ATTACHMENT != 0
    }
}

const BASEVIEW_CAP_PARENT_ATTACHMENT: u16 = 1 << 0;
const BASEVIEW_CAP_CREATE_DESTROY: u16 = 1 << 1;
const BASEVIEW_CAP_HOST_RESIZE: u16 = 1 << 2;
const BASEVIEW_CAP_DPI: u16 = 1 << 3;
const BASEVIEW_CAP_REPAINT: u16 = 1 << 4;
const BASEVIEW_CAP_FOCUS: u16 = 1 << 5;
const BASEVIEW_CAP_KEYBOARD: u16 = 1 << 6;
const BASEVIEW_CAP_POINTER: u16 = 1 << 7;
const BASEVIEW_CAP_SAFE_TEARDOWN: u16 = 1 << 8;

/// Baseview adapter error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BaseviewHostError {
    rule: String,
    message: String,
}

impl BaseviewHostError {
    /// Creates a Baseview host error.
    #[must_use]
    pub fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            rule: rule.into(),
            message: message.into(),
        }
    }

    /// Returns the diagnostic rule.
    #[must_use]
    pub fn rule(&self) -> &str {
        &self.rule
    }
}

/// Native parent backend used by Baseview's `open_parented` entry point.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BaseviewNativeParentBackend {
    /// Windows HWND parent.
    Windows,
    /// macOS `AppKit` parent.
    MacOs,
    /// Linux Xlib parent.
    X11,
    /// Linux XCB parent.
    Xcb,
    /// `XWayland` parent exposed through `Xlib` handles.
    XWayland,
}

/// A validated parent handle that can be passed to `baseview::Window::open_parented`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BaseviewNativeParent {
    handle: BaseviewNativeParentHandle,
    backend: BaseviewNativeParentBackend,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BaseviewNativeParentHandle {
    WindowsHwnd { hwnd: u64 },
    MacOsNsViewInWindow { ns_window: u64, ns_view: u64 },
    LinuxX11 { display: u64, window: u64 },
    LinuxXcb { connection: u64, window: u64 },
    LinuxXWayland { display: u64, window: u64 },
}

impl BaseviewNativeParent {
    /// Creates a Baseview-native parent from a host platform handle.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when Baseview cannot safely attach to the handle.
    pub fn try_from_handle(handle: HostPlatformHandle) -> Result<Self, BaseviewHostError> {
        let (handle, backend) = match handle {
            HostPlatformHandle::WindowsHwnd { hwnd } => {
                require_nonzero_handle(hwnd)?;
                (
                    BaseviewNativeParentHandle::WindowsHwnd { hwnd },
                    BaseviewNativeParentBackend::Windows,
                )
            }
            HostPlatformHandle::MacOsNsView { .. } => {
                return Err(BaseviewHostError::new(
                    "baseview.native-parent.invalid",
                    "baseview macOS attachment requires both NSWindow and NSView handles",
                ));
            }
            HostPlatformHandle::MacOsNsViewInWindow { ns_window, ns_view } => {
                require_nonzero_handle(ns_window)?;
                require_nonzero_handle(ns_view)?;
                (
                    BaseviewNativeParentHandle::MacOsNsViewInWindow { ns_window, ns_view },
                    BaseviewNativeParentBackend::MacOs,
                )
            }
            HostPlatformHandle::MacOsNsWindow { .. } => {
                return Err(BaseviewHostError::new(
                    "baseview.native-parent.invalid",
                    "baseview plugin editors must attach to a child NSView, not a top-level NSWindow",
                ));
            }
            HostPlatformHandle::LinuxWayland { .. } => {
                return Err(BaseviewHostError::new(
                    "baseview.platform.unsupported",
                    "baseview 0.1 Linux backend attaches through X11/XCB/XWayland parent handles, not native Wayland surfaces",
                ));
            }
            HostPlatformHandle::LinuxX11 { display, window } => {
                require_nonzero_handle(display)?;
                require_nonzero_handle(window)?;
                (
                    BaseviewNativeParentHandle::LinuxX11 { display, window },
                    BaseviewNativeParentBackend::X11,
                )
            }
            HostPlatformHandle::LinuxXcb { connection, window } => {
                require_nonzero_handle(connection)?;
                require_xcb_window(window)?;
                (
                    BaseviewNativeParentHandle::LinuxXcb { connection, window },
                    BaseviewNativeParentBackend::Xcb,
                )
            }
            HostPlatformHandle::LinuxXWayland { display, window } => {
                require_nonzero_handle(display)?;
                require_nonzero_handle(window)?;
                (
                    BaseviewNativeParentHandle::LinuxXWayland { display, window },
                    BaseviewNativeParentBackend::XWayland,
                )
            }
        };
        Ok(Self { handle, backend })
    }

    /// Returns the original host handle.
    #[must_use]
    pub const fn handle(&self) -> HostPlatformHandle {
        match self.handle {
            BaseviewNativeParentHandle::WindowsHwnd { hwnd } => {
                HostPlatformHandle::WindowsHwnd { hwnd }
            }
            BaseviewNativeParentHandle::MacOsNsViewInWindow { ns_window, ns_view } => {
                HostPlatformHandle::MacOsNsViewInWindow { ns_window, ns_view }
            }
            BaseviewNativeParentHandle::LinuxX11 { display, window } => {
                HostPlatformHandle::LinuxX11 { display, window }
            }
            BaseviewNativeParentHandle::LinuxXcb { connection, window } => {
                HostPlatformHandle::LinuxXcb { connection, window }
            }
            BaseviewNativeParentHandle::LinuxXWayland { display, window } => {
                HostPlatformHandle::LinuxXWayland { display, window }
            }
        }
    }

    /// Returns the Baseview backend used for this parent.
    #[must_use]
    pub const fn backend(&self) -> BaseviewNativeParentBackend {
        self.backend
    }

    fn ensure_supported_on_current_target(&self) -> Result<(), BaseviewHostError> {
        let supported = matches!(
            self.backend,
            BaseviewNativeParentBackend::X11
                | BaseviewNativeParentBackend::Xcb
                | BaseviewNativeParentBackend::XWayland
        ) && cfg!(target_os = "linux")
            || self.backend == BaseviewNativeParentBackend::MacOs && cfg!(target_os = "macos")
            || self.backend == BaseviewNativeParentBackend::Windows && cfg!(target_os = "windows");
        if supported {
            Ok(())
        } else {
            Err(BaseviewHostError::new(
                "baseview.native-parent.target-mismatch",
                "baseview parent handle backend does not match the current target OS",
            ))
        }
    }
}

// SAFETY: Baseview 0.1 accepts `raw-window-handle` 0.5 parent values by contract.
// `BaseviewNativeParent` is constructed only after validating that every pointer/window ID
// required by the selected backend is non-zero and representable on this target.
//
// Lifetime caveat (inherent to embedded plugin hosting; NOT covered by the validation above):
// these handles are *borrowed* from the DAW-owned parent and stored as plain integers, so this
// `Copy` value cannot observe the parent being destroyed. `raw-window-handle`'s contract — that
// the handle is valid for the lifetime of the value — therefore holds only while the DAW keeps
// the parent alive. Upholding it is the plugin lifecycle's responsibility (attach before
// `open_parented`, no use after the host tears the editor down); validation gates value and
// representability, not liveness or provenance, so a handle used after DAW-side teardown is a
// use-after-free this crate cannot detect.
#[allow(unsafe_code)]
unsafe impl HasRawWindowHandle for BaseviewNativeParent {
    fn raw_window_handle(&self) -> RawWindowHandle {
        match self.handle {
            BaseviewNativeParentHandle::WindowsHwnd { hwnd } => {
                let mut handle = Win32WindowHandle::empty();
                handle.hwnd = handle_to_ptr(hwnd);
                RawWindowHandle::Win32(handle)
            }
            BaseviewNativeParentHandle::MacOsNsViewInWindow { ns_window, ns_view } => {
                let mut handle = AppKitWindowHandle::empty();
                handle.ns_window = handle_to_ptr(ns_window);
                handle.ns_view = handle_to_ptr(ns_view);
                RawWindowHandle::AppKit(handle)
            }
            BaseviewNativeParentHandle::LinuxX11 { window, .. }
            | BaseviewNativeParentHandle::LinuxXWayland { window, .. } => {
                let mut handle = XlibWindowHandle::empty();
                handle.window = window;
                RawWindowHandle::Xlib(handle)
            }
            BaseviewNativeParentHandle::LinuxXcb { window, .. } => {
                let mut handle = XcbWindowHandle::empty();
                #[allow(clippy::cast_possible_truncation)]
                {
                    handle.window = window as u32;
                }
                RawWindowHandle::Xcb(handle)
            }
        }
    }
}

// SAFETY: The display handle is derived from the same validated parent record as the window
// handle. The selected variants match Baseview's platform backend expectations. The same
// borrowed-handle lifetime caveat as `HasRawWindowHandle` applies: the display/connection pointer
// is reconstituted from an integer checked only for non-zero, pointer-width-representable value —
// not for liveness or provenance — so it is valid only while the DAW keeps the parent alive.
#[allow(unsafe_code)]
unsafe impl HasRawDisplayHandle for BaseviewNativeParent {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        match self.handle {
            BaseviewNativeParentHandle::WindowsHwnd { .. } => {
                RawDisplayHandle::Windows(WindowsDisplayHandle::empty())
            }
            BaseviewNativeParentHandle::MacOsNsViewInWindow { .. } => {
                RawDisplayHandle::AppKit(AppKitDisplayHandle::empty())
            }
            BaseviewNativeParentHandle::LinuxX11 { display, .. }
            | BaseviewNativeParentHandle::LinuxXWayland { display, .. } => {
                let mut handle = XlibDisplayHandle::empty();
                handle.display = handle_to_ptr(display);
                RawDisplayHandle::Xlib(handle)
            }
            BaseviewNativeParentHandle::LinuxXcb { connection, .. } => {
                let mut handle = XcbDisplayHandle::empty();
                handle.connection = handle_to_ptr(connection);
                RawDisplayHandle::Xcb(handle)
            }
        }
    }
}

/// Host events produced from one native Baseview event.
#[derive(Clone, Debug, PartialEq)]
pub struct BaseviewTranslatedEvent {
    /// Whether the native event was handled by `Hawk2UI` or should continue to the parent host.
    pub status: EventStatus,
    /// Plugin host events emitted by the translation.
    pub events: Vec<PluginHostEvent>,
}

impl BaseviewTranslatedEvent {
    fn captured(events: Vec<PluginHostEvent>) -> Self {
        Self {
            status: EventStatus::Captured,
            events,
        }
    }
}

/// Stateful translator from native Baseview events into `Hawk2UI` plugin host events.
#[derive(Clone, Debug, PartialEq)]
pub struct BaseviewEventTranslator {
    metrics: SurfaceMetrics,
    last_pointer_position: (f64, f64),
    destroyed: bool,
}

impl BaseviewEventTranslator {
    /// Creates a native event translator with the current editor metrics.
    #[must_use]
    pub const fn new(metrics: SurfaceMetrics) -> Self {
        Self {
            metrics,
            last_pointer_position: (0.0, 0.0),
            destroyed: false,
        }
    }

    /// Returns the latest metrics observed from native resize/DPI events.
    #[must_use]
    pub const fn metrics(&self) -> SurfaceMetrics {
        self.metrics
    }

    /// Translates a Baseview event into plugin host events.
    #[must_use]
    pub fn translate(&mut self, event: &baseview::Event) -> BaseviewTranslatedEvent {
        match event {
            baseview::Event::Window(event) => self.translate_window_event(event),
            baseview::Event::Keyboard(event) => {
                BaseviewTranslatedEvent::captured(vec![PluginHostEvent::KeyboardRouted(
                    KeyboardInput::new(keyboard_key_label(event), event.state == KeyState::Down),
                )])
            }
            baseview::Event::Mouse(event) => self.translate_mouse_event(event),
        }
    }

    fn translate_window_event(&mut self, event: &baseview::WindowEvent) -> BaseviewTranslatedEvent {
        match event {
            baseview::WindowEvent::Resized(info) => {
                let logical_size = info.logical_size();
                let metrics =
                    SurfaceMetrics::new(logical_size.width, logical_size.height, info.scale());
                if validate_baseview_metrics(metrics).is_err() {
                    return BaseviewTranslatedEvent::captured(Vec::new());
                }
                let scale_changed =
                    (self.metrics.scale_factor - metrics.scale_factor).abs() > f64::EPSILON;
                self.metrics = metrics;
                let mut events = vec![PluginHostEvent::HostResize(metrics)];
                if scale_changed {
                    events.push(PluginHostEvent::DpiChanged(metrics.scale_factor));
                }
                BaseviewTranslatedEvent::captured(events)
            }
            baseview::WindowEvent::Focused => {
                BaseviewTranslatedEvent::captured(vec![PluginHostEvent::FocusRouted(true)])
            }
            baseview::WindowEvent::Unfocused => {
                BaseviewTranslatedEvent::captured(vec![PluginHostEvent::FocusRouted(false)])
            }
            baseview::WindowEvent::WillClose => {
                if self.destroyed {
                    BaseviewTranslatedEvent::captured(Vec::new())
                } else {
                    self.destroyed = true;
                    BaseviewTranslatedEvent::captured(vec![
                        PluginHostEvent::EditorDestroyed("baseview child window closed".into()),
                        PluginHostEvent::SafeTeardownComplete,
                    ])
                }
            }
        }
    }

    fn translate_mouse_event(&mut self, event: &baseview::MouseEvent) -> BaseviewTranslatedEvent {
        let pointer = match event {
            baseview::MouseEvent::CursorMoved { position, .. } => {
                self.last_pointer_position = (position.x, position.y);
                PointerInput::new(position.x, position.y, "move")
            }
            baseview::MouseEvent::ButtonPressed { button, .. } => {
                let (x, y) = self.last_pointer_position;
                PointerInput::new(x, y, format!("{}-down", mouse_button_label(*button)))
            }
            baseview::MouseEvent::ButtonReleased { button, .. } => {
                let (x, y) = self.last_pointer_position;
                PointerInput::new(x, y, format!("{}-up", mouse_button_label(*button)))
            }
            baseview::MouseEvent::WheelScrolled { delta, .. } => {
                let (x, y) = self.last_pointer_position;
                PointerInput::new(x, y, scroll_delta_label(*delta))
            }
            baseview::MouseEvent::CursorEntered => {
                let (x, y) = self.last_pointer_position;
                PointerInput::new(x, y, "enter")
            }
            baseview::MouseEvent::CursorLeft => {
                let (x, y) = self.last_pointer_position;
                PointerInput::new(x, y, "leave")
            }
            baseview::MouseEvent::DragEntered { position, .. } => {
                self.last_pointer_position = (position.x, position.y);
                PointerInput::new(position.x, position.y, "drag-entered")
            }
            baseview::MouseEvent::DragMoved { position, .. } => {
                self.last_pointer_position = (position.x, position.y);
                PointerInput::new(position.x, position.y, "drag-moved")
            }
            baseview::MouseEvent::DragLeft => {
                let (x, y) = self.last_pointer_position;
                PointerInput::new(x, y, "drag-left")
            }
            baseview::MouseEvent::DragDropped { position, .. } => {
                self.last_pointer_position = (position.x, position.y);
                PointerInput::new(position.x, position.y, "drag-dropped")
            }
        };
        BaseviewTranslatedEvent::captured(vec![PluginHostEvent::PointerRouted(pointer)])
    }
}

fn keyboard_key_label(event: &KeyboardEvent) -> String {
    match &event.key {
        Key::Character(value) => value.clone(),
        key => format!("{key:?}"),
    }
}

fn mouse_button_label(button: baseview::MouseButton) -> &'static str {
    match button {
        baseview::MouseButton::Left => "left",
        baseview::MouseButton::Middle => "middle",
        baseview::MouseButton::Right => "right",
        baseview::MouseButton::Back => "back",
        baseview::MouseButton::Forward => "forward",
        baseview::MouseButton::Other(_) => "other",
    }
}

fn scroll_delta_label(delta: baseview::ScrollDelta) -> String {
    match delta {
        baseview::ScrollDelta::Lines { x, y } => {
            format!("wheel-lines:{}:{}", compact_f32(x), compact_f32(y))
        }
        baseview::ScrollDelta::Pixels { x, y } => {
            format!("wheel-pixels:{}:{}", compact_f32(x), compact_f32(y))
        }
    }
}

fn compact_f32(value: f32) -> String {
    if value.fract() == 0.0 {
        #[allow(clippy::cast_possible_truncation)]
        {
            (value as i32).to_string()
        }
    } else {
        value.to_string()
    }
}

/// Linux `X11`/`XWayland` Baseview handler that renders a runtime scene with Skia and presents it
/// into the native child window during frame callbacks.
#[cfg(target_os = "linux")]
pub struct BaseviewX11SkiaFrameHandler {
    scene: RuntimeSceneFrame,
    event_translator: BaseviewEventTranslator,
    event_sink: Arc<Mutex<Vec<PluginHostEvent>>>,
    presented_frames: Arc<AtomicU64>,
    last_error: Arc<Mutex<Option<BaseviewHostError>>>,
    close_after_first_frame: bool,
}

#[cfg(target_os = "linux")]
impl BaseviewX11SkiaFrameHandler {
    /// Creates a frame handler for an attached Baseview `X11`/`XWayland` child window.
    #[must_use]
    pub fn new(
        scene: RuntimeSceneFrame,
        metrics: SurfaceMetrics,
        presented_frames: Arc<AtomicU64>,
        last_error: Arc<Mutex<Option<BaseviewHostError>>>,
    ) -> Self {
        Self {
            scene,
            event_translator: BaseviewEventTranslator::new(metrics),
            event_sink: Arc::new(Mutex::new(Vec::new())),
            presented_frames,
            last_error,
            close_after_first_frame: false,
        }
    }

    /// Configures whether the handler closes the native child after the first presented frame.
    #[must_use]
    pub const fn close_after_first_frame(mut self, close_after_first_frame: bool) -> Self {
        self.close_after_first_frame = close_after_first_frame;
        self
    }

    /// Records translated native events into a caller-owned sink.
    #[must_use]
    pub fn with_event_sink(mut self, event_sink: Arc<Mutex<Vec<PluginHostEvent>>>) -> Self {
        self.event_sink = event_sink;
        self
    }

    fn record_error(&self, error: BaseviewHostError) {
        if let Ok(mut last_error) = self.last_error.lock() {
            *last_error = Some(error);
        }
    }

    fn record_events(&self, events: Vec<PluginHostEvent>) {
        if events.is_empty() {
            return;
        }
        if let Ok(mut event_sink) = self.event_sink.lock() {
            event_sink.extend(events);
        }
    }
}

#[cfg(target_os = "linux")]
impl WindowHandler for BaseviewX11SkiaFrameHandler {
    fn on_frame(&mut self, window: &mut Window) {
        let frame_index = self.presented_frames.load(Ordering::SeqCst);
        let metrics = self.event_translator.metrics();
        match render_scene_to_skia_snapshot(&self.scene, metrics, frame_index)
            .and_then(|snapshot| present_snapshot_to_x11_window(window, &snapshot))
        {
            Ok(()) => {
                self.presented_frames.fetch_add(1, Ordering::SeqCst);
                self.record_events(vec![PluginHostEvent::FramePresented {
                    frame_id: frame_index,
                    metrics,
                }]);
                if self.close_after_first_frame {
                    window.close();
                }
            }
            Err(error) => {
                self.record_error(error);
                window.close();
            }
        }
    }

    fn on_event(&mut self, _window: &mut Window, event: baseview::Event) -> EventStatus {
        let translated = self.event_translator.translate(&event);
        self.record_events(translated.events);
        translated.status
    }
}

/// `Send` owner of a live Baseview child window for an embedded plugin editor.
///
/// `baseview::WindowHandle` holds a raw native child-window pointer and is not
/// auto-`Send`, but the truce `Editor` trait that consumers of this crate
/// implement *is* `Send`. This wrapper carries the justified `unsafe impl Send`
/// so an embedder outside this crate — notably the `unsafe`-free
/// `hawk2ui-plugin-truce` editor binding — can own a live editor window without
/// writing `unsafe` itself. The window must still be driven (`is_open`,
/// `close`) only from the GUI thread that opened it.
pub struct BaseviewEditorWindowHandle {
    handle: WindowHandle,
}

// SAFETY: the wrapped `WindowHandle` is `!Send` because it carries a raw native
// child-window pointer (HWND / NSView / X11 Window). The embedded-editor
// lifecycle guarantees single-threaded access: the DAW host opens, polls
// (`idle`), and closes the editor on one dedicated GUI thread, never
// concurrently and never from the audio thread, so the handle is only ever
// touched on the thread that created it. This is the same single-GUI-thread
// invariant truce's own `GpuEditor` relies on for its `unsafe impl Send`, and
// the same lifetime contract `BaseviewNativeParent`'s raw-handle impls assume.
#[allow(unsafe_code)]
unsafe impl Send for BaseviewEditorWindowHandle {}

impl BaseviewEditorWindowHandle {
    /// Returns whether the native child window is still open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.handle.is_open()
    }

    /// Closes the native child window. Idempotent once the window is gone.
    pub fn close(&mut self) {
        self.handle.close();
    }
}

impl Drop for BaseviewEditorWindowHandle {
    fn drop(&mut self) {
        // Dropping a baseview `WindowHandle` does not by itself cancel the
        // platform frame timer, so a handle dropped without a prior explicit
        // `close()` would keep firing `on_frame` into a dead surface — the same
        // defect truce's own `GpuEditor` guards against in its `Drop`. `close()`
        // is idempotent, so this composes with an earlier explicit close.
        self.handle.close();
    }
}

/// Headless-safe Baseview plugin adapter.
#[derive(Clone, Debug, PartialEq)]
pub struct BaseviewPluginAdapter {
    config: PluginEditorConfig,
    parent_fixture: BaseviewParentFixture,
    capabilities: BaseviewCapabilities,
    open_options: WindowOpenOptions,
    destroyed: bool,
    visible: bool,
    events: Vec<PluginHostEvent>,
    repaint_reasons: Vec<String>,
    presented_frame_count: u64,
    last_presented_frame: Option<SkiaFrameSnapshot>,
}

impl BaseviewPluginAdapter {
    /// Attaches a plugin editor to a DAW-owned parent.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when the parent handle is incompatible with plugin embedding.
    pub fn attach(
        config: PluginEditorConfig,
        parent_fixture: BaseviewParentFixture,
    ) -> Result<Self, BaseviewHostError> {
        validate_baseview_parent(parent_fixture.handle())?;
        validate_baseview_metrics(config.metrics)?;
        parent_fixture
            .handle()
            .validate_for(SurfaceOwnership::PluginEditor)
            .map_err(|diagnostic| BaseviewHostError::new(diagnostic.code, diagnostic.message))?;
        let open_options = WindowOpenOptions {
            title: config.editor_id.clone(),
            size: Size::new(config.metrics.logical_width, config.metrics.logical_height),
            scale: WindowScalePolicy::ScaleFactor(config.metrics.scale_factor),
        };
        Ok(Self {
            events: vec![
                PluginHostEvent::ParentAttached(config.parent.clone()),
                PluginHostEvent::EditorCreated(config.editor_id.clone()),
            ],
            config,
            parent_fixture,
            capabilities: BaseviewCapabilities::plugin_editor(),
            open_options,
            destroyed: false,
            visible: true,
            repaint_reasons: Vec::new(),
            presented_frame_count: 0,
            last_presented_frame: None,
        })
    }

    /// Returns parent fixture.
    #[must_use]
    pub const fn parent_fixture(&self) -> BaseviewParentFixture {
        self.parent_fixture
    }

    /// Returns capabilities.
    #[must_use]
    pub const fn capabilities(&self) -> BaseviewCapabilities {
        self.capabilities
    }

    /// Returns whether the editor is destroyed.
    #[must_use]
    pub const fn destroyed(&self) -> bool {
        self.destroyed
    }

    /// Returns whether the editor is currently visible.
    #[must_use]
    pub const fn visible(&self) -> bool {
        self.visible
    }

    /// Returns repaint reasons.
    #[must_use]
    pub fn repaint_reasons(&self) -> &[String] {
        &self.repaint_reasons
    }

    /// Returns the number of runtime scene frames presented by the adapter.
    #[must_use]
    pub const fn presented_frame_count(&self) -> u64 {
        self.presented_frame_count
    }

    /// Returns the last presented Skia frame snapshot.
    #[must_use]
    pub const fn last_presented_frame(&self) -> Option<&SkiaFrameSnapshot> {
        self.last_presented_frame.as_ref()
    }

    /// Returns the Baseview open options used for native attachment.
    #[must_use]
    pub const fn open_options(&self) -> &WindowOpenOptions {
        &self.open_options
    }

    /// Returns the validated native parent for Baseview attachment.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when the recorded parent is not sufficient for native
    /// Baseview attachment.
    pub fn native_parent(&self) -> Result<BaseviewNativeParent, BaseviewHostError> {
        BaseviewNativeParent::try_from_handle(self.parent_fixture.handle())
    }

    /// Opens a real Baseview child window against the validated native parent.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when the editor is destroyed, the parent handle is invalid, or
    /// the parent handle backend does not match the current target OS.
    pub fn open_parented_window<H, B>(&self, build: B) -> Result<WindowHandle, BaseviewHostError>
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut Window) -> H + Send + 'static,
    {
        self.ensure_accepts_host_event()?;
        let native_parent = self.native_parent()?;
        native_parent.ensure_supported_on_current_target()?;
        Ok(Window::open_parented(
            &native_parent,
            self.open_options.clone(),
            build,
        ))
    }

    /// Drains host events.
    pub fn drain_events(&mut self) -> Vec<PluginHostEvent> {
        std::mem::take(&mut self.events)
    }

    /// Records a host-driven editor show request.
    pub fn show_editor(&mut self, reason: impl Into<String>) {
        if self.accepts_host_event() && !self.visible {
            self.visible = true;
            self.events
                .push(PluginHostEvent::EditorShown(reason.into()));
        }
    }

    /// Records a host-driven editor hide request.
    pub fn hide_editor(&mut self, reason: impl Into<String>) {
        if self.accepts_host_event() && self.visible {
            self.visible = false;
            self.events
                .push(PluginHostEvent::EditorHidden(reason.into()));
            self.events.push(PluginHostEvent::FocusRouted(false));
        }
    }

    /// Handles host resize events and reports invalid metrics.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when the editor is destroyed or the resize metrics are invalid.
    pub fn try_host_resize(&mut self, metrics: SurfaceMetrics) -> Result<(), BaseviewHostError> {
        self.ensure_accepts_host_event()?;
        validate_baseview_metrics(metrics)?;
        self.config.metrics = metrics;
        self.open_options.size = Size::new(metrics.logical_width, metrics.logical_height);
        self.events.push(PluginHostEvent::HostResize(metrics));
        Ok(())
    }

    /// Handles host DPI changes and reports invalid scale factors.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when the editor is destroyed or the resulting metrics are invalid.
    pub fn try_dpi_changed(&mut self, scale_factor: f64) -> Result<(), BaseviewHostError> {
        self.ensure_accepts_host_event()?;
        let metrics = SurfaceMetrics::new(
            self.config.metrics.logical_width,
            self.config.metrics.logical_height,
            scale_factor,
        );
        validate_baseview_metrics(metrics)?;
        self.config.metrics.scale_factor = scale_factor;
        self.open_options.scale = WindowScalePolicy::ScaleFactor(scale_factor);
        self.events.push(PluginHostEvent::DpiChanged(scale_factor));
        Ok(())
    }

    /// Renders a runtime scene frame into the plugin editor surface.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when the editor is destroyed, metrics are invalid, or Skia
    /// cannot present the frame.
    pub fn render_scene_frame(
        &mut self,
        scene: &RuntimeSceneFrame,
    ) -> Result<&SkiaFrameSnapshot, BaseviewHostError> {
        self.ensure_accepts_host_event()?;
        validate_baseview_metrics(self.config.metrics)?;
        let frame_index = self.presented_frame_count;
        let snapshot = render_scene_to_skia_snapshot(scene, self.config.metrics, frame_index)?;
        self.presented_frame_count = self.presented_frame_count.saturating_add(1);
        self.last_presented_frame = Some(snapshot);
        self.events.push(PluginHostEvent::RepaintScheduled(
            "runtime scene presented".into(),
        ));
        self.last_presented_frame.as_ref().ok_or_else(|| {
            BaseviewHostError::new(
                "baseview.render.snapshot-missing",
                "baseview render completed without a retained frame snapshot",
            )
        })
    }

    fn accepts_host_event(&self) -> bool {
        !self.destroyed
    }

    fn ensure_accepts_host_event(&self) -> Result<(), BaseviewHostError> {
        if self.accepts_host_event() {
            Ok(())
        } else {
            Err(BaseviewHostError::new(
                "baseview.editor.destroyed",
                "baseview editor has already been destroyed",
            ))
        }
    }
}

#[cfg(target_os = "linux")]
impl BaseviewPluginAdapter {
    /// Opens a real Baseview child window that renders `scene` each frame and
    /// returns a [`Send`] owner of its native handle.
    ///
    /// Linux/X11 software-presentation path: every frame is rendered to a CPU
    /// Skia snapshot and presented into the child window via X11 `PutImage`
    /// (see [`BaseviewX11SkiaFrameHandler`]). `presented_frames`, `last_error`,
    /// and `event_sink` are shared with the frame handler so the caller can
    /// observe rendering progress, surface errors, and drain host events while
    /// the window lives. The returned [`BaseviewEditorWindowHandle`] is `Send`
    /// so a truce `Editor` can own it, but must still be driven from the GUI
    /// thread that opened it.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when the editor is destroyed, the parent
    /// handle is invalid, or the parent handle backend does not match the
    /// current target OS.
    pub fn open_editor_window(
        &self,
        scene: RuntimeSceneFrame,
        presented_frames: Arc<AtomicU64>,
        last_error: Arc<Mutex<Option<BaseviewHostError>>>,
        event_sink: Arc<Mutex<Vec<PluginHostEvent>>>,
    ) -> Result<BaseviewEditorWindowHandle, BaseviewHostError> {
        let metrics = self.config.metrics;
        let handle = self.open_parented_window(move |_window| {
            BaseviewX11SkiaFrameHandler::new(scene, metrics, presented_frames, last_error)
                .with_event_sink(event_sink)
        })?;
        Ok(BaseviewEditorWindowHandle { handle })
    }
}

/// Live CLAP runtime editor attached through `Baseview` and rendered with Skia.
#[derive(Debug)]
pub struct BaseviewClapRuntimeEditor {
    session: ClapRuntimeEditorSession,
    adapter: BaseviewPluginAdapter,
}

impl BaseviewClapRuntimeEditor {
    /// Attaches a verified CLAP runtime editor session to a DAW-owned parent.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when the CLAP parent cannot be represented for Baseview or the
    /// Baseview adapter rejects the resulting editor configuration.
    pub fn attach(
        session: ClapRuntimeEditorSession,
        parent: ClapGuiParentHandle,
        linux_display_handle: Option<u64>,
        parent_fixture_id: &'static str,
    ) -> Result<Self, BaseviewHostError> {
        let host_config = session
            .baseview_host_config(parent, linux_display_handle)
            .map_err(|diagnostic| baseview_error_from_package_diagnostic(&diagnostic))?;
        let parent_fixture = BaseviewParentFixture::from_platform_handle(
            parent_fixture_id,
            host_config.host_parent(),
        );
        let adapter =
            BaseviewPluginAdapter::attach(host_config.editor_config().clone(), parent_fixture)?;
        Ok(Self { session, adapter })
    }

    /// Returns the verified CLAP runtime editor session.
    #[must_use]
    pub const fn session(&self) -> &ClapRuntimeEditorSession {
        &self.session
    }

    /// Returns the attached Baseview adapter.
    #[must_use]
    pub const fn adapter(&self) -> &BaseviewPluginAdapter {
        &self.adapter
    }

    /// Returns the current editor metrics.
    #[must_use]
    pub fn metrics(&self) -> SurfaceMetrics {
        self.adapter.metrics()
    }

    /// Returns whether the editor is currently visible.
    #[must_use]
    pub const fn visible(&self) -> bool {
        self.adapter.visible()
    }

    /// Returns whether the editor is destroyed.
    #[must_use]
    pub const fn destroyed(&self) -> bool {
        self.adapter.destroyed()
    }

    /// Returns the number of runtime scene frames presented by the live editor.
    #[must_use]
    pub const fn presented_frame_count(&self) -> u64 {
        self.adapter.presented_frame_count()
    }

    /// Presents the verified sealed runtime scene into the attached Baseview surface.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when the sealed runtime scene cannot be built or the Baseview
    /// surface cannot render the frame.
    pub fn present_runtime_frame(&mut self) -> Result<&SkiaFrameSnapshot, BaseviewHostError> {
        let frame = self
            .session
            .runtime_scene_frame()
            .map_err(|error| baseview_error_from_materialization_error(&error))?;
        self.adapter.render_scene_frame(&frame)
    }

    /// Handles a host-driven resize for the attached live editor.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when the editor is destroyed or metrics are invalid.
    pub fn try_host_resize(&mut self, metrics: SurfaceMetrics) -> Result<(), BaseviewHostError> {
        self.adapter.try_host_resize(metrics)
    }

    /// Records a host-driven show request.
    pub fn show_editor(&mut self, reason: impl Into<String>) {
        self.adapter.show_editor(reason);
    }

    /// Records a host-driven hide request.
    pub fn hide_editor(&mut self, reason: impl Into<String>) {
        self.adapter.hide_editor(reason);
    }

    /// Destroys the live editor safely.
    pub fn destroy_editor(&mut self, reason: impl Into<String>) {
        self.adapter.destroy_editor(reason);
    }

    /// Drains host events emitted by the attached editor.
    pub fn drain_events(&mut self) -> Vec<PluginHostEvent> {
        self.adapter.drain_events()
    }
}

/// Host-side CLAP GUI lifecycle bridge for a runtime-backed `Baseview` editor.
#[derive(Debug)]
pub struct BaseviewClapRuntimeEditorHost {
    plugin_path: PathBuf,
    linux_display_handle: Option<u64>,
    session: Option<ClapRuntimeEditorSession>,
    editor: Option<BaseviewClapRuntimeEditor>,
    created_api: Option<ClapGuiWindowApi>,
    parameter_values: BTreeMap<String, ParameterValue>,
    latest_realtime_packets: Vec<RealtimeVisualPacket>,
}

/// Typed command surface for driving a CLAP runtime editor host bridge from an embedding layer.
#[derive(Clone, Debug, PartialEq)]
pub enum BaseviewClapRuntimeEditorHostCommand {
    /// CLAP GUI create callback.
    Create {
        /// Requested CLAP GUI parent API.
        api: ClapGuiWindowApi,
        /// Whether the host requested a floating editor.
        is_floating: bool,
    },
    /// CLAP GUI set-parent callback.
    SetParent {
        /// Validated CLAP parent handle.
        parent: ClapGuiParentHandle,
        /// Stable parent fixture ID for diagnostics and tests.
        parent_fixture_id: &'static str,
    },
    /// CLAP GUI show callback.
    Show,
    /// CLAP GUI hide callback.
    Hide,
    /// CLAP GUI destroy callback.
    Destroy,
    /// Host parameter event.
    ApplyParameter {
        /// Stable parameter identifier.
        parameter_id: String,
        /// Typed parameter value.
        value: ParameterValue,
    },
    /// Host state save request.
    SaveState,
    /// Host state load request.
    LoadState(PluginStateEnvelope),
}

/// Typed response returned by [`BaseviewClapRuntimeEditorHostCommand`] dispatch.
#[derive(Clone, Debug, PartialEq)]
pub enum BaseviewClapRuntimeEditorHostResponse {
    /// Runtime editor session was created.
    Created,
    /// Runtime editor was attached to a parent.
    ParentAttached,
    /// Runtime frame was presented through Skia.
    FramePresented {
        /// Snapshot width in physical pixels.
        width: u32,
        /// Snapshot height in physical pixels.
        height: u32,
        /// Total frame count after presentation.
        presented_frame_count: u64,
    },
    /// Runtime editor was hidden.
    Hidden,
    /// Runtime editor was destroyed.
    Destroyed,
    /// Parameter event was accepted.
    ParameterApplied,
    /// State was saved.
    StateSaved(PluginStateEnvelope),
    /// State was loaded.
    StateLoaded,
    /// Realtime visual packets were drained through the UI frame gate.
    RealtimeVisualsDrained {
        /// Number of packets drained into the latest packet batch.
        packet_count: usize,
    },
}

/// Text ABI bridge matching the generated `hawk2ui_editor_dispatch` command vocabulary.
///
/// This lets a host-side CLAP integration drive the same command/response protocol used by the
/// generated C ABI trampoline while routing commands into the live `Baseview` editor host.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BaseviewClapRuntimeEditorHostAbiBridge;

impl BaseviewClapRuntimeEditorHostAbiBridge {
    /// Creates the host ABI bridge.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Returns the stable text ABI contract.
    #[must_use]
    pub const fn abi_contract(&self) -> &'static str {
        "hawk2ui_host_bridge_abi=1\ncommand=create\ncommand=set_parent\ncommand=show\ncommand=hide\ncommand=destroy\ncommand=apply_parameter\ncommand=save_state\ncommand=load_state\ncommand=drain_realtime_visuals\nresponse=created\nresponse=parent_attached\nresponse=frame_presented\nresponse=hidden\nresponse=destroyed\nresponse=parameter_applied\nresponse=state_saved\nresponse=state_loaded\nresponse=realtime_visuals_drained\nfunction=hawk2ui_editor_dispatch\n"
    }

    /// Dispatches one generated text ABI command into a live `Baseview` CLAP runtime editor host.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when the command is malformed or the live host rejects the
    /// lifecycle operation.
    pub fn dispatch_text(
        &self,
        host: &mut BaseviewClapRuntimeEditorHost,
        command: &str,
    ) -> Result<String, BaseviewHostError> {
        Self::dispatch_text_inner(host, command, None)
    }

    /// Dispatches one generated text ABI command with live realtime visual transport access.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when the command is malformed, the realtime frame gate rejects
    /// the drain, or the live host rejects the lifecycle operation.
    pub fn dispatch_text_with_realtime(
        &self,
        host: &mut BaseviewClapRuntimeEditorHost,
        command: &str,
        reader: &mut RealtimeVisualUiReader,
        frame_gate: &mut RealtimeVisualFrameGate,
    ) -> Result<String, BaseviewHostError> {
        Self::dispatch_text_inner(host, command, Some((reader, frame_gate)))
    }

    fn dispatch_text_inner(
        host: &mut BaseviewClapRuntimeEditorHost,
        command: &str,
        realtime: Option<(&mut RealtimeVisualUiReader, &mut RealtimeVisualFrameGate)>,
    ) -> Result<String, BaseviewHostError> {
        let fields = parse_host_abi_fields(command)?;
        let command_name = require_host_abi_field(&fields, "command")?;
        match command_name {
            "create" => {
                let api = parse_host_abi_api(require_host_abi_field(&fields, "api")?)?;
                let is_floating = parse_host_abi_bool(
                    fields.get("floating").map_or("false", String::as_str),
                    "floating",
                )?;
                let response = host
                    .dispatch(BaseviewClapRuntimeEditorHostCommand::Create { api, is_floating })?;
                Ok(response_to_host_abi_text(response))
            }
            "set_parent" => {
                let api = parse_host_abi_api(require_host_abi_field(&fields, "api")?)?;
                let raw_handle =
                    parse_host_abi_u64(require_host_abi_field(&fields, "parent")?, "parent")?;
                let parent = ClapGuiParentHandle::from_raw_parts(api, raw_handle)
                    .map_err(|error| BaseviewHostError::new(error.rule(), error.message()))?;
                let response = host.dispatch(BaseviewClapRuntimeEditorHostCommand::SetParent {
                    parent,
                    parent_fixture_id: parent_fixture_id_for_api(api),
                })?;
                Ok(response_to_host_abi_text(response))
            }
            "show" => Ok(response_to_host_abi_text(
                host.dispatch(BaseviewClapRuntimeEditorHostCommand::Show)?,
            )),
            "hide" => Ok(response_to_host_abi_text(
                host.dispatch(BaseviewClapRuntimeEditorHostCommand::Hide)?,
            )),
            "destroy" => Ok(response_to_host_abi_text(
                host.dispatch(BaseviewClapRuntimeEditorHostCommand::Destroy)?,
            )),
            "apply_parameter" => {
                let parameter_id = require_host_abi_field(&fields, "parameter_id")?.to_string();
                let value = parse_host_abi_f64(require_host_abi_field(&fields, "value")?, "value")?;
                let response =
                    host.dispatch(BaseviewClapRuntimeEditorHostCommand::ApplyParameter {
                        parameter_id,
                        value: ParameterValue::Float(value),
                    })?;
                Ok(response_to_host_abi_text(response))
            }
            "save_state" => Ok(response_to_host_abi_text(
                host.dispatch(BaseviewClapRuntimeEditorHostCommand::SaveState)?,
            )),
            "load_state" => {
                let mut state = PluginStateEnvelope::new(1);
                for (key, value) in &fields {
                    let Some(parameter_id) = key
                        .strip_prefix("param.")
                        .and_then(|rest| rest.strip_suffix(".bits"))
                    else {
                        continue;
                    };
                    let bits = parse_host_abi_u64(value, key)?;
                    state = state.parameter(
                        parameter_id.to_string(),
                        StateValue::Float(f64::from_bits(bits)),
                    );
                }
                Ok(response_to_host_abi_text(host.dispatch(
                    BaseviewClapRuntimeEditorHostCommand::LoadState(state),
                )?))
            }
            "drain_realtime_visuals" => {
                let Some((reader, frame_gate)) = realtime else {
                    return Err(BaseviewHostError::new(
                        "baseview.clap-host-abi.missing-realtime-channel",
                        "CLAP host ABI realtime drains require a live UI reader and frame gate",
                    ));
                };
                let timestamp_ms = fields
                    .get("timestamp_ms")
                    .map(|value| parse_host_abi_u64(value, "timestamp_ms"))
                    .transpose()?
                    .unwrap_or(0);
                Ok(response_to_host_abi_text(host.dispatch_realtime_visuals(
                    reader,
                    timestamp_ms,
                    frame_gate,
                )?))
            }
            other => Err(BaseviewHostError::new(
                "baseview.clap-host-abi.unknown-command",
                format!("unsupported CLAP host ABI command `{other}`"),
            )),
        }
    }
}

impl BaseviewClapRuntimeEditorHost {
    /// Creates a host lifecycle bridge for the CLAP plugin path received by the plugin host.
    #[must_use]
    pub fn new(plugin_path: impl Into<PathBuf>, linux_display_handle: Option<u64>) -> Self {
        Self {
            plugin_path: plugin_path.into(),
            linux_display_handle,
            session: None,
            editor: None,
            created_api: None,
            parameter_values: BTreeMap::new(),
            latest_realtime_packets: Vec::new(),
        }
    }

    /// Returns whether CLAP GUI create has resolved a verified runtime editor session.
    #[must_use]
    pub const fn created(&self) -> bool {
        self.created_api.is_some()
    }

    /// Returns whether a live `Baseview` runtime editor is attached to a parent.
    #[must_use]
    pub const fn attached(&self) -> bool {
        self.editor.is_some()
    }

    /// Returns whether the attached live editor is visible.
    #[must_use]
    pub fn visible(&self) -> bool {
        self.editor
            .as_ref()
            .is_some_and(BaseviewClapRuntimeEditor::visible)
    }

    /// Returns the number of runtime scene frames presented by the live editor.
    #[must_use]
    pub fn presented_frame_count(&self) -> u64 {
        self.editor
            .as_ref()
            .map_or(0, BaseviewClapRuntimeEditor::presented_frame_count)
    }

    /// Returns the current editor-side value for a host parameter.
    #[must_use]
    pub fn parameter_value(&self, parameter_id: &str) -> Option<&ParameterValue> {
        self.parameter_values.get(parameter_id)
    }

    /// Returns the latest realtime visual packet batch drained by the UI side.
    #[must_use]
    pub fn latest_realtime_visual_packets(&self) -> &[RealtimeVisualPacket] {
        &self.latest_realtime_packets
    }

    /// Dispatches a typed host bridge command into the live CLAP runtime editor bridge.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when the command violates lifecycle ordering or when the
    /// underlying live editor operation fails.
    pub fn dispatch(
        &mut self,
        command: BaseviewClapRuntimeEditorHostCommand,
    ) -> Result<BaseviewClapRuntimeEditorHostResponse, BaseviewHostError> {
        match command {
            BaseviewClapRuntimeEditorHostCommand::Create { api, is_floating } => {
                self.create(api, is_floating)?;
                Ok(BaseviewClapRuntimeEditorHostResponse::Created)
            }
            BaseviewClapRuntimeEditorHostCommand::SetParent {
                parent,
                parent_fixture_id,
            } => {
                self.set_parent(parent, parent_fixture_id)?;
                Ok(BaseviewClapRuntimeEditorHostResponse::ParentAttached)
            }
            BaseviewClapRuntimeEditorHostCommand::Show => {
                let (width, height) = {
                    let snapshot = self.show()?;
                    (snapshot.width(), snapshot.height())
                };
                Ok(BaseviewClapRuntimeEditorHostResponse::FramePresented {
                    width,
                    height,
                    presented_frame_count: self.presented_frame_count(),
                })
            }
            BaseviewClapRuntimeEditorHostCommand::Hide => {
                self.hide()?;
                Ok(BaseviewClapRuntimeEditorHostResponse::Hidden)
            }
            BaseviewClapRuntimeEditorHostCommand::Destroy => {
                self.destroy()?;
                Ok(BaseviewClapRuntimeEditorHostResponse::Destroyed)
            }
            BaseviewClapRuntimeEditorHostCommand::ApplyParameter {
                parameter_id,
                value,
            } => {
                self.apply_parameter_value(parameter_id, value)?;
                Ok(BaseviewClapRuntimeEditorHostResponse::ParameterApplied)
            }
            BaseviewClapRuntimeEditorHostCommand::SaveState => Ok(
                BaseviewClapRuntimeEditorHostResponse::StateSaved(self.save_state()?),
            ),
            BaseviewClapRuntimeEditorHostCommand::LoadState(state) => {
                self.load_state(state)?;
                Ok(BaseviewClapRuntimeEditorHostResponse::StateLoaded)
            }
        }
    }

    /// Dispatches a realtime visual drain through the typed host bridge response surface.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when the live editor is not attached.
    pub fn dispatch_realtime_visuals(
        &mut self,
        reader: &mut RealtimeVisualUiReader,
        timestamp_ms: u64,
        frame_gate: &mut RealtimeVisualFrameGate,
    ) -> Result<BaseviewClapRuntimeEditorHostResponse, BaseviewHostError> {
        let packet_count = self.drain_realtime_visuals(reader, timestamp_ms, frame_gate)?;
        Ok(BaseviewClapRuntimeEditorHostResponse::RealtimeVisualsDrained { packet_count })
    }

    /// Handles the CLAP GUI create callback by loading and verifying the runtime editor package.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when the callback requests a floating editor, an unsupported
    /// parent API, or the plugin path does not resolve to a verified runtime editor package.
    pub fn create(
        &mut self,
        api: ClapGuiWindowApi,
        is_floating: bool,
    ) -> Result<(), BaseviewHostError> {
        if is_floating {
            return Err(BaseviewHostError::new(
                "baseview.clap-runtime-editor.floating-unsupported",
                "Baseview CLAP runtime editors must be embedded in a host parent",
            ));
        }
        if api == ClapGuiWindowApi::Wayland {
            return Err(BaseviewHostError::new(
                "baseview.clap-runtime-editor.unsupported-api",
                "Baseview CLAP runtime editors do not support native Wayland parent handles",
            ));
        }
        let session = ClapRuntimeEditorSession::load_from_clap_plugin_path(&self.plugin_path)
            .map_err(|error| baseview_error_from_materialization_error(&error))?;
        self.session = Some(session);
        self.editor = None;
        self.created_api = Some(api);
        self.parameter_values.clear();
        self.latest_realtime_packets.clear();
        Ok(())
    }

    /// Handles the CLAP GUI set-parent callback by attaching the live Baseview runtime editor.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when create has not succeeded, the parent API does not match the
    /// create API, or Baseview rejects the parent.
    pub fn set_parent(
        &mut self,
        parent: ClapGuiParentHandle,
        parent_fixture_id: &'static str,
    ) -> Result<(), BaseviewHostError> {
        let created_api = self.created_api.ok_or_else(not_created_error)?;
        if parent.api() != created_api {
            return Err(BaseviewHostError::new(
                "baseview.clap-runtime-editor.parent-api-mismatch",
                "CLAP runtime editor parent API must match the API used during create",
            ));
        }
        let session = self.session.clone().ok_or_else(not_created_error)?;
        let editor = BaseviewClapRuntimeEditor::attach(
            session,
            parent,
            self.linux_display_handle,
            parent_fixture_id,
        )?;
        self.editor = Some(editor);
        Ok(())
    }

    /// Handles the CLAP GUI show callback by presenting the current verified runtime frame.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when the editor is not attached or rendering fails.
    pub fn show(&mut self) -> Result<&SkiaFrameSnapshot, BaseviewHostError> {
        self.editor
            .as_mut()
            .ok_or_else(not_attached_error)?
            .show_editor("clap gui show");
        self.editor
            .as_mut()
            .ok_or_else(not_attached_error)?
            .present_runtime_frame()
    }

    /// Handles the CLAP GUI hide callback.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when the editor is not attached.
    pub fn hide(&mut self) -> Result<(), BaseviewHostError> {
        self.editor
            .as_mut()
            .ok_or_else(not_attached_error)?
            .hide_editor("clap gui hide");
        Ok(())
    }

    /// Handles the CLAP GUI destroy callback and clears the current live editor.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when the editor has not been created.
    pub fn destroy(&mut self) -> Result<(), BaseviewHostError> {
        if self.created_api.is_none() {
            return Err(not_created_error());
        }
        if let Some(editor) = self.editor.as_mut() {
            editor.destroy_editor("clap gui destroy");
        }
        self.editor = None;
        self.session = None;
        self.created_api = None;
        self.parameter_values.clear();
        self.latest_realtime_packets.clear();
        Ok(())
    }

    /// Applies a host parameter event to the live editor-side state cache.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when create has not succeeded, the parameter ID is invalid, or
    /// the value is non-finite.
    pub fn apply_parameter_value(
        &mut self,
        parameter_id: impl Into<String>,
        value: ParameterValue,
    ) -> Result<(), BaseviewHostError> {
        self.ensure_created()?;
        let parameter_id = parameter_id.into();
        validate_parameter_event(&parameter_id, &value)?;
        self.parameter_values.insert(parameter_id, value);
        if let Some(editor) = self.editor.as_mut() {
            editor.show_editor("clap parameter changed");
        }
        Ok(())
    }

    /// Saves the current editor-side parameter state.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when create has not succeeded.
    pub fn save_state(&self) -> Result<PluginStateEnvelope, BaseviewHostError> {
        self.ensure_created()?;
        let mut state = PluginStateEnvelope::new(1);
        for (parameter_id, value) in &self.parameter_values {
            state = state.parameter(parameter_id.clone(), state_value_from_parameter(value));
        }
        Ok(state)
    }

    /// Loads host-provided state into the editor-side parameter cache.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when create has not succeeded or the state contains unsupported
    /// parameter values.
    pub fn load_state(&mut self, state: PluginStateEnvelope) -> Result<(), BaseviewHostError> {
        self.ensure_created()?;
        let mut values = BTreeMap::new();
        for (parameter_id, value) in state.parameter_state {
            let parameter_value = parameter_value_from_state(&value)?;
            validate_parameter_event(&parameter_id, &parameter_value)?;
            values.insert(parameter_id, parameter_value);
        }
        self.parameter_values = values;
        if let Some(editor) = self.editor.as_mut() {
            editor.show_editor("clap state loaded");
        }
        Ok(())
    }

    /// Drains realtime visual packets into the live editor bridge when the UI frame gate is due.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when the live editor is not attached.
    pub fn drain_realtime_visuals(
        &mut self,
        reader: &mut RealtimeVisualUiReader,
        timestamp_ms: u64,
        frame_gate: &mut RealtimeVisualFrameGate,
    ) -> Result<usize, BaseviewHostError> {
        let editor = self.editor.as_mut().ok_or_else(not_attached_error)?;
        let Some(packets) = reader.ui_drain_due(timestamp_ms, frame_gate) else {
            return Ok(0);
        };
        let packet_count = packets.len();
        if packet_count > 0 {
            editor.show_editor("clap realtime visuals drained");
            self.latest_realtime_packets = packets;
        }
        Ok(packet_count)
    }

    fn ensure_created(&self) -> Result<(), BaseviewHostError> {
        if self.created_api.is_some() {
            Ok(())
        } else {
            Err(not_created_error())
        }
    }
}

fn parse_host_abi_fields(command: &str) -> Result<BTreeMap<String, String>, BaseviewHostError> {
    let mut fields = BTreeMap::new();
    for line in command.lines().filter(|line| !line.trim().is_empty()) {
        let Some((key, value)) = line.split_once('=') else {
            return Err(BaseviewHostError::new(
                "baseview.clap-host-abi.malformed-line",
                format!("CLAP host ABI command line `{line}` is missing `=`"),
            ));
        };
        if key.trim().is_empty() {
            return Err(BaseviewHostError::new(
                "baseview.clap-host-abi.malformed-key",
                "CLAP host ABI command keys must not be empty",
            ));
        }
        fields.insert(key.trim().to_string(), value.trim().to_string());
    }
    if fields.is_empty() {
        return Err(BaseviewHostError::new(
            "baseview.clap-host-abi.empty-command",
            "CLAP host ABI command must contain key/value fields",
        ));
    }
    Ok(fields)
}

fn require_host_abi_field<'a>(
    fields: &'a BTreeMap<String, String>,
    key: &str,
) -> Result<&'a str, BaseviewHostError> {
    fields.get(key).map(String::as_str).ok_or_else(|| {
        BaseviewHostError::new(
            "baseview.clap-host-abi.missing-field",
            format!("CLAP host ABI command missing required field `{key}`"),
        )
    })
}

fn parse_host_abi_api(value: &str) -> Result<ClapGuiWindowApi, BaseviewHostError> {
    match value {
        "win32" => Ok(ClapGuiWindowApi::Win32),
        "cocoa" => Ok(ClapGuiWindowApi::Cocoa),
        "x11" => Ok(ClapGuiWindowApi::X11),
        "wayland" => Ok(ClapGuiWindowApi::Wayland),
        other => Err(BaseviewHostError::new(
            "baseview.clap-host-abi.invalid-api",
            format!("unsupported CLAP host ABI window API `{other}`"),
        )),
    }
}

fn parse_host_abi_bool(value: &str, field: &str) -> Result<bool, BaseviewHostError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(BaseviewHostError::new(
            "baseview.clap-host-abi.invalid-bool",
            format!("CLAP host ABI field `{field}` has invalid bool value `{other}`"),
        )),
    }
}

fn parse_host_abi_u64(value: &str, field: &str) -> Result<u64, BaseviewHostError> {
    value.parse::<u64>().map_err(|error| {
        BaseviewHostError::new(
            "baseview.clap-host-abi.invalid-integer",
            format!("CLAP host ABI field `{field}` has invalid integer value `{value}`: {error}"),
        )
    })
}

fn parse_host_abi_f64(value: &str, field: &str) -> Result<f64, BaseviewHostError> {
    let value = value.parse::<f64>().map_err(|error| {
        BaseviewHostError::new(
            "baseview.clap-host-abi.invalid-float",
            format!("CLAP host ABI field `{field}` has invalid float value `{value}`: {error}"),
        )
    })?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(BaseviewHostError::new(
            "baseview.clap-host-abi.invalid-float",
            format!("CLAP host ABI field `{field}` must be finite"),
        ))
    }
}

const fn parent_fixture_id_for_api(api: ClapGuiWindowApi) -> &'static str {
    match api {
        ClapGuiWindowApi::Win32 => "abi-win32-parent",
        ClapGuiWindowApi::Cocoa => "abi-cocoa-parent",
        ClapGuiWindowApi::X11 => "abi-x11-parent",
        ClapGuiWindowApi::Wayland => "abi-wayland-parent",
    }
}

fn response_to_host_abi_text(response: BaseviewClapRuntimeEditorHostResponse) -> String {
    match response {
        BaseviewClapRuntimeEditorHostResponse::Created => "response=created\n".to_string(),
        BaseviewClapRuntimeEditorHostResponse::ParentAttached => {
            "response=parent_attached\n".to_string()
        }
        BaseviewClapRuntimeEditorHostResponse::FramePresented {
            width,
            height,
            presented_frame_count,
        } => format!(
            "response=frame_presented\nwidth={width}\nheight={height}\npresented_frame_count={presented_frame_count}\n"
        ),
        BaseviewClapRuntimeEditorHostResponse::Hidden => "response=hidden\n".to_string(),
        BaseviewClapRuntimeEditorHostResponse::Destroyed => "response=destroyed\n".to_string(),
        BaseviewClapRuntimeEditorHostResponse::ParameterApplied => {
            "response=parameter_applied\n".to_string()
        }
        BaseviewClapRuntimeEditorHostResponse::StateSaved(state) => {
            let mut response = "response=state_saved\n".to_string();
            for (parameter_id, value) in state.parameter_state {
                if let StateValue::Float(value) = value {
                    let _ = writeln!(response, "param.{parameter_id}.bits={}", value.to_bits());
                }
            }
            response
        }
        BaseviewClapRuntimeEditorHostResponse::StateLoaded => "response=state_loaded\n".to_string(),
        BaseviewClapRuntimeEditorHostResponse::RealtimeVisualsDrained { packet_count } => {
            format!("response=realtime_visuals_drained\npacket_count={packet_count}\n")
        }
    }
}

fn not_created_error() -> BaseviewHostError {
    BaseviewHostError::new(
        "baseview.clap-runtime-editor.not-created",
        "CLAP runtime editor create must succeed before this host callback",
    )
}

fn not_attached_error() -> BaseviewHostError {
    BaseviewHostError::new(
        "baseview.clap-runtime-editor.not-attached",
        "CLAP runtime editor parent must be attached before this host callback",
    )
}

fn validate_parameter_event(
    parameter_id: &str,
    value: &ParameterValue,
) -> Result<(), BaseviewHostError> {
    if parameter_id.trim().is_empty() {
        return Err(BaseviewHostError::new(
            "baseview.clap-runtime-editor.parameter-invalid",
            "CLAP runtime editor parameter events require a non-empty parameter ID",
        ));
    }
    match value {
        ParameterValue::Float(value) if !value.is_finite() => Err(BaseviewHostError::new(
            "baseview.clap-runtime-editor.parameter-invalid",
            "CLAP runtime editor parameter float values must be finite",
        )),
        ParameterValue::Float(_) | ParameterValue::Bool(_) | ParameterValue::Choice(_) => Ok(()),
    }
}

fn state_value_from_parameter(value: &ParameterValue) -> StateValue {
    match value {
        ParameterValue::Float(value) => StateValue::Float(*value),
        ParameterValue::Bool(value) => StateValue::Bool(*value),
        ParameterValue::Choice(value) => StateValue::Float(f64::from(*value)),
    }
}

fn parameter_value_from_state(value: &StateValue) -> Result<ParameterValue, BaseviewHostError> {
    match value {
        StateValue::Float(value) if value.is_finite() => Ok(ParameterValue::Float(*value)),
        StateValue::Float(_) => Err(BaseviewHostError::new(
            "baseview.clap-runtime-editor.parameter-invalid",
            "CLAP runtime editor state float values must be finite",
        )),
        StateValue::Bool(value) => Ok(ParameterValue::Bool(*value)),
        StateValue::String(_) => Err(BaseviewHostError::new(
            "baseview.clap-runtime-editor.parameter-invalid",
            "CLAP runtime editor parameter state does not accept string values",
        )),
    }
}

fn baseview_error_from_package_diagnostic(diagnostic: &PackageDiagnostic) -> BaseviewHostError {
    BaseviewHostError::new(diagnostic.rule(), diagnostic.message())
}

fn baseview_error_from_materialization_error(
    error: &PackageMaterializationError,
) -> BaseviewHostError {
    let diagnostic = error.diagnostic();
    BaseviewHostError::new(diagnostic.rule(), diagnostic.message())
}

fn validate_baseview_parent(handle: HostPlatformHandle) -> Result<(), BaseviewHostError> {
    match handle {
        HostPlatformHandle::LinuxWayland { .. } => Err(BaseviewHostError::new(
            "baseview.platform.unsupported",
            "baseview 0.1 Linux backend attaches through X11/XCB/XWayland parent handles, not native Wayland surfaces",
        )),
        HostPlatformHandle::WindowsHwnd { .. }
        | HostPlatformHandle::MacOsNsView { .. }
        | HostPlatformHandle::MacOsNsViewInWindow { .. }
        | HostPlatformHandle::MacOsNsWindow { .. }
        | HostPlatformHandle::LinuxX11 { .. }
        | HostPlatformHandle::LinuxXcb { .. }
        | HostPlatformHandle::LinuxXWayland { .. } => Ok(()),
    }
}

fn validate_baseview_metrics(metrics: SurfaceMetrics) -> Result<(), BaseviewHostError> {
    if metrics.logical_width.is_finite()
        && metrics.logical_height.is_finite()
        && metrics.scale_factor.is_finite()
        && metrics.logical_width > 0.0
        && metrics.logical_height > 0.0
        && metrics.scale_factor > 0.0
    {
        Ok(())
    } else {
        Err(BaseviewHostError::new(
            "baseview.metrics.invalid",
            "baseview editor metrics must be finite and greater than zero",
        ))
    }
}

fn scale_factor_to_f32(scale_factor: f64) -> Result<f32, BaseviewHostError> {
    if !scale_factor.is_finite() || scale_factor <= 0.0 {
        return Err(BaseviewHostError::new(
            "baseview.metrics.invalid",
            "baseview editor metrics must be finite and greater than zero",
        ));
    }
    #[allow(clippy::cast_possible_truncation)]
    let converted = scale_factor as f32;
    if converted.is_finite() && converted > 0.0 {
        Ok(converted)
    } else {
        Err(BaseviewHostError::new(
            "baseview.metrics.invalid",
            "baseview editor metrics must be representable as a positive f32 scale",
        ))
    }
}

fn map_backend_error(error: &hawk2ui_render::BackendError) -> BaseviewHostError {
    BaseviewHostError::new(
        format!("baseview.render.{}", error.diagnostic().rule()),
        error.diagnostic().message(),
    )
}

fn render_scene_to_skia_snapshot(
    scene: &RuntimeSceneFrame,
    metrics: SurfaceMetrics,
    frame_index: u64,
) -> Result<SkiaFrameSnapshot, BaseviewHostError> {
    let (width, height) = metrics.physical_size();
    let dpi_scale = scale_factor_to_f32(metrics.scale_factor)?;
    let mut backend = SkiaRendererBackend::new();
    backend
        .create_surface_with_config(SkiaSurfaceConfig::cpu_raster(
            "baseview-editor",
            width,
            height,
        ))
        .map_err(|error| map_backend_error(&error))?;
    backend
        .begin_frame("baseview-editor")
        .map_err(|error| map_backend_error(&error))?;
    backend
        .clear(Color::rgba(0, 0, 0, 0))
        .map_err(|error| map_backend_error(&error))?;
    backend
        .draw_runtime_scene_frame(scene, frame_index, dpi_scale)
        .map_err(|error| map_backend_error(&error))?;
    backend
        .end_frame("baseview-editor")
        .map_err(|error| map_backend_error(&error))?;
    backend
        .frame_snapshot("baseview-editor")
        .map_err(|error| map_backend_error(&error))
        .cloned()
}

#[cfg(target_os = "linux")]
fn present_snapshot_to_x11_window(
    window: &Window,
    snapshot: &SkiaFrameSnapshot,
) -> Result<(), BaseviewHostError> {
    let drawable = x11_drawable_from_window(window)?;
    let width = u16::try_from(snapshot.width()).map_err(|_| {
        BaseviewHostError::new(
            "baseview.x11-present.invalid-size",
            "baseview X11 presentation width must fit u16",
        )
    })?;
    let height = u16::try_from(snapshot.height()).map_err(|_| {
        BaseviewHostError::new(
            "baseview.x11-present.invalid-size",
            "baseview X11 presentation height must fit u16",
        )
    })?;
    let (connection, _screen_number) = x11rb::connect(None).map_err(|error| {
        BaseviewHostError::new(
            "baseview.x11-present.connect-failed",
            format!("failed to connect to X11 display for Baseview presentation: {error}"),
        )
    })?;
    // `PutImage` requires `depth` to equal the *drawable's* depth. Baseview creates the child
    // window at depth 32 whenever the screen exposes a 32-bit TrueColor visual, so the root
    // window's depth (commonly 24) is the wrong value and the server rejects the request with
    // `BadMatch`. Query the child drawable's actual depth instead of assuming the root's.
    let depth = connection
        .get_geometry(drawable)
        .map_err(|error| {
            BaseviewHostError::new(
                "baseview.x11-present.geometry-failed",
                format!("failed to request Baseview X11 child window geometry: {error}"),
            )
        })?
        .reply()
        .map_err(|error| {
            BaseviewHostError::new(
                "baseview.x11-present.geometry-failed",
                format!("failed to read Baseview X11 child window geometry: {error}"),
            )
        })?
        .depth;
    let gc = create_x11_gc(&connection, drawable)?;
    let data = snapshot_to_x11_bgrx(snapshot);
    connection
        .put_image(
            ImageFormat::Z_PIXMAP,
            drawable,
            gc,
            width,
            height,
            0,
            0,
            0,
            depth,
            &data,
        )
        .map_err(|error| {
            BaseviewHostError::new(
                "baseview.x11-present.put-image-failed",
                format!("failed to put Baseview frame pixels into X11 child window: {error}"),
            )
        })?;
    connection.free_gc(gc).map_err(|error| {
        BaseviewHostError::new(
            "baseview.x11-present.free-gc-failed",
            format!("failed to release Baseview X11 presentation graphics context: {error}"),
        )
    })?;
    connection.flush().map_err(|error| {
        BaseviewHostError::new(
            "baseview.x11-present.flush-failed",
            format!("failed to flush Baseview X11 presentation commands: {error}"),
        )
    })
}

#[cfg(target_os = "linux")]
fn x11_drawable_from_window(window: &Window) -> Result<u32, BaseviewHostError> {
    match window.raw_window_handle() {
        RawWindowHandle::Xlib(handle) => u32::try_from(handle.window).map_err(|_| {
            BaseviewHostError::new(
                "baseview.x11-present.invalid-window",
                "baseview Xlib child window handle must fit X11 window id",
            )
        }),
        RawWindowHandle::Xcb(handle) => Ok(handle.window),
        _ => Err(BaseviewHostError::new(
            "baseview.x11-present.unsupported-window",
            "baseview X11 software presentation requires an Xlib or XCB child window",
        )),
    }
}

#[cfg(target_os = "linux")]
fn create_x11_gc(connection: &RustConnection, drawable: u32) -> Result<u32, BaseviewHostError> {
    let gc = connection.generate_id().map_err(|error| {
        BaseviewHostError::new(
            "baseview.x11-present.gc-id-failed",
            format!("failed to allocate Baseview X11 graphics context id: {error}"),
        )
    })?;
    connection
        .create_gc(gc, drawable, &CreateGCAux::new())
        .map_err(|error| {
            BaseviewHostError::new(
                "baseview.x11-present.create-gc-failed",
                format!("failed to create Baseview X11 graphics context: {error}"),
            )
        })?;
    Ok(gc)
}

#[cfg(target_os = "linux")]
fn snapshot_to_x11_bgrx(snapshot: &SkiaFrameSnapshot) -> Vec<u8> {
    pixels_to_x11_bgrx(snapshot.pixels())
}

/// Converts `0x00RRGGBB` snapshot pixels into X11 `Z_PIXMAP` 32-bpp bytes laid out `[B, G, R, 0]`.
#[cfg(target_os = "linux")]
fn pixels_to_x11_bgrx(pixels: &[u32]) -> Vec<u8> {
    let mut data = Vec::with_capacity(pixels.len().saturating_mul(4));
    for pixel in pixels {
        data.push((pixel & 0x0000_00ff) as u8);
        data.push(((pixel & 0x0000_ff00) >> 8) as u8);
        data.push(((pixel & 0x00ff_0000) >> 16) as u8);
        data.push(0);
    }
    data
}

fn require_nonzero_handle(handle: u64) -> Result<(), BaseviewHostError> {
    if usize::try_from(handle).ok().is_some_and(|value| value != 0) {
        Ok(())
    } else {
        Err(BaseviewHostError::new(
            "baseview.native-parent.invalid",
            "baseview native parent handles must be non-zero and representable as pointer-sized values",
        ))
    }
}

fn require_xcb_window(handle: u64) -> Result<(), BaseviewHostError> {
    if u32::try_from(handle).ok().is_some_and(|value| value != 0) {
        Ok(())
    } else {
        Err(BaseviewHostError::new(
            "baseview.native-parent.invalid",
            "baseview XCB window handles must be non-zero and fit xcb_window_t",
        ))
    }
}

fn handle_to_ptr(handle: u64) -> *mut c_void {
    #[allow(clippy::cast_possible_truncation)]
    let value = handle as usize;
    value as *mut c_void
}

impl PluginHostAdapter for BaseviewPluginAdapter {
    fn metrics(&self) -> SurfaceMetrics {
        self.config.metrics
    }

    fn host_resize(&mut self, metrics: SurfaceMetrics) {
        let _ = self.try_host_resize(metrics);
    }

    fn dpi_changed(&mut self, scale_factor: f64) {
        let _ = self.try_dpi_changed(scale_factor);
    }

    fn schedule_repaint(&mut self, reason: impl Into<String>) {
        if !self.accepts_host_event() {
            return;
        }
        let reason = reason.into();
        self.repaint_reasons.push(reason.clone());
        self.events.push(PluginHostEvent::RepaintScheduled(reason));
    }

    fn route_focus(&mut self, focused: bool) {
        if !self.accepts_host_event() {
            return;
        }
        self.events.push(PluginHostEvent::FocusRouted(focused));
    }

    fn route_keyboard(&mut self, input: KeyboardInput) {
        if !self.accepts_host_event() {
            return;
        }
        self.events.push(PluginHostEvent::KeyboardRouted(input));
    }

    fn route_pointer(&mut self, input: PointerInput) {
        if !self.accepts_host_event() {
            return;
        }
        self.events.push(PluginHostEvent::PointerRouted(input));
    }

    fn destroy_editor(&mut self, reason: impl Into<String>) {
        if !self.destroyed {
            self.destroyed = true;
            self.visible = false;
            self.events
                .push(PluginHostEvent::EditorDestroyed(reason.into()));
            self.events.push(PluginHostEvent::SafeTeardownComplete);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-host-baseview");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn pixels_convert_to_x11_bgrx_byte_layout() {
        // A `0x00RRGGBB` snapshot pixel must become `[B, G, R, 0]` for an X11 `Z_PIXMAP` put_image.
        assert_eq!(
            pixels_to_x11_bgrx(&[0x0012_3456, 0x00ff_8800]),
            vec![0x56, 0x34, 0x12, 0x00, 0x00, 0x88, 0xff, 0x00]
        );
    }
}
