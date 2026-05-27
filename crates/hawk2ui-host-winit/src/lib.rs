#![forbid(unsafe_code)]
//! `Winit`-backed desktop host adapter for `Hawk2UI`.

mod runtime;
mod software_frame;

use hawk2ui_host::{
    ClipboardCapability, DesktopHostAdapter, DesktopHostEvent, DesktopWindowConfig,
    HostPlatformHandle, KeyboardInput, LinuxWindowSystem, PointerInput, RepaintRequest,
    SurfaceMetrics, SurfaceOwnership, WindowMode,
};
use winit::dpi::LogicalSize;

pub use runtime::{
    DesktopRuntimeEvent, WinitDesktopRuntime, WinitDesktopRuntimeConfig, WinitDesktopRuntimeSummary,
};
pub use software_frame::{SoftwareFrame, SoftwareFrameRenderer, physical_frame_size};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-host-winit";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

/// Winit platform fixture used by headless tests and adapter validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WinitPlatformFixture {
    handle: HostPlatformHandle,
}

impl WinitPlatformFixture {
    /// Creates a Linux fixture for the selected window system.
    #[must_use]
    pub const fn linux(window_system: LinuxWindowSystem) -> Self {
        let handle = match window_system {
            LinuxWindowSystem::Wayland => HostPlatformHandle::linux_wayland(1, 2),
            LinuxWindowSystem::X11 | LinuxWindowSystem::XWayland => {
                HostPlatformHandle::linux_x11(1, 3)
            }
            LinuxWindowSystem::Xcb => HostPlatformHandle::linux_xcb(4, 5),
        };
        Self { handle }
    }

    /// Creates a Windows HWND fixture.
    #[must_use]
    pub const fn windows() -> Self {
        Self {
            handle: HostPlatformHandle::windows_hwnd(6),
        }
    }

    /// Creates a macOS `NSWindow` fixture.
    #[must_use]
    pub const fn macos() -> Self {
        Self {
            handle: HostPlatformHandle::macos_ns_window(7),
        }
    }

    /// Returns the platform handle.
    #[must_use]
    pub const fn handle(&self) -> HostPlatformHandle {
        self.handle
    }
}

/// Winit host capability report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WinitHostCapabilities {
    flags: u16,
}

impl WinitHostCapabilities {
    /// Returns desktop Winit capabilities.
    #[must_use]
    pub const fn desktop() -> Self {
        Self {
            flags: WINIT_CAP_OWNS_WINDOW
                | WINIT_CAP_CLOSE
                | WINIT_CAP_MINIMIZE
                | WINIT_CAP_MAXIMIZE
                | WINIT_CAP_FULLSCREEN
                | WINIT_CAP_FOCUS
                | WINIT_CAP_KEYBOARD
                | WINIT_CAP_POINTER
                | WINIT_CAP_CLIPBOARD
                | WINIT_CAP_RESIZE
                | WINIT_CAP_REPAINT,
        }
    }

    /// Returns whether Winit owns the desktop window.
    #[must_use]
    pub const fn owns_window(&self) -> bool {
        self.supports(WinitCapability::OwnsWindow)
    }

    /// Returns whether a capability is supported.
    #[must_use]
    pub const fn supports(&self, capability: WinitCapability) -> bool {
        self.flags & capability.flag() != 0
    }
}

/// Single Winit desktop capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WinitCapability {
    /// Owns the desktop top-level window.
    OwnsWindow,
    /// Handles close requests.
    Close,
    /// Handles minimize requests.
    Minimize,
    /// Handles maximize requests.
    Maximize,
    /// Handles fullscreen requests.
    Fullscreen,
    /// Handles focus events.
    Focus,
    /// Handles keyboard input.
    Keyboard,
    /// Handles pointer input.
    Pointer,
    /// Handles clipboard availability.
    Clipboard,
    /// Handles resize events.
    Resize,
    /// Handles repaint requests.
    Repaint,
}

impl WinitCapability {
    const fn flag(self) -> u16 {
        match self {
            Self::OwnsWindow => WINIT_CAP_OWNS_WINDOW,
            Self::Close => WINIT_CAP_CLOSE,
            Self::Minimize => WINIT_CAP_MINIMIZE,
            Self::Maximize => WINIT_CAP_MAXIMIZE,
            Self::Fullscreen => WINIT_CAP_FULLSCREEN,
            Self::Focus => WINIT_CAP_FOCUS,
            Self::Keyboard => WINIT_CAP_KEYBOARD,
            Self::Pointer => WINIT_CAP_POINTER,
            Self::Clipboard => WINIT_CAP_CLIPBOARD,
            Self::Resize => WINIT_CAP_RESIZE,
            Self::Repaint => WINIT_CAP_REPAINT,
        }
    }
}

const WINIT_CAP_OWNS_WINDOW: u16 = 1 << 0;
const WINIT_CAP_CLOSE: u16 = 1 << 1;
const WINIT_CAP_MINIMIZE: u16 = 1 << 2;
const WINIT_CAP_MAXIMIZE: u16 = 1 << 3;
const WINIT_CAP_FULLSCREEN: u16 = 1 << 4;
const WINIT_CAP_FOCUS: u16 = 1 << 5;
const WINIT_CAP_KEYBOARD: u16 = 1 << 6;
const WINIT_CAP_POINTER: u16 = 1 << 7;
const WINIT_CAP_CLIPBOARD: u16 = 1 << 8;
const WINIT_CAP_RESIZE: u16 = 1 << 9;
const WINIT_CAP_REPAINT: u16 = 1 << 10;

/// Desktop Winit adapter error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WinitHostError {
    rule: String,
    message: String,
}

