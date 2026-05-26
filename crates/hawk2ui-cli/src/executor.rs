//! Filesystem-backed command execution for the `Hawk2UI` CLI.

use std::{
    fs,
    path::{Path, PathBuf},
};

use hawk2ui_build::{
    ArtifactSchemaVersion, AssetCompilationError, BuildWorkspace, BuildWorkspaceError,
    BuildWorkspaceOutput, HawkManifest, ManifestError, PackageTarget,
};
use hawk2ui_host::{DesktopWindowConfig, SurfaceMetrics};
use hawk2ui_host_winit::{WinitDesktopRuntime, WinitDesktopRuntimeConfig};
use hawk2ui_plugin::{
    BundleOutput, FormatMetadata, ParameterModel, ParameterRange, ParameterRecord,
};
use hawk2ui_plugin_adapters::{
    PackageAdapterSet, PackageFormat, PackageMaterializationError, PackageRequest,
};

use crate::{CliCommand, CliDiagnostic, CliExitCode};

/// Result of a concrete CLI command execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandExecution {
    /// Exit code returned by the process.
    pub exit_code: CliExitCode,
    /// Text written to stdout.
    pub stdout: String,
    /// Text written to stderr.
    pub stderr: String,
    /// Structured diagnostics produced while executing the command.
    pub diagnostics: Vec<CliDiagnostic>,
}

impl CommandExecution {
    /// Creates a successful execution result.
    #[must_use]
    pub fn success(stdout: impl Into<String>) -> Self {
        Self {
            exit_code: CliExitCode::Success,
            stdout: stdout.into(),
            stderr: String::new(),
            diagnostics: Vec::new(),
        }
    }

    /// Creates a failed execution result with rendered diagnostics on stderr.
    #[must_use]
    pub fn failure(exit_code: CliExitCode, diagnostics: Vec<CliDiagnostic>) -> Self {
        let stderr = render_diagnostics(&diagnostics);
        Self {
            exit_code,
            stdout: String::new(),
            stderr,
            diagnostics,
        }
    }
}

/// Filesystem-backed command runner rooted at a `Hawk2UI` project directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceCommandRunner {
    root: PathBuf,
}

impl WorkspaceCommandRunner {
    /// Creates a command runner rooted at a project directory.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Executes one parsed command.
    #[must_use]
    pub fn execute(&self, command: CliCommand) -> CommandExecution {
        match command {
            CliCommand::NewProject => self.new_project(),
            CliCommand::Validate => self.validate(),
            CliCommand::BuildDev => self.build("development"),
            CliCommand::BuildRelease => self.build("production"),
            CliCommand::VerifyArtifact => self.verify_artifact(),
            CliCommand::RunDesktop => self.run_desktop(),
            CliCommand::PackagePlugin => self.package_plugin(),
            CliCommand::Diagnostics => self.diagnostics(),
        }
    }

    fn new_project(&self) -> CommandExecution {
        let manifest_path = self.manifest_path();
        if manifest_path.exists() {
            return CommandExecution::failure(
                CliExitCode::Usage,
                vec![
                    CliDiagnostic::error("project.exists", "manifest.hawk.toml already exists")
                        .file(manifest_path.display().to_string()),
                ],
            );
        }

        let source_dir = self.root.join("src");
        if let Err(error) = fs::create_dir_all(&source_dir) {
            return io_failure("project.create-dir-failed", &source_dir, &error);
        }
        let source_path = source_dir.join("main.ts");
        if let Err(error) = fs::write(&source_path, "export const app = \"hawk2ui\";\n") {
            return io_failure("project.write-source-failed", &source_path, &error);
        }
        if let Err(error) = fs::write(&manifest_path, default_manifest()) {
            return io_failure("project.write-manifest-failed", &manifest_path, &error);
        }
        CommandExecution::success(format!(
            "created Hawk2UI project at {}\n",
            self.root.display()
        ))
    }

    fn validate(&self) -> CommandExecution {
        match self.build_workspace() {
            Ok(output) => CommandExecution::success(format!(
                "validated manifest {}\n",
                output.manifest.identity.id
            )),
            Err(execution) => execution,
        }
    }

    fn build(&self, profile: &str) -> CommandExecution {
        let output = match self.build_workspace() {
            Ok(output) => output,
            Err(execution) => return execution,
        };
        CommandExecution::success(format!(
            "built {profile} artifact for {}\nmanifest-hash: {}\ncontent-hash: {}\ncompiled-scripts: {}\ncompiled-styles: {}\ncompiled-assets: {}\n",
            output.manifest.identity.id,
            output.artifact.hashes.manifest.0,
            output.artifact.hashes.content.0,
            output.artifact.compiled_scripts.len(),
            output.artifact.compiled_styles.len(),
            output.artifact.compiled_assets.len()
        ))
    }

