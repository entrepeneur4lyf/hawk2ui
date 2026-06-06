//! Skia Ganesh GPU presentation for native `winit` desktop windows.

use std::{ffi::CString, num::NonZeroU32, sync::Arc};

use glutin::{
    config::{Config, ConfigTemplateBuilder, GlConfig},
    context::{ContextApi, ContextAttributesBuilder, NotCurrentContext, PossiblyCurrentContext},
    display::{Display, DisplayApiPreference, GetGlDisplay, GlDisplay},
    prelude::*,
    surface::{Surface as GlutinSurface, SurfaceAttributesBuilder, SwapInterval, WindowSurface},
};
use glutin_winit::GlWindow;
use hawk2ui_assets::AssetRecord;
use hawk2ui_render::{Color, RendererBackend, Transform};
use hawk2ui_render_skia::{
    RuntimeSceneAssetFallback, RuntimeSceneReplayOptions, SkiaFrameSnapshot, SkiaRendererBackend,
    SkiaSurfaceKind,
};
use hawk2ui_runtime::RuntimeSceneFrame;
use skia_safe::{
    ColorType,
    gpu::{
        DirectContext, Protected, SurfaceOrigin, backend_render_targets, direct_contexts,
        gl::{FramebufferInfo, Interface},
        surfaces,
    },
};
use winit::{
    dpi::PhysicalSize,
    event_loop::ActiveEventLoop,
    raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle},
    window::{Window, WindowAttributes},
};

use crate::WinitHostError;
use crate::software_frame::{map_backend_error, register_runtime_assets, scale_factor_to_f32};

const GPU_FRAME_SURFACE_ID: &str = "winit-gpu-frame";
const GL_RGBA8: u32 = 0x8058;

/// Winit-owned GPU presenter for a native Wayland desktop window.
pub(crate) struct WinitGpuFramePresenter {
    gl_context: PossiblyCurrentContext,
    gl_surface: GlutinSurface<WindowSurface>,
    direct_context: DirectContext,
    backend: SkiaRendererBackend,
    assets: Vec<AssetRecord>,
    width: u32,
    height: u32,
    dpi_scale: f32,
    last_snapshot: Option<SkiaFrameSnapshot>,
}

impl WinitGpuFramePresenter {
    /// Creates a native Wayland Winit window and a Skia GPU presenter for it.
    pub(crate) fn create_wayland_window(
        event_loop: &ActiveEventLoop,
        attributes: WindowAttributes,
        assets: impl IntoIterator<Item = AssetRecord>,
    ) -> Result<(Arc<Window>, Self), WinitHostError> {
        let raw_display = event_loop
            .display_handle()
            .map_err(|error| {
                WinitHostError::new(
                    "desktop.gpu.display-handle-failed",
                    format!("failed to read native display handle: {error}"),
                )
            })?
            .as_raw();
        if !matches!(raw_display, RawDisplayHandle::Wayland(_)) {
            return Err(WinitHostError::new(
                "desktop.gpu.wayland-required",
                "Winit GPU presentation currently requires a native Wayland display",
            ));
        }

        let gl_display = create_egl_display(raw_display)?;
        let gl_config = choose_gl_config(&gl_display)?;
        let window =
            glutin_winit::finalize_window(event_loop, attributes, &gl_config).map_err(|error| {
                WinitHostError::new(
                    "desktop.gpu.window-create-failed",
                    format!("failed to create GPU-compatible native window: {error}"),
                )
            })?;
        let size = non_zero_size(window.inner_size())?;
        let gl_surface = create_window_surface(&window, &gl_config)?;
        let gl_context = create_gl_context(&window, &gl_config)?
            .make_current(&gl_surface)
            .map_err(|error| {
                WinitHostError::new(
                    "desktop.gpu.context-current-failed",
                    format!("failed to make the GPU context current: {error}"),
                )
            })?;
        let _ = gl_surface.set_swap_interval(&gl_context, SwapInterval::Wait(NonZeroU32::MIN));

        let mut presenter = Self {
            gl_context,
            gl_surface,
            direct_context: create_direct_context(&gl_config)?,
            backend: SkiaRendererBackend::new(),
            assets: assets.into_iter().collect(),
            width: 0,
            height: 0,
            dpi_scale: 1.0,
            last_snapshot: None,
        };
        presenter.rebuild_skia_surface(size.0.get(), size.1.get(), window.scale_factor())?;
        Ok((Arc::new(window), presenter))
    }

    /// Resizes the Wayland EGL surface and re-wraps the Skia GPU render target when dimensions or
    /// scale change.
    pub(crate) fn resize_to_window(&mut self, window: &Window) -> Result<(), WinitHostError> {
        let Ok(size) = non_zero_size(window.inner_size()) else {
            return Ok(());
        };
        self.gl_surface.resize(&self.gl_context, size.0, size.1);
        self.rebuild_skia_surface(size.0.get(), size.1.get(), window.scale_factor())
    }

