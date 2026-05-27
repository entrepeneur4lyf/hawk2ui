//! Plugin host lifecycle records.

use serde::{Deserialize, Serialize};

use crate::{KeyboardInput, PointerInput, SurfaceMetrics};

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

/// Recording plugin adapter for deterministic tests.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RecordingPluginAdapter {
    config: PluginEditorConfig,
    requested_process_quit: bool,
    destroyed: bool,
    events: Vec<PluginHostEvent>,
}

impl RecordingPluginAdapter {
    /// Attaches a plugin editor to a host parent.
    #[must_use]
    pub fn attach(config: PluginEditorConfig) -> Self {
        Self {
            events: vec![
                PluginHostEvent::ParentAttached(config.parent.clone()),
                PluginHostEvent::EditorCreated(config.editor_id.clone()),
            ],
            config,
            requested_process_quit: false,
            destroyed: false,
        }
    }

    /// Returns whether the adapter requested process quit.
    #[must_use]
    pub const fn requested_process_quit(&self) -> bool {
        self.requested_process_quit
    }

    /// Drains recorded plugin host events.
    pub fn drain_events(&mut self) -> Vec<PluginHostEvent> {
        std::mem::take(&mut self.events)
    }

    fn accepts_host_event(&self) -> bool {
        !self.destroyed
    }
}

impl PluginHostAdapter for RecordingPluginAdapter {
    fn metrics(&self) -> SurfaceMetrics {
        self.config.metrics
    }

    fn host_resize(&mut self, metrics: SurfaceMetrics) {
        if !self.accepts_host_event() {
            return;
        }
        self.config.metrics = metrics;
        self.events.push(PluginHostEvent::HostResize(metrics));
    }

    fn dpi_changed(&mut self, scale_factor: f64) {
        if !self.accepts_host_event() {
            return;
        }
        self.config.metrics.scale_factor = scale_factor;
        self.events.push(PluginHostEvent::DpiChanged(scale_factor));
    }

    fn schedule_repaint(&mut self, reason: impl Into<String>) {
        if !self.accepts_host_event() {
            return;
        }
        self.events
            .push(PluginHostEvent::RepaintScheduled(reason.into()));
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
            self.events
                .push(PluginHostEvent::EditorDestroyed(reason.into()));
            self.events.push(PluginHostEvent::SafeTeardownComplete);
        }
    }
}
