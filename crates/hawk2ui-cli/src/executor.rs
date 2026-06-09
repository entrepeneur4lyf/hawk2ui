//! Filesystem-backed command execution for the `Hawk2UI` CLI.

use std::{
    collections::BTreeSet,
    env,
    fmt::Write as _,
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
    sync::mpsc,
    time::Duration,
};

use hawk2ui_assets::{AssetBackend, AssetHash, AssetLimits, AssetRecord};
use hawk2ui_authoring::{
    FrameworkDynamicBinding, FrameworkNativeProgram, FrameworkNativeProgramWire,
    NativeRuntimeBridge, NativeRuntimeBridgeArtifact,
};
use hawk2ui_build::{
    ArtifactSchemaVersion, ArtifactSignaturePolicy, ArtifactSignatureVerificationKey,
    ArtifactSignatureVerifier, ArtifactSigningKey, AssetCompilationError, BuildDiagnostic,
    BuildWorkspace, BuildWorkspaceError, BuildWorkspaceOutput, CompiledFrameworkRecord,
    CompiledScriptRecord, HawkManifest, ManifestError, PackageTarget, PinParamIds, SealedArtifact,
    SealedArtifactError, SourceFramework, emit_truce_params_struct, migrate_toml_manifest_to_json,
    pin_param_ids,
};
use hawk2ui_host::{DesktopWindowConfig, SurfaceMetrics};
use hawk2ui_host_winit::{
    WinitDesktopReload, WinitDesktopReloadKind, WinitDesktopRuntime, WinitDesktopRuntimeConfig,
    WinitDesktopRuntimeSummary, WinitDesktopScriptEntry, WinitPresentationBackend,
};
use hawk2ui_plugin::{BundleOutput, FormatMetadata, PluginEditor, PluginEditorSize};
use hawk2ui_plugin_adapters::{
    MaterializedPackageOutput, PackageAdapterSet, PackageFormat, PackageMaterializationError,
    PackageRequest, VerificationReport as PackageVerificationReport, VerificationStatus,
};
use hawk2ui_runtime::{EntryNode, RuntimeSceneError, RuntimeViewTree};
use hawk2ui_schema::schema_catalog_json;
use hawk2ui_script::{
    FrameworkRuntimeController, HostCallPolicy, HostSnapshot, ScriptBackend, ScriptModule,
    StructuredValue, TimerPolicy, entry_mount_bootstrap,
};
use hawk2ui_security_model::{PackageTrustRecord, PackageTrustValidator, VerificationReportStatus};

use crate::{
    CliCommand, CliDiagnostic, CliExitCode, CliPackageManager, CliPresentationBackend,
    CliProjectTemplate, DevChangeBatch, DevChangeClassifier, DevLoop, DevPatchKind, DevPatchPlan,
    DevWatchKind, DevWatchedPath, FileSystemWatcher, NotifyFileSystemWatcher,
};