    /// Presents one runtime scene frame through Skia GPU rendering and swaps the native buffers.
    pub(crate) fn present_scene_frame(
        &mut self,
        window: &Window,
        scene: &RuntimeSceneFrame,
        frame_index: u64,
    ) -> Result<(), WinitHostError> {
        self.resize_to_window(window)?;
        if !self.gl_context.is_current() {
            self.gl_context
                .make_current(&self.gl_surface)
                .map_err(|error| {
                    WinitHostError::new(
                        "desktop.gpu.context-current-failed",
                        format!("failed to make the GPU context current before rendering: {error}"),
                    )
                })?;
        }

        self.backend
            .begin_frame(GPU_FRAME_SURFACE_ID)
            .map_err(|error| map_backend_error(&error))?;
        self.backend
            .clear(Color::rgba(0, 0, 0, 0))
            .map_err(|error| map_backend_error(&error))?;
        self.backend
            .push_transform(Transform::affine(
                self.dpi_scale,
                0.0,
                0.0,
                self.dpi_scale,
                0.0,
                0.0,
            ))
            .map_err(|error| map_backend_error(&error))?;
        self.backend
            .draw_runtime_scene_frame_with_options(
                scene,
                RuntimeSceneReplayOptions::new(frame_index, self.dpi_scale)
                    .with_missing_asset_fallback(RuntimeSceneAssetFallback::Placeholder),
            )
            .map_err(|error| map_backend_error(&error))?;
        self.backend
            .end_frame(GPU_FRAME_SURFACE_ID)
            .map_err(|error| map_backend_error(&error))?;
        self.direct_context.flush_and_submit();
        self.last_snapshot = self
            .backend
            .read_surface_snapshot(GPU_FRAME_SURFACE_ID)
            .ok();
        window.pre_present_notify();
        self.gl_surface
            .swap_buffers(&self.gl_context)
            .map_err(|error| {
                WinitHostError::new(
                    "desktop.gpu.swap-failed",
                    format!("failed to swap the native GPU buffers: {error}"),
                )
            })
    }

    /// Returns the latest verification readback captured after a submitted GPU frame.
    #[must_use]
    pub(crate) fn last_snapshot(&self) -> Option<&SkiaFrameSnapshot> {
        self.last_snapshot.as_ref()
    }

    fn rebuild_skia_surface(
        &mut self,
        width: u32,
        height: u32,
        scale_factor: f64,
    ) -> Result<(), WinitHostError> {
        let dpi_scale = scale_factor_to_f32(scale_factor)?;
        if self.width == width
            && self.height == height
            && (self.dpi_scale - dpi_scale).abs() < f32::EPSILON
        {
            return Ok(());
        }
        let skia_width = i32::try_from(width).map_err(|_| gpu_size_error())?;
        let skia_height = i32::try_from(height).map_err(|_| gpu_size_error())?;
        let framebuffer_info = FramebufferInfo {
            fboid: 0,
            format: GL_RGBA8,
            protected: Protected::No,
        };
        let render_target =
            backend_render_targets::make_gl((skia_width, skia_height), None, 8, framebuffer_info);
        let surface = surfaces::wrap_backend_render_target(
            &mut self.direct_context,
            &render_target,
            SurfaceOrigin::BottomLeft,
            ColorType::RGBA8888,
            None,
            None,
        )
        .ok_or_else(|| {
            WinitHostError::new(
                "desktop.gpu.surface-wrap-failed",
                "failed to wrap the native GPU framebuffer as a Skia surface",
            )
        })?;
        let mut backend = SkiaRendererBackend::new();
        register_runtime_assets(&mut backend, &self.assets)?;
        backend
            .adopt_surface(
                GPU_FRAME_SURFACE_ID,
                surface,
                width,
                height,
                dpi_scale,
                SkiaSurfaceKind::GpuGl,
            )
            .map_err(|error| map_backend_error(&error))?;
        self.backend = backend;
        self.width = width;
        self.height = height;
        self.dpi_scale = dpi_scale;
        self.last_snapshot = None;
        Ok(())
    }
}

impl Drop for WinitGpuFramePresenter {
    fn drop(&mut self) {
        self.direct_context.abandon();
    }
}

fn non_zero_size(size: PhysicalSize<u32>) -> Result<(NonZeroU32, NonZeroU32), WinitHostError> {
    match (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) {
        (Some(width), Some(height)) => Ok((width, height)),
        _ => Err(WinitHostError::new(
            "desktop.gpu.zero-sized-surface",
            "GPU presentation requires a non-zero native surface size",
        )),
    }
}

