#![forbid(unsafe_code)]
//! Retained scene records, paint layers, renderer backend boundary, text contracts, and custom draw surfaces for `Hawk2UI`.

pub mod assets;
pub mod backend;
pub mod custom_surface;
pub mod export;
pub mod layer;
pub mod scene;
pub mod text;

pub use backend::{
    BackendCacheHandle, BackendCapabilities, BackendDiagnostic, BackendError, RecordingBackend,
    RendererBackend,
};
pub use custom_surface::{CustomDrawSurface, CustomSurfaceCapability, CustomSurfaceCategory};
pub use export::{PaintCommand, PaintCommandList, export_paint_commands};
pub use layer::{
    Color, GlowLayer, GradientLayer, LayerKind, LayerStack, LayerValidationError, PaintLayer,
    PathLayer, RoundedRect, ShadowLayer, Stroke, TextLayer,
};
pub use scene::{
    AccessibilityRef, Geometry, HitTestGeometry, InvalidationReason, SceneGraph, SceneGraphError,
    SceneNode, SceneNodeId, Transform,
};
pub use text::{
    DeterministicTextMeasurer, FontRegistry, GlyphCacheKey, LineBreakMode, TextMeasureOutput,
    TextRenderInput, TextRenderTextError,
};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-render";

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
        assert_eq!(crate_name(), "hawk2ui-render");
    }
}
pub use assets::{
    AssetDiagnostic, AssetDrawRecord, AssetError, AssetKind, BackendRequirement, CompiledAsset,
};