const ARTIFACT_SCHEMA_VERSION: ArtifactSchemaVersion = ArtifactSchemaVersion::new(1, 0);
const MAX_ARTIFACT_CONTAINER_BYTES: u64 = 256 * 1024 * 1024;
const MAX_RUNTIME_ASSET_BYTES: u64 = 64 * 1024 * 1024;
const RELEASE_SIGNING_KEY_ID_ENV: &str = "HAWK2UI_RELEASE_SIGNING_KEY_ID";
const RELEASE_SIGNING_KEY_HEX_ENV: &str = "HAWK2UI_RELEASE_SIGNING_KEY_HEX";
const TRUSTED_RELEASE_KEYS_ENV: &str = "HAWK2UI_TRUSTED_RELEASE_KEYS";
const DESKTOP_LAUNCHER_BINARY_ENV: &str = "HAWK2UI_DESKTOP_LAUNCHER_BINARY";
const CANONICAL_MANIFEST_FILE: &str = "hawk.json";
const LEGACY_MANIFEST_FILE: &str = "manifest.hawk.toml";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BuildProfile {
    Development,
    Production,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DesktopPackageOutput {
    package_root: PathBuf,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct PackagedDesktopRuntimeDescriptor {
    manifest_source: String,
    artifact: SealedArtifact,
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

/// Runs a packaged desktop app using the descriptor located next to the generated launcher.
///
/// # Errors
///
/// Returns a rendered diagnostic string when the descriptor, signed artifact, runtime config, or
/// native desktop host fails.
pub fn run_packaged_desktop_from_default_location() -> Result<(), String> {
    let executable = env::current_exe()
        .map_err(|error| format!("package.desktop.current-exe-failed: {error}"))?;
    let usr_dir = executable.parent().and_then(Path::parent).ok_or_else(|| {
        format!(
            "package.desktop.layout-invalid: launcher path {} is not inside usr/bin",
            executable.display()
        )
    })?;
    let descriptor_path = usr_dir
        .join("share")
        .join("hawk2ui")
        .join("hawk2ui-desktop-runtime.json");
    run_packaged_desktop_from_descriptor_path(descriptor_path).map(|_| ())
}

/// Runs a packaged desktop app from an explicit runtime descriptor path.
///
/// # Errors
///
/// Returns a rendered diagnostic string when the descriptor, signed artifact, runtime config, or
/// native desktop host fails.
pub fn run_packaged_desktop_from_descriptor_path(
    descriptor_path: impl AsRef<Path>,
) -> Result<WinitDesktopRuntimeSummary, String> {
    let descriptor_path = descriptor_path.as_ref();
    let descriptor_source = fs::read_to_string(descriptor_path).map_err(|error| {
        format!(
            "package.desktop.descriptor-read-failed: failed to read {}: {error}",
            descriptor_path.display()
        )
    })?;
    let descriptor: PackagedDesktopRuntimeDescriptor = serde_json::from_str(&descriptor_source)
        .map_err(|error| {
            format!(
                "package.desktop.descriptor-parse-failed: failed to parse {}: {error}",
                descriptor_path.display()
            )
        })?;
    descriptor
        .artifact
        .ensure_signature_policy(ArtifactSignaturePolicy::RequireVerifiedSignature)
        .map_err(|error| sealed_artifact_error_diagnostic(error).render())?;
    let manifest = HawkManifest::parse(&descriptor.manifest_source)
        .map_err(|error| manifest_error_diagnostic(error).render())?;
    let exit_after_first_frame = env::var_os("HAWK2UI_EXIT_AFTER_FIRST_FRAME").is_some();
    let mut config = desktop_runtime_config_from_manifest_and_artifact(
        &manifest,
        &descriptor.artifact,
        exit_after_first_frame,
    )
    .map_err(|diagnostic| diagnostic.render())?;
    let descriptor_dir = descriptor_path.parent().ok_or_else(|| {
        format!(
            "package.desktop.descriptor-layout-invalid: descriptor {} has no parent directory",
            descriptor_path.display()
        )
    })?;
    let runtime_assets = desktop_runtime_assets(&descriptor_dir.join("workspace"), &manifest)
        .map_err(|diagnostic| diagnostic.render())?;
    config = config.with_runtime_assets(runtime_assets);
    WinitDesktopRuntime::new()
        .run_blocking(config)
        .map_err(|error| format!("{}: {}", error.rule(), error.message()))
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
    release_signing_key: Option<ArtifactSigningKey>,
    trusted_release_keys: Vec<ArtifactSignatureVerificationKey>,
    desktop_exit_after_first_frame: bool,
    desktop_launcher_binary: Option<PathBuf>,
}

impl WorkspaceCommandRunner {
    /// Creates a command runner rooted at a project directory.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            dev_iteration_limit: Some(1),
            release_signing_key: None,
            trusted_release_keys: Vec::new(),
            desktop_exit_after_first_frame: false,
            desktop_launcher_binary: None,
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

    /// Configures desktop runs to exit after presenting the first frame.
    #[must_use]
    pub const fn with_desktop_exit_after_first_frame(mut self) -> Self {
        self.desktop_exit_after_first_frame = true;
        self
    }

    /// Uses an already-built native launcher binary when packaging desktop targets.
    #[must_use]
    pub fn with_desktop_launcher_binary(mut self, launcher: impl Into<PathBuf>) -> Self {
        self.desktop_launcher_binary = Some(launcher.into());
        self
    }

    /// Loads an optional prebuilt desktop launcher path from process environment.
    #[must_use]
    pub fn with_desktop_launcher_binary_from_environment(self) -> Self {
        match env::var_os(DESKTOP_LAUNCHER_BINARY_ENV) {
            Some(path) if !path.is_empty() => self.with_desktop_launcher_binary(path),
            _ => self,
        }
    }

    /// Configures the release signing key used by `build-release`.
    #[must_use]
    pub fn with_release_signing_key(mut self, signing_key: ArtifactSigningKey) -> Self {
        self.release_signing_key = Some(signing_key);
        self
    }

    /// Configures the release signing key from hex-encoded Ed25519 private key bytes.
    ///
    /// # Errors
    ///
    /// Returns [`CliDiagnostic`] when the key id is empty or the key material is malformed.
    pub fn with_release_signing_key_hex(
        self,
        key_id: impl Into<String>,
        signing_key: impl Into<String>,
    ) -> Result<Self, Box<CliDiagnostic>> {
        let key_id = key_id.into();
        if key_id.trim().is_empty() {
            return Err(Box::new(CliDiagnostic::error(
                "artifact.signature.signing-key-id-missing",
                "release signing key id must not be empty",
            )));
        }
        let signing_key = ArtifactSigningKey::ed25519_sha256_v1_hex(key_id, signing_key)
            .map_err(|error| Box::new(sealed_artifact_error_diagnostic(error)))?;
        Ok(self.with_release_signing_key(signing_key))
    }

    /// Adds a trusted release verification key used by `verify-artifact`.
    #[must_use]
    pub fn with_trusted_release_key(mut self, key: ArtifactSignatureVerificationKey) -> Self {
        self.trusted_release_keys.push(key);
        self
    }

    /// Adds a trusted release verification key from hex-encoded Ed25519 public key bytes.
    #[must_use]
    pub fn with_trusted_release_key_hex(
        self,
        key_id: impl Into<String>,
        public_key: impl Into<String>,
    ) -> Self {
        self.with_trusted_release_key(ArtifactSignatureVerificationKey::ed25519_sha256_v1_hex(
            key_id, public_key,
        ))
    }

    /// Loads release signing and trust configuration from process environment variables.
    ///
    /// Supported variables:
    ///
    /// - `HAWK2UI_RELEASE_SIGNING_KEY_ID`
    /// - `HAWK2UI_RELEASE_SIGNING_KEY_HEX`
    /// - `HAWK2UI_TRUSTED_RELEASE_KEYS` as comma-separated `key-id:public-key-hex` entries
    ///
    /// # Errors
    ///
    /// Returns [`CliDiagnostic`] when the environment is partially configured or malformed.
    pub fn with_release_security_from_environment(self) -> Result<Self, Box<CliDiagnostic>> {
        self.with_release_security_values(
            env::var(RELEASE_SIGNING_KEY_ID_ENV).ok(),
            env::var(RELEASE_SIGNING_KEY_HEX_ENV).ok(),
            env::var(TRUSTED_RELEASE_KEYS_ENV).ok(),
        )
    }

    fn with_release_security_values(
        mut self,
        signing_key_id: Option<String>,
        signing_key_hex: Option<String>,
        trusted_release_keys: Option<String>,
    ) -> Result<Self, Box<CliDiagnostic>> {
        match (
            non_empty_env(signing_key_id),
            non_empty_env(signing_key_hex),
        ) {
            (Some(key_id), Some(signing_key)) => {
                self = self.with_release_signing_key_hex(key_id, signing_key)?;
            }
            (None, None) => {}
            _ => {
                return Err(Box::new(CliDiagnostic::error(
                    "artifact.signature.signing-config-incomplete",
                    format!(
                        "{RELEASE_SIGNING_KEY_ID_ENV} and {RELEASE_SIGNING_KEY_HEX_ENV} must be set together"
                    ),
                )));
            }
        }

        if let Some(entries) = non_empty_env(trusted_release_keys) {
            for entry in entries
                .split(',')
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
            {
                let Some((key_id, public_key)) = entry.split_once(':') else {
                    return Err(Box::new(CliDiagnostic::error(
                        "artifact.signature.trusted-key-config-invalid",
                        format!(
                            "{TRUSTED_RELEASE_KEYS_ENV} entries must use key-id:public-key-hex"
                        ),
                    )));
                };
                if key_id.trim().is_empty() || public_key.trim().is_empty() {
                    return Err(Box::new(CliDiagnostic::error(
                        "artifact.signature.trusted-key-config-invalid",
                        format!(
                            "{TRUSTED_RELEASE_KEYS_ENV} entries must include key id and public key"
                        ),
                    )));
                }
                self = self.with_trusted_release_key_hex(key_id.trim(), public_key.trim());
            }
        }
        Ok(self)
    }

    /// Executes one parsed command.
    #[must_use]
    pub fn execute(&self, command: CliCommand) -> CommandExecution {
        match command {
            CliCommand::NewProject {
                template,
                package_manager,
            } => self.new_project(template, package_manager),
            CliCommand::Run => self.run(),
            CliCommand::Dev => self.dev(),
            CliCommand::Validate => self.validate(),
            CliCommand::BuildDev => self.build(BuildProfile::Development),
            CliCommand::BuildRelease => self.build(BuildProfile::Production),
            CliCommand::VerifyArtifact { path } => self.verify_artifact(path.as_deref()),
            CliCommand::RunDesktop {
                presentation_backend,
            } => self.run_desktop(presentation_backend),
            CliCommand::PackageDesktop => self.package_desktop(),
            CliCommand::PackagePlugin => self.package_plugin(),
            CliCommand::ExportSchemas => Self::export_schemas(),
            CliCommand::ExportParams => self.export_params(),
            CliCommand::PinIds => self.pin_ids(),
            CliCommand::MigrateManifest { force } => self.migrate_manifest(force),
            CliCommand::Diagnostics => self.diagnostics(),
            CliCommand::Explain => self.explain(),
        }
    }

    fn new_project(
        &self,
        template: CliProjectTemplate,
        package_manager: CliPackageManager,
    ) -> CommandExecution {
        let manifest_path = self.canonical_manifest_path();
        if manifest_path.exists() || self.legacy_manifest_path().exists() {
            return CommandExecution::failure(
                CliExitCode::Usage,
                vec![
                    CliDiagnostic::error("project.exists", "hawk.json already exists")
                        .file(manifest_path.display().to_string()),
                ],
            );
        }

        for (relative_path, contents) in default_project_files(template, package_manager) {
            let path = self.root.join(relative_path);
            if let Err(error) = write_project_file(&path, &contents) {
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
            return self.run_desktop(CliPresentationBackend::Software);
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
            file_watcher.prime();
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
                DevChangeBatch::new(changed_files),
                crate::DevReloadAcknowledgement,
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
            Ok(summary) => CommandExecution::success(dev_live_runtime_summary_output(&summary)),
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
        let artifact = match self.artifact_for_profile(&output.artifact, profile) {
            Ok(artifact) => artifact,
            Err(execution) => return execution,
        };
        let artifact_path = match self.write_artifact_container(&artifact, profile) {
            Ok(path) => path,
            Err(execution) => return execution,
        };
        CommandExecution::success(format!(
            "built {} artifact for {}\nartifact-path: {}\nmanifest-hash: {}\ncontent-hash: {}\ncompiled-scripts: {}\ncompiled-frameworks: {}\njs-module-graphs: {}\ncompiled-styles: {}\ncompiled-assets: {}\nverification-status: release-ready\nsignature-policy: {}\n",
            profile.label(),
            output.manifest.identity.id,
            artifact_path.display(),
            artifact.hashes.manifest.0,
            artifact.hashes.content.0,
            artifact.compiled_scripts.len(),
            artifact.compiled_frameworks.len(),
            artifact.js_module_graphs.len(),
            artifact.compiled_styles.len(),
            artifact.compiled_assets.len(),
            artifact_signature_policy_label(profile)
        ))
    }

    fn artifact_for_profile(
        &self,
        artifact: &SealedArtifact,
        profile: BuildProfile,
    ) -> Result<SealedArtifact, CommandExecution> {
        match profile {
            BuildProfile::Development => Ok(artifact.clone()),
            BuildProfile::Production => {
                let Some(signing_key) = &self.release_signing_key else {
                    return Err(CommandExecution::failure(
                        CliExitCode::Verification,
                        vec![CliDiagnostic::error(
                            "artifact.signature.signing-key-missing",
                            "build-release requires a release signing key",
                        )],
                    ));
                };
                let signed = signing_key.sign(artifact);
                let verifier = self.release_verifier_with_signing_key(signing_key);
                Self::validate_release_artifact_trust(&signed, &verifier).map_err(
                    |diagnostic| {
                        CommandExecution::failure(CliExitCode::Verification, vec![*diagnostic])
                    },
                )?;
                Ok(signed)
            }
        }
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
        let verifier = self.trusted_release_verifier();
        if let Err(diagnostic) = Self::validate_release_artifact_trust(&artifact, &verifier) {
            return CommandExecution::failure(
                CliExitCode::Verification,
                vec![(*diagnostic).file(artifact_path.display().to_string())],
            );
        }
        if let Err(diagnostic) = validate_artifact_runtime_payload(&artifact) {
            return CommandExecution::failure(
                CliExitCode::Verification,
                vec![(*diagnostic).file(artifact_path.display().to_string())],
            );
        }
        CommandExecution::success(format!(
            "verified artifact container\npath: {}\ncontent-hash: {}\nsignature-status: {}\ntrust-status: release-ready\ncompiled-scripts: {}\ncompiled-frameworks: {}\njs-module-graphs: {}\ncompiled-assets: {}\nruntime-scene: {}\n",
            artifact_path.display(),
            artifact.hashes.content.0,
            artifact_signature_status(&artifact),
            artifact.compiled_scripts.len(),
            artifact.compiled_frameworks.len(),
            artifact.js_module_graphs.len(),
            artifact.compiled_assets.len(),
            if artifact.runtime_scene.is_some() {
                "present"
            } else {
                "absent"
            },
        ))
    }

    fn run_desktop(&self, presentation_backend: CliPresentationBackend) -> CommandExecution {
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
        let exit_after_first_frame = self.desktop_exit_after_first_frame
            || std::env::var_os("HAWK2UI_EXIT_AFTER_FIRST_FRAME").is_some();
        let config =
            match desktop_runtime_config_with_assets(&self.root, &output, exit_after_first_frame) {
                Ok(config) => config,
                Err(diagnostic) => {
                    return CommandExecution::failure(CliExitCode::Runtime, vec![*diagnostic]);
                }
            }
            .with_presentation_backend(runtime_presentation_backend(presentation_backend));
        match WinitDesktopRuntime::new().run_blocking(config) {
            Ok(summary) => CommandExecution::success(desktop_runtime_summary_output(
                presentation_backend,
                &summary,
            )),
            Err(error) => CommandExecution::failure(
                CliExitCode::Runtime,
                vec![CliDiagnostic::error(error.rule(), error.message())],
            ),
        }
    }

    fn package_desktop(&self) -> CommandExecution {
        let output = match self.build_workspace() {
            Ok(output) => output,
            Err(execution) => return execution,
        };
        if !output.manifest.has_target(PackageTarget::Desktop) {
            return CommandExecution::failure(
                CliExitCode::Runtime,
                vec![CliDiagnostic::target_incompatibility(
                    "desktop",
                    "manifest does not declare a desktop target",
                )],
            );
        }
        if !output.verification.is_release_ready() {
            let diagnostics = output
                .verification
                .diagnostics
                .iter()
                .map(build_diagnostic_to_cli)
                .collect();
            return CommandExecution::failure(CliExitCode::Verification, diagnostics);
        }
        let artifact = match self.artifact_for_profile(&output.artifact, BuildProfile::Production) {
            Ok(artifact) => artifact,
            Err(execution) => return execution,
        };
        let package = match self.materialize_desktop_package(&output.manifest, &artifact) {
            Ok(package) => package,
            Err(diagnostic) => {
                return CommandExecution::failure(CliExitCode::Runtime, vec![*diagnostic]);
            }
        };
        CommandExecution::success(format!(
            "materialized desktop package:\n- {}\nlauncher-verification-status: passed\nartifact-signature: verified\n",
            package.package_root.display()
        ))
    }

    fn materialize_desktop_package(
        &self,
        manifest: &HawkManifest,
        artifact: &SealedArtifact,
    ) -> Result<DesktopPackageOutput, Box<CliDiagnostic>> {
        artifact
            .ensure_signature_policy(ArtifactSignaturePolicy::RequireVerifiedSignature)
            .map_err(|error| Box::new(sealed_artifact_error_diagnostic(error)))?;
        let package_root = self
            .root
            .join("target")
            .join("hawk2ui")
            .join(format!("{}.AppDir", bundle_name(&manifest.identity.id)));
        if package_root.exists() {
            fs::remove_dir_all(&package_root).map_err(|error| {
                Box::new(CliDiagnostic::error(
                    "package.desktop.clean-failed",
                    format!(
                        "failed to clean desktop package {}: {error}",
                        package_root.display()
                    ),
                ))
            })?;
        }

        let bin_dir = package_root.join("usr").join("bin");
        let resource_dir = package_root.join("usr").join("share").join("hawk2ui");
        fs::create_dir_all(&bin_dir).map_err(|error| {
            desktop_package_io_error("package.desktop.bin-dir-failed", &bin_dir, &error)
        })?;
        fs::create_dir_all(&resource_dir).map_err(|error| {
            desktop_package_io_error("package.desktop.resource-dir-failed", &resource_dir, &error)
        })?;

        let (manifest_file_name, manifest_source) = self.packaged_desktop_manifest_source()?;
        let descriptor = PackagedDesktopRuntimeDescriptor {
            manifest_source: manifest_source.clone(),
            artifact: artifact.clone(),
        };
        let descriptor_path = resource_dir.join("hawk2ui-desktop-runtime.json");
        write_desktop_package_json(&descriptor_path, &descriptor)?;
        let artifact_path = resource_dir.join("hawk2ui-artifact.hawk");
        write_desktop_package_json(&artifact_path, artifact)?;
        let package_manifest_path = package_root.join("hawk2ui-desktop-package.json");
        write_desktop_package_json(
            &package_manifest_path,
            &desktop_package_manifest(manifest, &artifact.hashes.content.0),
        )?;

        let mut package_files = vec![
            package_manifest_path,
            descriptor_path,
            artifact_path,
            Self::write_packaged_desktop_workspace(
                &manifest_file_name,
                &manifest_source,
                &resource_dir,
            )?,
        ];
        package_files.extend(self.copy_packaged_desktop_assets(manifest, &resource_dir)?);
        let launcher_path = if let Some(launcher_binary) = &self.desktop_launcher_binary {
            Self::install_prebuilt_desktop_launcher(manifest, &package_root, launcher_binary)?
        } else {
            let generated_root = Self::write_desktop_launcher_workspace(manifest, &resource_dir)?;
            package_files.push(generated_root.join("Cargo.toml"));
            package_files.push(generated_root.join("src").join("main.rs"));
            Self::build_desktop_launcher(manifest, &package_root, &generated_root)?
        };
        verify_native_desktop_launcher(&launcher_path)?;
        package_files.push(launcher_path);

        let hash_manifest_path = resource_dir.join("hawk2ui-hashes.json");
        write_desktop_package_json(
            &hash_manifest_path,
            &desktop_package_hash_manifest(&package_root, &package_files)?,
        )?;
        Ok(DesktopPackageOutput { package_root })
    }

    fn packaged_desktop_manifest_source(&self) -> Result<(String, String), Box<CliDiagnostic>> {
        let canonical = self.canonical_manifest_path();
        if canonical.is_file() {
            let source = fs::read_to_string(&canonical).map_err(|error| {
                desktop_package_io_error("package.desktop.manifest-read-failed", &canonical, &error)
            })?;
            return Ok((CANONICAL_MANIFEST_FILE.to_string(), source));
        }
        let legacy = self.legacy_manifest_path();
        let source = fs::read_to_string(&legacy).map_err(|error| {
            desktop_package_io_error("package.desktop.manifest-read-failed", &legacy, &error)
        })?;
        Ok((LEGACY_MANIFEST_FILE.to_string(), source))
    }

    fn write_packaged_desktop_workspace(
        manifest_file_name: &str,
        manifest_source: &str,
        resource_dir: &Path,
    ) -> Result<PathBuf, Box<CliDiagnostic>> {
        let workspace_manifest_path = resource_dir.join("workspace").join(manifest_file_name);
        write_desktop_package_file(&workspace_manifest_path, manifest_source)?;
        Ok(workspace_manifest_path)
    }

    fn copy_packaged_desktop_assets(
        &self,
        manifest: &HawkManifest,
        resource_dir: &Path,
    ) -> Result<Vec<PathBuf>, Box<CliDiagnostic>> {
        let workspace_root = resource_dir.join("workspace");
        let mut copied = Vec::new();
        for asset in &manifest.assets {
            if asset.kind == "design-token" {
                continue;
            }
            let bytes = read_runtime_asset_bytes(&self.root, &asset.path)?;
            let destination = workspace_root.join(&asset.path);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    desktop_package_io_error("package.desktop.asset-dir-failed", parent, &error)
                })?;
            }
            fs::write(&destination, bytes).map_err(|error| {
                desktop_package_io_error("package.desktop.asset-copy-failed", &destination, &error)
            })?;
            copied.push(destination);
        }
        Ok(copied)
    }

    fn write_desktop_launcher_workspace(
        manifest: &HawkManifest,
        resource_dir: &Path,
    ) -> Result<PathBuf, Box<CliDiagnostic>> {
        let generated_root = resource_dir.join("generated-launcher");
        let source_dir = generated_root.join("src");
        fs::create_dir_all(&source_dir).map_err(|error| {
            desktop_package_io_error(
                "package.desktop.launcher-source-dir-failed",
                &source_dir,
                &error,
            )
        })?;
        write_desktop_package_file(
            &generated_root.join("Cargo.toml"),
            desktop_launcher_cargo_toml(manifest, Path::new(env!("CARGO_MANIFEST_DIR"))),
        )?;
        write_desktop_package_file(
            &source_dir.join("main.rs"),
            "fn main() {\n    if let Err(error) = hawk2ui_cli::run_packaged_desktop_from_default_location() {\n        eprintln!(\"{error}\");\n        std::process::exit(1);\n    }\n}\n",
        )?;
        Ok(generated_root)
    }

    fn build_desktop_launcher(
        manifest: &HawkManifest,
        package_root: &Path,
        generated_root: &Path,
    ) -> Result<PathBuf, Box<CliDiagnostic>> {
        let manifest_path = generated_root.join("Cargo.toml");
        let target_dir = generated_root.join("target");
        let command_output = Command::new(cargo_executable())
            .arg("build")
            .arg("--release")
            .arg("--manifest-path")
            .arg(&manifest_path)
            .arg("--target-dir")
            .arg(&target_dir)
            .current_dir(generated_root)
            .output()
            .map_err(|error| {
                Box::new(
                    CliDiagnostic::error(
                        "package.desktop.launcher-build-launch-failed",
                        format!("failed to launch Cargo for desktop launcher: {error}"),
                    )
                    .file(manifest_path.display().to_string()),
                )
            })?;
        if !command_output.status.success() {
            return Err(Box::new(
                CliDiagnostic::error(
                    "package.desktop.launcher-build-failed",
                    format!(
                        "generated desktop launcher build failed:{}",
                        render_process_output(&command_output)
                    ),
                )
                .file(manifest_path.display().to_string()),
            ));
        }
        remove_generated_lockfile(generated_root)?;
        let built_launcher =
            target_dir
                .join("release")
                .join(executable_filename(&desktop_launcher_package_name(
                    manifest,
                )));
        let launcher_path = desktop_launcher_install_path(manifest, package_root);
        fs::copy(&built_launcher, &launcher_path).map_err(|error| {
            Box::new(
                CliDiagnostic::error(
                    "package.desktop.launcher-install-failed",
                    format!(
                        "failed to install desktop launcher {} into {}: {error}",
                        built_launcher.display(),
                        launcher_path.display()
                    ),
                )
                .file(launcher_path.display().to_string()),
            )
        })?;
        fs::remove_dir_all(&target_dir).map_err(|error| {
            Box::new(
                CliDiagnostic::error(
                    "package.desktop.launcher-build-cache-clean-failed",
                    format!(
                        "failed to remove generated launcher build cache {}: {error}",
                        target_dir.display()
                    ),
                )
                .file(target_dir.display().to_string()),
            )
        })?;
        Ok(launcher_path)
    }

    fn install_prebuilt_desktop_launcher(
        manifest: &HawkManifest,
        package_root: &Path,
        launcher_binary: &Path,
    ) -> Result<PathBuf, Box<CliDiagnostic>> {
        if !launcher_binary.is_file() {
            return Err(Box::new(
                CliDiagnostic::error(
                    "package.desktop.launcher-prebuilt-missing",
                    format!(
                        "prebuilt desktop launcher binary {} does not exist",
                        launcher_binary.display()
                    ),
                )
                .file(launcher_binary.display().to_string()),
            ));
        }

        let launcher_path = desktop_launcher_install_path(manifest, package_root);
        fs::copy(launcher_binary, &launcher_path).map_err(|error| {
            Box::new(
                CliDiagnostic::error(
                    "package.desktop.launcher-prebuilt-install-failed",
                    format!(
                        "failed to install prebuilt desktop launcher {} into {}: {error}",
                        launcher_binary.display(),
                        launcher_path.display()
                    ),
                )
                .file(launcher_path.display().to_string()),
            )
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let source_mode = fs::metadata(launcher_binary)
                .map_err(|error| {
                    Box::new(
                        CliDiagnostic::error(
                            "package.desktop.launcher-prebuilt-metadata-failed",
                            format!(
                                "failed to read prebuilt desktop launcher metadata {}: {error}",
                                launcher_binary.display()
                            ),
                        )
                        .file(launcher_binary.display().to_string()),
                    )
                })?
                .permissions()
                .mode();
            fs::set_permissions(
                &launcher_path,
                fs::Permissions::from_mode(source_mode | 0o755),
            )
            .map_err(|error| {
                Box::new(
                    CliDiagnostic::error(
                        "package.desktop.launcher-prebuilt-permissions-failed",
                        format!(
                            "failed to mark packaged desktop launcher {} executable: {error}",
                            launcher_path.display()
                        ),
                    )
                    .file(launcher_path.display().to_string()),
                )
            })?;
        }
        Ok(launcher_path)
    }

    fn package_plugin(&self) -> CommandExecution {
        let build_output = match self.build_workspace() {
            Ok(output) => output,
            Err(execution) => return execution,
        };
        let request = match self.package_plugin_request(build_output) {
            Ok(request) => request,
            Err(execution) => return execution,
        };
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
        let produced_binaries = match self.build_host_loadable_plugin_binaries(&outputs) {
            Ok(produced_binaries) => produced_binaries,
            Err(diagnostic) => {
                return CommandExecution::failure(CliExitCode::Runtime, vec![*diagnostic]);
            }
        };
        let verification =
            plan.verify_trusted_materialized(&outputs, &self.trusted_release_verifier());
        if verification.status() == VerificationStatus::Failed {
            return CommandExecution::failure(
                CliExitCode::Verification,
                package_verification_diagnostics(&verification),
            );
        }
        let mut stdout = String::from("materialized plugin package layouts:\n");
        for target in &outputs {
            stdout.push_str("- ");
            stdout.push_str(&target.output_path);
            stdout.push('\n');
        }
        stdout.push_str("layout-verification-status: passed\n");
        let _ = writeln!(
            stdout,
            "host-loadable-binaries: produced={produced_binaries}"
        );
        CommandExecution::success(stdout)
    }

    fn build_host_loadable_plugin_binaries(
        &self,
        outputs: &[MaterializedPackageOutput],
    ) -> Result<usize, Box<CliDiagnostic>> {
        let mut produced = 0_usize;
        for output in outputs {
            if self.build_host_loadable_plugin_binary(output)? {
                produced += 1;
            }
        }
        Ok(produced)
    }

    fn build_host_loadable_plugin_binary(
        &self,
        output: &MaterializedPackageOutput,
    ) -> Result<bool, Box<CliDiagnostic>> {
        let Some(build) = generated_plugin_binary_build(output.format) else {
            return Ok(false);
        };
        let package_root = Path::new(&output.output_path);
        let generated_root = package_root
            .join("Contents")
            .join("Resources")
            .join(build.generated_root);
        let manifest_path = generated_root.join("Cargo.toml");
        let target_dir = self.plugin_binary_target_dir(package_root, build);
        let command_output = Command::new(cargo_executable())
            .arg("build")
            .arg("--release")
            .arg("--manifest-path")
            .arg(&manifest_path)
            .arg("--target-dir")
            .arg(&target_dir)
            .current_dir(&generated_root)
            .output()
            .map_err(|error| {
                Box::new(
                    CliDiagnostic::error(
                        "package.plugin-binary.build-launch-failed",
                        format!(
                            "failed to launch Cargo for generated {} plugin binary: {error}",
                            build.label
                        ),
                    )
                    .file(manifest_path.display().to_string()),
                )
            })?;
        if !command_output.status.success() {
            return Err(Box::new(
                CliDiagnostic::error(
                    "package.plugin-binary.build-failed",
                    format!(
                        "generated {} plugin binary build failed:{}",
                        build.label,
                        render_process_output(&command_output)
                    ),
                )
                .file(manifest_path.display().to_string()),
            ));
        }

        remove_generated_lockfile(&generated_root)?;
        let compiled_library = target_dir
            .join("release")
            .join(dynamic_library_filename(build.library_file_stem));
        let binary_slot = find_host_binary_slot(package_root, build.package_extension)?;
        fs::copy(&compiled_library, &binary_slot).map_err(|error| {
            Box::new(
                CliDiagnostic::error(
                    "package.plugin-binary.install-failed",
                    format!(
                        "failed to install generated {} plugin binary {} into {}: {error}",
                        build.label,
                        compiled_library.display(),
                        binary_slot.display()
                    ),
                )
                .file(binary_slot.display().to_string()),
            )
        })?;
        refresh_package_hash_for_file(
            package_root,
            Path::new(&output.hash_manifest_path),
            &binary_slot,
        )?;
        Ok(true)
    }

    fn plugin_binary_target_dir(
        &self,
        package_root: &Path,
        build: GeneratedPluginBinaryBuild,
    ) -> PathBuf {
        let package_name = package_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("package");
        self.root
            .join("target")
            .join("hawk2ui")
            .join("plugin-binary-builds")
            .join(package_name)
            .join(build.generated_root)
    }

    fn package_plugin_request(
        &self,
        build_output: BuildWorkspaceOutput,
    ) -> Result<PackageRequest, CommandExecution> {
        let BuildWorkspaceOutput {
            manifest, artifact, ..
        } = build_output;
        if !manifest.has_target(PackageTarget::Plugin) {
            return Err(CommandExecution::failure(
                CliExitCode::Validation,
                vec![CliDiagnostic::target_incompatibility(
                    "plugin",
                    "manifest does not declare a plugin target",
                )],
            ));
        }
        let Some(plugin) = &manifest.plugin else {
            return Err(CommandExecution::failure(
                CliExitCode::Validation,
                vec![CliDiagnostic::error(
                    "plugin.metadata.missing",
                    "plugin target requires [plugin] metadata",
                )],
            ));
        };
        if manifest.editor.is_none() {
            return Err(CommandExecution::failure(
                CliExitCode::Validation,
                vec![CliDiagnostic::error(
                    "plugin.editor.missing",
                    "plugin target requires [editor] metadata",
                )],
            ));
        }

        let artifact = self.artifact_for_profile(&artifact, BuildProfile::Production)?;
        let metadata = FormatMetadata::new(&plugin.id, &plugin.name, "Hawk2UI")
            .version(&manifest.identity.version);
        let output = BundleOutput::new(
            self.root.join("target/hawk2ui").to_string_lossy(),
            bundle_name(&manifest.identity.id),
        );
        Ok(plugin_package_formats().fold(
            PackageRequest::new(metadata, output, manifest.parameter_model())
                .with_editor(plugin_editor_from_manifest(&manifest))
                .with_runtime_artifact(signed_runtime_artifact_value(&artifact)?),
            PackageRequest::with_format,
        ))
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
        let manifest_path = self.legacy_manifest_path();
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

    fn migrate_manifest(&self, force: bool) -> CommandExecution {
        let legacy_path = self.legacy_manifest_path();
        let canonical_path = self.canonical_manifest_path();
        if !legacy_path.is_file() {
            return CommandExecution::failure(
                CliExitCode::Validation,
                vec![
                    CliDiagnostic::error(
                        "manifest.migration.legacy-missing",
                        "legacy manifest.hawk.toml is required for migration",
                    )
                    .file(legacy_path.display().to_string()),
                ],
            );
        }
        if canonical_path.exists() && !force {
            return CommandExecution::failure(
                CliExitCode::Usage,
                vec![
                    CliDiagnostic::error(
                        "manifest.migration.would-overwrite",
                        "hawk.json already exists; pass --force to overwrite it",
                    )
                    .file(canonical_path.display().to_string()),
                ],
            );
        }
        let source = match fs::read_to_string(&legacy_path) {
            Ok(source) => source,
            Err(error) => return io_failure("manifest.read-failed", &legacy_path, &error),
        };
        let migrated = match migrate_toml_manifest_to_json(&source) {
            Ok(migrated) => migrated,
            Err(error) => {
                return CommandExecution::failure(
                    CliExitCode::Validation,
                    vec![manifest_error_diagnostic(error).file(legacy_path.display().to_string())],
                );
            }
        };
        if let Err(error) = fs::write(&canonical_path, migrated) {
            return io_failure("manifest.write-failed", &canonical_path, &error);
        }
        CommandExecution::success(format!(
            "migrated {LEGACY_MANIFEST_FILE} to {CANONICAL_MANIFEST_FILE}\n"
        ))
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
        artifact: &SealedArtifact,
        profile: BuildProfile,
    ) -> Result<PathBuf, CommandExecution> {
        let artifact_path = self.artifact_output_path(profile);
        let bytes = artifact
            .to_container_bytes(artifact_signature_policy(profile))
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

    fn trusted_release_verifier(&self) -> ArtifactSignatureVerifier {
        ArtifactSignatureVerifier::new(self.configured_trusted_release_keys())
    }

    fn release_verifier_with_signing_key(
        &self,
        signing_key: &ArtifactSigningKey,
    ) -> ArtifactSignatureVerifier {
        let mut keys = self.configured_trusted_release_keys();
        let signing_verification_key = signing_key.verification_key();
        if !keys.contains(&signing_verification_key) {
            keys.push(signing_verification_key);
        }
        ArtifactSignatureVerifier::new(keys)
    }

    fn configured_trusted_release_keys(&self) -> Vec<ArtifactSignatureVerificationKey> {
        let mut keys = self.trusted_release_keys.clone();
        if let Some(signing_key) = &self.release_signing_key {
            let verification_key = signing_key.verification_key();
            if !keys.contains(&verification_key) {
                keys.push(verification_key);
            }
        }
        keys
    }

    fn validate_release_artifact_trust(
        artifact: &SealedArtifact,
        verifier: &ArtifactSignatureVerifier,
    ) -> Result<(), Box<CliDiagnostic>> {
        let record = PackageTrustRecord::from_trusted_sealed_artifact(
            artifact,
            verifier,
            VerificationReportStatus::Present,
        );
        PackageTrustValidator::new(ARTIFACT_SCHEMA_VERSION.major)
            .validate(&record)
            .map_err(|violation| Box::new(package_trust_violation_diagnostic(violation)))
    }

    fn manifest_path(&self) -> PathBuf {
        existing_manifest_path(&self.root)
    }

    fn canonical_manifest_path(&self) -> PathBuf {
        self.root.join(CANONICAL_MANIFEST_FILE)
    }

    fn legacy_manifest_path(&self) -> PathBuf {
        self.root.join(LEGACY_MANIFEST_FILE)
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

fn non_empty_env(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn existing_manifest_path(root: &Path) -> PathBuf {
    let canonical = root.join(CANONICAL_MANIFEST_FILE);
    if canonical.is_file() {
        canonical
    } else {
        root.join(LEGACY_MANIFEST_FILE)
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
            .file(existing_manifest_path(root).display().to_string()),
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
        BuildWorkspaceError::FrameworkCompilation {
            path,
            framework,
            message,
        } => CliDiagnostic::error(
            "build.framework.compile-failed",
            format!(
                "declared {framework:?} framework source failed production compilation: {message}"
            ),
        )
        .file(root.join(path).display().to_string()),
        BuildWorkspaceError::PackageManager(error) => CliDiagnostic::error(
            error.rule(),
            format!(
                "package-manager metadata could not be captured: {}",
                error.message()
            ),
        ),
        BuildWorkspaceError::JsBundle { path, error } => CliDiagnostic::error(
            error.rule(),
            format!("JavaScript bundle could not be sealed: {}", error.message()),
        )
        .file(root.join(path).display().to_string()),
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

fn package_verification_diagnostics(report: &PackageVerificationReport) -> Vec<CliDiagnostic> {
    let mut diagnostics = Vec::new();
    for entry in report
        .entries()
        .iter()
        .filter(|entry| entry.status() == VerificationStatus::Failed)
    {
        if entry.diagnostics().is_empty() {
            diagnostics.push(
                CliDiagnostic::error(
                    "plugin.package.verification-failed",
                    format!(
                        "plugin package {:?} verification failed without detailed diagnostics",
                        entry.target().format()
                    ),
                )
                .file(entry.target().output_path()),
            );
            continue;
        }
        diagnostics.extend(entry.diagnostics().iter().map(|diagnostic| {
            CliDiagnostic::error(
                diagnostic.rule(),
                format!(
                    "plugin package {:?} verification failed: {}",
                    entry.target().format(),
                    diagnostic.message()
                ),
            )
            .file(entry.target().output_path())
        }));
    }
    if diagnostics.is_empty() {
        diagnostics.push(CliDiagnostic::error(
            "plugin.package.verification-failed",
            "plugin package verification failed",
        ));
    }
    diagnostics
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

fn validate_artifact_runtime_payload(artifact: &SealedArtifact) -> Result<(), Box<CliDiagnostic>> {
    if artifact.compiled_scripts.is_empty()
        && artifact.compiled_frameworks.is_empty()
        && artifact.js_module_graphs.is_empty()
        && artifact.runtime_scene.is_none()
    {
        return Err(Box::new(CliDiagnostic::error(
            "artifact.runtime-payload.missing",
            "sealed artifact does not include compiled scripts, sealed JS module graphs, runtime scene, or runtime assets",
        )));
    }
    Ok(())
}

fn package_trust_violation_diagnostic(
    violation: hawk2ui_security_model::PackageTrustViolation,
) -> CliDiagnostic {
    let diagnostic = hawk2ui_api::Diagnostic::from(violation);
    CliDiagnostic::error(diagnostic.rule.as_str(), diagnostic.message)
}

const fn artifact_signature_policy(profile: BuildProfile) -> ArtifactSignaturePolicy {
    match profile {
        BuildProfile::Development => ArtifactSignaturePolicy::AllowUnsignedDevelopment,
        BuildProfile::Production => ArtifactSignaturePolicy::RequireVerifiedSignature,
    }
}

const fn artifact_signature_policy_label(profile: BuildProfile) -> &'static str {
    match profile {
        BuildProfile::Development => "unsigned-development",
        BuildProfile::Production => "verified-release",
    }
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

fn desktop_package_io_error(
    rule: &'static str,
    path: &Path,
    error: &std::io::Error,
) -> Box<CliDiagnostic> {
    Box::new(CliDiagnostic::error(rule, error.to_string()).file(path.display().to_string()))
}

fn write_desktop_package_file(
    path: &Path,
    contents: impl AsRef<[u8]>,
) -> Result<(), Box<CliDiagnostic>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            desktop_package_io_error("package.desktop.create-dir-failed", parent, &error)
        })?;
    }
    fs::write(path, contents)
        .map_err(|error| desktop_package_io_error("package.desktop.write-failed", path, &error))
}

fn write_desktop_package_json<T: serde::Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), Box<CliDiagnostic>> {
    let mut payload = serde_json::to_string_pretty(value).map_err(|error| {
        Box::new(CliDiagnostic::error(
            "package.desktop.json-encode-failed",
            format!(
                "failed to encode desktop package JSON {}: {error}",
                path.display()
            ),
        ))
    })?;
    payload.push('\n');
    write_desktop_package_file(path, payload)
}

fn desktop_package_manifest(manifest: &HawkManifest, content_hash: &str) -> serde_json::Value {
    let launcher_file_name = desktop_launcher_file_name(manifest);
    serde_json::json!({
        "packageType": "desktop",
        "id": manifest.identity.id,
        "displayName": manifest.identity.name,
        "version": manifest.identity.version,
        "entry": format!("usr/bin/{launcher_file_name}"),
        "runtimeDescriptor": "usr/share/hawk2ui/hawk2ui-desktop-runtime.json",
        "artifact": "usr/share/hawk2ui/hawk2ui-artifact.hawk",
        "contentHash": content_hash,
    })
}

fn cargo_toml_basic_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 2);
    output.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            '\u{08}' => output.push_str("\\b"),
            '\t' => output.push_str("\\t"),
            '\n' => output.push_str("\\n"),
            '\u{0c}' => output.push_str("\\f"),
            '\r' => output.push_str("\\r"),
            '\u{00}'..='\u{1f}' => {
                let _ = write!(output, "\\u{:04X}", u32::from(ch));
            }
            _ => output.push(ch),
        }
    }
    output.push('"');
    output
}

fn desktop_launcher_package_name(manifest: &HawkManifest) -> String {
    format!(
        "hawk2ui_desktop_launcher_{}",
        bundle_name(&manifest.identity.id).replace('-', "_")
    )
}

fn desktop_launcher_install_path(manifest: &HawkManifest, package_root: &Path) -> PathBuf {
    package_root
        .join("usr")
        .join("bin")
        .join(desktop_launcher_file_name(manifest))
}

fn desktop_launcher_file_name(manifest: &HawkManifest) -> String {
    executable_filename(&bundle_name(&manifest.identity.id))
}

fn desktop_launcher_cargo_toml(manifest: &HawkManifest, cli_crate_path: &Path) -> String {
    format!(
        "[package]\nname = {}\nversion = {}\nedition = \"2024\"\n\n[dependencies]\nhawk2ui-cli = {{ path = {} }}\n",
        cargo_toml_basic_string(&desktop_launcher_package_name(manifest)),
        cargo_toml_basic_string(&manifest.identity.version),
        cargo_toml_basic_string(&cli_crate_path.display().to_string())
    )
}

fn desktop_package_hash_manifest(
    package_root: &Path,
    package_files: &[PathBuf],
) -> Result<serde_json::Value, Box<CliDiagnostic>> {
    let mut entries = Vec::new();
    for file in package_files {
        let bytes = fs::read(file).map_err(|error| {
            desktop_package_io_error("package.desktop.hash-read-failed", file, &error)
        })?;
        entries.push((
            normalized_package_relative_path(package_root, file)?,
            AssetHash::sha256_bytes(&bytes),
        ));
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    let files = entries
        .into_iter()
        .map(|(path, hash)| {
            serde_json::json!({
                "path": path,
                "hash": hash.as_str(),
            })
        })
        .collect::<Vec<_>>();
    Ok(serde_json::json!({
        "algorithm": "sha256",
        "files": files,
    }))
}

fn verify_native_desktop_launcher(path: &Path) -> Result<(), Box<CliDiagnostic>> {
    let bytes = fs::read(path).map_err(|error| {
        desktop_package_io_error("package.desktop.launcher-read-failed", path, &error)
    })?;
    if native_executable_magic_matches(&bytes) {
        Ok(())
    } else {
        Err(Box::new(
            CliDiagnostic::error(
                "package.desktop.launcher-not-native",
                format!(
                    "desktop launcher {} is not a native executable for this platform",
                    path.display()
                ),
            )
            .file(path.display().to_string()),
        ))
    }
}

fn native_executable_magic_matches(bytes: &[u8]) -> bool {
    #[cfg(target_os = "linux")]
    {
        bytes.starts_with(b"\x7fELF")
    }
    #[cfg(target_os = "windows")]
    {
        bytes.starts_with(b"MZ")
    }
    #[cfg(target_os = "macos")]
    {
        bytes.starts_with(&[0xfe, 0xed, 0xfa, 0xce])
            || bytes.starts_with(&[0xfe, 0xed, 0xfa, 0xcf])
            || bytes.starts_with(&[0xca, 0xfe, 0xba, 0xbe])
            || bytes.starts_with(&[0xcf, 0xfa, 0xed, 0xfe])
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        !bytes.is_empty()
    }
}

fn render_diagnostics(diagnostics: &[CliDiagnostic]) -> String {
    diagnostics
        .iter()
        .map(CliDiagnostic::render)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GeneratedPluginBinaryBuild {
    label: &'static str,
    generated_root: &'static str,
    library_file_stem: &'static str,
    package_extension: &'static str,
}

const fn generated_plugin_binary_build(
    format: PackageFormat,
) -> Option<GeneratedPluginBinaryBuild> {
    match format {
        PackageFormat::Clap => Some(GeneratedPluginBinaryBuild {
            label: "CLAP",
            generated_root: "generated-clap",
            library_file_stem: "hawk2ui_generated_clap",
            package_extension: "clap",
        }),
        PackageFormat::Vst3 => Some(GeneratedPluginBinaryBuild {
            label: "VST3",
            generated_root: "generated-vst3",
            library_file_stem: "hawk2ui_generated_vst3",
            package_extension: "vst3",
        }),
        PackageFormat::Au => Some(GeneratedPluginBinaryBuild {
            label: "AU",
            generated_root: "generated-au",
            library_file_stem: "hawk2ui_generated_au",
            package_extension: "component",
        }),
        PackageFormat::Standalone
        | PackageFormat::DesktopBundle
        | PackageFormat::SealedArtifact => None,
    }
}

fn cargo_executable() -> std::ffi::OsString {
    match env::var_os("CARGO") {
        Some(path) => path,
        None => "cargo".into(),
    }
}

fn executable_filename(file_stem: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        format!("{file_stem}.exe")
    }
    #[cfg(not(target_os = "windows"))]
    {
        file_stem.to_string()
    }
}

fn dynamic_library_filename(library_file_stem: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        format!("{library_file_stem}.dll")
    }
    #[cfg(target_os = "macos")]
    {
        format!("lib{library_file_stem}.dylib")
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        format!("lib{library_file_stem}.so")
    }
}

fn render_process_output(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    format!(
        "\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
        output.status, stdout, stderr
    )
}

fn remove_generated_lockfile(generated_root: &Path) -> Result<(), Box<CliDiagnostic>> {
    let lockfile = generated_root.join("Cargo.lock");
    if lockfile.is_file() {
        fs::remove_file(&lockfile).map_err(|error| {
            Box::new(
                CliDiagnostic::error(
                    "package.plugin-binary.lockfile-cleanup-failed",
                    format!(
                        "failed to remove generated plugin Cargo.lock {}: {error}",
                        lockfile.display()
                    ),
                )
                .file(lockfile.display().to_string()),
            )
        })?;
    }
    Ok(())
}

fn find_host_binary_slot(
    package_root: &Path,
    package_extension: &str,
) -> Result<PathBuf, Box<CliDiagnostic>> {
    if package_extension == "component"
        && let Some(path) = find_au_host_binary_slot(package_root)?
    {
        return Ok(path);
    }

    let mut stack = vec![package_root.to_path_buf()];
    let mut matches = Vec::new();
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).map_err(|error| {
            Box::new(
                CliDiagnostic::error(
                    "package.plugin-binary.scan-failed",
                    format!(
                        "failed to scan package directory {}: {error}",
                        dir.display()
                    ),
                )
                .file(dir.display().to_string()),
            )
        })?;
        for entry in entries {
            let path = entry
                .map_err(|error| {
                    Box::new(
                        CliDiagnostic::error(
                            "package.plugin-binary.scan-failed",
                            format!("failed to read package directory entry: {error}"),
                        )
                        .file(dir.display().to_string()),
                    )
                })?
                .path();
            if path.is_dir() {
                if path.file_name().and_then(|name| name.to_str()) != Some("target") {
                    stack.push(path);
                }
            } else if path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case(package_extension))
            {
                matches.push(path);
            }
        }
    }
    match matches.as_slice() {
        [path] => Ok(path.clone()),
        [] => Err(Box::new(
            CliDiagnostic::error(
                "package.plugin-binary.slot-missing",
                format!(
                    "package {} does not contain a .{package_extension} host binary slot",
                    package_root.display()
                ),
            )
            .file(package_root.display().to_string()),
        )),
        _ => Err(Box::new(
            CliDiagnostic::error(
                "package.plugin-binary.slot-ambiguous",
                format!(
                    "package {} contains multiple .{package_extension} host binary slots",
                    package_root.display()
                ),
            )
            .file(package_root.display().to_string()),
        )),
    }
}

