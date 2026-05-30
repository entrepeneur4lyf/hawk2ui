//! Filesystem-backed command execution for the `Hawk2UI` CLI.

use std::{
    collections::BTreeSet,
    fmt::Write as _,
    fs,
    path::{Component, Path, PathBuf},
    sync::mpsc,
    time::Duration,
};

use hawk2ui_assets::{AssetBackend, AssetHash, AssetLimits, AssetRecord};
use hawk2ui_build::{
    ArtifactSchemaVersion, ArtifactSignaturePolicy, AssetCompilationError, BuildDiagnostic,
    BuildWorkspace, BuildWorkspaceError, BuildWorkspaceOutput, CompiledScriptRecord, HawkManifest,
    ManifestError, PackageTarget, PinParamIds, SealedArtifact, SealedArtifactError,
    emit_truce_params_struct, pin_param_ids,
};
use hawk2ui_host::{DesktopWindowConfig, SurfaceMetrics};
use hawk2ui_host_winit::{
    WinitDesktopReload, WinitDesktopReloadKind, WinitDesktopRuntime, WinitDesktopRuntimeConfig,
};
use hawk2ui_plugin::{BundleOutput, FormatMetadata};
use hawk2ui_plugin_adapters::{
    PackageAdapterSet, PackageFormat, PackageMaterializationError, PackageRequest,
    VerificationStatus,
};
use hawk2ui_runtime::{EntryNode, RuntimeSceneError, RuntimeViewTree};
use hawk2ui_schema::schema_catalog_json;
use hawk2ui_script::{
    HostCallPolicy, ScriptBackend, ScriptModule, StructuredValue, TimerPolicy,
    entry_mount_bootstrap,
};

use crate::{
    CliCommand, CliDiagnostic, CliExitCode, DevChangeClassifier, DevLoop, DevPatchKind,
    DevPatchPlan, DevWatchKind, DevWatchedPath, FileSystemWatcher, NotifyFileSystemWatcher,
    RecordingReloadTarget, RecordingWatcher,
};

const ARTIFACT_SCHEMA_VERSION: ArtifactSchemaVersion = ArtifactSchemaVersion::new(1, 0);
const MAX_ARTIFACT_CONTAINER_BYTES: u64 = 256 * 1024 * 1024;
const MAX_RUNTIME_ASSET_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BuildProfile {
    Development,
    Production,
}

