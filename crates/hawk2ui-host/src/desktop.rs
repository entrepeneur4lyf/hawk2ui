//! Desktop host lifecycle records.

use serde::{Deserialize, Serialize};

use crate::{
    HostCapabilities, HostSurface, RepaintRequest, SurfaceClipboardRequest, SurfaceMetrics,
    SurfaceWindowCommand, SurfaceWindowMode,
};

/// Clipboard capability exposed by a desktop host.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ClipboardCapability {
    /// Clipboard unavailable.
    None,
    /// Read-only clipboard access.
    Read,
    /// Write-only clipboard access.
    Write,
    /// Read/write clipboard access.
    ReadWrite,
}

/// Desktop window mode.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum WindowMode {
    /// Normal restored window.
    Normal,
    /// Minimized window.
    Minimized,
    /// Maximized window.
    Maximized,
    /// Fullscreen window.
    Fullscreen,
}

/// Desktop window creation configuration.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DesktopWindowConfig {
    /// Window title.
    pub title: String,
    /// Initial metrics.
    pub metrics: SurfaceMetrics,
    /// Clipboard capability.
    pub clipboard: ClipboardCapability,
}

impl DesktopWindowConfig {
    /// Creates desktop window configuration.
    #[must_use]
    pub fn new(title: impl Into<String>, metrics: SurfaceMetrics) -> Self {
        Self {
            title: title.into(),
            metrics,
            clipboard: ClipboardCapability::None,
        }
    }

    /// Sets clipboard capability.
    #[must_use]
    pub const fn with_clipboard(mut self, clipboard: ClipboardCapability) -> Self {
        self.clipboard = clipboard;
        self
    }
}

/// Keyboard input record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct KeyboardInput {
    /// Physical or logical key code.
    pub key: String,
    /// Whether the key is pressed.
    pub pressed: bool,
}

impl KeyboardInput {
    /// Creates a keyboard input record.
    #[must_use]
    pub fn new(key: impl Into<String>, pressed: bool) -> Self {
        Self {
            key: key.into(),
            pressed,
        }
    }
}

/// Pointer input record.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PointerInput {
    /// Logical x coordinate.
    pub x: f64,
    /// Logical y coordinate.
    pub y: f64,
    /// Pointer button or pointer action label.
    pub button: String,
}

impl PointerInput {
    /// Creates a pointer input record.
    #[must_use]
    pub fn new(x: f64, y: f64, button: impl Into<String>) -> Self {
        Self {
            x,
            y,
            button: button.into(),
        }
    }
}

/// Desktop host event.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum DesktopHostEvent {
    /// Window was created.
    WindowCreated(DesktopWindowConfig),
    /// Close was requested.
    CloseRequested(String),
    /// Window mode changed.
    ModeChanged(WindowMode),
    /// Focus changed.
    FocusChanged(bool),
    /// Keyboard input.
    KeyboardInput(KeyboardInput),
    /// Pointer input.
    PointerInput(PointerInput),
    /// Clipboard capability changed.
    ClipboardCapabilityChanged(ClipboardCapability),
    /// DPI scale changed.
    DpiChanged(f64),
    /// Renderer target must be recreated.
    RendererTargetRecreateRequested,
    /// Explicit repaint was requested.
    RepaintRequested(RepaintRequest),
    /// Surface metrics changed.
    Resized(SurfaceMetrics),
    /// Clipboard operation was requested.
    ClipboardRequested(SurfaceClipboardRequest),
    /// Frame was presented to the host surface.
    FramePresented {
        /// Monotonic frame identifier.
        frame_id: u64,
        /// Surface metrics used for presentation.
        metrics: SurfaceMetrics,
    },
}

/// Desktop host adapter contract.
pub trait DesktopHostAdapter {
    /// Returns desktop window configuration.
    fn config(&self) -> &DesktopWindowConfig;

    /// Returns current surface metrics.
    fn metrics(&self) -> SurfaceMetrics;

    /// Requests minimized state.
    fn request_minimize(&mut self, minimized: bool);

    /// Requests maximized state.
    fn request_maximize(&mut self, maximized: bool);

    /// Requests fullscreen state.
    fn request_fullscreen(&mut self, fullscreen: bool);

    /// Requests window close.
    fn request_close(&mut self, reason: impl Into<String>);

    /// Sets focus.
    fn set_focus(&mut self, focused: bool);

    /// Records keyboard input.
    fn keyboard_input(&mut self, input: KeyboardInput);

    /// Records pointer input.
    fn pointer_input(&mut self, input: PointerInput);

    /// Updates clipboard capability.
    fn clipboard_available(&mut self, capability: ClipboardCapability);

    /// Updates DPI scale and requests renderer target recreation.
    fn dpi_changed(&mut self, scale_factor: f64);
}

