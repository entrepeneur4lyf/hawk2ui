//! Filesystem-backed project build workspace.

use std::{
    env, fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::{
    ArtifactHash, ArtifactSchemaVersion, AssetCompilationError, AssetCompilationPlan,
    AssetManifestEntry, AssetSource, AssetSourceIndex, BuildDiagnostic, BuildDiagnosticSeverity,
    BuildPhase, BuildPipeline, BuildPipelineError, CompiledAssetRecord, CompiledFrameworkRecord,
    CompiledScriptRecord, CompiledStyleRecord, HawkManifest, ManifestError, PackageTargetRecord,
    SealedArtifact, SourceFramework, VerificationReport,
};
use hawk2ui_script::{ScriptBackend, ScriptBackendError, ScriptExecutionLimits, ScriptModule};
use hawk2ui_style::{StyleCompileError, compile_style_source};

const MAX_DECLARED_FILE_BYTES: u64 = 64 * 1024 * 1024;
const FRAMEWORK_COMPILER_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_FRAMEWORK_COMPILER_OUTPUT_BYTES: usize = 1024 * 1024;
const MAX_FRAMEWORK_COMPILER_OUTPUT_READ_BYTES: u64 = 1024 * 1024 + 1;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

trait CommandNoWindow {
    fn no_window(&mut self) -> &mut Self;
}

impl CommandNoWindow for Command {
    #[cfg(target_os = "windows")]
    fn no_window(&mut self) -> &mut Self {
        use std::os::windows::process::CommandExt;
        self.creation_flags(CREATE_NO_WINDOW)
    }

    #[cfg(not(target_os = "windows"))]
    fn no_window(&mut self) -> &mut Self {
        self
    }
}

/// A Hawk project directory loaded from the filesystem.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildWorkspace {
    root: PathBuf,
    manifest: HawkManifest,
}

/// Output produced by building a project workspace.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildWorkspaceOutput {
    /// Parsed and validated manifest.
    pub manifest: HawkManifest,
    /// Production pipeline record used by the build.
    pub pipeline: BuildPipeline,
    /// Sealed artifact produced from manifest, source, style, script, and assets.
    pub artifact: SealedArtifact,
    /// Verification report for release gating.
    pub verification: VerificationReport,
}

impl BuildWorkspace {
    /// Loads and validates a Hawk project workspace from a directory containing `manifest.hawk.toml`.
    ///
    /// # Errors
    ///
    /// Returns [`BuildWorkspaceError`] when the manifest file is missing, unreadable, or invalid.
    pub fn load(root: impl AsRef<Path>) -> Result<Self, BuildWorkspaceError> {
        let requested_root = root.as_ref();
        let manifest_path = requested_root.join("manifest.hawk.toml");
        if !manifest_path.is_file() {
            return Err(BuildWorkspaceError::MissingFile(
                "manifest.hawk.toml".into(),
            ));
        }
        let manifest_source =
            String::from_utf8(read_bounded_file(&manifest_path, "manifest.hawk.toml")?)
                .map_err(|_| BuildWorkspaceError::UnreadableFile("manifest.hawk.toml".into()))?;
        let manifest =
            HawkManifest::parse(&manifest_source).map_err(BuildWorkspaceError::ManifestInvalid)?;
        let root = requested_root
            .canonicalize()
            .map_err(|_| BuildWorkspaceError::UnreadableFile(".".into()))?;
        Ok(Self { root, manifest })
    }

