#![forbid(unsafe_code)]
//! Core public facade for `Hawk2UI` product records and runtime entry points.

pub use hawk2ui_api::{
    Diagnostic, DiagnosticSeverity, RelatedContext, RuleId, SourceSpan, SuggestedFix,
};
pub use hawk2ui_schema::{
    HostTarget, ProductCapability, ProductModel, ProductModelError, SurfaceKind,
};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-core";

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
        assert_eq!(crate_name(), "hawk2ui-core");
    }

    #[test]
    fn product_model_is_available_from_core_facade() {
        let model = ProductModel::new("facade")
            .with_surface_kind(SurfaceKind::DesktopWindow)
            .with_surface_kind(SurfaceKind::PluginEditor);

        assert!(model.supports_surface(SurfaceKind::DesktopWindow));
        assert_eq!(HostTarget::PluginHost, HostTarget::PluginHost);
        assert_eq!(
            ProductCapability::PluginEditorEmbedding,
            ProductCapability::PluginEditorEmbedding
        );
    }

    #[test]
    fn diagnostic_contract_is_available_from_core_facade() {
        let diagnostic = Diagnostic::error("runtime.failed", "runtime failed");

        assert_eq!(diagnostic.rule.as_str(), "runtime.failed");
    }
}
