use clap_sys::ext::params::{CLAP_PARAM_IS_AUTOMATABLE, CLAP_PARAM_IS_STEPPED};
use hawk2ui_build::{
    ArtifactHash, ArtifactSchemaVersion, ArtifactSignatureVerifier, ArtifactSigningKey,
    CompiledScriptRecord, HawkManifest, SealedArtifact,
};
use hawk2ui_plugin::{
    BundleOutput, EnumVariant, FormatMetadata, ParameterFlags, ParameterModel, ParameterRange,
    ParameterRecord, PluginEditor, PluginEditorSize,
};
use hawk2ui_plugin_adapters::{
    ClapCdylibScaffold, ClapGuiParentHandle, ClapGuiWindowApi, ClapPluginEntryPlan,
    ClapRuntimeEditorDescriptor, ClapRuntimeEditorSession, MaterializedPackageOutput,
    PackageAdapterSet, PackageFormat, PackagePlan, PackageRequest, VerificationReport,
    VerificationStatus,
};
use hawk2ui_runtime::{RuntimeDrawCommand, RuntimeViewId};
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn plugin_adapters_generate_all_supported_package_targets() {
    let metadata = FormatMetadata::new("com.hawk2ui.demo", "Demo", "Hawk2UI").version("1.2.3");
    let parameters = ParameterModel::new([ParameterRecord::numeric(
        "gain",
        "Gain",
        "dB",
        ParameterRange::new(-60.0, 12.0, 0.0),
    )]);
    let request = PackageRequest::new(metadata, BundleOutput::new("dist", "Demo"), parameters)
        .with_format(PackageFormat::Clap)
        .with_format(PackageFormat::Vst3)
        .with_format(PackageFormat::Au)
        .with_format(PackageFormat::Standalone)
        .with_format(PackageFormat::DesktopBundle)
        .with_format(PackageFormat::SealedArtifact);

    let plan = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds");

    assert_eq!(plan.targets().len(), 6);
    assert!(
        plan.targets()
            .iter()
            .any(|target| target.format() == PackageFormat::Clap)
    );
    assert!(
        plan.targets()
            .iter()
            .any(|target| target.output_path().ends_with("Demo.clap"))
    );
    assert!(
        plan.targets()
            .iter()
            .any(|target| target.output_path().ends_with("Demo.vst3"))
    );
    assert!(
        plan.targets()
            .iter()
            .any(|target| target.output_path().ends_with("Demo.component"))
    );
    assert!(
        plan.targets()
            .iter()
            .any(|target| target.output_path().ends_with("Demo.app"))
    );
    assert!(
        plan.targets()
            .iter()
            .any(|target| target.output_path().ends_with("Demo.hawk2ui"))
    );
}

#[test]
fn plugin_adapters_emit_metadata_and_verification_reports() {
    let metadata =
        FormatMetadata::new("com.hawk2ui.demo", "Demo", "Hawk2UI").feature("audio-effect");
    let request = PackageRequest::new(
        metadata,
        BundleOutput::new("dist", "Demo"),
        ParameterModel::new([]),
    )
    .with_format(PackageFormat::Clap)
    .with_format(PackageFormat::Vst3);

    let plan = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds");
    let report = plan.verify();

    assert_eq!(report.status(), VerificationStatus::Passed);
    assert!(
        report
            .entries()
            .iter()
            .any(|entry| entry.target().format() == PackageFormat::Clap)
    );
    assert!(
        report
            .entries()
            .iter()
            .all(|entry| entry.metadata().id == "com.hawk2ui.demo")
    );
}

#[test]
fn plugin_adapters_generate_and_validate_verification_report_schema() {
    let metadata =
        FormatMetadata::new("com.hawk2ui.schema", "Schema", "Hawk2UI").feature("audio-effect");
    let request = PackageRequest::new(
        metadata,
        BundleOutput::new("dist", "Schema"),
        ParameterModel::new([]),
    )
    .with_format(PackageFormat::Clap)
    .with_format(PackageFormat::DesktopBundle);

    let plan = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds");
    let report = plan.verify();
    let schema = VerificationReport::json_schema().expect("verification report schema generates");
    let value = serde_json::to_value(&report).expect("verification report serializes");

    VerificationReport::validate_json(&value)
        .expect("serialized verification report validates against generated schema");
    assert_eq!(schema["title"], "VerificationReport");
    assert!(schema["properties"]["entries"].is_object());

    let mut invalid = value;
    invalid["unexpected"] = serde_json::json!(true);
    let error = VerificationReport::validate_json(&invalid)
        .expect_err("unknown verification report fields fail schema validation");
    assert_eq!(error.rule(), "package.schema.verification-report.invalid");
}

