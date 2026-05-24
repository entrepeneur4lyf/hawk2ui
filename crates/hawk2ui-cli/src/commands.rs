//! CLI command surface definitions.

use serde::{Deserialize, Serialize};

/// CLI command.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CliCommand {
    /// Create a new project.
    NewProject,
    /// Validate project manifests and sources.
    Validate,
    /// Build a development artifact.
    BuildDev,
    /// Build a production artifact.
    BuildRelease,
    /// Verify a sealed artifact.
    VerifyArtifact,
    /// Run a desktop app.
    RunDesktop,
    /// Package plugin targets.
    PackagePlugin,
    /// Render diagnostics.
    Diagnostics,
}

impl CliCommand {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "new" => Some(Self::NewProject),
            "validate" => Some(Self::Validate),
            "build-dev" => Some(Self::BuildDev),
            "build-release" => Some(Self::BuildRelease),
            "verify-artifact" => Some(Self::VerifyArtifact),
            "run-desktop" => Some(Self::RunDesktop),
            "package-plugin" => Some(Self::PackagePlugin),
            "diagnostics" => Some(Self::Diagnostics),
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
        CliCommand::from_name(command_name.as_ref()).ok_or_else(|| CliError {
            exit_code: CliExitCode::Usage,
            message: format!("unknown command: {}", command_name.as_ref()),
        })
    }

    /// Renders top-level help text.
    #[must_use]
    pub fn render_help(&self) -> String {
        [
            "Hawk2UI CLI",
            "",
            "Commands:",
            "  new              Create a new Hawk2UI project",
            "  validate         Validate manifests, sources, and capabilities",
            "  build-dev        Build a development artifact",
            "  build-release    Build a production artifact",
            "  verify-artifact  Verify a sealed artifact",
            "  run-desktop      Run a desktop native surface",
            "  package-plugin   Package CLAP, VST3, AU, and standalone targets",
            "  diagnostics      Render structured diagnostics",
        ]
        .join("\n")
    }
}
