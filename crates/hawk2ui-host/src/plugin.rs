//! Plugin host lifecycle records.

use serde::{Deserialize, Serialize};

use crate::{
    HostCapabilities, HostSurface, KeyboardInput, PointerInput, RepaintRequest,
    SurfaceClipboardRequest, SurfaceMetrics, SurfaceWindowCommand,
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

/// Recording plugin adapter for deterministic tests.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RecordingPluginAdapter {
    config: PluginEditorConfig,
    capabilities: HostCapabilities,
    focus: PluginEditorFocus,
    visibility: PluginEditorVisibility,
    lifetime: PluginEditorLifetime,
    events: Vec<PluginHostEvent>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum PluginEditorFocus {
    Focused,
    Unfocused,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum PluginEditorVisibility {
    Visible,
    Hidden,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum PluginEditorLifetime {
    Live,
    Destroyed,
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
            capabilities: HostCapabilities::plugin(),
            focus: PluginEditorFocus::Unfocused,
            visibility: PluginEditorVisibility::Visible,
            lifetime: PluginEditorLifetime::Live,
        }
    }

    /// Drains recorded plugin host events.
    pub fn drain_events(&mut self) -> Vec<PluginHostEvent> {
        std::mem::take(&mut self.events)
    }

    /// Returns current editor metrics.
    #[must_use]
    pub const fn metrics(&self) -> SurfaceMetrics {
        self.config.metrics
    }

    /// Returns whether the editor is currently visible.
    #[must_use]
    pub const fn visible(&self) -> bool {
        matches!(self.visibility, PluginEditorVisibility::Visible)
    }

    /// Records a host-driven editor show request.
    pub fn show_editor(&mut self, reason: impl Into<String>) {
        if self.accepts_host_event() && !self.visible() {
            self.visibility = PluginEditorVisibility::Visible;
            self.events
                .push(PluginHostEvent::EditorShown(reason.into()));
        }
    }

    /// Records a host-driven editor hide request.
    pub fn hide_editor(&mut self, reason: impl Into<String>) {
        if self.accepts_host_event() && self.visible() {
            self.visibility = PluginEditorVisibility::Hidden;
            self.focus = PluginEditorFocus::Unfocused;
            self.events
                .push(PluginHostEvent::EditorHidden(reason.into()));
            self.events.push(PluginHostEvent::FocusRouted(false));
        }
    }

    fn accepts_host_event(&self) -> bool {
        matches!(self.lifetime, PluginEditorLifetime::Live)
    }
}

impl HostSurface for RecordingPluginAdapter {
    fn metrics(&self) -> SurfaceMetrics {
        self.config.metrics
    }

    fn capabilities(&self) -> &HostCapabilities {
        &self.capabilities
    }

    fn has_focus(&self) -> bool {
        matches!(self.focus, PluginEditorFocus::Focused)
    }

    fn set_focus(&mut self, focused: bool) {
        self.route_focus(focused);
    }

    fn request_repaint(&mut self, request: RepaintRequest) {
        self.schedule_repaint(request.reason);
    }

    fn resize(&mut self, metrics: SurfaceMetrics) {
        self.host_resize(metrics);
    }

    fn request_window_command(&mut self, command: SurfaceWindowCommand) {
        match command {
            SurfaceWindowCommand::Close(reason) => self.destroy_editor(reason),
            SurfaceWindowCommand::SetMode(mode) if self.accepts_host_event() => self.events.push(
                PluginHostEvent::WindowCommandRejected(SurfaceWindowCommand::SetMode(mode)),
            ),
            SurfaceWindowCommand::SetMode(_) => {}
        }
    }

    fn request_clipboard(&mut self, request: SurfaceClipboardRequest) {
        if self.accepts_host_event() {
            self.events
                .push(PluginHostEvent::ClipboardRequested(request));
        }
    }

    fn record_presented_frame(&mut self, frame_id: u64) {
        if self.accepts_host_event() {
            self.events.push(PluginHostEvent::FramePresented {
                frame_id,
                metrics: self.config.metrics,
            });
        }
    }

    fn teardown(&mut self, reason: impl Into<String>) {
        self.destroy_editor(reason);
    }
}

impl PluginHostAdapter for RecordingPluginAdapter {
    fn metrics(&self) -> SurfaceMetrics {
        self.metrics()
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
        self.focus = if focused {
            PluginEditorFocus::Focused
        } else {
            PluginEditorFocus::Unfocused
        };
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
        if self.accepts_host_event() {
            self.lifetime = PluginEditorLifetime::Destroyed;
            self.visibility = PluginEditorVisibility::Hidden;
            self.focus = PluginEditorFocus::Unfocused;
            self.events
                .push(PluginHostEvent::EditorDestroyed(reason.into()));
            self.events.push(PluginHostEvent::SafeTeardownComplete);
        }
    }
}
