#![forbid(unsafe_code)]
//! `Winit`-backed desktop host adapter for `Hawk2UI`.

mod runtime;
mod software_frame;

use std::path::{Path, PathBuf};

use hawk2ui_api::Diagnostic;
use hawk2ui_host::{
    ClipboardCapability, DesktopDialogFileFilter, DesktopDialogLevel, DesktopDialogRequest,
    DesktopDialogResponse, DesktopHostAdapter, DesktopHostEvent, DesktopWindowConfig,
    HostPlatformHandle, KeyboardInput, LinuxWindowSystem, PointerInput, RepaintRequest,
    SurfaceClipboardRequest, SurfaceMetrics, SurfaceOwnership, WindowMode,
};
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent};

pub use runtime::{
    DesktopRuntimeEvent, WinitDesktopRuntime, WinitDesktopRuntimeConfig, WinitDesktopRuntimeSummary,
};
pub use software_frame::{SoftwareFrame, SoftwareFrameRenderer, physical_frame_size};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-host-winit";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

/// Winit platform fixture used by headless tests and adapter validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WinitPlatformFixture {
    handle: HostPlatformHandle,
}

impl WinitPlatformFixture {
    /// Creates a Linux fixture for the selected window system.
    #[must_use]
    pub const fn linux(window_system: LinuxWindowSystem) -> Self {
        let handle = match window_system {
            LinuxWindowSystem::Wayland => HostPlatformHandle::linux_wayland(1, 2),
            LinuxWindowSystem::X11 | LinuxWindowSystem::XWayland => {
                HostPlatformHandle::linux_x11(1, 3)
            }
            LinuxWindowSystem::Xcb => HostPlatformHandle::linux_xcb(4, 5),
        };
        Self { handle }
    }

    /// Creates a Windows HWND fixture.
    #[must_use]
    pub const fn windows() -> Self {
        Self {
            handle: HostPlatformHandle::windows_hwnd(6),
        }
    }

    /// Creates a macOS `NSWindow` fixture.
    #[must_use]
    pub const fn macos() -> Self {
        Self {
            handle: HostPlatformHandle::macos_ns_window(7),
        }
    }

    /// Returns the platform handle.
    #[must_use]
    pub const fn handle(&self) -> HostPlatformHandle {
        self.handle
    }
}

/// Winit host capability report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WinitHostCapabilities {
    flags: u16,
}

impl WinitHostCapabilities {
    /// Returns desktop Winit capabilities.
    #[must_use]
    pub const fn desktop() -> Self {
        Self {
            flags: WINIT_CAP_OWNS_WINDOW
                | WINIT_CAP_CLOSE
                | WINIT_CAP_MINIMIZE
                | WINIT_CAP_MAXIMIZE
                | WINIT_CAP_FULLSCREEN
                | WINIT_CAP_FOCUS
                | WINIT_CAP_KEYBOARD
                | WINIT_CAP_POINTER
                | WINIT_CAP_CLIPBOARD
                | WINIT_CAP_RESIZE
                | WINIT_CAP_REPAINT,
        }
    }

    /// Returns whether Winit owns the desktop window.
    #[must_use]
    pub const fn owns_window(&self) -> bool {
        self.supports(WinitCapability::OwnsWindow)
    }

    /// Returns whether a capability is supported.
    #[must_use]
    pub const fn supports(&self, capability: WinitCapability) -> bool {
        self.flags & capability.flag() != 0
    }
}

/// Single Winit desktop capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WinitCapability {
    /// Owns the desktop top-level window.
    OwnsWindow,
    /// Handles close requests.
    Close,
    /// Handles minimize requests.
    Minimize,
    /// Handles maximize requests.
    Maximize,
    /// Handles fullscreen requests.
    Fullscreen,
    /// Handles focus events.
    Focus,
    /// Handles keyboard input.
    Keyboard,
    /// Handles pointer input.
    Pointer,
    /// Handles clipboard availability.
    Clipboard,
    /// Handles resize events.
    Resize,
    /// Handles repaint requests.
    Repaint,
}