fn find_au_host_binary_slot(package_root: &Path) -> Result<Option<PathBuf>, Box<CliDiagnostic>> {
    let macos_dir = package_root.join("Contents").join("MacOS");
    if !macos_dir.is_dir() {
        return Ok(None);
    }

    let mut slots = Vec::new();
    let entries = fs::read_dir(&macos_dir).map_err(|error| {
        Box::new(
            CliDiagnostic::error(
                "package.plugin-binary.scan-failed",
                format!(
                    "failed to scan AU executable directory {}: {error}",
                    macos_dir.display()
                ),
            )
            .file(macos_dir.display().to_string()),
        )
    })?;
    for entry in entries {
        let path = entry
            .map_err(|error| {
                Box::new(
                    CliDiagnostic::error(
                        "package.plugin-binary.scan-failed",
                        format!("failed to read AU executable directory entry: {error}"),
                    )
                    .file(macos_dir.display().to_string()),
                )
            })?
            .path();
        if path.is_file() {
            slots.push(path);
        }
    }

    match slots.as_slice() {
        [path] => Ok(Some(path.clone())),
        [] => Ok(None),
        _ => Err(Box::new(
            CliDiagnostic::error(
                "package.plugin-binary.slot-ambiguous",
                format!(
                    "AU package {} contains multiple executable slots in Contents/MacOS",
                    package_root.display()
                ),
            )
            .file(macos_dir.display().to_string()),
        )),
    }
}