    /// Builds the workspace into a sealed artifact and verification report.
    ///
    /// # Errors
    ///
    /// Returns [`BuildWorkspaceError`] when any declared source, style, script, or asset cannot be
    /// read, fails validation, or blocks the production build pipeline.
    pub fn build(
        self,
        schema_version: ArtifactSchemaVersion,
    ) -> Result<BuildWorkspaceOutput, BuildWorkspaceError> {
        let mut pipeline = BuildPipeline::production();

        let mut artifact = SealedArtifact::from_manifest(schema_version, &self.manifest);
        artifact = match self.manifest.source.framework {
            Some(
                framework @ (SourceFramework::React
                | SourceFramework::Solid
                | SourceFramework::Svelte
                | SourceFramework::Vue),
            ) => artifact.with_compiled_framework(self.compiled_framework(
                "entry",
                framework,
                &self.manifest.source.entry,
            )?),
            Some(SourceFramework::Native) | None => artifact
                .with_compiled_script(self.compiled_script("entry", &self.manifest.source.entry)?),
        };

        if let Some(script) = &self.manifest.source.script
            && script != &self.manifest.source.entry
        {
            artifact = artifact.with_compiled_script(self.compiled_script("script", script)?);
        }

        if let Some(style) = &self.manifest.source.style {
            artifact = artifact.with_compiled_style(self.compiled_style("main", style)?);
        }

        let asset_sources = self.asset_sources()?;
        let asset_records = AssetCompilationPlan::compile_manifest(&self.manifest, &asset_sources)
            .map_err(BuildWorkspaceError::AssetCompilation)?;
        for record in asset_records {
            artifact = artifact
                .with_asset_manifest_entry(AssetManifestEntry::new(
                    &record.id,
                    asset_kind_label(record.kind),
                    &record.package.package_path,
                    record.source_hash.clone(),
                ))
                .with_compiled_asset(CompiledAssetRecord::new(
                    &record.id,
                    &record.source_path,
                    &record.package.package_path,
                    record.source_hash,
                ));
        }

        let verification = self.manifest.targets.iter().fold(
            VerificationReport::new(&self.manifest.identity.id),
            |report, target| {
                report.with_package_target(PackageTargetRecord::new(target.kind, &target.name))
            },
        );
        let verification = self.verify_artifact(verification, &artifact);
        for diagnostic in &verification.diagnostics {
            pipeline = pipeline.with_diagnostic(BuildPhase::Verification, diagnostic.clone());
        }
        pipeline
            .ensure_release_ready()
            .map_err(BuildWorkspaceError::PipelineBlocked)?;

        Ok(BuildWorkspaceOutput {
            manifest: self.manifest,
            pipeline,
            artifact,
            verification,
        })
    }

    fn compiled_script(
        &self,
        entrypoint_id: &str,
        path: &str,
    ) -> Result<CompiledScriptRecord, BuildWorkspaceError> {
        let bytes = self.read_declared_file(path)?;
        let source = String::from_utf8(bytes.clone())
            .map_err(|_| BuildWorkspaceError::UnreadableFile(path.into()))?;
        let module = script_module_from_path(path, entrypoint_id, source)?;
        let compiled_source =
            ScriptBackend::compile_module_source(&module, ScriptExecutionLimits::default())
                .map_err(|error| BuildWorkspaceError::ScriptCompilation {
                    path: path.into(),
                    error,
                })?;
        Ok(CompiledScriptRecord::new(
            entrypoint_id,
            path,
            format!("scripts/{entrypoint_id}.hawk.js"),
            ArtifactHash::from_bytes(&bytes),
        )
        .with_compiled_source(compiled_source))
    }

    fn compiled_style(
        &self,
        entrypoint_id: &str,
        path: &str,
    ) -> Result<CompiledStyleRecord, BuildWorkspaceError> {
        let bytes = self.read_declared_file(path)?;
        let source = String::from_utf8(bytes.clone())
            .map_err(|_| BuildWorkspaceError::UnreadableFile(path.into()))?;
        compile_style_source(&source).map_err(|error| BuildWorkspaceError::StyleCompilation {
            path: path.into(),
            error,
        })?;
        Ok(CompiledStyleRecord::new(
            entrypoint_id,
            path,
            format!("styles/{entrypoint_id}.hawk.style"),
            ArtifactHash::from_bytes(&bytes),
        ))
    }

    fn compiled_framework(
        &self,
        entrypoint_id: &str,
        framework: SourceFramework,
        path: &str,
    ) -> Result<CompiledFrameworkRecord, BuildWorkspaceError> {
        let bytes = self.read_declared_file(path)?;
        let compiler_artifact_json = self.compile_framework_source(framework, path)?;
        serde_json::from_str::<serde_json::Value>(&compiler_artifact_json).map_err(|error| {
            BuildWorkspaceError::FrameworkCompilation {
                path: path.into(),
                framework,
                message: format!("framework compiler emitted invalid JSON: {error}"),
            }
        })?;
        Ok(CompiledFrameworkRecord::new(
            entrypoint_id,
            framework,
            path,
            format!("frameworks/{entrypoint_id}.hawk.framework.json"),
            ArtifactHash::from_bytes(&bytes),
        )
        .with_compiler_artifact_json(compiler_artifact_json))
    }