    fn verify_artifact(&self) -> CommandExecution {
        let output = match self.build_workspace() {
            Ok(output) => output,
            Err(execution) => return execution,
        };
        if output
            .artifact
            .ensure_compatible_with(ArtifactSchemaVersion::new(1, 0))
            .is_err()
        {
            return CommandExecution::failure(
                CliExitCode::Verification,
                vec![CliDiagnostic::error(
                    "artifact.schema.incompatible",
                    "sealed artifact schema is incompatible",
                )],
            );
        }
        let expected_hash = output.artifact.content_hash();
        if expected_hash != output.artifact.hashes.content {
            return CommandExecution::failure(
                CliExitCode::Verification,
                vec![CliDiagnostic::error(
                    "artifact.hash.mismatch",
                    "sealed artifact content hash does not match stable payload",
                )],
            );
        }
        CommandExecution::success(format!(
            "verified artifact {}\n",
            output.artifact.hashes.content.0
        ))
    }

    fn run_desktop(&self) -> CommandExecution {
        let manifest = match self.validated_manifest() {
            Ok(manifest) => manifest,
            Err(execution) => return execution,
        };
        if !manifest.has_target(PackageTarget::Desktop) {
            return CommandExecution::failure(
                CliExitCode::Runtime,
                vec![CliDiagnostic::target_incompatibility(
                    "desktop",
                    "manifest does not declare a desktop target",
                )],
            );
        }
        let exit_after_first_frame = std::env::var_os("HAWK2UI_EXIT_AFTER_FIRST_FRAME").is_some();
        let config = desktop_runtime_config_from_manifest(&manifest, exit_after_first_frame);
        match WinitDesktopRuntime::new().run_blocking(config) {
            Ok(summary) => CommandExecution::success(format!(
                "desktop runtime exited cleanly\nframes-presented: {}\nresizes: {}\ndpi-changes: {}\ninput-events: {}\nclose-requested: {}\n",
                summary.frames_presented,
                summary.resizes,
                summary.dpi_changes,
                summary.input_events,
                summary.close_requested
            )),
            Err(error) => CommandExecution::failure(
                CliExitCode::Runtime,
                vec![CliDiagnostic::error(error.rule(), error.message())],
            ),
        }
    }

    fn package_plugin(&self) -> CommandExecution {
        let manifest = match self.validated_manifest() {
            Ok(manifest) => manifest,
            Err(execution) => return execution,
        };
        if !manifest.has_target(PackageTarget::Plugin) {
            return CommandExecution::failure(
                CliExitCode::Validation,
                vec![CliDiagnostic::target_incompatibility(
                    "plugin",
                    "manifest does not declare a plugin target",
                )],
            );
        }
        let Some(plugin) = &manifest.plugin else {
            return CommandExecution::failure(
                CliExitCode::Validation,
                vec![CliDiagnostic::error(
                    "plugin.metadata.missing",
                    "plugin target requires [plugin] metadata",
                )],
            );
        };
        if manifest.editor.is_none() {
            return CommandExecution::failure(
                CliExitCode::Validation,
                vec![CliDiagnostic::error(
                    "plugin.editor.missing",
                    "plugin target requires [editor] metadata",
                )],
            );
        }

        let parameters = match parameter_model(&manifest) {
            Ok(parameters) => parameters,
            Err(diagnostics) => {
                return CommandExecution::failure(CliExitCode::Validation, diagnostics);
            }
        };
        let metadata = FormatMetadata::new(&plugin.id, &plugin.name, "Hawk2UI")
            .version(&manifest.identity.version);
        let output = BundleOutput::new(
            self.root.join("target/hawk2ui").to_string_lossy(),
            bundle_name(&manifest.identity.id),
        );
        let request = [
            PackageFormat::Clap,
            PackageFormat::Vst3,
            PackageFormat::Au,
            PackageFormat::Standalone,
        ]
        .into_iter()
        .fold(
            PackageRequest::new(metadata, output, parameters),
            PackageRequest::with_format,
        );
        let plan = match PackageAdapterSet::new().plan(&request) {
            Ok(plan) => plan,
            Err(error) => {
                let diagnostics = error
                    .diagnostics()
                    .iter()
                    .map(|diagnostic| {
                        CliDiagnostic::error(diagnostic.rule(), "plugin package planning failed")
                    })
                    .collect();
                return CommandExecution::failure(CliExitCode::Validation, diagnostics);
            }
        };
        let outputs = match plan.materialize() {
            Ok(outputs) => outputs,
            Err(error) => {
                return CommandExecution::failure(
                    CliExitCode::Validation,
                    vec![package_materialization_diagnostic(&error)],
                );
            }
        };
        let mut stdout = String::from("materialized plugin package outputs:\n");
        for target in &outputs {
            stdout.push_str("- ");
            stdout.push_str(&target.output_path);
            stdout.push('\n');
        }
        CommandExecution::success(stdout)
    }