fn refresh_package_hash_for_file(
    package_root: &Path,
    hash_manifest_path: &Path,
    file_path: &Path,
) -> Result<(), Box<CliDiagnostic>> {
    let bytes = fs::read(file_path).map_err(|error| {
        Box::new(
            CliDiagnostic::error(
                "package.hash.file-read-failed",
                format!(
                    "failed to read package file {}: {error}",
                    file_path.display()
                ),
            )
            .file(file_path.display().to_string()),
        )
    })?;
    let relative_path = normalized_package_relative_path(package_root, file_path)?;
    let hash = AssetHash::sha256_bytes(&bytes);
    let source = fs::read_to_string(hash_manifest_path).map_err(|error| {
        Box::new(
            CliDiagnostic::error(
                "package.hash.manifest-read-failed",
                format!(
                    "failed to read package hash manifest {}: {error}",
                    hash_manifest_path.display()
                ),
            )
            .file(hash_manifest_path.display().to_string()),
        )
    })?;
    let refreshed = refresh_hash_manifest_entry(&source, &relative_path, hash.as_str())?;
    fs::write(hash_manifest_path, refreshed).map_err(|error| {
        Box::new(
            CliDiagnostic::error(
                "package.hash.manifest-write-failed",
                format!(
                    "failed to write package hash manifest {}: {error}",
                    hash_manifest_path.display()
                ),
            )
            .file(hash_manifest_path.display().to_string()),
        )
    })
}