    fn compile_framework_source(
        &self,
        framework: SourceFramework,
        path: &str,
    ) -> Result<String, BuildWorkspaceError> {
        FrameworkCompilerHost::from_environment()
            .and_then(|host| host.compile(&self.root, framework, path))
            .map_err(|message| BuildWorkspaceError::FrameworkCompilation {
                path: path.into(),
                framework,
                message,
            })
    }

    fn asset_sources(&self) -> Result<AssetSourceIndex, BuildWorkspaceError> {
        self.manifest
            .assets
            .iter()
            .map(|asset| {
                let bytes = self.read_declared_file(&asset.path)?;
                Ok(AssetSource::new(&asset.path, bytes))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(AssetSourceIndex::new)
    }

    fn read_declared_file(&self, path: &str) -> Result<Vec<u8>, BuildWorkspaceError> {
        validate_workspace_relative_path(path)?;
        let absolute = self.root.join(path);
        if !absolute.is_file() {
            return Err(BuildWorkspaceError::MissingFile(path.into()));
        }
        let resolved = absolute
            .canonicalize()
            .map_err(|_| BuildWorkspaceError::UnreadableFile(path.into()))?;
        if !resolved.starts_with(&self.root) {
            return Err(BuildWorkspaceError::UnsafePath(path.into()));
        }
        read_bounded_file(&resolved, path)
    }

    fn verify_artifact(
        &self,
        mut report: VerificationReport,
        artifact: &SealedArtifact,
    ) -> VerificationReport {
        if artifact.compiled_scripts.is_empty() && artifact.compiled_frameworks.is_empty() {
            report = report.with_diagnostic(BuildDiagnostic::new(
                BuildDiagnosticSeverity::Error,
                "verification.entry-payload.missing",
                "artifact must contain at least one compiled script or framework artifact",
            ));
        }
        if self.manifest.source.style.is_some() && artifact.compiled_styles.is_empty() {
            report = report.with_diagnostic(BuildDiagnostic::new(
                BuildDiagnosticSeverity::Error,
                "verification.style.missing",
                "artifact must contain the declared compiled style",
            ));
        }
        for asset in &self.manifest.assets {
            if !artifact
                .asset_manifest
                .iter()
                .any(|entry| entry.id == asset.id)
            {
                report = report.with_diagnostic(BuildDiagnostic::new(
                    BuildDiagnosticSeverity::Error,
                    "verification.asset-manifest.missing",
                    format!("artifact manifest is missing asset `{}`", asset.id),
                ));
            }
            if !artifact
                .compiled_assets
                .iter()
                .any(|compiled| compiled.id == asset.id)
            {
                report = report.with_diagnostic(BuildDiagnostic::new(
                    BuildDiagnosticSeverity::Error,
                    "verification.compiled-asset.missing",
                    format!("artifact is missing compiled asset `{}`", asset.id),
                ));
            }
        }
        report
    }
}

fn read_bounded_file(path: &Path, display_path: &str) -> Result<Vec<u8>, BuildWorkspaceError> {
    let metadata = fs::metadata(path)
        .map_err(|_| BuildWorkspaceError::UnreadableFile(display_path.to_string()))?;
    if metadata.len() > MAX_DECLARED_FILE_BYTES {
        return Err(BuildWorkspaceError::FileTooLarge(display_path.to_string()));
    }
    fs::read(path).map_err(|_| BuildWorkspaceError::UnreadableFile(display_path.to_string()))
}

fn validate_workspace_relative_path(path: &str) -> Result<(), BuildWorkspaceError> {
    let candidate = Path::new(path);
    if candidate.is_absolute()
        || candidate
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(BuildWorkspaceError::UnsafePath(path.into()));
    }
    Ok(())
}

fn asset_kind_label(kind: crate::AssetKind) -> &'static str {
    match kind {
        crate::AssetKind::Image => "image",
        crate::AssetKind::Vector => "vector",
        crate::AssetKind::Font => "font",
        crate::AssetKind::DesignToken => "design-token",
    }
}