    fn diagnostics(&self) -> CommandExecution {
        match self.build_workspace() {
            Ok(_) => CommandExecution::success("no diagnostics\n"),
            Err(execution) => execution,
        }
    }

    fn validated_manifest(&self) -> Result<HawkManifest, CommandExecution> {
        match self.load_manifest() {
            Ok(manifest) => match self.validate_manifest_sources(&manifest) {
                Ok(()) => Ok(manifest),
                Err(diagnostics) => Err(CommandExecution::failure(
                    CliExitCode::Validation,
                    diagnostics,
                )),
            },
            Err(diagnostic) => Err(CommandExecution::failure(
                CliExitCode::Validation,
                vec![*diagnostic],
            )),
        }
    }

    fn build_workspace(&self) -> Result<BuildWorkspaceOutput, CommandExecution> {
        BuildWorkspace::load(&self.root)
            .and_then(|workspace| workspace.build(ArtifactSchemaVersion::new(1, 0)))
            .map_err(|error| {
                CommandExecution::failure(
                    CliExitCode::Validation,
                    vec![build_workspace_error_diagnostic(error, &self.root)],
                )
            })
    }

    fn load_manifest(&self) -> Result<HawkManifest, Box<CliDiagnostic>> {
        let manifest_path = self.manifest_path();
        let input = fs::read_to_string(&manifest_path).map_err(|error| {
            Box::new(
                CliDiagnostic::error(
                    "manifest.read-failed",
                    format!("failed to read manifest.hawk.toml: {error}"),
                )
                .file(manifest_path.display().to_string()),
            )
        })?;
        HawkManifest::parse(&input).map_err(|error| {
            Box::new(manifest_error_diagnostic(error).file(manifest_path.display().to_string()))
        })
    }

