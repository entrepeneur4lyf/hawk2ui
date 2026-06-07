//! Production `winit` desktop event-loop runtime.

use std::{
    collections::BTreeMap,
    num::NonZeroU32,
    sync::{Arc, mpsc::Receiver},
    time::{Duration, Instant},
};

use hawk2ui_assets::AssetRecord;
use hawk2ui_host::{DesktopHostEvent, DesktopWindowConfig, PointerInput};
use hawk2ui_layout::{FlexDirection, LayoutSizing, LayoutStyle, Viewport};
use hawk2ui_runtime::{
    AnimationCadencePolicy, AnimationFrameScheduler, EntryNode, RuntimeEvent, RuntimeSceneBridge,
    RuntimeSceneFrame, RuntimeTextVisual, RuntimeViewId, RuntimeViewNode, RuntimeViewTree,
    RuntimeVisual, StructuredValue as RuntimeStructuredValue,
};
use hawk2ui_script::{
    FrameInput, FrameworkRuntimeController, HostCallPolicy, HostSnapshot, ScriptBackend,
    ScriptModule, StructuredValue, TimerPolicy, entry_mount_bootstrap_with_host,
    parse_entry_envelope,
};
use softbuffer::{Context, Surface};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalSize},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

use crate::{
    SoftwareFrameRenderer, WinitEventTranslator, WinitHostError, WinitTranslatedEvent,
    gpu_frame::WinitGpuFramePresenter, physical_frame_size,
};

/// Production desktop runtime configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct WinitDesktopRuntimeConfig {
    window: DesktopWindowConfig,
    exit_after_first_frame: bool,
    runtime_tree: Option<RuntimeViewTree>,
    runtime_assets: Vec<AssetRecord>,
    animation_policy: AnimationCadencePolicy,
    script_entry: Option<WinitDesktopScriptEntry>,
    framework_controller: Option<FrameworkRuntimeController>,
    presentation_backend: WinitPresentationBackend,
}

/// Native presentation backend requested for the desktop runtime.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WinitPresentationBackend {
    /// Skia CPU raster rendering copied into a `softbuffer` native surface.
    #[default]
    Software,
    /// Prefer Skia GPU rendering when a platform GPU surface can be created, otherwise fall back to
    /// software presentation with a diagnostic.
    GpuPreferred,
    /// Require Skia GPU rendering and fail startup if a platform GPU surface cannot be created.
    GpuRequired,
}

/// Native presentation backend that actually presented frames for a desktop runtime.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WinitPresentationBackendUsed {
    /// Skia CPU raster rendering copied into a `softbuffer` native surface.
    #[default]
    Software,
    /// Skia GPU rendering presented through a native OpenGL surface.
    Gpu,
}

impl WinitPresentationBackendUsed {
    /// Returns the stable diagnostic label for the backend.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Software => "software",
            Self::Gpu => "gpu",
        }
    }
}

/// Executable desktop entry script retained by the native runtime for host-event rerenders.
#[derive(Clone, Debug, PartialEq)]
pub struct WinitDesktopScriptEntry {
    source_path: String,
    compiled_source: String,
    host_snapshot: HostSnapshot,
}

impl WinitDesktopScriptEntry {
    /// Creates a desktop script entry.
    #[must_use]
    pub fn new(
        source_path: impl Into<String>,
        compiled_source: impl Into<String>,
        host_snapshot: HostSnapshot,
    ) -> Self {
        Self {
            source_path: source_path.into(),
            compiled_source: compiled_source.into(),
            host_snapshot,
        }
    }

    /// Returns the source path used in diagnostics.
    #[must_use]
    pub fn source_path(&self) -> &str {
        &self.source_path
    }

    /// Returns the compiled source.
    #[must_use]
    pub fn compiled_source(&self) -> &str {
        &self.compiled_source
    }

    /// Returns the host snapshot projected into the script.
    #[must_use]
    pub const fn host_snapshot(&self) -> &HostSnapshot {
        &self.host_snapshot
    }
}

impl WinitDesktopRuntimeConfig {
    /// Creates a production desktop runtime configuration.
    #[must_use]
    pub const fn new(window: DesktopWindowConfig) -> Self {
        Self {
            window,
            exit_after_first_frame: false,
            runtime_tree: None,
            runtime_assets: Vec::new(),
            animation_policy: AnimationCadencePolicy::disabled(),
            script_entry: None,
            framework_controller: None,
            presentation_backend: WinitPresentationBackend::Software,
        }
    }

    /// Enables or disables automated exit after the first presented frame.
    #[must_use]
    pub const fn with_exit_after_first_frame(mut self, exit_after_first_frame: bool) -> Self {
        self.exit_after_first_frame = exit_after_first_frame;
        self
    }

    /// Sets the runtime view tree rendered by this desktop runtime.
    #[must_use]
    pub fn with_runtime_tree(mut self, runtime_tree: RuntimeViewTree) -> Self {
        self.runtime_tree = Some(runtime_tree);
        self
    }

    /// Sets the compiled runtime assets rendered by scene asset draw commands.
    #[must_use]
    pub fn with_runtime_assets(mut self, assets: impl IntoIterator<Item = AssetRecord>) -> Self {
        self.runtime_assets = assets.into_iter().collect();
        self
    }

    /// Sets the animation cadence policy used by the desktop host.
    #[must_use]
    pub const fn with_animation_policy(mut self, animation_policy: AnimationCadencePolicy) -> Self {
        self.animation_policy = animation_policy;
        self
    }

    /// Sets the executable entry script used to rerender script apps from host input.
    #[must_use]
    pub fn with_script_entry(mut self, script_entry: WinitDesktopScriptEntry) -> Self {
        self.script_entry = Some(script_entry);
        self
    }

    /// Sets the executable framework runtime controller used to handle framework UI events.
    #[must_use]
    pub fn with_framework_controller(mut self, controller: FrameworkRuntimeController) -> Self {
        self.runtime_tree = Some(controller.runtime_tree().clone());
        self.framework_controller = Some(controller);
        self
    }

    /// Sets the native presentation backend preference.
    #[must_use]
    pub const fn with_presentation_backend(
        mut self,
        presentation_backend: WinitPresentationBackend,
    ) -> Self {
        self.presentation_backend = presentation_backend;
        self
    }

    /// Returns the desktop window configuration.
    #[must_use]
    pub const fn window(&self) -> &DesktopWindowConfig {
        &self.window
    }

    /// Returns whether the runtime exits after one successful frame.
    #[must_use]
    pub const fn exit_after_first_frame(&self) -> bool {
        self.exit_after_first_frame
    }

    /// Returns the optional runtime view tree rendered by this desktop runtime.
    #[must_use]
    pub const fn runtime_tree(&self) -> Option<&RuntimeViewTree> {
        self.runtime_tree.as_ref()
    }

    /// Returns compiled runtime assets available to the renderer.
    #[must_use]
    pub fn runtime_assets(&self) -> &[AssetRecord] {
        &self.runtime_assets
    }

    /// Returns the animation cadence policy.
    #[must_use]
    pub const fn animation_policy(&self) -> AnimationCadencePolicy {
        self.animation_policy
    }

    /// Returns the executable desktop entry script, when configured.
    #[must_use]
    pub const fn script_entry(&self) -> Option<&WinitDesktopScriptEntry> {
        self.script_entry.as_ref()
    }

    /// Returns the executable framework runtime controller, when configured.
    #[must_use]
    pub const fn framework_controller(&self) -> Option<&FrameworkRuntimeController> {
        self.framework_controller.as_ref()
    }

    /// Returns the requested native presentation backend.
    #[must_use]
    pub const fn presentation_backend(&self) -> WinitPresentationBackend {
        self.presentation_backend
    }

    /// Validates runtime configuration before entering the native event loop.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the window title, logical size, or scale factor is invalid.
    pub fn validate(&self) -> Result<(), WinitHostError> {
        if self.window.title.trim().is_empty() {
            return Err(WinitHostError::new(
                "desktop.window.invalid-title",
                "desktop window title must not be empty",
            ));
        }
        let metrics = self.window.metrics;
        if !metrics.logical_width.is_finite()
            || !metrics.logical_height.is_finite()
            || !metrics.scale_factor.is_finite()
            || metrics.logical_width <= 0.0
            || metrics.logical_height <= 0.0
            || metrics.scale_factor <= 0.0
        {
            return Err(WinitHostError::new(
                "desktop.window.invalid-size",
                "desktop window dimensions and scale factor must be finite and greater than zero",
            ));
        }
        physical_frame_size(metrics).map(|_| ())
    }
}

/// Development reload category for a running `winit` desktop surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WinitDesktopReloadKind {
    /// Style-only patch; the existing native window can be reused.
    StylePatch,
    /// Asset patch; the renderer asset registry is refreshed and the window can be reused.
    AssetPatch,
    /// Runtime tree patch; the retained scene source is replaced and the window can be reused.
    RuntimeTreePatch,
    /// Script rebuild; the rebuilt runtime tree is applied without recreating the window.
    ScriptRebuild,
    /// Manifest or unknown source changed; the current event loop must exit and be restarted.
    FullRebuildRequired,
}

impl WinitDesktopReloadKind {
    /// Returns whether this reload requires terminating and recreating the native event loop.
    #[must_use]
    pub const fn requires_event_loop_restart(self) -> bool {
        matches!(self, Self::FullRebuildRequired)
    }
}

/// Reload request sent to a running desktop surface during `hawk2ui dev`.
#[derive(Clone, Debug, PartialEq)]
pub struct WinitDesktopReload {
    kind: WinitDesktopReloadKind,
    config: WinitDesktopRuntimeConfig,
    preserve_state: bool,
}

impl WinitDesktopReload {
    /// Creates a reload request from a classified change and rebuilt runtime configuration.
    #[must_use]
    pub const fn new(kind: WinitDesktopReloadKind, config: WinitDesktopRuntimeConfig) -> Self {
        Self {
            kind,
            config,
            preserve_state: true,
        }
    }

