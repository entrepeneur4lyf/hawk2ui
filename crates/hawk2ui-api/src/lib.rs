#![forbid(unsafe_code)]
//! Public API contracts shared across `Hawk2UI` crates, generated artifacts, diagnostics, and tooling.

pub mod artifact;
pub mod diagnostic;
pub mod inventory;
pub mod plugin;
pub mod runtime;
pub mod surface;

pub use artifact::{ArtifactHash, ArtifactId, ArtifactSchemaVersion, ArtifactVersionError};
pub use diagnostic::{
    Diagnostic, DiagnosticSeverity, RelatedContext, RuleId, SourceSpan, SuggestedFix,
};
pub use inventory::{ApiInventory, ApiModule, ApiTypeAudience, ApiTypeEntry, ApiTypeStatus};
pub use plugin::{AutomationGesture, ParameterId, PluginEditorContract, PluginParameterContract};
pub use runtime::{CapabilityKey, HostBindingContract, RuntimePhase};
pub use surface::{HostSurfaceContract, SurfaceKind, SurfaceMetrics};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_api_exports_diagnostic_contract() {
        let diagnostic = Diagnostic::error("manifest.invalid", "manifest is missing app identity")
            .with_source(SourceSpan::new("hawk.toml", 1, 1, 1, 8))
            .with_fix(SuggestedFix::new("Add [app] identity metadata."));

        assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
        assert_eq!(diagnostic.rule.as_str(), "manifest.invalid");
        assert_eq!(
            diagnostic.source.as_ref().expect("source span").path,
            "hawk.toml"
        );
        assert_eq!(diagnostic.fixes[0].message, "Add [app] identity metadata.");
    }

    #[test]
    fn artifact_schema_versions_reject_incompatible_major_versions() {
        let runtime = ArtifactSchemaVersion::new(1, 2, 0);
        let artifact = ArtifactSchemaVersion::new(2, 0, 0);

        let error = runtime
            .ensure_can_read(artifact)
            .expect_err("major mismatch must fail");
        assert_eq!(error.runtime, runtime);
        assert_eq!(error.artifact, artifact);
    }

    #[test]
    fn host_binding_contract_tracks_capability_and_lifecycle() {
        let binding = HostBindingContract::new(
            "clipboard.write_text",
            CapabilityKey::new("clipboard"),
            RuntimePhase::Running,
        );

        assert_eq!(binding.name, "clipboard.write_text");
        assert_eq!(binding.required_capability.as_str(), "clipboard");
        assert_eq!(binding.available_from, RuntimePhase::Running);
    }

    #[test]
    fn host_surface_contract_tracks_kind_metrics_and_focus() {
        let metrics = SurfaceMetrics::new(800.0, 600.0, 1600, 1200, 2.0);
        let surface = HostSurfaceContract::new(SurfaceKind::Desktop, metrics, true);

        assert_eq!(surface.kind, SurfaceKind::Desktop);
        assert_eq!(surface.metrics.scale_factor, 2.0);
        assert!(surface.focused);
    }

    #[test]
    fn plugin_contracts_track_parameters_editors_and_automation() {
        let parameter = PluginParameterContract::new(ParameterId::new("gain"), "Gain", 0.5, true);
        let editor = PluginEditorContract::new(900, 560, 640, 360);
        let gesture = AutomationGesture::Change {
            parameter: ParameterId::new("gain"),
            normalized: 0.75,
        };

        assert_eq!(parameter.id.as_str(), "gain");
        assert_eq!(parameter.default_normalized, 0.5);
        assert!(parameter.automatable);
        assert_eq!(editor.default_width, 900);
        assert_eq!(editor.min_height, 360);
        assert_eq!(
            gesture,
            AutomationGesture::Change {
                parameter: ParameterId::new("gain"),
                normalized: 0.75,
            }
        );
    }
}