impl WinitHostError {
    /// Creates a Winit host error.
    #[must_use]
    pub fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            rule: rule.into(),
            message: message.into(),
        }
    }

    /// Returns the diagnostic rule.
    #[must_use]
    pub fn rule(&self) -> &str {
        &self.rule
    }

    /// Returns the diagnostic message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Headless-safe Winit desktop adapter.
#[derive(Clone, Debug, PartialEq)]
pub struct WinitDesktopAdapter {
    config: DesktopWindowConfig,
    platform: WinitPlatformFixture,
    capabilities: WinitHostCapabilities,
    logical_size: LogicalSize<f64>,
    mode: WindowMode,
    focused: bool,
    close_requested: bool,
    events: Vec<DesktopHostEvent>,
    repaint_requests: Vec<RepaintRequest>,
}

impl WinitDesktopAdapter {
    /// Creates a desktop window adapter from a platform fixture.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the platform handle cannot own a desktop window.
    pub fn create_window(
        config: DesktopWindowConfig,
        platform: WinitPlatformFixture,
    ) -> Result<Self, WinitHostError> {
        validate_desktop_metrics(config.metrics)?;
        platform
            .handle()
            .validate_for(SurfaceOwnership::DesktopWindow)
            .map_err(|diagnostic| WinitHostError::new(diagnostic.code, diagnostic.message))?;
        let logical_size =
            LogicalSize::new(config.metrics.logical_width, config.metrics.logical_height);
        Ok(Self {
            config: config.clone(),
            platform,
            capabilities: WinitHostCapabilities::desktop(),
            logical_size,
            mode: WindowMode::Normal,
            focused: false,
            close_requested: false,
            events: vec![DesktopHostEvent::WindowCreated(config)],
            repaint_requests: Vec::new(),
        })
    }

    /// Returns Winit-specific capabilities.
    #[must_use]
    pub const fn capabilities(&self) -> WinitHostCapabilities {
        self.capabilities
    }

    /// Returns current window mode.
    #[must_use]
    pub const fn mode(&self) -> WindowMode {
        self.mode
    }

    /// Returns whether the window is focused.
    #[must_use]
    pub const fn focused(&self) -> bool {
        self.focused
    }

    /// Returns whether close was requested.
    #[must_use]
    pub const fn close_requested(&self) -> bool {
        self.close_requested
    }

    /// Returns the platform handle.
    #[must_use]
    pub const fn platform_handle(&self) -> HostPlatformHandle {
        self.platform.handle()
    }

    /// Returns repaint requests.
    #[must_use]
    pub fn repaint_requests(&self) -> &[RepaintRequest] {
        &self.repaint_requests
    }

    /// Handles Winit resize events.
    pub fn handle_resize(&mut self, metrics: SurfaceMetrics) {
        let _ = self.try_handle_resize(metrics);
    }

    /// Handles Winit resize events and reports invalid metrics.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the resize metrics are not finite and positive.
    pub fn try_handle_resize(&mut self, metrics: SurfaceMetrics) -> Result<(), WinitHostError> {
        validate_desktop_metrics(metrics)?;
        self.config.metrics = metrics;
        self.logical_size = LogicalSize::new(metrics.logical_width, metrics.logical_height);
        self.events
            .push(DesktopHostEvent::RendererTargetRecreateRequested);
        Ok(())
    }

    /// Handles Winit DPI changes and reports invalid scale factors.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the resulting surface metrics are not finite and positive.
    pub fn try_dpi_changed(&mut self, scale_factor: f64) -> Result<(), WinitHostError> {
        let metrics = SurfaceMetrics::new(
            self.config.metrics.logical_width,
            self.config.metrics.logical_height,
            scale_factor,
        );
        validate_desktop_metrics(metrics)?;
        self.config.metrics.scale_factor = scale_factor;
        self.events.push(DesktopHostEvent::DpiChanged(scale_factor));
        self.events
            .push(DesktopHostEvent::RendererTargetRecreateRequested);
        Ok(())
    }

    /// Requests a repaint.
    pub fn request_repaint(&mut self, reason: impl Into<String>) {
        self.repaint_requests
            .push(RepaintRequest::full_surface(reason));
    }

    /// Drains host events.
    pub fn drain_events(&mut self) -> Vec<DesktopHostEvent> {
        std::mem::take(&mut self.events)
    }

    fn set_mode(&mut self, mode: WindowMode) {
        self.mode = mode;
        self.events.push(DesktopHostEvent::ModeChanged(mode));
    }
}

impl DesktopHostAdapter for WinitDesktopAdapter {
    fn config(&self) -> &DesktopWindowConfig {
        &self.config
    }

    fn metrics(&self) -> SurfaceMetrics {
        self.config.metrics
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
        self.close_requested = true;
        self.events
            .push(DesktopHostEvent::CloseRequested(reason.into()));
    }

    fn set_focus(&mut self, focused: bool) {
        self.focused = focused;
        self.events.push(DesktopHostEvent::FocusChanged(focused));
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
        let _ = self.try_dpi_changed(scale_factor);
    }
}

fn validate_desktop_metrics(metrics: SurfaceMetrics) -> Result<(), WinitHostError> {
    if metrics.logical_width.is_finite()
        && metrics.logical_height.is_finite()
        && metrics.scale_factor.is_finite()
        && metrics.logical_width > 0.0
        && metrics.logical_height > 0.0
        && metrics.scale_factor > 0.0
    {
        Ok(())
    } else {
        Err(WinitHostError::new(
            "desktop.window.invalid-size",
            "desktop window dimensions and scale factor must be finite and greater than zero",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-host-winit");
    }
}
