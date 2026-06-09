//! CLI command surface definitions.

use richrs::{
    prelude::{Color, Column, Panel, Style, Table, Text},
    table::Row,
};
use serde::{Deserialize, Serialize};

/// CLI command.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CliCommand {
    /// Render top-level help.
    Help,
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
            "help" | "--help" | "-h" => Some(Self::Help),
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

const HAWK2UI_CLI_LOGO: &str = r#"░▒▓█▓▒░░▒▓█▓▒░░▒▓██████▓▒░░▒▓█▓▒░░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░░▒▓█▓▒░▒▓███████▓▒░░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░
░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░░▒▓█▓▒░      ░▒▓█▓▒░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░
░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░░▒▓█▓▒░      ░▒▓█▓▒░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░
░▒▓████████▓▒░▒▓████████▓▒░▒▓█▓▒░░▒▓█▓▒░░▒▓█▓▒░▒▓███████▓▒░ ░▒▓██████▓▒░░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░
░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░      ░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░
░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░      ░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░
░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░░▒▓█▓▒░░▒▓█████████████▓▒░░▒▓█▓▒░░▒▓█▓▒░▒▓████████▓▒░░▒▓██████▓▒░░▒▓█▓▒░"#;

const HELP_RENDER_WIDTH: usize = 120;

struct HelpCommand {
    area: &'static str,
    name: &'static str,
    purpose: &'static str,
}

const HELP_COMMANDS: &[HelpCommand] = &[
    HelpCommand {
        area: "CLI",
        name: "help",
        purpose: "Show this help",
    },
    HelpCommand {
        area: "Create",
        name: "init",
        purpose: "Create a project with --template and --package-manager options",
    },
    HelpCommand {
        area: "Create",
        name: "new",
        purpose: "Alias for init; accepts the same scaffold options",
    },
    HelpCommand {
        area: "Develop",
        name: "run",
        purpose: "Build and run the default native target",
    },
    HelpCommand {
        area: "Develop",
        name: "dev",
        purpose: "Watch, rebuild, validate, and hot-reload the native surface",
    },
    HelpCommand {
        area: "Verify",
        name: "validate",
        purpose: "Validate manifests, sources, and capabilities",
    },
    HelpCommand {
        area: "Build",
        name: "build-dev",
        purpose: "Build and write a development sealed artifact",
    },
    HelpCommand {
        area: "Build",
        name: "build-release",
        purpose: "Build and write a production sealed artifact",
    },
    HelpCommand {
        area: "Verify",
        name: "verify-artifact",
        purpose: "Verify a sealed artifact container",
    },
    HelpCommand {
        area: "Develop",
        name: "run-desktop",
        purpose: "Run a desktop surface with software or GPU presentation",
    },
    HelpCommand {
        area: "Package",
        name: "package-desktop",
        purpose: "Materialize a signed native desktop launcher bundle",
    },
    HelpCommand {
        area: "Package",
        name: "package-plugin",
        purpose: "Materialize release-backed CLAP, VST3, and AU package layouts",
    },
    HelpCommand {
        area: "Inspect",
        name: "export-schemas",
        purpose: "Export the central generated JSON Schema catalog",
    },
    HelpCommand {
        area: "Inspect",
        name: "export-params",
        purpose: "Emit truce parameter source generated from the manifest",
    },
    HelpCommand {
        area: "Manifest",
        name: "pin-ids",
        purpose: "Pin stable numeric ids to unpinned parameters",
    },
    HelpCommand {
        area: "Manifest",
        name: "migrate-manifest",
        purpose: "Convert legacy manifest.hawk.toml into canonical hawk.json [--force]",
    },
    HelpCommand {
        area: "Inspect",
        name: "diagnostics",
        purpose: "Render structured diagnostics",
    },
    HelpCommand {
        area: "Inspect",
        name: "explain",
        purpose: "Explain project targets, capabilities, and next commands",
    },
];

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
        let mut help = String::new();
        help.push_str(HAWK2UI_CLI_LOGO);
        help.push_str("\n\n");
        help.push_str(&render_help_panel());
        help.push_str("\n\n");
        help.push_str(&render_workflow_table());
        help.push_str("\n\nCommands:\n");
        help.push_str(&render_commands_table());
        help.push_str("\n\nScaffold options:\n");
        help.push_str("  --template react-app|react-plugin|vue-app|vue-plugin|native\n");
        help.push_str("  --package-manager bun|npm|pnpm|yarn");
        help
    }
}

