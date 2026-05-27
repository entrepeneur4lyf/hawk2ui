use hawk2ui_api::{Diagnostic, DiagnosticSeverity};
use hawk2ui_build::{
    ArtifactHash, ArtifactSchemaVersion, AssetCompilationError, AssetCompilationPlan,
    AssetDimensions, AssetKind, AssetManifestEntry, AssetSanitizationStatus, AssetSource,
    AssetSourceIndex, BuildDiagnostic, BuildDiagnosticSeverity, BuildPhase, BuildPipeline,
    BuildPipelineError, BuildWorkspace, BuildWorkspaceError, CompiledAssetRecord,
    CompiledScriptRecord, CompiledStyleRecord, HawkManifest, ManifestError, PackageTarget,
    PackageTargetRecord, SealedArtifact, SealedArtifactError, SourceSpan, VerificationReport,
};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn build_diagnostic_converts_to_shared_diagnostic_with_location_context() {
    let diagnostic = Diagnostic::from(
        BuildDiagnostic::new(
            BuildDiagnosticSeverity::Warning,
            "build.asset.large",
            "asset is large",
        )
        .with_location("src/app.ts", SourceSpan::new(10, 25)),
    );

    assert_eq!(diagnostic.severity, DiagnosticSeverity::Warning);
    assert_eq!(diagnostic.rule.as_str(), "build.asset.large");
    assert_eq!(diagnostic.message, "asset is large");
    assert!(
        diagnostic
            .related
            .iter()
            .any(|context| context.label == "file" && context.value == "src/app.ts")
    );
    assert!(
        diagnostic
            .related
            .iter()
            .any(|context| context.label == "span" && context.value == "10..25")
    );
}

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
fn manifest_validation_rejects_schema_invalid_unknown_fields() {
    let input = r#"[identity]
id = "com.example.schema-invalid"
name = "Schema Invalid"
version = "1.0.0"

[source]
entry = "src/main.ts"

[unknown]
enabled = true

[[targets]]
kind = "desktop"
name = "desktop"
"#;

    let error = HawkManifest::parse(input).expect_err("unknown manifest sections must fail schema");

    match error {
        ManifestError::SchemaValidation { path, message } => {
            assert_eq!(path, "");
            assert!(message.contains("unknown"));
        }
        other => panic!("expected schema validation error, got {other:?}"),
    }
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
    assert_eq!(artifact.manifest_snapshot, manifest.snapshot());
    assert_eq!(
        artifact.manifest_snapshot_hash,
        ArtifactHash::from_bytes(manifest.snapshot().as_bytes())
    );
    assert!(artifact.is_compatible_with(ArtifactSchemaVersion::new(1, 2)));
    assert!(!artifact.is_compatible_with(ArtifactSchemaVersion::new(2, 0)));
}

#[test]
fn sealed_artifact_carries_compiled_records_and_metadata() {
    let manifest = HawkManifest::parse(VALID_MANIFEST).expect("valid manifest parses");
    let artifact = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest)
        .with_compiled_script(CompiledScriptRecord::new(
            "main",
            "src/main.ts",
            "scripts/main.hawk.js",
            ArtifactHash::from_bytes(b"script"),
        ))
        .with_compiled_style(CompiledStyleRecord::new(
            "main",
            "src/style.hawk.css",
            "styles/main.hawk.style",
            ArtifactHash::from_bytes(b"style"),
        ))
        .with_asset_manifest_entry(AssetManifestEntry::new(
            "hero",
            "image",
            "assets/hero.png",
            ArtifactHash::from_bytes(b"asset"),
        ))
        .with_compiled_asset(CompiledAssetRecord::new(
            "hero",
            "assets/hero.png",
            "assets/hero.pack",
            ArtifactHash::from_bytes(b"asset"),
        ));

    assert_eq!(artifact.compiled_scripts.len(), 1);
    assert_eq!(artifact.compiled_styles.len(), 1);
    assert_eq!(artifact.asset_manifest.len(), 1);
    assert_eq!(artifact.compiled_assets.len(), 1);
    assert_eq!(
        artifact.capabilities,
        vec![
            "native-windowing".to_string(),
            "sealed-artifacts".to_string()
        ]
    );
    assert_eq!(artifact.hashes.manifest, artifact.manifest_snapshot_hash);
    assert_eq!(artifact.build_metadata.generator, "hawk2ui-build");
    assert_eq!(artifact.target_metadata[0].kind, PackageTarget::Desktop);
    assert_eq!(artifact.target_metadata[0].name, "linux-wayland");
}

