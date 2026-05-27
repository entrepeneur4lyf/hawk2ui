//! Filesystem-backed project build workspace.

use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use crate::{
    ArtifactHash, ArtifactSchemaVersion, AssetCompilationError, AssetCompilationPlan,
    AssetManifestEntry, AssetSource, AssetSourceIndex, BuildPipeline, BuildPipelineError,
    CompiledAssetRecord, CompiledScriptRecord, CompiledStyleRecord, HawkManifest, ManifestError,
    PackageTargetRecord, SealedArtifact, VerificationReport,
};

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
        let manifest_source = fs::read_to_string(&manifest_path)
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
        let pipeline = BuildPipeline::production();
        pipeline
            .ensure_release_ready()
            .map_err(BuildWorkspaceError::PipelineBlocked)?;

        let mut artifact = SealedArtifact::from_manifest(schema_version, &self.manifest);
        artifact = artifact
            .with_compiled_script(self.compiled_script("entry", &self.manifest.source.entry)?);

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
        Ok(CompiledScriptRecord::new(
            entrypoint_id,
            path,
            format!("scripts/{entrypoint_id}.hawk.js"),
            ArtifactHash::from_bytes(&bytes),
        ))
    }

    fn compiled_style(
        &self,
        entrypoint_id: &str,
        path: &str,
    ) -> Result<CompiledStyleRecord, BuildWorkspaceError> {
        let bytes = self.read_declared_file(path)?;
        Ok(CompiledStyleRecord::new(
            entrypoint_id,
            path,
            format!("styles/{entrypoint_id}.hawk.style"),
            ArtifactHash::from_bytes(&bytes),
        ))
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
        fs::read(&absolute).map_err(|_| BuildWorkspaceError::UnreadableFile(path.into()))
    }
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

/// Filesystem-backed build workspace error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BuildWorkspaceError {
    /// A required workspace file is missing.
    MissingFile(String),
    /// A required workspace file exists but could not be read.
    UnreadableFile(String),
    /// A manifest path attempts to escape the workspace root.
    UnsafePath(String),
    /// Manifest parsing or validation failed.
    ManifestInvalid(ManifestError),
    /// Asset compilation failed.
    AssetCompilation(AssetCompilationError),
    /// Production pipeline verification failed.
    PipelineBlocked(BuildPipelineError),
}