fn script_module_from_path(
    path: &str,
    entrypoint_id: &str,
    source: String,
) -> Result<ScriptModule, BuildWorkspaceError> {
    let Some(extension) = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
    else {
        return Err(BuildWorkspaceError::UnsupportedScriptExtension(path.into()));
    };
    match extension {
        "js" | "mjs" | "cjs" => Ok(ScriptModule::javascript(entrypoint_id, source)),
        "ts" | "tsx" | "mts" | "cts" => Ok(ScriptModule::typescript(path, source)),
        _ => Err(BuildWorkspaceError::UnsupportedScriptExtension(path.into())),
    }
}

/// Filesystem-backed build workspace error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BuildWorkspaceError {
    /// A required workspace file is missing.
    MissingFile(String),
    /// A required workspace file exists but could not be read.
    UnreadableFile(String),
    /// A declared workspace file exceeds the build read limit.
    FileTooLarge(String),
    /// A manifest path attempts to escape the workspace root.
    UnsafePath(String),
    /// Manifest parsing or validation failed.
    ManifestInvalid(ManifestError),
    /// Asset compilation failed.
    AssetCompilation(AssetCompilationError),
    /// Script compilation failed.
    ScriptCompilation {
        /// Script path that failed compilation.
        path: String,
        /// Script compiler error.
        error: ScriptBackendError,
    },
    /// Style compilation failed.
    StyleCompilation {
        /// Style path that failed compilation.
        path: String,
        /// Style compiler error.
        error: StyleCompileError,
    },
    /// Framework source compilation failed.
    FrameworkCompilation {
        /// Framework source path that failed compilation.
        path: String,
        /// Framework selected for compilation.
        framework: SourceFramework,
        /// Compiler failure message.
        message: String,
    },
    /// Script file extension is not supported by the production compiler.
    UnsupportedScriptExtension(String),
    /// Production pipeline verification failed.
    PipelineBlocked(BuildPipelineError),
}

