//! Common host surface contract.

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
            (self.logical_width * self.scale_factor).round() as u32,
            (self.logical_height * self.scale_factor).round() as u32,
        )
    }
}

/// Host surface capabilities.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HostCapabilities {
    /// Host owns the native top-level window.
    pub owns_window: bool,
    /// Host can request process/application quit.
    pub can_request_quit: bool,
    /// Host can read from the clipboard.
    pub clipboard_read: bool,
    /// Host can write to the clipboard.
    pub clipboard_write: bool,
    /// Host supports resize requests.
    pub resize: bool,
    /// Host supports focus requests.
    pub focus: bool,
}

impl HostCapabilities {
    /// Desktop host capabilities.
    #[must_use]
    pub const fn desktop() -> Self {
        Self {
            owns_window: true,
            can_request_quit: true,
            clipboard_read: true,
            clipboard_write: true,
            resize: true,
            focus: true,
        }
    }

    /// Embedded plugin host capabilities.
    #[must_use]
    pub const fn plugin() -> Self {
        Self {
            owns_window: false,
            can_request_quit: false,
            clipboard_read: false,
            clipboard_write: false,
            resize: false,
            focus: true,
        }
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

/// Host surface event.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum SurfaceEvent {
    /// Focus changed.
    FocusChanged(bool),
    /// Repaint requested.
    RepaintRequested(RepaintRequest),
    /// Surface resized.
    Resized(SurfaceMetrics),
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