    /// Sets whether runtime state should be preserved while applying the reload.
    #[must_use]
    pub const fn with_preserve_state(mut self, preserve_state: bool) -> Self {
        self.preserve_state = preserve_state;
        self
    }

    /// Returns the reload category.
    #[must_use]
    pub const fn kind(&self) -> WinitDesktopReloadKind {
        self.kind
    }

    /// Returns the rebuilt runtime configuration.
    #[must_use]
    pub const fn config(&self) -> &WinitDesktopRuntimeConfig {
        &self.config
    }

    /// Returns whether runtime state should be preserved.
    #[must_use]
    pub const fn preserve_state(&self) -> bool {
        self.preserve_state
    }
}

/// Result of applying a desktop reload request to a live surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WinitDesktopReloadReport {
    kind: WinitDesktopReloadKind,
    reload_generation: u64,
    preserve_state: bool,
    redraw_requested: bool,
    requires_event_loop_restart: bool,
}

impl WinitDesktopReloadReport {
    /// Returns the reload category that produced this report.
    #[must_use]
    pub const fn kind(&self) -> WinitDesktopReloadKind {
        self.kind
    }

    /// Returns the monotonic reload generation after applying the patch.
    #[must_use]
    pub const fn reload_generation(&self) -> u64 {
        self.reload_generation
    }

    /// Returns whether runtime state was preserved.
    #[must_use]
    pub const fn preserve_state(&self) -> bool {
        self.preserve_state
    }

    /// Returns whether the native window should request a redraw.
    #[must_use]
    pub const fn redraw_requested(&self) -> bool {
        self.redraw_requested
    }

    /// Returns whether the current event loop must exit and be restarted.
    #[must_use]
    pub const fn requires_event_loop_restart(&self) -> bool {
        self.requires_event_loop_restart
    }
}

/// Mutable runtime state for a native desktop surface.
#[derive(Clone, Debug, PartialEq)]
pub struct WinitDesktopRuntimeSurfaceState {
    config: WinitDesktopRuntimeConfig,
    reload_generation: u64,
}

impl WinitDesktopRuntimeSurfaceState {
    /// Creates validated state for a native desktop surface.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the initial runtime configuration is invalid.
    pub fn new(config: WinitDesktopRuntimeConfig) -> Result<Self, WinitHostError> {
        config.validate()?;
        Ok(Self {
            config,
            reload_generation: 0,
        })
    }

    /// Returns the current runtime configuration.
    #[must_use]
    pub const fn config(&self) -> &WinitDesktopRuntimeConfig {
        &self.config
    }

    /// Returns the number of reloads applied without recreating the event loop.
    #[must_use]
    pub const fn reload_generation(&self) -> u64 {
        self.reload_generation
    }

    /// Applies a reload to the retained desktop surface state.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the rebuilt runtime configuration is invalid.
    pub fn apply_reload(
        &mut self,
        reload: WinitDesktopReload,
    ) -> Result<WinitDesktopReloadReport, WinitHostError> {
        reload.config.validate()?;
        if reload.kind.requires_event_loop_restart() {
            return Ok(WinitDesktopReloadReport {
                kind: reload.kind,
                reload_generation: self.reload_generation,
                preserve_state: false,
                redraw_requested: false,
                requires_event_loop_restart: true,
            });
        }
        self.config = reload.config;
        self.reload_generation = self.reload_generation.saturating_add(1);
        Ok(WinitDesktopReloadReport {
            kind: reload.kind,
            reload_generation: self.reload_generation,
            preserve_state: reload.preserve_state,
            redraw_requested: true,
            requires_event_loop_restart: false,
        })
    }
}

/// Summary returned after the desktop runtime exits.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WinitDesktopRuntimeSummary {
    /// Native presentation backend that actually presented runtime frames.
    pub presentation_backend_used: WinitPresentationBackendUsed,
    /// Whether a native window was created.
    pub window_created: bool,
    /// Number of full frames presented.
    pub frames_presented: u64,
    /// Number of full frames presented by the native GPU backend.
    pub gpu_frames_presented: u64,
    /// Duration of the last successfully presented frame in microseconds.
    pub last_frame_duration_micros: u64,
    /// Maximum duration of any successfully presented frame in microseconds.
    pub max_frame_duration_micros: u64,
    /// Sum of all successfully presented frame durations in microseconds.
    pub total_frame_duration_micros: u64,
    /// Whether a submitted GPU frame was read back for verification.
    pub gpu_readback_verified: bool,
    /// Diagnostic explaining why a preferred backend fell back to another presentation path.
    pub presentation_fallback_reason: Option<WinitHostError>,
    /// Number of resize events processed.
    pub resizes: u64,
    /// Number of DPI change events processed.
    pub dpi_changes: u64,
    /// Number of input or focus events processed.
    pub input_events: u64,
    /// Number of animation ticks accepted before presentation.
    pub animation_ticks: u64,
    /// Number of dev reloads applied without recreating the native event loop.
    pub native_reloads: u64,
    /// Whether a close request was received.
    pub close_requested: bool,
}

impl WinitDesktopRuntimeSummary {
    /// Returns the average successful frame duration in microseconds.
    #[must_use]
    pub fn average_frame_duration_micros(&self) -> u64 {
        self.total_frame_duration_micros
            .checked_div(self.frames_presented)
            .unwrap_or(0)
    }
}

/// Production `winit` desktop runtime.
#[derive(Clone, Debug, Default)]
pub struct WinitDesktopRuntime {
    renderer: SoftwareFrameRenderer,
}

impl WinitDesktopRuntime {
    /// Creates a runtime with the default Skia software renderer.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            renderer: SoftwareFrameRenderer::new(),
        }
    }

    /// Runs the native desktop event loop until close or configured first-frame exit.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when event-loop creation, window creation, rendering, resize, or
    /// presentation fails.
    pub fn run_blocking(
        &self,
        config: WinitDesktopRuntimeConfig,
    ) -> Result<WinitDesktopRuntimeSummary, WinitHostError> {
        config.validate()?;
        let event_loop = EventLoop::new().map_err(|error| {
            WinitHostError::new(
                "desktop.event-loop.create-failed",
                format!("failed to create winit event loop: {error}"),
            )
        })?;
        let mut app = RuntimeApplication::new(config, self.renderer.clone());
        event_loop.run_app(&mut app).map_err(|error| {
            WinitHostError::new(
                "desktop.event-loop.run-failed",
                format!("winit event loop failed: {error}"),
            )
        })?;
        app.finish()
    }

    /// Runs the native Wayland event loop from a non-main test or embedding thread.
    ///
    /// Normal applications should use [`Self::run_blocking`]. This entry point exists for native
    /// smoke harnesses and embedders that intentionally create the Linux Wayland event loop away
    /// from the process main thread.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when event-loop creation, window creation, rendering, resize, or
    /// presentation fails.
    #[cfg(target_os = "linux")]
    pub fn run_wayland_any_thread_blocking(
        &self,
        config: WinitDesktopRuntimeConfig,
    ) -> Result<WinitDesktopRuntimeSummary, WinitHostError> {
        use winit::platform::wayland::EventLoopBuilderExtWayland;

        config.validate()?;
        let mut builder = EventLoop::builder();
        builder.with_wayland().with_any_thread(true);
        let event_loop = builder.build().map_err(|error| {
            WinitHostError::new(
                "desktop.event-loop.create-failed",
                format!("failed to create winit Wayland event loop: {error}"),
            )
        })?;
        let mut app = RuntimeApplication::new(config, self.renderer.clone());
        event_loop.run_app(&mut app).map_err(|error| {
            WinitHostError::new(
                "desktop.event-loop.run-failed",
                format!("winit Wayland event loop failed: {error}"),
            )
        })?;
        app.finish()
    }

    /// Runs the native desktop event loop with a development reload channel.
    ///
    /// The event loop owns the native window. Rebuilt runtime configurations are delivered through
    /// `reloads` and applied on the event-loop thread by requesting a redraw instead of tearing down
    /// the surface for patchable reload kinds.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when event-loop creation, window creation, rendering, reload
    /// validation, resize, or presentation fails.
    pub fn run_dev_blocking(
        &self,
        config: WinitDesktopRuntimeConfig,
        reloads: Receiver<WinitDesktopReload>,
    ) -> Result<WinitDesktopRuntimeSummary, WinitHostError> {
        config.validate()?;
        let event_loop = EventLoop::<WinitDesktopRuntimeUserEvent>::with_user_event()
            .build()
            .map_err(|error| {
                WinitHostError::new(
                    "desktop.event-loop.create-failed",
                    format!("failed to create winit dev event loop: {error}"),
                )
            })?;
        let proxy = event_loop.create_proxy();
        std::thread::spawn(move || {
            for reload in reloads {
                if proxy
                    .send_event(WinitDesktopRuntimeUserEvent::Reload(reload))
                    .is_err()
                {
                    break;
                }
            }
        });

        let mut app = RuntimeApplication::new(config, self.renderer.clone());
        event_loop.run_app(&mut app).map_err(|error| {
            WinitHostError::new(
                "desktop.event-loop.run-failed",
                format!("winit dev event loop failed: {error}"),
            )
        })?;
        app.finish()
    }
}

#[derive(Clone, Debug)]
enum WinitDesktopRuntimeUserEvent {
    Reload(WinitDesktopReload),
}

struct RuntimeApplication {
    config: WinitDesktopRuntimeConfig,
    renderer: SoftwareFrameRenderer,
    window: Option<Arc<Window>>,
    context: Option<Context<Arc<Window>>>,
    surface: Option<Surface<Arc<Window>, Arc<Window>>>,
    gpu_presenter: Option<WinitGpuFramePresenter>,
    lifecycle: RuntimeLifecycle,
    event_translator: WinitEventTranslator,
    animation: AnimationFrameScheduler,
    script_ui_json: String,
    started_at: Instant,
    last_error: Option<WinitHostError>,
}