fn default_framework_compiler_script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap_or_else(|| Path::new("."))
        .join("packages/hawk2ui-compiler/src/cli.ts")
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FrameworkCompilerHost {
    runner: FrameworkCompilerRunner,
    timeout: Duration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum FrameworkCompilerRunner {
    CompilerBinary(PathBuf),
    BunScript { bun: PathBuf, script: PathBuf },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CompilerProcessOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl FrameworkCompilerHost {
    fn from_environment() -> Result<Self, String> {
        if let Ok(binary) = env::var("HAWK2UI_COMPILER_BIN")
            && !binary.trim().is_empty()
        {
            return Ok(Self {
                runner: FrameworkCompilerRunner::CompilerBinary(PathBuf::from(binary)),
                timeout: FRAMEWORK_COMPILER_TIMEOUT,
            });
        }

        let bun = find_bun_executable().ok_or_else(|| {
            "could not find Bun executable; install Bun, set HAWK2UI_BUN, bundle Bun next to the hawk2ui binary, or set HAWK2UI_COMPILER_BIN to a packaged compiler".to_string()
        })?;
        let script = default_framework_compiler_script();
        if !script.is_file() {
            return Err(format!(
                "default framework compiler script is missing at `{}`; set HAWK2UI_COMPILER_BIN to a packaged compiler",
                script.display()
            ));
        }
        Ok(Self {
            runner: FrameworkCompilerRunner::BunScript { bun, script },
            timeout: FRAMEWORK_COMPILER_TIMEOUT,
        })
    }

    #[cfg(test)]
    fn for_test(bun: PathBuf, script: PathBuf, timeout: Duration) -> Self {
        Self {
            runner: FrameworkCompilerRunner::BunScript { bun, script },
            timeout,
        }
    }

    fn compile(
        &self,
        root: &Path,
        framework: SourceFramework,
        path: &str,
    ) -> Result<String, String> {
        let mut command = self.command();
        command
            .current_dir(root)
            .arg("--framework")
            .arg(source_framework_label(framework))
            .arg("--input")
            .arg(path);
        let process = run_compiler_process(&mut command, self.timeout)?;
        if !process.status.success() {
            return Err(format_compiler_failure(
                process.status,
                &process.stdout,
                &process.stderr,
            ));
        }
        String::from_utf8(process.stdout)
            .map_err(|error| format!("framework compiler emitted non-UTF-8 output: {error}"))
    }

    fn command(&self) -> Command {
        match &self.runner {
            FrameworkCompilerRunner::CompilerBinary(binary) => Command::new(binary),
            FrameworkCompilerRunner::BunScript { bun, script } => {
                let mut command = Command::new(bun);
                command.arg(script);
                command
            }
        }
    }
}

fn find_bun_executable() -> Option<PathBuf> {
    env::var("HAWK2UI_BUN")
        .ok()
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
        .or_else(find_bundled_bun)
        .or_else(|| find_executable_on_path("bun"))
}

fn find_bundled_bun() -> Option<PathBuf> {
    let exe = env::current_exe().ok()?;
    let dir = exe.parent()?;
    bun_executable_candidates("bun")
        .into_iter()
        .map(|candidate| dir.join(candidate))
        .find(|candidate| candidate.is_file())
}

fn find_executable_on_path(name: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|path| {
        env::split_paths(&path)
            .flat_map(|dir| {
                bun_executable_candidates(name)
                    .into_iter()
                    .map(move |candidate| dir.join(candidate))
            })
            .find(|candidate| candidate.is_file())
    })
}

fn bun_executable_candidates(name: &str) -> Vec<String> {
    if cfg!(windows) {
        vec![
            format!("{name}.exe"),
            format!("{name}.cmd"),
            format!("{name}.bat"),
            name.to_string(),
        ]
    } else {
        vec![name.to_string()]
    }
}

fn run_compiler_process(
    command: &mut Command,
    timeout: Duration,
) -> Result<CompilerProcessOutput, String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .no_window();
    let command_label = format!("{command:?}");
    let mut child = command.spawn().map_err(|error| {
        format!("failed to launch framework compiler `{command_label}`: {error}")
    })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture framework compiler stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "failed to capture framework compiler stderr".to_string())?;
    let stdout_reader = thread::spawn(move || read_bounded_pipe(stdout));
    let stderr_reader = thread::spawn(move || read_bounded_pipe(stderr));
    let start = Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return Ok(CompilerProcessOutput {
                    status,
                    stdout: join_reader(stdout_reader),
                    stderr: join_reader(stderr_reader),
                });
            }
            Ok(None) if start.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                let stdout = join_reader(stdout_reader);
                let stderr = join_reader(stderr_reader);
                return Err(format!(
                    "framework compiler timed out after {} ms\nstdout:\n{}\nstderr:\n{}",
                    timeout.as_millis(),
                    decode_process_output(&stdout),
                    decode_process_output(&stderr)
                ));
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "failed to wait for framework compiler `{command_label}`: {error}"
                ));
            }
        }
    }
}

fn read_bounded_pipe(mut reader: impl Read) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(MAX_FRAMEWORK_COMPILER_OUTPUT_READ_BYTES)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_FRAMEWORK_COMPILER_OUTPUT_BYTES {
        bytes.truncate(MAX_FRAMEWORK_COMPILER_OUTPUT_BYTES);
        bytes.extend_from_slice(b"\n[hawk2ui: framework compiler output truncated]\n");
    }
    Ok(bytes)
}

fn join_reader(handle: thread::JoinHandle<io::Result<Vec<u8>>>) -> Vec<u8> {
    match handle.join() {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(error)) => {
            format!("[hawk2ui: failed to read compiler output: {error}]").into_bytes()
        }
        Err(_) => b"[hawk2ui: compiler output reader panicked]".to_vec(),
    }
}

fn format_compiler_failure(status: ExitStatus, stdout: &[u8], stderr: &[u8]) -> String {
    let stderr = decode_process_output(stderr);
    let stdout = decode_process_output(stdout);
    let detail = if stderr.trim().is_empty() {
        stdout.trim().to_string()
    } else {
        stderr.trim().to_string()
    };
    if detail.is_empty() {
        format!("framework compiler exited with status {status}")
    } else {
        format!("framework compiler exited with status {status}: {detail}")
    }
}

