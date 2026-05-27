//! Production `winit` desktop event-loop runtime.

use std::{num::NonZeroU32, sync::Arc, time::Instant};

use hawk2ui_assets::AssetRecord;
use hawk2ui_host::DesktopWindowConfig;
use hawk2ui_layout::Viewport;
use hawk2ui_runtime::{
    AnimationCadencePolicy, AnimationFrameScheduler, RuntimeSceneBridge, RuntimeViewTree,
};
use softbuffer::{Context, Surface};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalSize},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

use crate::{SoftwareFrameRenderer, WinitHostError, physical_frame_size};

/// Production desktop runtime configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct WinitDesktopRuntimeConfig {
    window: DesktopWindowConfig,
    exit_after_first_frame: bool,
    runtime_tree: Option<RuntimeViewTree>,
    runtime_assets: Vec<AssetRecord>,
    animation_policy: AnimationCadencePolicy,
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

/// Summary returned after the desktop runtime exits.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WinitDesktopRuntimeSummary {
    /// Whether a native window was created.
    pub window_created: bool,
    /// Number of full frames presented.
    pub frames_presented: u64,
    /// Number of resize events processed.
    pub resizes: u64,
    /// Number of DPI change events processed.
    pub dpi_changes: u64,
    /// Number of input or focus events processed.
    pub input_events: u64,
    /// Number of animation ticks accepted before presentation.
    pub animation_ticks: u64,
    /// Whether a close request was received.
    pub close_requested: bool,
}

/// Runtime event category used for host repaint policy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DesktopRuntimeEvent {
    /// Native surface was resized.
    Resized {
        /// New physical width.
        physical_width: u32,
        /// New physical height.
        physical_height: u32,
        /// Current scale factor.
        scale_factor: f64,
    },
    /// Native DPI scale changed.
    DpiChanged {
        /// Current scale factor rounded for deterministic classification.
        scale_factor: f64,
    },
    /// Keyboard input was received.
    KeyboardInput,
    /// Pointer input was received.
    PointerInput,
    /// Focus state changed.
    FocusChanged,
}

