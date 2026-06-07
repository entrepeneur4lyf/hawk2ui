//! Plugin host lifecycle records.

use serde::{Deserialize, Serialize};

use crate::{
    KeyboardInput, PointerInput, SurfaceClipboardRequest, SurfaceMetrics, SurfaceWindowCommand,
};

/// Plugin parent handle record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PluginParentHandle {
    /// Opaque host-provided parent identifier.
    pub id: String,
}

impl PluginParentHandle {
    /// Creates an opaque plugin parent handle.
    #[must_use]
    pub fn opaque(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

/// Plugin editor creation configuration.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PluginEditorConfig {
    /// Editor identifier.
    pub editor_id: String,
    /// Host parent handle.
    pub parent: PluginParentHandle,
    /// Initial editor metrics.
    pub metrics: SurfaceMetrics,
}

impl PluginEditorConfig {
    /// Creates plugin editor configuration.
    #[must_use]
    pub fn new(
        editor_id: impl Into<String>,
        parent: PluginParentHandle,
        metrics: SurfaceMetrics,
    ) -> Self {
        Self {
            editor_id: editor_id.into(),
            parent,
            metrics,
        }
    }
}

/// Plugin host lifecycle event.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum PluginHostEvent {
    /// Parent was attached.
    ParentAttached(PluginParentHandle),
    /// Editor was created.
    EditorCreated(String),
    /// Editor was shown by the host.
    EditorShown(String),
    /// Editor was hidden by the host.
    EditorHidden(String),
    /// Host resized the editor.
    HostResize(SurfaceMetrics),
    /// DPI scale changed.
    DpiChanged(f64),
    /// Repaint was scheduled.
    RepaintScheduled(String),
    /// Focus input was routed.
    FocusRouted(bool),
    /// Keyboard input was routed.
    KeyboardRouted(KeyboardInput),
    /// Pointer input was routed.
    PointerRouted(PointerInput),
    /// Editor was destroyed.
    EditorDestroyed(String),
    /// Safe teardown completed.
    SafeTeardownComplete,
    /// Window command was rejected because plugin editors cannot own top-level windows.
    WindowCommandRejected(SurfaceWindowCommand),
    /// Clipboard operation was requested through the plugin host.
    ClipboardRequested(SurfaceClipboardRequest),
    /// Frame was presented to the plugin editor surface.
    FramePresented {
        /// Monotonic frame identifier.
        frame_id: u64,
        /// Surface metrics used for presentation.
        metrics: SurfaceMetrics,
    },
}

/// Plugin host adapter contract.
pub trait PluginHostAdapter {
    /// Returns current metrics.
    fn metrics(&self) -> SurfaceMetrics;

    /// Handles host-driven resize.
    fn host_resize(&mut self, metrics: SurfaceMetrics);

    /// Handles DPI changes.
    fn dpi_changed(&mut self, scale_factor: f64);

    /// Schedules repaint through the host.
    fn schedule_repaint(&mut self, reason: impl Into<String>);

    /// Routes focus to the plugin editor.
    fn route_focus(&mut self, focused: bool);

    /// Routes keyboard input to the plugin editor.
    fn route_keyboard(&mut self, input: KeyboardInput);

    /// Routes pointer input to the plugin editor.
    fn route_pointer(&mut self, input: PointerInput);

    /// Destroys the plugin editor safely.
    fn destroy_editor(&mut self, reason: impl Into<String>);
}