const FRAMEWORK_LIFECYCLE_MOUNTED: &str = "lifecycle.mounted";
const FRAMEWORK_LIFECYCLE_SUSPENDED: &str = "lifecycle.suspended";
const FRAMEWORK_LIFECYCLE_RESUMED: &str = "lifecycle.resumed";
const FRAMEWORK_LIFECYCLE_HOT_RELOADED: &str = "lifecycle.hot-reloaded";
const FRAMEWORK_LIFECYCLE_ERROR_BOUNDARY: &str = "lifecycle.error-boundary";
const FRAMEWORK_LIFECYCLE_SHUTDOWN: &str = "lifecycle.shutdown";
const FRAMEWORK_LIFECYCLE_UNMOUNTED: &str = "lifecycle.unmounted";

impl RuntimeApplication {
    fn new(config: WinitDesktopRuntimeConfig, renderer: SoftwareFrameRenderer) -> Self {
        let animation = AnimationFrameScheduler::new(config.animation_policy());
        let renderer = renderer.with_assets(config.runtime_assets().iter().cloned());
        let metrics = config.window.metrics;
        Self {
            config,
            renderer,
            window: None,
            context: None,
            surface: None,
            gpu_presenter: None,
            lifecycle: RuntimeLifecycle::default(),
            event_translator: WinitEventTranslator::new(metrics),
            animation,
            script_ui_json: "null".to_string(),
            started_at: Instant::now(),
            last_error: None,
        }
    }

    fn finish(self) -> Result<WinitDesktopRuntimeSummary, WinitHostError> {
        if let Some(error) = self.last_error {
            Err(error)
        } else {
            Ok(self.lifecycle.into_summary())
        }
    }

    fn apply_reload(
        &mut self,
        reload: WinitDesktopReload,
    ) -> Result<WinitDesktopReloadReport, WinitHostError> {
        let mut state = WinitDesktopRuntimeSurfaceState {
            config: self.config.clone(),
            reload_generation: self.lifecycle.summary.native_reloads,
        };
        let report = state.apply_reload(reload)?;
        if report.requires_event_loop_restart() {
            return Ok(report);
        }
        self.config = state.config;
        self.renderer =
            SoftwareFrameRenderer::new().with_assets(self.config.runtime_assets().iter().cloned());
        self.event_translator = WinitEventTranslator::new(self.config.window.metrics);
        self.animation = AnimationFrameScheduler::new(self.config.animation_policy());
        self.lifecycle.record_native_reload();
        self.dispatch_framework_lifecycle_event(FRAMEWORK_LIFECYCLE_HOT_RELOADED)?;
        self.request_redraw();
        Ok(report)
    }

    fn create_window(&mut self, event_loop: &ActiveEventLoop) -> Result<(), WinitHostError> {
        let metrics = self.config.window.metrics;
        let attributes = window_attributes(&self.config.window);
        match self.config.presentation_backend {
            WinitPresentationBackend::Software => {
                self.create_software_window(event_loop, attributes)
            }
            WinitPresentationBackend::GpuRequired => self.create_gpu_window(event_loop, attributes),
            WinitPresentationBackend::GpuPreferred => {
                match self.create_gpu_window(event_loop, attributes.clone()) {
                    Ok(()) => Ok(()),
                    Err(error) => {
                        self.lifecycle.record_gpu_preferred_fallback(&error);
                        self.create_software_window(event_loop, attributes)
                    }
                }
            }
        }?;
        self.event_translator = WinitEventTranslator::new(metrics);
        self.request_redraw();
        Ok(())
    }