#[test]
fn sealed_artifact_generates_and_validates_json_schema() {
    let manifest = HawkManifest::parse(VALID_MANIFEST).expect("valid manifest parses");
    let artifact = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest)
        .with_compiled_script(CompiledScriptRecord::new(
            "main",
            "src/main.ts",
            "scripts/main.hawk.js",
            ArtifactHash::from_bytes(b"script"),
        ))
        .with_asset_manifest_entry(AssetManifestEntry::new(
            "hero",
            "image",
            "assets/hero.png",
            ArtifactHash::from_bytes(b"asset"),
        ));
    let artifact_json = serde_json::to_value(&artifact).expect("artifact serializes");

    let schema = SealedArtifact::json_schema().expect("artifact schema generates");
    SealedArtifact::validate_json(&artifact_json).expect("artifact schema accepts artifact JSON");

    let schema_text = schema.to_string();
    assert!(schema_text.contains("manifest_snapshot_hash"));
    assert!(schema_text.contains("compiled_scripts"));
    assert!(schema_text.contains("asset_manifest"));
    assert!(schema_text.contains("target_metadata"));
}

#[test]
fn sealed_artifact_content_hash_changes_when_compiled_payload_changes() {
    let manifest = HawkManifest::parse(VALID_MANIFEST).expect("valid manifest parses");
    let first = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest)
        .with_compiled_script(CompiledScriptRecord::new(
            "main",
            "src/main.ts",
            "scripts/main.hawk.js",
            ArtifactHash::from_bytes(b"script-a"),
        ));
    let second = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest)
        .with_compiled_script(CompiledScriptRecord::new(
            "main",
            "src/main.ts",
            "scripts/main.hawk.js",
            ArtifactHash::from_bytes(b"script-a"),
        ));
    let changed = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest)
        .with_compiled_script(CompiledScriptRecord::new(
            "main",
            "src/main.ts",
            "scripts/main.hawk.js",
            ArtifactHash::from_bytes(b"script-b"),
        ));

    assert_eq!(first.content_hash(), second.content_hash());
    assert_ne!(first.content_hash(), changed.content_hash());
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
fn pipeline_phases_expose_required_phase_records() {
    let pipeline = BuildPipeline::production();

    assert_eq!(pipeline.phases.len(), 10);
    assert_eq!(
        pipeline
            .phase(BuildPhase::StyleCompilation)
            .expect("style phase must exist")
            .phase,
        BuildPhase::StyleCompilation
    );
    assert!(
        pipeline
            .phases
            .iter()
            .all(|record| record.diagnostics.is_empty())
    );
}

