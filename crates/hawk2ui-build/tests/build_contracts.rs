use hawk2ui_build::{
    ArtifactHash, ArtifactSchemaVersion, BuildDiagnostic, BuildDiagnosticSeverity, HawkManifest,
    ManifestError, PackageTarget, SealedArtifact, SealedArtifactError,
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