    fn validate_manifest_sources(&self, manifest: &HawkManifest) -> Result<(), Vec<CliDiagnostic>> {
        let mut diagnostics = Vec::new();
        require_file(
            &self.root,
            &manifest.source.entry,
            "source.entry",
            &mut diagnostics,
        );
        if let Some(style) = &manifest.source.style {
            require_file(&self.root, style, "source.style", &mut diagnostics);
        }
        if let Some(script) = &manifest.source.script {
            require_file(&self.root, script, "source.script", &mut diagnostics);
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }

    fn manifest_path(&self) -> PathBuf {
        self.root.join("manifest.hawk.toml")
    }
}

fn require_file(root: &Path, relative: &str, field: &str, diagnostics: &mut Vec<CliDiagnostic>) {
    let path = root.join(relative);
    if !path.is_file() {
        diagnostics.push(
            CliDiagnostic::error(
                "source.file-missing",
                format!("{field} does not point to an existing file: {relative}"),
            )
            .file(path.display().to_string()),
        );
    }
}

fn parameter_model(manifest: &HawkManifest) -> Result<ParameterModel, Vec<CliDiagnostic>> {
    let mut diagnostics = Vec::new();
    let mut parameters = Vec::new();
    for parameter in &manifest.parameters {
        match ParameterRange::try_new(0.0, 1.0, parameter.default) {
            Ok(range) => {
                parameters.push(ParameterRecord::numeric(
                    &parameter.id,
                    &parameter.name,
                    "",
                    range,
                ));
            }
            Err(error) => diagnostics.push(CliDiagnostic::error(
                error.code,
                format!("parameter {} is invalid: {}", parameter.id, error.message),
            )),
        }
    }
    let model = ParameterModel::new(parameters);
    if let Err(errors) = model.validate() {
        diagnostics.extend(
            errors
                .into_iter()
                .map(|error| CliDiagnostic::error(error.code, error.message)),
        );
    }
    if diagnostics.is_empty() {
        Ok(model)
    } else {
        Err(diagnostics)
    }
}

fn manifest_error_diagnostic(error: ManifestError) -> CliDiagnostic {
    match error {
        ManifestError::Parse(message) => CliDiagnostic::error("manifest.parse", message),
        ManifestError::MissingSection(section) => CliDiagnostic::error(
            "manifest.section-missing",
            format!("required manifest section is missing: {section}"),
        ),
        ManifestError::MissingField(field) => CliDiagnostic::error(
            "manifest.field-missing",
            format!("required manifest field is missing: {field}"),
        ),
        ManifestError::DuplicateTarget(target) => CliDiagnostic::error(
            "manifest.target.duplicate",
            format!("duplicate target declaration: {target}"),
        ),
        ManifestError::DuplicateAsset(asset) => CliDiagnostic::error(
            "manifest.asset.duplicate",
            format!("duplicate asset declaration: {asset}"),
        ),
        ManifestError::DuplicatePreset(preset) => CliDiagnostic::error(
            "manifest.preset.duplicate",
            format!("duplicate preset declaration: {preset}"),
        ),
        ManifestError::InvalidCapability(capability) => CliDiagnostic::error(
            "manifest.capability.invalid",
            format!("invalid capability key: {capability}"),
        ),
        ManifestError::InvalidPluginMetadata(message) => {
            CliDiagnostic::error("manifest.plugin.invalid", message)
        }
    }
}

fn build_workspace_error_diagnostic(error: BuildWorkspaceError, root: &Path) -> CliDiagnostic {
    match error {
        BuildWorkspaceError::MissingFile(path) => CliDiagnostic::error(
            "build.file-missing",
            format!("declared build file is missing: {path}"),
        )
        .file(root.join(path).display().to_string()),
        BuildWorkspaceError::UnreadableFile(path) => CliDiagnostic::error(
            "build.file-unreadable",
            format!("declared build file could not be read: {path}"),
        )
        .file(root.join(path).display().to_string()),
        BuildWorkspaceError::UnsafePath(path) => CliDiagnostic::error(
            "build.path.unsafe",
            format!("declared build path escapes the workspace: {path}"),
        ),
        BuildWorkspaceError::ManifestInvalid(error) => manifest_error_diagnostic(error)
            .file(root.join("manifest.hawk.toml").display().to_string()),
        BuildWorkspaceError::AssetCompilation(error) => asset_compilation_diagnostic(error, root),
        BuildWorkspaceError::PipelineBlocked(error) => CliDiagnostic::error(
            "build.pipeline.blocked",
            format!("production build pipeline is blocked: {error:?}"),
        ),
    }
}

fn asset_compilation_diagnostic(error: AssetCompilationError, root: &Path) -> CliDiagnostic {
    match error {
        AssetCompilationError::MissingAsset { path, .. } => CliDiagnostic::error(
            "asset.missing",
            format!("declared asset source is missing: {path}"),
        )
        .file(root.join(path).display().to_string()),
        AssetCompilationError::UnsafeAsset { path, .. } => CliDiagnostic::error(
            "asset.unsafe",
            format!("declared asset failed safety validation: {path}"),
        )
        .file(root.join(path).display().to_string()),
        AssetCompilationError::UnsupportedAssetKind { kind, .. } => CliDiagnostic::error(
            "asset.kind.unsupported",
            format!("declared asset kind is unsupported: {kind}"),
        ),
    }
}

fn package_materialization_diagnostic(error: &PackageMaterializationError) -> CliDiagnostic {
    CliDiagnostic::error(
        error.diagnostic().rule(),
        "plugin package materialization failed",
    )
}

fn io_failure(rule: &str, path: &Path, error: &std::io::Error) -> CommandExecution {
    CommandExecution::failure(
        CliExitCode::Runtime,
        vec![CliDiagnostic::error(rule, error.to_string()).file(path.display().to_string())],
    )
}

fn render_diagnostics(diagnostics: &[CliDiagnostic]) -> String {
    diagnostics
        .iter()
        .map(CliDiagnostic::render)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn bundle_name(identity_id: &str) -> String {
    identity_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn desktop_runtime_config_from_manifest(
    manifest: &HawkManifest,
    exit_after_first_frame: bool,
) -> WinitDesktopRuntimeConfig {
    let (width, height) = manifest.editor.as_ref().map_or((960.0, 540.0), |editor| {
        (f64::from(editor.width), f64::from(editor.height))
    });
    WinitDesktopRuntimeConfig::new(DesktopWindowConfig::new(
        manifest.identity.name.clone(),
        SurfaceMetrics::new(width, height, 1.0),
    ))
    .with_exit_after_first_frame(exit_after_first_frame)
}

fn default_manifest() -> &'static str {
    r#"[identity]
id = "com.example.hawk2ui-app"
name = "Hawk2UI App"
version = "0.1.0"

[source]
entry = "src/main.ts"

[capabilities]
keys = ["native-windowing"]

[[targets]]
kind = "desktop"
name = "local-desktop"
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_runtime_config_from_manifest_uses_editor_size_and_title() {
        let manifest = HawkManifest::parse(
            r#"[identity]
id = "com.example.desktop"
name = "Desktop Smoke"
version = "1.0.0"

[source]
entry = "src/main.ts"

[editor]
width = 1280
height = 720

[[targets]]
kind = "desktop"
name = "linux-wayland"
"#,
        )
        .expect("manifest parses");

        let config = desktop_runtime_config_from_manifest(&manifest, true);

        assert_eq!(config.window().title, "Desktop Smoke");
        assert_eq!(config.window().metrics.logical_width, 1280.0);
        assert_eq!(config.window().metrics.logical_height, 720.0);
        assert!(config.exit_after_first_frame());
    }
}