#[allow(unsafe_code)]
fn create_egl_display(raw_display: RawDisplayHandle) -> Result<Display, WinitHostError> {
    // SAFETY: The raw display handle comes from Winit's active event loop and remains valid for the
    // lifetime of the window and GL objects created by this presenter.
    unsafe { Display::new(raw_display, DisplayApiPreference::Egl) }.map_err(|error| {
        WinitHostError::new(
            "desktop.gpu.display-create-failed",
            format!("failed to create a Wayland EGL display: {error}"),
        )
    })
}

#[allow(unsafe_code)]
fn choose_gl_config(gl_display: &Display) -> Result<Config, WinitHostError> {
    let template = ConfigTemplateBuilder::new()
        .with_alpha_size(8)
        .with_transparency(false)
        .build();
    // SAFETY: No native window handle is embedded in this template, and the display came from Winit.
    let mut configs = unsafe { gl_display.find_configs(template) }.map_err(|error| {
        WinitHostError::new(
            "desktop.gpu.config-query-failed",
            format!("failed to query Wayland EGL framebuffer configurations: {error}"),
        )
    })?;
    let Some(first) = configs.next() else {
        return Err(WinitHostError::new(
            "desktop.gpu.config-missing",
            "Wayland EGL returned no window-renderable framebuffer configurations",
        ));
    };
    Ok(configs.fold(first, |best, candidate| {
        if config_score(&candidate) > config_score(&best) {
            candidate
        } else {
            best
        }
    }))
}

fn config_score(config: &Config) -> i32 {
    let transparency = i32::from(config.supports_transparency().unwrap_or(false));
    i32::from(config.num_samples()) * 10 + transparency
}

#[allow(unsafe_code)]
fn create_window_surface(
    window: &Window,
    gl_config: &Config,
) -> Result<GlutinSurface<WindowSurface>, WinitHostError> {
    let attributes = window
        .build_surface_attributes(SurfaceAttributesBuilder::new().with_srgb(Some(false)))
        .map_err(|error| {
            WinitHostError::new(
                "desktop.gpu.surface-attributes-failed",
                format!("failed to build native GPU surface attributes: {error}"),
            )
        })?;
    // SAFETY: The attributes were built from the live Winit window handle, and the resulting
    // surface is dropped before the window is dropped by `RuntimeApplication`.
    unsafe {
        gl_config
            .display()
            .create_window_surface(gl_config, &attributes)
    }
    .map_err(|error| {
        WinitHostError::new(
            "desktop.gpu.surface-create-failed",
            format!("failed to create the native GPU window surface: {error}"),
        )
    })
}

#[allow(unsafe_code)]
fn create_gl_context(
    window: &Window,
    gl_config: &Config,
) -> Result<NotCurrentContext, WinitHostError> {
    let raw_window_handle = Some(
        window
            .window_handle()
            .map_err(|error| {
                WinitHostError::new(
                    "desktop.gpu.window-handle-failed",
                    format!("failed to read native window handle: {error}"),
                )
            })?
            .as_raw(),
    );
    let context_attributes = ContextAttributesBuilder::new().build(raw_window_handle);
    let fallback_context_attributes = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::Gles(None))
        .build(raw_window_handle);
    // SAFETY: The raw window handle came from the live Winit window, and the context is tied to the
    // Glutin display/config selected for that same display.
    unsafe {
        gl_config
            .display()
            .create_context(gl_config, &context_attributes)
            .or_else(|_| {
                gl_config
                    .display()
                    .create_context(gl_config, &fallback_context_attributes)
            })
    }
    .map_err(|error| {
        WinitHostError::new(
            "desktop.gpu.context-create-failed",
            format!("failed to create a Wayland EGL OpenGL context: {error}"),
        )
    })
}

fn create_direct_context(gl_config: &Config) -> Result<DirectContext, WinitHostError> {
    let display = gl_config.display();
    let interface =
        Interface::new_load_with(|symbol| gl_symbol(&display, symbol)).ok_or_else(|| {
            WinitHostError::new(
                "desktop.gpu.skia-interface-failed",
                "failed to load an OpenGL interface for Skia Ganesh",
            )
        })?;
    direct_contexts::make_gl(interface, None).ok_or_else(|| {
        WinitHostError::new(
            "desktop.gpu.skia-context-failed",
            "failed to create a Skia Ganesh GL DirectContext",
        )
    })
}

fn gl_symbol(display: &Display, symbol: &str) -> *const std::ffi::c_void {
    let Ok(name) = CString::new(symbol) else {
        return std::ptr::null();
    };
    display.get_proc_address(name.as_c_str()).cast()
}

fn gpu_size_error() -> WinitHostError {
    WinitHostError::new(
        "desktop.gpu.size-overflow",
        "GPU presentation surface size exceeds the Skia render-target range",
    )
}