fn normalized_package_relative_path(
    package_root: &Path,
    file_path: &Path,
) -> Result<String, Box<CliDiagnostic>> {
    file_path
        .strip_prefix(package_root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|error| {
            Box::new(
                CliDiagnostic::error(
                    "package.hash.path-outside-package",
                    format!(
                        "package file {} is not below package root {}: {error}",
                        file_path.display(),
                        package_root.display()
                    ),
                )
                .file(file_path.display().to_string()),
            )
        })
}

fn refresh_hash_manifest_entry(
    manifest: &str,
    relative_path: &str,
    replacement_hash: &str,
) -> Result<String, Box<CliDiagnostic>> {
    let mut output = String::with_capacity(manifest.len());
    let mut awaiting_hash = false;
    let mut replaced = false;
    for line in manifest.lines() {
        if awaiting_hash && line.trim_start().starts_with("hash = ") {
            let _ = writeln!(output, "hash = {replacement_hash:?}");
            awaiting_hash = false;
            replaced = true;
            continue;
        }
        if manifest_line_path(line) == Some(relative_path) {
            awaiting_hash = true;
        }
        output.push_str(line);
        output.push('\n');
    }
    if replaced {
        Ok(output)
    } else {
        Err(Box::new(CliDiagnostic::error(
            "package.hash.entry-missing",
            format!("package hash manifest has no entry for {relative_path}"),
        )))
    }
}

