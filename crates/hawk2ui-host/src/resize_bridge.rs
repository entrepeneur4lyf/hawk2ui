//! Bridge host resize and DPI events to renderer target recreation.

use serde::{Deserialize, Serialize};

use crate::{DesktopHostEvent, PluginHostEvent, SurfaceEvent, SurfaceMetrics, WindowMode};

/// Renderer target recreation request.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RendererTargetRequest {
    /// Metrics for the recreated renderer target.
    pub metrics: SurfaceMetrics,
    /// Diagnostic reason.
    pub reason: String,
    /// Whether the bridge must force a redraw after recreation.
    pub force_redraw: bool,
}

impl RendererTargetRequest {
    /// Creates a renderer target recreation request.
    #[must_use]
    pub fn recreate(metrics: SurfaceMetrics, reason: impl Into<String>) -> Self {
        Self {
            metrics,
            reason: reason.into(),
            force_redraw: true,
        }
    }
}

/// Combined host update request for layout invalidation and renderer target recreation.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HostSurfaceUpdateRequest {
    /// Metrics that should drive layout and renderer target sizing.
    pub metrics: SurfaceMetrics,
    /// Renderer target request for the same host change.
    pub renderer_target: RendererTargetRequest,
    /// Diagnostic reason.
    pub reason: String,
    /// Whether the retained layout tree must be recomputed.
    pub invalidate_layout: bool,
}

impl HostSurfaceUpdateRequest {
    /// Creates a full host surface update request.
    #[must_use]
    pub fn new(
        metrics: SurfaceMetrics,
        renderer_target: RendererTargetRequest,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            metrics,
            renderer_target,
            reason: reason.into(),
            invalidate_layout: true,
        }
    }

    /// Returns logical viewport dimensions.
    #[must_use]
    pub const fn logical_viewport(&self) -> (f64, f64) {
        (self.metrics.logical_width, self.metrics.logical_height)
    }

    /// Returns physical target dimensions.
    #[must_use]
    pub fn physical_size(&self) -> (u32, u32) {
        self.metrics.physical_size()
    }
}

/// Renderer resize bridge.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RendererResizeBridge;

impl RendererResizeBridge {
    /// Converts a common surface event into a renderer target request.
    #[must_use]
    pub fn surface_event_to_target_request(
        &self,
        event: &SurfaceEvent,
    ) -> Option<RendererTargetRequest> {
        match event {
            SurfaceEvent::Resized(metrics) => {
                Some(RendererTargetRequest::recreate(*metrics, "surface resized"))
            }
            SurfaceEvent::FocusChanged(_)
            | SurfaceEvent::RepaintRequested(_)
            | SurfaceEvent::WindowCommandRequested(_)
            | SurfaceEvent::ClipboardRequested(_)
            | SurfaceEvent::FramePresented { .. }
            | SurfaceEvent::TeardownRequested(_) => None,
        }
    }

    /// Converts a common surface event into a layout invalidation and renderer target request.
    #[must_use]
    pub fn surface_event_to_update_request(
        &self,
        event: &SurfaceEvent,
    ) -> Option<HostSurfaceUpdateRequest> {
        let request = self.surface_event_to_target_request(event)?;
        Some(HostSurfaceUpdateRequest::new(
            request.metrics,
            request.clone(),
            request.reason,
        ))
    }

    /// Converts a desktop host event into a renderer target request.
    #[must_use]
    pub fn desktop_event_to_target_request(
        &self,
        event: &DesktopHostEvent,
        current_metrics: SurfaceMetrics,
    ) -> Option<RendererTargetRequest> {
        match event {
            DesktopHostEvent::ModeChanged(
                mode @ (WindowMode::Maximized | WindowMode::Fullscreen),
            ) => Some(RendererTargetRequest::recreate(
                current_metrics,
                format!("desktop window mode changed: {mode:?}"),
            )),
            DesktopHostEvent::DpiChanged(scale_factor) => Some(RendererTargetRequest::recreate(
                current_metrics,
                format!("desktop DPI changed to {scale_factor}"),
            )),
            DesktopHostEvent::RendererTargetRecreateRequested => {
                Some(RendererTargetRequest::recreate(
                    current_metrics,
                    "desktop requested target recreation",
                ))
            }
            DesktopHostEvent::Resized(metrics) => Some(RendererTargetRequest::recreate(
                *metrics,
                "desktop surface resized",
            )),
            DesktopHostEvent::WindowCreated(_)
            | DesktopHostEvent::CloseRequested(_)
            | DesktopHostEvent::ModeChanged(_)
            | DesktopHostEvent::FocusChanged(_)
            | DesktopHostEvent::KeyboardInput(_)
            | DesktopHostEvent::PointerInput(_)
            | DesktopHostEvent::ImeInput(_)
            | DesktopHostEvent::FileDragDrop(_)
            | DesktopHostEvent::WindowOcclusionChanged(_)
            | DesktopHostEvent::ClipboardCapabilityChanged(_)
            | DesktopHostEvent::RepaintRequested(_)
            | DesktopHostEvent::ClipboardRequested(_)
            | DesktopHostEvent::DialogRequested(_)
            | DesktopHostEvent::FramePresented { .. } => None,
        }
    }

    /// Converts a desktop host event into a layout invalidation and renderer target request.
    #[must_use]
    pub fn desktop_event_to_update_request(
        &self,
        event: &DesktopHostEvent,
        current_metrics: SurfaceMetrics,
    ) -> Option<HostSurfaceUpdateRequest> {
        let request = self.desktop_event_to_target_request(event, current_metrics)?;
        Some(HostSurfaceUpdateRequest::new(
            request.metrics,
            request.clone(),
            request.reason,
        ))
    }

    /// Converts a plugin host event into a renderer target request.
    #[must_use]
    pub fn plugin_event_to_target_request(
        &self,
        event: &PluginHostEvent,
        current_metrics: SurfaceMetrics,
    ) -> Option<RendererTargetRequest> {
        match event {
            PluginHostEvent::HostResize(metrics) => Some(RendererTargetRequest::recreate(
                *metrics,
                "plugin host resized editor",
            )),
            PluginHostEvent::DpiChanged(scale_factor) => Some(RendererTargetRequest::recreate(
                current_metrics,
                format!("plugin DPI changed to {scale_factor}"),
            )),
            PluginHostEvent::ParentAttached(_)
            | PluginHostEvent::EditorCreated(_)
            | PluginHostEvent::EditorShown(_)
            | PluginHostEvent::EditorHidden(_)
            | PluginHostEvent::RepaintScheduled(_)
            | PluginHostEvent::FocusRouted(_)
            | PluginHostEvent::KeyboardRouted(_)
            | PluginHostEvent::PointerRouted(_)
            | PluginHostEvent::EditorDestroyed(_)
            | PluginHostEvent::SafeTeardownComplete
            | PluginHostEvent::WindowCommandRejected(_)
            | PluginHostEvent::ClipboardRequested(_)
            | PluginHostEvent::FramePresented { .. } => None,
        }
    }

    /// Converts a plugin host event into a layout invalidation and renderer target request.
    #[must_use]
    pub fn plugin_event_to_update_request(
        &self,
        event: &PluginHostEvent,
        current_metrics: SurfaceMetrics,
    ) -> Option<HostSurfaceUpdateRequest> {
        let request = self.plugin_event_to_target_request(event, current_metrics)?;
        Some(HostSurfaceUpdateRequest::new(
            request.metrics,
            request.clone(),
            request.reason,
        ))
    }
}
