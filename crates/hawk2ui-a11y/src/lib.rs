#![forbid(unsafe_code)]
//! Accessibility tree, semantics, host export, and plugin-safe accessibility support for `Hawk2UI`.

pub mod actions;
pub mod component;
pub mod host;
pub mod plugin;
pub mod tree;

pub use actions::{A11yActionDispatchError, A11yActionDispatcher, A11yActionEvent};
pub use component::{ComponentKind, ComponentSemantics, VisualStyleSemantics};
pub use host::{
    A11yHostExportSnapshot, A11yHostExporter, A11yHostSurfaceKind, LayoutGeometryUpdate,
};
pub use plugin::{A11yPluginDenial, A11yPluginGuard, A11yPluginOperation, A11yThreadContext};
pub use tree::{A11yAction, A11yBounds, A11yNode, A11yRole, A11yTree, CheckedState};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-a11y";

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
        assert_eq!(crate_name(), "hawk2ui-a11y");
    }

    #[test]
    fn a11y_workspace_filter_marker() {
        assert_eq!(crate_name(), "hawk2ui-a11y");
    }
}