    fn create_software_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        attributes: winit::window::WindowAttributes,
    ) -> Result<(), WinitHostError> {
        let window = Arc::new(event_loop.create_window(attributes).map_err(|error| {
            WinitHostError::new(
                "desktop.window.create-failed",
                format!("failed to create native desktop window: {error}"),
            )
        })?);
        let context = Context::new(Arc::clone(&window)).map_err(|error| {
            WinitHostError::new(
                "desktop.surface.context-failed",
                format!("failed to create native surface context: {error}"),
            )
        })?;
        let surface = Surface::new(&context, Arc::clone(&window)).map_err(|error| {
            WinitHostError::new(
                "desktop.surface.create-failed",
                format!("failed to create native presentation surface: {error}"),
            )
        })?;

        self.lifecycle.record_window_created();
        self.lifecycle
            .record_presentation_backend_used(WinitPresentationBackendUsed::Software);
        self.context = Some(context);
        self.surface = Some(surface);
        self.gpu_presenter = None;
        self.window = Some(window);
        Ok(())
    }

    fn create_gpu_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        attributes: winit::window::WindowAttributes,
    ) -> Result<(), WinitHostError> {
        let (window, presenter) = WinitGpuFramePresenter::create_wayland_window(
            event_loop,
            attributes,
            self.config.runtime_assets().iter().cloned(),
        )?;
        self.lifecycle.record_window_created();
        self.lifecycle
            .record_presentation_backend_used(WinitPresentationBackendUsed::Gpu);
        self.context = None;
        self.surface = None;
        self.gpu_presenter = Some(presenter);
        self.window = Some(window);
        Ok(())
    }

    fn resize_surface(&mut self, size: PhysicalSize<u32>) -> Result<(), WinitHostError> {
        if let Some(gpu_presenter) = self.gpu_presenter.as_mut() {
            if let Some(window) = self.window.as_ref() {
                gpu_presenter.resize_to_window(window)?;
            }
            self.request_redraw();
            return Ok(());
        }

        let Some(surface) = self.surface.as_mut() else {
            return Ok(());
        };
        let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return Ok(());
        };
        surface.resize(width, height).map_err(|error| {
            WinitHostError::new(
                "desktop.surface.resize-failed",
                format!("failed to resize native presentation surface: {error}"),
            )
        })?;
        self.request_redraw();
        Ok(())
    }

    fn build_runtime_scene_for_window(
        &self,
        window: &Window,
        size: PhysicalSize<u32>,
    ) -> Result<RuntimeSceneFrame, WinitHostError> {
        let logical_width = f64::from(size.width) / window.scale_factor();
        let logical_height = f64::from(size.height) / window.scale_factor();
        let viewport = Viewport::new(
            logical_size_to_f32(logical_width)?,
            logical_size_to_f32(logical_height)?,
        );
        let fallback_tree;
        let runtime_tree = if let Some(runtime_tree) = self.current_runtime_tree() {
            runtime_tree
        } else {
            fallback_tree =
                default_runtime_tree(&self.config.window.title, viewport.width, viewport.height);
            &fallback_tree
        };
        RuntimeSceneBridge::new(viewport)
            .build(runtime_tree)
            .map_err(|error| {
                WinitHostError::new(
                    "desktop.runtime-scene.build-failed",
                    format!("failed to build runtime scene for desktop frame: {error:?}"),
                )
            })
    }

    fn present_gpu_frame(
        &mut self,
        event_loop: &ActiveEventLoop,
        window: &Window,
        size: PhysicalSize<u32>,
    ) -> Result<(), WinitHostError> {
        let scene = self.build_runtime_scene_for_window(window, size)?;
        let frame_index = self.lifecycle.summary.frames_presented;
        let Some(gpu_presenter) = self.gpu_presenter.as_mut() else {
            return Ok(());
        };
        let frame_started_at = Instant::now();
        gpu_presenter.present_scene_frame(window, &scene, frame_index)?;
        let frame_duration = frame_started_at.elapsed();
        self.lifecycle
            .record_gpu_frame_presented(gpu_presenter.last_snapshot().is_some(), frame_duration);
        if self.config.exit_after_first_frame {
            event_loop.exit();
        }
        Ok(())
    }

    fn present_software_frame(
        &mut self,
        event_loop: &ActiveEventLoop,
        window: &Window,
        size: PhysicalSize<u32>,
    ) -> Result<(), WinitHostError> {
        let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return Ok(());
        };
        let frame_started_at = Instant::now();
        {
            let Some(surface) = self.surface.as_mut() else {
                return Ok(());
            };
            surface.resize(width, height).map_err(|error| {
                WinitHostError::new(
                    "desktop.surface.resize-failed",
                    format!("failed to resize native presentation surface before render: {error}"),
                )
            })?;
        }

        let frame = if self.current_runtime_tree().is_some() {
            let scene = self.build_runtime_scene_for_window(window, size)?;
            self.renderer.render_scene_frame(
                &scene,
                size.width,
                size.height,
                window.scale_factor(),
            )?
        } else {
            self.renderer.render_frame(
                &self.config.window.title,
                size.width,
                size.height,
                window.scale_factor(),
            )?
        };
        let Some(surface) = self.surface.as_mut() else {
            return Ok(());
        };
        let mut buffer = surface.buffer_mut().map_err(|error| {
            WinitHostError::new(
                "desktop.surface.buffer-failed",
                format!("failed to acquire native presentation buffer: {error}"),
            )
        })?;
        buffer.copy_from_slice(frame.pixels());
        window.pre_present_notify();
        buffer.present().map_err(|error| {
            WinitHostError::new(
                "desktop.surface.present-failed",
                format!("failed to present native frame: {error}"),
            )
        })?;
        self.lifecycle
            .record_frame_presented(frame_started_at.elapsed());
        if self.config.exit_after_first_frame {
            event_loop.exit();
        }
        Ok(())
    }

    fn present_frame(&mut self, event_loop: &ActiveEventLoop) -> Result<(), WinitHostError> {
        if !self.lifecycle.accepts_frame_presentation() {
            return Ok(());
        }
        let Some(window) = self.window.clone() else {
            return Ok(());
        };
        let size = window.inner_size();
        if NonZeroU32::new(size.width).is_none() || NonZeroU32::new(size.height).is_none() {
            return Ok(());
        }
        if self.gpu_presenter.is_some() {
            self.present_gpu_frame(event_loop, &window, size)?;
        } else {
            self.present_software_frame(event_loop, &window, size)?;
        }
        if let Some(tick) = self.animation.step_at(self.elapsed_ms()) {
            self.lifecycle.record_animation_tick();
            if tick.reduced_rate_due {
                self.request_redraw();
            }
        }
        Ok(())
    }

    fn elapsed_ms(&self) -> u64 {
        u64::try_from(self.started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn current_runtime_tree(&self) -> Option<&RuntimeViewTree> {
        self.config
            .framework_controller
            .as_ref()
            .map(FrameworkRuntimeController::runtime_tree)
            .or_else(|| self.config.runtime_tree())
    }

    fn dispatch_framework_lifecycle_events(
        &mut self,
        event_names: &[&str],
    ) -> Result<bool, WinitHostError> {
        let mut changed = false;
        for event_name in event_names {
            changed |= self.dispatch_framework_lifecycle_event(event_name)?;
        }
        Ok(changed)
    }

    fn dispatch_framework_lifecycle_event(
        &mut self,
        event_name: &str,
    ) -> Result<bool, WinitHostError> {
        let mut changed = false;
        let mut rebound_tree = None;
        if let Some(controller) = self.config.framework_controller.as_mut() {
            let targets = controller.event_targets(event_name);
            for target in targets {
                let event = RuntimeEvent::lifecycle(target.as_str(), event_name);
                let handled = controller.dispatch_runtime_event(&event).map_err(|error| {
                    WinitHostError::new(
                        "desktop.framework-lifecycle.execute-failed",
                        format!(
                            "failed to execute desktop framework lifecycle event {event_name} for target {target} ({}): {}",
                            error.rule(),
                            error.message()
                        ),
                    )
                })?;
                changed |= handled;
            }
            if changed {
                rebound_tree = Some(controller.runtime_tree().clone());
            }
        }
        if let Some(tree) = rebound_tree {
            self.config.runtime_tree = Some(tree);
            self.request_redraw();
        }
        Ok(changed)
    }

    fn apply_script_entry_events(
        &mut self,
        translated: &WinitTranslatedEvent,
    ) -> Result<bool, WinitHostError> {
        let inputs = desktop_frame_inputs_from_host_events(&translated.events);
        if inputs.is_empty() {
            return Ok(false);
        }
        let Some(entry) = self.config.script_entry() else {
            return Ok(false);
        };
        let Some(window) = self.window.as_ref() else {
            return Ok(false);
        };
        let size = window.inner_size();
        let logical_width = logical_size_to_f32(f64::from(size.width) / window.scale_factor())?;
        let logical_height = logical_size_to_f32(f64::from(size.height) / window.scale_factor())?;
        let frame = run_script_entry_frame(
            entry,
            &inputs,
            &self.script_ui_json,
            logical_width,
            logical_height,
        )?;
        self.config.runtime_tree = Some(frame.runtime_tree);
        self.script_ui_json = frame.ui_json;
        Ok(true)
    }

    fn apply_framework_entry_events(
        &mut self,
        translated: &WinitTranslatedEvent,
    ) -> Result<bool, WinitHostError> {
        if self.config.framework_controller.is_none() {
            return Ok(false);
        }
        if translated.events.is_empty() {
            return Ok(false);
        }
        let Some(window) = self.window.clone() else {
            return Ok(false);
        };
        let size = window.inner_size();
        let mut changed = false;
        for host_event in &translated.events {
            let scene = self.build_runtime_scene_for_window(&window, size)?;
            let events = {
                let Some(controller) = self.config.framework_controller.as_ref() else {
                    continue;
                };
                framework_events_from_host_event(&scene, host_event, controller)
            };
            if events.is_empty() {
                continue;
            }
            let Some(controller) = self.config.framework_controller.as_mut() else {
                continue;
            };
            for event in events {
                let handled = controller.dispatch_runtime_event(&event).map_err(|error| {
                    WinitHostError::new(
                        "desktop.framework-handler.execute-failed",
                        format!(
                            "failed to execute desktop framework handler ({}): {}",
                            error.rule(),
                            error.message()
                        ),
                    )
                })?;
                if handled {
                    self.config.runtime_tree = Some(controller.runtime_tree().clone());
                    changed = true;
                }
            }
        }
        Ok(changed)
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: WinitHostError) {
        if let Err(boundary_error) =
            self.dispatch_framework_lifecycle_event(FRAMEWORK_LIFECYCLE_ERROR_BOUNDARY)
        {
            self.last_error = Some(WinitHostError::new(
                "desktop.framework-lifecycle.error-boundary-failed",
                format!(
                    "desktop failure ({}): {}; framework error boundary failure ({}): {}",
                    error.rule(),
                    error.message(),
                    boundary_error.rule(),
                    boundary_error.message()
                ),
            ));
        } else {
            self.last_error = Some(error);
        }
        event_loop.exit();
    }

    fn handle_resumed(&mut self, event_loop: &ActiveEventLoop) {
        let event_name = if self.window.is_none() {
            if let Err(error) = self.create_window(event_loop) {
                self.fail(event_loop, error);
                return;
            }
            FRAMEWORK_LIFECYCLE_MOUNTED
        } else {
            FRAMEWORK_LIFECYCLE_RESUMED
        };
        if let Err(error) = self.dispatch_framework_lifecycle_event(event_name) {
            self.fail(event_loop, error);
        }
    }

    fn handle_window_event(&mut self, event_loop: &ActiveEventLoop, event: &WindowEvent) {
        if !self.lifecycle.accepts_host_event() && !matches!(event, WindowEvent::CloseRequested) {
            return;
        }
        let translated = self.event_translator.translate(event);
        self.lifecycle.record_translated_event(&translated);
        let script_rerendered = match self.apply_script_entry_events(&translated) {
            Ok(script_rerendered) => script_rerendered,
            Err(error) => {
                self.fail(event_loop, error);
                return;
            }
        };
        let framework_rerendered = match self.apply_framework_entry_events(&translated) {
            Ok(framework_rerendered) => framework_rerendered,
            Err(error) => {
                self.fail(event_loop, error);
                return;
            }
        };
        let result = match event {
            WindowEvent::CloseRequested | WindowEvent::Destroyed => {
                self.lifecycle.request_close();
                event_loop.exit();
                Ok(())
            }
            WindowEvent::Resized(size) => self.resize_surface(*size),
            WindowEvent::ScaleFactorChanged { .. } => {
                self.request_redraw();
                Ok(())
            }
            WindowEvent::RedrawRequested => self.present_frame(event_loop),
            WindowEvent::Focused(_)
            | WindowEvent::KeyboardInput { .. }
            | WindowEvent::Ime(_)
            | WindowEvent::CursorMoved { .. }
            | WindowEvent::CursorEntered { .. }
            | WindowEvent::CursorLeft { .. }
            | WindowEvent::MouseInput { .. }
            | WindowEvent::MouseWheel { .. }
            | WindowEvent::DroppedFile(_)
            | WindowEvent::HoveredFile(_)
            | WindowEvent::HoveredFileCancelled
            | WindowEvent::Occluded(_)
            | WindowEvent::ModifiersChanged(_) => {
                if translated.requires_redraw || script_rerendered || framework_rerendered {
                    self.request_redraw();
                }
                Ok(())
            }
            _ => Ok(()),
        };
        if let Err(error) = result {
            self.fail(event_loop, error);
        }
    }

    fn handle_about_to_wait(&mut self) {
        if self.animation.should_request_frame(self.elapsed_ms())
            && self.lifecycle.accepts_frame_presentation()
        {
            self.request_redraw();
        }
    }

    fn handle_suspended(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(error) = self.dispatch_framework_lifecycle_event(FRAMEWORK_LIFECYCLE_SUSPENDED) {
            self.fail(event_loop, error);
        }
    }

    fn handle_exiting(&mut self) {
        if let Err(error) = self.dispatch_framework_lifecycle_events(&[
            FRAMEWORK_LIFECYCLE_SHUTDOWN,
            FRAMEWORK_LIFECYCLE_UNMOUNTED,
        ]) && self.last_error.is_none()
        {
            self.last_error = Some(error);
        }
        self.gpu_presenter = None;
        self.surface = None;
        self.context = None;
        self.window = None;
    }
}

fn logical_size_to_f32(value: f64) -> Result<f32, WinitHostError> {
    if !value.is_finite() || value <= 0.0 {
        return Err(WinitHostError::new(
            "desktop.runtime-scene.invalid-viewport",
            "runtime scene viewport dimensions must be finite and greater than zero",
        ));
    }
    #[allow(clippy::cast_possible_truncation)]
    let value = value as f32;
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(WinitHostError::new(
            "desktop.runtime-scene.invalid-viewport",
            "runtime scene viewport dimensions must be finite and greater than zero",
        ))
    }
}

fn window_attributes(config: &DesktopWindowConfig) -> winit::window::WindowAttributes {
    Window::default_attributes()
        .with_title(config.title.clone())
        .with_inner_size(LogicalSize::new(
            config.metrics.logical_width,
            config.metrics.logical_height,
        ))
}

