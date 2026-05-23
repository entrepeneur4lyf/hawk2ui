#![forbid(unsafe_code)]
//! Common host surface contracts for `Hawk2UI` desktop windows and embedded plugin surfaces.

pub mod desktop;
pub mod plugin;
pub mod surface;

pub use desktop::{
    ClipboardCapability, DesktopHostAdapter, DesktopHostEvent, DesktopWindowConfig, KeyboardInput,
    PointerInput, RecordingDesktopAdapter, WindowMode,
};
pub use plugin::{
    PluginEditorConfig, PluginHostAdapter, PluginHostEvent, PluginParentHandle,
    RecordingPluginAdapter,
};
pub use surface::{
    FramePresenter, HostCapabilities, HostSurface, PresentedFrame, RecordingFramePresenter,
    RecordingHostSurface, RepaintRequest, SurfaceEvent, SurfaceMetrics,
};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-host";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-host");
    }
}