/// Recording desktop adapter for deterministic tests.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RecordingDesktopAdapter {
    config: DesktopWindowConfig,
    capabilities: HostCapabilities,
    mode: WindowMode,
    focused: bool,
    events: Vec<DesktopHostEvent>,
}

impl RecordingDesktopAdapter {
    /// Creates a recording adapter and records window creation.
    #[must_use]
    pub fn create_window(config: DesktopWindowConfig) -> Self {
        Self {
            config: config.clone(),
            capabilities: HostCapabilities::desktop(),
            mode: WindowMode::Normal,
            focused: false,
            events: vec![DesktopHostEvent::WindowCreated(config)],
        }
    }

    /// Drains recorded desktop host events.
    pub fn drain_events(&mut self) -> Vec<DesktopHostEvent> {
        std::mem::take(&mut self.events)
    }

    /// Returns current surface metrics.
    #[must_use]
    pub const fn metrics(&self) -> SurfaceMetrics {
        self.config.metrics
    }

    /// Sets focus.
    pub fn set_focus(&mut self, focused: bool) {
        self.focused = focused;
        self.events.push(DesktopHostEvent::FocusChanged(focused));
    }

    fn set_mode(&mut self, mode: WindowMode) {
        self.mode = mode;
        self.events.push(DesktopHostEvent::ModeChanged(mode));
    }
}

impl HostSurface for RecordingDesktopAdapter {
    fn metrics(&self) -> SurfaceMetrics {
        self.config.metrics
    }

    fn capabilities(&self) -> &HostCapabilities {
        &self.capabilities
    }

    fn has_focus(&self) -> bool {
        self.focused
    }

    fn set_focus(&mut self, focused: bool) {
        DesktopHostAdapter::set_focus(self, focused);
    }

    fn request_repaint(&mut self, request: RepaintRequest) {
        self.events
            .push(DesktopHostEvent::RepaintRequested(request));
    }

    fn resize(&mut self, metrics: SurfaceMetrics) {
        self.config.metrics = metrics;
        self.events.push(DesktopHostEvent::Resized(metrics));
        self.events
            .push(DesktopHostEvent::RendererTargetRecreateRequested);
    }

    fn request_window_command(&mut self, command: SurfaceWindowCommand) {
        match command {
            SurfaceWindowCommand::SetMode(SurfaceWindowMode::Normal) => {
                self.set_mode(WindowMode::Normal);
            }
            SurfaceWindowCommand::SetMode(SurfaceWindowMode::Minimized) => {
                self.set_mode(WindowMode::Minimized);
            }
            SurfaceWindowCommand::SetMode(SurfaceWindowMode::Maximized) => {
                self.set_mode(WindowMode::Maximized);
            }
            SurfaceWindowCommand::SetMode(SurfaceWindowMode::Fullscreen) => {
                self.set_mode(WindowMode::Fullscreen);
            }
            SurfaceWindowCommand::Close(reason) => {
                DesktopHostAdapter::request_close(self, reason);
            }
        }
    }

    fn request_clipboard(&mut self, request: SurfaceClipboardRequest) {
        self.events
            .push(DesktopHostEvent::ClipboardRequested(request));
    }

    fn record_presented_frame(&mut self, frame_id: u64) {
        self.events.push(DesktopHostEvent::FramePresented {
            frame_id,
            metrics: self.config.metrics,
        });
    }

    fn teardown(&mut self, reason: impl Into<String>) {
        DesktopHostAdapter::request_close(self, reason);
    }
}

impl DesktopHostAdapter for RecordingDesktopAdapter {
    fn config(&self) -> &DesktopWindowConfig {
        &self.config
    }

    fn metrics(&self) -> SurfaceMetrics {
        self.metrics()
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
        self.events
            .push(DesktopHostEvent::CloseRequested(reason.into()));
    }

    fn set_focus(&mut self, focused: bool) {
        self.set_focus(focused);
    }

    fn keyboard_input(&mut self, input: KeyboardInput) {
        self.events.push(DesktopHostEvent::KeyboardInput(input));
    }

    fn pointer_input(&mut self, input: PointerInput) {
        self.events.push(DesktopHostEvent::PointerInput(input));
    }

    fn clipboard_available(&mut self, capability: ClipboardCapability) {
        self.config.clipboard = capability;
        self.events
            .push(DesktopHostEvent::ClipboardCapabilityChanged(capability));
    }

    fn dpi_changed(&mut self, scale_factor: f64) {
        self.config.metrics.scale_factor = scale_factor;
        self.events.push(DesktopHostEvent::DpiChanged(scale_factor));
        self.events
            .push(DesktopHostEvent::RendererTargetRecreateRequested);
    }
}