impl WinitCapability {
    const fn flag(self) -> u16 {
        match self {
            Self::OwnsWindow => WINIT_CAP_OWNS_WINDOW,
            Self::Close => WINIT_CAP_CLOSE,
            Self::Minimize => WINIT_CAP_MINIMIZE,
            Self::Maximize => WINIT_CAP_MAXIMIZE,
            Self::Fullscreen => WINIT_CAP_FULLSCREEN,
            Self::Focus => WINIT_CAP_FOCUS,
            Self::Keyboard => WINIT_CAP_KEYBOARD,
            Self::Pointer => WINIT_CAP_POINTER,
            Self::Clipboard => WINIT_CAP_CLIPBOARD,
            Self::Resize => WINIT_CAP_RESIZE,
            Self::Repaint => WINIT_CAP_REPAINT,
        }
    }
}

/// Host events produced from a single native `winit` window event.
#[derive(Clone, Debug, PartialEq)]
pub struct WinitTranslatedEvent {
    /// Desktop host events emitted by the translation.
    pub events: Vec<DesktopHostEvent>,
    /// Whether this event requires a redraw request.
    pub requires_redraw: bool,
    /// Whether this event requests native event-loop exit.
    pub requests_close: bool,
}

impl WinitTranslatedEvent {
    fn new(events: Vec<DesktopHostEvent>) -> Self {
        Self {
            events,
            requires_redraw: false,
            requests_close: false,
        }
    }

    fn redraw(mut self) -> Self {
        self.requires_redraw = true;
        self
    }

    fn close(mut self) -> Self {
        self.requests_close = true;
        self
    }
}

/// Stateful translator from native `winit` events into `Hawk2UI` desktop host events.
#[derive(Clone, Debug, PartialEq)]
pub struct WinitEventTranslator {
    metrics: SurfaceMetrics,
    last_pointer_position: (f64, f64),
}

/// Response from a native clipboard request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WinitClipboardResponse {
    /// Text read from the native clipboard.
    Text(String),
    /// Text was written to the native clipboard.
    Written,
    /// Native clipboard text was cleared.
    Cleared,
}

/// Backend boundary for native desktop clipboard access.
pub trait WinitClipboardBackend {
    /// Reads clipboard text.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the platform clipboard cannot be read.
    fn read_text(&mut self) -> Result<String, WinitHostError>;

    /// Writes clipboard text.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the platform clipboard cannot be written.
    fn write_text(&mut self, text: String) -> Result<(), WinitHostError>;

    /// Clears clipboard text when supported.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the platform clipboard cannot be cleared.
    fn clear_text(&mut self) -> Result<(), WinitHostError>;
}

/// Production native clipboard backend backed by the operating-system clipboard.
pub struct ArboardClipboardBackend {
    clipboard: arboard::Clipboard,
}

impl ArboardClipboardBackend {
    /// Opens the native clipboard.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the platform clipboard cannot be opened.
    pub fn new() -> Result<Self, WinitHostError> {
        let clipboard = arboard::Clipboard::new().map_err(|error| {
            WinitHostError::new(
                "desktop.clipboard.open-failed",
                format!("failed to open native clipboard: {error}"),
            )
        })?;
        Ok(Self { clipboard })
    }
}

impl WinitClipboardBackend for ArboardClipboardBackend {
    fn read_text(&mut self) -> Result<String, WinitHostError> {
        self.clipboard.get_text().map_err(|error| {
            WinitHostError::new(
                "desktop.clipboard.read-failed",
                format!("failed to read native clipboard text: {error}"),
            )
        })
    }

    fn write_text(&mut self, text: String) -> Result<(), WinitHostError> {
        self.clipboard.set_text(text).map_err(|error| {
            WinitHostError::new(
                "desktop.clipboard.write-failed",
                format!("failed to write native clipboard text: {error}"),
            )
        })
    }

    fn clear_text(&mut self) -> Result<(), WinitHostError> {
        self.write_text(String::new())
    }
}

/// Capability-checked native clipboard bridge for Winit desktop hosts.
#[derive(Clone, Debug)]
pub struct WinitClipboardBridge<B> {
    capability: ClipboardCapability,
    backend: B,
}

