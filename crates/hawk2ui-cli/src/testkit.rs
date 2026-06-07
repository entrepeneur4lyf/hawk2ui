//! Test doubles for CLI command and dev-loop conformance.

use serde::{Deserialize, Serialize};

use crate::{CliDiagnostic, CliExitCode, DevChangeBatch, DevSurfaceReloader};

/// Deterministic scenario used by command tests.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum BuildCommandScenario {
    /// Command succeeds.
    Success,
    /// Validation fails.
    ValidationFailure,
    /// Artifact verification fails.
    VerificationFailure,
}

/// Result of a build-family CLI command test double.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BuildCommandResult {
    /// Exit code.
    pub exit_code: CliExitCode,
    /// Structured diagnostics.
    pub diagnostics: Vec<CliDiagnostic>,
}

/// Deterministic build command runner for command tests.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct BuildCommandRunner;

impl BuildCommandRunner {
    /// Runs validation.
    #[must_use]
    pub fn validate(&self, scenario: BuildCommandScenario) -> BuildCommandResult {
        match scenario {
            BuildCommandScenario::Success | BuildCommandScenario::VerificationFailure => success(),
            BuildCommandScenario::ValidationFailure => BuildCommandResult {
                exit_code: CliExitCode::Validation,
                diagnostics: vec![CliDiagnostic::error(
                    "manifest.invalid",
                    "project manifest validation failed",
                )],
            },
        }
    }

    /// Runs development build.
    #[must_use]
    pub fn build_dev(&self, scenario: BuildCommandScenario) -> BuildCommandResult {
        self.validate(scenario)
    }

    /// Runs release build.
    #[must_use]
    pub fn build_release(&self, scenario: BuildCommandScenario) -> BuildCommandResult {
        self.validate(scenario)
    }

    /// Runs artifact verification.
    #[must_use]
    pub fn verify_artifact(&self, scenario: BuildCommandScenario) -> BuildCommandResult {
        match scenario {
            BuildCommandScenario::Success | BuildCommandScenario::ValidationFailure => success(),
            BuildCommandScenario::VerificationFailure => BuildCommandResult {
                exit_code: CliExitCode::Verification,
                diagnostics: vec![CliDiagnostic::error(
                    "artifact.verification-failed",
                    "sealed artifact verification failed",
                )],
            },
        }
    }
}

/// Deterministic changed-file source for dev-loop tests.
pub type RecordingWatcher = DevChangeBatch;

/// Recording runtime reload target for dev-loop tests.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RecordingReloadTarget {
    reload_count: usize,
}

impl RecordingReloadTarget {
    /// Returns the number of accepted reloads.
    #[must_use]
    pub const fn reload_count(&self) -> usize {
        self.reload_count
    }
}

impl DevSurfaceReloader for RecordingReloadTarget {
    fn reload(&mut self, _preserve_state: bool) -> Result<(), String> {
        self.reload_count += 1;
        Ok(())
    }
}

fn success() -> BuildCommandResult {
    BuildCommandResult {
        exit_code: CliExitCode::Success,
        diagnostics: Vec::new(),
    }
}
