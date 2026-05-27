//! Filesystem-backed command execution for the `Hawk2UI` CLI.

use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path, PathBuf},
};

use hawk2ui_assets::{AssetBackend, AssetHash, AssetLimits, AssetRecord};
use hawk2ui_build::{
    ArtifactSchemaVersion, AssetCompilationError, BuildWorkspace, BuildWorkspaceError,
    BuildWorkspaceOutput, CompiledScriptRecord, HawkManifest, ManifestError, PackageTarget,
};
use hawk2ui_host::{DesktopWindowConfig, SurfaceMetrics};
use hawk2ui_host_winit::{WinitDesktopRuntime, WinitDesktopRuntimeConfig};
use hawk2ui_layout::{BoxEdges, FlexDirection, LayoutSizing, LayoutStyle, LayoutValue};
use hawk2ui_plugin::{
    BundleOutput, FormatMetadata, ParameterModel, ParameterRange, ParameterRecord,
};
use hawk2ui_plugin_adapters::{
    PackageAdapterSet, PackageFormat, PackageMaterializationError, PackageRequest,
    VerificationStatus,
};
use hawk2ui_render::Color;
use hawk2ui_runtime::{
    RuntimeSceneError, RuntimeTextVisual, RuntimeViewId, RuntimeViewNode, RuntimeViewTree,
    RuntimeVisual,
};
use hawk2ui_script::{HostCallPolicy, ScriptBackend, ScriptModule, StructuredValue, TimerPolicy};

use crate::{CliCommand, CliDiagnostic, CliExitCode};

#[derive(Clone, Debug, PartialEq)]
struct DesktopEntryAppModel {
    root: DesktopEntryNode,
}

impl DesktopEntryAppModel {
    fn manifest_fallback(visible_title: impl Into<String>) -> Self {
        let root_id = "root".to_string();
        Self {
            root: DesktopEntryNode::view(
                root_id.clone(),
                vec![DesktopEntryNode::text(
                    format!("{root_id}-title"),
                    visible_title.into(),
                )],
            ),
        }
    }

    fn from_mount_json(value: &str) -> Result<Self, String> {
        let value: serde_json::Value = serde_json::from_str(value)
            .map_err(|error| format!("native mount result is not valid JSON: {error}"))?;
        let root = DesktopEntryNode::from_json(&value)?;
        validate_desktop_entry_tree_ids(&root)?;
        Ok(Self { root })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DesktopEntryNodeKind {
    View,
    Text,
}

#[derive(Clone, Debug, PartialEq)]
struct DesktopEntryNode {
    id: String,
    kind: DesktopEntryNodeKind,
    text: Option<String>,
    props: DesktopEntryNodeProps,
    children: Vec<Self>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct DesktopEntryNodeProps {
    background_color: Option<Color>,
    text_color: Option<Color>,
    font_size: Option<f32>,
    width: Option<f32>,
    height: Option<f32>,
    padding: Option<f32>,
    gap: Option<f32>,
}

impl DesktopEntryNode {
    fn view(id: impl Into<String>, children: Vec<Self>) -> Self {
        Self {
            id: id.into(),
            kind: DesktopEntryNodeKind::View,
            text: None,
            props: DesktopEntryNodeProps::default(),
            children,
        }
    }

    fn text(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: DesktopEntryNodeKind::Text,
            text: Some(text.into()),
            props: DesktopEntryNodeProps::default(),
            children: Vec::new(),
        }
    }

    fn with_props(mut self, props: DesktopEntryNodeProps) -> Self {
        self.props = props;
        self
    }

    fn from_json(value: &serde_json::Value) -> Result<Self, String> {
        let id = non_empty_json_string(value, "id")?;
        let props = DesktopEntryNodeProps::from_json(value.get("props"))?;
        let raw_kind = value
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| {
                if value.get("text").is_some() {
                    "text"
                } else {
                    "view"
                }
            });
        match raw_kind {
            "view" => {
                let children = json_children(value)?
                    .iter()
                    .map(Self::from_json)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Self::view(id, children).with_props(props))
            }
            "text" => {
                let text = non_empty_json_string(value, "text")?;
                if value.get("children").is_some() {
                    return Err(format!("text node '{id}' must not declare children"));
                }
                Ok(Self::text(id, text).with_props(props))
            }
            _ => Err(format!("node '{id}' uses unsupported type '{raw_kind}'")),
        }
    }
}