impl DesktopRuntimeEvent {
    /// Returns whether the event requires full-surface repaint.
    #[must_use]
    pub const fn requires_full_repaint(self) -> bool {
        matches!(self, Self::Resized { .. } | Self::DpiChanged { .. })
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
}

struct RuntimeApplication {
    config: WinitDesktopRuntimeConfig,
    renderer: SoftwareFrameRenderer,
    window: Option<Arc<Window>>,
    context: Option<Context<Arc<Window>>>,
    surface: Option<Surface<Arc<Window>, Arc<Window>>>,
    lifecycle: RuntimeLifecycle,
    animation: AnimationFrameScheduler,
    started_at: Instant,
    last_error: Option<WinitHostError>,
}

impl RuntimeApplication {
    fn new(config: WinitDesktopRuntimeConfig, renderer: SoftwareFrameRenderer) -> Self {
        let animation = AnimationFrameScheduler::new(config.animation_policy());
        let renderer = renderer.with_assets(config.runtime_assets().iter().cloned());
        Self {
            config,
            renderer,
            window: None,
            context: None,
            surface: None,
            lifecycle: RuntimeLifecycle::default(),
            animation,
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

    fn create_window(&mut self, event_loop: &ActiveEventLoop) -> Result<(), WinitHostError> {
        let metrics = self.config.window.metrics;
        let attributes = Window::default_attributes()
            .with_title(self.config.window.title.clone())
            .with_inner_size(LogicalSize::new(
                metrics.logical_width,
                metrics.logical_height,
            ));
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
        self.context = Some(context);
        self.surface = Some(surface);
        self.window = Some(window);
        self.request_redraw();
        Ok(())
    }

    fn resize_surface(&mut self, size: PhysicalSize<u32>) -> Result<(), WinitHostError> {
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
        self.lifecycle.record_resize();
        self.request_redraw();
        Ok(())
    }

    fn present_frame(&mut self, event_loop: &ActiveEventLoop) -> Result<(), WinitHostError> {
        if !self.lifecycle.accepts_frame_presentation() {
            return Ok(());
        }
        let Some(window) = self.window.as_ref() else {
            return Ok(());
        };
        let size = window.inner_size();
        let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return Ok(());
        };
        let Some(surface) = self.surface.as_mut() else {
            return Ok(());
        };
        surface.resize(width, height).map_err(|error| {
            WinitHostError::new(
                "desktop.surface.resize-failed",
                format!("failed to resize native presentation surface before render: {error}"),
            )
        })?;

        let frame = if let Some(runtime_tree) = self.config.runtime_tree() {
            let logical_width = f64::from(size.width) / window.scale_factor();
            let logical_height = f64::from(size.height) / window.scale_factor();
            let viewport = Viewport::new(
                logical_size_to_f32(logical_width)?,
                logical_size_to_f32(logical_height)?,
            );
            let scene = RuntimeSceneBridge::new(viewport)
                .build(runtime_tree)
                .map_err(|error| {
                    WinitHostError::new(
                        "desktop.runtime-scene.build-failed",
                        format!("failed to build runtime scene for desktop frame: {error:?}"),
                    )
                })?;
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
        self.lifecycle.record_frame_presented();
        if let Some(tick) = self.animation.step_at(self.elapsed_ms()) {
            self.lifecycle.record_animation_tick();
            if tick.reduced_rate_due {
                self.request_redraw();
            }
        }
        if self.config.exit_after_first_frame {
            event_loop.exit();
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

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: WinitHostError) {
        self.last_error = Some(error);
        event_loop.exit();
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

    fn record_frame_presented(&mut self) {
        if self.accepts_frame_presentation() {
            self.summary.frames_presented += 1;
        }
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

    fn request_close(&mut self) -> bool {
        if self.summary.close_requested {
            false
        } else {
            self.summary.close_requested = true;
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RuntimeLifecycle, logical_size_to_f32};

    #[test]
    fn runtime_lifecycle_stops_presenting_frames_after_close() {
        let mut lifecycle = RuntimeLifecycle::default();

        assert!(lifecycle.accepts_frame_presentation());
        lifecycle.record_frame_presented();
        assert_eq!(lifecycle.summary().frames_presented, 1);

        assert!(lifecycle.request_close());
        assert!(!lifecycle.request_close());
        assert!(!lifecycle.accepts_host_event());
        assert!(!lifecycle.accepts_frame_presentation());
        assert!(lifecycle.summary().close_requested);

        lifecycle.record_resize();
        lifecycle.record_dpi_change();
        lifecycle.record_input_event();
        lifecycle.record_frame_presented();
        assert_eq!(lifecycle.summary().resizes, 0);
        assert_eq!(lifecycle.summary().dpi_changes, 0);
        assert_eq!(lifecycle.summary().input_events, 0);
        assert_eq!(lifecycle.summary().frames_presented, 1);
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
    fn logical_size_to_f32_rejects_invalid_viewport_values() {
        assert_eq!(logical_size_to_f32(1280.0).expect("valid size"), 1280.0);
        assert!(logical_size_to_f32(0.0).is_err());
        assert!(logical_size_to_f32(f64::INFINITY).is_err());
        assert!(logical_size_to_f32(f64::MAX).is_err());
    }
}

impl ApplicationHandler for RuntimeApplication {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none()
            && let Err(error) = self.create_window(event_loop)
        {
            self.fail(event_loop, error);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if !self.lifecycle.accepts_host_event() && !matches!(&event, &WindowEvent::CloseRequested) {
            return;
        }
        let result = match event {
            WindowEvent::CloseRequested => {
                self.lifecycle.request_close();
                event_loop.exit();
                Ok(())
            }
            WindowEvent::Resized(size) => self.resize_surface(size),
            WindowEvent::ScaleFactorChanged { .. } => {
                self.lifecycle.record_dpi_change();
                self.request_redraw();
                Ok(())
            }
            WindowEvent::RedrawRequested => self.present_frame(event_loop),
            WindowEvent::Focused(_)
            | WindowEvent::KeyboardInput { .. }
            | WindowEvent::CursorMoved { .. }
            | WindowEvent::CursorEntered { .. }
            | WindowEvent::CursorLeft { .. }
            | WindowEvent::MouseInput { .. }
            | WindowEvent::MouseWheel { .. }
            | WindowEvent::ModifiersChanged(_) => {
                self.lifecycle.record_input_event();
                Ok(())
            }
            _ => Ok(()),
        };
        if let Err(error) = result {
            self.fail(event_loop, error);
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if self.animation.should_request_frame(self.elapsed_ms())
            && self.lifecycle.accepts_frame_presentation()
        {
            self.request_redraw();
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.surface = None;
        self.context = None;
        self.window = None;
    }
}
