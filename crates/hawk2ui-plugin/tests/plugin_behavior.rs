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

use hawk2ui_plugin::{
    EditorEvent, EditorKind, EditorParent, PluginEditor, PluginEditorLifecycle, PluginEditorSize,
};

#[test]
fn editor_embedding_records_create_attach_resize_dpi_repaint_and_destroy() {
    let editor = PluginEditor::generated("main-editor", PluginEditorSize::new(640.0, 360.0, 1.0));
    let mut lifecycle = PluginEditorLifecycle::create(editor.clone());

    lifecycle.attach_parent(EditorParent::opaque("host-parent"));
    lifecycle.report_initial_size();
    lifecycle.dpi_changed(1.5);
    lifecycle.host_resize(PluginEditorSize::new(800.0, 500.0, 1.5));
    lifecycle.request_repaint("parameter changed");
    lifecycle.destroy("host closed editor");

    assert_eq!(editor.kind, EditorKind::Generated);
    assert_eq!(lifecycle.current_size().physical_size(), (1200, 750));
    assert_eq!(
        lifecycle.events(),
        &[
            EditorEvent::Created(editor),
            EditorEvent::ParentAttached(EditorParent::opaque("host-parent")),
            EditorEvent::InitialSizeReported(PluginEditorSize::new(640.0, 360.0, 1.0)),
            EditorEvent::DpiChanged(1.5),
            EditorEvent::HostResized(PluginEditorSize::new(800.0, 500.0, 1.5)),
            EditorEvent::RepaintRequested("parameter changed".into()),
            EditorEvent::Destroyed("host closed editor".into()),
        ]
    );
}

#[test]
fn editor_embedding_custom_editors_do_not_assume_top_level_window_ownership() {
    let editor = PluginEditor::custom("reactive-editor", PluginEditorSize::new(480.0, 320.0, 2.0));
    let mut lifecycle = PluginEditorLifecycle::create(editor);
    lifecycle.attach_parent(EditorParent::opaque("vst3-parent"));

    assert!(!lifecycle.assumes_top_level_window_ownership());
    assert_eq!(
        lifecycle.parent(),
        Some(&EditorParent::opaque("vst3-parent"))
    );
}
