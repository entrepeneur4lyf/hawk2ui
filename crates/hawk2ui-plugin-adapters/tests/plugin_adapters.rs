use hawk2ui_plugin::{
    BundleOutput, FormatMetadata, ParameterModel, ParameterRange, ParameterRecord,
};
use hawk2ui_plugin_adapters::{
    PackageAdapterSet, PackageFormat, PackageRequest, VerificationStatus,
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

    let report = plan.verify_materialized(&outputs);
    assert_eq!(report.status(), VerificationStatus::Passed);
    std::fs::remove_file(&outputs[0].artifact_descriptor_path)
        .expect("artifact descriptor should be removable");
    let failed = plan.verify_materialized(&outputs);
    assert_eq!(failed.status(), VerificationStatus::Failed);
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
