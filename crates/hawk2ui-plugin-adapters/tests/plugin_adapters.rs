use hawk2ui_plugin::{
    BundleOutput, FormatMetadata, ParameterModel, ParameterRange, ParameterRecord,
};
use hawk2ui_plugin_adapters::{
    PackageAdapterSet, PackageFormat, PackageRequest, VerificationStatus,
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