impl BuildProfile {
    const fn label(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Production => "production",
        }
    }

    const fn output_dir(self) -> &'static str {
        match self {
            Self::Development => "dev",
            Self::Production => "release",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct DesktopEntryAppModel {
    root: EntryNode,
}

impl DesktopEntryAppModel {
    fn manifest_fallback(visible_title: impl Into<String>) -> Self {
        let root_id = "root".to_string();
        Self {
            root: EntryNode::view(
                root_id.clone(),
                vec![EntryNode::text(
                    format!("{root_id}-title"),
                    visible_title.into(),
                )],
            ),
        }
    }

    fn from_mount_json(value: &str) -> Result<Self, String> {
        EntryNode::from_tree_json(value).map(|root| Self { root })
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
    dev_iteration_limit: Option<usize>,
}

impl WorkspaceCommandRunner {
    /// Creates a command runner rooted at a project directory.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            dev_iteration_limit: Some(1),
        }
    }

    /// Configures how many filesystem-backed dev reload cycles should run before returning.
    #[must_use]
    pub const fn with_dev_iteration_limit(mut self, limit: usize) -> Self {
        self.dev_iteration_limit = Some(limit);
        self
    }

    /// Configures the development loop to watch continuously until the process is interrupted.
    #[must_use]
    pub const fn with_unbounded_dev_loop(mut self) -> Self {
        self.dev_iteration_limit = None;
        self
    }

    /// Executes one parsed command.
    #[must_use]
    pub fn execute(&self, command: CliCommand) -> CommandExecution {
        match command {
            CliCommand::NewProject => self.new_project(),
            CliCommand::Run => self.run(),
            CliCommand::Dev => self.dev(),
            CliCommand::Validate => self.validate(),
            CliCommand::BuildDev => self.build(BuildProfile::Development),
            CliCommand::BuildRelease => self.build(BuildProfile::Production),
            CliCommand::VerifyArtifact { path } => self.verify_artifact(path.as_deref()),
            CliCommand::RunDesktop => self.run_desktop(),
            CliCommand::PackagePlugin => self.package_plugin(),
            CliCommand::ExportSchemas => Self::export_schemas(),
            CliCommand::ExportParams => self.export_params(),
            CliCommand::PinIds => self.pin_ids(),
            CliCommand::Diagnostics => self.diagnostics(),
            CliCommand::Explain => self.explain(),
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

        for (relative_path, contents) in default_project_files() {
            let path = self.root.join(relative_path);
            if let Err(error) = write_project_file(&path, contents) {
                return io_failure("project.write-file-failed", &path, &error);
            }
        }
        CommandExecution::success(format!(
            "created Hawk2UI project at {}\n",
            self.root.display()
        ))
    }

    fn validate(&self) -> CommandExecution {
        match self.build_workspace() {
            Ok(output) => {
                let stdout = format!("validated manifest {}\n", output.manifest.identity.id);
                let warnings = unpinned_param_id_warnings(&output.manifest, &self.manifest_path());
                if warnings.is_empty() {
                    CommandExecution::success(stdout)
                } else {
                    let stderr = render_diagnostics(&warnings);
                    CommandExecution {
                        exit_code: CliExitCode::Success,
                        stdout,
                        stderr,
                        diagnostics: warnings,
                    }
                }
            }
            Err(execution) => execution,
        }
    }

    fn run(&self) -> CommandExecution {
        let manifest = match self.validated_manifest() {
            Ok(manifest) => manifest,
            Err(execution) => return execution,
        };
        if manifest.has_target(PackageTarget::Desktop) {
            return self.run_desktop();
        }
        if manifest.has_target(PackageTarget::Plugin) {
            return self.package_plugin();
        }
        CommandExecution::failure(
            CliExitCode::Validation,
            vec![CliDiagnostic::target_incompatibility(
                "run",
                "manifest must declare a desktop or plugin target",
            )],
        )
    }

    fn dev(&self) -> CommandExecution {
        let output = match self.build_workspace() {
            Ok(output) => output,
            Err(execution) => return execution,
        };
        if self.dev_iteration_limit.is_none() && output.manifest.has_target(PackageTarget::Desktop)
        {
            return self.dev_live_desktop(&output);
        }
        let mut watched_paths = dev_watched_paths(&output.manifest);
        let mut classifier = DevChangeClassifier::new(watched_paths.clone());
        let mut file_watcher =
            FileSystemWatcher::new(&self.root, watched_paths.iter().map(DevWatchedPath::path));
        let mut notify_watcher = None;
        if self.dev_iteration_limit.is_none() {
            let _ = file_watcher.changed_files();
            match NotifyFileSystemWatcher::new(
                &self.root,
                watched_paths.iter().map(DevWatchedPath::path),
                Duration::from_millis(75),
            ) {
                Ok(watcher) => notify_watcher = Some(watcher),
                Err(error) => {
                    eprintln!(
                        "native filesystem watcher unavailable: {}; falling back to hash polling",
                        error.message()
                    );
                }
            }
        }
        let mut completed_iterations = 0_usize;
        let mut stdout = String::from("development loop ready\n");

        loop {
            if self
                .dev_iteration_limit
                .is_some_and(|limit| completed_iterations >= limit)
            {
                return CommandExecution::success(stdout);
            }

            let changed_files =
                match Self::dev_changed_files(&mut notify_watcher, &mut file_watcher) {
                    Ok(files) => files,
                    Err(execution) => return execution,
                };
            if changed_files.is_empty() {
                if self.dev_iteration_limit.is_some() {
                    return CommandExecution::success(stdout);
                }
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }

            let output = match self.build_workspace() {
                Ok(output) => output,
                Err(execution) => return execution,
            };
            let patch_plan = classifier.classify(changed_files.clone());
            watched_paths = dev_watched_paths(&output.manifest);
            classifier = DevChangeClassifier::new(watched_paths.clone());
            file_watcher.replace_watched_files(watched_paths.iter().map(DevWatchedPath::path));
            if self.dev_iteration_limit.is_none() {
                notify_watcher = match NotifyFileSystemWatcher::new(
                    &self.root,
                    watched_paths.iter().map(DevWatchedPath::path),
                    Duration::from_millis(75),
                ) {
                    Ok(watcher) => Some(watcher),
                    Err(error) => {
                        eprintln!(
                            "native filesystem watcher unavailable: {}; falling back to hash polling",
                            error.message()
                        );
                        None
                    }
                };
            }

            let mut dev_loop = DevLoop::new(
                RecordingWatcher::new(changed_files),
                RecordingReloadTarget::default(),
            )
            .preserve_state(true);
            match dev_loop.run_once() {
                Ok(report) => {
                    completed_iterations += 1;
                    let _ = writeln!(stdout, "cycle: {completed_iterations}");
                    stdout.push_str(&render_patch_plan(&patch_plan));
                    for event in report.events {
                        stdout.push_str("- ");
                        stdout.push_str(&event_debug(&event));
                        stdout.push('\n');
                    }
                }
                Err(error) => {
                    return CommandExecution::failure(
                        CliExitCode::Runtime,
                        vec![CliDiagnostic::error("dev.loop-failed", error)],
                    );
                }
            }
        }
    }

    fn dev_live_desktop(&self, output: &BuildWorkspaceOutput) -> CommandExecution {
        let config = match desktop_runtime_config_with_assets(&self.root, output, false) {
            Ok(config) => config,
            Err(diagnostic) => {
                return CommandExecution::failure(CliExitCode::Runtime, vec![*diagnostic]);
            }
        };
        let root = self.root.clone();
        let initial_watched_paths = dev_watched_paths(&output.manifest);
        let (reload_sender, reload_receiver) = mpsc::channel();
        println!("development loop ready; native desktop surface attached");
        std::thread::spawn(move || {
            let mut watched_paths = initial_watched_paths;
            let mut classifier = DevChangeClassifier::new(watched_paths.clone());
            let mut file_watcher =
                FileSystemWatcher::new(&root, watched_paths.iter().map(DevWatchedPath::path));
            let _ = file_watcher.changed_files();
            let mut notify_watcher = match NotifyFileSystemWatcher::new(
                &root,
                watched_paths.iter().map(DevWatchedPath::path),
                Duration::from_millis(75),
            ) {
                Ok(watcher) => Some(watcher),
                Err(error) => {
                    eprintln!(
                        "native file watcher unavailable; falling back to hash polling: {}",
                        error.message()
                    );
                    None
                }
            };

            loop {
                let changed_files =
                    match Self::dev_changed_files(&mut notify_watcher, &mut file_watcher) {
                        Ok(files) => files,
                        Err(execution) => {
                            eprintln!("{}", execution.stderr);
                            break;
                        }
                    };
                if changed_files.is_empty() {
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }
                let patch_plan = classifier.classify(changed_files);
                let runner = WorkspaceCommandRunner::new(&root);
                let output = match runner.build_workspace() {
                    Ok(output) => output,
                    Err(execution) => {
                        eprintln!("{}", execution.stderr);
                        continue;
                    }
                };
                let config = match desktop_runtime_config_with_assets(&root, &output, false) {
                    Ok(config) => config,
                    Err(diagnostic) => {
                        eprintln!("{}", render_diagnostics(&[*diagnostic]));
                        continue;
                    }
                };
                let reload_kind = winit_reload_kind(&patch_plan);
                let reload = WinitDesktopReload::new(reload_kind, config)
                    .with_preserve_state(!reload_kind.requires_event_loop_restart());
                if reload_sender.send(reload).is_err() {
                    break;
                }
                eprintln!("dev reload queued: {reload_kind:?}");
                watched_paths = dev_watched_paths(&output.manifest);
                classifier = DevChangeClassifier::new(watched_paths.clone());
                file_watcher.replace_watched_files(watched_paths.iter().map(DevWatchedPath::path));
                notify_watcher = match NotifyFileSystemWatcher::new(
                    &root,
                    watched_paths.iter().map(DevWatchedPath::path),
                    Duration::from_millis(75),
                ) {
                    Ok(watcher) => Some(watcher),
                    Err(error) => {
                        eprintln!(
                            "native file watcher unavailable; falling back to hash polling: {}",
                            error.message()
                        );
                        None
                    }
                };
            }
        });

        match WinitDesktopRuntime::new().run_dev_blocking(config, reload_receiver) {
            Ok(summary) => CommandExecution::success(format!(
                "development loop exited cleanly\nframes-presented: {}\nresizes: {}\ndpi-changes: {}\ninput-events: {}\nnative-reloads: {}\nclose-requested: {}\n",
                summary.frames_presented,
                summary.resizes,
                summary.dpi_changes,
                summary.input_events,
                summary.native_reloads,
                summary.close_requested
            )),
            Err(error) => CommandExecution::failure(
                CliExitCode::Runtime,
                vec![CliDiagnostic::error(error.rule(), error.message())],
            ),
        }
    }

    fn build(&self, profile: BuildProfile) -> CommandExecution {
        let output = match self.build_workspace() {
            Ok(output) => output,
            Err(execution) => return execution,
        };
        if !output.verification.is_release_ready() {
            let diagnostics = output
                .verification
                .diagnostics
                .iter()
                .map(build_diagnostic_to_cli)
                .collect();
            return CommandExecution::failure(CliExitCode::Verification, diagnostics);
        }
        let artifact_path = match self.write_artifact_container(&output, profile) {
            Ok(path) => path,
            Err(execution) => return execution,
        };
        CommandExecution::success(format!(
            "built {} artifact for {}\nartifact-path: {}\nmanifest-hash: {}\ncontent-hash: {}\ncompiled-scripts: {}\ncompiled-styles: {}\ncompiled-assets: {}\nverification-status: release-ready\nsignature-policy: unsigned-development\n",
            profile.label(),
            output.manifest.identity.id,
            artifact_path.display(),
            output.artifact.hashes.manifest.0,
            output.artifact.hashes.content.0,
            output.artifact.compiled_scripts.len(),
            output.artifact.compiled_styles.len(),
            output.artifact.compiled_assets.len()
        ))
    }

    fn dev_changed_files(
        notify_watcher: &mut Option<NotifyFileSystemWatcher>,
        file_watcher: &mut FileSystemWatcher,
    ) -> Result<Vec<String>, CommandExecution> {
        match notify_watcher.as_mut() {
            Some(watcher) => watcher
                .next_changed_files(Duration::from_millis(500))
                .map_err(|error| {
                    CommandExecution::failure(
                        CliExitCode::Runtime,
                        vec![CliDiagnostic::error("dev.watch-failed", error.message())],
                    )
                }),
            None => Ok(file_watcher.changed_files()),
        }
    }

    fn verify_artifact(&self, path: Option<&str>) -> CommandExecution {
        let artifact_path = path.map_or_else(
            || self.artifact_output_path(BuildProfile::Production),
            PathBuf::from,
        );
        let bytes = match read_artifact_container_bytes(&artifact_path) {
            Ok(bytes) => bytes,
            Err(error) => return error,
        };
        let artifact = match SealedArtifact::from_container_bytes(
            &bytes,
            ARTIFACT_SCHEMA_VERSION,
            ArtifactSignaturePolicy::AllowUnsignedDevelopment,
        ) {
            Ok(artifact) => artifact,
            Err(error) => {
                return CommandExecution::failure(
                    CliExitCode::Verification,
                    vec![
                        sealed_artifact_error_diagnostic(error)
                            .file(artifact_path.display().to_string()),
                    ],
                );
            }
        };
        CommandExecution::success(format!(
            "verified artifact container\npath: {}\ncontent-hash: {}\nsignature-status: {}\n",
            artifact_path.display(),
            artifact.hashes.content.0,
            artifact_signature_status(&artifact),
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
        let config =
            match desktop_runtime_config_with_assets(&self.root, &output, exit_after_first_frame) {
                Ok(config) => config,
                Err(diagnostic) => {
                    return CommandExecution::failure(CliExitCode::Runtime, vec![*diagnostic]);
                }
            };
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

        // The manifest is already validated, so its parameter model is valid by
        // construction (kinds, ranges, units, and defaults all checked at parse).
        let parameters = manifest.parameter_model();
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
                        CliDiagnostic::error(
                            diagnostic.rule(),
                            format!("plugin package planning failed: {}", diagnostic.message()),
                        )
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
        let mut stdout = String::from("materialized plugin package layouts:\n");
        for target in &outputs {
            stdout.push_str("- ");
            stdout.push_str(&target.output_path);
            stdout.push('\n');
        }
        stdout.push_str("layout-verification-status: passed\n");
        stdout.push_str("host-loadable-binaries: not-produced-by-this-command\n");
        CommandExecution::success(stdout)
    }

    fn diagnostics(&self) -> CommandExecution {
        match self.build_workspace() {
            Ok(_) => CommandExecution::success("no diagnostics\n"),
            Err(execution) => execution,
        }
    }

    fn explain(&self) -> CommandExecution {
        match self.build_workspace() {
            Ok(output) => {
                let manifest = output.manifest;
                let mut stdout = format!(
                    "project: {}\nname: {}\nversion: {}\n",
                    manifest.identity.id, manifest.identity.name, manifest.identity.version
                );
                stdout.push_str("targets:\n");
                for target in &manifest.targets {
                    stdout.push_str("- ");
                    stdout.push_str(&target.name);
                    stdout.push_str(" (");
                    stdout.push_str(match target.kind {
                        PackageTarget::Desktop => "desktop",
                        PackageTarget::Plugin => "plugin",
                    });
                    stdout.push_str(")\n");
                }
                stdout.push_str("capabilities:\n");
                for capability in &manifest.capabilities {
                    stdout.push_str("- ");
                    stdout.push_str(capability);
                    stdout.push('\n');
                }
                stdout.push_str("next commands:\n");
                stdout.push_str("- hawk2ui validate\n");
                stdout.push_str("- hawk2ui build-release\n");
                if manifest.has_target(PackageTarget::Desktop) {
                    stdout.push_str("- hawk2ui run-desktop\n");
                    stdout.push_str("- hawk2ui dev\n");
                }
                if manifest.has_target(PackageTarget::Plugin) {
                    stdout.push_str("- hawk2ui package-plugin\n");
                }
                CommandExecution::success(stdout)
            }
            Err(execution) => execution,
        }
    }

    fn export_schemas() -> CommandExecution {
        match schema_catalog_json()
            .and_then(|catalog| {
                serde_json::to_string_pretty(&catalog).map_err(|error| {
                    hawk2ui_schema::SchemaValidationError::new(
                        "schema.catalog.render-failed",
                        format!("schema catalog could not be rendered: {error}"),
                    )
                })
            })
            .map(|mut catalog| {
                catalog.push('\n');
                catalog
            }) {
            Ok(catalog) => CommandExecution::success(catalog),
            Err(error) => CommandExecution::failure(
                CliExitCode::Validation,
                vec![CliDiagnostic::error(error.rule(), error.message())],
            ),
        }
    }

    /// Emits the truce `#[derive(Params)]` source generated from the project's
    /// manifest parameters to stdout. This is the build-time codegen seam the
    /// plugin packaging path will compile into the DSP crate; here it is exposed
    /// for inspection, mirroring `export-schemas`.
    fn export_params(&self) -> CommandExecution {
        // Parse the manifest directly rather than building the whole workspace:
        // previewing parameter codegen should not require the app's sources to
        // compile first.
        let manifest_path = self.manifest_path();
        let source = match fs::read_to_string(&manifest_path) {
            Ok(source) => source,
            Err(error) => {
                return CommandExecution::failure(
                    CliExitCode::Validation,
                    vec![
                        CliDiagnostic::error(
                            "manifest.read-failed",
                            format!("could not read manifest: {error}"),
                        )
                        .file(manifest_path.display().to_string()),
                    ],
                );
            }
        };
        let manifest = match HawkManifest::parse(&source) {
            Ok(manifest) => manifest,
            Err(error) => {
                return CommandExecution::failure(
                    CliExitCode::Validation,
                    vec![manifest_error_diagnostic(error)],
                );
            }
        };
        let model = manifest.parameter_model();
        if model.parameters.is_empty() {
            return CommandExecution::failure(
                CliExitCode::Validation,
                vec![CliDiagnostic::error(
                    "params.export.empty",
                    "no parameters declared; truce requires at least one parameter to derive Params",
                )],
            );
        }
        CommandExecution::success(emit_truce_params_struct("PluginParams", &model))
    }

    fn pin_ids(&self) -> CommandExecution {
        let manifest_path = self.manifest_path();
        let source = match fs::read_to_string(&manifest_path) {
            Ok(source) => source,
            Err(error) => return io_failure("manifest.read-failed", &manifest_path, &error),
        };
        match pin_param_ids(&source) {
            Ok(PinParamIds::Unchanged) => CommandExecution::success(
                "all parameters already have a pinned param_id; manifest unchanged\n",
            ),
            Ok(PinParamIds::Pinned { source, assigned }) => {
                if let Err(error) = fs::write(&manifest_path, source) {
                    return io_failure("manifest.write-failed", &manifest_path, &error);
                }
                let mut message = format!("pinned {} parameter id(s):\n", assigned.len());
                for (id, param_id) in &assigned {
                    let _ = writeln!(message, "  {id} = {param_id}");
                }
                CommandExecution::success(message)
            }
            Err(error) => CommandExecution::failure(
                CliExitCode::Validation,
                vec![manifest_error_diagnostic(error)],
            ),
        }
    }

    fn validated_manifest(&self) -> Result<HawkManifest, CommandExecution> {
        self.build_workspace().map(|output| output.manifest)
    }

    fn build_workspace(&self) -> Result<BuildWorkspaceOutput, CommandExecution> {
        BuildWorkspace::load(&self.root)
            .and_then(|workspace| workspace.build(ARTIFACT_SCHEMA_VERSION))
            .map_err(|error| {
                CommandExecution::failure(
                    CliExitCode::Validation,
                    vec![build_workspace_error_diagnostic(error, &self.root)],
                )
            })
    }

    fn write_artifact_container(
        &self,
        output: &BuildWorkspaceOutput,
        profile: BuildProfile,
    ) -> Result<PathBuf, CommandExecution> {
        let artifact_path = self.artifact_output_path(profile);
        let bytes = output
            .artifact
            .to_container_bytes(ArtifactSignaturePolicy::AllowUnsignedDevelopment)
            .map_err(|error| {
                CommandExecution::failure(
                    CliExitCode::Verification,
                    vec![sealed_artifact_error_diagnostic(error)],
                )
            })?;
        if let Some(parent) = artifact_path.parent()
            && let Err(error) = fs::create_dir_all(parent)
        {
            return Err(io_failure(
                "artifact.output.create-dir-failed",
                parent,
                &error,
            ));
        }
        fs::write(&artifact_path, bytes)
            .map_err(|error| io_failure("artifact.output.write-failed", &artifact_path, &error))?;
        Ok(artifact_path)
    }

    fn artifact_output_path(&self, profile: BuildProfile) -> PathBuf {
        self.root
            .join("target/hawk2ui")
            .join(profile.output_dir())
            .join("hawk2ui-artifact.hawk")
    }

    fn manifest_path(&self) -> PathBuf {
        self.root.join("manifest.hawk.toml")
    }
}

fn read_artifact_container_bytes(path: &Path) -> Result<Vec<u8>, CommandExecution> {
    let metadata = fs::metadata(path)
        .map_err(|error| io_failure("artifact.container.read-failed", path, &error))?;
    if metadata.len() > MAX_ARTIFACT_CONTAINER_BYTES {
        return Err(CommandExecution::failure(
            CliExitCode::Verification,
            vec![
                 CliDiagnostic::error(
                     "artifact.container.too-large",
                     format!(
                         "sealed artifact container exceeds maximum supported size of {MAX_ARTIFACT_CONTAINER_BYTES} bytes"
                     ),
                 )
                .file(path.display().to_string()),
            ],
        ));
    }
    fs::read(path).map_err(|error| io_failure("artifact.container.read-failed", path, &error))
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
        ManifestError::DuplicateParameter(parameter) => CliDiagnostic::error(
            "manifest.parameter.duplicate",
            format!("duplicate parameter declaration: {parameter}"),
        ),
        ManifestError::DuplicateParameterId { id, param_id } => CliDiagnostic::error(
            "manifest.parameter.param-id-duplicate",
            format!(
                "parameter {id} pins numeric id {param_id}, which another parameter already pins"
            ),
        ),
        ManifestError::ReservedParameterId { id, param_id } => CliDiagnostic::error(
            "manifest.parameter.param-id-reserved",
            format!(
                "parameter {id} pins numeric id {param_id}, which is in truce's reserved meter range (>= 2^24)"
            ),
        ),
        ManifestError::DuplicateMeter(meter) => CliDiagnostic::error(
            "manifest.meter.duplicate",
            format!("duplicate meter declaration (parameter/meter id namespace): {meter}"),
        ),
        ManifestError::InvalidCapability(capability) => CliDiagnostic::error(
            "manifest.capability.invalid",
            format!("invalid capability key: {capability}"),
        ),
        ManifestError::InvalidPluginMetadata(message) => {
            CliDiagnostic::error("manifest.plugin.invalid", message)
        }
        ManifestError::InvalidPluginParameter(parameter) => CliDiagnostic::error(
            "manifest.parameter.invalid",
            format!("invalid plugin parameter declaration: {parameter}"),
        ),
        ManifestError::InvalidPluginMeter(meter) => CliDiagnostic::error(
            "manifest.meter.invalid",
            format!("invalid plugin meter declaration: {meter}"),
        ),
        ManifestError::InvalidEnumVariant(variant) => CliDiagnostic::error(
            "manifest.parameter.enum.variant-invalid",
            format!("invalid enum parameter variant: {variant}"),
        ),
        ManifestError::CollidingEnumVariant(variant) => CliDiagnostic::error(
            "manifest.parameter.enum.variant-collision",
            format!(
                "enum parameter variant id collides with another (same generated identifier): {variant}"
            ),
        ),
        ManifestError::CollidingFieldIdentifier(id) => CliDiagnostic::error(
            "manifest.field-collision",
            format!(
                "parameter/meter id derives the same generated struct field as another (parameter/meter id namespace): {id}"
            ),
        ),
        ManifestError::CollidingEnumType(parameter) => CliDiagnostic::error(
            "manifest.parameter.enum.type-collision",
            format!(
                "enum parameter id derives the same generated ParamEnum type as another: {parameter}"
            ),
        ),
        ManifestError::SchemaValidation { path, message } => CliDiagnostic::error(
            "manifest.schema.invalid",
            format!(
                "manifest does not match the generated Hawk2UI manifest schema at {path}: {message}"
            ),
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
        BuildWorkspaceError::FileTooLarge(path) => CliDiagnostic::error(
            "build.file-too-large",
            format!("declared build file exceeds the maximum supported size: {path}"),
        )
        .file(root.join(path).display().to_string()),
        BuildWorkspaceError::UnsafePath(path) => CliDiagnostic::error(
            "build.path.unsafe",
            format!("declared build path escapes the workspace: {path}"),
        ),
        BuildWorkspaceError::ManifestInvalid(error) => manifest_error_diagnostic(error)
            .file(root.join("manifest.hawk.toml").display().to_string()),
        BuildWorkspaceError::AssetCompilation(error) => asset_compilation_diagnostic(error, root),
        BuildWorkspaceError::ScriptCompilation { path, error } => CliDiagnostic::error(
            error.diagnostic().rule(),
            format!(
                "declared script failed production compilation: {}",
                error.diagnostic().message()
            ),
        )
        .file(root.join(path).display().to_string()),
        BuildWorkspaceError::UnsupportedScriptExtension(path) => CliDiagnostic::error(
            "build.script.unsupported-extension",
            format!("declared script file extension is not supported: {path}"),
        )
        .file(root.join(path).display().to_string()),
        BuildWorkspaceError::StyleCompilation { path, error } => {
            let diagnostic = error.diagnostics().first();
            CliDiagnostic::error(
                diagnostic.map_or("build.style.compile-failed", |diagnostic| diagnostic.rule()),
                diagnostic.map_or_else(
                    || "declared style failed production compilation".to_string(),
                    |diagnostic| {
                        format!(
                            "declared style failed production compilation: {}",
                            diagnostic.message()
                        )
                    },
                ),
            )
            .file(root.join(path).display().to_string())
        }
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
        format!(
            "plugin package materialization failed: {}",
            error.diagnostic().message()
        ),
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

fn build_diagnostic_to_cli(diagnostic: &BuildDiagnostic) -> CliDiagnostic {
    let mut cli = CliDiagnostic::error(&diagnostic.rule, &diagnostic.message);
    if let Some(location) = &diagnostic.location {
        cli = cli.file(location.file_path.clone());
    }
    cli
}

fn sealed_artifact_error_diagnostic(error: SealedArtifactError) -> CliDiagnostic {
    let diagnostic = match error {
        SealedArtifactError::IncompatibleSchema { diagnostic, .. }
        | SealedArtifactError::SchemaGeneration { diagnostic }
        | SealedArtifactError::SchemaValidation { diagnostic }
        | SealedArtifactError::ContainerSerialization { diagnostic }
        | SealedArtifactError::ContainerVerification { diagnostic }
        | SealedArtifactError::SignaturePolicy { diagnostic }
        | SealedArtifactError::SignatureVerification { diagnostic } => diagnostic,
    };
    CliDiagnostic::error(diagnostic.rule, diagnostic.message)
}

fn artifact_signature_status(artifact: &SealedArtifact) -> &'static str {
    match artifact.signature.status {
        hawk2ui_build::ArtifactSignatureStatus::Unsigned => "unsigned-development",
        hawk2ui_build::ArtifactSignatureStatus::Verified => "verified",
    }
}

fn write_project_file(path: &Path, contents: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}

fn render_diagnostics(diagnostics: &[CliDiagnostic]) -> String {
    diagnostics
        .iter()
        .map(CliDiagnostic::render)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// Builds a non-fatal warning when any manifest parameter has no pinned
/// `param_id`. An unpinned id is positional, so reordering the manifest
/// renumbers it and breaks saved automation, presets, and state — `hawk2ui
/// pin-ids` fixes it. Returns an empty vec when every parameter is pinned.
fn unpinned_param_id_warnings(manifest: &HawkManifest, manifest_path: &Path) -> Vec<CliDiagnostic> {
    let unpinned: Vec<&str> = manifest
        .parameters
        .iter()
        .filter(|parameter| parameter.param_id.is_none())
        .map(|parameter| parameter.id.as_str())
        .collect();
    if unpinned.is_empty() {
        return Vec::new();
    }
    vec![
        CliDiagnostic::warning(
            "manifest.parameter.param-id-unpinned",
            format!(
                "{} parameter(s) have no pinned param_id ({}); reordering the manifest will renumber them and break saved automation, presets, and state",
                unpinned.len(),
                unpinned.join(", ")
            ),
        )
        .file(manifest_path.display().to_string())
        .suggested_fix("run `hawk2ui pin-ids` to pin them"),
    ]
}

#[cfg(test)]
mod param_id_warning_tests {
    use super::*;

    #[test]
    fn validate_warning_names_only_unpinned_parameters() {
        const UNPINNED: &str = r#"
[identity]
id = "com.hawk2ui.warn"
name = "Warn"
version = "0.1.0"

[source]
entry = "src/main.ts"

[plugin]
id = "com.hawk2ui.warn"
name = "Warn"

[[parameters]]
id = "gain"
name = "Gain"
default = 0.5

[[parameters]]
id = "mix"
name = "Mix"
param_id = 1
default = 0.5
"#;
        const ALL_PINNED: &str = r#"
[identity]
id = "com.hawk2ui.warn"
name = "Warn"
version = "0.1.0"

[source]
entry = "src/main.ts"

[plugin]
id = "com.hawk2ui.warn"
name = "Warn"

[[parameters]]
id = "gain"
name = "Gain"
param_id = 0
default = 0.5
"#;

        let manifest = HawkManifest::parse(UNPINNED).expect("manifest parses");
        let warnings = unpinned_param_id_warnings(&manifest, Path::new("manifest.hawk.toml"));
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].rule, "manifest.parameter.param-id-unpinned");
        assert!(
            warnings[0].message.contains("gain"),
            "{}",
            warnings[0].message
        );
        assert!(
            !warnings[0].message.contains("mix"),
            "a pinned parameter must not be warned about"
        );

        let pinned = HawkManifest::parse(ALL_PINNED).expect("pinned manifest parses");
        assert!(unpinned_param_id_warnings(&pinned, Path::new("m")).is_empty());
    }
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

fn event_debug(event: &crate::DevLoopEvent) -> String {
    format!("{event:?}")
}

fn dev_watched_paths(manifest: &HawkManifest) -> Vec<DevWatchedPath> {
    let mut paths = BTreeSet::from([
        DevWatchedPath::new("manifest.hawk.toml", DevWatchKind::Manifest),
        DevWatchedPath::new(manifest.source.entry.clone(), DevWatchKind::RuntimeTree),
    ]);
    if let Some(style) = &manifest.source.style {
        paths.insert(DevWatchedPath::new(style.clone(), DevWatchKind::Style));
    }
    if let Some(script) = &manifest.source.script {
        paths.insert(DevWatchedPath::new(script.clone(), DevWatchKind::Script));
    }
    for asset in &manifest.assets {
        paths.insert(DevWatchedPath::new(asset.path.clone(), DevWatchKind::Asset));
    }
    paths.into_iter().collect()
}

fn render_patch_plan(plan: &DevPatchPlan) -> String {
    let mut output = format!("patch: {:?}\n", plan.kind());
    for file in plan.changed_files() {
        output.push_str("changed: ");
        output.push_str(file);
        output.push('\n');
    }
    output
}

fn winit_reload_kind(plan: &DevPatchPlan) -> WinitDesktopReloadKind {
    match plan.kind() {
        DevPatchKind::StylePatch => WinitDesktopReloadKind::StylePatch,
        DevPatchKind::AssetPatch => WinitDesktopReloadKind::AssetPatch,
        DevPatchKind::RuntimeTreePatch => WinitDesktopReloadKind::RuntimeTreePatch,
        DevPatchKind::ScriptRebuild => WinitDesktopReloadKind::ScriptRebuild,
        DevPatchKind::FullRebuildRequired => WinitDesktopReloadKind::FullRebuildRequired,
    }
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

fn desktop_runtime_config_with_assets(
    root: &Path,
    output: &BuildWorkspaceOutput,
    exit_after_first_frame: bool,
) -> Result<WinitDesktopRuntimeConfig, Box<CliDiagnostic>> {
    let config = desktop_runtime_config_from_build_output(output, exit_after_first_frame)?;
    let runtime_assets = desktop_runtime_assets(root, &output.manifest)?;
    Ok(config.with_runtime_assets(runtime_assets))
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
    let metadata = fs::metadata(&resolved).map_err(|error| {
        Box::new(CliDiagnostic::error(
            "asset.read-failed",
            format!("failed to inspect runtime asset {path}: {error}"),
        ))
    })?;
    if metadata.len() > MAX_RUNTIME_ASSET_BYTES {
        return Err(Box::new(CliDiagnostic::error(
            "asset.too-large",
            format!(
                "runtime asset {path} exceeds maximum supported size of {MAX_RUNTIME_ASSET_BYTES} bytes"
            ),
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
    app_model
        .root
        .to_view_tree(width, height)
        .map_err(|error| runtime_scene_diagnostic(&error))
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
    let Some(source) = entry_mount_bootstrap(script.compiled_source.as_str()) else {
        return Ok(None);
    };
    let mut backend = ScriptBackend::new(HostCallPolicy::deny_all(), TimerPolicy::deterministic());
    let execution = backend
        .execute_module(ScriptModule::for_source_path(
            &script.source_path,
            source.as_str(),
        ))
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
        .execute_module(ScriptModule::for_source_path(
            &script.source_path,
            script.compiled_source.as_str(),
        ))
        .ok()?;
    match execution.value() {
        StructuredValue::String(value) if !value.trim().is_empty() => Some(value.clone()),
        StructuredValue::Null
        | StructuredValue::Bool(_)
        | StructuredValue::Number(_)
        | StructuredValue::String(_) => None,
    }
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

fn runtime_dimension_to_f32(value: f64) -> Result<f32, Box<CliDiagnostic>> {
    if !value.is_finite() || value <= 0.0 {
        return Err(Box::new(CliDiagnostic::error(
            "desktop.runtime-scene.invalid-dimension",
            "runtime scene dimensions must be finite and greater than zero",
        )));
    }
    #[allow(clippy::cast_possible_truncation)]
    let value = value as f32;
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

fn default_project_files() -> [(&'static str, &'static str); 6] {
    [
        ("manifest.hawk.toml", default_manifest()),
        ("src/main.ts", default_entry_source()),
        ("src/bootstrap.ts", default_bootstrap_source()),
        ("styles/main.hawk.css", default_style_source()),
        ("assets/logo.svg", default_logo_svg()),
        ("README.md", default_project_readme()),
    ]
}

fn default_manifest() -> &'static str {
    r#"[identity]
id = "com.example.hawk2ui-app"
name = "Hawk2UI App"
version = "0.1.0"

[package]
name = "hawk2ui-app"
bundle_id = "com.example.hawk2ui-app"

[source]
entry = "src/main.ts"
style = "styles/main.hawk.css"
script = "src/bootstrap.ts"

[capabilities]
keys = ["native-windowing", "sealed-artifacts"]

[plugin]
id = "com.example.hawk2ui-app"
name = "Hawk2UI App"

[editor]
width = 960
height = 540

[[parameters]]
id = "gain"
name = "Gain"
param_id = 0
default = 0.5

[[parameters]]
id = "mix"
name = "Mix"
param_id = 1
default = 0.75

[[assets]]
id = "logo"
kind = "vector"
path = "assets/logo.svg"

[[presets]]
id = "init"
name = "Init"

[[targets]]
kind = "desktop"
name = "linux-wayland"

[[targets]]
kind = "plugin"
name = "clap"
"#
}

fn default_entry_source() -> &'static str {
    r##"export function mount(host) {
    host.on("ready", () => {});
    return {
        id: "root",
        type: "view",
        props: {
            backgroundColor: "#080b10",
            width: 960,
            height: 540,
            padding: 32,
            gap: 18
        },
        children: [
            {
                id: "title",
                type: "text",
                text: "Hawk2UI Native Surface",
                props: {
                    textColor: "#f8fafc",
                    fontSize: 28,
                    width: 720,
                    height: 44
                }
            },
            {
                id: "subtitle",
                type: "text",
                text: "Desktop and plugin-ready scaffold with typed source, styles, assets, and parameters.",
                props: {
                    textColor: "#a9b4c7",
                    fontSize: 18,
                    width: 820,
                    height: 34
                }
            },
            {
                id: "panel",
                type: "view",
                props: {
                    backgroundColor: "#131a24",
                    width: 720,
                    height: 160,
                    padding: 22,
                    gap: 12
                },
                children: [
                    {
                        id: "panel-heading",
                        type: "text",
                        text: "Production scaffold",
                        props: {
                            textColor: "#ffffff",
                            fontSize: 22,
                            width: 420,
                            height: 34
                        }
                    },
                    {
                        id: "panel-copy",
                        type: "text",
                        text: "Run validate, build-release, run-desktop, or package-plugin from this project directory.",
                        props: {
                            textColor: "#d6deea",
                            fontSize: 16,
                            width: 640,
                            height: 32
                        }
                    }
                ]
            }
        ]
    };
}
"##
}

fn default_bootstrap_source() -> &'static str {
    r#"export const hawk2uiTemplate = {
    surface: "native-window",
    renderer: "skia",
    pluginFormats: ["clap"],
};
"#
}

fn default_style_source() -> &'static str {
    r".root {
    display: flex;
    font-size: 18px;
    background-color: token(color.surface);
    color: #f8fafc;
}

.panel {
    display: flex;
    background-color: token(color.surface);
    color: #d6deea;
}
"
}

fn default_logo_svg() -> &'static str {
    r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 96">
    <path d="M12 70L28 18L44 70L56 34L68 70L84 18" fill="none" stroke="#7dd3fc" stroke-width="8" stroke-linecap="round" stroke-linejoin="round"/>
    <path d="M20 78H76" fill="none" stroke="#f8fafc" stroke-width="6" stroke-linecap="round"/>
</svg>
"##
}

fn default_project_readme() -> &'static str {
    r"# Hawk2UI App

Generated project scaffold.

Commands:

- `hawk2ui validate`
- `hawk2ui build-release`
- `hawk2ui run-desktop`
- `hawk2ui package-plugin`

The starter manifest includes a desktop target, a CLAP plugin target, TypeScript entrypoints,
CSS, an SVG asset, plugin parameters, and an initial preset.
"
}

#[cfg(test)]
mod tests {
    use super::*;
    use hawk2ui_build::{
        ArtifactHash, BuildPipeline, CompiledScriptRecord, SealedArtifact, VerificationReport,
    };
    use hawk2ui_layout::Viewport;
    use hawk2ui_render::Color;
    use hawk2ui_runtime::RuntimeSceneBridge;

    fn float_eq(left: f32, right: f32) -> bool {
        (left - right).abs() < f32::EPSILON
    }

    fn float_eq_f64(left: f64, right: f64) -> bool {
        (left - right).abs() < f64::EPSILON
    }

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
        assert!(float_eq_f64(config.window().metrics.logical_width, 1280.0));
        assert!(float_eq_f64(config.window().metrics.logical_height, 720.0));
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
                && float_eq(geometry.width, 320.0)
                && float_eq(geometry.height, 40.0)
                && float_eq(*font_size, 18.0)
                && *color == Color::rgba(170, 187, 204, 255)
        )));
    }
}