fn render_help_panel() -> String {
    let content = Text::assemble([
        ("Usage: ", Some(label_style())),
        ("hawk2ui-cli <command> [options]\n", Some(command_style())),
        (
            "Scaffold React/Vue app and plugin projects, validate manifests, build sealed artifacts, and package releases.",
            Some(body_style()),
        ),
    ]);
    Panel::fit(content)
        .title(Text::styled("Hawk2UI CLI", title_style()))
        .subtitle(Text::styled(
            "React, Vue, native, and plugin workflows",
            muted_style(),
        ))
        .border_style(accent_style())
        .padding(2, 0, 2, 0)
        .render(HELP_RENDER_WIDTH)
        .to_ansi()
        .trim_end_matches('\n')
        .to_owned()
}

fn render_workflow_table() -> String {
    let mut table = Table::new()
        .title(Text::styled("Start here", title_style()))
        .border_style(border_style())
        .header_style(label_style())
        .show_lines(false)
        .padding(1, 0);
    table.add_column(Column::new(Text::styled("Goal", label_style())).min_width(18));
    table.add_column(Column::new(Text::styled("Command", label_style())).min_width(36));
    table.add_column(Column::new(Text::styled("Notes", label_style())).min_width(52));
    for row in [
        (
            "React app",
            "hawk2ui-cli init --template react-app",
            "Scaffold a desktop app backed by @hawk2ui/react",
        ),
        (
            "Vue plugin",
            "hawk2ui-cli init --template vue-plugin",
            "Create a plugin editor using @hawk2ui/vue",
        ),
        (
            "Release",
            "hawk2ui-cli validate && hawk2ui-cli build-release",
            "Check project inputs before sealing artifacts",
        ),
        (
            "Plugin package",
            "hawk2ui-cli package-plugin",
            "Materialize CLAP, VST3, and AU package layouts",
        ),
    ] {
        table.add_row(Row::new(vec![
            Text::styled(row.0, area_style()),
            Text::styled(row.1, command_style()),
            Text::styled(row.2, body_style()),
        ]));
    }
    table
        .render(HELP_RENDER_WIDTH)
        .to_ansi()
        .trim_end_matches('\n')
        .to_owned()
}

fn render_commands_table() -> String {
    let mut table = Table::new()
        .title(Text::styled("Command reference", title_style()))
        .border_style(border_style())
        .header_style(label_style())
        .show_lines(false)
        .padding(1, 0);
    table.add_column(Column::new(Text::styled("Area", label_style())).min_width(10));
    table.add_column(Column::new(Text::styled("Command", label_style())).min_width(18));
    table.add_column(Column::new(Text::styled("Purpose", label_style())).min_width(76));
    for command in HELP_COMMANDS {
        table.add_row(Row::new(vec![
            Text::styled(command.area, area_style()),
            Text::styled(command.name, command_style()),
            Text::styled(command.purpose, body_style()),
        ]));
    }
    table
        .render(HELP_RENDER_WIDTH)
        .to_ansi()
        .trim_end_matches('\n')
        .to_owned()
}

fn title_style() -> Style {
    Style::color(Color::rgb(125, 249, 255)).bold()
}

fn accent_style() -> Style {
    Style::color(Color::rgb(45, 212, 191)).bold()
}

fn border_style() -> Style {
    Style::color(Color::rgb(56, 189, 248))
}

fn label_style() -> Style {
    Style::color(Color::rgb(203, 213, 225)).bold()
}

fn area_style() -> Style {
    Style::color(Color::rgb(167, 139, 250)).bold()
}

fn command_style() -> Style {
    Style::color(Color::rgb(134, 239, 172)).bold()
}

fn body_style() -> Style {
    Style::color(Color::rgb(226, 232, 240))
}

fn muted_style() -> Style {
    Style::color(Color::rgb(148, 163, 184)).italic()
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
