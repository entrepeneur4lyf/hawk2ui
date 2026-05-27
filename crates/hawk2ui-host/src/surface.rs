//! Common host surface contract.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// Surface metrics shared by desktop and embedded plugin hosts.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct SurfaceMetrics {
    /// Logical width in points.
    pub logical_width: f64,
    /// Logical height in points.
    pub logical_height: f64,
    /// Device scale factor.
    pub scale_factor: f64,
}

impl SurfaceMetrics {
    /// Creates surface metrics.
    #[must_use]
    pub const fn new(logical_width: f64, logical_height: f64, scale_factor: f64) -> Self {
        Self {
            logical_width,
            logical_height,
            scale_factor,
        }
    }

    /// Returns physical pixel size rounded to the nearest pixel.
    #[must_use]
    pub fn physical_size(&self) -> (u32, u32) {
        (
            scaled_physical_dimension(self.logical_width, self.scale_factor),
            scaled_physical_dimension(self.logical_height, self.scale_factor),
        )
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn scaled_physical_dimension(logical: f64, scale_factor: f64) -> u32 {
    let scaled = (logical.max(0.0) * scale_factor.max(0.0)).round();
    if !scaled.is_finite() {
        0
    } else if scaled >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        scaled as u32
    }
}

/// Single host surface capability.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum HostCapability {
    /// Host owns the native top-level window.
    OwnsWindow,
    /// Host can request process/application quit.
    RequestQuit,
    /// Host can read from the clipboard.
    ClipboardRead,
    /// Host can write to the clipboard.
    ClipboardWrite,
    /// Host supports resize requests.
    Resize,
    /// Host supports focus requests.
    Focus,
    /// Host supports input routing.
    InputRouting,
    /// Host supports window state requests such as minimize, maximize, and fullscreen.
    WindowState,
    /// Host reports frame presentation timing.
    PresentationTiming,
}

/// Host surface capabilities.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HostCapabilities {
    capabilities: BTreeSet<HostCapability>,
}

impl HostCapabilities {
    /// Desktop host capabilities.
    #[must_use]
    pub fn desktop() -> Self {
        Self {
            capabilities: BTreeSet::from([
                HostCapability::OwnsWindow,
                HostCapability::RequestQuit,
                HostCapability::ClipboardRead,
                HostCapability::ClipboardWrite,
                HostCapability::Resize,
                HostCapability::Focus,
                HostCapability::InputRouting,
                HostCapability::WindowState,
                HostCapability::PresentationTiming,
            ]),
        }
    }

    /// Embedded plugin host capabilities.
    #[must_use]
    pub fn plugin() -> Self {
        Self {
            capabilities: BTreeSet::from([
                HostCapability::Resize,
                HostCapability::Focus,
                HostCapability::InputRouting,
                HostCapability::PresentationTiming,
            ]),
        }
    }

    /// Returns whether the capability is present.
    #[must_use]
    pub fn supports(&self, capability: HostCapability) -> bool {
        self.capabilities.contains(&capability)
    }

    /// Returns all capabilities in deterministic order.
    #[must_use]
    pub fn all(&self) -> &BTreeSet<HostCapability> {
        &self.capabilities
    }
}

/// Repaint request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepaintRequest {
    /// Optional dirty rectangle in physical pixels: x, y, width, height.
    pub dirty_rect: Option<(u32, u32, u32, u32)>,
    /// Diagnostic reason for the repaint.
    pub reason: String,
}

impl RepaintRequest {
    /// Creates a repaint request for the full surface.
    #[must_use]
    pub fn full_surface(reason: impl Into<String>) -> Self {
        Self {
            dirty_rect: None,
            reason: reason.into(),
        }
    }
}

/// Common host window mode request.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SurfaceWindowMode {
    /// Normal restored window.
    Normal,
    /// Minimized window.
    Minimized,
    /// Maximized window.
    Maximized,
    /// Fullscreen window.
    Fullscreen,
}

/// Common host window command.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SurfaceWindowCommand {
    /// Request a host-owned window mode change.
    SetMode(SurfaceWindowMode),
    /// Request host surface close or editor destruction.
    Close(String),
}

/// Common clipboard operation requested through a host surface.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SurfaceClipboardRequest {
    /// Read the host clipboard.
    Read,
    /// Write text to the host clipboard.
    Write(String),
    /// Clear host clipboard text owned by the app when supported.
    Clear,
}

