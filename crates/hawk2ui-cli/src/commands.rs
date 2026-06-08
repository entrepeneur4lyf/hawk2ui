//! CLI command surface definitions.

use serde::{Deserialize, Serialize};

/// CLI command.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CliCommand {
    /// Create a new project.
    NewProject {
        /// Project scaffold template.
        template: CliProjectTemplate,
        /// JavaScript package manager metadata to write into generated framework projects.
        package_manager: CliPackageManager,
    },
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
    /// Package desktop targets.
    PackageDesktop,
    /// Package plugin targets.
    PackagePlugin,
    /// Export the central generated JSON Schema catalog.
    ExportSchemas,
    /// Emit the generated truce parameter source for the project's manifest.
    ExportParams,
    /// Pin a stable numeric id to every unpinned manifest parameter.
    PinIds,
    /// Convert legacy `manifest.hawk.toml` into canonical `hawk.json`.
    MigrateManifest {
        /// Overwrite an existing canonical manifest.
        force: bool,
    },
    /// Render diagnostics.
    Diagnostics,
    /// Explain the current project and available workflows.
    Explain,
}

/// Project scaffold template requested by `init`/`new`.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum CliProjectTemplate {
    /// React desktop app template.
    #[default]
    ReactApp,
    /// React plugin editor template.
    ReactPlugin,
    /// Vue desktop app template.
    VueApp,
    /// Vue plugin editor template.
    VuePlugin,
    /// Legacy native desktop+plugin scaffold.
    Native,
}

impl CliProjectTemplate {
    /// Parses a scaffold template name.
    #[must_use]
    pub fn parse_name(name: &str) -> Option<Self> {
        match name {
            "react-app" => Some(Self::ReactApp),
            "react-plugin" => Some(Self::ReactPlugin),
            "vue-app" => Some(Self::VueApp),
            "vue-plugin" => Some(Self::VuePlugin),
            "native" => Some(Self::Native),
            _ => None,
        }
    }

    /// Returns the stable CLI label for the template.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ReactApp => "react-app",
            Self::ReactPlugin => "react-plugin",
            Self::VueApp => "vue-app",
            Self::VuePlugin => "vue-plugin",
            Self::Native => "native",
        }
    }
}

/// JavaScript package manager selected for generated framework projects.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum CliPackageManager {
    /// Bun package manager.
    #[default]
    Bun,
    /// npm package manager.
    Npm,
    /// pnpm package manager.
    Pnpm,
    /// Yarn package manager.
    Yarn,
}

impl CliPackageManager {
    /// Parses a package-manager name.
    #[must_use]
    pub fn parse_name(name: &str) -> Option<Self> {
        match name {
            "bun" => Some(Self::Bun),
            "npm" => Some(Self::Npm),
            "pnpm" => Some(Self::Pnpm),
            "yarn" => Some(Self::Yarn),
            _ => None,
        }
    }

    /// Returns the package manager string written to generated package manifests.
    #[must_use]
    pub const fn package_manager_field(self) -> &'static str {
        match self {
            Self::Bun => "bun@1.0.0",
            Self::Npm => "npm@10.0.0",
            Self::Pnpm => "pnpm@9.0.0",
            Self::Yarn => "yarn@4.0.0",
        }
    }
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
            "init" | "new" => Some(Self::NewProject {
                template: CliProjectTemplate::default(),
                package_manager: CliPackageManager::default(),
            }),
            "run" => Some(Self::Run),
            "dev" => Some(Self::Dev),
            "validate" => Some(Self::Validate),
            "build-dev" => Some(Self::BuildDev),
            "build-release" => Some(Self::BuildRelease),
            "verify-artifact" => Some(Self::VerifyArtifact { path: None }),
            "run-desktop" => Some(Self::RunDesktop {
                presentation_backend: CliPresentationBackend::Software,
            }),
            "package-desktop" => Some(Self::PackageDesktop),
            "package-plugin" => Some(Self::PackagePlugin),
            "export-schemas" => Some(Self::ExportSchemas),
            "export-params" => Some(Self::ExportParams),
            "pin-ids" => Some(Self::PinIds),
            "migrate-manifest" => Some(Self::MigrateManifest { force: false }),
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
            CliCommand::NewProject {
                template,
                package_manager,
            } => {
                parse_new_project_args(&mut args, template, package_manager)?;
            }
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
            CliCommand::MigrateManifest { force } => {
                for argument in args.by_ref() {
                    match argument.as_ref() {
                        "--force" => *force = true,
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
            "  init             Create a new Hawk2UI project [--template react-app|react-plugin|native] [--package-manager bun|npm|pnpm|yarn]",
            "  new              Alias for init",
            "  run              Build and run the default native target",
            "  dev              Watch, rebuild, validate, and hot-reload the native surface",
            "  validate         Validate manifests, sources, and capabilities",
            "  build-dev        Build and write a development sealed artifact",
            "  build-release    Build and write a production sealed artifact",
              "  verify-artifact  Verify a sealed artifact container",
              "  run-desktop      Run a desktop native surface [--presentation-backend software|gpu-preferred|gpu-required]",
              "  package-desktop  Materialize a signed native desktop launcher bundle",
              "  package-plugin   Materialize release-backed CLAP, VST3, and AU package layouts",
            "  export-schemas   Export the central generated JSON Schema catalog",
            "  export-params    Emit truce parameter source generated from the manifest",
            "  pin-ids          Pin a stable numeric id to every unpinned manifest parameter",
            "  migrate-manifest Convert legacy manifest.hawk.toml into canonical hawk.json [--force]",
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

fn parse_new_project_args<I, S>(
    args: &mut I,
    template: &mut CliProjectTemplate,
    package_manager: &mut CliPackageManager,
) -> Result<(), CliError>
where
    I: Iterator<Item = S>,
    S: AsRef<str>,
{
    while let Some(argument) = args.next() {
        match argument.as_ref() {
            "--template" => {
                let Some(value) = args.next() else {
                    return Err(CliError {
                        exit_code: CliExitCode::Usage,
                        message: "--template requires a value".into(),
                    });
                };
                *template =
                    CliProjectTemplate::parse_name(value.as_ref()).ok_or_else(|| CliError {
                        exit_code: CliExitCode::Usage,
                        message: format!("unknown project template: {}", value.as_ref()),
                    })?;
            }
            "--package-manager" | "--pm" => {
                let Some(value) = args.next() else {
                    return Err(CliError {
                        exit_code: CliExitCode::Usage,
                        message: "--package-manager requires a value".into(),
                    });
                };
                *package_manager =
                    CliPackageManager::parse_name(value.as_ref()).ok_or_else(|| CliError {
                        exit_code: CliExitCode::Usage,
                        message: format!("unknown package manager: {}", value.as_ref()),
                    })?;
            }
            other => return Err(unexpected_argument(other)),
        }
    }
    Ok(())
}
