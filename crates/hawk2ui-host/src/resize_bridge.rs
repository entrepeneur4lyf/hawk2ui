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
            | SurfaceEvent::TeardownRequested(_) => None,
        }
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
            DesktopHostEvent::WindowCreated(_)
            | DesktopHostEvent::CloseRequested(_)
            | DesktopHostEvent::ModeChanged(_)
            | DesktopHostEvent::FocusChanged(_)
            | DesktopHostEvent::KeyboardInput(_)
            | DesktopHostEvent::PointerInput(_)
            | DesktopHostEvent::ClipboardCapabilityChanged(_) => None,
        }
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
            | PluginHostEvent::RepaintScheduled(_)
            | PluginHostEvent::FocusRouted(_)
            | PluginHostEvent::KeyboardRouted(_)
            | PluginHostEvent::PointerRouted(_)
            | PluginHostEvent::EditorDestroyed(_)
            | PluginHostEvent::SafeTeardownComplete => None,
        }
    }
}
