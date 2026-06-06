use super::{GlConfig, Profile};
use std::{error::Error, ffi::c_void, fmt};

use khronos_egl as egl;
use wayland_client::protocol::wl_surface;
use wayland_client::Proxy;
use wayland_egl::WlEglSurface;

type Egl = egl::DynamicInstance<egl::EGL1_4>;

#[derive(Debug)]
pub enum CreationFailedError {
    EglLibrary(String),
    NoDisplay,
    EglInitialize(String),
    BindOpenGl(String),
    NoConfig,
    ContextCreation(String),
    WaylandEglUnavailable,
    WaylandEglWindow(String),
    SurfaceCreation(String),
    MakeCurrent(String),
    SwapInterval(String),
    InvalidSurfaceSize,
}

impl fmt::Display for CreationFailedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EglLibrary(error) => write!(formatter, "failed to load EGL library: {error}"),
            Self::NoDisplay => write!(formatter, "EGL did not return a Wayland display"),
            Self::EglInitialize(error) => write!(formatter, "failed to initialize EGL: {error}"),
            Self::BindOpenGl(error) => write!(formatter, "failed to bind EGL OpenGL API: {error}"),
            Self::NoConfig => write!(formatter, "no EGL framebuffer config matched the GL request"),
            Self::ContextCreation(error) => {
                write!(formatter, "failed to create EGL OpenGL context: {error}")
            }
            Self::WaylandEglUnavailable => write!(formatter, "libwayland-egl is unavailable"),
            Self::WaylandEglWindow(error) => {
                write!(formatter, "failed to create Wayland EGL window: {error}")
            }
            Self::SurfaceCreation(error) => {
                write!(formatter, "failed to create EGL window surface: {error}")
            }
            Self::MakeCurrent(error) => {
                write!(formatter, "failed to make EGL context current: {error}")
            }
            Self::SwapInterval(error) => {
                write!(formatter, "failed to set EGL swap interval: {error}")
            }
            Self::InvalidSurfaceSize => write!(formatter, "Wayland EGL surface size is invalid"),
        }
    }
}

impl Error for CreationFailedError {}

pub struct GlContext {
    egl: Egl,
    display: egl::Display,
    context: egl::Context,
    surface: egl::Surface,
    egl_window: WlEglSurface,
}

impl GlContext {
    pub fn create(
        display: *mut c_void, surface: &wl_surface::WlSurface, width: u32, height: u32,
        config: GlConfig,
    ) -> Result<Self, CreationFailedError> {
        let (width, height) = egl_surface_size(width, height)?;
        let egl = unsafe { Egl::load_required() }
            .map_err(|error| CreationFailedError::EglLibrary(format!("{error:?}")))?;
        let display = unsafe { egl.get_display(display) }.ok_or(CreationFailedError::NoDisplay)?;
        egl.initialize(display)
            .map_err(|error| CreationFailedError::EglInitialize(error.to_string()))?;
        egl.bind_api(egl::OPENGL_API)
            .map_err(|error| CreationFailedError::BindOpenGl(error.to_string()))?;

        let egl_config = egl
            .choose_first_config(display, &config_attributes(&config))
            .map_err(|error| CreationFailedError::ContextCreation(error.to_string()))?
            .ok_or(CreationFailedError::NoConfig)?;
        let context = egl
            .create_context(display, egl_config, None, &context_attributes(&config))
            .map_err(|error| CreationFailedError::ContextCreation(error.to_string()))?;

        if !wayland_egl::is_available() {
            let _ = egl.destroy_context(display, context);
            return Err(CreationFailedError::WaylandEglUnavailable);
        }

        let egl_window = match WlEglSurface::new(surface.id(), width, height) {
            Ok(window) => window,
            Err(error) => {
                let _ = egl.destroy_context(display, context);
                return Err(CreationFailedError::WaylandEglWindow(error.to_string()));
            }
        };
        let egl_surface =
            match create_window_surface(&egl, display, egl_config, &egl_window, &config) {
                Ok(surface) => surface,
                Err(error) => {
                    let _ = egl.destroy_context(display, context);
                    return Err(error);
                }
            };

        if let Err(error) =
            egl.make_current(display, Some(egl_surface), Some(egl_surface), Some(context))
        {
            let _ = egl.destroy_surface(display, egl_surface);
            let _ = egl.destroy_context(display, context);
            return Err(CreationFailedError::MakeCurrent(error.to_string()));
        }
        if let Err(error) = egl.swap_interval(display, i32::from(config.vsync)) {
            let _ = egl.make_current(display, None, None, None);
            let _ = egl.destroy_surface(display, egl_surface);
            let _ = egl.destroy_context(display, context);
            return Err(CreationFailedError::SwapInterval(error.to_string()));
        }
        let _ = egl.make_current(display, None, None, None);

        Ok(Self { egl, display, context, surface: egl_surface, egl_window })
    }

