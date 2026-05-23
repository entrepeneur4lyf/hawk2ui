use hawk2ui_plugin::{
    BundleOutput, FormatMetadata, PackageTarget, PluginFormat, PluginFormatTarget,
};

#[test]
fn format_records_validate_target_metadata() {
    let target = PluginFormatTarget::clap(
        FormatMetadata::new("com.hawk2ui.delay", "Hawk2 Delay", "Hawk2 Labs")
            .version("1.2.3")
            .category("delay")
            .feature("stereo"),
        BundleOutput::new("dist/Hawk2Delay.clap", "Hawk2Delay.clap"),
    );

    assert_eq!(target.format, PluginFormat::Clap);
    assert!(target.validate().is_ok());
    assert_eq!(
        target.metadata.generated_display_name(),
        "Hawk2 Delay 1.2.3"
    );
}

#[test]
fn format_records_reject_invalid_metadata() {
    let target = PluginFormatTarget::vst3(
        FormatMetadata::new("not valid id", "", ""),
        BundleOutput::new("dist/bad.vst3", "bad.vst3"),
    );

    let errors = target.validate().expect_err("invalid metadata should fail");
    let codes: Vec<_> = errors.iter().map(|error| error.code.as_str()).collect();

    assert!(codes.contains(&"format.metadata-id-invalid"));
    assert!(codes.contains(&"format.metadata-name-empty"));
    assert!(codes.contains(&"format.metadata-vendor-empty"));
}

#[test]
fn format_records_enumerate_package_targets() {
    let targets = PackageTarget::new([
        PluginFormatTarget::clap(
            FormatMetadata::new("com.hawk2ui.delay", "Delay", "Hawk2"),
            BundleOutput::new("dist/Delay.clap", "Delay.clap"),
        ),
        PluginFormatTarget::vst3(
            FormatMetadata::new("com.hawk2ui.delay", "Delay", "Hawk2"),
            BundleOutput::new("dist/Delay.vst3", "Delay.vst3"),
        ),
        PluginFormatTarget::au(
            FormatMetadata::new("com.hawk2ui.delay", "Delay", "Hawk2"),
            BundleOutput::new("dist/Delay.component", "Delay.component"),
        ),
        PluginFormatTarget::standalone(
            FormatMetadata::new("com.hawk2ui.delay", "Delay", "Hawk2"),
            BundleOutput::new("dist/Delay", "Delay"),
        ),
    ]);

    assert_eq!(
        targets.formats(),
        vec![
            PluginFormat::Clap,
            PluginFormat::Vst3,
            PluginFormat::Au,
            PluginFormat::Standalone,
        ]
    );
    assert_eq!(
        targets.bundle_paths(),
        vec![
            "dist/Delay.clap",
            "dist/Delay.vst3",
            "dist/Delay.component",
            "dist/Delay"
        ]
    );
}