fn default_runtime_tree(title: &str, width: f32, height: f32) -> RuntimeViewTree {
    let root_id = RuntimeViewId::new("desktop-default-root");
    let root = RuntimeViewNode::new(
        root_id.clone(),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(width, height)),
        RuntimeVisual::Fill(hawk2ui_render::Color::rgba(8, 10, 14, 255)),
    );
    let tree = RuntimeViewTree::new(root);
    let title_node = RuntimeViewNode::new(
        RuntimeViewId::new("desktop-default-title"),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(width.max(1.0), 64.0)),
        RuntimeVisual::Text(RuntimeTextVisual::new(
            title,
            18.0,
            hawk2ui_render::Color::rgba(241, 245, 249, 255),
        )),
    );
    match tree.with_child(&root_id, title_node) {
        Ok(tree) => tree,
        Err(_) => RuntimeViewTree::new(RuntimeViewNode::new(
            root_id,
            LayoutStyle::flex_container(FlexDirection::Column)
                .with_size(LayoutSizing::fixed(width, height)),
            RuntimeVisual::Fill(hawk2ui_render::Color::rgba(8, 10, 14, 255)),
        )),
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ScriptEntryFrame {
    runtime_tree: RuntimeViewTree,
    ui_json: String,
}

fn desktop_frame_inputs_from_host_events(events: &[DesktopHostEvent]) -> Vec<FrameInput> {
    events
        .iter()
        .filter_map(|event| match event {
            DesktopHostEvent::PointerInput(pointer) => Some(FrameInput::Pointer {
                x: pointer.x,
                y: pointer.y,
                button: pointer.button.clone(),
            }),
            DesktopHostEvent::KeyboardInput(keyboard) => Some(FrameInput::Key {
                key: keyboard.key.clone(),
                pressed: keyboard.pressed,
            }),
            DesktopHostEvent::FocusChanged(focused) => {
                Some(FrameInput::Focus { focused: *focused })
            }
            DesktopHostEvent::WindowCreated(_)
            | DesktopHostEvent::CloseRequested(_)
            | DesktopHostEvent::ModeChanged(_)
            | DesktopHostEvent::ImeInput(_)
            | DesktopHostEvent::FileDragDrop(_)
            | DesktopHostEvent::WindowOcclusionChanged(_)
            | DesktopHostEvent::ClipboardCapabilityChanged(_)
            | DesktopHostEvent::DpiChanged(_)
            | DesktopHostEvent::RendererTargetRecreateRequested
            | DesktopHostEvent::RepaintRequested(_)
            | DesktopHostEvent::Resized(_)
            | DesktopHostEvent::ClipboardRequested(_)
            | DesktopHostEvent::DialogRequested(_)
            | DesktopHostEvent::FramePresented { .. } => None,
        })
        .collect()
}

fn framework_events_from_host_event(
    scene: &RuntimeSceneFrame,
    host_event: &DesktopHostEvent,
    controller: &FrameworkRuntimeController,
) -> Vec<RuntimeEvent> {
    match host_event {
        DesktopHostEvent::PointerInput(pointer) => {
            framework_event_from_pointer_input(scene, pointer, controller)
                .into_iter()
                .collect()
        }
        DesktopHostEvent::KeyboardInput(keyboard) => framework_events_for_targets(
            controller,
            if keyboard.pressed {
                "keyboard.key-down"
            } else {
                "keyboard.key-up"
            },
            |target| keyboard_event_payload(target, keyboard),
        ),
        DesktopHostEvent::FocusChanged(focused) => framework_events_for_targets(
            controller,
            if *focused {
                "focus.focus-in"
            } else {
                "focus.focus-out"
            },
            |target| focus_event_payload(target, *focused),
        ),
        DesktopHostEvent::ImeInput(value) if value.starts_with("commit:") => {
            framework_events_for_targets(controller, "keyboard.text-input", |target| {
                text_input_event_payload(target, value)
            })
        }
        DesktopHostEvent::Resized(metrics) => {
            framework_events_for_targets(controller, "resize", |target| {
                resize_event_payload(target, *metrics)
            })
        }
        DesktopHostEvent::WindowCreated(_)
        | DesktopHostEvent::CloseRequested(_)
        | DesktopHostEvent::ModeChanged(_)
        | DesktopHostEvent::ImeInput(_)
        | DesktopHostEvent::FileDragDrop(_)
        | DesktopHostEvent::WindowOcclusionChanged(_)
        | DesktopHostEvent::ClipboardCapabilityChanged(_)
        | DesktopHostEvent::DpiChanged(_)
        | DesktopHostEvent::RendererTargetRecreateRequested
        | DesktopHostEvent::RepaintRequested(_)
        | DesktopHostEvent::ClipboardRequested(_)
        | DesktopHostEvent::DialogRequested(_)
        | DesktopHostEvent::FramePresented { .. } => Vec::new(),
    }
}

fn framework_events_for_targets(
    controller: &FrameworkRuntimeController,
    event_name: &'static str,
    payload_for_target: impl Fn(&str) -> RuntimeStructuredValue,
) -> Vec<RuntimeEvent> {
    controller
        .event_targets(event_name)
        .into_iter()
        .map(|target| {
            RuntimeEvent::new(
                hawk2ui_runtime::RuntimeEventKind::Ui,
                target.as_str(),
                event_name,
                payload_for_target(&target),
            )
        })
        .collect()
}

fn base_event_payload(target: &str, event_name: &str) -> BTreeMap<String, RuntimeStructuredValue> {
    BTreeMap::from([
        (
            "name".to_string(),
            RuntimeStructuredValue::String(event_name.to_string()),
        ),
        (
            "target".to_string(),
            RuntimeStructuredValue::String(target.to_string()),
        ),
    ])
}

fn pointer_event_payload(
    target: &str,
    event_name: &str,
    pointer: &PointerInput,
) -> RuntimeStructuredValue {
    let mut payload = base_event_payload(target, event_name);
    payload.insert("x".to_string(), RuntimeStructuredValue::Number(pointer.x));
    payload.insert("y".to_string(), RuntimeStructuredValue::Number(pointer.y));
    payload.insert(
        "button".to_string(),
        RuntimeStructuredValue::String(pointer.button.clone()),
    );
    if let Some((delta_x, delta_y)) = pointer_wheel_delta(&pointer.button) {
        payload.insert(
            "delta_x".to_string(),
            RuntimeStructuredValue::Number(delta_x),
        );
        payload.insert(
            "delta_y".to_string(),
            RuntimeStructuredValue::Number(delta_y),
        );
    }
    RuntimeStructuredValue::Object(payload)
}

fn keyboard_event_payload(
    target: &str,
    keyboard: &hawk2ui_host::KeyboardInput,
) -> RuntimeStructuredValue {
    let event_name = if keyboard.pressed {
        "keyboard.key-down"
    } else {
        "keyboard.key-up"
    };
    let mut payload = base_event_payload(target, event_name);
    payload.insert(
        "key".to_string(),
        RuntimeStructuredValue::String(keyboard.key.clone()),
    );
    payload.insert(
        "pressed".to_string(),
        RuntimeStructuredValue::Bool(keyboard.pressed),
    );
    RuntimeStructuredValue::Object(payload)
}

fn text_input_event_payload(target: &str, value: &str) -> RuntimeStructuredValue {
    let mut payload = base_event_payload(target, "keyboard.text-input");
    payload.insert(
        "value".to_string(),
        RuntimeStructuredValue::String(value.strip_prefix("commit:").unwrap_or(value).to_string()),
    );
    RuntimeStructuredValue::Object(payload)
}

fn focus_event_payload(target: &str, focused: bool) -> RuntimeStructuredValue {
    let event_name = if focused {
        "focus.focus-in"
    } else {
        "focus.focus-out"
    };
    let mut payload = base_event_payload(target, event_name);
    payload.insert("focused".to_string(), RuntimeStructuredValue::Bool(focused));
    RuntimeStructuredValue::Object(payload)
}

fn resize_event_payload(
    target: &str,
    metrics: hawk2ui_host::SurfaceMetrics,
) -> RuntimeStructuredValue {
    let mut payload = base_event_payload(target, "resize");
    payload.insert(
        "width".to_string(),
        RuntimeStructuredValue::Number(metrics.logical_width),
    );
    payload.insert(
        "height".to_string(),
        RuntimeStructuredValue::Number(metrics.logical_height),
    );
    payload.insert(
        "scale_factor".to_string(),
        RuntimeStructuredValue::Number(metrics.scale_factor),
    );
    RuntimeStructuredValue::Object(payload)
}

fn framework_event_from_pointer_input(
    scene: &RuntimeSceneFrame,
    pointer: &PointerInput,
    controller: &FrameworkRuntimeController,
) -> Option<RuntimeEvent> {
    let event_name = framework_pointer_event_name(&pointer.button)?;
    scene
        .geometry_entries()
        .iter()
        .rev()
        .find(|(id, geometry)| {
            geometry_contains(*geometry, pointer.x, pointer.y)
                && controller.has_event_handler(id.as_str(), event_name)
        })
        .map(|(id, _)| {
            RuntimeEvent::new(
                hawk2ui_runtime::RuntimeEventKind::Ui,
                id.as_str(),
                event_name,
                pointer_event_payload(id.as_str(), event_name, pointer),
            )
        })
}

fn framework_pointer_event_name(button: &str) -> Option<&'static str> {
    if button.ends_with("-down") || button.starts_with("touch-started:") {
        return Some("pointer.press");
    }
    if button.ends_with("-up") || button.starts_with("touch-ended:") {
        return Some("pointer.release");
    }
    if button == "move" || button.starts_with("touch-moved:") {
        return Some("pointer.move");
    }
    if button == "drag" || button.ends_with("-drag") {
        return Some("pointer.drag");
    }
    if button == "enter" {
        return Some("pointer.enter");
    }
    if button == "leave" || button.starts_with("touch-cancelled:") {
        return Some("pointer.leave");
    }
    if button.starts_with("wheel-lines:") || button.starts_with("wheel-pixels:") {
        return Some("pointer.wheel");
    }
    None
}

fn pointer_wheel_delta(button: &str) -> Option<(f64, f64)> {
    let values = button
        .strip_prefix("wheel-lines:")
        .or_else(|| button.strip_prefix("wheel-pixels:"))?;
    let (x, y) = values.split_once(':')?;
    Some((x.parse().ok()?, y.parse().ok()?))
}

fn geometry_contains(geometry: hawk2ui_render::Geometry, x: f64, y: f64) -> bool {
    let left = f64::from(geometry.x);
    let top = f64::from(geometry.y);
    let right = left + f64::from(geometry.width);
    let bottom = top + f64::from(geometry.height);
    x >= left && y >= top && x < right && y < bottom
}