impl<B: WinitClipboardBackend> WinitClipboardBridge<B> {
    /// Creates a clipboard bridge from a host capability and backend.
    #[must_use]
    pub const fn new(capability: ClipboardCapability, backend: B) -> Self {
        Self {
            capability,
            backend,
        }
    }

    /// Executes a clipboard request against the native backend.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when capability policy denies the operation or the native
    /// clipboard backend fails.
    pub fn handle_request(
        &mut self,
        request: SurfaceClipboardRequest,
    ) -> Result<WinitClipboardResponse, WinitHostError> {
        match request {
            SurfaceClipboardRequest::Read => {
                self.ensure_can_read()?;
                self.backend.read_text().map(WinitClipboardResponse::Text)
            }
            SurfaceClipboardRequest::Write(text) => {
                self.ensure_can_write()?;
                self.backend.write_text(text)?;
                Ok(WinitClipboardResponse::Written)
            }
            SurfaceClipboardRequest::Clear => {
                self.ensure_can_write()?;
                self.backend.clear_text()?;
                Ok(WinitClipboardResponse::Cleared)
            }
        }
    }

    fn ensure_can_read(&self) -> Result<(), WinitHostError> {
        match self.capability {
            ClipboardCapability::Read | ClipboardCapability::ReadWrite => Ok(()),
            ClipboardCapability::None | ClipboardCapability::Write => Err(WinitHostError::new(
                "desktop.clipboard.read-denied",
                "desktop clipboard read requires read capability",
            )),
        }
    }

    fn ensure_can_write(&self) -> Result<(), WinitHostError> {
        match self.capability {
            ClipboardCapability::Write | ClipboardCapability::ReadWrite => Ok(()),
            ClipboardCapability::None | ClipboardCapability::Read => Err(WinitHostError::new(
                "desktop.clipboard.write-denied",
                "desktop clipboard write requires write capability",
            )),
        }
    }
}

/// Backend boundary for native desktop dialog access.
pub trait WinitDialogBackend {
    /// Shows a native message dialog.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the platform dialog backend fails.
    fn show_message(
        &mut self,
        title: String,
        message: String,
        level: DesktopDialogLevel,
    ) -> Result<(), WinitHostError>;

    /// Shows a native single-file open dialog.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the platform dialog backend fails.
    fn open_file(
        &mut self,
        title: String,
        directory: Option<PathBuf>,
        filters: Vec<DesktopDialogFileFilter>,
    ) -> Result<Option<PathBuf>, WinitHostError>;

    /// Shows a native save-file dialog.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the platform dialog backend fails.
    fn save_file(
        &mut self,
        title: String,
        directory: Option<PathBuf>,
        file_name: Option<String>,
        filters: Vec<DesktopDialogFileFilter>,
    ) -> Result<Option<PathBuf>, WinitHostError>;
}

/// Production native dialog backend backed by platform dialogs.
#[derive(Clone, Copy, Debug, Default)]
pub struct RfdDialogBackend;

impl RfdDialogBackend {
    /// Creates the default native dialog backend.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl WinitDialogBackend for RfdDialogBackend {
    fn show_message(
        &mut self,
        title: String,
        message: String,
        level: DesktopDialogLevel,
    ) -> Result<(), WinitHostError> {
        let _result = rfd::MessageDialog::new()
            .set_title(title)
            .set_description(message)
            .set_level(rfd_message_level(level))
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
        Ok(())
    }

    fn open_file(
        &mut self,
        title: String,
        directory: Option<PathBuf>,
        filters: Vec<DesktopDialogFileFilter>,
    ) -> Result<Option<PathBuf>, WinitHostError> {
        let dialog = configure_file_dialog(title, directory.as_deref(), filters);
        Ok(dialog.pick_file())
    }

    fn save_file(
        &mut self,
        title: String,
        directory: Option<PathBuf>,
        file_name: Option<String>,
        filters: Vec<DesktopDialogFileFilter>,
    ) -> Result<Option<PathBuf>, WinitHostError> {
        let mut dialog = configure_file_dialog(title, directory.as_deref(), filters);
        if let Some(file_name) = file_name {
            dialog = dialog.set_file_name(file_name);
        }
        Ok(dialog.save_file())
    }
}

