use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle,
};

use crate::{MouseCursor, Size, WindowHandler, WindowOpenOptions};

pub struct WindowHandle {
    inner: WindowHandleInner,
}

enum WindowHandleInner {
    X11(crate::x11::WindowHandle),
    Wayland(crate::wayland::WindowHandle),
}

impl WindowHandle {
    pub fn close(&mut self) {
        match &mut self.inner {
            WindowHandleInner::X11(handle) => handle.close(),
            WindowHandleInner::Wayland(handle) => handle.close(),
        }
    }

    pub fn is_open(&self) -> bool {
        match &self.inner {
            WindowHandleInner::X11(handle) => handle.is_open(),
            WindowHandleInner::Wayland(handle) => handle.is_open(),
        }
    }
}

unsafe impl HasRawWindowHandle for WindowHandle {
    fn raw_window_handle(&self) -> RawWindowHandle {
        match &self.inner {
            WindowHandleInner::X11(handle) => handle.raw_window_handle(),
            WindowHandleInner::Wayland(handle) => handle.raw_window_handle(),
        }
    }
}

pub enum Window<'a> {
    X11(crate::x11::Window<'a>),
    Wayland(crate::wayland::Window<'a>),
}

impl<'a> Window<'a> {
    pub fn open_parented<P, H, B>(parent: &P, options: WindowOpenOptions, build: B) -> WindowHandle
    where
        P: HasRawDisplayHandle + HasRawWindowHandle,
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        match parent.raw_window_handle() {
            RawWindowHandle::Wayland(_) => WindowHandle {
                inner: WindowHandleInner::Wayland(crate::wayland::Window::open_parented(
                    parent, options, build,
                )),
            },
            RawWindowHandle::Xlib(_) | RawWindowHandle::Xcb(_) => WindowHandle {
                inner: WindowHandleInner::X11(crate::x11::Window::open_parented(
                    parent, options, build,
                )),
            },
            _ => WindowHandle {
                inner: WindowHandleInner::Wayland(crate::wayland::WindowHandle::closed()),
            },
        }
    }

    pub fn open_blocking<H, B>(options: WindowOpenOptions, build: B)
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        crate::x11::Window::open_blocking::<H, B>(options, build);
    }

    pub fn set_mouse_cursor(&self, mouse_cursor: MouseCursor) {
        match self {
            Self::X11(window) => window.set_mouse_cursor(mouse_cursor),
            Self::Wayland(window) => window.set_mouse_cursor(mouse_cursor),
        }
    }

    pub fn close(&mut self) {
        match self {
            Self::X11(window) => window.close(),
            Self::Wayland(window) => window.close(),
        }
    }

    pub fn has_focus(&mut self) -> bool {
        match self {
            Self::X11(window) => window.has_focus(),
            Self::Wayland(window) => window.has_focus(),
        }
    }

    pub fn focus(&mut self) {
        match self {
            Self::X11(window) => window.focus(),
            Self::Wayland(window) => window.focus(),
        }
    }

    pub fn resize(&mut self, size: Size) {
        match self {
            Self::X11(window) => window.resize(size),
            Self::Wayland(window) => window.resize(size),
        }
    }

    pub fn hawk2ui_present_software_frame(
        &self, width: u32, height: u32, xrgb_pixels: &[u8],
    ) -> Result<(), String> {
        match self {
            Self::X11(_) => {
                Err("Baseview X11 presentation is handled by Hawk2UI's X11 presenter".to_owned())
            }
            Self::Wayland(window) => {
                window.hawk2ui_present_software_frame(width, height, xrgb_pixels)
            }
        }
    }

    #[cfg(feature = "opengl")]
    pub fn gl_context(&self) -> Option<&crate::gl::GlContext> {
        match self {
            Self::X11(window) => window.gl_context(),
            Self::Wayland(window) => window.gl_context(),
        }
    }
}

unsafe impl<'a> HasRawWindowHandle for Window<'a> {
    fn raw_window_handle(&self) -> RawWindowHandle {
        match self {
            Self::X11(window) => window.raw_window_handle(),
            Self::Wayland(window) => window.raw_window_handle(),
        }
    }
}

unsafe impl<'a> HasRawDisplayHandle for Window<'a> {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        match self {
            Self::X11(window) => window.raw_display_handle(),
            Self::Wayland(window) => window.raw_display_handle(),
        }
    }
}
