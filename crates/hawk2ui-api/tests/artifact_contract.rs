use hawk2ui_api::{
    ArtifactCapability, ArtifactHash, ArtifactId, ArtifactManifestSnapshot, ArtifactSchemaVersion,
    CompiledAssetRecord, CompiledScriptRecord, CompiledStyleRecord, TargetMetadata,
};

#[test]
fn artifact_contract_records_manifest_hashes_capabilities_assets_styles_scripts_and_targets() {
    let manifest = ArtifactManifestSnapshot::new(
        ArtifactId::new("com.hawk2ui.demo"),
        ArtifactSchemaVersion::new(1, 4, 0),
        ArtifactHash::new("sha256:manifest"),
    )
    .with_capability(ArtifactCapability::new("native-windowing"))
    .with_asset(CompiledAssetRecord::image(
        "hero",
        ArtifactHash::new("sha256:hero"),
        1024,
        512,
    ))
    .with_style(CompiledStyleRecord::new(
        "main",
        ArtifactHash::new("sha256:style"),
    ))
    .with_script(CompiledScriptRecord::module(
        "app",
        ArtifactHash::new("sha256:script"),
    ))
    .with_target(TargetMetadata::desktop("linux-wayland"));

    assert_eq!(manifest.id().as_str(), "com.hawk2ui.demo");
    assert!(manifest.has_capability("native-windowing"));
    assert_eq!(
        manifest.assets()[0].stable_key(),
        "image:hero:sha256:hero:1024x512"
    );
    assert_eq!(manifest.styles()[0].id(), "main");
    assert_eq!(manifest.scripts()[0].id(), "app");
    assert_eq!(manifest.targets()[0].name(), "linux-wayland");
}

#[test]
fn artifact_contract_allows_older_minor_and_rejects_newer_minor_or_major() {
    let runtime = ArtifactSchemaVersion::new(1, 4, 0);

    assert!(
        runtime
            .ensure_can_read(ArtifactSchemaVersion::new(1, 2, 9))
            .is_ok()
    );
    assert!(
        runtime
            .ensure_can_read(ArtifactSchemaVersion::new(1, 5, 0))
            .is_err()
    );
    assert!(
        runtime
            .ensure_can_read(ArtifactSchemaVersion::new(2, 0, 0))
            .is_err()
    );
}

#[test]
fn artifact_contract_serializes_manifest_snapshot() {
    let manifest = ArtifactManifestSnapshot::new(
        ArtifactId::new("com.hawk2ui.demo"),
        ArtifactSchemaVersion::new(1, 0, 0),
        ArtifactHash::new("sha256:manifest"),
    )
    .with_target(TargetMetadata::plugin("vst3"));

    let json = serde_json::to_string(&manifest).expect("artifact manifest serializes");

    assert!(json.contains("com.hawk2ui.demo"));
    assert!(json.contains("vst3"));
}