    pub unsafe fn make_current(&self) {
        let _ = self.egl.make_current(
            self.display,
            Some(self.surface),
            Some(self.surface),
            Some(self.context),
        );
    }

    pub unsafe fn make_not_current(&self) {
        let _ = self.egl.make_current(self.display, None, None, None);
    }

    pub fn get_proc_address(&self, symbol: &str) -> *const c_void {
        if symbol.as_bytes().contains(&0) {
            return std::ptr::null();
        }
        self.egl
            .get_proc_address(symbol)
            .map_or(std::ptr::null(), |address| address as *const () as *const c_void)
    }

    pub fn swap_buffers(&self) {
        let _ = self.egl.swap_buffers(self.display, self.surface);
    }

    pub fn resize(&self, width: u32, height: u32) {
        if let Ok((width, height)) = egl_surface_size(width, height) {
            self.egl_window.resize(width, height, 0, 0);
        }
    }
}

impl Drop for GlContext {
    fn drop(&mut self) {
        let _ = self.egl.make_current(self.display, None, None, None);
        let _ = self.egl.destroy_surface(self.display, self.surface);
        let _ = self.egl.destroy_context(self.display, self.context);
    }
}

fn create_window_surface(
    egl: &Egl, display: egl::Display, config: egl::Config, window: &WlEglSurface,
    gl_config: &GlConfig,
) -> Result<egl::Surface, CreationFailedError> {
    let srgb_attributes = [egl::GL_COLORSPACE, egl::GL_COLORSPACE_SRGB, egl::NONE];
    if gl_config.srgb {
        if let Ok(surface) = unsafe {
            egl.create_window_surface(
                display,
                config,
                window.ptr().cast_mut(),
                Some(&srgb_attributes),
            )
        } {
            return Ok(surface);
        }
    }
    unsafe { egl.create_window_surface(display, config, window.ptr().cast_mut(), None) }
        .map_err(|error| CreationFailedError::SurfaceCreation(error.to_string()))
}

fn config_attributes(config: &GlConfig) -> Vec<egl::Int> {
    let mut attributes = vec![
        egl::SURFACE_TYPE,
        egl::WINDOW_BIT,
        egl::RENDERABLE_TYPE,
        egl::OPENGL_BIT,
        egl::CONFORMANT,
        egl::OPENGL_BIT,
        egl::RED_SIZE,
        i32::from(config.red_bits),
        egl::GREEN_SIZE,
        i32::from(config.green_bits),
        egl::BLUE_SIZE,
        i32::from(config.blue_bits),
        egl::ALPHA_SIZE,
        i32::from(config.alpha_bits),
        egl::DEPTH_SIZE,
        i32::from(config.depth_bits),
        egl::STENCIL_SIZE,
        i32::from(config.stencil_bits),
    ];
    if let Some(samples) = config.samples {
        if samples > 0 {
            attributes.extend([egl::SAMPLE_BUFFERS, 1, egl::SAMPLES, i32::from(samples)]);
        }
    }
    attributes.push(egl::NONE);
    attributes
}

fn context_attributes(config: &GlConfig) -> [egl::Int; 8] {
    let profile = match config.profile {
        Profile::Compatibility => egl::CONTEXT_OPENGL_COMPATIBILITY_PROFILE_BIT,
        Profile::Core => egl::CONTEXT_OPENGL_CORE_PROFILE_BIT,
    };
    [
        egl::CONTEXT_MAJOR_VERSION,
        i32::from(config.version.0),
        egl::CONTEXT_MINOR_VERSION,
        i32::from(config.version.1),
        egl::CONTEXT_OPENGL_PROFILE_MASK,
        profile,
        egl::NONE,
        egl::NONE,
    ]
}

fn egl_surface_size(width: u32, height: u32) -> Result<(i32, i32), CreationFailedError> {
    let width = i32::try_from(width.max(1)).map_err(|_| CreationFailedError::InvalidSurfaceSize)?;
    let height =
        i32::try_from(height.max(1)).map_err(|_| CreationFailedError::InvalidSurfaceSize)?;
    Ok((width, height))
}
