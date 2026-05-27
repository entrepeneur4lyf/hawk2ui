use hawk2ui_plugin::{
    BundleOutput, FormatMetadata, ParameterModel, ParameterRange, ParameterRecord,
};
use hawk2ui_plugin_adapters::{
    ClapCdylibScaffold, ClapPluginEntryPlan, MaterializedPackageOutput, PackageAdapterSet,
    PackageFormat, PackagePlan, PackageRequest, VerificationReport, VerificationStatus,
};
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
fn plugin_adapters_generate_compilable_clap_cdylib_scaffold() {
    let metadata = FormatMetadata::new("com.hawk2ui.loadable", "Loadable", "Hawk2UI")
        .version("1.0.0")
        .feature("audio-effect");
    let output_root = std::env::temp_dir().join(format!(
        "hawk2ui-clap-cdylib-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));

    let scaffold = ClapCdylibScaffold::from_metadata(&metadata);
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
                assert!(hashes.contains("Contents/Info.plist"));
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