#[test]
fn plugin_adapters_generate_and_validate_package_output_schemas() {
    let metadata = FormatMetadata::new("com.hawk2ui.output", "Output", "Hawk2UI");
    let output_root = std::env::temp_dir().join(format!(
        "hawk2ui-plugin-output-schema-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));
    let request = PackageRequest::new(
        metadata,
        BundleOutput::new(output_root.to_string_lossy(), "Output"),
        ParameterModel::new([]),
    )
    .with_format(PackageFormat::Clap);

    let plan = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds");
    let plan_schema = PackagePlan::json_schema().expect("package plan schema generates");
    let plan_value = serde_json::to_value(&plan).expect("package plan serializes");
    PackagePlan::validate_json(&plan_value).expect("serialized package plan validates");
    assert_eq!(plan_schema["title"], "PackagePlan");
    assert!(plan_schema["properties"]["targets"].is_object());

    let outputs = plan.materialize().expect("materialization succeeds");
    let output_schema =
        MaterializedPackageOutput::json_schema().expect("materialized output schema generates");
    let output_value = serde_json::to_value(&outputs[0]).expect("materialized output serializes");
    MaterializedPackageOutput::validate_json(&output_value)
        .expect("serialized materialized output validates");
    assert_eq!(output_schema["title"], "MaterializedPackageOutput");
    assert!(output_schema["properties"]["hash_manifest_path"].is_object());

    let mut invalid = output_value;
    invalid["unexpected"] = serde_json::json!(true);
    let error = MaterializedPackageOutput::validate_json(&invalid)
        .expect_err("unknown materialized output fields fail schema validation");
    assert_eq!(error.rule(), "package.schema.materialized-output.invalid");
}

#[test]
fn plugin_adapters_materialize_package_metadata_outputs() {
    let metadata =
        FormatMetadata::new("com.hawk2ui.demo", "Demo", "Hawk2UI").feature("audio-effect");
    let output_root = std::env::temp_dir().join(format!(
        "hawk2ui-plugin-adapters-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));
    let request = PackageRequest::new(
        metadata,
        BundleOutput::new(output_root.to_string_lossy(), "Demo"),
        ParameterModel::new([]),
    )
    .with_format(PackageFormat::Clap);

    let plan = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds");
    let outputs = plan.materialize().expect("materialization succeeds");

    assert_eq!(outputs.len(), 1);
    assert!(std::path::Path::new(&outputs[0].manifest_path).is_file());
    assert!(std::path::Path::new(&outputs[0].artifact_descriptor_path).is_file());
    let manifest =
        std::fs::read_to_string(&outputs[0].manifest_path).expect("metadata manifest reads");
    assert!(manifest.contains("format = \"clap\""));
    assert!(manifest.contains("id = \"com.hawk2ui.demo\""));
    let artifact = std::fs::read_to_string(&outputs[0].artifact_descriptor_path)
        .expect("artifact descriptor reads");
    assert!(artifact.contains("artifact_format = \"hawk2ui-plugin-package\""));
    assert!(artifact.contains("entry_library = \"Demo.clap\""));
    let clap_entry = std::fs::read_to_string(
        Path::new(&outputs[0].output_path).join("Contents/Resources/clap-entry.toml"),
    )
    .expect("CLAP entry descriptor reads");
    assert!(clap_entry.contains("entry_symbol = \"clap_entry\""));
    assert!(clap_entry.contains("factory_id = \"clap.plugin-factory\""));
    assert!(clap_entry.contains("clap_version = \"1.2.2\""));
    assert!(clap_entry.contains("features = [\"audio-effect\"]"));

    let report = plan.verify_materialized(&outputs);
    assert_eq!(report.status(), VerificationStatus::Passed);
    std::fs::remove_file(&outputs[0].artifact_descriptor_path)
        .expect("artifact descriptor should be removable");
    let failed = plan.verify_materialized(&outputs);
    assert_eq!(failed.status(), VerificationStatus::Failed);
}

#[test]
#[allow(clippy::too_many_lines)]
fn plugin_adapters_materialize_runtime_artifact_payload_into_package_resources() {
    let metadata =
        FormatMetadata::new("com.hawk2ui.runtime", "Runtime", "Hawk2UI").feature("audio-effect");
    let sealed_artifact = SealedArtifact::from_manifest(
        ArtifactSchemaVersion::new(1, 0),
        &HawkManifest::parse(VALID_PLUGIN_MANIFEST).expect("valid plugin manifest parses"),
    )
    .with_runtime_scene_payload(serde_json::json!({
        "viewport": { "width": 1024.0, "height": 640.0 },
        "root": {
            "id": "runtime-root",
            "width": 1024.0,
            "height": 640.0,
            "visual": { "fill": [8, 10, 14, 255] },
            "children": [
                {
                    "id": "runtime-title",
                    "width": 320.0,
                    "height": 48.0,
                    "visual": {
                        "text": {
                            "value": "Runtime Editor",
                            "font_size": 24.0,
                            "color": [240, 245, 255, 255]
                        }
                    }
                }
            ]
        }
    }));
    let runtime_artifact =
        serde_json::to_value(&sealed_artifact).expect("sealed artifact serializes");
    let output_root = std::env::temp_dir().join(format!(
        "hawk2ui-plugin-runtime-artifact-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));
    let request = PackageRequest::new(
        metadata,
        BundleOutput::new(output_root.to_string_lossy(), "Runtime"),
        ParameterModel::new([]),
    )
    .with_editor(PluginEditor::custom(
        "main-editor",
        PluginEditorSize::new(1024.0, 640.0, 1.25),
    ))
    .with_runtime_artifact(runtime_artifact.clone())
    .with_format(PackageFormat::Clap);

    let plan = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds");
    let outputs = plan.materialize().expect("materialization succeeds");
    let root = Path::new(&outputs[0].output_path);
    let runtime_artifact_path = root
        .join("Contents")
        .join("Resources")
        .join("hawk2ui-runtime-artifact.json");

    assert!(runtime_artifact_path.is_file());
    let materialized_artifact: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&runtime_artifact_path).expect("runtime artifact reads"),
    )
    .expect("runtime artifact is JSON");
    assert_eq!(materialized_artifact, runtime_artifact);

    let artifact_descriptor = std::fs::read_to_string(&outputs[0].artifact_descriptor_path)
        .expect("artifact descriptor reads");
    assert!(
        artifact_descriptor
            .contains("runtime_artifact = \"Contents/Resources/hawk2ui-runtime-artifact.json\"")
    );
    let editor_descriptor_path = root
        .join("Contents")
        .join("Resources")
        .join("hawk2ui-editor.toml");
    let editor_descriptor =
        std::fs::read_to_string(&editor_descriptor_path).expect("editor descriptor reads");
    assert!(editor_descriptor.contains("host_adapter = \"baseview\""));
    assert!(editor_descriptor.contains("renderer = \"skia\""));
    assert!(
        editor_descriptor
            .contains("runtime_artifact = \"Contents/Resources/hawk2ui-runtime-artifact.json\"")
    );
    let generated_clap_cargo = root
        .join("Contents")
        .join("Resources")
        .join("generated-clap")
        .join("Cargo.toml");
    let generated_clap_source_path = root
        .join("Contents")
        .join("Resources")
        .join("generated-clap")
        .join("src")
        .join("lib.rs");
    assert!(generated_clap_cargo.is_file());
    assert!(generated_clap_source_path.is_file());
    let generated_clap_manifest =
        std::fs::read_to_string(&generated_clap_cargo).expect("generated CLAP manifest reads");
    assert!(
        generated_clap_manifest.contains("[workspace]"),
        "generated CLAP cdylib must be buildable as a standalone Cargo workspace"
    );
    let generated_clap_source =
        std::fs::read_to_string(&generated_clap_source_path).expect("generated CLAP source reads");
    assert!(generated_clap_source.contains("hawk2ui_editor_descriptor"));
    assert!(generated_clap_source.contains("Contents/Resources/hawk2ui-runtime-artifact.json"));
    assert!(generated_clap_source.contains("host_adapter=baseview"));
    let editor_session =
        ClapRuntimeEditorSession::load_from_package(root).expect("editor session loads");
    let clap_plugin_path = root.join("Runtime.clap");
    let editor_session_from_clap_path =
        ClapRuntimeEditorSession::load_from_clap_plugin_path(&clap_plugin_path)
            .expect("editor session loads from CLAP plugin path");
    assert_eq!(editor_session_from_clap_path, editor_session);
    let editor_session_from_generated_source =
        ClapRuntimeEditorSession::load_from_clap_plugin_path(&generated_clap_source_path)
            .expect("editor session loads from generated binary-adjacent package path");
    assert_eq!(editor_session_from_generated_source, editor_session);
    assert_eq!(editor_session.descriptor().host_adapter(), "baseview");
    assert_eq!(editor_session.descriptor().renderer(), "skia");
    assert_eq!(
        editor_session.descriptor().runtime_artifact(),
        "Contents/Resources/hawk2ui-runtime-artifact.json"
    );
    assert_eq!(editor_session.descriptor().format(), PackageFormat::Clap);
    assert_eq!(
        editor_session.descriptor().plugin_id(),
        "com.hawk2ui.runtime"
    );
    assert_eq!(editor_session.descriptor().parameter_count(), 0);
    assert_eq!(editor_session.descriptor().editor_id(), "main-editor");
    assert!((editor_session.descriptor().logical_width() - 1024.0).abs() < f64::EPSILON);
    assert!((editor_session.descriptor().logical_height() - 640.0).abs() < f64::EPSILON);
    assert!((editor_session.descriptor().scale_factor() - 1.25).abs() < f64::EPSILON);
    assert_eq!(editor_session.runtime_artifact(), &runtime_artifact);
    assert_eq!(editor_session.sealed_artifact(), &sealed_artifact);
    let host_config = editor_session
        .baseview_host_config(
            ClapGuiParentHandle::from_raw_parts(ClapGuiWindowApi::X11, 42)
                .expect("CLAP parent handle validates"),
            Some(7),
        )
        .expect("Baseview host config builds");
    assert_eq!(
        host_config.host_parent(),
        hawk2ui_host::HostPlatformHandle::linux_x11(7, 42)
    );
    assert_eq!(host_config.editor_config().editor_id, "main-editor");
    assert!((host_config.editor_config().metrics.logical_width - 1024.0).abs() < f64::EPSILON);
    assert!((host_config.editor_config().metrics.logical_height - 640.0).abs() < f64::EPSILON);
    assert!((host_config.editor_config().metrics.scale_factor - 1.25).abs() < f64::EPSILON);
    let frame = editor_session
        .runtime_scene_frame()
        .expect("runtime scene frame builds from sealed artifact payload");
    let root_width = frame
        .geometry_for(&RuntimeViewId::new("runtime-root"))
        .expect("root geometry exists")
        .width;
    assert!((root_width - 1024.0).abs() < f32::EPSILON);
    assert!(frame.draw_commands().iter().any(|command| {
        matches!(
            command,
            RuntimeDrawCommand::Text { id, text, .. }
                if id.as_str() == "runtime-title" && text == "Runtime Editor"
        )
    }));

    let invalid_output_root = std::env::temp_dir().join(format!(
        "hawk2ui-plugin-invalid-runtime-artifact-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));
    let invalid_request = PackageRequest::new(
        FormatMetadata::new("com.hawk2ui.invalid-runtime", "InvalidRuntime", "Hawk2UI"),
        BundleOutput::new(invalid_output_root.to_string_lossy(), "InvalidRuntime"),
        ParameterModel::new([]),
    )
    .with_runtime_artifact(serde_json::json!({ "schema_version": { "major": 1 } }))
    .with_format(PackageFormat::Clap);
    let invalid_outputs = PackageAdapterSet::new()
        .plan(&invalid_request)
        .expect("invalid artifact package plan succeeds")
        .materialize()
        .expect("invalid artifact materializes with hash coverage");
    let schema_error = ClapRuntimeEditorSession::load_from_package(&invalid_outputs[0].output_path)
        .expect_err("schema-invalid runtime artifact is denied");
    assert_eq!(
        schema_error.diagnostic().rule(),
        "package.clap-runtime-editor.runtime-artifact-schema-invalid"
    );
    let unresolved_error =
        ClapRuntimeEditorSession::load_from_clap_plugin_path(std::env::temp_dir())
            .expect_err("unrelated path cannot resolve a CLAP runtime editor package");
    assert_eq!(
        unresolved_error.diagnostic().rule(),
        "package.clap-runtime-editor.package-root-unresolved"
    );

    let hashes =
        std::fs::read_to_string(&outputs[0].hash_manifest_path).expect("hash manifest reads");
    assert!(hashes.contains("Contents/Resources/hawk2ui-runtime-artifact.json"));
    assert!(hashes.contains("Contents/Resources/hawk2ui-editor.toml"));
    assert!(hashes.contains("Contents/Resources/generated-clap/Cargo.toml"));
    assert!(hashes.contains("Contents/Resources/generated-clap/src/lib.rs"));
    assert_eq!(
        plan.verify_materialized(&outputs).status(),
        VerificationStatus::Passed
    );
    std::fs::remove_file(runtime_artifact_path).expect("runtime artifact should be removable");
    assert_eq!(
        plan.verify_materialized(&outputs).status(),
        VerificationStatus::Failed
    );
    let load_error =
        ClapRuntimeEditorSession::load_from_package(root).expect_err("missing artifact is denied");
    assert_eq!(
        load_error.diagnostic().rule(),
        "package.clap-runtime-editor.hash-invalid"
    );
}

#[test]
fn plugin_adapters_trusted_runtime_editor_loader_enforces_release_keys() {
    let (signed_artifact, verifier) = signed_runtime_artifact();
    let runtime_artifact =
        serde_json::to_value(&signed_artifact).expect("signed artifact serializes");
    let output_root = unique_temp_dir("hawk2ui-plugin-trusted-runtime-artifact");
    let request = PackageRequest::new(
        FormatMetadata::new("com.hawk2ui.trusted-runtime", "Trusted Runtime", "Hawk2UI"),
        BundleOutput::new(output_root.to_string_lossy(), "TrustedRuntime"),
        ParameterModel::new([]),
    )
    .with_editor(PluginEditor::custom(
        "main-editor",
        PluginEditorSize::new(320.0, 180.0, 1.0),
    ))
    .with_runtime_artifact(runtime_artifact)
    .with_format(PackageFormat::Clap);
    let outputs = PackageAdapterSet::new()
        .plan(&request)
        .expect("trusted artifact package plan succeeds")
        .materialize()
        .expect("trusted artifact package materializes");
    let root = Path::new(&outputs[0].output_path);

    let trusted_session = ClapRuntimeEditorSession::load_trusted_from_package(root, &verifier)
        .expect("trusted signed runtime editor package loads");
    assert_eq!(trusted_session.sealed_artifact(), &signed_artifact);

    let clap_plugin_path = root.join("TrustedRuntime.clap");
    let trusted_from_plugin_path =
        ClapRuntimeEditorSession::load_trusted_from_clap_plugin_path(&clap_plugin_path, &verifier)
            .expect("trusted signed runtime editor package resolves from CLAP entry path");
    assert_eq!(trusted_from_plugin_path, trusted_session);

    let untrusted_error = ClapRuntimeEditorSession::load_trusted_from_package(
        root,
        &ArtifactSignatureVerifier::default(),
    )
    .expect_err("signed packages from unknown keys must be denied");
    assert_eq!(
        untrusted_error.diagnostic().rule(),
        "package.clap-runtime-editor.security.package.signature-invalid"
    );

    let unsigned_artifact = unsigned_runtime_artifact();
    let unsigned_output_root = unique_temp_dir("hawk2ui-plugin-unsigned-runtime-artifact");
    let unsigned_request = PackageRequest::new(
        FormatMetadata::new(
            "com.hawk2ui.unsigned-runtime",
            "Unsigned Runtime",
            "Hawk2UI",
        ),
        BundleOutput::new(unsigned_output_root.to_string_lossy(), "UnsignedRuntime"),
        ParameterModel::new([]),
    )
    .with_editor(PluginEditor::custom(
        "main-editor",
        PluginEditorSize::new(320.0, 180.0, 1.0),
    ))
    .with_runtime_artifact(
        serde_json::to_value(&unsigned_artifact).expect("unsigned artifact serializes"),
    )
    .with_format(PackageFormat::Clap);
    let unsigned_outputs = PackageAdapterSet::new()
        .plan(&unsigned_request)
        .expect("unsigned artifact package plan succeeds")
        .materialize()
        .expect("unsigned artifact package materializes");

    let unsigned_error = ClapRuntimeEditorSession::load_trusted_from_package(
        &unsigned_outputs[0].output_path,
        &verifier,
    )
    .expect_err("unsigned runtime editor packages must be denied");
    assert_eq!(
        unsigned_error.diagnostic().rule(),
        "package.clap-runtime-editor.security.package.signature-missing"
    );
}

const VALID_PLUGIN_MANIFEST: &str = r#"
[identity]
id = "com.hawk2ui.runtime"
name = "Runtime"
version = "0.1.0"

[source]
entry = "src/main.ts"

[capabilities]
keys = ["plugin-editor"]

[[targets]]
kind = "plugin"
name = "clap"

[plugin]
id = "com.hawk2ui.runtime"
name = "Runtime"

[editor]
width = 960
height = 540
"#;

fn signed_runtime_artifact() -> (SealedArtifact, ArtifactSignatureVerifier) {
    let signing_key = ArtifactSigningKey::ed25519_sha256_v1("release-key", [7; 32]);
    let artifact = unsigned_runtime_artifact();
    (
        signing_key.sign(&artifact),
        ArtifactSignatureVerifier::new([signing_key.verification_key()]),
    )
}

fn unsigned_runtime_artifact() -> SealedArtifact {
    SealedArtifact::from_manifest(
        ArtifactSchemaVersion::new(1, 0),
        &HawkManifest::parse(VALID_PLUGIN_MANIFEST).expect("valid plugin manifest parses"),
    )
    .with_compiled_script(CompiledScriptRecord::new(
        "main",
        "src/main.ts",
        "scripts/main.hawk.js",
        ArtifactHash::from_bytes(b"trusted-runtime-script"),
    ))
    .with_runtime_scene_payload(serde_json::json!({
        "viewport": { "width": 320.0, "height": 180.0 },
        "root": {
            "id": "runtime-root",
            "width": 320.0,
            "height": 180.0,
            "visual": { "fill": [8, 10, 14, 255] },
            "children": []
        }
    }))
}

fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "{prefix}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ))
}

#[test]
fn plugin_adapters_build_clap_entry_plan_from_clap_sys_contract() {
    let metadata = FormatMetadata::new("com.hawk2ui.clap", "Clap", "Hawk2UI")
        .version("1.0.0")
        .feature("audio-effect")
        .feature("utility");

    let entry = ClapPluginEntryPlan::from_metadata(&metadata);

    assert_eq!(entry.entry_symbol(), "clap_entry");
    assert_eq!(entry.factory_id(), "clap.plugin-factory");
    assert_eq!(entry.clap_version(), "1.2.2");
    assert_eq!(entry.plugin_id(), "com.hawk2ui.clap");
    assert_eq!(entry.features(), &["audio-effect", "utility"]);
}

#[test]
fn plugin_adapters_map_clap_gui_parent_handles_to_baseview_hosts() {
    let x11_parent = ClapGuiParentHandle::from_raw_parts(ClapGuiWindowApi::X11, 42)
        .expect("nonzero X11 handle maps");
    assert_eq!(x11_parent.api(), ClapGuiWindowApi::X11);
    assert_eq!(
        x11_parent
            .to_baseview_host_handle(Some(7))
            .expect("X11 parent with display maps to host handle"),
        hawk2ui_host::HostPlatformHandle::linux_x11(7, 42)
    );

    let windows_parent = ClapGuiParentHandle::from_raw_parts(ClapGuiWindowApi::Win32, 99)
        .expect("nonzero HWND maps");
    assert_eq!(
        windows_parent
            .to_baseview_host_handle(None)
            .expect("Windows parent maps directly"),
        hawk2ui_host::HostPlatformHandle::windows_hwnd(99)
    );

    let macos_parent = ClapGuiParentHandle::from_raw_parts(ClapGuiWindowApi::Cocoa, 123)
        .expect("nonzero NSView maps");
    assert_eq!(
        macos_parent
            .to_baseview_host_handle(None)
            .expect("macOS parent maps directly"),
        hawk2ui_host::HostPlatformHandle::macos_ns_view(123)
    );

    let wayland_parent = ClapGuiParentHandle::from_raw_parts(ClapGuiWindowApi::Wayland, 777)
        .expect("nonzero Wayland surface maps");
    assert_eq!(
        wayland_parent
            .to_baseview_host_handle(Some(888))
            .expect("Wayland parent with display maps to host handle"),
        hawk2ui_host::HostPlatformHandle::linux_wayland(888, 777)
    );
}

#[test]
fn plugin_adapters_reject_invalid_clap_gui_parent_handles() {
    let zero = ClapGuiParentHandle::from_raw_parts(ClapGuiWindowApi::X11, 0)
        .expect_err("zero native parent handles must be rejected");
    assert_eq!(zero.rule(), "package.clap-gui-parent.invalid-handle");

    let missing_display = ClapGuiParentHandle::from_raw_parts(ClapGuiWindowApi::X11, 42)
        .expect("X11 handle maps")
        .to_baseview_host_handle(None)
        .expect_err("X11 Baseview attachment requires an explicit display handle");
    assert_eq!(
        missing_display.rule(),
        "package.clap-gui-parent.missing-display"
    );

    let missing_wayland_display =
        ClapGuiParentHandle::from_raw_parts(ClapGuiWindowApi::Wayland, 42)
            .expect("Wayland handle maps")
            .to_baseview_host_handle(None)
            .expect_err("Wayland Baseview attachment requires an explicit display handle");
    assert_eq!(
        missing_wayland_display.rule(),
        "package.clap-gui-parent.missing-display"
    );
}

#[test]
fn plugin_adapters_validate_clap_runtime_editor_descriptor() {
    let descriptor = ClapRuntimeEditorDescriptor::new(
        "Contents/Resources/hawk2ui-runtime-artifact.json",
        "baseview",
        "skia",
    )
    .expect("valid descriptor builds");

    assert_eq!(
        descriptor.to_export_payload(),
        "runtime_artifact=Contents/Resources/hawk2ui-runtime-artifact.json\nhost_adapter=baseview\nrenderer=skia\n"
    );
    assert_eq!(
        ClapRuntimeEditorDescriptor::new("", "baseview", "skia")
            .expect_err("empty runtime artifact path is rejected")
            .rule(),
        "package.clap-editor-descriptor.invalid-runtime-artifact"
    );
    assert_eq!(
        ClapRuntimeEditorDescriptor::new(
            "Contents/Resources/hawk2ui-runtime-artifact.json",
            "",
            "skia"
        )
        .expect_err("empty host adapter is rejected")
        .rule(),
        "package.clap-editor-descriptor.invalid-host-adapter"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn plugin_adapters_generate_compilable_clap_cdylib_scaffold() {
    let metadata = FormatMetadata::new("com.hawk2ui.loadable", "Loadable", "Hawk2UI")
        .version("1.0.0")
        .feature("audio-effect");
    let editor = PluginEditor::custom("main", PluginEditorSize::new(1024.0, 640.0, 1.0));
    let parameters = ParameterModel::new([ParameterRecord::numeric(
        "gain",
        "Gain",
        "dB",
        ParameterRange::new(-60.0, 6.0, 0.0),
    )
    .flags(ParameterFlags::automatable())]);
    let output_root = std::env::temp_dir().join(format!(
        "hawk2ui-clap-cdylib-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));

    let scaffold = ClapCdylibScaffold::from_metadata(&metadata)
        .with_editor(&editor)
        .with_parameters(&parameters)
        .with_runtime_editor_descriptor(
            ClapRuntimeEditorDescriptor::new(
                "Contents/Resources/hawk2ui-runtime-artifact.json",
                "baseview",
                "skia",
            )
            .expect("runtime editor descriptor builds"),
        );
    let output = scaffold
        .write_to(&output_root)
        .expect("CLAP scaffold should write");

    assert!(Path::new(&output.cargo_toml_path).is_file());
    assert!(Path::new(&output.lib_rs_path).is_file());
    let source = std::fs::read_to_string(&output.lib_rs_path).expect("generated source reads");
    assert!(source.contains("pub static clap_entry"));
    assert!(source.contains("clap_plugin_factory"));
    assert!(source.contains("clap_plugin_entry"));
    assert!(source.contains("get_plugin_descriptor"));
    assert!(source.contains("create_plugin"));
    assert!(source.contains("plugin_activate"));
    assert!(source.contains("plugin_process"));
    assert!(source.contains("clap_plugin_audio_ports"));
    assert!(source.contains("clap_plugin_gui"));
    assert!(source.contains("Hawk2uiPluginInstance"));
    assert!(source.contains("editor_attached"));
    assert!(source.contains("clap_plugin_params"));
    assert!(source.contains("clap_plugin_state"));
    assert!(source.contains("PARAMETERS"));
    assert!(source.contains("Gain"));
    assert!(source.contains("hawk2ui_editor_descriptor"));
    assert!(source.contains("hawk2ui_editor_state_for_plugin"));
    assert!(source.contains("hawk2ui_editor_dispatch_for_plugin"));
    assert!(source.contains("hawk2ui_editor_host_abi"));
    assert!(source.contains("hawk2ui_realtime_safety_policy"));
    assert!(source.contains("Hawk2uiRealtimeOperation::PreallocatedWrite"));
    assert!(source.contains("Contents/Resources/hawk2ui-runtime-artifact.json"));
    assert!(source.contains("host_adapter=baseview"));

    let target_dir = output_root.join("target");
    let status = std::process::Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("--manifest-path")
        .arg(&output.cargo_toml_path)
        .env("CARGO_TARGET_DIR", &target_dir)
        .status()
        .expect("cargo build should launch for generated CLAP scaffold");
    assert!(status.success(), "generated CLAP scaffold should compile");

    let library_path = target_dir
        .join("release")
        .join(format!(
            "{}{}",
            std::env::consts::DLL_PREFIX,
            output.library_file_stem
        ))
        .with_extension(std::env::consts::DLL_EXTENSION);
    assert!(library_path.is_file());
    let library_bytes = std::fs::read(&library_path).expect("compiled CLAP library reads");
    assert!(
        library_bytes
            .windows("clap_entry".len())
            .any(|window| window == b"clap_entry")
    );
    assert!(
        library_bytes
            .windows("plugin-factory".len())
            .any(|window| window == b"plugin-factory")
    );
    assert!(
        library_bytes
            .windows("com.hawk2ui.loadable".len())
            .any(|window| window == b"com.hawk2ui.loadable")
    );

    let host_check_root = output_root.join("host-check");
    write_generated_clap_host_check(&host_check_root, &library_path);
    let host_target_dir = output_root.join("host-check-target");
    let host_check = std::process::Command::new("cargo")
        .arg("run")
        .arg("--release")
        .arg("--manifest-path")
        .arg(host_check_root.join("Cargo.toml"))
        .arg("--")
        .arg(&library_path)
        .env("CARGO_TARGET_DIR", &host_target_dir)
        .output()
        .expect("generated CLAP host check should launch");
    assert!(
        host_check.status.success(),
        "generated CLAP host check should load the compiled library\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
        host_check.status,
        String::from_utf8_lossy(&host_check.stdout),
        String::from_utf8_lossy(&host_check.stderr)
    );
}

#[test]
fn plugin_adapters_generate_loadable_vst3_cdylib_factory() {
    let metadata = FormatMetadata::new("com.hawk2ui.vst3-loadable", "VST3 Loadable", "Hawk2UI")
        .version("1.0.0")
        .feature("audio-effect");
    let output_root = std::env::temp_dir().join(format!(
        "hawk2ui-vst3-cdylib-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));
    let request = PackageRequest::new(
        metadata,
        BundleOutput::new(output_root.to_string_lossy(), "Vst3Loadable"),
        ParameterModel::new([ParameterRecord::numeric(
            "gain",
            "Gain",
            "dB",
            ParameterRange::new(-60.0, 12.0, -24.0),
        )
        .flags(ParameterFlags::automatable())
        .param_id(7)]),
    )
    .with_format(PackageFormat::Vst3);

    let plan = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds");
    let outputs = plan.materialize().expect("materialization succeeds");
    let vst3_output = outputs
        .iter()
        .find(|output| output.format == PackageFormat::Vst3)
        .expect("VST3 output exists");
    let package_root = Path::new(&vst3_output.output_path);
    let generated_root = package_root.join("Contents/Resources/generated-vst3");
    let generated_manifest = generated_root.join("Cargo.toml");
    assert!(generated_manifest.is_file());

    let target_dir = output_root.join("target");
    let status = std::process::Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("--manifest-path")
        .arg(&generated_manifest)
        .env("CARGO_TARGET_DIR", &target_dir)
        .status()
        .expect("cargo build should launch for generated VST3 scaffold");
    assert!(status.success(), "generated VST3 scaffold should compile");

    let library_path = target_dir
        .join("release")
        .join(format!(
            "{}{}",
            std::env::consts::DLL_PREFIX,
            "hawk2ui_generated_vst3"
        ))
        .with_extension(std::env::consts::DLL_EXTENSION);
    assert!(library_path.is_file());

    let host_check_root = output_root.join("vst3-host-check");
    write_generated_vst3_host_check(&host_check_root, &library_path);
    let host_target_dir = output_root.join("vst3-host-check-target");
    let status = std::process::Command::new("cargo")
        .arg("run")
        .arg("--release")
        .arg("--manifest-path")
        .arg(host_check_root.join("Cargo.toml"))
        .arg("--")
        .arg(&library_path)
        .env("CARGO_TARGET_DIR", &host_target_dir)
        .status()
        .expect("generated VST3 host check should launch");
    assert!(
        status.success(),
        "generated VST3 host check should load factory and instantiate classes"
    );
}

#[test]
fn plugin_adapters_preserve_choice_defaults_and_stepped_flags_in_clap_scaffold() {
    let metadata = FormatMetadata::new("com.hawk2ui.choice", "Choice", "Hawk2UI").version("1.0.0");
    let bypass = ParameterRecord::boolean("bypass", "Bypass", true)
        .flags(ParameterFlags::automatable())
        .param_id(7);
    let mut mode = ParameterRecord::enumerated(
        "mode",
        "Mode",
        2,
        [
            EnumVariant::new("clean", "Clean"),
            EnumVariant::new("drive", "Drive"),
            EnumVariant::new("wide", "Wide"),
        ],
    )
    .flags(ParameterFlags::automatable())
    .param_id(42);
    mode.steps = None;
    let parameters = ParameterModel::new([bypass, mode]);
    let output_root = std::env::temp_dir().join(format!(
        "hawk2ui-clap-choice-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));

    let output = ClapCdylibScaffold::from_metadata(&metadata)
        .with_parameters(&parameters)
        .write_to(&output_root)
        .expect("CLAP scaffold should write");
    let source = std::fs::read_to_string(&output.lib_rs_path).expect("generated source reads");
    let expected_flags = CLAP_PARAM_IS_STEPPED | CLAP_PARAM_IS_AUTOMATABLE;

    assert!(
        source.contains("GeneratedParameter { id: 7, name: b\"Bypass\\x00\""),
        "pinned bool parameter id should be emitted"
    );
    assert!(
        source.contains("default_value: 1.0"),
        "bool true default should be emitted as the CLAP scalar default"
    );
    assert!(
        source.contains("GeneratedParameter { id: 42, name: b\"Mode\\x00\""),
        "pinned choice parameter id should be emitted"
    );
    assert!(
        source.contains("max_value: 2.0"),
        "three-choice parameter should expose max variant index as the CLAP max value"
    );
    assert!(
        source.contains("name: b\"Mode\\x00\""),
        "choice parameter name should be emitted"
    );
    assert!(
        source.contains("default_value: 2.0"),
        "choice default index should be emitted as the CLAP default value"
    );
    assert!(
        source.contains(&format!("flags: {expected_flags}")),
        "choice parameters should be stepped even when explicit steps are absent"
    );
}

#[test]
fn plugin_adapters_preserve_parameter_metadata_in_vst3_scaffold() {
    let metadata =
        FormatMetadata::new("com.hawk2ui.vst3-params", "Vst3Params", "Hawk2UI").version("1.0.0");
    let bypass = ParameterRecord::boolean("bypass", "Bypass", true)
        .flags(ParameterFlags::automatable())
        .param_id(7);
    let mode = ParameterRecord::enumerated(
        "mode",
        "Mode",
        2,
        [
            EnumVariant::new("clean", "Clean"),
            EnumVariant::new("drive", "Drive"),
            EnumVariant::new("wide", "Wide"),
        ],
    )
    .flags(ParameterFlags::automatable())
    .param_id(42);
    let parameters = ParameterModel::new([bypass, mode]);
    let output_root = std::env::temp_dir().join(format!(
        "hawk2ui-vst3-params-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));
    let request = PackageRequest::new(
        metadata,
        BundleOutput::new(output_root.to_string_lossy(), "Vst3Params"),
        parameters,
    )
    .with_format(PackageFormat::Vst3);

    let plan = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds");
    let outputs = plan.materialize().expect("materialization succeeds");
    let vst3_output = outputs
        .iter()
        .find(|output| output.format == PackageFormat::Vst3)
        .expect("VST3 output exists");
    let generated_lib =
        Path::new(&vst3_output.output_path).join("Contents/Resources/generated-vst3/src/lib.rs");
    let source = std::fs::read_to_string(generated_lib).expect("VST3 scaffold source reads");

    assert!(source.contains("struct GeneratedVst3Parameter"));
    assert!(source.contains("unsafe fn getParameterCount(&self) -> i32 {\n        2\n    }"));
    assert!(
        source.contains(
            "GeneratedVst3Parameter { id: 7, title: \"Bypass\", short_title: \"Bypass\", units: \"\", min_value: 0.0, max_value: 1.0, default_plain_value: 1.0, default_normalized_value: 1.0, step_count: 1, flags: 1 }"
        ),
        "bool parameter should keep pinned ID, true default, one step, and automation flag"
    );
    assert!(
        source.contains(
            "GeneratedVst3Parameter { id: 42, title: \"Mode\", short_title: \"Mode\", units: \"\", min_value: 0.0, max_value: 2.0, default_plain_value: 2.0, default_normalized_value: 1.0, step_count: 2, flags: 9 }"
        ),
        "choice parameter should keep pinned ID, max variant index, normalized default, steps, and list flag"
    );
    assert!(source.contains("static PARAMETER_VALUES: [AtomicU64; 2] = ["));
}

fn write_generated_clap_host_check(root: &Path, library_path: &Path) {
    assert!(
        library_path.is_file(),
        "host checker requires an already-built CLAP library"
    );
    std::fs::create_dir_all(root.join("src")).expect("host checker src directory writes");
    std::fs::write(
        root.join("Cargo.toml"),
        generated_clap_host_check_manifest(),
    )
    .expect("host checker manifest writes");
    std::fs::write(
        root.join("src").join("main.rs"),
        generated_clap_host_check_source(),
    )
    .expect("host checker source writes");
}

fn generated_clap_host_check_manifest() -> &'static str {
    r#"[workspace]

[package]
name = "hawk2ui-clap-host-check"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
clap-sys = "0.5.0"
libloading = "0.8.9"
"#
}

fn write_generated_vst3_host_check(root: &Path, library_path: &Path) {
    assert!(
        library_path.is_file(),
        "host checker requires an already-built VST3 library"
    );
    std::fs::create_dir_all(root.join("src")).expect("VST3 host checker src directory writes");
    std::fs::write(
        root.join("Cargo.toml"),
        generated_vst3_host_check_manifest(),
    )
    .expect("VST3 host checker manifest writes");
    std::fs::write(
        root.join("src").join("main.rs"),
        generated_vst3_host_check_source(),
    )
    .expect("VST3 host checker source writes");
}

fn generated_vst3_host_check_manifest() -> &'static str {
    r#"[workspace]

[package]
name = "hawk2ui-vst3-host-check"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
libloading = "0.8"
vst3 = "0.3.0"
"#
}

#[allow(clippy::too_many_lines)]
fn generated_vst3_host_check_source() -> &'static str {
    r#"use std::{
    env,
    ffi::{c_char, c_void},
    ptr,
};

use libloading::{Library, Symbol};
use vst3::Steinberg::Vst::*;
use vst3::Steinberg::*;

type GetPluginFactory = unsafe extern "system" fn() -> *mut IPluginFactory;

#[cfg(target_os = "linux")]
type ModuleEntry = unsafe extern "system" fn(*mut c_void) -> bool;

fn main() {
    let library_path = env::args().nth(1).expect("library path argument is required");
    unsafe {
        let library = Library::new(library_path).expect("VST3 library loads");

        #[cfg(target_os = "linux")]
        {
            let module_entry: Symbol<ModuleEntry> =
                library.get(b"ModuleEntry").expect("ModuleEntry exports");
            assert!(module_entry(ptr::null_mut()));
        }

        let get_plugin_factory: Symbol<GetPluginFactory> = library
            .get(b"GetPluginFactory")
            .expect("GetPluginFactory exports");
        let factory = get_plugin_factory();
        assert!(!factory.is_null());

        let factory_vtbl = &*(*factory).vtbl;
        assert_eq!((factory_vtbl.countClasses)(factory), 2);

        let mut processor = std::mem::MaybeUninit::<PClassInfo>::zeroed().assume_init();
        assert_eq!(
            (factory_vtbl.getClassInfo)(factory, 0, &mut processor),
            kResultOk
        );
        assert_eq!(c_chars_to_string(&processor.category), "Audio Module Class");
        assert_eq!(c_chars_to_string(&processor.name), "VST3 Loadable");
        instantiate_class(factory, processor.cid);

        let mut controller = std::mem::MaybeUninit::<PClassInfo>::zeroed().assume_init();
        assert_eq!(
            (factory_vtbl.getClassInfo)(factory, 1, &mut controller),
            kResultOk
        );
        assert_eq!(
            c_chars_to_string(&controller.category),
            "Component Controller Class"
        );
        assert_eq!(c_chars_to_string(&controller.name), "VST3 Loadable");
        instantiate_controller_with_parameters(factory, controller.cid);

        let factory_unknown = factory.cast::<FUnknown>();
        ((*(*factory_unknown).vtbl).release)(factory_unknown);
    }
}

unsafe fn instantiate_class(factory: *mut IPluginFactory, cid: TUID) {
    let mut object = ptr::null_mut::<c_void>();
    let result = ((*(*factory).vtbl).createInstance)(
        factory,
        cid.as_ptr(),
        FUnknown_iid.as_ptr(),
        &mut object,
    );
    assert_eq!(result, kResultOk);
    assert!(!object.is_null());
    let unknown = object.cast::<FUnknown>();
    ((*(*unknown).vtbl).release)(unknown);
}

unsafe fn instantiate_controller_with_parameters(factory: *mut IPluginFactory, cid: TUID) {
    let mut object = ptr::null_mut::<c_void>();
    let result = ((*(*factory).vtbl).createInstance)(
        factory,
        cid.as_ptr(),
        IEditController_iid.as_ptr(),
        &mut object,
    );
    assert_eq!(result, kResultOk);
    assert!(!object.is_null());
    let controller = object.cast::<IEditController>();
    let vtbl = &*(*controller).vtbl;
    assert_eq!((vtbl.getParameterCount)(controller), 1);

    let mut info = std::mem::MaybeUninit::<ParameterInfo>::zeroed().assume_init();
    assert_eq!((vtbl.getParameterInfo)(controller, 0, &mut info), kResultOk);
    assert_eq!(info.id, 7);
    assert_eq!(wstring_to_string(&info.title), "Gain");
    assert_eq!(wstring_to_string(&info.units), "dB");
    assert!((info.defaultNormalizedValue - 0.5).abs() < f64::EPSILON);
    assert_eq!(info.flags, ParameterInfo_::ParameterFlags_::kCanAutomate as i32);

    assert!(((vtbl.normalizedParamToPlain)(controller, 7, 0.5) + 24.0).abs() < f64::EPSILON);
    assert!(((vtbl.plainParamToNormalized)(controller, 7, -24.0) - 0.5).abs() < f64::EPSILON);
    assert_eq!((vtbl.setParamNormalized)(controller, 7, 0.75), kResultOk);
    assert!(((vtbl.getParamNormalized)(controller, 7) - 0.75).abs() < f64::EPSILON);

    let mut display = [0_u16; 128];
    assert_eq!(
        (vtbl.getParamStringByValue)(controller, 7, 0.5, &mut display),
        kResultOk
    );
    assert_eq!(wstring_to_string(&display), "-24");
    let mut parsed = 0.0;
    let parsed_input = utf16_with_nul("-24");
    assert_eq!(
        (vtbl.getParamValueByString)(controller, 7, parsed_input.as_ptr().cast_mut(), &mut parsed),
        kResultOk
    );
    assert!((parsed - 0.5).abs() < f64::EPSILON);

    let unknown = object.cast::<FUnknown>();
    ((*(*unknown).vtbl).release)(unknown);
}

fn c_chars_to_string<const N: usize>(source: &[c_char; N]) -> String {
    let bytes: Vec<u8> = source
        .iter()
        .copied()
        .take_while(|byte| *byte != 0)
        .map(|byte| byte as u8)
        .collect();
    String::from_utf8(bytes).expect("VST3 class strings are UTF-8")
}

fn wstring_to_string<const N: usize>(source: &[u16; N]) -> String {
    let units: Vec<u16> = source.iter().copied().take_while(|unit| *unit != 0).collect();
    String::from_utf16(&units).expect("VST3 strings are UTF-16")
}

fn utf16_with_nul(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
"#
}

#[allow(clippy::too_many_lines)]
fn generated_clap_host_check_source() -> &'static str {
    r#"use std::{env, ffi::{c_void, CStr}, ptr};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Hawk2uiEditorState {
    created: bool,
    attached: bool,
    visible: bool,
    width: u32,
    height: u32,
}

fn main() {
    let library_path = env::args().nth(1).expect("library path argument");

    unsafe {
        let library = libloading::Library::new(library_path).expect("library loads");
        let entry_symbol: libloading::Symbol<*const clap_sys::entry::clap_plugin_entry> =
            library.get(b"clap_entry\0").expect("clap_entry resolves");
        let entry = &**entry_symbol;
        assert!((entry.init.expect("entry init"))(ptr::null()));

        let factory = (entry.get_factory.expect("factory"))(
            b"clap.plugin-factory\0".as_ptr().cast(),
        );
        assert!(!factory.is_null());
        let factory = &*(factory as *const clap_sys::factory::plugin_factory::clap_plugin_factory);
        assert_eq!((factory.get_plugin_count.expect("count"))(factory), 1);

        let descriptor = (factory.get_plugin_descriptor.expect("descriptor"))(factory, 0);
        assert!(!descriptor.is_null());
        let descriptor = &*descriptor;
        assert_eq!(
            CStr::from_ptr(descriptor.id).to_string_lossy(),
            "com.hawk2ui.loadable"
        );

        let plugin = (factory.create_plugin.expect("create"))(
            factory,
            ptr::null(),
            descriptor.id,
        );
        assert!(!plugin.is_null());
        assert_eq!((*plugin).desc, descriptor as *const _);
        let second_plugin = (factory.create_plugin.expect("create second"))(
            factory,
            ptr::null(),
            descriptor.id,
        );
        assert!(!second_plugin.is_null());
        assert_ne!(plugin, second_plugin);
        assert_eq!((*second_plugin).desc, descriptor as *const _);

        assert!(((*plugin).init.expect("plugin init"))(plugin));
        assert!(((*second_plugin).init.expect("second plugin init"))(
            second_plugin
        ));
        assert!(((*plugin).activate.expect("activate"))(plugin, 48_000.0, 32, 1_024));
        assert!(((*plugin).start_processing.expect("start processing"))(plugin));
        let process = clap_sys::process::clap_process {
            steady_time: 0,
            frames_count: 0,
            transport: ptr::null(),
            audio_inputs: ptr::null(),
            audio_outputs: ptr::null_mut(),
            audio_inputs_count: 0,
            audio_outputs_count: 0,
            in_events: ptr::null(),
            out_events: ptr::null(),
        };
        assert_eq!(
            ((*plugin).process.expect("process"))(plugin, &process),
            clap_sys::process::CLAP_PROCESS_CONTINUE
        );
        ((*plugin).stop_processing.expect("stop processing"))(plugin);
        ((*plugin).deactivate.expect("deactivate"))(plugin);

        let audio_ports = ((*plugin).get_extension.expect("extension"))(
            plugin,
            b"clap.audio-ports\0".as_ptr().cast(),
        );
        assert!(!audio_ports.is_null());
        let audio_ports =
            &*(audio_ports as *const clap_sys::ext::audio_ports::clap_plugin_audio_ports);
        assert_eq!((audio_ports.count.expect("audio port count"))(plugin, true), 1);
        assert_eq!((audio_ports.count.expect("audio port count"))(plugin, false), 1);

        let gui = ((*plugin).get_extension.expect("gui extension"))(
            plugin,
            b"clap.gui\0".as_ptr().cast(),
        );
        assert!(!gui.is_null());
        let gui = &*(gui as *const clap_sys::ext::gui::clap_plugin_gui);
        let mut width = 0;
        let mut height = 0;
        assert!((gui.get_size.expect("gui size"))(plugin, &mut width, &mut height));
        assert_eq!((width, height), (1024, 640));
        let mut preferred_api = ptr::null();
        let mut is_floating = true;
        assert!((gui.get_preferred_api.expect("preferred gui api"))(
            plugin,
            &mut preferred_api,
            &mut is_floating,
        ));
          assert!(!preferred_api.is_null());
          assert!(!is_floating);
          #[cfg(target_os = "linux")]
          {
              assert_eq!(
                  CStr::from_ptr(preferred_api).to_bytes(),
                  clap_sys::ext::gui::CLAP_WINDOW_API_X11.to_bytes()
              );
              assert!((gui.is_api_supported.expect("x11 supported"))(
                  plugin,
                  clap_sys::ext::gui::CLAP_WINDOW_API_X11.as_ptr(),
                  false,
              ));
            assert!((gui.is_api_supported.expect("wayland supported for baseview"))(
                plugin,
                clap_sys::ext::gui::CLAP_WINDOW_API_WAYLAND.as_ptr(),
                false,
            ));
          }
            let editor_descriptor: libloading::Symbol<unsafe extern "C" fn(*mut usize) -> *const u8> =
                library.get(b"hawk2ui_editor_descriptor\0").expect("editor descriptor export resolves");
          let mut descriptor_len = 0usize;
          let descriptor_ptr = editor_descriptor(&mut descriptor_len);
          assert!(!descriptor_ptr.is_null());
          assert!(descriptor_len > 0);
          let descriptor = std::str::from_utf8(std::slice::from_raw_parts(
              descriptor_ptr,
              descriptor_len,
          ))
          .expect("editor descriptor is utf8");
            assert!(descriptor.contains("runtime_artifact=Contents/Resources/hawk2ui-runtime-artifact.json"));
            assert!(descriptor.contains("host_adapter=baseview"));
            assert!(descriptor.contains("renderer=skia"));
            let editor_host_abi: libloading::Symbol<unsafe extern "C" fn(*mut usize) -> *const u8> =
                library.get(b"hawk2ui_editor_host_abi\0").expect("editor host ABI export resolves");
            let mut host_abi_len = 0usize;
            let host_abi_ptr = editor_host_abi(&mut host_abi_len);
            assert!(!host_abi_ptr.is_null());
            assert!(host_abi_len > 0);
            let host_abi = std::str::from_utf8(std::slice::from_raw_parts(
                host_abi_ptr,
                host_abi_len,
            ))
            .expect("editor host ABI is utf8");
            for required_entry in [
                "hawk2ui_host_bridge_abi=1",
                "command=create",
                "command=set_parent",
                "command=show",
                "command=hide",
                "command=destroy",
                "command=apply_parameter",
                "command=save_state",
                "command=load_state",
                "command=drain_realtime_visuals",
                "response=created",
                "response=parent_attached",
                "response=frame_presented",
                "response=hidden",
                "response=destroyed",
                "response=parameter_applied",
                "response=state_saved",
                "response=state_loaded",
                "response=realtime_visuals_drained",
                "function=hawk2ui_editor_dispatch_for_plugin",
                "function=hawk2ui_editor_state_for_plugin",
                "compat_function=hawk2ui_editor_dispatch",
            ] {
                assert!(
                    host_abi.contains(required_entry),
                    "host ABI missing {required_entry}"
                );
            }
            let realtime_policy: libloading::Symbol<unsafe extern "C" fn(*mut usize) -> *const u8> =
                library.get(b"hawk2ui_realtime_safety_policy\0").expect("realtime safety policy export resolves");
            let mut realtime_policy_len = 0usize;
            let realtime_policy_ptr = realtime_policy(&mut realtime_policy_len);
            assert!(!realtime_policy_ptr.is_null());
            assert!(realtime_policy_len > 0);
            let realtime_policy = std::str::from_utf8(std::slice::from_raw_parts(
                realtime_policy_ptr,
                realtime_policy_len,
            ))
            .expect("realtime policy is utf8");
            for required_entry in [
                "hawk2ui_realtime_safety_policy=1",
                "context=audio_thread",
                "process_callback=preallocated_audio_buffer_copy",
                "policy_check=operation_allowlist_self_check",
                "allowed=preallocated_write",
                "forbidden=allocation",
                "forbidden=blocking_wait",
                "lock_policy=no_blocking_locks",
            ] {
                assert!(
                    realtime_policy.contains(required_entry),
                    "realtime policy missing {required_entry}"
                );
            }
            let editor_dispatch: libloading::Symbol<
                unsafe extern "C" fn(
                    *const clap_sys::plugin::clap_plugin,
                    *const u8,
                    usize,
                    *mut u8,
                    usize,
                    *mut usize,
                ) -> bool,
            > = library
                .get(b"hawk2ui_editor_dispatch_for_plugin\0")
                .expect("per-plugin editor dispatch export resolves");
            let editor_state: libloading::Symbol<
                unsafe extern "C" fn(*const clap_sys::plugin::clap_plugin) -> Hawk2uiEditorState,
            > = library
                .get(b"hawk2ui_editor_state_for_plugin\0")
                .expect("per-plugin editor state export resolves");
            assert_editor_state(editor_state(plugin), false, false, false, 1024, 640);
            let create_response = dispatch_editor(
                *editor_dispatch,
                plugin,
                "command=create\napi=x11\nfloating=false\n",
            );
            assert!(create_response.contains("response=created"));
            assert_editor_state(editor_state(plugin), true, false, false, 1024, 640);
            let blocked_show_response = dispatch_editor(*editor_dispatch, plugin, "command=show\n");
            assert!(blocked_show_response.contains("error=editor-not-attached"));
            assert_editor_state(editor_state(plugin), true, false, false, 1024, 640);
            let attach_response = dispatch_editor(
                *editor_dispatch,
                plugin,
                "command=set_parent\napi=x11\nparent=1\n",
            );
            assert!(attach_response.contains("response=parent_attached"));
            assert_editor_state(editor_state(plugin), true, true, false, 1024, 640);
            let show_response = dispatch_editor(*editor_dispatch, plugin, "command=show\n");
            assert!(show_response.contains("response=frame_presented"));
            assert!(show_response.contains("width=1024"));
            assert!(show_response.contains("height=640"));
            assert!(show_response.contains("presented_frame_count=1"));
            assert_editor_state(editor_state(plugin), true, true, true, 1024, 640);
            let hide_response = dispatch_editor(*editor_dispatch, plugin, "command=hide\n");
            assert!(hide_response.contains("response=hidden"));
            assert_editor_state(editor_state(plugin), true, true, false, 1024, 640);
            let destroy_response = dispatch_editor(*editor_dispatch, plugin, "command=destroy\n");
            assert!(destroy_response.contains("response=destroyed"));
            assert_editor_state(editor_state(plugin), false, false, false, 1024, 640);
            assert!((gui.create.expect("gui create"))(plugin, preferred_api, false));
            assert_editor_state(editor_state(plugin), true, false, false, 1024, 640);
            assert!(!(gui.show.expect("gui show before parent"))(plugin));
            assert_editor_state(editor_state(plugin), true, false, false, 1024, 640);
            let null_parent = clap_sys::ext::gui::clap_window {
                api: preferred_api,
                specific: clap_sys::ext::gui::clap_window_handle {
                    ptr: ptr::null_mut(),
                },
            };
            assert!(!(gui.set_parent.expect("gui rejects null parent handle"))(
                plugin,
                &null_parent
            ));
            assert_editor_state(editor_state(plugin), true, false, false, 1024, 640);
            let parent = clap_sys::ext::gui::clap_window {
                api: preferred_api,
                specific: clap_sys::ext::gui::clap_window_handle {
                    ptr: 0x1usize as *mut c_void,
                },
            };
            assert!((gui.set_parent.expect("gui set parent"))(plugin, &parent));
            assert_editor_state(editor_state(plugin), true, true, false, 1024, 640);
            assert!((gui.set_size.expect("gui set size"))(plugin, 1200, 720));
            assert_editor_state(editor_state(plugin), true, true, false, 1200, 720);
            assert!((gui.show.expect("gui show"))(plugin));
            assert_editor_state(editor_state(plugin), true, true, true, 1200, 720);
            let second_gui = ((*second_plugin)
                .get_extension
                .expect("second gui extension"))(
                second_plugin,
                b"clap.gui\0".as_ptr().cast(),
            );
            assert!(!second_gui.is_null());
            let second_gui =
                &*(second_gui as *const clap_sys::ext::gui::clap_plugin_gui);
            assert_editor_state(editor_state(second_plugin), false, false, false, 1024, 640);
            assert!((second_gui.create.expect("second gui create"))(
                second_plugin,
                preferred_api,
                false
            ));
            assert!((second_gui.set_parent.expect("second gui set parent"))(
                second_plugin,
                &parent
            ));
            assert!((second_gui.set_size.expect("second gui set size"))(
                second_plugin,
                640,
                480
            ));
            assert!((second_gui.show.expect("second gui show"))(second_plugin));
            assert_editor_state(editor_state(plugin), true, true, true, 1200, 720);
            assert_editor_state(editor_state(second_plugin), true, true, true, 640, 480);
            assert!((gui.hide.expect("gui hide"))(plugin));
            assert_editor_state(editor_state(plugin), true, true, false, 1200, 720);
            (gui.destroy.expect("gui destroy"))(plugin);
            assert_editor_state(editor_state(plugin), false, false, false, 1200, 720);
            assert_editor_state(editor_state(second_plugin), true, true, true, 640, 480);
            assert!((second_gui.hide.expect("second gui hide"))(second_plugin));
            (second_gui.destroy.expect("second gui destroy"))(second_plugin);
            assert_editor_state(editor_state(second_plugin), false, false, false, 640, 480);

            let params = ((*plugin).get_extension.expect("params extension"))(
                plugin,
            b"clap.params\0".as_ptr().cast(),
        );
        assert!(!params.is_null());
        let params = &*(params as *const clap_sys::ext::params::clap_plugin_params);
        let second_params = ((*second_plugin)
            .get_extension
            .expect("second params extension"))(
            second_plugin,
            b"clap.params\0".as_ptr().cast(),
        );
        assert!(!second_params.is_null());
        let second_params =
            &*(second_params as *const clap_sys::ext::params::clap_plugin_params);
        assert_eq!((params.count.expect("param count"))(plugin), 1);
        assert_eq!(
            (second_params.count.expect("second param count"))(second_plugin),
            1
        );
        let mut info =
            std::mem::MaybeUninit::<clap_sys::ext::params::clap_param_info>::zeroed()
                .assume_init();
        assert!((params.get_info.expect("param info"))(plugin, 0, &mut info));
        assert_eq!(info.id, 0);
        assert_eq!(CStr::from_ptr(info.name.as_ptr()).to_string_lossy(), "Gain");
        assert_eq!(info.min_value, -60.0);
        assert_eq!(info.max_value, 6.0);
        assert_eq!(info.default_value, 0.0);
          let mut value = f64::NAN;
          assert!((params.get_value.expect("param value"))(plugin, 0, &mut value));
        assert_eq!(value, 0.0);
        let mut second_value = f64::NAN;
        assert!((second_params
            .get_value
            .expect("second initial param value"))(
            second_plugin,
            0,
            &mut second_value
        ));
        assert_eq!(second_value, 0.0);
          let automation_event = clap_sys::events::clap_event_param_value {
              header: clap_sys::events::clap_event_header {
                  size: std::mem::size_of::<clap_sys::events::clap_event_param_value>() as u32,
                  time: 0,
                  space_id: clap_sys::events::CLAP_CORE_EVENT_SPACE_ID,
                  type_: clap_sys::events::CLAP_EVENT_PARAM_VALUE,
                  flags: 0,
              },
              param_id: 0,
              cookie: ptr::null_mut(),
              note_id: -1,
              port_index: -1,
              channel: -1,
              key: -1,
              value: 2.25,
          };
          let input_event = SingleInputEvent {
              event: automation_event,
          };
          let input_events = clap_sys::events::clap_input_events {
              ctx: (&input_event as *const SingleInputEvent).cast_mut().cast(),
              size: Some(single_input_event_size),
              get: Some(single_input_event_get),
          };
          (params.flush.expect("param flush"))(plugin, &input_events, ptr::null());
          let mut automated_value = f64::NAN;
          assert!((params.get_value.expect("automated param value"))(
              plugin,
              0,
              &mut automated_value,
        ));
        assert_eq!(automated_value, 2.25);
        let mut second_after_automation = f64::NAN;
        assert!((second_params
            .get_value
            .expect("second param remains isolated"))(
            second_plugin,
            0,
            &mut second_after_automation
        ));
        assert_eq!(second_after_automation, 0.0);

          let state = ((*plugin).get_extension.expect("state extension"))(
              plugin,
            b"clap.state\0".as_ptr().cast(),
        );
        assert!(!state.is_null());
        let state = &*(state as *const clap_sys::ext::state::clap_plugin_state);
        let mut saved = Vec::new();
          let ostream = clap_sys::stream::clap_ostream {
              ctx: (&mut saved as *mut Vec<u8>).cast(),
              write: Some(write_stream),
          };
          assert!((state.save.expect("state save"))(plugin, &ostream));
          let saved_state = std::str::from_utf8(&saved).expect("state is utf8");
          assert!(saved_state.starts_with("hawk2ui-state-v1\n"));
          assert!(saved_state.contains("param 0 "));
          let loaded_payload = format!("hawk2ui-state-v1\nparam 0 {}\n", 3.5f64.to_bits());
          let mut read_cursor = ReadCursor {
              bytes: loaded_payload.into_bytes(),
              offset: 0,
          };
          let istream = clap_sys::stream::clap_istream {
              ctx: (&mut read_cursor as *mut ReadCursor).cast(),
              read: Some(read_stream),
          };
          assert!((state.load.expect("state load"))(plugin, &istream));
          let mut loaded_value = f64::NAN;
          assert!((params.get_value.expect("loaded param value"))(
              plugin,
              0,
              &mut loaded_value,
          ));
            assert_eq!(loaded_value, 3.5);
            let applied_response = dispatch_editor(
                *editor_dispatch,
                plugin,
                "command=apply_parameter\nparameter_id=0\nvalue=4.25\n",
            );
            assert!(applied_response.contains("response=parameter_applied"));
            let mut dispatched_value = f64::NAN;
            assert!((params.get_value.expect("dispatched param value"))(
                plugin,
                0,
                &mut dispatched_value,
            ));
            assert_eq!(dispatched_value, 4.25);
            let mut second_after_dispatch = f64::NAN;
            assert!((second_params
                .get_value
                .expect("second param remains isolated after dispatch"))(
                second_plugin,
                0,
                &mut second_after_dispatch
            ));
            assert_eq!(second_after_dispatch, 0.0);
            let saved_response = dispatch_editor(*editor_dispatch, plugin, "command=save_state\n");
            assert!(saved_response.contains("response=state_saved"));
            assert!(saved_response.contains("param.0.bits="));
            let load_command = format!(
                "command=load_state\nparam.0.bits={}\n",
                1.75f64.to_bits()
            );
            let loaded_response = dispatch_editor(*editor_dispatch, plugin, &load_command);
            assert!(loaded_response.contains("response=state_loaded"));
            let mut c_abi_loaded_value = f64::NAN;
            assert!((params.get_value.expect("c abi loaded param value"))(
                plugin,
                0,
                &mut c_abi_loaded_value,
            ));
            assert_eq!(c_abi_loaded_value, 1.75);
            let visual_response = dispatch_editor(
                *editor_dispatch,
                plugin,
                "command=drain_realtime_visuals\npacket_count=2\n",
            );
            assert!(visual_response.contains("response=realtime_visuals_drained"));
            assert!(visual_response.contains("packet_count=2"));
            ((*second_plugin).destroy.expect("second plugin destroy"))(second_plugin);
            ((*plugin).destroy.expect("plugin destroy"))(plugin);
        }
    }

    unsafe fn dispatch_editor(
        dispatch: unsafe extern "C" fn(
            *const clap_sys::plugin::clap_plugin,
            *const u8,
            usize,
            *mut u8,
            usize,
            *mut usize,
        ) -> bool,
        plugin: *const clap_sys::plugin::clap_plugin,
        command: &str,
    ) -> String {
        let mut response = [0u8; 4096];
        let mut response_len = 0usize;
        assert!(unsafe {
            dispatch(
                plugin,
                command.as_ptr(),
                command.len(),
                response.as_mut_ptr(),
                response.len(),
                &mut response_len,
            )
        });
        assert!(response_len <= response.len());
        std::str::from_utf8(&response[..response_len])
            .expect("dispatch response is utf8")
            .to_owned()
    }

  struct ReadCursor {
      bytes: Vec<u8>,
      offset: usize,
  }

    struct SingleInputEvent {
        event: clap_sys::events::clap_event_param_value,
    }

    fn assert_editor_state(
        state: Hawk2uiEditorState,
        created: bool,
        attached: bool,
        visible: bool,
        width: u32,
        height: u32,
    ) {
        assert_eq!(state.created, created);
        assert_eq!(state.attached, attached);
        assert_eq!(state.visible, visible);
        assert_eq!(state.width, width);
        assert_eq!(state.height, height);
    }

unsafe extern "C" fn write_stream(
    stream: *const clap_sys::stream::clap_ostream,
    buffer: *const c_void,
    size: u64,
) -> i64 {
    let output = unsafe { &mut *((*stream).ctx as *mut Vec<u8>) };
    let bytes = unsafe { std::slice::from_raw_parts(buffer.cast::<u8>(), size as usize) };
    output.extend_from_slice(bytes);
    size as i64
}

  unsafe extern "C" fn read_stream(
      stream: *const clap_sys::stream::clap_istream,
      buffer: *mut c_void,
      size: u64,
  ) -> i64 {
      let cursor = unsafe { &mut *((*stream).ctx as *mut ReadCursor) };
      let remaining = cursor.bytes.len().saturating_sub(cursor.offset);
      let read_len = remaining.min(size as usize);
      if read_len == 0 {
          return 0;
      }
      unsafe {
          std::ptr::copy_nonoverlapping(
              cursor.bytes.as_ptr().add(cursor.offset),
              buffer.cast::<u8>(),
              read_len,
          );
      }
      cursor.offset += read_len;
      read_len as i64
  }

  unsafe extern "C" fn single_input_event_size(
      _list: *const clap_sys::events::clap_input_events,
  ) -> u32 {
      1
  }

  unsafe extern "C" fn single_input_event_get(
      list: *const clap_sys::events::clap_input_events,
      index: u32,
  ) -> *const clap_sys::events::clap_event_header {
      if index != 0 {
          return ptr::null();
      }
      let input = unsafe { &*((*list).ctx as *const SingleInputEvent) };
      &input.event.header
  }
  "#
}

#[test]
fn plugin_adapters_materialize_format_specific_layouts_and_hash_manifest() {
    let metadata = FormatMetadata::new("com.hawk2ui.layout", "Layout", "Hawk2UI").version("2.0.0");
    let output_root = std::env::temp_dir().join(format!(
        "hawk2ui-plugin-layouts-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));
    let request = PackageRequest::new(
        metadata,
        BundleOutput::new(output_root.to_string_lossy(), "Layout"),
        ParameterModel::new([]),
    )
    .with_format(PackageFormat::Clap)
    .with_format(PackageFormat::Vst3)
    .with_format(PackageFormat::Au)
    .with_format(PackageFormat::Standalone);

    let plan = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds");
    let outputs = plan.materialize().expect("materialization succeeds");

    for output in &outputs {
        let root = Path::new(&output.output_path);
        assert!(
            root.join("Contents/Resources/hawk2ui-artifact.toml")
                .is_file()
        );
        assert!(
            root.join("Contents/Resources/hawk2ui-hashes.toml")
                .is_file()
        );
        let hashes = std::fs::read_to_string(root.join("Contents/Resources/hawk2ui-hashes.toml"))
            .expect("hash manifest reads");
        assert!(hashes.contains("algorithm = \"sha256\""));
        assert!(hashes.contains("hawk2ui-package.toml"));
        assert!(hashes.contains("Contents/Resources/hawk2ui-artifact.toml"));
        match output.format {
            PackageFormat::Clap => {
                assert!(root.join("Layout.clap").is_file());
                assert!(root.join("Contents/Resources/clap.json").is_file());
                assert!(hashes.contains("Contents/Resources/clap.json"));
            }
            PackageFormat::Vst3 => {
                assert!(root.join("Contents/Info.plist").is_file());
                assert!(root.join("Contents/x86_64-linux/Layout.vst3").is_file());
                let generated_cargo = root.join("Contents/Resources/generated-vst3/Cargo.toml");
                let generated_lib = root.join("Contents/Resources/generated-vst3/src/lib.rs");
                assert!(generated_cargo.is_file());
                assert!(generated_lib.is_file());
                let generated_cargo =
                    std::fs::read_to_string(generated_cargo).expect("VST3 scaffold manifest reads");
                let generated_lib =
                    std::fs::read_to_string(generated_lib).expect("VST3 scaffold source reads");
                assert!(
                    generated_cargo.contains("[workspace]"),
                    "generated VST3 cdylib must be buildable as a standalone Cargo workspace"
                );
                assert!(generated_cargo.contains("hawk2ui-vst3"));
                assert!(generated_cargo.contains("vst3 = \"0.3.0\""));
                assert!(generated_lib.contains("Vst3ClassId"));
                assert!(generated_lib.contains("GetPluginFactory"));
                assert!(generated_lib.contains("ComWrapper::new(Hawk2uiVst3Factory)"));
                assert!(!generated_lib.contains("std::ptr::null_mut()\n}"));
                assert!(generated_lib.contains("unsafe fn createInstance"));
                assert!(hashes.contains("Contents/Info.plist"));
                assert!(hashes.contains("Contents/Resources/generated-vst3/Cargo.toml"));
                assert!(hashes.contains("Contents/Resources/generated-vst3/src/lib.rs"));
            }
            PackageFormat::Au => {
                assert!(root.join("Contents/Info.plist").is_file());
                assert!(root.join("Contents/MacOS/Layout").is_file());
                assert!(hashes.contains("Contents/MacOS/Layout"));
            }
            PackageFormat::Standalone => {
                assert!(root.join("Contents/Info.plist").is_file());
                assert!(root.join("Contents/MacOS/Layout").is_file());
                assert!(
                    root.join("Contents/Resources/hawk2ui-launch.toml")
                        .is_file()
                );
                assert!(hashes.contains("Contents/Resources/hawk2ui-launch.toml"));
            }
            PackageFormat::DesktopBundle | PackageFormat::SealedArtifact => {
                panic!("unexpected format in layout test");
            }
        }
    }
}

#[test]
fn plugin_adapters_materialize_removes_stale_output_payloads() {
    let metadata = FormatMetadata::new("com.hawk2ui.clean", "Clean", "Hawk2UI").version("1.0.0");
    let output_root = std::env::temp_dir().join(format!(
        "hawk2ui-plugin-clean-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));
    let request = PackageRequest::new(
        metadata,
        BundleOutput::new(output_root.to_string_lossy(), "Clean"),
        ParameterModel::new([]),
    )
    .with_format(PackageFormat::Clap);

    let plan = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds");
    let outputs = plan
        .materialize()
        .expect("initial materialization succeeds");
    let stale_path = Path::new(&outputs[0].output_path).join("Contents/Resources/stale.bin");
    std::fs::write(&stale_path, "stale payload").expect("stale payload should be writable");

    let outputs = plan
        .materialize()
        .expect("repeat materialization should succeed");

    assert!(!stale_path.exists());
    assert_eq!(
        plan.verify_materialized(&outputs).status(),
        VerificationStatus::Passed
    );
}

#[test]
fn plugin_adapters_verify_materialized_rejects_tampered_package_payloads() {
    let metadata = FormatMetadata::new("com.hawk2ui.tamper", "Tamper", "Hawk2UI").version("3.0.0");
    let output_root = std::env::temp_dir().join(format!(
        "hawk2ui-plugin-tamper-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));
    let request = PackageRequest::new(
        metadata,
        BundleOutput::new(output_root.to_string_lossy(), "Tamper"),
        ParameterModel::new([]),
    )
    .with_format(PackageFormat::Clap);

    let plan = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds");
    let outputs = plan.materialize().expect("materialization succeeds");
    assert_eq!(
        plan.verify_materialized(&outputs).status(),
        VerificationStatus::Passed
    );

    std::fs::write(
        Path::new(&outputs[0].output_path).join("Tamper.clap"),
        "tampered",
    )
    .expect("entry payload should be writable");

    assert_eq!(
        plan.verify_materialized(&outputs).status(),
        VerificationStatus::Failed
    );
}

#[test]
fn plugin_adapters_verify_materialized_rejects_incomplete_or_extra_hash_coverage() {
    let metadata =
        FormatMetadata::new("com.hawk2ui.coverage", "Coverage", "Hawk2UI").version("4.0.0");
    let output_root = std::env::temp_dir().join(format!(
        "hawk2ui-plugin-coverage-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));
    let request = PackageRequest::new(
        metadata,
        BundleOutput::new(output_root.to_string_lossy(), "Coverage"),
        ParameterModel::new([]),
    )
    .with_format(PackageFormat::Clap);

    let plan = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds");
    let outputs = plan.materialize().expect("materialization succeeds");
    let output = &outputs[0];
    let original_hash_manifest =
        std::fs::read_to_string(&output.hash_manifest_path).expect("hash manifest reads");
    let incomplete_hash_manifest = original_hash_manifest
        .split("\n\n")
        .filter(|entry| !entry.contains("Contents/Resources/clap.json"))
        .collect::<Vec<_>>()
        .join("\n\n");
    std::fs::write(&output.hash_manifest_path, incomplete_hash_manifest)
        .expect("hash manifest should be writable");

    assert_eq!(
        plan.verify_materialized(&outputs).status(),
        VerificationStatus::Failed
    );

    std::fs::write(&output.hash_manifest_path, original_hash_manifest)
        .expect("hash manifest should be restorable");
    std::fs::write(
        Path::new(&output.output_path).join("Contents/Resources/injected.bin"),
        "unexpected payload",
    )
    .expect("extra payload should be writable");

    assert_eq!(
        plan.verify_materialized(&outputs).status(),
        VerificationStatus::Failed
    );
}

#[test]
fn plugin_adapters_escape_package_metadata_in_generated_descriptors() {
    let metadata = FormatMetadata::new("com.hawk2ui.escape", "Quote\"Name&<", "Hawk \"A&B\" <Co>")
        .category("audio \"effect\"")
        .feature("quoted \"feature\"");
    let output_root = std::env::temp_dir().join(format!(
        "hawk2ui-plugin-escape-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));
    let request = PackageRequest::new(
        metadata,
        BundleOutput::new(output_root.to_string_lossy(), "Escape"),
        ParameterModel::new([]),
    )
    .with_format(PackageFormat::Clap)
    .with_format(PackageFormat::Standalone);

    let plan = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds");
    let outputs = plan.materialize().expect("materialization succeeds");

    let clap_root = outputs
        .iter()
        .find(|output| output.format == PackageFormat::Clap)
        .map(|output| Path::new(&output.output_path))
        .expect("clap output exists");
    let standalone_root = outputs
        .iter()
        .find(|output| output.format == PackageFormat::Standalone)
        .map(|output| Path::new(&output.output_path))
        .expect("standalone output exists");
    let package_manifest =
        std::fs::read_to_string(clap_root.join("hawk2ui-package.toml")).expect("manifest reads");
    let clap_manifest = std::fs::read_to_string(clap_root.join("Contents/Resources/clap.json"))
        .expect("clap manifest reads");
    let info_plist =
        std::fs::read_to_string(standalone_root.join("Contents/Info.plist")).expect("plist reads");
    let launch_manifest =
        std::fs::read_to_string(standalone_root.join("Contents/Resources/hawk2ui-launch.toml"))
            .expect("launch manifest reads");

    assert!(package_manifest.contains(r#"display_name = "Quote\"Name&<""#));
    assert!(package_manifest.contains(r#""quoted \"feature\"""#));
    assert!(clap_manifest.contains(r#""name": "Quote\"Name&<""#));
    assert!(clap_manifest.contains(r#""vendor": "Hawk \"A&B\" <Co>""#));
    assert!(info_plist.contains("Quote&quot;Name&amp;&lt;"));
    assert!(info_plist.contains("Hawk &quot;A&amp;B&quot; &lt;Co&gt;"));
    assert!(launch_manifest.contains(r#"entry = "Contents/MacOS/Quote\"Name&<""#));
}

#[test]
fn plugin_adapters_reject_invalid_package_metadata() {
    let request = PackageRequest::new(
        FormatMetadata::new("not-reverse-dns", "Demo", "Hawk2UI"),
        BundleOutput::new("dist", "Demo"),
        ParameterModel::new([]),
    )
    .with_format(PackageFormat::Clap);

    let error = PackageAdapterSet::new()
        .plan(&request)
        .expect_err("invalid metadata must fail");

    assert_eq!(error.diagnostics()[0].rule(), "package.metadata.invalid");
}

#[test]
fn plugin_adapters_reject_path_unsafe_metadata_names() {
    let request = PackageRequest::new(
        FormatMetadata::new("com.hawk2ui.unsafe", "../Escape", "Hawk2UI"),
        BundleOutput::new("dist", "SafeBundle"),
        ParameterModel::new([]),
    )
    .with_format(PackageFormat::Vst3);

    let error = PackageAdapterSet::new()
        .plan(&request)
        .expect_err("path-unsafe display names must fail before materialization");

    assert!(
        error
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.rule() == "package.display-name.invalid")
    );
}

#[test]
fn plugin_adapters_reject_reserved_bundle_names() {
    let request = PackageRequest::new(
        FormatMetadata::new("com.hawk2ui.reserved", "Reserved", "Hawk2UI"),
        BundleOutput::new("dist", "."),
        ParameterModel::new([]),
    )
    .with_format(PackageFormat::Clap);

    let error = PackageAdapterSet::new()
        .plan(&request)
        .expect_err("reserved bundle names must fail before materialization");

    assert!(
        error
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.rule() == "package.bundle-name.invalid")
    );
}
