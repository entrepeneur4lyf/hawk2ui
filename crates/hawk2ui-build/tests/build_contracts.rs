use hawk2ui_build::{
    ArtifactHash, ArtifactSchemaVersion, BuildDiagnostic, BuildDiagnosticSeverity, BuildPhase,
    BuildPipeline, BuildPipelineError, HawkManifest, ManifestError, PackageTarget,
    PackageTargetRecord, SealedArtifact, SealedArtifactError, VerificationReport,
};

const VALID_MANIFEST: &str = r#"
[identity]
id = "com.hawk2ui.desktop-basic"
name = "Desktop Basic"
version = "0.1.0"

[source]
entry = "src/main.ts"

[capabilities]
keys = ["native-windowing", "sealed-artifacts"]

[[targets]]
kind = "desktop"
name = "linux-wayland"

[plugin]
id = "com.hawk2ui.plugin-basic"
name = "Plugin Basic"

[editor]
width = 960
height = 540

[[parameters]]
id = "gain"
name = "Gain"
default = 0.5
"#;

#[test]
fn manifest_validation_accepts_complete_manifest() {
    let manifest = HawkManifest::parse(VALID_MANIFEST).expect("valid manifest parses");

    assert_eq!(manifest.identity.id, "com.hawk2ui.desktop-basic");
    assert!(manifest.has_capability("native-windowing"));
    assert!(manifest.has_target(PackageTarget::Desktop));
    assert_eq!(manifest.parameters.len(), 1);
}

#[test]
fn manifest_validation_accepts_package_assets_entrypoints_and_presets() {
    let input = r#"
[identity]
id = "com.hawk2ui.full"
name = "Full"
version = "1.2.3"

[package]
name = "full"
bundle_id = "com.hawk2ui.full"

[source]
entry = "src/main.ts"
style = "src/style.hawk.css"
script = "src/main.ts"

[capabilities]
keys = ["native-windowing", "assets-read"]

[[assets]]
id = "hero"
kind = "image"
path = "assets/hero.png"

[[targets]]
kind = "desktop"
name = "linux-wayland"

[[presets]]
id = "default"
name = "Default"
"#;

    let manifest = HawkManifest::parse(input).expect("complete production manifest parses");

    assert_eq!(
        manifest.package.as_ref().unwrap().bundle_id,
        "com.hawk2ui.full"
    );
    assert_eq!(manifest.source.style.as_deref(), Some("src/style.hawk.css"));
    assert_eq!(manifest.assets[0].id, "hero");
    assert_eq!(manifest.presets[0].id, "default");
}

#[test]
fn manifest_validation_rejects_duplicate_assets_and_presets() {
    let input = r#"
[identity]
id = "com.hawk2ui.duplicates"
name = "Duplicates"
version = "1.0.0"

[source]
entry = "src/main.ts"

[[assets]]
id = "hero"
kind = "image"
path = "assets/hero.png"

[[assets]]
id = "hero"
kind = "image"
path = "assets/hero-copy.png"

[[presets]]
id = "default"
name = "Default"

[[presets]]
id = "default"
name = "Default Copy"
"#;

    let error = HawkManifest::parse(input).expect_err("duplicate assets must fail first");

    assert_eq!(error, ManifestError::DuplicateAsset("hero".into()));
}

#[test]
fn manifest_validation_rejects_missing_identity() {
    let input = r#"
[source]
entry = "src/main.ts"
"#;

    let error = HawkManifest::parse(input).expect_err("missing identity must fail");

    assert_eq!(error, ManifestError::MissingSection("identity"));
}

#[test]
fn manifest_validation_rejects_duplicate_targets() {
    let input = r#"
[identity]
id = "com.hawk2ui.duplicate"
name = "Duplicate"
version = "0.1.0"

[source]
entry = "src/main.ts"

[[targets]]
kind = "desktop"
name = "linux-wayland"

[[targets]]
kind = "desktop"
name = "linux-wayland"
"#;

    let error = HawkManifest::parse(input).expect_err("duplicate targets must fail");

    assert_eq!(
        error,
        ManifestError::DuplicateTarget("linux-wayland".into())
    );
}

#[test]
fn sealed_artifact_hashes_manifest_snapshot_stably() {
    let manifest = HawkManifest::parse(VALID_MANIFEST).expect("valid manifest parses");
    let artifact = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest);

    assert_eq!(artifact.schema_version, ArtifactSchemaVersion::new(1, 0));
    assert_eq!(
        artifact.manifest_snapshot_hash,
        ArtifactHash::from_bytes(manifest.snapshot().as_bytes())
    );
    assert!(artifact.is_compatible_with(ArtifactSchemaVersion::new(1, 2)));
    assert!(!artifact.is_compatible_with(ArtifactSchemaVersion::new(2, 0)));
}

#[test]
fn sealed_artifact_reports_incompatible_schema_diagnostic() {
    let manifest = HawkManifest::parse(VALID_MANIFEST).expect("valid manifest parses");
    let artifact = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(2, 0), &manifest);

    let error = artifact
        .ensure_compatible_with(ArtifactSchemaVersion::new(1, 0))
        .expect_err("major version mismatch must fail");

    assert_eq!(
        error,
        SealedArtifactError::IncompatibleSchema {
            expected: ArtifactSchemaVersion::new(1, 0),
            actual: ArtifactSchemaVersion::new(2, 0),
            diagnostic: BuildDiagnostic::new(
                BuildDiagnosticSeverity::Error,
                "artifact.schema.incompatible",
                "sealed artifact schema version is incompatible"
            )
        }
    );
}

#[test]
fn build_pipeline_records_required_phase_order() {
    let pipeline = BuildPipeline::production();

    assert_eq!(
        pipeline.phase_names(),
        [
            "source-discovery",
            "manifest-validation",
            "asset-discovery",
            "source-validation",
            "style-compilation",
            "script-compilation",
            "asset-compilation",
            "artifact-generation",
            "packaging",
            "verification",
        ]
    );
}

#[test]
fn build_pipeline_propagates_phase_diagnostics() {
    let pipeline = BuildPipeline::production().with_diagnostic(
        BuildPhase::ManifestValidation,
        BuildDiagnostic::new(
            BuildDiagnosticSeverity::Error,
            "manifest.identity.missing",
            "manifest identity is required",
        ),
    );

    let error = pipeline
        .ensure_release_ready()
        .expect_err("error diagnostic must block release");

    assert_eq!(
        error,
        BuildPipelineError::ReleaseBlocked("manifest.identity.missing".into())
    );
}

#[test]
fn verification_report_tracks_package_targets_and_diagnostics() {
    let report = VerificationReport::new("com.hawk2ui.desktop-basic")
        .with_package_target(PackageTargetRecord::new(
            PackageTarget::Desktop,
            "linux-wayland",
        ))
        .with_diagnostic(BuildDiagnostic::new(
            BuildDiagnosticSeverity::Warning,
            "style.unsupported.warning",
            "style warning",
        ));

    assert_eq!(report.product_id, "com.hawk2ui.desktop-basic");
    assert_eq!(report.package_targets.len(), 1);
    assert_eq!(report.diagnostics.len(), 1);
    assert!(report.is_release_ready());
}
