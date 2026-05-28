#![deny(unsafe_code)]
//! `Baseview`-backed embedded plugin host adapter for `Hawk2UI`.

use baseview::{
    EventStatus, Size, Window, WindowHandle, WindowHandler, WindowOpenOptions, WindowScalePolicy,
};
use hawk2ui_host::{
    HostPlatformHandle, KeyboardInput, PluginEditorConfig, PluginHostAdapter, PluginHostEvent,
    PointerInput, SurfaceMetrics, SurfaceOwnership,
};
use hawk2ui_plugin_adapters::{
    ClapGuiParentHandle, ClapRuntimeEditorSession, PackageDiagnostic, PackageMaterializationError,
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
use std::ffi::c_void;
#[cfg(target_os = "linux")]
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
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
    handle: HostPlatformHandle,
    backend: BaseviewNativeParentBackend,
}

impl BaseviewNativeParent {
    /// Creates a Baseview-native parent from a host platform handle.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when Baseview cannot safely attach to the handle.
    pub fn try_from_handle(handle: HostPlatformHandle) -> Result<Self, BaseviewHostError> {
        validate_baseview_parent(handle)?;
        let backend = match handle {
            HostPlatformHandle::WindowsHwnd { hwnd } => {
                require_nonzero_handle(hwnd)?;
                BaseviewNativeParentBackend::Windows
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
                BaseviewNativeParentBackend::MacOs
            }
            HostPlatformHandle::MacOsNsWindow { .. } => {
                return Err(BaseviewHostError::new(
                    "baseview.native-parent.invalid",
                    "baseview plugin editors must attach to a child NSView, not a top-level NSWindow",
                ));
            }
            HostPlatformHandle::LinuxWayland { .. } => unreachable!("validated above"),
            HostPlatformHandle::LinuxX11 { display, window } => {
                require_nonzero_handle(display)?;
                require_nonzero_handle(window)?;
                BaseviewNativeParentBackend::X11
            }
            HostPlatformHandle::LinuxXcb { connection, window } => {
                require_nonzero_handle(connection)?;
                require_xcb_window(window)?;
                BaseviewNativeParentBackend::Xcb
            }
            HostPlatformHandle::LinuxXWayland { display, window } => {
                require_nonzero_handle(display)?;
                require_nonzero_handle(window)?;
                BaseviewNativeParentBackend::XWayland
            }
        };
        Ok(Self { handle, backend })
    }

    /// Returns the original host handle.
    #[must_use]
    pub const fn handle(&self) -> HostPlatformHandle {
        self.handle
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
#[allow(unsafe_code)]
unsafe impl HasRawWindowHandle for BaseviewNativeParent {
    fn raw_window_handle(&self) -> RawWindowHandle {
        match self.handle {
            HostPlatformHandle::WindowsHwnd { hwnd } => {
                let mut handle = Win32WindowHandle::empty();
                handle.hwnd = handle_to_ptr(hwnd);
                RawWindowHandle::Win32(handle)
            }
            HostPlatformHandle::MacOsNsViewInWindow { ns_window, ns_view } => {
                let mut handle = AppKitWindowHandle::empty();
                handle.ns_window = handle_to_ptr(ns_window);
                handle.ns_view = handle_to_ptr(ns_view);
                RawWindowHandle::AppKit(handle)
            }
            HostPlatformHandle::LinuxX11 { window, .. }
            | HostPlatformHandle::LinuxXWayland { window, .. } => {
                let mut handle = XlibWindowHandle::empty();
                handle.window = window;
                RawWindowHandle::Xlib(handle)
            }
            HostPlatformHandle::LinuxXcb { window, .. } => {
                let mut handle = XcbWindowHandle::empty();
                #[allow(clippy::cast_possible_truncation)]
                {
                    handle.window = window as u32;
                }
                RawWindowHandle::Xcb(handle)
            }
            HostPlatformHandle::MacOsNsView { .. }
            | HostPlatformHandle::MacOsNsWindow { .. }
            | HostPlatformHandle::LinuxWayland { .. } => {
                unreachable!("BaseviewNativeParent rejects unsupported parent handles")
            }
        }
    }
}

// SAFETY: The display handle is derived from the same validated parent record as the window
// handle. The selected variants match Baseview's platform backend expectations.
#[allow(unsafe_code)]
unsafe impl HasRawDisplayHandle for BaseviewNativeParent {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        match self.handle {
            HostPlatformHandle::WindowsHwnd { .. } => {
                RawDisplayHandle::Windows(WindowsDisplayHandle::empty())
            }
            HostPlatformHandle::MacOsNsViewInWindow { .. } => {
                RawDisplayHandle::AppKit(AppKitDisplayHandle::empty())
            }
            HostPlatformHandle::LinuxX11 { display, .. }
            | HostPlatformHandle::LinuxXWayland { display, .. } => {
                let mut handle = XlibDisplayHandle::empty();
                handle.display = handle_to_ptr(display);
                RawDisplayHandle::Xlib(handle)
            }
            HostPlatformHandle::LinuxXcb { connection, .. } => {
                let mut handle = XcbDisplayHandle::empty();
                handle.connection = handle_to_ptr(connection);
                RawDisplayHandle::Xcb(handle)
            }
            HostPlatformHandle::MacOsNsView { .. }
            | HostPlatformHandle::MacOsNsWindow { .. }
            | HostPlatformHandle::LinuxWayland { .. } => {
                unreachable!("BaseviewNativeParent rejects unsupported parent handles")
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

/// Headless-safe Baseview plugin adapter.
#[derive(Clone, Debug, PartialEq)]
pub struct BaseviewPluginAdapter {
    config: PluginEditorConfig,
    parent_fixture: BaseviewParentFixture,
    capabilities: BaseviewCapabilities,
    open_options: WindowOpenOptions,
    destroyed: bool,
    visible: bool,
    requested_process_quit: bool,
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
            requested_process_quit: false,
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

    /// Returns whether process quit was requested.
    #[must_use]
    pub const fn requested_process_quit(&self) -> bool {
        self.requested_process_quit
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
    let (connection, screen_number) = x11rb::connect(None).map_err(|error| {
        BaseviewHostError::new(
            "baseview.x11-present.connect-failed",
            format!("failed to connect to X11 display for Baseview presentation: {error}"),
        )
    })?;
    let depth = connection.setup().roots[screen_number].root_depth;
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
    let mut data = Vec::with_capacity(snapshot.pixels().len().saturating_mul(4));
    for pixel in snapshot.pixels() {
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
}
