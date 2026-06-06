//! CLI command surface definitions.

use serde::{Deserialize, Serialize};

use crate::CliDiagnostic;

/// CLI command.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CliCommand {
    /// Create a new project.
    NewProject,
    /// Build and run the default target.
    Run,
    /// Run a development loop with native reload.
    Dev,
    /// Validate project manifests and sources.
    Validate,
    /// Build a development artifact.
    BuildDev,
    /// Build a production artifact.
    BuildRelease,
    /// Verify a sealed artifact container.
    VerifyArtifact {
        /// Optional artifact container path. Defaults to the release build output.
        path: Option<String>,
    },
    /// Run a desktop app.
    RunDesktop {
        /// Native presentation backend requested for the desktop runtime.
        presentation_backend: CliPresentationBackend,
    },
    /// Package plugin targets.
    PackagePlugin,
    /// Export the central generated JSON Schema catalog.
    ExportSchemas,
    /// Emit the generated truce parameter source for the project's manifest.
    ExportParams,
    /// Pin a stable numeric id to every unpinned manifest parameter.
    PinIds,
    /// Render diagnostics.
    Diagnostics,
    /// Explain the current project and available workflows.
    Explain,
}

/// Native desktop presentation backend requested by the CLI.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum CliPresentationBackend {
    /// Use Skia CPU raster rendering copied into a native software surface.
    #[default]
    Software,
    /// Prefer Skia GPU presentation and fall back to software when unavailable.
    GpuPreferred,
    /// Require Skia GPU presentation and fail startup when unavailable.
    GpuRequired,
}

impl CliPresentationBackend {
    /// Parses a presentation backend name accepted by `run-desktop`.
    #[must_use]
    pub fn parse_name(name: &str) -> Option<Self> {
        match name {
            "software" => Some(Self::Software),
            "gpu-preferred" => Some(Self::GpuPreferred),
            "gpu-required" => Some(Self::GpuRequired),
            _ => None,
        }
    }

    /// Returns the stable CLI label for the backend.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Software => "software",
            Self::GpuPreferred => "gpu-preferred",
            Self::GpuRequired => "gpu-required",
        }
    }
}

impl CliCommand {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "new" => Some(Self::NewProject),
            "run" => Some(Self::Run),
            "dev" => Some(Self::Dev),
            "validate" => Some(Self::Validate),
            "build-dev" => Some(Self::BuildDev),
            "build-release" => Some(Self::BuildRelease),
            "verify-artifact" => Some(Self::VerifyArtifact { path: None }),
            "run-desktop" => Some(Self::RunDesktop {
                presentation_backend: CliPresentationBackend::Software,
            }),
            "package-plugin" => Some(Self::PackagePlugin),
            "export-schemas" => Some(Self::ExportSchemas),
            "export-params" => Some(Self::ExportParams),
            "pin-ids" => Some(Self::PinIds),
            "diagnostics" => Some(Self::Diagnostics),
            "explain" => Some(Self::Explain),
            _ => None,
        }
    }
}

/// CLI process exit code.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CliExitCode {
    /// Successful execution.
    Success = 0,
    /// Usage or command-line parsing failure.
    Usage = 2,
    /// Validation failure.
    Validation = 10,
    /// Artifact verification failure.
    Verification = 11,
    /// Runtime failure.
    Runtime = 12,
}

/// CLI error.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CliError {
    /// Exit code.
    pub exit_code: CliExitCode,
    /// Human-readable error message.
    pub message: String,
}

/// CLI command catalog.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandCatalog;

impl CommandCatalog {
    /// Parses a command from argv-like input.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] for missing or unknown commands.
    pub fn parse(
        &self,
        args: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<CliCommand, CliError> {
        let mut args = args.into_iter();
        let _program = args.next();
        let Some(command_name) = args.next() else {
            return Err(CliError {
                exit_code: CliExitCode::Usage,
                message: "missing command".into(),
            });
        };
        let mut command = CliCommand::from_name(command_name.as_ref()).ok_or_else(|| CliError {
            exit_code: CliExitCode::Usage,
            message: format!("unknown command: {}", command_name.as_ref()),
        })?;
        match &mut command {
            CliCommand::VerifyArtifact { path } => {
                if let Some(value) = args.next() {
                    *path = Some(value.as_ref().to_string());
                }
                if let Some(extra) = args.next() {
                    return Err(unexpected_argument(extra.as_ref()));
                }
            }
            CliCommand::RunDesktop {
                presentation_backend,
            } => {
                while let Some(argument) = args.next() {
                    match argument.as_ref() {
                        "--presentation-backend" => {
                            let Some(value) = args.next() else {
                                return Err(CliError {
                                    exit_code: CliExitCode::Usage,
                                    message: "--presentation-backend requires a value".into(),
                                });
                            };
                            *presentation_backend = CliPresentationBackend::parse_name(
                                value.as_ref(),
                            )
                            .ok_or_else(|| CliError {
                                exit_code: CliExitCode::Usage,
                                message: format!(
                                    "unknown presentation backend: {}",
                                    value.as_ref()
                                ),
                            })?;
                        }
                        "--software" => {
                            *presentation_backend = CliPresentationBackend::Software;
                        }
                        "--gpu-preferred" => {
                            *presentation_backend = CliPresentationBackend::GpuPreferred;
                        }
                        "--gpu-required" => {
                            *presentation_backend = CliPresentationBackend::GpuRequired;
                        }
                        other => return Err(unexpected_argument(other)),
                    }
                }
            }
            _ => {
                if let Some(extra) = args.next() {
                    return Err(unexpected_argument(extra.as_ref()));
                }
            }
        }
        Ok(command)
    }

    /// Renders top-level help text.
    #[must_use]
    pub fn render_help(&self) -> String {
        [
            "Hawk2UI CLI",
            "",
            "Commands:",
            "  new              Create a new Hawk2UI project",
            "  run              Build and run the default native target",
            "  dev              Watch, rebuild, validate, and hot-reload the native surface",
            "  validate         Validate manifests, sources, and capabilities",
            "  build-dev        Build and write a development sealed artifact",
            "  build-release    Build and write a production sealed artifact",
            "  verify-artifact  Verify a sealed artifact container",
            "  run-desktop      Run a desktop native surface [--presentation-backend software|gpu-preferred|gpu-required]",
            "  package-plugin   Materialize CLAP, VST3, AU, and standalone package layouts",
            "  export-schemas   Export the central generated JSON Schema catalog",
            "  export-params    Emit truce parameter source generated from the manifest",
            "  pin-ids          Pin a stable numeric id to every unpinned manifest parameter",
            "  diagnostics      Render structured diagnostics",
            "  explain          Explain project targets, capabilities, and next commands",
        ]
        .join("\n")
    }
}

fn unexpected_argument(argument: &str) -> CliError {
    CliError {
        exit_code: CliExitCode::Usage,
        message: format!("unexpected argument: {argument}"),
    }
}

/// Deterministic scenario used by command tests and recording runners.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum BuildCommandScenario {
    /// Command succeeds.
    Success,
    /// Validation fails.
    ValidationFailure,
    /// Artifact verification fails.
    VerificationFailure,
}

/// Result of a build-family CLI command.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BuildCommandResult {
    /// Exit code.
    pub exit_code: CliExitCode,
    /// Structured diagnostics.
    pub diagnostics: Vec<CliDiagnostic>,
}

/// Recording build command runner.
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

fn success() -> BuildCommandResult {
    BuildCommandResult {
        exit_code: CliExitCode::Success,
        diagnostics: Vec::new(),
    }
}
