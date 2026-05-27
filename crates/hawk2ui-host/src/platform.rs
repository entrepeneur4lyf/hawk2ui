//! Platform-native host handle records.

use hawk2ui_api::Diagnostic;
use serde::{Deserialize, Serialize};

/// Linux window system represented by a platform handle.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum LinuxWindowSystem {
    /// Native Wayland.
    Wayland,
    /// X11 window.
    X11,
    /// XCB window.
    Xcb,
    /// `XWayland` surface.
    XWayland,
}

/// Surface ownership model.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SurfaceOwnership {
    /// `Hawk2UI` owns the desktop top-level window.
    DesktopWindow,
    /// Host owns the parent surface for an embedded plugin editor.
    PluginEditor,
}

/// Platform handle validation diagnostic.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PlatformHandleDiagnostic {
    /// Stable diagnostic code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
}

impl PlatformHandleDiagnostic {
    fn ownership_mismatch(message: impl Into<String>) -> Self {
        Self {
            code: "platform.handle-ownership-mismatch".into(),
            message: message.into(),
        }
    }
}

impl From<PlatformHandleDiagnostic> for Diagnostic {
    fn from(diagnostic: PlatformHandleDiagnostic) -> Self {
        Self::error(diagnostic.code, diagnostic.message)
    }
}

/// Native platform handle record.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum HostPlatformHandle {
    /// Windows HWND.
    WindowsHwnd {
        /// Raw handle value captured as an integer for safe records.
        hwnd: u64,
    },
    /// macOS `NSView`.
    MacOsNsView {
        /// Raw handle value captured as an integer for safe records.
        ns_view: u64,
    },
    /// macOS `NSWindow`.
    MacOsNsWindow {
        /// Raw handle value captured as an integer for safe records.
        ns_window: u64,
    },
    /// Linux Wayland display and surface.
    LinuxWayland {
        /// Wayland display handle.
        display: u64,
        /// Wayland surface handle.
        surface: u64,
    },
    /// Linux X11 display and window.
    LinuxX11 {
        /// X11 display handle.
        display: u64,
        /// X11 window handle.
        window: u64,
    },
    /// Linux XCB connection and window.
    LinuxXcb {
        /// XCB connection handle.
        connection: u64,
        /// XCB window handle.
        window: u64,
    },
    /// Linux `XWayland` display and window.
    LinuxXWayland {
        /// `XWayland` display handle.
        display: u64,
        /// `XWayland` window handle.
        window: u64,
    },
}

impl HostPlatformHandle {
    /// Creates a Windows HWND handle record.
    #[must_use]
    pub const fn windows_hwnd(hwnd: u64) -> Self {
        Self::WindowsHwnd { hwnd }
    }

    /// Creates a macOS `NSView` handle record.
    #[must_use]
    pub const fn macos_ns_view(ns_view: u64) -> Self {
        Self::MacOsNsView { ns_view }
    }

    /// Creates a macOS `NSWindow` handle record.
    #[must_use]
    pub const fn macos_ns_window(ns_window: u64) -> Self {
        Self::MacOsNsWindow { ns_window }
    }

    /// Creates a Linux Wayland handle record.
    #[must_use]
    pub const fn linux_wayland(display: u64, surface: u64) -> Self {
        Self::LinuxWayland { display, surface }
    }

    /// Creates a Linux X11 handle record.
    #[must_use]
    pub const fn linux_x11(display: u64, window: u64) -> Self {
        Self::LinuxX11 { display, window }
    }

    /// Creates a Linux XCB handle record.
    #[must_use]
    pub const fn linux_xcb(connection: u64, window: u64) -> Self {
        Self::LinuxXcb { connection, window }
    }

    /// Creates a Linux `XWayland` handle record.
    #[must_use]
    pub const fn linux_xwayland(display: u64, window: u64) -> Self {
        Self::LinuxXWayland { display, window }
    }

    /// Returns the Linux window system when this is a Linux handle.
    #[must_use]
    pub const fn linux_window_system(&self) -> Option<LinuxWindowSystem> {
        match self {
            Self::LinuxWayland { .. } => Some(LinuxWindowSystem::Wayland),
            Self::LinuxX11 { .. } => Some(LinuxWindowSystem::X11),
            Self::LinuxXcb { .. } => Some(LinuxWindowSystem::Xcb),
            Self::LinuxXWayland { .. } => Some(LinuxWindowSystem::XWayland),
            Self::WindowsHwnd { .. } | Self::MacOsNsView { .. } | Self::MacOsNsWindow { .. } => {
                None
            }
        }
    }

    /// Validates that a platform handle can back the requested surface ownership model.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformHandleDiagnostic`] when the handle type is incompatible with the ownership model.
    pub fn validate_for(
        &self,
        ownership: SurfaceOwnership,
    ) -> Result<(), PlatformHandleDiagnostic> {
        match (self, ownership) {
            (Self::MacOsNsView { .. }, SurfaceOwnership::DesktopWindow) => {
                Err(PlatformHandleDiagnostic::ownership_mismatch(
                    "macOS NSView cannot own a desktop top-level window",
                ))
            }
            (Self::MacOsNsWindow { .. }, SurfaceOwnership::PluginEditor) => {
                Err(PlatformHandleDiagnostic::ownership_mismatch(
                    "macOS plugin editors must attach to NSView-compatible child surfaces",
                ))
            }
            _ => Ok(()),
        }
    }
}