#[test]
fn pipeline_phases_collect_release_blocking_diagnostics_by_phase() {
    let diagnostic = BuildDiagnostic::new(
        BuildDiagnosticSeverity::Error,
        "script.unsupported.syntax",
        "script syntax is unsupported",
    );
    let pipeline = BuildPipeline::production()
        .with_diagnostic(BuildPhase::ScriptCompilation, diagnostic.clone());

    let blockers = pipeline.release_blocking_diagnostics();

    assert_eq!(blockers.len(), 1);
    assert_eq!(blockers[0].phase, BuildPhase::ScriptCompilation);
    assert_eq!(blockers[0].diagnostic, diagnostic);
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
fn asset_compilation_records_metadata_for_supported_asset_kinds() {
    let input = r#"
[identity]
id = "com.hawk2ui.assets"
name = "Assets"
version = "1.0.0"

[source]
entry = "src/main.ts"

[[assets]]
id = "hero"
kind = "image"
path = "assets/hero.png"

[[assets]]
id = "logo"
kind = "vector"
path = "assets/logo.svg"

[[assets]]
id = "display"
kind = "font"
path = "assets/display.otf"

[[assets]]
id = "theme"
kind = "design-token"
path = "tokens/theme.json"
"#;
    let manifest = HawkManifest::parse(input).expect("asset manifest parses");
    let index = AssetSourceIndex::new([
        AssetSource::new("assets/hero.png", b"hero")
            .with_dimensions(AssetDimensions::new(1920, 1080)),
        AssetSource::new("assets/logo.svg", b"logo"),
        AssetSource::new("assets/display.otf", b"font"),
        AssetSource::new("tokens/theme.json", b"theme"),
    ]);

    let records = AssetCompilationPlan::compile_manifest(&manifest, &index)
        .expect("all declared assets compile");

    assert_eq!(records.len(), 4);
    assert_eq!(records[0].kind, AssetKind::Image);
    assert_eq!(
        records[0].dimensions,
        Some(AssetDimensions::new(1920, 1080))
    );
    assert_eq!(records[0].sanitization, AssetSanitizationStatus::Clean);
    assert_eq!(records[0].package.package_path, "assets/hero.pack");
    assert!(records[0].package.cache_key.starts_with("image:hero:"));
    assert_eq!(records[1].kind, AssetKind::Vector);
    assert_eq!(records[2].kind, AssetKind::Font);
    assert_eq!(records[3].kind, AssetKind::DesignToken);
}

#[test]
fn asset_compilation_reports_missing_asset() {
    let input = r#"
[identity]
id = "com.hawk2ui.missing-asset"
name = "Missing Asset"
version = "1.0.0"

[source]
entry = "src/main.ts"

[[assets]]
id = "hero"
kind = "image"
path = "assets/hero.png"
"#;
    let manifest = HawkManifest::parse(input).expect("asset manifest parses");

    let error = AssetCompilationPlan::compile_manifest(&manifest, &AssetSourceIndex::empty())
        .expect_err("missing assets must fail");

    assert_eq!(
        error,
        AssetCompilationError::MissingAsset {
            id: "hero".into(),
            path: "assets/hero.png".into(),
            diagnostic: Box::new(BuildDiagnostic::new(
                BuildDiagnosticSeverity::Error,
                "asset.missing",
                "declared asset source is missing"
            ))
        }
    );
}

#[test]
fn asset_compilation_rejects_unsafe_asset() {
    let input = r#"
[identity]
id = "com.hawk2ui.unsafe-asset"
name = "Unsafe Asset"
version = "1.0.0"

[source]
entry = "src/main.ts"

[[assets]]
id = "hero"
kind = "image"
path = "assets/hero.png"
"#;
    let manifest = HawkManifest::parse(input).expect("asset manifest parses");
    let index =
        AssetSourceIndex::new([AssetSource::new("assets/hero.png", b"hero").unsafe_asset()]);

    let error = AssetCompilationPlan::compile_manifest(&manifest, &index)
        .expect_err("unsafe assets must fail");

    assert_eq!(
        error,
        AssetCompilationError::UnsafeAsset {
            id: "hero".into(),
            path: "assets/hero.png".into(),
            diagnostic: Box::new(BuildDiagnostic::new(
                BuildDiagnosticSeverity::Error,
                "asset.unsafe",
                "declared asset failed safety validation"
            ))
        }
    );
}

#[test]
fn asset_compilation_cache_metadata_changes_when_source_changes() {
    let input = r#"
[identity]
id = "com.hawk2ui.cache-asset"
name = "Cache Asset"
version = "1.0.0"

[source]
entry = "src/main.ts"

[[assets]]
id = "hero"
kind = "image"
path = "assets/hero.png"
"#;
    let manifest = HawkManifest::parse(input).expect("asset manifest parses");
    let first = AssetSourceIndex::new([AssetSource::new("assets/hero.png", b"first")]);
    let second = AssetSourceIndex::new([AssetSource::new("assets/hero.png", b"second")]);

    let first_records =
        AssetCompilationPlan::compile_manifest(&manifest, &first).expect("first asset compiles");
    let second_records =
        AssetCompilationPlan::compile_manifest(&manifest, &second).expect("second asset compiles");

    assert_ne!(
        first_records[0].package.cache_key,
        second_records[0].package.cache_key
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

#[test]
fn verification_report_snapshots_diagnostics_with_locations() {
    let report = VerificationReport::new("com.hawk2ui.report")
        .with_package_target(PackageTargetRecord::new(
            PackageTarget::Desktop,
            "linux-wayland",
        ))
        .with_invalid_manifest(
            "Hawk.toml",
            SourceSpan::new(0, 12),
            "manifest identity is invalid",
        )
        .with_unsupported_style("src/app.css", SourceSpan::new(13, 21))
        .with_unsupported_script("src/app.ts", SourceSpan::new(22, 34))
        .with_unsafe_asset("assets/hero.svg", SourceSpan::new(35, 46))
        .with_missing_asset("assets/missing.png", SourceSpan::new(47, 58))
        .with_undeclared_capability("native-windowing", SourceSpan::new(59, 74))
        .with_target_incompatibility("linux-wayland", SourceSpan::new(75, 88));

    assert_eq!(
        report.render_text(),
        "\
product: com.hawk2ui.report
targets:
- desktop linux-wayland
diagnostics:
- error manifest.invalid Hawk.toml:0..12 manifest identity is invalid
- error style.unsupported src/app.css:13..21 style entrypoint is unsupported
- error script.unsupported src/app.ts:22..34 script entrypoint is unsupported
- error asset.unsafe assets/hero.svg:35..46 asset failed safety validation
- error asset.missing assets/missing.png:47..58 asset source is missing
- error capability.undeclared <manifest>:59..74 capability is not declared: native-windowing
- error target.incompatible <manifest>:75..88 target is incompatible: linux-wayland
"
    );
    assert!(!report.is_release_ready());
}

#[test]
fn build_workspace_reads_project_files_and_materializes_sealed_artifact() {
    let root = temp_build_workspace("complete");
    write_file(
        &root.join("manifest.hawk.toml"),
        r#"
[identity]
id = "com.hawk2ui.workspace"
name = "Workspace"
version = "1.0.0"

[source]
entry = "src/main.ts"
style = "styles/main.hawk.css"
script = "src/bootstrap.ts"

[capabilities]
keys = ["native-windowing", "sealed-artifacts"]

[[targets]]
kind = "desktop"
name = "linux-wayland"

[[assets]]
id = "logo"
kind = "vector"
path = "assets/logo.svg"
"#,
    );
    write_file(&root.join("src/main.ts"), "export const app = 'hawk';");
    write_file(&root.join("src/bootstrap.ts"), "export const boot = true;");
    write_file(
        &root.join("styles/main.hawk.css"),
        ".root { color: white; }",
    );
    write_file(&root.join("assets/logo.svg"), "<svg />");

    let output = BuildWorkspace::load(&root)
        .and_then(|workspace| workspace.build(ArtifactSchemaVersion::new(1, 0)))
        .expect("workspace should build from real files");

    assert_eq!(output.manifest.identity.id, "com.hawk2ui.workspace");
    assert!(output.pipeline.ensure_release_ready().is_ok());
    assert!(output.verification.is_release_ready());
    assert_eq!(output.artifact.compiled_scripts.len(), 2);
    assert_eq!(output.artifact.compiled_styles.len(), 1);
    assert_eq!(output.artifact.asset_manifest.len(), 1);
    assert_eq!(output.artifact.compiled_assets.len(), 1);
    assert_eq!(
        output.artifact.compiled_scripts[0].source_hash,
        ArtifactHash::from_bytes(b"export const app = 'hawk';")
    );
    assert_eq!(
        output.artifact.compiled_scripts[0].compiled_source,
        "export const app = 'hawk';"
    );
    assert_eq!(
        output.artifact.compiled_styles[0].source_path,
        "styles/main.hawk.css"
    );
    assert_eq!(
        output.artifact.asset_manifest[0].artifact_path,
        "assets/logo.pack"
    );
}

#[test]
fn build_workspace_rejects_missing_declared_source_file() {
    let root = temp_build_workspace("missing-source");
    write_file(
        &root.join("manifest.hawk.toml"),
        r#"
[identity]
id = "com.hawk2ui.missing-source"
name = "Missing Source"
version = "1.0.0"

[source]
entry = "src/main.ts"
"#,
    );

    let error = BuildWorkspace::load(&root)
        .and_then(|workspace| workspace.build(ArtifactSchemaVersion::new(1, 0)))
        .expect_err("missing source must fail");

    assert_eq!(
        error,
        BuildWorkspaceError::MissingFile("src/main.ts".into())
    );
}

#[cfg(unix)]
#[test]
fn build_workspace_rejects_symlinked_declared_files_outside_workspace() {
    let root = temp_build_workspace("symlink-escape");
    let outside = temp_build_workspace("symlink-outside");
    write_file(&outside.join("secret.ts"), "export const secret = true;");
    write_file(
        &root.join("manifest.hawk.toml"),
        r#"
[identity]
id = "com.hawk2ui.symlink"
name = "Symlink"
version = "1.0.0"

[source]
entry = "src/main.ts"
"#,
    );
    fs::create_dir_all(root.join("src")).expect("source directory should be created");
    std::os::unix::fs::symlink(outside.join("secret.ts"), root.join("src/main.ts"))
        .expect("test symlink should be created");

    let error = BuildWorkspace::load(&root)
        .and_then(|workspace| workspace.build(ArtifactSchemaVersion::new(1, 0)))
        .expect_err("symlink escape must fail");

    assert_eq!(error, BuildWorkspaceError::UnsafePath("src/main.ts".into()));
}

fn temp_build_workspace(label: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("hawk2ui-build-{label}-{now}"));
    fs::create_dir_all(&root).expect("temp build workspace should be created");
    root
}

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("test parent directory should be created");
    }
    fs::write(path, contents).expect("test file should be written");
}