fn run_script_entry_frame(
    entry: &WinitDesktopScriptEntry,
    inputs: &[FrameInput],
    incoming_ui: &str,
    logical_width: f32,
    logical_height: f32,
) -> Result<ScriptEntryFrame, WinitHostError> {
    let Some(bootstrap) = entry_mount_bootstrap_with_host(
        entry.compiled_source(),
        entry.host_snapshot(),
        inputs,
        incoming_ui,
    ) else {
        return Err(WinitHostError::new(
            "desktop.entry-script.missing-mount",
            format!(
                "desktop entry script {} no longer exposes a mount(host) function",
                entry.source_path()
            ),
        ));
    };
    let mut backend = ScriptBackend::new(HostCallPolicy::deny_all(), TimerPolicy::deterministic());
    let execution = backend
        .execute_module(ScriptModule::for_source_path(
            entry.source_path(),
            bootstrap.as_str(),
        ))
        .map_err(|error| {
            WinitHostError::new(
                "desktop.entry-script.execute-failed",
                format!(
                    "failed to execute desktop entry script {} for host input frame ({}): {}",
                    entry.source_path(),
                    error.rule(),
                    error.diagnostic().message()
                ),
            )
        })?;
    let StructuredValue::String(envelope_json) = execution.value() else {
        return Err(WinitHostError::new(
            "desktop.entry-script.invalid-result",
            format!(
                "desktop entry script {} returned a non-envelope value for host input frame",
                entry.source_path()
            ),
        ));
    };
    let envelope = parse_entry_envelope(envelope_json).map_err(|error| {
        WinitHostError::new(
            "desktop.entry-script.envelope-invalid",
            format!(
                "desktop entry script {} returned an invalid host envelope: {}",
                entry.source_path(),
                error.message()
            ),
        )
    })?;
    let entry_tree = EntryNode::from_tree_json(&envelope.tree_json).map_err(|message| {
        WinitHostError::new(
            "desktop.entry-script.tree-invalid",
            format!(
                "desktop entry script {} returned an invalid runtime tree: {message}",
                entry.source_path()
            ),
        )
    })?;
    let runtime_tree = entry_tree
        .to_view_tree(logical_width, logical_height)
        .map_err(|error| {
            WinitHostError::new(
                "desktop.entry-script.tree-build-failed",
                format!(
                    "desktop entry script {} produced a tree that cannot be rendered: {error:?}",
                    entry.source_path()
                ),
            )
        })?;
    Ok(ScriptEntryFrame {
        runtime_tree,
        ui_json: envelope.ui_json,
    })
}

#[derive(Clone, Debug, Default)]
struct RuntimeLifecycle {
    summary: WinitDesktopRuntimeSummary,
}

impl RuntimeLifecycle {
    #[cfg(test)]
    fn summary(&self) -> &WinitDesktopRuntimeSummary {
        &self.summary
    }

    fn into_summary(self) -> WinitDesktopRuntimeSummary {
        self.summary
    }

    fn accepts_host_event(&self) -> bool {
        !self.summary.close_requested
    }

    fn accepts_frame_presentation(&self) -> bool {
        self.accepts_host_event()
    }

    fn record_window_created(&mut self) {
        self.summary.window_created = true;
    }

    fn record_presentation_backend_used(
        &mut self,
        presentation_backend_used: WinitPresentationBackendUsed,
    ) {
        self.summary.presentation_backend_used = presentation_backend_used;
    }

    fn record_frame_presented(&mut self, frame_duration: Duration) {
        if self.accepts_frame_presentation() {
            self.summary.frames_presented += 1;
            self.record_frame_duration(frame_duration);
        }
    }

    fn record_gpu_frame_presented(&mut self, readback_verified: bool, frame_duration: Duration) {
        if self.accepts_frame_presentation() {
            self.summary.frames_presented += 1;
            self.summary.gpu_frames_presented += 1;
            self.summary.gpu_readback_verified |= readback_verified;
            self.record_frame_duration(frame_duration);
        }
    }

    fn record_frame_duration(&mut self, frame_duration: Duration) {
        let micros = duration_to_micros_u64(frame_duration);
        self.summary.last_frame_duration_micros = micros;
        self.summary.max_frame_duration_micros = self.summary.max_frame_duration_micros.max(micros);
        self.summary.total_frame_duration_micros = self
            .summary
            .total_frame_duration_micros
            .saturating_add(micros);
    }

    fn record_gpu_preferred_fallback(&mut self, error: &WinitHostError) {
        self.summary.presentation_fallback_reason = Some(error.clone());
    }

    fn record_resize(&mut self) {
        if self.accepts_frame_presentation() {
            self.summary.resizes += 1;
        }
    }

    fn record_dpi_change(&mut self) {
        if self.accepts_frame_presentation() {
            self.summary.dpi_changes += 1;
        }
    }

    fn record_input_event(&mut self) {
        if self.accepts_frame_presentation() {
            self.summary.input_events += 1;
        }
    }

    fn record_animation_tick(&mut self) {
        if self.accepts_frame_presentation() {
            self.summary.animation_ticks += 1;
        }
    }

    fn record_native_reload(&mut self) {
        if self.accepts_frame_presentation() {
            self.summary.native_reloads += 1;
        }
    }

    fn record_translated_event(&mut self, translated: &WinitTranslatedEvent) {
        for event in &translated.events {
            match event {
                DesktopHostEvent::Resized(_) => self.record_resize(),
                DesktopHostEvent::DpiChanged(_) => self.record_dpi_change(),
                DesktopHostEvent::FocusChanged(_)
                | DesktopHostEvent::KeyboardInput(_)
                | DesktopHostEvent::PointerInput(_)
                | DesktopHostEvent::ImeInput(_)
                | DesktopHostEvent::FileDragDrop(_)
                | DesktopHostEvent::WindowOcclusionChanged(_) => self.record_input_event(),
                DesktopHostEvent::WindowCreated(_)
                | DesktopHostEvent::CloseRequested(_)
                | DesktopHostEvent::ModeChanged(_)
                | DesktopHostEvent::RendererTargetRecreateRequested
                | DesktopHostEvent::ClipboardCapabilityChanged(_)
                | DesktopHostEvent::RepaintRequested(_)
                | DesktopHostEvent::ClipboardRequested(_)
                | DesktopHostEvent::DialogRequested(_)
                | DesktopHostEvent::FramePresented { .. } => {}
            }
        }
    }

    fn request_close(&mut self) -> bool {
        if self.summary.close_requested {
            false
        } else {
            self.summary.close_requested = true;
            true
        }
    }
}

fn duration_to_micros_u64(duration: Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}

impl ApplicationHandler for RuntimeApplication {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.handle_resumed(event_loop);
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        self.handle_suspended(event_loop);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        self.handle_window_event(event_loop, &event);
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        self.handle_about_to_wait();
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.handle_exiting();
    }
}