fn manifest_line_path(line: &str) -> Option<&str> {
    line.trim()
        .strip_prefix("path = \"")
        .and_then(|value| value.strip_suffix('"'))
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

fn plugin_editor_from_manifest(manifest: &HawkManifest) -> PluginEditor {
    let editor = manifest
        .editor
        .as_ref()
        .expect("plugin editor metadata was checked before package planning");
    PluginEditor::custom(
        "main-editor",
        PluginEditorSize::new(f64::from(editor.width), f64::from(editor.height), 1.0),
    )
}

fn plugin_package_formats() -> impl Iterator<Item = PackageFormat> {
    [PackageFormat::Clap, PackageFormat::Vst3, PackageFormat::Au].into_iter()
}

fn runtime_presentation_backend(
    presentation_backend: CliPresentationBackend,
) -> WinitPresentationBackend {
    match presentation_backend {
        CliPresentationBackend::Software => WinitPresentationBackend::Software,
        CliPresentationBackend::GpuPreferred => WinitPresentationBackend::GpuPreferred,
        CliPresentationBackend::GpuRequired => WinitPresentationBackend::GpuRequired,
    }
}

fn desktop_runtime_summary_output(
    presentation_backend: CliPresentationBackend,
    summary: &WinitDesktopRuntimeSummary,
) -> String {
    let mut output = format!(
        "desktop runtime exited cleanly\npresentation-backend-requested: {}\npresentation-backend-used: {}\nframes-presented: {}\ngpu-frames-presented: {}\ngpu-readback-verified: {}\nframe-duration-last-us: {}\nframe-duration-max-us: {}\nframe-duration-average-us: {}\nframe-duration-total-us: {}\nresizes: {}\ndpi-changes: {}\ninput-events: {}\nclose-requested: {}\n",
        presentation_backend.label(),
        summary.presentation_backend_used.label(),
        summary.frames_presented,
        summary.gpu_frames_presented,
        summary.gpu_readback_verified,
        summary.last_frame_duration_micros,
        summary.max_frame_duration_micros,
        summary.average_frame_duration_micros(),
        summary.total_frame_duration_micros,
        summary.resizes,
        summary.dpi_changes,
        summary.input_events,
        summary.close_requested
    );
    if let Some(reason) = summary.presentation_fallback_reason.as_ref() {
        let _ = writeln!(output, "presentation-fallback-rule: {}", reason.rule());
        let _ = writeln!(
            output,
            "presentation-fallback-message: {}",
            reason.message()
        );
    }
    output
}

fn dev_live_runtime_summary_output(summary: &WinitDesktopRuntimeSummary) -> String {
    format!(
        "development loop exited cleanly\nframes-presented: {}\nframe-duration-last-us: {}\nframe-duration-max-us: {}\nframe-duration-average-us: {}\nresizes: {}\ndpi-changes: {}\ninput-events: {}\nnative-reloads: {}\nclose-requested: {}\n",
        summary.frames_presented,
        summary.last_frame_duration_micros,
        summary.max_frame_duration_micros,
        summary.average_frame_duration_micros(),
        summary.resizes,
        summary.dpi_changes,
        summary.input_events,
        summary.native_reloads,
        summary.close_requested
    )
}

fn signed_runtime_artifact_value(
    artifact: &SealedArtifact,
) -> Result<serde_json::Value, CommandExecution> {
    serde_json::to_value(artifact).map_err(|error| {
        CommandExecution::failure(
            CliExitCode::Verification,
            vec![CliDiagnostic::error(
                "artifact.runtime.serialize-failed",
                format!("signed runtime artifact could not be serialized: {error}"),
            )],
        )
    })
}

fn event_debug(event: &crate::DevLoopEvent) -> String {
    format!("{event:?}")
}

fn dev_watched_paths(manifest: &HawkManifest) -> Vec<DevWatchedPath> {
    const PROJECT_INPUTS: &[&str] = &[
        "package.json",
        "bun.lock",
        "package-lock.json",
        "pnpm-lock.yaml",
        "yarn.lock",
        "Cargo.toml",
        "build.rs",
        "src/lib.rs",
        "src/main.rs",
    ];

    let mut paths = BTreeSet::from([
        DevWatchedPath::new(CANONICAL_MANIFEST_FILE, DevWatchKind::Manifest),
        DevWatchedPath::new(
            manifest.source.entry.clone(),
            if manifest.source.framework.is_some() {
                DevWatchKind::Script
            } else {
                DevWatchKind::RuntimeTree
            },
        ),
    ]);
    for path in PROJECT_INPUTS {
        paths.insert(DevWatchedPath::new(*path, DevWatchKind::Manifest));
    }
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
    desktop_runtime_config_from_manifest_and_artifact(
        &output.manifest,
        &output.artifact,
        exit_after_first_frame,
    )
}

fn desktop_runtime_config_from_manifest_and_artifact(
    manifest: &HawkManifest,
    artifact: &SealedArtifact,
    exit_after_first_frame: bool,
) -> Result<WinitDesktopRuntimeConfig, Box<CliDiagnostic>> {
    if let Some(controller) = entry_framework_runtime_controller(artifact)? {
        return Ok(desktop_runtime_config_from_manifest_with_runtime_tree(
            manifest,
            controller.runtime_tree().clone(),
            exit_after_first_frame,
        )?
        .with_framework_controller(controller));
    }
    let Some(script) = entry_script_record(artifact) else {
        let app_model = DesktopEntryAppModel::manifest_fallback(manifest.identity.name.clone());
        return desktop_runtime_config_from_manifest_with_app_model(
            manifest,
            &app_model,
            exit_after_first_frame,
        );
    };
    if let Some(app_model) = entry_script_mount_app_model(script)? {
        let config = desktop_runtime_config_from_manifest_with_app_model(
            manifest,
            &app_model,
            exit_after_first_frame,
        )?;
        return Ok(config.with_script_entry(WinitDesktopScriptEntry::new(
            script.source_path.clone(),
            script.compiled_source.clone(),
            HostSnapshot::default(),
        )));
    }
    let app_model = entry_script_visible_title(script).map_or_else(
        || DesktopEntryAppModel::manifest_fallback(manifest.identity.name.clone()),
        DesktopEntryAppModel::manifest_fallback,
    );
    desktop_runtime_config_from_manifest_with_app_model(
        manifest,
        &app_model,
        exit_after_first_frame,
    )
}

fn desktop_runtime_config_from_manifest_with_runtime_tree(
    manifest: &HawkManifest,
    runtime_tree: RuntimeViewTree,
    exit_after_first_frame: bool,
) -> Result<WinitDesktopRuntimeConfig, Box<CliDiagnostic>> {
    let (width, height) = manifest.editor.as_ref().map_or((960.0, 540.0), |editor| {
        (f64::from(editor.width), f64::from(editor.height))
    });
    runtime_dimension_to_f32(width)?;
    runtime_dimension_to_f32(height)?;
    Ok(WinitDesktopRuntimeConfig::new(DesktopWindowConfig::new(
        manifest.identity.name.clone(),
        SurfaceMetrics::new(width, height, 1.0),
    ))
    .with_runtime_tree(runtime_tree)
    .with_exit_after_first_frame(exit_after_first_frame))
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

fn entry_script_record(artifact: &SealedArtifact) -> Option<&CompiledScriptRecord> {
    artifact
        .compiled_scripts
        .iter()
        .find(|script| script.entrypoint_id == "entry")
}

fn entry_framework_runtime_controller(
    artifact: &SealedArtifact,
) -> Result<Option<FrameworkRuntimeController>, Box<CliDiagnostic>> {
    let Some(framework) = artifact
        .compiled_frameworks
        .iter()
        .find(|framework| framework.entrypoint_id == "entry")
    else {
        return Ok(None);
    };
    let wire = FrameworkNativeProgramWire::from_json(&framework.compiler_artifact_json)
        .map_err(|error| framework_artifact_diagnostic(framework, error.rule(), error.message()))?;
    let program = FrameworkNativeProgram::try_from(wire)
        .map_err(|error| framework_artifact_diagnostic(framework, error.rule(), error.message()))?;
    let authoring = program
        .to_native_authoring_artifact(&framework.source_path, true)
        .map_err(|error| {
            let message = error.diagnostics().first().map_or_else(
                || "native authoring rejected the framework program".to_string(),
                |diagnostic| format!("{}: {}", diagnostic.rule, diagnostic.message),
            );
            framework_artifact_diagnostic(framework, "native.authoring.error", message)
        })?;
    let bridged = NativeRuntimeBridge::new()
        .bridge_artifact(&authoring)
        .map_err(|error| framework_artifact_diagnostic(framework, error.rule(), error.message()))?;
    validate_framework_dynamic_environment(framework, &program, &bridged)?;
    let controller = FrameworkRuntimeController::from_program(&program, bridged).map_err(|error| {
          Box::new(
              CliDiagnostic::error(
                  "runtime.desktop.framework-controller-failed",
                  format!(
                      "compiled {} framework artifact for {} cannot initialize runtime controller ({}): {}",
                      source_framework_label(framework.framework),
                      framework.source_path,
                      error.rule(),
                      error.diagnostic().message()
                  ),
              )
              .file(framework.source_path.clone()),
          )
      })?;
    Ok(Some(controller))
}

fn validate_framework_dynamic_environment(
    framework: &CompiledFrameworkRecord,
    program: &FrameworkNativeProgram,
    artifact: &NativeRuntimeBridgeArtifact,
) -> Result<(), Box<CliDiagnostic>> {
    let dependencies = artifact
        .dynamic_bindings()
        .iter()
        .flat_map(FrameworkDynamicBinding::dependencies)
        .cloned()
        .collect::<BTreeSet<_>>();
    let available_initial_values = program
        .initial_dynamic_values()
        .iter()
        .map(|value| value.name().to_string())
        .collect::<BTreeSet<_>>();
    let missing_dependencies = dependencies
        .difference(&available_initial_values)
        .cloned()
        .collect::<Vec<_>>();
    if !missing_dependencies.is_empty() {
        return Err(Box::new(
            CliDiagnostic::error(
                "runtime.desktop.dynamic-environment-missing",
                format!(
                    "compiled {} framework artifact for {} declares dynamic dependencies [{}] without compiler-provided initial values",
                    source_framework_label(framework.framework),
                    framework.source_path,
                    missing_dependencies.join(", ")
                ),
            )
            .file(framework.source_path.clone()),
          ));
    }
    Ok(())
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

fn framework_artifact_diagnostic(
    framework: &CompiledFrameworkRecord,
    underlying_rule: &str,
    message: impl Into<String>,
) -> Box<CliDiagnostic> {
    Box::new(
        CliDiagnostic::error(
            "runtime.desktop.framework-artifact-invalid",
            format!(
                "compiled {} framework artifact for {} is invalid ({underlying_rule}): {}",
                source_framework_label(framework.framework),
                framework.source_path,
                message.into()
            ),
        )
        .file(framework.source_path.clone()),
    )
}

const fn source_framework_label(framework: SourceFramework) -> &'static str {
    match framework {
        SourceFramework::Native => "native",
        SourceFramework::React => "react",
        SourceFramework::Solid => "solid",
        SourceFramework::Svelte => "svelte",
        SourceFramework::Vue => "vue",
    }
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

fn default_project_files(
    template: CliProjectTemplate,
    package_manager: CliPackageManager,
) -> Vec<(&'static str, String)> {
    match template {
        CliProjectTemplate::Native => vec![
            ("hawk.json", default_manifest().to_owned()),
            ("src/main.ts", default_entry_source().to_owned()),
            ("src/bootstrap.ts", default_bootstrap_source().to_owned()),
            ("styles/main.hawk.css", default_style_source().to_owned()),
            ("assets/logo.svg", default_logo_svg().to_owned()),
            ("README.md", default_project_readme().to_owned()),
        ],
        CliProjectTemplate::ReactApp => vec![
            ("hawk.json", react_app_manifest().to_owned()),
            ("src/App.tsx", react_app_source().to_owned()),
            (
                "package.json",
                react_package_json("hawk2ui-react-app", package_manager),
            ),
            ("README.md", react_app_readme().to_owned()),
        ],
        CliProjectTemplate::ReactPlugin => vec![
            ("hawk.json", react_plugin_manifest().to_owned()),
            ("src/App.tsx", react_plugin_source().to_owned()),
            (
                "package.json",
                react_package_json("hawk2ui-react-plugin", package_manager),
            ),
            ("README.md", react_plugin_readme().to_owned()),
        ],
        CliProjectTemplate::VueApp => vec![
            ("hawk.json", vue_app_manifest(package_manager)),
            ("src/main.ts", vue_main_source().to_owned()),
            ("src/App.vue", vue_app_source().to_owned()),
            ("vite.hawk.config.ts", vue_vite_config().to_owned()),
            (
                "package.json",
                vue_package_json("hawk2ui-vue-app", package_manager),
            ),
            ("README.md", vue_app_readme().to_owned()),
        ],
        CliProjectTemplate::VuePlugin => vec![
            ("hawk.json", vue_plugin_manifest(package_manager)),
            ("src/main.ts", vue_main_source().to_owned()),
            ("src/App.vue", vue_plugin_source().to_owned()),
            ("vite.hawk.config.ts", vue_vite_config().to_owned()),
            (
                "package.json",
                vue_package_json("hawk2ui-vue-plugin", package_manager),
            ),
            ("README.md", vue_plugin_readme().to_owned()),
        ],
    }
}

fn default_manifest() -> &'static str {
    r#"{
  "$schema": "https://hawk2ui.dev/schemas/hawk.schema.json",
  "schemaVersion": 1,
  "package": {
    "id": "com.example.hawk2ui-app",
    "name": "Hawk2UI App",
    "version": "0.1.0",
    "bundleId": "com.example.hawk2ui-app"
  },
  "app": {
    "entry": "src/main.ts",
    "style": "styles/main.hawk.css",
    "script": "src/bootstrap.ts"
  },
  "targets": {
    "desktop": [
      {
        "name": "linux-wayland",
        "platforms": ["linux-wayland"],
        "window": {
          "title": "Hawk2UI App",
          "width": 960,
          "height": 540
        }
      }
    ],
    "plugin": [
      {
        "name": "clap",
        "formats": ["clap", "vst3", "au"],
        "editor": {
          "width": 960,
          "height": 540
        }
      }
    ]
  },
  "plugin": {
    "id": "com.example.hawk2ui-app",
    "name": "Hawk2UI App",
    "parameters": [
      {
        "id": "gain",
        "paramId": 0,
        "name": "Gain",
        "default": 0.5
      },
      {
        "id": "mix",
        "paramId": 1,
        "name": "Mix",
        "default": 0.75
      }
    ]
  },
  "assets": {
    "entries": [
      {
        "id": "logo",
        "kind": "vector",
        "path": "assets/logo.svg"
      }
    ]
  },
  "presets": [
    {
      "id": "init",
      "name": "Init"
    }
  ],
  "permissions": {
    "capabilities": ["native-windowing", "sealed-artifacts"]
  }
}
"#
}

fn react_app_manifest() -> &'static str {
    r#"{
  "$schema": "https://hawk2ui.dev/schemas/hawk.schema.json",
  "schemaVersion": 1,
  "package": {
    "id": "com.example.hawk2ui-react-app",
    "name": "Hawk2UI React App",
    "version": "0.1.0",
    "bundleId": "com.example.hawk2ui-react-app"
  },
  "app": {
    "entry": "src/App.tsx",
    "framework": "react"
  },
  "targets": {
    "desktop": [
      {
        "name": "linux-wayland",
        "platforms": ["linux-wayland"],
        "window": {
          "title": "Hawk2UI React App",
          "width": 960,
          "height": 540
        }
      }
    ]
  },
  "permissions": {
    "capabilities": ["native-windowing", "sealed-artifacts"]
  }
}
"#
}

fn react_plugin_manifest() -> &'static str {
    r#"{
  "$schema": "https://hawk2ui.dev/schemas/hawk.schema.json",
  "schemaVersion": 1,
  "package": {
    "id": "com.example.hawk2ui-react-plugin",
    "name": "Hawk2UI React Plugin",
    "version": "0.1.0",
    "bundleId": "com.example.hawk2ui-react-plugin"
  },
  "app": {
    "entry": "src/App.tsx",
    "framework": "react"
  },
  "targets": {
    "plugin": [
      {
        "name": "clap",
        "formats": ["clap", "vst3", "au"],
        "editor": {
          "width": 960,
          "height": 540
        }
      }
    ]
  },
  "plugin": {
    "id": "com.example.hawk2ui-react-plugin",
    "name": "Hawk2UI React Plugin",
    "parameters": [
      {
        "id": "gain",
        "paramId": 0,
        "name": "Gain",
        "default": 0.5
      }
    ]
  },
  "permissions": {
    "capabilities": ["plugin-parameters", "sealed-artifacts"]
  }
}
"#
}

fn react_app_source() -> &'static str {
    r#"import React, { useState } from "react";
import { createRoot } from "@hawk2ui/react";

function App() {
  const [count, setCount] = useState(0);

  return (
    <view id="react-desktop-root">
      <text id="count">{String(count)}</text>
      <button id="increment" onPointerPress={() => setCount(count + 1)}>
        Increment
      </button>
    </view>
  );
}

createRoot("main").render(<App />);
"#
}

fn react_plugin_source() -> &'static str {
    r#"import React, { useEffect, useState } from "react";
import { createRoot } from "@hawk2ui/react";
import { readParameter, writeParameter } from "hawk:plugin";

function App() {
  const [gain, setGain] = useState(0);

  useEffect(() => {
    readParameter("gain").then(setGain);
  }, []);

  async function boost() {
    await writeParameter("gain", 0.75);
    setGain(await readParameter("gain"));
  }

  return (
    <view id="react-plugin-root">
      <text id="gain">{gain.toFixed(2)}</text>
      <button id="boost" onPointerPress={boost}>
        Boost
      </button>
    </view>
  );
}

createRoot("editor").render(<App />);
"#
}

fn react_package_json(name: &str, package_manager: CliPackageManager) -> String {
    format!(
        r#"{{
  "name": "{name}",
  "private": true,
  "type": "module",
  "packageManager": "{}",
  "scripts": {{
    "build": "hawk2ui build-release",
    "dev": "hawk2ui dev",
    "validate": "hawk2ui validate"
  }},
  "dependencies": {{
    "@hawk2ui/react": "^0.1.0",
    "react": "^19.0.0"
  }}
}}
"#,
        package_manager.package_manager_field()
    )
}

fn package_manager_manifest_value(package_manager: CliPackageManager) -> &'static str {
    match package_manager {
        CliPackageManager::Bun => "bun",
        CliPackageManager::Npm => "npm",
        CliPackageManager::Pnpm => "pnpm",
        CliPackageManager::Yarn => "yarn",
    }
}