/// Host surface event.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum SurfaceEvent {
    /// Focus changed.
    FocusChanged(bool),
    /// Repaint requested.
    RepaintRequested(RepaintRequest),
    /// Surface resized.
    Resized(SurfaceMetrics),
    /// Window command was requested.
    WindowCommandRequested(SurfaceWindowCommand),
    /// Clipboard operation was requested.
    ClipboardRequested(SurfaceClipboardRequest),
    /// Frame presentation completed for the current metrics.
    FramePresented {
        /// Monotonic frame identifier.
        frame_id: u64,
        /// Surface metrics used for presentation.
        metrics: SurfaceMetrics,
    },
    /// Surface teardown requested.
    TeardownRequested(String),
}

/// Common host surface API.
pub trait HostSurface {
    /// Returns current surface metrics.
    fn metrics(&self) -> SurfaceMetrics;

    /// Returns host capabilities.
    fn capabilities(&self) -> &HostCapabilities;

    /// Returns whether the surface has focus.
    fn has_focus(&self) -> bool;

    /// Requests or records focus state.
    fn set_focus(&mut self, focused: bool);

    /// Requests a repaint.
    fn request_repaint(&mut self, request: RepaintRequest);

    /// Updates surface metrics after resize or DPI change.
    fn resize(&mut self, metrics: SurfaceMetrics);

    /// Requests a host window command.
    fn request_window_command(&mut self, command: SurfaceWindowCommand);

    /// Requests a clipboard operation through the host.
    fn request_clipboard(&mut self, request: SurfaceClipboardRequest);

    /// Records frame presentation for the current surface metrics.
    fn record_presented_frame(&mut self, frame_id: u64);

    /// Requests teardown.
    fn teardown(&mut self, reason: impl Into<String>);
}

/// Recording host surface for deterministic tests.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RecordingHostSurface {
    metrics: SurfaceMetrics,
    capabilities: HostCapabilities,
    focused: bool,
    events: Vec<SurfaceEvent>,
}

impl RecordingHostSurface {
    /// Creates a recording host surface.
    #[must_use]
    pub fn new(metrics: SurfaceMetrics, capabilities: HostCapabilities) -> Self {
        Self {
            metrics,
            capabilities,
            focused: false,
            events: Vec::new(),
        }
    }

    /// Drains recorded surface events.
    pub fn drain_events(&mut self) -> Vec<SurfaceEvent> {
        std::mem::take(&mut self.events)
    }
}

impl HostSurface for RecordingHostSurface {
    fn metrics(&self) -> SurfaceMetrics {
        self.metrics
    }

    fn capabilities(&self) -> &HostCapabilities {
        &self.capabilities
    }

    fn has_focus(&self) -> bool {
        self.focused
    }

    fn set_focus(&mut self, focused: bool) {
        self.focused = focused;
        self.events.push(SurfaceEvent::FocusChanged(focused));
    }

    fn request_repaint(&mut self, request: RepaintRequest) {
        self.events.push(SurfaceEvent::RepaintRequested(request));
    }

    fn resize(&mut self, metrics: SurfaceMetrics) {
        self.metrics = metrics;
        self.events.push(SurfaceEvent::Resized(metrics));
    }

    fn request_window_command(&mut self, command: SurfaceWindowCommand) {
        self.events
            .push(SurfaceEvent::WindowCommandRequested(command));
    }

    fn request_clipboard(&mut self, request: SurfaceClipboardRequest) {
        self.events.push(SurfaceEvent::ClipboardRequested(request));
    }

    fn record_presented_frame(&mut self, frame_id: u64) {
        self.events.push(SurfaceEvent::FramePresented {
            frame_id,
            metrics: self.metrics,
        });
    }

    fn teardown(&mut self, reason: impl Into<String>) {
        self.events
            .push(SurfaceEvent::TeardownRequested(reason.into()));
    }
}

/// Presented frame record.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PresentedFrame {
    /// Monotonic frame identifier.
    pub frame_id: u64,
    /// Surface metrics used for presentation.
    pub metrics: SurfaceMetrics,
}

/// Frame presentation contract.
pub trait FramePresenter {
    /// Presents a frame to the current host surface.
    fn present_frame(&mut self, frame_id: u64, metrics: SurfaceMetrics);
}

/// Recording frame presenter for deterministic tests.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct RecordingFramePresenter {
    presented_frames: Vec<PresentedFrame>,
}

impl RecordingFramePresenter {
    /// Returns presented frames in order.
    #[must_use]
    pub fn presented_frames(&self) -> &[PresentedFrame] {
        &self.presented_frames
    }
}

impl FramePresenter for RecordingFramePresenter {
    fn present_frame(&mut self, frame_id: u64, metrics: SurfaceMetrics) {
        self.presented_frames
            .push(PresentedFrame { frame_id, metrics });
    }
}