/// Native dialog bridge for Winit desktop hosts.
#[derive(Clone, Debug)]
pub struct WinitDialogBridge<B> {
    backend: B,
}

impl<B: WinitDialogBackend> WinitDialogBridge<B> {
    /// Creates a dialog bridge from a native backend.
    #[must_use]
    pub const fn new(backend: B) -> Self {
        Self { backend }
    }

    /// Executes a native dialog request.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when request validation or the native dialog backend fails.
    pub fn handle_request(
        &mut self,
        request: DesktopDialogRequest,
    ) -> Result<DesktopDialogResponse, WinitHostError> {
        match request {
            DesktopDialogRequest::Message {
                title,
                message,
                level,
            } => {
                validate_dialog_title(&title)?;
                validate_dialog_message(&message)?;
                self.backend.show_message(title, message, level)?;
                Ok(DesktopDialogResponse::Acknowledged)
            }
            DesktopDialogRequest::OpenFile {
                title,
                directory,
                filters,
            } => {
                validate_dialog_title(&title)?;
                validate_dialog_filters(&filters)?;
                Ok(self
                    .backend
                    .open_file(title, directory, filters)?
                    .map_or(DesktopDialogResponse::Cancelled, |path| {
                        DesktopDialogResponse::SelectedFile(path)
                    }))
            }
            DesktopDialogRequest::SaveFile {
                title,
                directory,
                file_name,
                filters,
            } => {
                validate_dialog_title(&title)?;
                if let Some(file_name) = file_name.as_ref() {
                    validate_dialog_file_name(file_name)?;
                }
                validate_dialog_filters(&filters)?;
                Ok(self
                    .backend
                    .save_file(title, directory, file_name, filters)?
                    .map_or(DesktopDialogResponse::Cancelled, |path| {
                        DesktopDialogResponse::SavedFile(path)
                    }))
            }
        }
    }
}

impl WinitEventTranslator {
    /// Creates a translator with the current surface metrics.
    #[must_use]
    pub const fn new(metrics: SurfaceMetrics) -> Self {
        Self {
            metrics,
            last_pointer_position: (0.0, 0.0),
        }
    }

    /// Returns the latest metrics observed from native resize/DPI events.
    #[must_use]
    pub const fn metrics(&self) -> SurfaceMetrics {
        self.metrics
    }

