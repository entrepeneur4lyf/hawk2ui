#![deny(unsafe_code)]
//! `Baseview`-backed embedded plugin host adapter for `Hawk2UI`.

use baseview::{
    EventStatus, Size, Window, WindowHandle, WindowHandler, WindowOpenOptions, WindowScalePolicy,
};
use hawk2ui_build::ArtifactSignatureVerifier;
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
#[cfg(target_os = "linux")]
use hawk2ui_render_skia::SkiaSurfaceKind;
use hawk2ui_render_skia::{
    RuntimeSceneAssetFallback, RuntimeSceneReplayOptions, SkiaFrameSnapshot, SkiaRendererBackend,
    SkiaSurfaceConfig,
};
use hawk2ui_runtime::RuntimeSceneFrame;
use keyboard_types::{Key, KeyState, KeyboardEvent};
use raw_window_handle::{
    AppKitDisplayHandle, AppKitWindowHandle, HasRawDisplayHandle, HasRawWindowHandle,
    RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
    Win32WindowHandle, WindowsDisplayHandle, XcbDisplayHandle, XcbWindowHandle, XlibDisplayHandle,
    XlibWindowHandle,
};
#[cfg(target_os = "linux")]
use skia_safe::{
    ColorType,
    gpu::{
        DirectContext, Protected, SurfaceOrigin, backend_render_targets, direct_contexts,
        gl::{FramebufferInfo, Interface},
        surfaces,
    },
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::{
    collections::BTreeMap,
    ffi::c_void,
    fmt::{self, Write as _},
    path::PathBuf,
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

    /// Returns the CLAP parent-window APIs this Baseview bridge can attach.
    #[must_use]
    pub fn supported_clap_parent_apis(&self) -> &'static [ClapGuiWindowApi] {
        if self.embedded_parent_attachment() {
            &BASEVIEW_SUPPORTED_CLAP_PARENT_APIS
        } else {
            &[]
        }
    }

    /// Returns whether this Baseview bridge can attach the requested CLAP parent API.
    #[must_use]
    pub const fn supports_clap_parent_api(&self, api: ClapGuiWindowApi) -> bool {
        self.embedded_parent_attachment()
            && matches!(
                api,
                ClapGuiWindowApi::Win32
                    | ClapGuiWindowApi::Cocoa
                    | ClapGuiWindowApi::X11
                    | ClapGuiWindowApi::Wayland
            )
    }

    /// Returns whether this Baseview bridge can attach a parent with the requested platform kind.
    #[must_use]
    pub const fn supports_platform_handle(&self, handle: HostPlatformHandle) -> bool {
        self.embedded_parent_attachment()
            && matches!(
                handle,
                HostPlatformHandle::WindowsHwnd { .. }
                    | HostPlatformHandle::MacOsNsView { .. }
                    | HostPlatformHandle::MacOsNsViewInWindow { .. }
                    | HostPlatformHandle::MacOsNsWindow { .. }
                    | HostPlatformHandle::LinuxX11 { .. }
                    | HostPlatformHandle::LinuxX11Window { .. }
                    | HostPlatformHandle::LinuxXcb { .. }
                    | HostPlatformHandle::LinuxWayland { .. }
                    | HostPlatformHandle::LinuxXWayland { .. }
            )
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
const BASEVIEW_SUPPORTED_CLAP_PARENT_APIS: [ClapGuiWindowApi; 4] = [
    ClapGuiWindowApi::Win32,
    ClapGuiWindowApi::Cocoa,
    ClapGuiWindowApi::X11,
    ClapGuiWindowApi::Wayland,
];

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
    /// Linux Wayland parent.
    Wayland,
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
    LinuxX11Window { window: u64 },
    LinuxXcb { connection: u64, window: u64 },
    LinuxWayland { display: u64, surface: u64 },
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
            HostPlatformHandle::LinuxWayland { display, surface } => {
                require_nonzero_handle(display)?;
                require_nonzero_handle(surface)?;
                (
                    BaseviewNativeParentHandle::LinuxWayland { display, surface },
                    BaseviewNativeParentBackend::Wayland,
                )
            }
            HostPlatformHandle::LinuxX11 { display, window } => {
                require_nonzero_handle(display)?;
                require_nonzero_handle(window)?;
                (
                    BaseviewNativeParentHandle::LinuxX11 { display, window },
                    BaseviewNativeParentBackend::X11,
                )
            }
            HostPlatformHandle::LinuxX11Window { window } => {
                require_nonzero_handle(window)?;
                (
                    BaseviewNativeParentHandle::LinuxX11Window { window },
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
            BaseviewNativeParentHandle::LinuxX11Window { window } => {
                HostPlatformHandle::LinuxX11Window { window }
            }
            BaseviewNativeParentHandle::LinuxXcb { connection, window } => {
                HostPlatformHandle::LinuxXcb { connection, window }
            }
            BaseviewNativeParentHandle::LinuxWayland { display, surface } => {
                HostPlatformHandle::LinuxWayland { display, surface }
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
                | BaseviewNativeParentBackend::Wayland
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
            | BaseviewNativeParentHandle::LinuxX11Window { window }
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
            BaseviewNativeParentHandle::LinuxWayland { surface, .. } => {
                let mut handle = WaylandWindowHandle::empty();
                handle.surface = handle_to_ptr(surface);
                RawWindowHandle::Wayland(handle)
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
            BaseviewNativeParentHandle::LinuxX11Window { .. } => {
                RawDisplayHandle::Xlib(XlibDisplayHandle::empty())
            }
            BaseviewNativeParentHandle::LinuxXcb { connection, .. } => {
                let mut handle = XcbDisplayHandle::empty();
                handle.connection = handle_to_ptr(connection);
                RawDisplayHandle::Xcb(handle)
            }
            BaseviewNativeParentHandle::LinuxWayland { display, .. } => {
                let mut handle = WaylandDisplayHandle::empty();
                handle.display = handle_to_ptr(display);
                RawDisplayHandle::Wayland(handle)
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

fn mouse_button_label(button: baseview::MouseButton) -> String {
    match button {
        baseview::MouseButton::Left => "left".to_string(),
        baseview::MouseButton::Middle => "middle".to_string(),
        baseview::MouseButton::Right => "right".to_string(),
        baseview::MouseButton::Back => "back".to_string(),
        baseview::MouseButton::Forward => "forward".to_string(),
        baseview::MouseButton::Other(button) => format!("other-{button}"),
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

/// Linux software Baseview handler that renders a runtime scene with Skia and presents it into the
/// native child window during frame callbacks.
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
    /// Creates a frame handler for an attached Linux Baseview child window.
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
            .and_then(|snapshot| present_snapshot_to_native_window(window, &snapshot))
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

/// `GL_RGBA8` internal format for a standard 8-bit RGBA framebuffer. Paired with
/// the non-sRGB `GlConfig` the GPU editor requests so Skia writes sRGB-encoded
/// bytes into a plain UNORM buffer — byte-identical to the CPU raster snapshot.
#[cfg(target_os = "linux")]
const GL_RGBA8: u32 = 0x8058;

/// Stable identifier for the GPU editor's adopted Ganesh surface.
#[cfg(target_os = "linux")]
const GPU_EDITOR_SURFACE_ID: &str = "baseview-gpu-editor";

/// The `GlConfig` for the GPU editor window: a plain (non-sRGB) double-buffered
/// RGBA8 framebuffer with an 8-bit stencil, matching the Ganesh surface Skia
/// wraps over it.
///
/// A **core** profile (Baseview's default) is chosen deliberately over
/// compatibility. The Skia-on-GLX crash this path first hit was Skia probing
/// EGL while assembling its interface — fixed by hiding `egl*` in
/// [`GlProcAddressLoader`], independent of the GL profile (a smoke run with a
/// core profile and the EGL hide passes). Core keeps the macOS port viable:
/// macOS exposes GL 3.2 core but caps the compatibility profile at GL 2.1,
/// below what Ganesh needs.
#[cfg(target_os = "linux")]
fn gpu_editor_gl_config() -> baseview::gl::GlConfig {
    baseview::gl::GlConfig {
        srgb: false,
        profile: baseview::gl::Profile::Core,
        ..baseview::gl::GlConfig::default()
    }
}

/// Takes a build-time GPU editor error recorded by the Baseview handler.
#[cfg(target_os = "linux")]
fn take_gpu_editor_open_error(
    last_error: &Arc<Mutex<Option<BaseviewHostError>>>,
) -> Option<BaseviewHostError> {
    match last_error.lock() {
        Ok(mut guard) => guard.take(),
        Err(_) => Some(BaseviewHostError::new(
            "baseview.gpu-error-sink-poisoned",
            "Baseview GPU editor error sink was poisoned while opening the editor window",
        )),
    }
}

// SAFETY: Baseview's `GlContext::make_current`/`make_not_current` are `unsafe`
// because making a context current is only sound on a single thread at a time.
// Every caller below invokes these on Baseview's GUI thread — the thread that
// created the window and owns the context — inside a frame or close callback,
// never concurrently and never from the audio thread. This is the same
// single-GUI-thread invariant the `byo_gui_gl` reference and truce's own GPU
// editor rely on. Isolating the two FFI calls here keeps the handler body
// `unsafe`-free.
#[cfg(target_os = "linux")]
#[allow(unsafe_code)]
fn gl_make_current(gl: &baseview::gl::GlContext) {
    unsafe { gl.make_current() }
}

#[cfg(target_os = "linux")]
#[allow(unsafe_code)]
fn gl_make_not_current(gl: &baseview::gl::GlContext) {
    unsafe { gl.make_not_current() }
}

/// Resolves OpenGL function pointers for Skia's assembled GL interface.
///
/// Baseview's `get_proc_address` (`glXGetProcAddress`) resolves extension and
/// modern entry points but returns null for core GL 1.0/1.1 functions on common
/// drivers. Skia's interface needs those too, so this falls back to `dlsym`
/// against `libGL` for any symbol Baseview cannot resolve — the hybrid strategy
/// GL loaders such as glutin use. A failed `dlopen` simply disables the
/// fallback (the interface assembly then fails cleanly rather than crashing).
#[cfg(target_os = "linux")]
struct GlProcAddressLoader {
    libgl: *mut c_void,
}

#[cfg(target_os = "linux")]
#[allow(unsafe_code)]
impl GlProcAddressLoader {
    fn open() -> Self {
        // SAFETY: opens the already-resident GL client library by soname; the
        // handle is only used for `dlsym` and released in `Drop`.
        let libgl =
            unsafe { libc::dlopen(c"libGL.so.1".as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
        Self { libgl }
    }

    fn resolve(&self, gl: &baseview::gl::GlContext, symbol: &str) -> *const c_void {
        // Hide EGL from Skia. On a GLX/libglvnd system `glXGetProcAddress`
        // resolves EGL symbols to non-null stubs, so while assembling the GL
        // interface Skia would conclude EGL is available and call
        // `eglGetCurrentDisplay`/`eglQueryString` against a bogus display,
        // segfaulting. Returning null for `egl*` keeps Skia on the GLX path.
        if symbol.starts_with("egl") {
            return std::ptr::null();
        }
        let from_glx = gl.get_proc_address(symbol);
        if !from_glx.is_null() || self.libgl.is_null() {
            return from_glx;
        }
        let Ok(name) = std::ffi::CString::new(symbol) else {
            return std::ptr::null();
        };
        // SAFETY: `libgl` is a valid handle from `dlopen`; `name` is a valid
        // NUL-terminated C string. `dlsym` returns null for absent symbols.
        unsafe { libc::dlsym(self.libgl, name.as_ptr()).cast_const() }
    }
}

#[cfg(target_os = "linux")]
#[allow(unsafe_code)]
impl Drop for GlProcAddressLoader {
    fn drop(&mut self) {
        if !self.libgl.is_null() {
            // SAFETY: `libgl` came from a successful `dlopen`; this balances it.
            unsafe {
                libc::dlclose(self.libgl);
            }
        }
    }
}

/// Linux/X11 Baseview handler that renders a runtime scene with Skia's Ganesh
/// GPU backend into the child window's OpenGL framebuffer and presents it with
/// a buffer swap.
///
/// Unlike [`BaseviewX11SkiaFrameHandler`] (CPU raster surface blitted via X11
/// `PutImage`), this wraps the Baseview-owned GL framebuffer as a Skia surface
/// through a Ganesh [`DirectContext`] and draws on the GPU. The GL context is
/// borrowed from the [`Window`] every callback and never owned, so GPU resource
/// release must happen while a live window is in hand: teardown runs on
/// `WillClose` — delivered by Baseview on every close path (user close, host
/// parent-drop, and programmatic `close`) with a live `Window` — not in `Drop`,
/// which has no window and therefore cannot make the context current.
/// A closure that produces the next scene each frame, for the live editor render
/// loop: the truce editor builds one (capturing its render state and bridge) and
/// the GPU handler calls it once per `on_frame`. `Send` so the handler remains
/// `Send` for Baseview's `WindowHandler`.
#[cfg(target_os = "linux")]
pub type EditorSceneProducer = Box<dyn FnMut() -> RuntimeSceneFrame + Send>;

#[cfg(target_os = "linux")]
#[derive(Clone, Debug)]
struct SharedRuntimeSceneProducer {
    scene: Arc<Mutex<RuntimeSceneFrame>>,
}

#[cfg(target_os = "linux")]
impl SharedRuntimeSceneProducer {
    fn new(scene: RuntimeSceneFrame) -> Self {
        Self {
            scene: Arc::new(Mutex::new(scene)),
        }
    }

    fn replace(&self, scene: RuntimeSceneFrame) {
        match self.scene.lock() {
            Ok(mut current) => *current = scene,
            Err(poisoned) => *poisoned.into_inner() = scene,
        }
    }

    fn current(&self) -> RuntimeSceneFrame {
        match self.scene.lock() {
            Ok(current) => current.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    fn scene_producer(&self) -> EditorSceneProducer {
        let shared = self.clone();
        Box::new(move || shared.current())
    }
}

#[cfg(target_os = "linux")]
pub struct BaseviewGlSkiaFrameHandler {
    backend: SkiaRendererBackend,
    context: Option<DirectContext>,
    /// When set, called once per frame to produce the live scene; otherwise the
    /// fixed `scene` is presented every frame (the construction-time / no-bridge
    /// path).
    scene_producer: Option<EditorSceneProducer>,
    scene: RuntimeSceneFrame,
    dpi_scale: f32,
    event_translator: BaseviewEventTranslator,
    event_sink: Arc<Mutex<Vec<PluginHostEvent>>>,
    presented_frames: Arc<AtomicU64>,
    last_error: Arc<Mutex<Option<BaseviewHostError>>>,
    snapshot_sink: Option<Arc<Mutex<Option<SkiaFrameSnapshot>>>>,
    close_after_first_frame: bool,
    torn_down: bool,
}

#[cfg(target_os = "linux")]
impl BaseviewGlSkiaFrameHandler {
    /// Creates a GPU frame handler, building a Ganesh [`DirectContext`] and a
    /// Skia surface wrapping the window's GL framebuffer.
    ///
    /// Must be called from Baseview's `open_parented` builder closure, which
    /// runs on the GUI thread with the GL context available. If GL/Ganesh
    /// initialization fails, the error is recorded into `last_error` and the
    /// handler renders nothing — its first frame closes the window so the editor
    /// lifecycle can observe the failure.
    #[must_use]
    pub fn new(
        window: &mut Window,
        scene: RuntimeSceneFrame,
        metrics: SurfaceMetrics,
        presented_frames: Arc<AtomicU64>,
        last_error: Arc<Mutex<Option<BaseviewHostError>>>,
    ) -> Self {
        let mut handler = Self {
            backend: SkiaRendererBackend::new(),
            context: None,
            scene_producer: None,
            scene,
            dpi_scale: 1.0,
            event_translator: BaseviewEventTranslator::new(metrics),
            event_sink: Arc::new(Mutex::new(Vec::new())),
            presented_frames,
            last_error,
            snapshot_sink: None,
            close_after_first_frame: false,
            torn_down: false,
        };
        if let Err(error) = handler.init_gpu_surface(window, metrics) {
            handler.record_error(error);
        }
        handler
    }

    /// Records translated native events into a caller-owned sink.
    #[must_use]
    pub fn with_event_sink(mut self, event_sink: Arc<Mutex<Vec<PluginHostEvent>>>) -> Self {
        self.event_sink = event_sink;
        self
    }

    /// Installs the per-frame scene producer for the live editor render loop. With
    /// it set, [`Self::on_frame`] calls the producer once per frame to refresh the
    /// presented scene; `None` leaves the handler presenting its fixed `scene`.
    #[must_use]
    pub fn with_scene_producer(mut self, scene_producer: Option<EditorSceneProducer>) -> Self {
        self.scene_producer = scene_producer;
        self
    }

    /// Captures the first presented frame's pixels into a caller-owned sink for
    /// verification. Without a sink, no readback is performed (the fast path).
    #[must_use]
    pub fn with_snapshot_sink(mut self, sink: Arc<Mutex<Option<SkiaFrameSnapshot>>>) -> Self {
        self.snapshot_sink = Some(sink);
        self
    }

    /// Configures whether the handler closes the native child after the first
    /// presented frame.
    #[must_use]
    pub const fn close_after_first_frame(mut self, close_after_first_frame: bool) -> Self {
        self.close_after_first_frame = close_after_first_frame;
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

    fn init_gpu_surface(
        &mut self,
        window: &mut Window,
        metrics: SurfaceMetrics,
    ) -> Result<(), BaseviewHostError> {
        let (width, height) = metrics.physical_size();
        let dpi_scale = scale_factor_to_f32(metrics.scale_factor)?;
        let skia_width = i32::try_from(width).map_err(|_| gl_surface_size_error())?;
        let skia_height = i32::try_from(height).map_err(|_| gl_surface_size_error())?;
        let gl = window.gl_context().ok_or_else(|| {
            BaseviewHostError::new(
                "baseview.gl.context-missing",
                "baseview did not create an OpenGL context; the window must be opened with a GlConfig",
            )
        })?;

        gl_make_current(gl);
        let result =
            self.build_ganesh_surface(gl, (skia_width, skia_height), width, height, dpi_scale);
        gl_make_not_current(gl);

        result.inspect(|()| {
            self.dpi_scale = dpi_scale;
        })
    }

    fn build_ganesh_surface(
        &mut self,
        gl: &baseview::gl::GlContext,
        (skia_width, skia_height): (i32, i32),
        width: u32,
        height: u32,
        dpi_scale: f32,
    ) -> Result<(), BaseviewHostError> {
        // Assemble Skia's GL interface from a hybrid loader. Baseview's
        // `get_proc_address` wraps `glXGetProcAddress`, which resolves extension
        // and modern entry points but returns null for core GL 1.0/1.1 functions
        // (`glGetString`, `glGetIntegerv`, ...) on common drivers — Skia would
        // then call a null pointer while querying extensions and segfault. The
        // loader falls back to `dlsym` for those. (Skia's own `new_native` loader
        // is the no-op stub in the rust-skia prebuilt, so it cannot be used.)
        let loader = GlProcAddressLoader::open();
        let interface =
            Interface::new_load_with(|symbol| loader.resolve(gl, symbol)).ok_or_else(|| {
                BaseviewHostError::new(
                    "baseview.gl.interface-failed",
                    "failed to load an OpenGL interface for Skia Ganesh",
                )
            })?;
        let mut context = direct_contexts::make_gl(interface, None).ok_or_else(|| {
            BaseviewHostError::new(
                "baseview.gl.context-failed",
                "failed to create a Skia Ganesh GL DirectContext",
            )
        })?;
        let framebuffer_info = FramebufferInfo {
            fboid: 0,
            format: GL_RGBA8,
            protected: Protected::No,
        };
        let render_target =
            backend_render_targets::make_gl((skia_width, skia_height), None, 8, framebuffer_info);
        let surface = surfaces::wrap_backend_render_target(
            &mut context,
            &render_target,
            SurfaceOrigin::BottomLeft,
            ColorType::RGBA8888,
            None,
            None,
        )
        .ok_or_else(|| {
            BaseviewHostError::new(
                "baseview.gl.surface-failed",
                "failed to wrap the OpenGL framebuffer as a Skia surface",
            )
        })?;
        self.backend
            .adopt_surface(
                GPU_EDITOR_SURFACE_ID,
                surface,
                width,
                height,
                dpi_scale,
                SkiaSurfaceKind::GpuGl,
            )
            .map_err(|error| map_backend_error(&error))?;
        self.context = Some(context);
        Ok(())
    }

    fn render_gpu_frame(&mut self, window: &mut Window) -> Result<(), BaseviewHostError> {
        let gl = window.gl_context().ok_or_else(|| {
            BaseviewHostError::new(
                "baseview.gl.context-missing",
                "baseview OpenGL context disappeared before a frame could be drawn",
            )
        })?;
        gl_make_current(gl);
        let render = self.draw_and_present(gl);
        gl_make_not_current(gl);
        render
    }

    fn draw_and_present(&mut self, gl: &baseview::gl::GlContext) -> Result<(), BaseviewHostError> {
        let frame_index = self.presented_frames.load(Ordering::SeqCst);
        self.backend
            .begin_frame(GPU_EDITOR_SURFACE_ID)
            .map_err(|error| map_backend_error(&error))?;
        self.backend
            .clear(Color::rgba(0, 0, 0, 0))
            .map_err(|error| map_backend_error(&error))?;
        self.backend
            .draw_runtime_scene_frame_with_options(
                &self.scene,
                runtime_scene_replay_options(frame_index, self.dpi_scale),
            )
            .map_err(|error| map_backend_error(&error))?;
        self.backend
            .end_frame(GPU_EDITOR_SURFACE_ID)
            .map_err(|error| map_backend_error(&error))?;
        // Submit the recorded GPU work, then optionally read it back once for
        // verification (no frame active, work submitted, context current), then
        // present with the buffer swap.
        if let Some(context) = self.context.as_mut() {
            context.flush_and_submit();
        }
        self.capture_verification_snapshot();
        gl.swap_buffers();
        Ok(())
    }

    fn capture_verification_snapshot(&mut self) {
        let Some(sink) = self.snapshot_sink.clone() else {
            return;
        };
        if sink.lock().is_ok_and(|guard| guard.is_some()) {
            return;
        }
        if let Ok(snapshot) = self.backend.read_surface_snapshot(GPU_EDITOR_SURFACE_ID)
            && let Ok(mut guard) = sink.lock()
        {
            *guard = Some(snapshot);
        }
    }

    /// Releases GPU resources while the GL context is still live. Idempotent.
    /// `abandon` orphans the Ganesh objects without issuing GL deletes; Baseview
    /// then destroys the GL context itself, reclaiming everything.
    fn teardown(&mut self, window: &mut Window) {
        if self.torn_down {
            return;
        }
        self.torn_down = true;
        if let Some(gl) = window.gl_context() {
            gl_make_current(gl);
            if let Some(context) = self.context.as_mut() {
                context.abandon();
            }
            gl_make_not_current(gl);
        } else if let Some(context) = self.context.as_mut() {
            context.abandon();
        }
    }
}

#[cfg(target_os = "linux")]
impl WindowHandler for BaseviewGlSkiaFrameHandler {
    fn on_frame(&mut self, window: &mut Window) {
        if self.torn_down {
            return;
        }
        if self.context.is_none() {
            // GL initialization failed in `new`; the error is already recorded.
            // Close so the editor lifecycle (and the smoke) can observe it and end.
            window.close();
            return;
        }
        // Live render loop: refresh the scene from the producer (the editor's
        // per-frame bridge-read → entry → replay cycle) before presenting. The
        // producer degrades internally (keeps the last good scene on failure), so
        // `on_frame` never sees an error here.
        if let Some(producer) = self.scene_producer.as_mut() {
            self.scene = producer();
        }
        let metrics = self.event_translator.metrics();
        match self.render_gpu_frame(window) {
            Ok(()) => {
                let frame_index = self.presented_frames.fetch_add(1, Ordering::SeqCst);
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

    fn on_event(&mut self, window: &mut Window, event: baseview::Event) -> EventStatus {
        // Teardown must run here, not in `Drop`: `WillClose` is the last callback
        // delivered with a live `Window` (hence a current-able GL context) on
        // every close path.
        if matches!(
            event,
            baseview::Event::Window(baseview::WindowEvent::WillClose)
        ) {
            self.teardown(window);
        }
        let translated = self.event_translator.translate(&event);
        self.record_events(translated.events);
        translated.status
    }
}

#[cfg(target_os = "linux")]
impl Drop for BaseviewGlSkiaFrameHandler {
    fn drop(&mut self) {
        // Safety net only. Normal teardown runs on `WillClose` with a live
        // context. By `Drop` the window and its GL context are gone, so the
        // context cannot be made current — but `abandon` frees Skia's CPU-side
        // bookkeeping without issuing GL calls, preventing the `DirectContext`
        // destructor from deleting GL objects against a dead context. If
        // `WillClose` already abandoned, this is a harmless no-op.
        if !self.torn_down
            && let Some(context) = self.context.as_mut()
        {
            context.abandon();
        }
    }
}

#[cfg(target_os = "linux")]
fn gl_surface_size_error() -> BaseviewHostError {
    BaseviewHostError::new(
        "baseview.gl.size-overflow",
        "baseview GPU editor surface size exceeds the Skia render-target range",
    )
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

/// Presentation backend for a CLAP runtime editor window.
///
/// The production backend opens a real Baseview child window. Tests can provide
/// a recording implementation so lifecycle wiring is verified without requiring
/// a real DAW parent window.
pub trait BaseviewRuntimeWindowBackend: fmt::Debug + Send {
    /// Opens the live runtime editor window for `scene`.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when the native child window cannot be opened.
    fn open(
        &mut self,
        adapter: &BaseviewPluginAdapter,
        scene: RuntimeSceneFrame,
    ) -> Result<(), BaseviewHostError>;

    /// Presents a runtime scene frame through the backend and returns a frame snapshot for host
    /// response metadata and deterministic verification.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when the live window is unavailable or rendering fails.
    fn present(
        &mut self,
        adapter: &mut BaseviewPluginAdapter,
        scene: &RuntimeSceneFrame,
    ) -> Result<SkiaFrameSnapshot, BaseviewHostError>;

    /// Closes the live runtime editor window.
    fn close(&mut self);

    /// Returns whether the live runtime editor window is open.
    fn is_open(&self) -> bool;

    /// Returns the number of frames presented through this backend.
    fn presented_frame_count(&self) -> u64;

    /// Drains host events produced by the live window backend.
    fn drain_events(&mut self) -> Vec<PluginHostEvent> {
        Vec::new()
    }
}

/// Production runtime-window backend backed by a real Baseview GPU child window.
pub struct BaseviewGpuRuntimeWindowBackend {
    window: Option<BaseviewEditorWindowHandle>,
    #[cfg(target_os = "linux")]
    live_scene: Option<SharedRuntimeSceneProducer>,
    presented_frames: Arc<AtomicU64>,
    last_error: Arc<Mutex<Option<BaseviewHostError>>>,
    event_sink: Arc<Mutex<Vec<PluginHostEvent>>>,
    host_presented_frames: u64,
}

impl fmt::Debug for BaseviewGpuRuntimeWindowBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BaseviewGpuRuntimeWindowBackend")
            .field("window_open", &self.is_open())
            .field("presented_frame_count", &self.presented_frame_count())
            .finish_non_exhaustive()
    }
}

impl Default for BaseviewGpuRuntimeWindowBackend {
    fn default() -> Self {
        Self {
            window: None,
            #[cfg(target_os = "linux")]
            live_scene: None,
            presented_frames: Arc::new(AtomicU64::new(0)),
            last_error: Arc::new(Mutex::new(None)),
            event_sink: Arc::new(Mutex::new(Vec::new())),
            host_presented_frames: 0,
        }
    }
}

#[cfg(target_os = "linux")]
impl BaseviewGpuRuntimeWindowBackend {
    fn install_live_scene_producer(&mut self, scene: RuntimeSceneFrame) -> EditorSceneProducer {
        let live_scene = SharedRuntimeSceneProducer::new(scene);
        let scene_producer = live_scene.scene_producer();
        self.live_scene = Some(live_scene);
        scene_producer
    }

    fn update_live_scene(&self, scene: &RuntimeSceneFrame) -> Result<(), BaseviewHostError> {
        let Some(live_scene) = self.live_scene.as_ref() else {
            return Err(BaseviewHostError::new(
                "baseview.runtime-window.scene-producer-missing",
                "Baseview GPU runtime window is open without a live scene producer",
            ));
        };
        live_scene.replace(scene.clone());
        Ok(())
    }
}

impl BaseviewRuntimeWindowBackend for BaseviewGpuRuntimeWindowBackend {
    fn open(
        &mut self,
        adapter: &BaseviewPluginAdapter,
        scene: RuntimeSceneFrame,
    ) -> Result<(), BaseviewHostError> {
        if self.is_open() {
            return Ok(());
        }

        #[cfg(target_os = "linux")]
        {
            let scene_producer = self.install_live_scene_producer(scene);
            let initial_scene = self.live_scene.as_ref().ok_or_else(|| {
                BaseviewHostError::new(
                    "baseview.runtime-window.scene-producer-missing",
                    "Baseview GPU runtime window could not install its live scene producer",
                )
            })?;
            let window = adapter.open_gpu_editor_window(
                initial_scene.current(),
                Some(scene_producer),
                Arc::clone(&self.presented_frames),
                Arc::clone(&self.last_error),
                Arc::clone(&self.event_sink),
            );
            let window = match window {
                Ok(window) => window,
                Err(error) => {
                    self.live_scene = None;
                    return Err(error);
                }
            };
            self.window = Some(window);
            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = (adapter, scene);
            Err(BaseviewHostError::new(
                "baseview.runtime-window.unsupported-platform",
                "Baseview runtime editor live windows are currently implemented for Linux; Windows and macOS remain release-gated targets",
            ))
        }
    }

    fn present(
        &mut self,
        adapter: &mut BaseviewPluginAdapter,
        scene: &RuntimeSceneFrame,
    ) -> Result<SkiaFrameSnapshot, BaseviewHostError> {
        if self.is_open() {
            #[cfg(target_os = "linux")]
            self.update_live_scene(scene)?;
        } else {
            self.open(adapter, scene.clone())?;
        }
        let frame_id = self.presented_frame_count();
        let snapshot = render_scene_to_skia_snapshot(scene, adapter.metrics(), frame_id)?;
        self.host_presented_frames = self.host_presented_frames.saturating_add(1);
        Ok(snapshot)
    }

    fn close(&mut self) {
        if let Some(mut window) = self.window.take() {
            window.close();
        }
        #[cfg(target_os = "linux")]
        {
            self.live_scene = None;
        }
    }

    fn is_open(&self) -> bool {
        self.window
            .as_ref()
            .is_some_and(BaseviewEditorWindowHandle::is_open)
    }

    fn presented_frame_count(&self) -> u64 {
        self.presented_frames
            .load(Ordering::SeqCst)
            .max(self.host_presented_frames)
    }

    fn drain_events(&mut self) -> Vec<PluginHostEvent> {
        match self.event_sink.lock() {
            Ok(mut events) => std::mem::take(&mut *events),
            Err(_) => Vec::new(),
        }
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
            // Enabling Baseview's `opengl` feature (for the GPU editor path) adds
            // this field to `WindowOpenOptions`; the CPU/X11 software path opens
            // without a GL context. The GPU path sets it in `open_gpu_editor_window`.
            gl_config: None,
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
        self.open_parented_window_with_options(self.open_options.clone(), build)
    }

    /// Opens a real Baseview child window with caller-provided open options
    /// (e.g. a `GlConfig` for the GPU presentation path) against the validated
    /// native parent.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when the editor is destroyed, the parent handle is invalid, or
    /// the parent handle backend does not match the current target OS.
    pub fn open_parented_window_with_options<H, B>(
        &self,
        options: WindowOpenOptions,
        build: B,
    ) -> Result<WindowHandle, BaseviewHostError>
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut Window) -> H + Send + 'static,
    {
        self.ensure_accepts_host_event()?;
        let native_parent = self.native_parent()?;
        native_parent.ensure_supported_on_current_target()?;
        Ok(Window::open_parented(&native_parent, options, build))
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
    /// Linux software-presentation path: every frame is rendered to a CPU Skia
    /// snapshot and presented into the child window through X11 `PutImage` or
    /// Wayland `wl_shm`, depending on the native child handle (see
    /// [`BaseviewX11SkiaFrameHandler`]). `presented_frames`, `last_error`, and
    /// `event_sink` are shared with the frame handler so the caller can observe
    /// rendering progress, surface errors, and drain host events while the
    /// window lives. The returned [`BaseviewEditorWindowHandle`] is `Send` so a
    /// truce `Editor` can own it, but must still be driven from the GUI thread
    /// that opened it.
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

    /// Opens a real Baseview child window that renders `scene` each frame with
    /// Skia's Ganesh GPU backend and presents it with an OpenGL buffer swap,
    /// returning a [`Send`] owner of its native handle.
    ///
    /// The cross-platform GPU presentation path: the window is opened with a
    /// non-sRGB `GlConfig`, Skia wraps its framebuffer as a Ganesh surface, and
    /// frames are drawn on the GPU (see [`BaseviewGlSkiaFrameHandler`]). GPU
    /// resources are released on `WillClose`, so the returned handle tears down
    /// cleanly when closed. `presented_frames`, `last_error`, and `event_sink`
    /// are shared with the frame handler as in [`Self::open_editor_window`]. The
    /// returned [`BaseviewEditorWindowHandle`] is `Send` so a truce `Editor` can
    /// own it, but must still be driven from the GUI thread that opened it.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when the editor is destroyed, the parent
    /// handle is invalid, or the parent handle backend does not match the
    /// current target OS.
    pub fn open_gpu_editor_window(
        &self,
        scene: RuntimeSceneFrame,
        scene_producer: Option<EditorSceneProducer>,
        presented_frames: Arc<AtomicU64>,
        last_error: Arc<Mutex<Option<BaseviewHostError>>>,
        event_sink: Arc<Mutex<Vec<PluginHostEvent>>>,
    ) -> Result<BaseviewEditorWindowHandle, BaseviewHostError> {
        let metrics = self.config.metrics;
        let mut options = self.open_options.clone();
        options.gl_config = Some(gpu_editor_gl_config());
        let open_error = Arc::clone(&last_error);
        let handle = self.open_parented_window_with_options(options, move |window| {
            BaseviewGlSkiaFrameHandler::new(window, scene, metrics, presented_frames, last_error)
                .with_event_sink(event_sink)
                .with_scene_producer(scene_producer)
        })?;
        let mut editor_handle = BaseviewEditorWindowHandle { handle };
        if let Some(error) = take_gpu_editor_open_error(&open_error) {
            editor_handle.close();
            return Err(error);
        }
        Ok(editor_handle)
    }
}

/// Live CLAP runtime editor attached through `Baseview` and rendered with Skia.
#[derive(Debug)]
pub struct BaseviewClapRuntimeEditor {
    session: ClapRuntimeEditorSession,
    adapter: BaseviewPluginAdapter,
    window_backend: Box<dyn BaseviewRuntimeWindowBackend>,
    last_presented_frame: Option<SkiaFrameSnapshot>,
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
        Self::attach_with_window_backend(
            session,
            parent,
            linux_display_handle,
            parent_fixture_id,
            Box::<BaseviewGpuRuntimeWindowBackend>::default(),
        )
    }

    /// Attaches a verified CLAP runtime editor with an explicit runtime-window backend.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when the CLAP parent cannot be represented for Baseview, the
    /// adapter rejects the configuration, the runtime scene cannot be built, or the backend cannot
    /// open the live editor window.
    pub fn attach_with_window_backend(
        session: ClapRuntimeEditorSession,
        parent: ClapGuiParentHandle,
        linux_display_handle: Option<u64>,
        parent_fixture_id: &'static str,
        mut window_backend: Box<dyn BaseviewRuntimeWindowBackend>,
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
        let initial_scene = session
            .runtime_scene_frame()
            .map_err(|error| baseview_error_from_materialization_error(&error))?;
        window_backend.open(&adapter, initial_scene)?;
        Ok(Self {
            session,
            adapter,
            window_backend,
            last_presented_frame: None,
        })
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

    /// Returns whether the live runtime window backend has an open child window.
    #[must_use]
    pub fn live_window_opened(&self) -> bool {
        self.window_backend.is_open()
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
    pub fn presented_frame_count(&self) -> u64 {
        self.window_backend.presented_frame_count()
    }

    /// Presents the verified sealed runtime scene into the attached Baseview surface.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when the sealed runtime scene cannot be built or the Baseview
    /// surface cannot render the frame.
    pub fn present_runtime_frame(&mut self) -> Result<&SkiaFrameSnapshot, BaseviewHostError> {
        if self.adapter.destroyed() {
            return Err(BaseviewHostError::new(
                "baseview.editor.destroyed",
                "baseview editor has already been destroyed",
            ));
        }
        let frame = self
            .session
            .runtime_scene_frame()
            .map_err(|error| baseview_error_from_materialization_error(&error))?;
        let snapshot = self.window_backend.present(&mut self.adapter, &frame)?;
        self.last_presented_frame = Some(snapshot);
        self.last_presented_frame.as_ref().ok_or_else(|| {
            BaseviewHostError::new(
                "baseview.runtime-window.snapshot-missing",
                "runtime window backend presented without returning frame metadata",
            )
        })
    }

    /// Handles a host-driven resize for the attached live editor.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when the editor is destroyed or metrics are invalid.
    pub fn try_host_resize(&mut self, metrics: SurfaceMetrics) -> Result<(), BaseviewHostError> {
        self.adapter.try_host_resize(metrics)
    }

    /// Routes host focus into the attached live editor.
    pub fn route_focus(&mut self, focused: bool) {
        self.adapter.route_focus(focused);
    }

    /// Routes host keyboard input into the attached live editor.
    pub fn route_keyboard(&mut self, input: KeyboardInput) {
        self.adapter.route_keyboard(input);
    }

    /// Routes host pointer input into the attached live editor.
    pub fn route_pointer(&mut self, input: PointerInput) {
        self.adapter.route_pointer(input);
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
        self.window_backend.close();
    }

    /// Drains host events emitted by the attached editor.
    pub fn drain_events(&mut self) -> Vec<PluginHostEvent> {
        let mut events = self.adapter.drain_events();
        events.extend(self.window_backend.drain_events());
        events
    }
}

type BaseviewRuntimeWindowBackendFactory =
    Arc<dyn Fn() -> Box<dyn BaseviewRuntimeWindowBackend> + Send + Sync>;

/// Host-side CLAP GUI lifecycle bridge for a runtime-backed `Baseview` editor.
pub struct BaseviewClapRuntimeEditorHost {
    plugin_path: PathBuf,
    release_verifier: ArtifactSignatureVerifier,
    linux_display_handle: Option<u64>,
    runtime_window_backend_factory: BaseviewRuntimeWindowBackendFactory,
    session: Option<ClapRuntimeEditorSession>,
    editor: Option<BaseviewClapRuntimeEditor>,
    created_api: Option<ClapGuiWindowApi>,
    parameter_values: BTreeMap<String, ParameterValue>,
    latest_realtime_packets: Vec<RealtimeVisualPacket>,
}

impl fmt::Debug for BaseviewClapRuntimeEditorHost {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BaseviewClapRuntimeEditorHost")
            .field("plugin_path", &self.plugin_path)
            .field("linux_display_handle", &self.linux_display_handle)
            .field("created_api", &self.created_api)
            .field("attached", &self.attached())
            .field("parameter_values", &self.parameter_values)
            .field("latest_realtime_packets", &self.latest_realtime_packets)
            .finish_non_exhaustive()
    }
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
        "hawk2ui_host_bridge_abi=1\ncommand=create\ncommand=set_parent\ncommand=show\ncommand=hide\ncommand=destroy\ncommand=apply_parameter\ncommand=save_state\ncommand=load_state\ncommand=drain_realtime_visuals\nparameter_field=value\nparameter_field=bits\nparameter_field=bool\nparameter_field=choice\nparameter_field=int\nstate_field=param.<id>.bits\nstate_field=param.<id>.bool\nstate_field=param.<id>.choice\nstate_field=param.<id>.int\nresponse=created\nresponse=parent_attached\nresponse=frame_presented\nresponse=hidden\nresponse=destroyed\nresponse=parameter_applied\nresponse=state_saved\nresponse=state_loaded\nresponse=realtime_visuals_drained\nfunction=hawk2ui_editor_dispatch\n"
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
                let value = parse_host_abi_parameter_value(&fields)?;
                let response =
                    host.dispatch(BaseviewClapRuntimeEditorHostCommand::ApplyParameter {
                        parameter_id,
                        value,
                    })?;
                Ok(response_to_host_abi_text(response))
            }
            "save_state" => Ok(response_to_host_abi_text(
                host.dispatch(BaseviewClapRuntimeEditorHostCommand::SaveState)?,
            )),
            "load_state" => {
                let state = parse_host_abi_state(&fields)?;
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
    ///
    /// The host starts with an empty release keyring and therefore rejects signed runtime packages
    /// until [`Self::with_release_verifier`] supplies trusted release keys.
    #[must_use]
    pub fn new(plugin_path: impl Into<PathBuf>, linux_display_handle: Option<u64>) -> Self {
        Self {
            plugin_path: plugin_path.into(),
            release_verifier: ArtifactSignatureVerifier::default(),
            linux_display_handle,
            runtime_window_backend_factory: Arc::new(|| {
                Box::<BaseviewGpuRuntimeWindowBackend>::default()
            }),
            session: None,
            editor: None,
            created_api: None,
            parameter_values: BTreeMap::new(),
            latest_realtime_packets: Vec::new(),
        }
    }

    /// Supplies the trusted release keyring used when resolving the runtime editor package.
    #[must_use]
    pub fn with_release_verifier(mut self, verifier: ArtifactSignatureVerifier) -> Self {
        self.release_verifier = verifier;
        self
    }

    /// Supplies the runtime-window backend factory used when CLAP set-parent attaches the editor.
    #[must_use]
    pub fn with_runtime_window_backend_factory(
        mut self,
        factory: impl Fn() -> Box<dyn BaseviewRuntimeWindowBackend> + Send + Sync + 'static,
    ) -> Self {
        self.runtime_window_backend_factory = Arc::new(factory);
        self
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

    /// Returns whether the attached runtime editor has an open live child window.
    #[must_use]
    pub fn live_window_opened(&self) -> bool {
        self.editor
            .as_ref()
            .is_some_and(BaseviewClapRuntimeEditor::live_window_opened)
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
        let session = ClapRuntimeEditorSession::load_trusted_from_clap_plugin_path(
            &self.plugin_path,
            &self.release_verifier,
        )
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
        let editor = BaseviewClapRuntimeEditor::attach_with_window_backend(
            session,
            parent,
            self.linux_display_handle,
            parent_fixture_id,
            (self.runtime_window_backend_factory)(),
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

    /// Routes a host resize into the attached live editor.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when no live editor is attached or metrics are invalid.
    pub fn host_resize(&mut self, metrics: SurfaceMetrics) -> Result<(), BaseviewHostError> {
        self.editor_mut()?.try_host_resize(metrics)
    }

    /// Routes host focus into the attached live editor.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when no live editor is attached.
    pub fn route_focus(&mut self, focused: bool) -> Result<(), BaseviewHostError> {
        self.editor_mut()?.route_focus(focused);
        Ok(())
    }

    /// Routes host keyboard input into the attached live editor.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when no live editor is attached.
    pub fn route_keyboard(&mut self, input: KeyboardInput) -> Result<(), BaseviewHostError> {
        self.editor_mut()?.route_keyboard(input);
        Ok(())
    }

    /// Routes host pointer input into the attached live editor.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when no live editor is attached.
    pub fn route_pointer(&mut self, input: PointerInput) -> Result<(), BaseviewHostError> {
        self.editor_mut()?.route_pointer(input);
        Ok(())
    }

    /// Drains host events emitted by the attached live editor.
    ///
    /// # Errors
    ///
    /// Returns [`BaseviewHostError`] when no live editor is attached.
    pub fn drain_events(&mut self) -> Result<Vec<PluginHostEvent>, BaseviewHostError> {
        Ok(self.editor_mut()?.drain_events())
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

    fn editor_mut(&mut self) -> Result<&mut BaseviewClapRuntimeEditor, BaseviewHostError> {
        self.editor.as_mut().ok_or_else(not_attached_error)
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

fn parse_host_abi_u32(value: &str, field: &str) -> Result<u32, BaseviewHostError> {
    let value = parse_host_abi_u64(value, field)?;
    u32::try_from(value).map_err(|error| {
        BaseviewHostError::new(
            "baseview.clap-host-abi.invalid-integer",
            format!("CLAP host ABI field `{field}` is outside the u32 range: {error}"),
        )
    })
}

fn parse_host_abi_i64(value: &str, field: &str) -> Result<i64, BaseviewHostError> {
    value.parse::<i64>().map_err(|error| {
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

fn parse_host_abi_parameter_value(
    fields: &BTreeMap<String, String>,
) -> Result<ParameterValue, BaseviewHostError> {
    let mut value = None;
    if let Some(raw) = fields.get("value") {
        set_host_abi_parameter_value(
            &mut value,
            ParameterValue::Float(parse_host_abi_f64(raw, "value")?),
        )?;
    }
    if let Some(raw) = fields.get("bits") {
        set_host_abi_parameter_value(
            &mut value,
            ParameterValue::Float(f64::from_bits(parse_host_abi_u64(raw, "bits")?)),
        )?;
    }
    if let Some(raw) = fields.get("bool") {
        set_host_abi_parameter_value(
            &mut value,
            ParameterValue::Bool(parse_host_abi_bool(raw, "bool")?),
        )?;
    }
    if let Some(raw) = fields.get("choice") {
        set_host_abi_parameter_value(
            &mut value,
            ParameterValue::Choice(parse_host_abi_u32(raw, "choice")?),
        )?;
    }
    if let Some(raw) = fields.get("int") {
        set_host_abi_parameter_value(
            &mut value,
            ParameterValue::Int(parse_host_abi_i64(raw, "int")?),
        )?;
    }
    value.ok_or_else(|| {
        BaseviewHostError::new(
            "baseview.clap-host-abi.missing-field",
            "CLAP host ABI command missing a parameter value field",
        )
    })
}

fn set_host_abi_parameter_value(
    slot: &mut Option<ParameterValue>,
    value: ParameterValue,
) -> Result<(), BaseviewHostError> {
    if slot.replace(value).is_some() {
        return Err(BaseviewHostError::new(
            "baseview.clap-host-abi.ambiguous-parameter-value",
            "CLAP host ABI parameter command must provide exactly one value field",
        ));
    }
    Ok(())
}

fn parse_host_abi_state(
    fields: &BTreeMap<String, String>,
) -> Result<PluginStateEnvelope, BaseviewHostError> {
    let mut state = PluginStateEnvelope::new(1);
    for (key, value) in fields {
        let Some(rest) = key.strip_prefix("param.") else {
            continue;
        };
        let Some((parameter_id, suffix)) = rest.rsplit_once('.') else {
            return Err(BaseviewHostError::new(
                "baseview.clap-host-abi.invalid-state-field",
                format!("CLAP host ABI state field `{key}` must include a typed suffix"),
            ));
        };
        let state_value = match suffix {
            "bits" => StateValue::Float(f64::from_bits(parse_host_abi_u64(value, key)?)),
            "bool" => StateValue::Bool(parse_host_abi_bool(value, key)?),
            "choice" => StateValue::Choice(parse_host_abi_u32(value, key)?),
            "int" => StateValue::Int(parse_host_abi_i64(value, key)?),
            other => {
                return Err(BaseviewHostError::new(
                    "baseview.clap-host-abi.invalid-state-field",
                    format!("CLAP host ABI state field `{key}` has unsupported suffix `{other}`"),
                ));
            }
        };
        if state
            .parameter_state
            .insert(parameter_id.to_string(), state_value)
            .is_some()
        {
            return Err(BaseviewHostError::new(
                "baseview.clap-host-abi.duplicate-state-field",
                format!("CLAP host ABI state provides multiple values for `{parameter_id}`"),
            ));
        }
    }
    Ok(state)
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
                match value {
                    StateValue::Float(value) => {
                        let _ = writeln!(response, "param.{parameter_id}.bits={}", value.to_bits());
                    }
                    StateValue::Bool(value) => {
                        let _ = writeln!(response, "param.{parameter_id}.bool={value}");
                    }
                    StateValue::Choice(value) => {
                        let _ = writeln!(response, "param.{parameter_id}.choice={value}");
                    }
                    StateValue::Int(value) => {
                        let _ = writeln!(response, "param.{parameter_id}.int={value}");
                    }
                    StateValue::String(_) => {}
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
        ParameterValue::Float(_)
        | ParameterValue::Int(_)
        | ParameterValue::Bool(_)
        | ParameterValue::Choice(_) => Ok(()),
    }
}

fn state_value_from_parameter(value: &ParameterValue) -> StateValue {
    match value {
        ParameterValue::Float(value) => StateValue::Float(*value),
        ParameterValue::Bool(value) => StateValue::Bool(*value),
        ParameterValue::Choice(value) => StateValue::Choice(*value),
        ParameterValue::Int(value) => StateValue::Int(*value),
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
        StateValue::Choice(value) => Ok(ParameterValue::Choice(*value)),
        StateValue::Int(value) => Ok(ParameterValue::Int(*value)),
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
    if BaseviewCapabilities::plugin_editor().supports_platform_handle(handle) {
        Ok(())
    } else {
        Err(BaseviewHostError::new(
            "baseview.platform.unsupported",
            "baseview Linux backend attaches through X11/XCB/Wayland/XWayland parent handles",
        ))
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
        .draw_runtime_scene_frame_with_options(
            scene,
            runtime_scene_replay_options(frame_index, dpi_scale),
        )
        .map_err(|error| map_backend_error(&error))?;
    backend
        .end_frame("baseview-editor")
        .map_err(|error| map_backend_error(&error))?;
    backend
        .frame_snapshot("baseview-editor")
        .map_err(|error| map_backend_error(&error))
        .cloned()
}

const fn runtime_scene_replay_options(
    frame_index: u64,
    dpi_scale: f32,
) -> RuntimeSceneReplayOptions {
    RuntimeSceneReplayOptions::new(frame_index, dpi_scale)
        .with_missing_asset_fallback(RuntimeSceneAssetFallback::Placeholder)
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BaseviewSoftwarePresentationTarget {
    X11 { drawable: u32 },
    Wayland { display: usize, surface: usize },
}

#[cfg(target_os = "linux")]
fn present_snapshot_to_native_window(
    window: &mut Window,
    snapshot: &SkiaFrameSnapshot,
) -> Result<(), BaseviewHostError> {
    match baseview_software_presentation_target(
        window.raw_display_handle(),
        window.raw_window_handle(),
    )? {
        BaseviewSoftwarePresentationTarget::X11 { drawable } => {
            present_snapshot_to_x11_drawable(drawable, snapshot)
        }
        BaseviewSoftwarePresentationTarget::Wayland { .. } => {
            present_snapshot_to_wayland_window(window, snapshot)
        }
    }
}

#[cfg(target_os = "linux")]
fn present_snapshot_to_x11_drawable(
    drawable: u32,
    snapshot: &SkiaFrameSnapshot,
) -> Result<(), BaseviewHostError> {
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
fn present_snapshot_to_wayland_window(
    window: &mut Window,
    snapshot: &SkiaFrameSnapshot,
) -> Result<(), BaseviewHostError> {
    let pixels = snapshot_to_wayland_xrgb8888(snapshot);
    window
        .hawk2ui_present_software_frame(snapshot.width(), snapshot.height(), &pixels)
        .map_err(|error| {
            BaseviewHostError::new(
                "baseview.wayland-present.failed",
                format!(
                    "failed to present Baseview frame pixels into Wayland child surface: {error}"
                ),
            )
        })
}

#[cfg(target_os = "linux")]
fn baseview_software_presentation_target(
    display: RawDisplayHandle,
    window: RawWindowHandle,
) -> Result<BaseviewSoftwarePresentationTarget, BaseviewHostError> {
    match (display, window) {
        (_, RawWindowHandle::Xlib(handle)) => {
            let drawable = u32::try_from(handle.window).map_err(|_| {
                BaseviewHostError::new(
                    "baseview.x11-present.invalid-window",
                    "baseview Xlib child window handle must fit X11 window id",
                )
            })?;
            Ok(BaseviewSoftwarePresentationTarget::X11 { drawable })
        }
        (_, RawWindowHandle::Xcb(handle)) => Ok(BaseviewSoftwarePresentationTarget::X11 {
            drawable: handle.window,
        }),
        (RawDisplayHandle::Wayland(display), RawWindowHandle::Wayland(window)) => {
            let display = wayland_ptr_to_usize(display.display, "display")?;
            let surface = wayland_ptr_to_usize(window.surface, "surface")?;
            Ok(BaseviewSoftwarePresentationTarget::Wayland { display, surface })
        }
        _ => Err(BaseviewHostError::new(
            "baseview.software-present.unsupported-window",
            "baseview Linux software presentation requires an Xlib, XCB, or Wayland child window",
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

#[cfg(target_os = "linux")]
fn snapshot_to_wayland_xrgb8888(snapshot: &SkiaFrameSnapshot) -> Vec<u8> {
    pixels_to_wayland_xrgb8888(snapshot.pixels())
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

/// Converts `0x00RRGGBB` snapshot pixels into native-endian Wayland `XRGB8888` words.
#[cfg(target_os = "linux")]
fn pixels_to_wayland_xrgb8888(pixels: &[u32]) -> Vec<u8> {
    let mut data = Vec::with_capacity(pixels.len().saturating_mul(4));
    for pixel in pixels {
        data.extend_from_slice(&(pixel & 0x00ff_ffff).to_ne_bytes());
    }
    data
}

#[cfg(target_os = "linux")]
fn wayland_ptr_to_usize(ptr: *mut c_void, label: &str) -> Result<usize, BaseviewHostError> {
    let value = ptr as usize;
    if value == 0 {
        Err(BaseviewHostError::new(
            "baseview.wayland-present.invalid-handle",
            format!("baseview Wayland {label} handle must be non-null"),
        ))
    } else {
        Ok(value)
    }
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
    use hawk2ui_layout::{FlexDirection, LayoutSizing, LayoutStyle, Viewport};
    use hawk2ui_runtime::{
        RuntimeDrawCommand, RuntimeSceneBridge, RuntimeViewId, RuntimeViewNode, RuntimeViewTree,
        RuntimeVisual,
    };

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

    #[cfg(target_os = "linux")]
    #[test]
    fn software_presentation_target_routes_wayland_surfaces_to_wayland_presenter() {
        let mut display = WaylandDisplayHandle::empty();
        display.display = handle_to_ptr(0x100);
        let mut window = WaylandWindowHandle::empty();
        window.surface = handle_to_ptr(0x200);

        assert_eq!(
            baseview_software_presentation_target(
                RawDisplayHandle::Wayland(display),
                RawWindowHandle::Wayland(window),
            ),
            Ok(BaseviewSoftwarePresentationTarget::Wayland {
                display: 0x100,
                surface: 0x200,
            })
        );
    }

    #[test]
    fn gpu_editor_open_error_is_taken_from_shared_error_sink() {
        let errors = Arc::new(Mutex::new(Some(BaseviewHostError::new(
            "baseview.gl.context-missing",
            "baseview did not create an OpenGL context",
        ))));

        let error = take_gpu_editor_open_error(&errors)
            .expect("GPU editor open should surface build-time GL errors");

        assert_eq!(error.rule(), "baseview.gl.context-missing");
        assert!(
            take_gpu_editor_open_error(&errors).is_none(),
            "GPU editor open error should only be reported once"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn shared_runtime_scene_producer_returns_latest_presented_scene() {
        let producer = SharedRuntimeSceneProducer::new(runtime_scene_frame(
            320.0,
            180.0,
            Color::rgba(12, 34, 56, 255),
        ));
        let mut render_loop = producer.scene_producer();

        assert_eq!(
            first_fill_color(&render_loop()),
            Some(Color::rgba(12, 34, 56, 255))
        );

        producer.replace(runtime_scene_frame(
            320.0,
            180.0,
            Color::rgba(90, 120, 30, 255),
        ));

        assert_eq!(
            first_fill_color(&render_loop()),
            Some(Color::rgba(90, 120, 30, 255))
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn gpu_runtime_window_backend_updates_live_render_loop_scene() {
        let mut backend = BaseviewGpuRuntimeWindowBackend::default();
        let mut render_loop = backend.install_live_scene_producer(runtime_scene_frame(
            320.0,
            180.0,
            Color::rgba(12, 34, 56, 255),
        ));

        assert_eq!(
            first_fill_color(&render_loop()),
            Some(Color::rgba(12, 34, 56, 255))
        );

        backend
            .update_live_scene(&runtime_scene_frame(
                320.0,
                180.0,
                Color::rgba(90, 120, 30, 255),
            ))
            .expect("open GPU backend updates its render-loop scene");

        assert_eq!(
            first_fill_color(&render_loop()),
            Some(Color::rgba(90, 120, 30, 255))
        );
    }

    #[cfg(target_os = "linux")]
    fn runtime_scene_frame(width: f32, height: f32, color: Color) -> RuntimeSceneFrame {
        let tree = RuntimeViewTree::new(RuntimeViewNode::new(
            RuntimeViewId::new("root"),
            LayoutStyle::flex_container(FlexDirection::Column)
                .with_size(LayoutSizing::fixed(width, height)),
            RuntimeVisual::Fill(color),
        ));
        RuntimeSceneBridge::new(Viewport::new(width, height))
            .build(&tree)
            .expect("runtime scene frame builds")
    }

    #[cfg(target_os = "linux")]
    fn first_fill_color(frame: &RuntimeSceneFrame) -> Option<Color> {
        frame.draw_commands().iter().find_map(|command| {
            if let RuntimeDrawCommand::Fill { color, .. } = command {
                Some(*color)
            } else {
                None
            }
        })
    }
}
