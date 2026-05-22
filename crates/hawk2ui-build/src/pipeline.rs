//! Build pipeline phase records.

use crate::{BuildDiagnostic, BuildDiagnosticSeverity};

/// Build pipeline phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuildPhase {
    /// Source discovery.
    SourceDiscovery,
    /// Manifest validation.
    ManifestValidation,
    /// Asset discovery.
    AssetDiscovery,
    /// Source validation.
    SourceValidation,
    /// Style compilation.
    StyleCompilation,
    /// Script compilation.
    ScriptCompilation,
    /// Asset compilation.
    AssetCompilation,
    /// Artifact generation.
    ArtifactGeneration,
    /// Package output.
    Packaging,
    /// Verification report generation.
    Verification,
}

impl BuildPhase {
    fn as_str(self) -> &'static str {
        match self {
            Self::SourceDiscovery => "source-discovery",
            Self::ManifestValidation => "manifest-validation",
            Self::AssetDiscovery => "asset-discovery",
            Self::SourceValidation => "source-validation",
            Self::StyleCompilation => "style-compilation",
            Self::ScriptCompilation => "script-compilation",
            Self::AssetCompilation => "asset-compilation",
            Self::ArtifactGeneration => "artifact-generation",
            Self::Packaging => "packaging",
            Self::Verification => "verification",
        }
    }
}

/// One build phase record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildPhaseRecord {
    /// Build phase.
    pub phase: BuildPhase,
    /// Diagnostics emitted by the phase.
    pub diagnostics: Vec<BuildDiagnostic>,
}

/// Production build pipeline record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildPipeline {
    /// Ordered phase records.
    pub phases: Vec<BuildPhaseRecord>,
}

impl BuildPipeline {
    /// Creates the production build pipeline phase order.
    #[must_use]
    pub fn production() -> Self {
        Self {
            phases: [
                BuildPhase::SourceDiscovery,
                BuildPhase::ManifestValidation,
                BuildPhase::AssetDiscovery,
                BuildPhase::SourceValidation,
                BuildPhase::StyleCompilation,
                BuildPhase::ScriptCompilation,
                BuildPhase::AssetCompilation,
                BuildPhase::ArtifactGeneration,
                BuildPhase::Packaging,
                BuildPhase::Verification,
            ]
            .into_iter()
            .map(|phase| BuildPhaseRecord {
                phase,
                diagnostics: Vec::new(),
            })
            .collect(),
        }
    }

    /// Returns phase names in execution order.
    #[must_use]
    pub fn phase_names(&self) -> Vec<&'static str> {
        self.phases
            .iter()
            .map(|record| record.phase.as_str())
            .collect()
    }

    /// Adds a diagnostic to a phase.
    #[must_use]
    pub fn with_diagnostic(mut self, phase: BuildPhase, diagnostic: BuildDiagnostic) -> Self {
        if let Some(record) = self.phases.iter_mut().find(|record| record.phase == phase) {
            record.diagnostics.push(diagnostic);
        }
        self
    }

    /// Ensures no phase emitted release-blocking diagnostics.
    ///
    /// # Errors
    ///
    /// Returns [`BuildPipelineError`] when an error diagnostic blocks release.
    pub fn ensure_release_ready(&self) -> Result<(), BuildPipelineError> {
        for diagnostic in self.phases.iter().flat_map(|record| &record.diagnostics) {
            if diagnostic.severity == BuildDiagnosticSeverity::Error {
                return Err(BuildPipelineError::ReleaseBlocked(diagnostic.rule.clone()));
            }
        }
        Ok(())
    }
}

/// Build pipeline validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BuildPipelineError {
    /// Release is blocked by a diagnostic rule.
    ReleaseBlocked(String),
}