    /// Translates a native `winit` window event into host events.
    #[must_use]
    pub fn translate(&mut self, event: &WindowEvent) -> WinitTranslatedEvent {
        match event {
            WindowEvent::Resized(size) => self.translate_resize(size.width, size.height),
            WindowEvent::CloseRequested | WindowEvent::Destroyed => {
                WinitTranslatedEvent::new(vec![DesktopHostEvent::CloseRequested(
                    "native close requested".into(),
                )])
                .close()
            }
            WindowEvent::DroppedFile(path) => {
                WinitTranslatedEvent::new(vec![DesktopHostEvent::FileDragDrop(format!(
                    "dropped:{}",
                    path.to_string_lossy()
                ))])
            }
            WindowEvent::HoveredFile(path) => {
                WinitTranslatedEvent::new(vec![DesktopHostEvent::FileDragDrop(format!(
                    "hovered:{}",
                    path.to_string_lossy()
                ))])
            }
            WindowEvent::HoveredFileCancelled => {
                WinitTranslatedEvent::new(vec![DesktopHostEvent::FileDragDrop(
                    "hover-cancelled".into(),
                )])
            }
            WindowEvent::Focused(focused) => {
                WinitTranslatedEvent::new(vec![DesktopHostEvent::FocusChanged(*focused)])
            }
            WindowEvent::KeyboardInput { event, .. } => {
                WinitTranslatedEvent::new(vec![DesktopHostEvent::KeyboardInput(
                    KeyboardInput::new(
                        format!("{:?}", event.logical_key),
                        event.state == ElementState::Pressed,
                    ),
                )])
            }
            WindowEvent::Ime(event) => {
                WinitTranslatedEvent::new(vec![DesktopHostEvent::ImeInput(ime_event_label(event))])
            }
            WindowEvent::CursorMoved { position, .. } => {
                let x = position.x / self.metrics.scale_factor;
                let y = position.y / self.metrics.scale_factor;
                self.last_pointer_position = (x, y);
                WinitTranslatedEvent::new(vec![DesktopHostEvent::PointerInput(PointerInput::new(
                    x, y, "move",
                ))])
            }
            WindowEvent::CursorEntered { .. } => {
                let (x, y) = self.last_pointer_position;
                WinitTranslatedEvent::new(vec![DesktopHostEvent::PointerInput(PointerInput::new(
                    x, y, "enter",
                ))])
            }
            WindowEvent::CursorLeft { .. } => {
                let (x, y) = self.last_pointer_position;
                WinitTranslatedEvent::new(vec![DesktopHostEvent::PointerInput(PointerInput::new(
                    x, y, "leave",
                ))])
            }
            WindowEvent::MouseWheel { delta, phase, .. } => {
                let (x, y) = self.last_pointer_position;
                WinitTranslatedEvent::new(vec![DesktopHostEvent::PointerInput(PointerInput::new(
                    x,
                    y,
                    mouse_wheel_label(*delta, *phase),
                ))])
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let (x, y) = self.last_pointer_position;
                let suffix = if *state == ElementState::Pressed {
                    "down"
                } else {
                    "up"
                };
                WinitTranslatedEvent::new(vec![DesktopHostEvent::PointerInput(PointerInput::new(
                    x,
                    y,
                    format!("{}-{suffix}", mouse_button_label(*button)),
                ))])
            }
            WindowEvent::Occluded(occluded) => {
                WinitTranslatedEvent::new(vec![DesktopHostEvent::WindowOcclusionChanged(*occluded)])
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.metrics.scale_factor = *scale_factor;
                WinitTranslatedEvent::new(vec![
                    DesktopHostEvent::DpiChanged(*scale_factor),
                    DesktopHostEvent::RendererTargetRecreateRequested,
                ])
                .redraw()
            }
            WindowEvent::RedrawRequested => WinitTranslatedEvent::new(Vec::new()).redraw(),
            _ => WinitTranslatedEvent::new(Vec::new()),
        }
    }

    fn translate_resize(
        &mut self,
        physical_width: u32,
        physical_height: u32,
    ) -> WinitTranslatedEvent {
        let scale = self.metrics.scale_factor;
        self.metrics = SurfaceMetrics::new(
            f64::from(physical_width) / scale,
            f64::from(physical_height) / scale,
            scale,
        );
        WinitTranslatedEvent::new(vec![
            DesktopHostEvent::Resized(self.metrics),
            DesktopHostEvent::RendererTargetRecreateRequested,
        ])
        .redraw()
    }
}

fn ime_event_label(event: &Ime) -> String {
    match event {
        Ime::Enabled => "enabled".into(),
        Ime::Preedit(value, Some((start, end))) => format!("preedit:{value}:{start}..{end}"),
        Ime::Preedit(value, None) => format!("preedit:{value}:hidden"),
        Ime::Commit(value) => format!("commit:{value}"),
        Ime::Disabled => "disabled".into(),
    }
}

fn mouse_button_label(button: MouseButton) -> &'static str {
    match button {
        MouseButton::Left => "left",
        MouseButton::Right => "right",
        MouseButton::Middle => "middle",
        MouseButton::Back => "back",
        MouseButton::Forward => "forward",
        MouseButton::Other(_) => "other",
    }
}