impl DesktopEntryNodeProps {
    fn from_json(value: Option<&serde_json::Value>) -> Result<Self, String> {
        let Some(value) = value else {
            return Ok(Self::default());
        };
        let serde_json::Value::Object(props) = value else {
            return Err("field 'props' must be an object".to_string());
        };
        let mut result = Self::default();
        for (name, value) in props {
            match name.as_str() {
                "backgroundColor" => {
                    result.background_color = Some(json_color_prop(value, name)?);
                }
                "color" => {
                    result.text_color = Some(json_color_prop(value, name)?);
                }
                "fontSize" => {
                    result.font_size = Some(json_positive_number_prop(value, name)?);
                }
                "width" => {
                    result.width = Some(json_positive_number_prop(value, name)?);
                }
                "height" => {
                    result.height = Some(json_positive_number_prop(value, name)?);
                }
                "padding" => {
                    result.padding = Some(json_non_negative_number_prop(value, name)?);
                }
                "gap" => {
                    result.gap = Some(json_non_negative_number_prop(value, name)?);
                }
                _ => return Err(format!("unsupported native node prop '{name}'")),
            }
        }
        Ok(result)
    }
}

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
        let output = match self.build_workspace() {
            Ok(output) => output,
            Err(execution) => return execution,
        };
        let manifest = &output.manifest;
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
        let mut config =
            match desktop_runtime_config_from_build_output(&output, exit_after_first_frame) {
                Ok(config) => config,
                Err(diagnostic) => {
                    return CommandExecution::failure(CliExitCode::Runtime, vec![*diagnostic]);
                }
            };
        let runtime_assets = match desktop_runtime_assets(&self.root, manifest) {
            Ok(assets) => assets,
            Err(diagnostic) => {
                return CommandExecution::failure(CliExitCode::Runtime, vec![*diagnostic]);
            }
        };
        config = config.with_runtime_assets(runtime_assets);
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
        let verification = plan.verify_materialized(&outputs);
        if verification.status() == VerificationStatus::Failed {
            return CommandExecution::failure(
                CliExitCode::Verification,
                vec![package_verification_diagnostic()],
            );
        }
        let mut stdout = String::from("materialized plugin package outputs:\n");
        for target in &outputs {
            stdout.push_str("- ");
            stdout.push_str(&target.output_path);
            stdout.push('\n');
        }
        stdout.push_str("verification-status: passed\n");
        CommandExecution::success(stdout)
    }

    fn diagnostics(&self) -> CommandExecution {
        match self.build_workspace() {
            Ok(_) => CommandExecution::success("no diagnostics\n"),
            Err(execution) => execution,
        }
    }

    fn validated_manifest(&self) -> Result<HawkManifest, CommandExecution> {
        self.build_workspace().map(|output| output.manifest)
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

    fn manifest_path(&self) -> PathBuf {
        self.root.join("manifest.hawk.toml")
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
        ManifestError::SchemaValidation => CliDiagnostic::error(
            "manifest.schema.invalid",
            "manifest does not match the generated Hawk2UI manifest schema",
        ),
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

fn package_verification_diagnostic() -> CliDiagnostic {
    CliDiagnostic::error(
        "plugin.package.verification-failed",
        "plugin package verification failed",
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

#[cfg(test)]
fn desktop_runtime_config_from_manifest(
    manifest: &HawkManifest,
    exit_after_first_frame: bool,
) -> Result<WinitDesktopRuntimeConfig, Box<CliDiagnostic>> {
    let app_model = DesktopEntryAppModel::manifest_fallback(manifest.identity.name.as_str());
    desktop_runtime_config_from_manifest_with_app_model(
        manifest,
        &app_model,
        exit_after_first_frame,
    )
}

fn desktop_runtime_config_from_build_output(
    output: &BuildWorkspaceOutput,
    exit_after_first_frame: bool,
) -> Result<WinitDesktopRuntimeConfig, Box<CliDiagnostic>> {
    let app_model = entry_script_app_model(output)?.unwrap_or_else(|| {
        DesktopEntryAppModel::manifest_fallback(output.manifest.identity.name.clone())
    });
    desktop_runtime_config_from_manifest_with_app_model(
        &output.manifest,
        &app_model,
        exit_after_first_frame,
    )
}

fn desktop_runtime_config_from_manifest_with_app_model(
    manifest: &HawkManifest,
    app_model: &DesktopEntryAppModel,
    exit_after_first_frame: bool,
) -> Result<WinitDesktopRuntimeConfig, Box<CliDiagnostic>> {
    let (width, height) = manifest.editor.as_ref().map_or((960.0, 540.0), |editor| {
        (f64::from(editor.width), f64::from(editor.height))
    });
    let runtime_tree = runtime_tree_from_manifest(app_model, width, height)?;
    Ok(WinitDesktopRuntimeConfig::new(DesktopWindowConfig::new(
        manifest.identity.name.clone(),
        SurfaceMetrics::new(width, height, 1.0),
    ))
    .with_runtime_tree(runtime_tree)
    .with_exit_after_first_frame(exit_after_first_frame))
}

fn desktop_runtime_assets(
    root: &Path,
    manifest: &HawkManifest,
) -> Result<Vec<AssetRecord>, Box<CliDiagnostic>> {
    let mut backend = AssetBackend::new(AssetLimits::default());
    let mut records = Vec::new();
    for asset in &manifest.assets {
        if asset.kind == "design-token" {
            continue;
        }
        let bytes = read_runtime_asset_bytes(root, &asset.path)?;
        let hash = AssetHash::sha256_bytes(&bytes);
        let record = match asset.kind.as_str() {
            "image" => backend.compile_image(&asset.id, &asset.path, &bytes, &hash),
            "vector" => backend.compile_vector(&asset.id, &asset.path, &bytes, &hash),
            "font" => backend.load_font(&asset.id, &asset.path, &bytes, &hash),
            _ => {
                return Err(Box::new(CliDiagnostic::error(
                    "asset.kind.unsupported",
                    format!(
                        "runtime asset {} declares unsupported kind {}",
                        asset.id, asset.kind
                    ),
                )));
            }
        }
        .map_err(|error| {
            Box::new(CliDiagnostic::error(
                error.diagnostic().rule(),
                format!(
                    "runtime asset {} at {} failed compilation: {}",
                    asset.id,
                    asset.path,
                    error.diagnostic().message()
                ),
            ))
        })?;
        records.push(record);
    }
    Ok(records)
}

fn read_runtime_asset_bytes(root: &Path, path: &str) -> Result<Vec<u8>, Box<CliDiagnostic>> {
    let relative = Path::new(path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(Box::new(CliDiagnostic::error(
            "asset.path.unsafe",
            format!("runtime asset path must stay inside the project: {path}"),
        )));
    }
    let root = root.canonicalize().map_err(|error| {
        Box::new(CliDiagnostic::error(
            "asset.workspace.unreadable",
            format!("failed to resolve project root for runtime assets: {error}"),
        ))
    })?;
    let absolute = root.join(relative);
    let resolved = absolute.canonicalize().map_err(|error| {
        Box::new(CliDiagnostic::error(
            "asset.read-failed",
            format!("failed to resolve runtime asset {path}: {error}"),
        ))
    })?;
    if !resolved.starts_with(&root) {
        return Err(Box::new(CliDiagnostic::error(
            "asset.path.unsafe",
            format!("runtime asset path escapes the project: {path}"),
        )));
    }
    fs::read(&resolved).map_err(|error| {
        Box::new(CliDiagnostic::error(
            "asset.read-failed",
            format!("failed to read runtime asset {path}: {error}"),
        ))
    })
}

fn runtime_tree_from_manifest(
    app_model: &DesktopEntryAppModel,
    width: f64,
    height: f64,
) -> Result<RuntimeViewTree, Box<CliDiagnostic>> {
    let width = runtime_dimension_to_f32(width)?;
    let height = runtime_dimension_to_f32(height)?;
    let content_width = (width - 48.0).max(1.0);

    let root_id = RuntimeViewId::new(app_model.root.id.clone());
    let root = runtime_node_from_desktop_entry(&app_model.root, width, height, true);
    append_desktop_entry_children(
        RuntimeViewTree::new(root),
        &root_id,
        &app_model.root.children,
        content_width,
    )
    .map_err(|error| runtime_scene_diagnostic(&error))
}

fn append_desktop_entry_children(
    mut tree: RuntimeViewTree,
    parent_id: &RuntimeViewId,
    children: &[DesktopEntryNode],
    content_width: f32,
) -> Result<RuntimeViewTree, RuntimeSceneError> {
    for child in children {
        let child_id = RuntimeViewId::new(child.id.clone());
        let child_height = desktop_entry_node_height(child);
        let child_node = runtime_node_from_desktop_entry(child, content_width, child_height, false);
        tree = tree.with_child(parent_id, child_node)?;
        tree = append_desktop_entry_children(tree, &child_id, &child.children, content_width)?;
    }
    Ok(tree)
}

fn runtime_node_from_desktop_entry(
    node: &DesktopEntryNode,
    width: f32,
    height: f32,
    is_root: bool,
) -> RuntimeViewNode {
    let node_width = node.props.width.unwrap_or(width);
    let node_height = node.props.height.unwrap_or(height);
    let layout_style = match node.kind {
        DesktopEntryNodeKind::View => LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(node_width, node_height))
            .with_padding(BoxEdges::all(LayoutValue::px(
                node.props
                    .padding
                    .unwrap_or(if is_root { 24.0 } else { 0.0 }),
            )))
            .with_gap(LayoutValue::px(node.props.gap.unwrap_or(12.0))),
        DesktopEntryNodeKind::Text => LayoutStyle::flex_container(FlexDirection::Row)
            .with_size(LayoutSizing::fixed(node_width, node_height)),
    };
    let visual = match node.kind {
        DesktopEntryNodeKind::View => RuntimeVisual::Fill(if is_root {
            node.props
                .background_color
                .unwrap_or(Color::rgba(11, 12, 18, 255))
        } else {
            node.props
                .background_color
                .unwrap_or(Color::rgba(20, 22, 31, 255))
        }),
        DesktopEntryNodeKind::Text => RuntimeVisual::Text(RuntimeTextVisual::new(
            node.text.clone().unwrap_or_default(),
            node.props.font_size.unwrap_or(20.0),
            node.props
                .text_color
                .unwrap_or(Color::rgba(241, 245, 249, 255)),
        )),
    };
    RuntimeViewNode::new(RuntimeViewId::new(node.id.clone()), layout_style, visual)
}

fn desktop_entry_node_height(node: &DesktopEntryNode) -> f32 {
    if let Some(height) = node.props.height {
        return height;
    }
    match node.kind {
        DesktopEntryNodeKind::Text => 32.0,
        DesktopEntryNodeKind::View => {
            let children_height: f32 = node.children.iter().map(desktop_entry_node_height).sum();
            let gap_count =
                u16::try_from(node.children.len().saturating_sub(1)).unwrap_or(u16::MAX);
            let gaps = f32::from(gap_count) * node.props.gap.unwrap_or(12.0);
            (children_height + gaps).max(32.0)
        }
    }
}

fn entry_script_app_model(
    output: &BuildWorkspaceOutput,
) -> Result<Option<DesktopEntryAppModel>, Box<CliDiagnostic>> {
    let Some(script) = output
        .artifact
        .compiled_scripts
        .iter()
        .find(|script| script.entrypoint_id == "entry")
    else {
        return Ok(None);
    };

    if let Some(app_model) = entry_script_mount_app_model(script)? {
        return Ok(Some(app_model));
    }
    Ok(entry_script_visible_title(script).map(DesktopEntryAppModel::manifest_fallback))
}

fn entry_script_mount_app_model(
    script: &CompiledScriptRecord,
) -> Result<Option<DesktopEntryAppModel>, Box<CliDiagnostic>> {
    let Some(source) = native_mount_bootstrap_source(script.compiled_source.as_str()) else {
        return Ok(None);
    };
    let mut backend = ScriptBackend::new(HostCallPolicy::deny_all(), TimerPolicy::deterministic());
    let execution = backend
        .execute_module(entry_script_module(script, source.as_str()))
        .map_err(|error| {
            Box::new(CliDiagnostic::error(
                "runtime.desktop.entry-script-failed",
                format!(
                    "failed to execute desktop entry mount function in {}: {}",
                    script.source_path,
                    error.diagnostic().rule()
                ),
            ))
        })?;
    match execution.value() {
        StructuredValue::String(value) => DesktopEntryAppModel::from_mount_json(value)
            .map(Some)
            .map_err(|message| invalid_entry_tree_diagnostic(script, message)),
        StructuredValue::Null | StructuredValue::Bool(_) | StructuredValue::Number(_) => {
            Err(invalid_entry_tree_diagnostic(
                script,
                "native mount function must return a serializable view or text node tree",
            ))
        }
    }
}

fn entry_script_visible_title(script: &CompiledScriptRecord) -> Option<String> {
    let mut backend = ScriptBackend::new(HostCallPolicy::deny_all(), TimerPolicy::deterministic());
    let execution = backend
        .execute_module(entry_script_module(script, script.compiled_source.as_str()))
        .ok()?;
    match execution.value() {
        StructuredValue::String(value) if !value.trim().is_empty() => Some(value.clone()),
        StructuredValue::Null
        | StructuredValue::Bool(_)
        | StructuredValue::Number(_)
        | StructuredValue::String(_) => None,
    }
}

fn entry_script_module(script: &CompiledScriptRecord, source: &str) -> ScriptModule {
    let extension = Path::new(&script.source_path)
        .extension()
        .and_then(|extension| extension.to_str());
    if extension.is_some_and(|extension| {
        extension.eq_ignore_ascii_case("ts") || extension.eq_ignore_ascii_case("tsx")
    }) {
        ScriptModule::typescript(&script.source_path, source)
    } else {
        ScriptModule::javascript(&script.source_path, source)
    }
}

fn native_mount_bootstrap_source(source: &str) -> Option<String> {
    let source = source.replacen("export function mount", "function mount", 1);
    if !source.contains("function mount") {
        return None;
    }
    Some(format!(
        r"{source}

const __hawk2ui_host = Object.freeze({{
    on(_name, _handler) {{}},
    setState(_value) {{}}
}});

JSON.stringify(mount(__hawk2ui_host));
"
    ))
}

fn invalid_entry_tree_diagnostic(
    script: &CompiledScriptRecord,
    message: impl Into<String>,
) -> Box<CliDiagnostic> {
    Box::new(
        CliDiagnostic::error("runtime.desktop.invalid-entry-tree", message.into())
            .file(script.source_path.clone()),
    )
}

fn json_color_prop(value: &serde_json::Value, name: &str) -> Result<Color, String> {
    let value = value
        .as_str()
        .ok_or_else(|| format!("prop '{name}' must be a CSS hex color string"))?;
    parse_hex_color(value)
        .ok_or_else(|| format!("prop '{name}' must use #RRGGBB or #RRGGBBAA hex color syntax"))
}

fn parse_hex_color(value: &str) -> Option<Color> {
    let hex = value.strip_prefix('#')?;
    let (r, g, b, a) = match hex.len() {
        6 => (
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
            255,
        ),
        8 => (
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
            u8::from_str_radix(&hex[6..8], 16).ok()?,
        ),
        _ => return None,
    };
    Some(Color::rgba(r, g, b, a))
}

fn json_positive_number_prop(value: &serde_json::Value, name: &str) -> Result<f32, String> {
    let number = json_number_prop(value, name)?;
    if number <= 0.0 {
        return Err(format!("prop '{name}' must be greater than zero"));
    }
    Ok(number)
}

fn json_non_negative_number_prop(value: &serde_json::Value, name: &str) -> Result<f32, String> {
    let number = json_number_prop(value, name)?;
    if number < 0.0 {
        return Err(format!("prop '{name}' must not be negative"));
    }
    Ok(number)
}

fn json_number_prop(value: &serde_json::Value, name: &str) -> Result<f32, String> {
    let serde_json::Value::Number(number) = value else {
        return Err(format!("prop '{name}' must be a number"));
    };
    let parsed = number
        .to_string()
        .parse::<f32>()
        .map_err(|_| format!("prop '{name}' cannot be represented as a 32-bit float"))?;
    if !parsed.is_finite() {
        return Err(format!("prop '{name}' must be finite"));
    }
    Ok(parsed)
}

fn non_empty_json_string(value: &serde_json::Value, key: &str) -> Result<String, String> {
    let value = value
        .get(key)
        .ok_or_else(|| format!("node is missing required '{key}' field"))?
        .as_str()
        .ok_or_else(|| format!("field '{key}' must be a string"))?
        .trim();
    if value.is_empty() {
        Err(format!("field '{key}' must not be empty"))
    } else {
        Ok(value.to_string())
    }
}

fn json_children(value: &serde_json::Value) -> Result<&[serde_json::Value], String> {
    match value.get("children") {
        Some(serde_json::Value::Array(children)) => Ok(children.as_slice()),
        Some(_) => Err("field 'children' must be an array".to_string()),
        None => Ok(&[]),
    }
}

fn validate_desktop_entry_tree_ids(root: &DesktopEntryNode) -> Result<(), String> {
    let mut ids = BTreeSet::new();
    collect_desktop_entry_ids(root, &mut ids)
}

fn collect_desktop_entry_ids<'a>(
    node: &'a DesktopEntryNode,
    ids: &mut BTreeSet<&'a str>,
) -> Result<(), String> {
    if !ids.insert(node.id.as_str()) {
        return Err(format!("duplicate native app node id '{}'", node.id));
    }
    for child in &node.children {
        collect_desktop_entry_ids(child, ids)?;
    }
    Ok(())
}

fn runtime_dimension_to_f32(value: f64) -> Result<f32, Box<CliDiagnostic>> {
    if !value.is_finite() || value <= 0.0 {
        return Err(Box::new(CliDiagnostic::error(
            "desktop.runtime-scene.invalid-dimension",
            "runtime scene dimensions must be finite and greater than zero",
        )));
    }
    let value = value.to_string().parse::<f32>().map_err(|_| {
        Box::new(CliDiagnostic::error(
            "desktop.runtime-scene.invalid-dimension",
            "runtime scene dimension cannot be represented by the layout engine",
        ))
    })?;
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(Box::new(CliDiagnostic::error(
            "desktop.runtime-scene.invalid-dimension",
            "runtime scene dimensions must be finite and greater than zero",
        )))
    }
}

fn runtime_scene_diagnostic(error: &RuntimeSceneError) -> Box<CliDiagnostic> {
    Box::new(CliDiagnostic::error(
        "desktop.runtime-scene.invalid-tree",
        format!("manifest desktop scene could not be constructed: {error:?}"),
    ))
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
    use hawk2ui_build::{
        ArtifactHash, BuildPipeline, CompiledScriptRecord, SealedArtifact, VerificationReport,
    };
    use hawk2ui_layout::Viewport;
    use hawk2ui_runtime::RuntimeSceneBridge;

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

        let config =
            desktop_runtime_config_from_manifest(&manifest, true).expect("runtime config builds");

        assert_eq!(config.window().title, "Desktop Smoke");
        assert_eq!(config.window().metrics.logical_width, 1280.0);
        assert_eq!(config.window().metrics.logical_height, 720.0);
        assert!(config.exit_after_first_frame());

        let scene = RuntimeSceneBridge::new(Viewport::new(1280.0, 720.0))
            .build(
                config
                    .runtime_tree()
                    .expect("desktop config carries runtime tree"),
            )
            .expect("manifest runtime tree builds a scene");
        assert!(
            scene
                .draw_commands()
                .iter()
                .any(|command| matches!(command, hawk2ui_runtime::RuntimeDrawCommand::Fill { .. }))
        );
        assert!(scene.draw_commands().iter().any(|command| matches!(
            command,
            hawk2ui_runtime::RuntimeDrawCommand::Text { text, .. } if text == "Desktop Smoke"
        )));
    }

    #[test]
    fn desktop_runtime_config_uses_compiled_entry_source_result_for_visible_title() {
        let manifest = HawkManifest::parse(
            r#"[identity]
id = "com.example.desktop"
name = "Manifest Only Title"
version = "1.0.0"

[source]
entry = "src/main.ts"

[editor]
width = 640
height = 360

[[targets]]
kind = "desktop"
name = "linux-wayland"
"#,
        )
        .expect("manifest parses");
        let artifact = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest)
            .with_compiled_script(
                CompiledScriptRecord::new(
                    "entry",
                    "src/main.ts",
                    "scripts/entry.hawk.js",
                    ArtifactHash::from_bytes(b"const title: string = 'Entry Driven Title'; title"),
                )
                .with_compiled_source("const title: string = 'Entry Driven Title'; title"),
            );
        let output = BuildWorkspaceOutput {
            manifest,
            pipeline: BuildPipeline::production(),
            artifact,
            verification: VerificationReport::new("com.example.desktop"),
        };

        let config =
            desktop_runtime_config_from_build_output(&output, true).expect("runtime config builds");
        let scene = RuntimeSceneBridge::new(Viewport::new(640.0, 360.0))
            .build(
                config
                    .runtime_tree()
                    .expect("desktop config carries runtime tree"),
            )
            .expect("runtime scene builds");

        assert!(scene.draw_commands().iter().any(|command| matches!(
            command,
            hawk2ui_runtime::RuntimeDrawCommand::Text { text, .. } if text == "Entry Driven Title"
        )));
        assert!(!scene.draw_commands().iter().any(|command| matches!(
            command,
            hawk2ui_runtime::RuntimeDrawCommand::Text { text, .. } if text == "Manifest Only Title"
        )));
    }

    #[test]
    fn desktop_runtime_config_mounts_compiled_entry_app_model() {
        let manifest = HawkManifest::parse(
            r#"[identity]
id = "com.example.desktop"
name = "Manifest Only Title"
version = "1.0.0"

[source]
entry = "src/main.ts"

[editor]
width = 640
height = 360

[[targets]]
kind = "desktop"
name = "linux-wayland"
"#,
        )
        .expect("manifest parses");
        let source = r#"
export function mount(host: { on(name: string, handler: () => void): void; setState(value: object): void }) {
    host.on("click", () => host.setState({ ready: true }));
    return {
        id: "desktop-basic-root",
        text: "Hello From Mount"
    };
}
"#;
        let artifact = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest)
            .with_compiled_script(
                CompiledScriptRecord::new(
                    "entry",
                    "src/main.ts",
                    "scripts/entry.hawk.js",
                    ArtifactHash::from_bytes(source.as_bytes()),
                )
                .with_compiled_source(source),
            );
        let output = BuildWorkspaceOutput {
            manifest,
            pipeline: BuildPipeline::production(),
            artifact,
            verification: VerificationReport::new("com.example.desktop"),
        };

        let config =
            desktop_runtime_config_from_build_output(&output, true).expect("runtime config builds");
        let runtime_tree = config
            .runtime_tree()
            .expect("desktop config carries runtime tree");
        assert_eq!(runtime_tree.root_id().as_str(), "desktop-basic-root");

        let scene = RuntimeSceneBridge::new(Viewport::new(640.0, 360.0))
            .build(runtime_tree)
            .expect("runtime scene builds");

        assert!(scene.draw_commands().iter().any(|command| matches!(
            command,
            hawk2ui_runtime::RuntimeDrawCommand::Text { text, .. } if text == "Hello From Mount"
        )));
        assert!(!scene.draw_commands().iter().any(|command| matches!(
            command,
            hawk2ui_runtime::RuntimeDrawCommand::Text { text, .. } if text == "Manifest Only Title"
        )));
    }

    #[test]
    fn desktop_runtime_config_mounts_nested_native_app_tree() {
        let manifest = HawkManifest::parse(
            r#"[identity]
id = "com.example.desktop"
name = "Manifest Only Title"
version = "1.0.0"

[source]
entry = "src/main.ts"

[editor]
width = 640
height = 360

[[targets]]
kind = "desktop"
name = "linux-wayland"
"#,
        )
        .expect("manifest parses");
        let source = r#"
export function mount(host: { on(name: string, handler: () => void): void; setState(value: object): void }) {
    host.on("click", () => host.setState({ ready: true }));
    return {
        id: "app-root",
        type: "view",
        children: [
            { id: "hero-title", type: "text", text: "Nested Hero" },
            {
                id: "details-panel",
                type: "view",
                children: [
                    { id: "details-copy", type: "text", text: "Nested Detail" }
                ]
            }
        ]
    };
}
"#;
        let artifact = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest)
            .with_compiled_script(
                CompiledScriptRecord::new(
                    "entry",
                    "src/main.ts",
                    "scripts/entry.hawk.js",
                    ArtifactHash::from_bytes(source.as_bytes()),
                )
                .with_compiled_source(source),
            );
        let output = BuildWorkspaceOutput {
            manifest,
            pipeline: BuildPipeline::production(),
            artifact,
            verification: VerificationReport::new("com.example.desktop"),
        };

        let config =
            desktop_runtime_config_from_build_output(&output, true).expect("runtime config builds");
        let runtime_tree = config
            .runtime_tree()
            .expect("desktop config carries runtime tree");
        assert_eq!(runtime_tree.root_id().as_str(), "app-root");

        let scene = RuntimeSceneBridge::new(Viewport::new(640.0, 360.0))
            .build(runtime_tree)
            .expect("runtime scene builds");

        assert!(scene.draw_commands().iter().any(|command| matches!(
            command,
            hawk2ui_runtime::RuntimeDrawCommand::Text { text, .. } if text == "Nested Hero"
        )));
        assert!(scene.draw_commands().iter().any(|command| matches!(
            command,
            hawk2ui_runtime::RuntimeDrawCommand::Text { text, .. } if text == "Nested Detail"
        )));
        assert!(!scene.draw_commands().iter().any(|command| matches!(
            command,
            hawk2ui_runtime::RuntimeDrawCommand::Text { text, .. } if text == "Manifest Only Title"
        )));
    }

    #[test]
    fn desktop_runtime_config_rejects_invalid_native_app_tree() {
        let manifest = HawkManifest::parse(
            r#"[identity]
id = "com.example.desktop"
name = "Manifest Only Title"
version = "1.0.0"

[source]
entry = "src/main.ts"

[editor]
width = 640
height = 360

[[targets]]
kind = "desktop"
name = "linux-wayland"
"#,
        )
        .expect("manifest parses");
        let source = r#"
export function mount() {
    return {
        id: "app-root",
        type: "view",
        children: [
            { id: "duplicate", type: "text", text: "First" },
            { id: "duplicate", type: "text", text: "Second" }
        ]
    };
}
"#;
        let artifact = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest)
            .with_compiled_script(
                CompiledScriptRecord::new(
                    "entry",
                    "src/main.ts",
                    "scripts/entry.hawk.js",
                    ArtifactHash::from_bytes(source.as_bytes()),
                )
                .with_compiled_source(source),
            );
        let output = BuildWorkspaceOutput {
            manifest,
            pipeline: BuildPipeline::production(),
            artifact,
            verification: VerificationReport::new("com.example.desktop"),
        };

        let diagnostic = desktop_runtime_config_from_build_output(&output, true)
            .expect_err("invalid native app tree should fail before runtime");

        assert_eq!(diagnostic.rule, "runtime.desktop.invalid-entry-tree");
        assert!(diagnostic.message.contains("duplicate"));
    }

    #[test]
    fn desktop_runtime_config_applies_native_app_tree_props() {
        let manifest = HawkManifest::parse(
            r#"[identity]
id = "com.example.desktop"
name = "Manifest Only Title"
version = "1.0.0"

[source]
entry = "src/main.ts"

[editor]
width = 640
height = 360

[[targets]]
kind = "desktop"
name = "linux-wayland"
"#,
        )
        .expect("manifest parses");
        let source = r##"
export function mount() {
    return {
        id: "app-root",
        type: "view",
        props: {
            backgroundColor: "#102030",
            padding: 8,
            gap: 4
        },
        children: [
            {
                id: "hero-title",
                type: "text",
                text: "Styled Hero",
                props: {
                    color: "#aabbcc",
                    fontSize: 18,
                    width: 320,
                    height: 40
                }
            }
        ]
    };
}
"##;
        let artifact = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest)
            .with_compiled_script(
                CompiledScriptRecord::new(
                    "entry",
                    "src/main.ts",
                    "scripts/entry.hawk.js",
                    ArtifactHash::from_bytes(source.as_bytes()),
                )
                .with_compiled_source(source),
            );
        let output = BuildWorkspaceOutput {
            manifest,
            pipeline: BuildPipeline::production(),
            artifact,
            verification: VerificationReport::new("com.example.desktop"),
        };

        let config =
            desktop_runtime_config_from_build_output(&output, true).expect("runtime config builds");
        let scene = RuntimeSceneBridge::new(Viewport::new(640.0, 360.0))
            .build(
                config
                    .runtime_tree()
                    .expect("desktop config carries runtime tree"),
            )
            .expect("runtime scene builds");

        assert!(scene.draw_commands().iter().any(|command| matches!(
            command,
            hawk2ui_runtime::RuntimeDrawCommand::Fill { id, color, .. }
                if id.as_str() == "app-root" && *color == Color::rgba(16, 32, 48, 255)
        )));
        assert!(scene.draw_commands().iter().any(|command| matches!(
            command,
            hawk2ui_runtime::RuntimeDrawCommand::Text {
                id,
                geometry,
                text,
                font_size,
                color
            } if id.as_str() == "hero-title"
                && text == "Styled Hero"
                && geometry.width == 320.0
                && geometry.height == 40.0
                && *font_size == 18.0
                && *color == Color::rgba(170, 187, 204, 255)
        )));
    }
}
