//! Test doubles for host adapter conformance.

use serde::{Deserialize, Serialize};

use crate::{
    desktop::{
        ClipboardCapability, DesktopHostAdapter, DesktopHostEvent, DesktopWindowConfig,
        KeyboardInput, PointerInput, WindowMode,
    },
    plugin::{PluginEditorConfig, PluginHostAdapter, PluginHostEvent},
    surface::{
        FramePresenter, HostCapabilities, HostSurface, PresentedFrame, RepaintRequest,
        SurfaceClipboardRequest, SurfaceEvent, SurfaceMetrics, SurfaceWindowCommand,
        SurfaceWindowMode,
    },
};

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

    fn teardown(&mut self, reason: String) {
        self.events.push(SurfaceEvent::TeardownRequested(reason));
    }
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

    fn teardown(&mut self, reason: String) {
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

    fn teardown(&mut self, reason: String) {
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
