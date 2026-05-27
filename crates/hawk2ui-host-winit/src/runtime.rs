//! Production `winit` desktop event-loop runtime.

use std::{num::NonZeroU32, sync::Arc};

use hawk2ui_host::DesktopWindowConfig;
use hawk2ui_layout::Viewport;
use hawk2ui_runtime::{RuntimeSceneBridge, RuntimeViewTree};
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
}

impl WinitDesktopRuntimeConfig {
    /// Creates a production desktop runtime configuration.
    #[must_use]
    pub const fn new(window: DesktopWindowConfig) -> Self {
        Self {
            window,
            exit_after_first_frame: false,
            runtime_tree: None,
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
            renderer: SoftwareFrameRenderer,
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
    summary: WinitDesktopRuntimeSummary,
    last_error: Option<WinitHostError>,
}

impl RuntimeApplication {
    fn new(config: WinitDesktopRuntimeConfig, renderer: SoftwareFrameRenderer) -> Self {
        Self {
            config,
            renderer,
            window: None,
            context: None,
            surface: None,
            summary: WinitDesktopRuntimeSummary::default(),
            last_error: None,
        }
    }

    fn finish(self) -> Result<WinitDesktopRuntimeSummary, WinitHostError> {
        if let Some(error) = self.last_error {
            Err(error)
        } else {
            Ok(self.summary)
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

        self.summary.window_created = true;
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
        self.summary.resizes += 1;
        self.request_redraw();
        Ok(())
    }

    fn present_frame(&mut self, event_loop: &ActiveEventLoop) -> Result<(), WinitHostError> {
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
        self.summary.frames_presented += 1;
        if self.config.exit_after_first_frame {
            event_loop.exit();
        }
        Ok(())
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
    let value = value.to_string().parse::<f32>().map_err(|_| {
        WinitHostError::new(
            "desktop.runtime-scene.invalid-viewport",
            "runtime scene viewport dimension is invalid",
        )
    })?;
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(WinitHostError::new(
            "desktop.runtime-scene.invalid-viewport",
            "runtime scene viewport dimensions must be finite and greater than zero",
        ))
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
        let result = match event {
            WindowEvent::CloseRequested => {
                self.summary.close_requested = true;
                event_loop.exit();
                Ok(())
            }
            WindowEvent::Resized(size) => self.resize_surface(size),
            WindowEvent::ScaleFactorChanged { .. } => {
                self.summary.dpi_changes += 1;
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
                self.summary.input_events += 1;
                Ok(())
            }
            _ => Ok(()),
        };
        if let Err(error) = result {
            self.fail(event_loop, error);
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.surface = None;
        self.context = None;
        self.window = None;
    }
}