impl ApplicationHandler<WinitDesktopRuntimeUserEvent> for RuntimeApplication {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.handle_resumed(event_loop);
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        self.handle_suspended(event_loop);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: WinitDesktopRuntimeUserEvent) {
        match event {
            WinitDesktopRuntimeUserEvent::Reload(reload) => match self.apply_reload(reload) {
                Ok(report) if report.requires_event_loop_restart() => event_loop.exit(),
                Ok(_) => {}
                Err(error) => self.fail(event_loop, error),
            },
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        self.handle_window_event(event_loop, &event);
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        self.handle_about_to_wait();
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.handle_exiting();
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use hawk2ui_authoring::{
        ElementKind, EventKind, EventPayloadField, FrameworkDynamicBinding, FrameworkDynamicValue,
        FrameworkEventHandler, FrameworkEventHandlerAction, FrameworkInitialDynamicValue,
        FrameworkNativeNode, FrameworkNativeProgram, HandlerRef, NativeLifecycleEvent,
        NativeRuntimeBridge, PointerEventKind, PropValue,
    };
    use hawk2ui_host::{
        DesktopHostEvent, DesktopWindowConfig, KeyboardInput, PointerInput, SurfaceMetrics,
    };
    use hawk2ui_layout::Viewport;
    use hawk2ui_runtime::{
        RuntimeDrawCommand, RuntimeSceneBridge, RuntimeSceneFrame, RuntimeViewId, RuntimeVisual,
        StructuredValue,
    };
    use hawk2ui_script::{FrameInput, FrameworkRuntimeController, HostSnapshot};

    use super::{
        RuntimeApplication, RuntimeLifecycle, SoftwareFrameRenderer, WinitDesktopRuntimeConfig,
        WinitDesktopScriptEntry, WinitHostError, desktop_frame_inputs_from_host_events,
        framework_event_from_pointer_input, framework_events_from_host_event, logical_size_to_f32,
        run_script_entry_frame,
    };

    #[test]
    fn runtime_lifecycle_stops_presenting_frames_after_close() {
        let mut lifecycle = RuntimeLifecycle::default();

        assert!(lifecycle.accepts_frame_presentation());
        lifecycle.record_frame_presented(Duration::from_millis(3));
        assert_eq!(lifecycle.summary().frames_presented, 1);
        assert_eq!(lifecycle.summary().last_frame_duration_micros, 3_000);
        assert_eq!(lifecycle.summary().max_frame_duration_micros, 3_000);
        assert_eq!(lifecycle.summary().total_frame_duration_micros, 3_000);
        assert_eq!(lifecycle.summary().average_frame_duration_micros(), 3_000);

        assert!(lifecycle.request_close());
        assert!(!lifecycle.request_close());
        assert!(!lifecycle.accepts_host_event());
        assert!(!lifecycle.accepts_frame_presentation());
        assert!(lifecycle.summary().close_requested);

        lifecycle.record_resize();
        lifecycle.record_dpi_change();
        lifecycle.record_input_event();
        lifecycle.record_frame_presented(Duration::from_millis(9));
        assert_eq!(lifecycle.summary().resizes, 0);
        assert_eq!(lifecycle.summary().dpi_changes, 0);
        assert_eq!(lifecycle.summary().input_events, 0);
        assert_eq!(lifecycle.summary().frames_presented, 1);
        assert_eq!(lifecycle.summary().last_frame_duration_micros, 3_000);
        assert_eq!(lifecycle.summary().max_frame_duration_micros, 3_000);
        assert_eq!(lifecycle.summary().total_frame_duration_micros, 3_000);
    }

    #[test]
    fn runtime_lifecycle_records_gpu_frame_timing_and_readback() {
        let mut lifecycle = RuntimeLifecycle::default();

        lifecycle.record_gpu_frame_presented(true, Duration::from_millis(4));
        lifecycle.record_gpu_frame_presented(false, Duration::from_millis(6));

        assert_eq!(lifecycle.summary().frames_presented, 2);
        assert_eq!(lifecycle.summary().gpu_frames_presented, 2);
        assert!(lifecycle.summary().gpu_readback_verified);
        assert_eq!(lifecycle.summary().last_frame_duration_micros, 6_000);
        assert_eq!(lifecycle.summary().max_frame_duration_micros, 6_000);
        assert_eq!(lifecycle.summary().total_frame_duration_micros, 10_000);
        assert_eq!(lifecycle.summary().average_frame_duration_micros(), 5_000);
    }

    #[test]
    fn runtime_lifecycle_counts_animation_ticks_before_close() {
        let mut lifecycle = RuntimeLifecycle::default();

        lifecycle.record_animation_tick();
        lifecycle.record_animation_tick();
        assert_eq!(lifecycle.summary().animation_ticks, 2);

        lifecycle.request_close();
        lifecycle.record_animation_tick();
        assert_eq!(lifecycle.summary().animation_ticks, 2);
    }

    #[test]
    fn runtime_lifecycle_records_gpu_preferred_fallback_reason() {
        let mut lifecycle = RuntimeLifecycle::default();
        lifecycle.record_gpu_preferred_fallback(&WinitHostError::new(
            "desktop.gpu.wayland-required",
            "Winit GPU presentation currently requires a native Wayland display",
        ));

        let reason = lifecycle
            .summary()
            .presentation_fallback_reason
            .as_ref()
            .unwrap_or_else(|| panic!("GPU preferred fallback should retain diagnostic evidence"));
        assert_eq!(reason.rule(), "desktop.gpu.wayland-required");
        assert_eq!(
            reason.message(),
            "Winit GPU presentation currently requires a native Wayland display"
        );
    }

    #[test]
    fn logical_size_to_f32_rejects_invalid_viewport_values() {
        let value =
            logical_size_to_f32(1280.0).unwrap_or_else(|error| panic!("valid size: {error:?}"));
        assert!((value - 1280.0).abs() < f32::EPSILON);
        assert!(logical_size_to_f32(0.0).is_err());
        assert!(logical_size_to_f32(f64::INFINITY).is_err());
        assert!(logical_size_to_f32(f64::MAX).is_err());
    }

    #[test]
    fn desktop_frame_inputs_project_pointer_keyboard_and_focus_events() {
        let inputs = desktop_frame_inputs_from_host_events(&[
            DesktopHostEvent::PointerInput(PointerInput::new(12.5, 34.0, "left-down")),
            DesktopHostEvent::KeyboardInput(KeyboardInput::new("KeyA", true)),
            DesktopHostEvent::FocusChanged(false),
            DesktopHostEvent::Resized(hawk2ui_host::SurfaceMetrics::new(640.0, 480.0, 1.0)),
        ]);

        assert_eq!(
            inputs,
            vec![
                FrameInput::Pointer {
                    x: 12.5,
                    y: 34.0,
                    button: "left-down".to_string(),
                },
                FrameInput::Key {
                    key: "KeyA".to_string(),
                    pressed: true,
                },
                FrameInput::Focus { focused: false },
            ]
        );
    }

    #[test]
    fn script_entry_frame_applies_current_host_events_and_threads_ui_state() {
        let entry = WinitDesktopScriptEntry::new(
            "src/entry.js",
            r#"
export function mount(host) {
    const seen = [];
    host.on("pointer", function (event) {
        seen.push(event.button + "@" + event.x);
    });
    const prior = host.ui && host.ui.count ? host.ui.count : 0;
    host.setUi({ count: prior + seen.length });
    return {
        id: "root",
        type: "view",
        children: [{ id: "title", type: "text", text: seen.length ? "event:" + seen[0] : "idle" }]
    };
}
"#,
            HostSnapshot::default(),
        );

        let frame = run_script_entry_frame(
            &entry,
            &[FrameInput::Pointer {
                x: 12.5,
                y: 34.0,
                button: "left-down".to_string(),
            }],
            "null",
            320.0,
            200.0,
        )
        .unwrap_or_else(|error| panic!("script entry frame runs: {error:?}"));

        assert_eq!(frame.ui_json, r#"{"count":1}"#);
        let scene = RuntimeSceneBridge::new(Viewport::new(320.0, 200.0))
            .build(&frame.runtime_tree)
            .unwrap_or_else(|error| panic!("script frame produces a runtime scene: {error:?}"));
        assert!(scene.draw_commands().iter().any(|command| matches!(
            command,
            RuntimeDrawCommand::Text { text, .. } if text == "event:left-down@12.5"
        )));
    }

    #[test]
    fn framework_pointer_input_hit_tests_scene_and_executes_handler() {
        let program = FrameworkNativeProgram::new(
            FrameworkNativeNode::new("root", ElementKind::View)
                .with_prop("width", PropValue::Number(320.0))
                .with_prop("height", PropValue::Number(200.0))
                .with_child(
                    "button",
                    FrameworkNativeNode::new("button", ElementKind::Button)
                        .with_prop("width", PropValue::Number(120.0))
                        .with_prop("height", PropValue::Number(48.0))
                        .with_event(
                            EventKind::Pointer(PointerEventKind::Press),
                            HandlerRef::new("handlePress"),
                            [EventPayloadField::Position],
                        )
                        .with_child(
                            "label",
                            FrameworkNativeNode::new("label", ElementKind::Text)
                                .with_prop("width", PropValue::Number(120.0))
                                .with_prop("height", PropValue::Number(32.0)),
                        ),
                ),
        )
        .with_initial_dynamic_value(FrameworkInitialDynamicValue::value(
            "label",
            FrameworkDynamicValue::String("Idle".to_string()),
        ))
        .with_dynamic_binding(FrameworkDynamicBinding::prop(
            "label",
            "text",
            "label",
            vec!["label".to_string()],
        ))
        .with_event_handler(FrameworkEventHandler::new("handlePress").with_action(
            FrameworkEventHandlerAction::set_dynamic_value(
                "label",
                FrameworkDynamicValue::String("Pressed".to_string()),
            ),
        ));
        let native = program
            .to_native_authoring_artifact("App.tsx", true)
            .unwrap_or_else(|error| panic!("program finalizes: {error:?}"));
        let runtime = NativeRuntimeBridge::new()
            .bridge_artifact(&native)
            .unwrap_or_else(|error| panic!("program bridges: {error:?}"));
        let mut controller = FrameworkRuntimeController::from_program(&program, runtime)
            .unwrap_or_else(|error| panic!("controller builds: {error:?}"));
        let scene = RuntimeSceneBridge::new(Viewport::new(320.0, 200.0))
            .build(controller.runtime_tree())
            .unwrap_or_else(|error| panic!("scene builds: {error:?}"));

        let event = framework_event_from_pointer_input(
            &scene,
            &PointerInput::new(16.0, 16.0, "left-down"),
            &controller,
        )
        .unwrap_or_else(|| panic!("button receives pointer press"));
        let changed = controller
            .dispatch_runtime_event(&event)
            .unwrap_or_else(|error| panic!("handler executes: {error:?}"));

        assert!(changed);
        let label = controller
            .runtime_tree()
            .node(&RuntimeViewId::new("label"))
            .unwrap_or_else(|| panic!("label node exists"));
        assert!(matches!(
            label.visual(),
            RuntimeVisual::Text(text) if text.text() == "Pressed"
        ));

        let outside = framework_event_from_pointer_input(
            &scene,
            &PointerInput::new(250.0, 180.0, "left-down"),
            &controller,
        );
        assert!(outside.is_none());
    }

    #[test]
    fn framework_pointer_input_maps_supported_native_pointer_labels() {
        let mut button = FrameworkNativeNode::new("button", ElementKind::Button)
            .with_prop("width", PropValue::Number(120.0))
            .with_prop("height", PropValue::Number(48.0));
        for event in [
            EventKind::Pointer(PointerEventKind::Press),
            EventKind::Pointer(PointerEventKind::Release),
            EventKind::Pointer(PointerEventKind::Move),
            EventKind::Pointer(PointerEventKind::Drag),
            EventKind::Pointer(PointerEventKind::Enter),
            EventKind::Pointer(PointerEventKind::Leave),
            EventKind::Pointer(PointerEventKind::Wheel),
        ] {
            button = button.with_event(
                event,
                HandlerRef::new("handlePointer"),
                [EventPayloadField::Position],
            );
        }
        let program = FrameworkNativeProgram::new(
            FrameworkNativeNode::new("root", ElementKind::View)
                .with_prop("width", PropValue::Number(320.0))
                .with_prop("height", PropValue::Number(200.0))
                .with_child("button", button),
        )
        .with_event_handler(FrameworkEventHandler::new("handlePointer").with_action(
            FrameworkEventHandlerAction::set_dynamic_value(
                "label",
                FrameworkDynamicValue::String("Handled".to_string()),
            ),
        ));
        let native = program
            .to_native_authoring_artifact("App.tsx", true)
            .unwrap_or_else(|error| panic!("program finalizes: {error:?}"));
        let runtime = NativeRuntimeBridge::new()
            .bridge_artifact(&native)
            .unwrap_or_else(|error| panic!("program bridges: {error:?}"));
        let controller = FrameworkRuntimeController::from_program(&program, runtime)
            .unwrap_or_else(|error| panic!("controller builds: {error:?}"));
        let scene = RuntimeSceneBridge::new(Viewport::new(320.0, 200.0))
            .build(controller.runtime_tree())
            .unwrap_or_else(|error| panic!("scene builds: {error:?}"));

        for (button_label, expected_name) in [
            ("left-down", "pointer.press"),
            ("left-up", "pointer.release"),
            ("move", "pointer.move"),
            ("drag", "pointer.drag"),
            ("left-drag", "pointer.drag"),
            ("enter", "pointer.enter"),
            ("leave", "pointer.leave"),
            ("wheel-lines:0:1", "pointer.wheel"),
            ("wheel-pixels:0:16", "pointer.wheel"),
        ] {
            let event = framework_event_from_pointer_input(
                &scene,
                &PointerInput::new(16.0, 16.0, button_label),
                &controller,
            )
            .unwrap_or_else(|| panic!("{button_label} should map to {expected_name}"));
            assert_eq!(event.target, "button");
            assert_eq!(event.name, expected_name);
            let StructuredValue::Object(payload) = &event.payload else {
                panic!("pointer events should carry object payloads");
            };
            assert_eq!(
                payload.get("name"),
                Some(&StructuredValue::String(expected_name.to_string()))
            );
            assert_eq!(
                payload.get("target"),
                Some(&StructuredValue::String("button".to_string()))
            );
            assert_eq!(
                payload.get("button"),
                Some(&StructuredValue::String(button_label.to_string()))
            );
            assert_eq!(payload.get("x"), Some(&StructuredValue::Number(16.0)));
            assert_eq!(payload.get("y"), Some(&StructuredValue::Number(16.0)));
            if button_label == "wheel-lines:0:1" {
                assert_eq!(payload.get("delta_x"), Some(&StructuredValue::Number(0.0)));
                assert_eq!(payload.get("delta_y"), Some(&StructuredValue::Number(1.0)));
            }
            if button_label == "wheel-pixels:0:16" {
                assert_eq!(payload.get("delta_x"), Some(&StructuredValue::Number(0.0)));
                assert_eq!(payload.get("delta_y"), Some(&StructuredValue::Number(16.0)));
            }
        }
    }

    fn framework_host_event_payload_fixture() -> (FrameworkRuntimeController, RuntimeSceneFrame) {
        let program = FrameworkNativeProgram::new(
            FrameworkNativeNode::new("root", ElementKind::View)
                .with_prop("width", PropValue::Number(320.0))
                .with_prop("height", PropValue::Number(200.0))
                .with_event(
                    EventKind::Keyboard(hawk2ui_authoring::KeyboardEventKind::KeyDown),
                    HandlerRef::new("handleKeyboard"),
                    [EventPayloadField::Key],
                )
                .with_event(
                    EventKind::Keyboard(hawk2ui_authoring::KeyboardEventKind::KeyUp),
                    HandlerRef::new("handleKeyboard"),
                    [EventPayloadField::Key],
                )
                .with_event(
                    EventKind::Keyboard(hawk2ui_authoring::KeyboardEventKind::TextInput),
                    HandlerRef::new("handleKeyboard"),
                    [EventPayloadField::Value],
                )
                .with_event(
                    EventKind::Focus(hawk2ui_authoring::FocusEventKind::FocusIn),
                    HandlerRef::new("handleFocus"),
                    [],
                )
                .with_event(
                    EventKind::Focus(hawk2ui_authoring::FocusEventKind::FocusOut),
                    HandlerRef::new("handleFocus"),
                    [],
                )
                .with_event(EventKind::Resize, HandlerRef::new("handleResize"), []),
        )
        .with_event_handler(FrameworkEventHandler::new("handleKeyboard").with_action(
            FrameworkEventHandlerAction::set_dynamic_value(
                "label",
                FrameworkDynamicValue::String("Keyboard".to_string()),
            ),
        ))
        .with_event_handler(FrameworkEventHandler::new("handleFocus").with_action(
            FrameworkEventHandlerAction::set_dynamic_value(
                "label",
                FrameworkDynamicValue::String("Focus".to_string()),
            ),
        ))
        .with_event_handler(FrameworkEventHandler::new("handleResize").with_action(
            FrameworkEventHandlerAction::set_dynamic_value(
                "label",
                FrameworkDynamicValue::String("Resize".to_string()),
            ),
        ));
        let native = program
            .to_native_authoring_artifact("App.tsx", true)
            .unwrap_or_else(|error| panic!("program finalizes: {error:?}"));
        let runtime = NativeRuntimeBridge::new()
            .bridge_artifact(&native)
            .unwrap_or_else(|error| panic!("program bridges: {error:?}"));
        let controller = FrameworkRuntimeController::from_program(&program, runtime)
            .unwrap_or_else(|error| panic!("controller builds: {error:?}"));
        let scene = RuntimeSceneBridge::new(Viewport::new(320.0, 200.0))
            .build(controller.runtime_tree())
            .unwrap_or_else(|error| panic!("scene builds: {error:?}"));
        (controller, scene)
    }

    #[test]
    fn framework_host_events_route_keyboard_focus_and_resize_handlers() {
        let (controller, scene) = framework_host_event_payload_fixture();
        for (host_event, expected_name) in [
            (
                DesktopHostEvent::KeyboardInput(KeyboardInput::new("KeyA", true)),
                "keyboard.key-down",
            ),
            (
                DesktopHostEvent::KeyboardInput(KeyboardInput::new("KeyA", false)),
                "keyboard.key-up",
            ),
            (DesktopHostEvent::FocusChanged(true), "focus.focus-in"),
            (DesktopHostEvent::FocusChanged(false), "focus.focus-out"),
            (
                DesktopHostEvent::Resized(SurfaceMetrics::new(640.0, 480.0, 1.0)),
                "resize",
            ),
        ] {
            let events = framework_events_from_host_event(&scene, &host_event, &controller);
            assert_eq!(events.len(), 1);
            assert_eq!(events[0].target, "root");
            assert_eq!(events[0].name, expected_name);
            let StructuredValue::Object(payload) = &events[0].payload else {
                panic!("framework host events should carry object payloads");
            };
            assert_eq!(
                payload.get("name"),
                Some(&StructuredValue::String(expected_name.to_string()))
            );
            assert_eq!(
                payload.get("target"),
                Some(&StructuredValue::String("root".to_string()))
            );
            match (&host_event, expected_name) {
                (DesktopHostEvent::KeyboardInput(keyboard), _) => {
                    assert_eq!(
                        payload.get("key"),
                        Some(&StructuredValue::String(keyboard.key.clone()))
                    );
                    assert_eq!(
                        payload.get("pressed"),
                        Some(&StructuredValue::Bool(keyboard.pressed))
                    );
                }
                (DesktopHostEvent::FocusChanged(focused), _) => {
                    assert_eq!(
                        payload.get("focused"),
                        Some(&StructuredValue::Bool(*focused))
                    );
                }
                (DesktopHostEvent::Resized(metrics), _) => {
                    assert_eq!(
                        payload.get("width"),
                        Some(&StructuredValue::Number(metrics.logical_width))
                    );
                    assert_eq!(
                        payload.get("height"),
                        Some(&StructuredValue::Number(metrics.logical_height))
                    );
                    assert_eq!(
                        payload.get("scale_factor"),
                        Some(&StructuredValue::Number(metrics.scale_factor))
                    );
                }
                _ => {}
            }
        }

        let text_events = framework_events_from_host_event(
            &scene,
            &DesktopHostEvent::ImeInput("commit:hello".to_string()),
            &controller,
        );
        assert_eq!(text_events.len(), 1);
        let StructuredValue::Object(payload) = &text_events[0].payload else {
            panic!("text input events should carry object payloads");
        };
        assert_eq!(
            payload.get("name"),
            Some(&StructuredValue::String("keyboard.text-input".to_string()))
        );
        assert_eq!(
            payload.get("value"),
            Some(&StructuredValue::String("hello".to_string()))
        );
    }

    #[test]
    fn runtime_application_dispatches_framework_unmounted_lifecycle_before_teardown() {
        let program = FrameworkNativeProgram::new(
            FrameworkNativeNode::new("root", ElementKind::View)
                .with_prop("width", PropValue::Number(320.0))
                .with_prop("height", PropValue::Number(200.0))
                .with_lifecycle(
                    NativeLifecycleEvent::Unmounted,
                    HandlerRef::new("handleUnmount"),
                )
                .with_child(
                    "label",
                    FrameworkNativeNode::new("label", ElementKind::Text)
                        .with_prop("width", PropValue::Number(120.0))
                        .with_prop("height", PropValue::Number(32.0)),
                ),
        )
        .with_initial_dynamic_value(FrameworkInitialDynamicValue::value(
            "label",
            FrameworkDynamicValue::String("Mounted".to_string()),
        ))
        .with_dynamic_binding(FrameworkDynamicBinding::prop(
            "label",
            "text",
            "label",
            vec!["label".to_string()],
        ))
        .with_event_handler(FrameworkEventHandler::new("handleUnmount").with_action(
            FrameworkEventHandlerAction::set_dynamic_value(
                "label",
                FrameworkDynamicValue::String("Unmounted".to_string()),
            ),
        ));
        let native = program
            .to_native_authoring_artifact("App.tsx", true)
            .unwrap_or_else(|error| panic!("program finalizes: {error:?}"));
        let runtime = NativeRuntimeBridge::new()
            .bridge_artifact(&native)
            .unwrap_or_else(|error| panic!("program bridges: {error:?}"));
        let controller = FrameworkRuntimeController::from_program(&program, runtime)
            .unwrap_or_else(|error| panic!("controller builds: {error:?}"));
        let config = WinitDesktopRuntimeConfig::new(DesktopWindowConfig::new(
            "Lifecycle",
            SurfaceMetrics::new(320.0, 200.0, 1.0),
        ))
        .with_framework_controller(controller);
        let mut app = RuntimeApplication::new(config, SoftwareFrameRenderer::new());

        app.handle_exiting();

        let label = app
            .config
            .framework_controller()
            .unwrap_or_else(|| panic!("framework controller remains inspectable after teardown"))
            .runtime_tree()
            .node(&RuntimeViewId::new("label"))
            .unwrap_or_else(|| panic!("label node exists"));
        assert!(matches!(
            label.visual(),
            RuntimeVisual::Text(text) if text.text() == "Unmounted"
        ));
    }
}