fn vue_app_manifest(package_manager: CliPackageManager) -> String {
    let package_manager = package_manager_manifest_value(package_manager);
    format!(
        r#"{{
  "$schema": "https://hawk2ui.dev/schemas/hawk.schema.json",
  "schemaVersion": 1,
  "package": {{
    "id": "com.example.hawk2ui-vue-app",
    "name": "Hawk2UI Vue App",
    "version": "0.1.0",
    "bundleId": "com.example.hawk2ui-vue-app"
  }},
  "app": {{
    "entry": "src/main.ts",
    "framework": "vue"
  }},
  "build": {{
    "packageManager": "{package_manager}",
    "output": "dist/main.js"
  }},
  "targets": {{
    "desktop": [
      {{
        "name": "linux-wayland",
        "platforms": ["windows", "macos", "linux-wayland", "linux-x11"],
        "window": {{
          "title": "Hawk2UI Vue App",
          "width": 960,
          "height": 540
        }}
      }}
    ]
  }},
  "permissions": {{
    "capabilities": ["native-windowing", "sealed-artifacts"]
  }}
}}
"#
    )
}

fn vue_plugin_manifest(package_manager: CliPackageManager) -> String {
    let package_manager = package_manager_manifest_value(package_manager);
    format!(
        r#"{{
  "$schema": "https://hawk2ui.dev/schemas/hawk.schema.json",
  "schemaVersion": 1,
  "package": {{
    "id": "com.example.hawk2ui-vue-plugin",
    "name": "Hawk2UI Vue Plugin",
    "version": "0.1.0",
    "bundleId": "com.example.hawk2ui-vue-plugin"
  }},
  "app": {{
    "entry": "src/main.ts",
    "framework": "vue"
  }},
  "build": {{
    "packageManager": "{package_manager}",
    "output": "dist/main.js"
  }},
  "targets": {{
    "plugin": [
      {{
        "name": "clap",
        "formats": ["clap", "vst3", "au"],
        "editor": {{
          "width": 960,
          "height": 540
        }}
      }}
    ]
  }},
  "plugin": {{
    "id": "com.example.hawk2ui-vue-plugin",
    "name": "Hawk2UI Vue Plugin",
    "parameters": [
      {{
        "id": "gain",
        "paramId": 0,
        "name": "Gain",
        "kind": "float",
        "min": 0.0,
        "max": 1.0,
        "default": 0.5
      }}
    ]
  }},
  "permissions": {{
    "capabilities": ["plugin-host", "plugin-parameters", "audio-dsp", "sealed-artifacts"]
  }}
}}
"#
    )
}

fn vue_main_source() -> &'static str {
    r#"import { createApp } from "@hawk2ui/vue";
import App from "./App.vue";

createApp(App).mount();
"#
}

fn vue_app_source() -> &'static str {
    r#"<script setup lang="ts">
import { computed, ref } from "vue";

const count = ref(0);
const countLabel = computed(() => `Count ${count.value}`);
</script>

<template>
  <hawk-view id="vue-desktop-root">
    <hawk-text id="count">{{ countLabel }}</hawk-text>
    <hawk-button id="increment" @pointer-press="count += 1">Increment</hawk-button>
  </hawk-view>
</template>
"#
}

fn vue_plugin_source() -> &'static str {
    r#"<script setup lang="ts">
import { onMounted, ref } from "vue";
import { readParameter, writeParameter } from "hawk:plugin";

const gain = ref(0);

onMounted(async () => {
  gain.value = await readParameter("gain");
});

async function boost() {
  await writeParameter("gain", 0.75);
  gain.value = await readParameter("gain");
}
</script>

<template>
  <hawk-view id="vue-plugin-root">
    <hawk-text id="gain">{{ gain.toFixed(2) }}</hawk-text>
    <hawk-button id="boost" @pointer-press="boost">Boost</hawk-button>
  </hawk-view>
</template>
"#
}

fn vue_vite_config() -> &'static str {
    r#"import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [vue()],
  build: {
    emptyOutDir: true,
    sourcemap: true,
    lib: {
      entry: "src/main.ts",
      formats: ["es"],
      fileName: () => "main.js",
    },
    rollupOptions: {
      external: [/^hawk:/],
    },
  },
});
"#
}

fn vue_package_json(name: &str, package_manager: CliPackageManager) -> String {
    format!(
        r#"{{
  "name": "{name}",
  "private": true,
  "type": "module",
  "packageManager": "{}",
  "scripts": {{
    "bundle": "vite build --config vite.hawk.config.ts",
    "build": "vite build --config vite.hawk.config.ts",
    "build:artifact": "hawk2ui build-release",
    "dev": "hawk2ui dev",
    "validate": "hawk2ui validate"
  }},
  "dependencies": {{
    "@hawk2ui/vue": "^0.1.0",
    "vue": "^3.5.0"
  }},
  "devDependencies": {{
    "@vitejs/plugin-vue": "^5.0.0",
    "typescript": "^5.0.0",
    "vite": "^6.0.0"
  }}
}}
"#,
        package_manager.package_manager_field()
    )
}

fn react_app_readme() -> &'static str {
    r"# Hawk2UI React App

Generated React desktop app scaffold.

Commands:

- `hawk2ui validate`
- `hawk2ui build-release`
- `hawk2ui run-desktop`
"
}

fn react_plugin_readme() -> &'static str {
    r"# Hawk2UI React Plugin

Generated React plugin editor scaffold.

Commands:

- `hawk2ui validate`
- `hawk2ui build-release`
- `hawk2ui package-plugin`
"
}

fn vue_app_readme() -> &'static str {
    r"# Hawk2UI Vue App

Generated Vue desktop app scaffold.

Commands:

- `hawk2ui validate`
- `npm run build`
- `npm run build:artifact`
- `hawk2ui run-desktop`
"
}

fn vue_plugin_readme() -> &'static str {
    r"# Hawk2UI Vue Plugin

Generated Vue plugin editor scaffold.

Commands:

- `hawk2ui validate`
- `npm run build`
- `npm run build:artifact`
- `hawk2ui package-plugin`
"
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
        ArtifactHash, BuildPipeline, CompiledFrameworkRecord, CompiledScriptRecord, SealedArtifact,
        SourceFramework, VerificationReport,
    };
    use hawk2ui_host_winit::{WinitHostError, WinitPresentationBackendUsed};
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
    fn desktop_runtime_summary_output_includes_gpu_preferred_fallback_reason() {
        let summary = WinitDesktopRuntimeSummary {
            presentation_backend_used: WinitPresentationBackendUsed::Software,
            frames_presented: 2,
            last_frame_duration_micros: 3_000,
            max_frame_duration_micros: 5_000,
            total_frame_duration_micros: 8_000,
            presentation_fallback_reason: Some(WinitHostError::new(
                "desktop.gpu.wayland-required",
                "Winit GPU presentation currently requires a native Wayland display",
            )),
            ..WinitDesktopRuntimeSummary::default()
        };

        let output = desktop_runtime_summary_output(CliPresentationBackend::GpuPreferred, &summary);

        assert!(output.contains("presentation-backend-requested: gpu-preferred"));
        assert!(output.contains("presentation-backend-used: software"));
        assert!(output.contains("frame-duration-last-us: 3000"));
        assert!(output.contains("frame-duration-max-us: 5000"));
        assert!(output.contains("frame-duration-average-us: 4000"));
        assert!(output.contains("frame-duration-total-us: 8000"));
        assert!(output.contains("presentation-fallback-rule: desktop.gpu.wayland-required"));
        assert!(output.contains(
            "presentation-fallback-message: Winit GPU presentation currently requires a native Wayland display"
        ));
    }

    #[test]
    fn package_verification_diagnostics_include_materialized_target_details() {
        let output_root = env::temp_dir().join(format!(
            "hawk2ui-cli-package-verification-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        ));
        let request = PackageRequest::new(
            FormatMetadata::new("com.hawk2ui.cli-diagnostics", "CLI Diagnostics", "Hawk2UI"),
            BundleOutput::new(output_root.to_string_lossy(), "CLI Diagnostics"),
            hawk2ui_plugin::ParameterModel::new([]),
        )
        .with_format(PackageFormat::Clap);
        let plan = PackageAdapterSet::new()
            .plan(&request)
            .expect("package plan succeeds");
        let outputs = plan.materialize().expect("package materializes");
        let report = plan.verify_materialized(&outputs);

        let diagnostics = package_verification_diagnostics(&report);

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.rule == "package.binary-slot.not-loadable"
                && diagnostic.message.contains("CLI Diagnostics.clap")
        }));
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
    fn dev_watched_paths_cover_react_project_package_and_rust_inputs() {
        let manifest = HawkManifest::parse(
            r#"{
  "$schema": "https://hawk2ui.dev/schemas/hawk.schema.json",
  "schemaVersion": 1,
  "package": {
    "id": "com.example.react",
    "name": "React Dev Watch",
    "version": "0.1.0"
  },
  "app": {
    "entry": "src/App.tsx",
    "framework": "react",
    "style": "styles/main.hawk.css"
  },
  "targets": {
    "desktop": [{ "name": "linux-wayland" }]
  },
  "permissions": {
    "capabilities": ["native-windowing"]
  }
}
"#,
        )
        .expect("manifest parses");

        let watched = dev_watched_paths(&manifest)
            .into_iter()
            .map(|path| (path.path().to_owned(), path.kind()))
            .collect::<std::collections::BTreeMap<_, _>>();

        assert_eq!(
            watched.get("src/App.tsx"),
            Some(&DevWatchKind::Script),
            "React entry changes must rebuild framework JS instead of patching the runtime tree"
        );
        assert_eq!(
            watched.get("styles/main.hawk.css"),
            Some(&DevWatchKind::Style)
        );
        for path in [
            "package.json",
            "bun.lock",
            "package-lock.json",
            "pnpm-lock.yaml",
            "yarn.lock",
            "Cargo.toml",
            "build.rs",
            "src/lib.rs",
            "src/main.rs",
        ] {
            assert_eq!(
                watched.get(path),
                Some(&DevWatchKind::Manifest),
                "{path} should be watched as a full-rebuild project input"
            );
        }
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
        let script_entry = config
            .script_entry()
            .expect("desktop config retains executable script entry");
        assert_eq!(script_entry.source_path(), "src/main.ts");
        assert!(script_entry.compiled_source().contains("Hello From Mount"));

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
                font_family,
                font_size,
                color
            } if id.as_str() == "hero-title"
                && text == "Styled Hero"
                && font_family == "Hawk2UI Sans"
                && float_eq(geometry.width, 320.0)
                && float_eq(geometry.height, 40.0)
                && float_eq(*font_size, 18.0)
                && *color == Color::rgba(170, 187, 204, 255)
        )));
    }

    #[test]
    fn desktop_runtime_config_mounts_compiled_framework_artifact() {
        let manifest = HawkManifest::parse(
            r#"[identity]
id = "com.example.framework-desktop"
name = "Manifest Only Title"
version = "1.0.0"

[source]
entry = "src/App.tsx"
framework = "react"

[editor]
width = 640
height = 360

[[targets]]
kind = "desktop"
name = "linux-wayland"
"#,
        )
        .expect("manifest parses");
        let compiler_artifact = r##"{
  "schema_version": 1,
  "root": {
    "id": "framework-root",
    "kind": "view",
    "props": [
      { "name": "background", "value": { "type": "string", "value": "#102030" } }
    ],
    "children": [
      {
        "key": "title",
        "node": {
          "id": "framework-title",
          "kind": "text",
          "props": [
            { "name": "text", "value": { "type": "string", "value": "Hello Framework Runtime" } },
            { "name": "font_size", "value": { "type": "number", "value": 21 } },
            { "name": "color", "value": { "type": "string", "value": "#aabbcc" } },
            { "name": "width", "value": { "type": "number", "value": 360 } },
            { "name": "height", "value": { "type": "number", "value": 48 } }
          ]
        }
      }
    ]
  }
}"##;
        let artifact = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest)
            .with_compiled_framework(
                CompiledFrameworkRecord::new(
                    "entry",
                    SourceFramework::React,
                    "src/App.tsx",
                    "frameworks/entry.hawk.framework.json",
                    ArtifactHash::from_bytes(compiler_artifact.as_bytes()),
                )
                .with_compiler_artifact_json(compiler_artifact),
            );
        let output = BuildWorkspaceOutput {
            manifest,
            pipeline: BuildPipeline::production(),
            artifact,
            verification: VerificationReport::new("com.example.framework-desktop"),
        };

        let config =
            desktop_runtime_config_from_build_output(&output, true).expect("runtime config builds");
        let runtime_tree = config
            .runtime_tree()
            .expect("desktop config carries runtime tree");
        assert_eq!(runtime_tree.root_id().as_str(), "framework-root");

        let scene = RuntimeSceneBridge::new(Viewport::new(640.0, 360.0))
            .build(runtime_tree)
            .expect("runtime scene builds");

        assert!(scene.draw_commands().iter().any(|command| matches!(
            command,
            hawk2ui_runtime::RuntimeDrawCommand::Fill { id, color, .. }
                if id.as_str() == "framework-root" && *color == Color::rgba(16, 32, 48, 255)
        )));
        assert!(scene.draw_commands().iter().any(|command| matches!(
            command,
            hawk2ui_runtime::RuntimeDrawCommand::Text {
                id,
                geometry,
                text,
                font_family,
                font_size,
                color
            } if id.as_str() == "framework-title"
                && text == "Hello Framework Runtime"
                && font_family == "Hawk2UI Sans"
                && float_eq(geometry.width, 360.0)
                && float_eq(geometry.height, 48.0)
                && float_eq(*font_size, 21.0)
                && *color == Color::rgba(170, 187, 204, 255)
        )));
        assert!(!scene.draw_commands().iter().any(|command| matches!(
            command,
            hawk2ui_runtime::RuntimeDrawCommand::Text { text, .. } if text == "Manifest Only Title"
        )));
    }

    #[test]
    fn desktop_runtime_config_applies_dependency_free_framework_dynamic_bindings() {
        let manifest = HawkManifest::parse(
            r#"[identity]
id = "com.example.framework-dynamic"
name = "Framework Dynamic"
version = "1.0.0"

[source]
entry = "src/App.tsx"
framework = "react"

[[targets]]
kind = "desktop"
name = "linux-wayland"
"#,
        )
        .expect("manifest parses");
        let compiler_artifact = r#"{
  "schema_version": 1,
  "root": {
    "id": "root",
    "kind": "view",
    "children": [
      {
        "key": "title",
        "node": {
          "id": "title",
          "kind": "text",
          "props": [
            { "name": "text", "value": { "type": "string", "value": "Static Fallback" } },
            { "name": "width", "value": { "type": "number", "value": 360 } },
            { "name": "height", "value": { "type": "number", "value": 48 } }
          ]
        }
      }
    ]
  },
  "dynamic_bindings": [
    {
      "node_id": "title",
      "target": { "type": "text" },
      "expression": "'Dynamic Initial Text'",
      "dependencies": []
    }
  ]
}"#;
        let artifact = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest)
            .with_compiled_framework(
                CompiledFrameworkRecord::new(
                    "entry",
                    SourceFramework::React,
                    "src/App.tsx",
                    "frameworks/entry.hawk.framework.json",
                    ArtifactHash::from_bytes(compiler_artifact.as_bytes()),
                )
                .with_compiler_artifact_json(compiler_artifact),
            );
        let output = BuildWorkspaceOutput {
            manifest,
            pipeline: BuildPipeline::production(),
            artifact,
            verification: VerificationReport::new("com.example.framework-dynamic"),
        };

        let config =
            desktop_runtime_config_from_build_output(&output, true).expect("runtime config builds");
        let scene = RuntimeSceneBridge::new(Viewport::new(960.0, 540.0))
            .build(
                config
                    .runtime_tree()
                    .expect("desktop config carries runtime tree"),
            )
            .expect("runtime scene builds");

        assert!(scene.draw_commands().iter().any(|command| matches!(
            command,
            hawk2ui_runtime::RuntimeDrawCommand::Text { id, text, .. }
                if id.as_str() == "title" && text == "Dynamic Initial Text"
        )));
        assert!(!scene.draw_commands().iter().any(|command| matches!(
            command,
            hawk2ui_runtime::RuntimeDrawCommand::Text { text, .. } if text == "Static Fallback"
        )));
    }

    #[test]
    fn desktop_runtime_config_applies_framework_initial_dynamic_environment() {
        let manifest = HawkManifest::parse(
            r#"[identity]
id = "com.example.framework-initial-env"
name = "Framework Initial Environment"
version = "1.0.0"

[source]
entry = "src/App.tsx"
framework = "react"

[[targets]]
kind = "desktop"
name = "linux-wayland"
"#,
        )
        .expect("manifest parses");
        let compiler_artifact = r##"{
  "schema_version": 1,
  "root": { "id": "root", "kind": "view", "children": [
    { "key": "panel", "node": { "id": "panel", "kind": "view", "props": [
      { "name": "width", "value": { "type": "number", "value": 360 } },
      { "name": "height", "value": { "type": "number", "value": 120 } },
      { "name": "background", "value": { "type": "string", "value": "#101010" } }
    ] } },
    { "key": "title", "node": { "id": "title", "kind": "text", "props": [
      { "name": "text", "value": { "type": "string", "value": "Static Fallback" } },
      { "name": "font_size", "value": { "type": "number", "value": 16 } },
      { "name": "color", "value": { "type": "string", "value": "#ffffff" } },
      { "name": "width", "value": { "type": "number", "value": 360 } },
      { "name": "height", "value": { "type": "number", "value": 48 } }
    ] } }
  ] },
  "initial_dynamic_values": [
    { "name": "label", "mode": "value", "value": { "type": "string", "value": "Live Title" } },
    { "name": "titleSize", "mode": "value", "value": { "type": "number", "value": 24 } },
    { "name": "titleColor", "mode": "value", "value": { "type": "string", "value": "#33ccff" } },
    { "name": "panelBackground", "mode": "value", "value": { "type": "string", "value": "#ff8800" } }
  ],
  "dynamic_bindings": [
    { "node_id": "title", "target": { "type": "prop", "name": "text" }, "expression": "label", "dependencies": ["label"] },
    { "node_id": "title", "target": { "type": "prop", "name": "font_size" }, "expression": "titleSize", "dependencies": ["titleSize"] },
    { "node_id": "title", "target": { "type": "prop", "name": "color" }, "expression": "titleColor", "dependencies": ["titleColor"] },
    { "node_id": "panel", "target": { "type": "prop", "name": "background" }, "expression": "panelBackground", "dependencies": ["panelBackground"] }
  ]
}"##;
        let artifact = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest)
            .with_compiled_framework(
                CompiledFrameworkRecord::new(
                    "entry",
                    SourceFramework::React,
                    "src/App.tsx",
                    "frameworks/entry.hawk.framework.json",
                    ArtifactHash::from_bytes(compiler_artifact.as_bytes()),
                )
                .with_compiler_artifact_json(compiler_artifact),
            );
        let output = BuildWorkspaceOutput {
            manifest,
            pipeline: BuildPipeline::production(),
            artifact,
            verification: VerificationReport::new("com.example.framework-initial-env"),
        };

        let config =
            desktop_runtime_config_from_build_output(&output, true).expect("runtime config builds");
        let scene = RuntimeSceneBridge::new(Viewport::new(960.0, 540.0))
            .build(
                config
                    .runtime_tree()
                    .expect("desktop config carries runtime tree"),
            )
            .expect("runtime scene builds");

        assert!(scene.draw_commands().iter().any(|command| matches!(
            command,
            hawk2ui_runtime::RuntimeDrawCommand::Fill { id, color, .. }
                if id.as_str() == "panel" && *color == Color::rgba(255, 136, 0, 255)
        )));
        assert!(scene.draw_commands().iter().any(|command| matches!(
            command,
            hawk2ui_runtime::RuntimeDrawCommand::Text {
                id,
                text,
                font_size,
                color,
                ..
            } if id.as_str() == "title"
                && text == "Live Title"
                && float_eq(*font_size, 24.0)
                && *color == Color::rgba(51, 204, 255, 255)
        )));
    }

    #[test]
    fn desktop_runtime_config_rejects_framework_dynamic_bindings_without_initial_environment() {
        let manifest = HawkManifest::parse(
            r#"[identity]
id = "com.example.framework-dynamic-missing-env"
name = "Framework Dynamic Missing Environment"
version = "1.0.0"

[source]
entry = "src/App.tsx"
framework = "react"

[[targets]]
kind = "desktop"
name = "linux-wayland"
"#,
        )
        .expect("manifest parses");
        let compiler_artifact = r#"{
  "schema_version": 1,
  "root": {
    "id": "root",
    "kind": "view",
    "children": [
      {
        "key": "title",
        "node": {
          "id": "title",
          "kind": "text",
          "props": [
            { "name": "text", "value": { "type": "string", "value": "Static Fallback" } }
          ]
        }
      }
    ]
  },
  "dynamic_bindings": [
    {
      "node_id": "title",
      "target": { "type": "text" },
      "expression": "label",
      "dependencies": ["label"]
    }
  ]
}"#;
        let artifact = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest)
            .with_compiled_framework(
                CompiledFrameworkRecord::new(
                    "entry",
                    SourceFramework::React,
                    "src/App.tsx",
                    "frameworks/entry.hawk.framework.json",
                    ArtifactHash::from_bytes(compiler_artifact.as_bytes()),
                )
                .with_compiler_artifact_json(compiler_artifact),
            );
        let output = BuildWorkspaceOutput {
            manifest,
            pipeline: BuildPipeline::production(),
            artifact,
            verification: VerificationReport::new("com.example.framework-dynamic-missing-env"),
        };

        let diagnostic = desktop_runtime_config_from_build_output(&output, true)
            .expect_err("dependency-backed bindings need an explicit initial environment");

        assert_eq!(
            diagnostic.rule,
            "runtime.desktop.dynamic-environment-missing"
        );
        assert!(diagnostic.message.contains("label"));
    }

    #[test]
    fn desktop_runtime_config_accepts_framework_handlers_with_executable_payloads() {
        let manifest = HawkManifest::parse(
            r#"[identity]
id = "com.example.framework-handlers"
name = "Framework Handlers"
version = "1.0.0"

[source]
entry = "src/App.tsx"
framework = "react"

[[targets]]
kind = "desktop"
name = "linux-wayland"
"#,
        )
        .expect("manifest parses");
        let compiler_artifact = r#"{
  "schema_version": 1,
    "root": {
      "id": "root",
      "kind": "view",
      "events": [
        { "kind": "pointer.press", "handler": "handlePress", "payload_fields": ["position"] }
      ]
    },
    "initial_dynamic_values": [
      { "name": "label", "mode": "value", "value": { "type": "string", "value": "Idle" } }
    ],
    "dynamic_bindings": [
      {
        "node_id": "root",
        "target": { "type": "prop", "name": "background" },
        "expression": "label === 'Pressed' ? '#336699' : '#101010'",
        "dependencies": ["label"]
      }
    ],
    "event_handlers": [
      {
        "name": "handlePress",
        "actions": [
          {
            "type": "set_dynamic_value",
            "name": "label",
            "value": { "type": "string", "value": "Pressed" }
          }
        ]
      }
    ]
  }"#;
        let artifact = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest)
            .with_compiled_framework(
                CompiledFrameworkRecord::new(
                    "entry",
                    SourceFramework::React,
                    "src/App.tsx",
                    "frameworks/entry.hawk.framework.json",
                    ArtifactHash::from_bytes(compiler_artifact.as_bytes()),
                )
                .with_compiler_artifact_json(compiler_artifact),
            );
        let output = BuildWorkspaceOutput {
            manifest,
            pipeline: BuildPipeline::production(),
            artifact,
            verification: VerificationReport::new("com.example.framework-handlers"),
        };

        let config = desktop_runtime_config_from_build_output(&output, true)
            .expect("framework handlers with executable payloads build runtime config");

        assert!(config.runtime_tree().is_some());
        let controller = config
            .framework_controller()
            .expect("desktop config retains executable framework controller");
        assert!(controller.has_event_handler("root", "pointer.press"));
    }

    #[test]
    fn desktop_runtime_config_rejects_invalid_compiled_framework_artifact() {
        let manifest = HawkManifest::parse(
            r#"[identity]
id = "com.example.bad-framework-desktop"
name = "Bad Framework"
version = "1.0.0"

[source]
entry = "src/App.svelte"
framework = "svelte"

[[targets]]
kind = "desktop"
name = "linux-wayland"
"#,
        )
        .expect("manifest parses");
        let artifact = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest)
            .with_compiled_framework(
                CompiledFrameworkRecord::new(
                    "entry",
                    SourceFramework::Svelte,
                    "src/App.svelte",
                    "frameworks/entry.hawk.framework.json",
                    ArtifactHash::from_bytes(b"not json"),
                )
                .with_compiler_artifact_json("not json"),
            );
        let output = BuildWorkspaceOutput {
            manifest,
            pipeline: BuildPipeline::production(),
            artifact,
            verification: VerificationReport::new("com.example.bad-framework-desktop"),
        };

        let diagnostic = desktop_runtime_config_from_build_output(&output, true)
            .expect_err("invalid framework artifact should fail before runtime");

        assert_eq!(
            diagnostic.rule,
            "runtime.desktop.framework-artifact-invalid"
        );
        assert!(diagnostic.message.contains("src/App.svelte"));
    }
}
