//! Filesystem-backed project build workspace.

use std::{
    env, fs,
    path::{Component, Path, PathBuf},
    process::Command,
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
        let mut command = framework_compiler_command();
        command
            .current_dir(&self.root)
            .arg("--framework")
            .arg(source_framework_label(framework))
            .arg("--input")
            .arg(path);
        let output =
            command
                .output()
                .map_err(|error| BuildWorkspaceError::FrameworkCompilation {
                    path: path.into(),
                    framework,
                    message: format!("failed to launch framework compiler: {error}"),
                })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let message = if stderr.trim().is_empty() {
                stdout.trim().to_string()
            } else {
                stderr.trim().to_string()
            };
            return Err(BuildWorkspaceError::FrameworkCompilation {
                path: path.into(),
                framework,
                message,
            });
        }
        String::from_utf8(output.stdout).map_err(|error| {
            BuildWorkspaceError::FrameworkCompilation {
                path: path.into(),
                framework,
                message: format!("framework compiler emitted non-UTF-8 output: {error}"),
            }
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

fn framework_compiler_command() -> Command {
    if let Ok(binary) = env::var("HAWK2UI_COMPILER_BIN") {
        return Command::new(binary);
    }
    let mut command = Command::new("bun");
    command.arg(default_framework_compiler_script());
    command
}

fn default_framework_compiler_script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap_or_else(|| Path::new("."))
        .join("packages/hawk2ui-compiler/src/cli.ts")
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