fn mouse_wheel_label(delta: MouseScrollDelta, _phase: TouchPhase) -> String {
    match delta {
        MouseScrollDelta::LineDelta(x, y) => {
            format!("wheel-lines:{}:{}", compact_f32(x), compact_f32(y))
        }
        MouseScrollDelta::PixelDelta(position) => {
            format!(
                "wheel-pixels:{}:{}",
                compact_f64(position.x),
                compact_f64(position.y)
            )
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

fn compact_f64(value: f64) -> String {
    if value.fract() == 0.0 {
        #[allow(clippy::cast_possible_truncation)]
        {
            (value as i64).to_string()
        }
    } else {
        value.to_string()
    }
}

const WINIT_CAP_OWNS_WINDOW: u16 = 1 << 0;
const WINIT_CAP_CLOSE: u16 = 1 << 1;
const WINIT_CAP_MINIMIZE: u16 = 1 << 2;
const WINIT_CAP_MAXIMIZE: u16 = 1 << 3;
const WINIT_CAP_FULLSCREEN: u16 = 1 << 4;
const WINIT_CAP_FOCUS: u16 = 1 << 5;
const WINIT_CAP_KEYBOARD: u16 = 1 << 6;
const WINIT_CAP_POINTER: u16 = 1 << 7;
const WINIT_CAP_CLIPBOARD: u16 = 1 << 8;
const WINIT_CAP_RESIZE: u16 = 1 << 9;
const WINIT_CAP_REPAINT: u16 = 1 << 10;

/// Desktop Winit adapter error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WinitHostError {
    rule: String,
    message: String,
}

impl WinitHostError {
    /// Creates a Winit host error.
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

    /// Returns the diagnostic message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl From<WinitHostError> for Diagnostic {
    fn from(error: WinitHostError) -> Self {
        Self::error(error.rule, error.message)
    }
}

/// Headless-safe Winit desktop adapter.
#[derive(Clone, Debug, PartialEq)]
pub struct WinitDesktopAdapter {
    config: DesktopWindowConfig,
    platform: WinitPlatformFixture,
    capabilities: WinitHostCapabilities,
    logical_size: LogicalSize<f64>,
    mode: WindowMode,
    focused: bool,
    close_requested: bool,
    events: Vec<DesktopHostEvent>,
    repaint_requests: Vec<RepaintRequest>,
}

impl WinitDesktopAdapter {
    /// Creates a desktop window adapter from a platform fixture.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the platform handle cannot own a desktop window.
    pub fn create_window(
        config: DesktopWindowConfig,
        platform: WinitPlatformFixture,
    ) -> Result<Self, WinitHostError> {
        validate_desktop_metrics(config.metrics)?;
        platform
            .handle()
            .validate_for(SurfaceOwnership::DesktopWindow)
            .map_err(|diagnostic| WinitHostError::new(diagnostic.code, diagnostic.message))?;
        let logical_size =
            LogicalSize::new(config.metrics.logical_width, config.metrics.logical_height);
        Ok(Self {
            config: config.clone(),
            platform,
            capabilities: WinitHostCapabilities::desktop(),
            logical_size,
            mode: WindowMode::Normal,
            focused: false,
            close_requested: false,
            events: vec![DesktopHostEvent::WindowCreated(config)],
            repaint_requests: Vec::new(),
        })
    }

    /// Returns Winit-specific capabilities.
    #[must_use]
    pub const fn capabilities(&self) -> WinitHostCapabilities {
        self.capabilities
    }

    /// Returns current window mode.
    #[must_use]
    pub const fn mode(&self) -> WindowMode {
        self.mode
    }

    /// Returns whether the window is focused.
    #[must_use]
    pub const fn focused(&self) -> bool {
        self.focused
    }

    /// Returns whether close was requested.
    #[must_use]
    pub const fn close_requested(&self) -> bool {
        self.close_requested
    }

    /// Returns the platform handle.
    #[must_use]
    pub const fn platform_handle(&self) -> HostPlatformHandle {
        self.platform.handle()
    }

    /// Returns repaint requests.
    #[must_use]
    pub fn repaint_requests(&self) -> &[RepaintRequest] {
        &self.repaint_requests
    }

    /// Handles Winit resize events.
    pub fn handle_resize(&mut self, metrics: SurfaceMetrics) {
        let _ = self.try_handle_resize(metrics);
    }

    /// Handles Winit resize events and reports invalid metrics.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the resize metrics are not finite and positive.
    pub fn try_handle_resize(&mut self, metrics: SurfaceMetrics) -> Result<(), WinitHostError> {
        self.ensure_accepts_host_event()?;
        validate_desktop_metrics(metrics)?;
        self.config.metrics = metrics;
        self.logical_size = LogicalSize::new(metrics.logical_width, metrics.logical_height);
        self.events
            .push(DesktopHostEvent::RendererTargetRecreateRequested);
        Ok(())
    }

    /// Handles Winit DPI changes and reports invalid scale factors.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the resulting surface metrics are not finite and positive.
    pub fn try_dpi_changed(&mut self, scale_factor: f64) -> Result<(), WinitHostError> {
        self.ensure_accepts_host_event()?;
        let metrics = SurfaceMetrics::new(
            self.config.metrics.logical_width,
            self.config.metrics.logical_height,
            scale_factor,
        );
        validate_desktop_metrics(metrics)?;
        self.config.metrics.scale_factor = scale_factor;
        self.events.push(DesktopHostEvent::DpiChanged(scale_factor));
        self.events
            .push(DesktopHostEvent::RendererTargetRecreateRequested);
        Ok(())
    }

    /// Requests a repaint.
    pub fn request_repaint(&mut self, reason: impl Into<String>) {
        if !self.accepts_host_event() {
            return;
        }
        self.repaint_requests
            .push(RepaintRequest::full_surface(reason));
    }

    /// Drains host events.
    pub fn drain_events(&mut self) -> Vec<DesktopHostEvent> {
        std::mem::take(&mut self.events)
    }

    /// Executes a clipboard request through a native clipboard bridge.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the window is closed, the bridge denies the request, or the
    /// native clipboard backend fails.
    pub fn try_request_clipboard<B: WinitClipboardBackend>(
        &mut self,
        request: SurfaceClipboardRequest,
        bridge: &mut WinitClipboardBridge<B>,
    ) -> Result<WinitClipboardResponse, WinitHostError> {
        self.ensure_accepts_host_event()?;
        let response = bridge.handle_request(request.clone())?;
        self.events
            .push(DesktopHostEvent::ClipboardRequested(request));
        Ok(response)
    }

    /// Executes a native dialog request through a dialog bridge.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the window is closed, request validation fails, or the
    /// native dialog backend fails.
    pub fn try_request_dialog<B: WinitDialogBackend>(
        &mut self,
        request: DesktopDialogRequest,
        bridge: &mut WinitDialogBridge<B>,
    ) -> Result<DesktopDialogResponse, WinitHostError> {
        self.ensure_accepts_host_event()?;
        let response = bridge.handle_request(request.clone())?;
        self.events.push(DesktopHostEvent::DialogRequested(request));
        Ok(response)
    }

    fn set_mode(&mut self, mode: WindowMode) {
        if !self.accepts_host_event() || self.mode == mode {
            return;
        }
        self.mode = mode;
        self.events.push(DesktopHostEvent::ModeChanged(mode));
    }

    fn accepts_host_event(&self) -> bool {
        !self.close_requested
    }

    fn ensure_accepts_host_event(&self) -> Result<(), WinitHostError> {
        if self.accepts_host_event() {
            Ok(())
        } else {
            Err(WinitHostError::new(
                "desktop.window.closed",
                "desktop window has already received a close request",
            ))
        }
    }
}

impl DesktopHostAdapter for WinitDesktopAdapter {
    fn config(&self) -> &DesktopWindowConfig {
        &self.config
    }

    fn metrics(&self) -> SurfaceMetrics {
        self.config.metrics
    }

    fn request_minimize(&mut self, minimized: bool) {
        self.set_mode(if minimized {
            WindowMode::Minimized
        } else {
            WindowMode::Normal
        });
    }

    fn request_maximize(&mut self, maximized: bool) {
        self.set_mode(if maximized {
            WindowMode::Maximized
        } else {
            WindowMode::Normal
        });
    }

    fn request_fullscreen(&mut self, fullscreen: bool) {
        self.set_mode(if fullscreen {
            WindowMode::Fullscreen
        } else {
            WindowMode::Normal
        });
    }

    fn request_close(&mut self, reason: impl Into<String>) {
        if self.close_requested {
            return;
        }
        self.close_requested = true;
        self.events
            .push(DesktopHostEvent::CloseRequested(reason.into()));
    }

    fn set_focus(&mut self, focused: bool) {
        if !self.accepts_host_event() || self.focused == focused {
            return;
        }
        self.focused = focused;
        self.events.push(DesktopHostEvent::FocusChanged(focused));
    }

    fn keyboard_input(&mut self, input: KeyboardInput) {
        if !self.accepts_host_event() {
            return;
        }
        self.events.push(DesktopHostEvent::KeyboardInput(input));
    }

    fn pointer_input(&mut self, input: PointerInput) {
        if !self.accepts_host_event() {
            return;
        }
        self.events.push(DesktopHostEvent::PointerInput(input));
    }

    fn clipboard_available(&mut self, capability: ClipboardCapability) {
        if !self.accepts_host_event() || self.config.clipboard == capability {
            return;
        }
        self.config.clipboard = capability;
        self.events
            .push(DesktopHostEvent::ClipboardCapabilityChanged(capability));
    }

    fn dpi_changed(&mut self, scale_factor: f64) {
        let _ = self.try_dpi_changed(scale_factor);
    }
}

fn configure_file_dialog(
    title: String,
    directory: Option<&Path>,
    filters: Vec<DesktopDialogFileFilter>,
) -> rfd::FileDialog {
    let mut dialog = rfd::FileDialog::new().set_title(title);
    if let Some(directory) = directory {
        dialog = dialog.set_directory(directory);
    }
    for filter in filters {
        dialog = dialog.add_filter(filter.name, &filter.extensions);
    }
    dialog
}

fn rfd_message_level(level: DesktopDialogLevel) -> rfd::MessageLevel {
    match level {
        DesktopDialogLevel::Info => rfd::MessageLevel::Info,
        DesktopDialogLevel::Warning => rfd::MessageLevel::Warning,
        DesktopDialogLevel::Error => rfd::MessageLevel::Error,
    }
}

fn validate_dialog_title(title: &str) -> Result<(), WinitHostError> {
    if title.trim().is_empty() {
        Err(WinitHostError::new(
            "desktop.dialog.invalid-title",
            "desktop dialog title must not be empty",
        ))
    } else {
        Ok(())
    }
}

fn validate_dialog_message(message: &str) -> Result<(), WinitHostError> {
    if message.trim().is_empty() {
        Err(WinitHostError::new(
            "desktop.dialog.invalid-message",
            "desktop dialog message must not be empty",
        ))
    } else {
        Ok(())
    }
}

fn validate_dialog_file_name(file_name: &str) -> Result<(), WinitHostError> {
    if file_name.trim().is_empty()
        || file_name.contains('/')
        || file_name.contains('\\')
        || file_name.contains('\0')
    {
        Err(WinitHostError::new(
            "desktop.dialog.invalid-file-name",
            "desktop dialog file name must be a non-empty file name, not a path",
        ))
    } else {
        Ok(())
    }
}

fn validate_dialog_filters(filters: &[DesktopDialogFileFilter]) -> Result<(), WinitHostError> {
    for filter in filters {
        if filter.name.trim().is_empty() || filter.extensions.is_empty() {
            return Err(WinitHostError::new(
                "desktop.dialog.invalid-filter",
                "desktop dialog filters require a non-empty name and at least one extension",
            ));
        }
        for extension in &filter.extensions {
            if extension.trim().is_empty()
                || extension.starts_with('.')
                || extension.contains('/')
                || extension.contains('\\')
                || extension.contains('\0')
            {
                return Err(WinitHostError::new(
                    "desktop.dialog.invalid-filter",
                    "desktop dialog filter extensions must not be empty and must omit path separators and leading dots",
                ));
            }
        }
    }
    Ok(())
}

fn validate_desktop_metrics(metrics: SurfaceMetrics) -> Result<(), WinitHostError> {
    if metrics.logical_width.is_finite()
        && metrics.logical_height.is_finite()
        && metrics.scale_factor.is_finite()
        && metrics.logical_width > 0.0
        && metrics.logical_height > 0.0
        && metrics.scale_factor > 0.0
    {
        Ok(())
    } else {
        Err(WinitHostError::new(
            "desktop.window.invalid-size",
            "desktop window dimensions and scale factor must be finite and greater than zero",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-host-winit");
    }
}