fn decode_process_output(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn source_framework_label(framework: SourceFramework) -> &'static str {
    match framework {
        SourceFramework::Native => "native",
        SourceFramework::React => "react",
        SourceFramework::Solid => "solid",
        SourceFramework::Svelte => "svelte",
        SourceFramework::Vue => "vue",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn framework_compiler_host_uses_explicit_bun_executable_and_captures_stdout() {
        let root = temp_dir("compiler-explicit-bun");
        let bun = fake_executable_path(&root, "fake-bun");
        let script = root.join("compiler.ts");
        let input = root.join("src/App.tsx");
        let args_log = root.join("args.txt");
        write_file(&script, "unused");
        write_file(&input, "export const app = 'hawk';");
        write_successful_fake_bun(&bun, &args_log);

        let host = FrameworkCompilerHost::for_test(
            bun.clone(),
            script.clone(),
            std::time::Duration::from_secs(5),
        );
        let output = host
            .compile(&root, SourceFramework::React, "src/App.tsx")
            .expect("fake Bun compiler should succeed");

        assert!(output.contains("\"kind\":\"react\""));
        let args = fs::read_to_string(args_log).expect("fake Bun args should be captured");
        assert!(
            args.contains(script.to_string_lossy().as_ref()),
            "compiler script path should be passed to Bun: {args}"
        );
        assert!(args.contains("--framework"), "{args}");
        assert!(args.contains("react"), "{args}");
        assert!(args.contains("--input"), "{args}");
        assert!(args.contains("src/App.tsx"), "{args}");
    }

    #[test]
    fn framework_compiler_host_reports_timeout_with_captured_output() {
        let root = temp_dir("compiler-timeout");
        let bun = fake_executable_path(&root, "slow-bun");
        let script = root.join("compiler.ts");
        write_file(&script, "unused");
        write_slow_fake_bun(&bun);

        let host =
            FrameworkCompilerHost::for_test(bun, script, std::time::Duration::from_millis(50));
        let error = host
            .compile(&root, SourceFramework::Svelte, "src/App.svelte")
            .expect_err("slow compiler should time out");

        assert!(error.contains("timed out"), "{error}");
        assert!(error.contains("starting compiler"), "{error}");
        assert!(error.contains("still compiling"), "{error}");
    }

    fn temp_dir(label: &str) -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let root = env::temp_dir().join(format!("hawk2ui-build-{label}-{now}"));
        fs::create_dir_all(&root).expect("test temp directory should be created");
        root
    }

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("test parent directory should be created");
        }
        fs::write(path, contents).expect("test file should be written");
    }

    fn fake_executable_path(root: &Path, name: &str) -> PathBuf {
        if cfg!(windows) {
            root.join(format!("{name}.cmd"))
        } else {
            root.join(name)
        }
    }

    fn write_successful_fake_bun(path: &Path, args_log: &Path) {
        if cfg!(windows) {
            write_file(
                path,
                &format!(
                    "@echo off\r\necho %* > \"{}\"\r\necho {{\"kind\":\"react\",\"root\":{{\"id\":\"root\"}}}}\r\n",
                    args_log.display()
                ),
            );
        } else {
            write_file(
                path,
                &format!(
                    "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nprintf '{{\"kind\":\"react\",\"root\":{{\"id\":\"root\"}}}}\\n'\n",
                    args_log.display()
                ),
            );
        }
        make_executable(path);
    }

    fn write_slow_fake_bun(path: &Path) {
        if cfg!(windows) {
            write_file(
                path,
                "@echo off\r\necho starting compiler\r\necho still compiling 1>&2\r\nping -n 6 127.0.0.1 > nul\r\n",
            );
        } else {
            write_file(
                path,
                "#!/bin/sh\nprintf 'starting compiler\\n'\nprintf 'still compiling\\n' >&2\nsleep 5\n",
            );
        }
        make_executable(path);
    }

    #[cfg(unix)]
    fn make_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .expect("test script metadata should be readable")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("test script should be executable");
    }

    #[cfg(not(unix))]
    fn make_executable(_path: &Path) {}
}
