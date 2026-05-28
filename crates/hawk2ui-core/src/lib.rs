#![forbid(unsafe_code)]
//! Curated public facade for `Hawk2UI` product schema records and stable API contracts.
//!
//! Executable desktop/plugin runtime entry points live in the host and runtime crates. This crate
//! intentionally exposes the cross-crate data contracts application authors need without requiring
//! direct dependencies on every lower-level contract crate.

pub use hawk2ui_api::{
    ApiInventory, ApiModule, ApiTypeAudience, ApiTypeEntry, ApiTypeStatus, ArtifactCapability,
    ArtifactHash, ArtifactId, ArtifactManifestSnapshot, ArtifactSchemaVersion,
    ArtifactVersionError, AutomationGesture, BindingDirection, CapabilityKey, CompiledAssetKind,
    CompiledAssetRecord, CompiledScriptRecord, CompiledStyleRecord, Diagnostic, DiagnosticSeverity,
    FrameSchedule, HostBindingContract, HostSurfaceContract, InputEvent, KeyEvent, KeyModifiers,
    MouseButton, ParameterId, PluginEditorContract, PluginEditorKind, PluginParameterContract,
    PluginPresetContract, PluginStateContract, PluginStateEntry, PluginStateFormat,
    RealtimeDataContract, RealtimeDataDirection, RealtimeDataKind, RelatedContext, RepaintReason,
    RepaintRequest, RuleId, RuntimeJob, RuntimeJobId, RuntimeJobKind, RuntimeJobStatus,
    RuntimeLifecycleHook, RuntimePhase, SourceSpan, SuggestedFix, SurfaceKind as ApiSurfaceKind,
    SurfaceMetrics, TargetKind, TargetMetadata,
};
pub use hawk2ui_schema::{
    HostTarget, ProductCapability, ProductModel, ProductModelError, SCHEMA_CATALOG_VERSION,
    SchemaCatalog, SchemaCatalogEntry, SchemaValidationError, SurfaceKind,
    product_model_json_schema, schema_catalog, schema_catalog_json, validate_product_model_json,
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
            .with_host_target(HostTarget::PluginHost)
            .with_surface_kind(SurfaceKind::DesktopWindow)
            .with_surface_kind(SurfaceKind::PluginEditor)
            .with_capability(ProductCapability::PluginEditorEmbedding);

        assert!(model.supports_surface(SurfaceKind::DesktopWindow));
        assert!(model.supports_surface(SurfaceKind::PluginEditor));
        assert!(model.host_targets.contains(&HostTarget::PluginHost));
        assert!(model.has_capability(ProductCapability::PluginEditorEmbedding));
    }

    #[test]
    fn diagnostic_contract_is_available_from_core_facade() {
        let diagnostic = Diagnostic::error("runtime.failed", "runtime failed");

        assert_eq!(diagnostic.rule.as_str(), "runtime.failed");
    }

    #[test]
    fn schema_operations_are_available_from_core_facade() {
        let schema = product_model_json_schema().expect("product schema generates");
        assert_eq!(schema["title"], "ProductModel");

        let catalog = schema_catalog().expect("schema catalog generates");
        assert_eq!(catalog.schema_version, SCHEMA_CATALOG_VERSION);
        assert!(
            catalog
                .schemas
                .iter()
                .any(|entry| entry.id == "hawk2ui.product-model")
        );

        let json = schema_catalog_json().expect("schema catalog serializes");
        assert_eq!(json["schema_version"], SCHEMA_CATALOG_VERSION);
    }

    #[test]
    fn api_contract_records_are_available_from_core_facade() {
        let artifact = ArtifactManifestSnapshot::new(
            ArtifactId::new("com.hawk2ui.facade"),
            ArtifactSchemaVersion::new(1, 0, 0),
            ArtifactHash::new("sha256:manifest"),
        )
        .with_capability(ArtifactCapability::new("native-windowing"))
        .with_asset(CompiledAssetRecord::vector(
            "logo",
            ArtifactHash::new("sha256:logo"),
        ))
        .with_target(TargetMetadata::desktop("linux-wayland"));
        assert!(artifact.has_capability("native-windowing"));
        assert_eq!(artifact.targets()[0].name(), "linux-wayland");

        let binding = HostBindingContract::new(
            "clipboard.write_text",
            CapabilityKey::new("clipboard"),
            RuntimePhase::Running,
        )
        .with_direction(BindingDirection::RuntimeToHost);
        assert_eq!(binding.required_capability.as_str(), "clipboard");

        let job = RuntimeJob::new(
            RuntimeJobId::new("render-root"),
            RuntimeJobKind::RenderFrame,
            RuntimePhase::Running,
        )
        .with_status(RuntimeJobStatus::Completed);
        assert_eq!(job.status, RuntimeJobStatus::Completed);

        let parameter = PluginParameterContract::new(ParameterId::new("gain"), "Gain", 0.5, true);
        assert!(parameter.accepts_normalized(0.5));

        let surface = HostSurfaceContract::new(
            ApiSurfaceKind::Desktop,
            SurfaceMetrics::new(800.0, 600.0, 1600, 1200, 2.0),
            true,
        );
        assert_eq!(surface.kind, ApiSurfaceKind::Desktop);

        let inventory = ApiInventory::production_baseline();
        assert!(
            inventory
                .types()
                .iter()
                .any(|entry| entry.name() == "RuntimeJob")
        );
    }
}
