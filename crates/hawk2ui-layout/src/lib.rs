#![forbid(unsafe_code)]
//! Layout tree, text measurement bridge, plugin constraints, and scene geometry attachment for `Hawk2UI`.

pub mod compute;
pub mod plugin;
pub mod scene;
pub mod text;
pub mod tree;

pub use compute::{ComputedGeometry, LayoutOutput, Viewport};
pub use plugin::{
    AnalyzerRegion, GeneratedParameterLayout, GraphRegion, PluginEditorConstraints,
    PluginEditorSize, PluginLayoutError,
};
pub use scene::{SceneGeometry, SceneGeometryAttachment};
pub use text::{
    HawkTextMeasurer, LayoutTextMeasurer, TestTextMeasurer, TextMeasureError, TextMeasureInput,
    TextMeasureKey, TextMeasureMode, TextMeasureResult,
};
pub use tree::{
    BoxEdges, FlexDirection, LayoutAlignItems, LayoutJustifyContent, LayoutNode, LayoutNodeId,
    LayoutSizing, LayoutStyle, LayoutTree, LayoutTreeError, LayoutValue,
};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-layout";

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
        assert_eq!(crate_name(), "hawk2ui-layout");
    }
}
