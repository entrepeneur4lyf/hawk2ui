use hawk2ui_api::{Diagnostic, DiagnosticSeverity};
use hawk2ui_plugin::{
    AutomationEvent, AutomationOrigin, AutomationSequence, BundleOutput, FormatMetadata,
    PackageTarget, PluginFormat, PluginFormatTarget,
};

#[test]
fn automation_event_error_converts_to_shared_diagnostic_with_parameter_context() {
    let mut sequence = AutomationSequence::default();
    let error = sequence
        .push(AutomationEvent::end_gesture("gain", AutomationOrigin::Ui))
        .expect_err("ending a missing gesture is rejected");
    let diagnostic = Diagnostic::from(error);

    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostic.rule.as_str(), "automation.gesture-not-open");
    assert!(
        diagnostic
            .related
            .iter()
            .any(|context| context.label == "parameter" && context.value == "gain")
    );
}

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

#[test]
fn format_records_generate_and_validate_package_metadata_schema() {
    let target = PluginFormatTarget::vst3(
        FormatMetadata::new("com.hawk2ui.delay", "Delay", "Hawk2")
            .version("1.2.3")
            .category("delay")
            .feature("stereo"),
        BundleOutput::new("dist/Delay.vst3", "Delay.vst3"),
    );
    let target_json = serde_json::to_value(&target).expect("plugin target serializes");

    let schema = PluginFormatTarget::json_schema().expect("plugin target schema generates");
    PluginFormatTarget::validate_json(&target_json).expect("plugin target schema accepts target");

    let schema_text = schema.to_string();
    assert!(schema_text.contains("metadata"));
    assert!(schema_text.contains("output"));
    assert!(schema_text.contains("display_name"));
    assert!(schema_text.contains("bundle_name"));
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

use hawk2ui_plugin::{
    ParameterDistribution, ParameterFlags, ParameterGroup, ParameterModel, ParameterRange,
    ParameterRecord, ParameterSmoothing, ParameterValue,
};

#[test]
fn parameter_model_validates_stable_ids_and_group_nesting() {
    let group = ParameterGroup::new("filter", "Filter")
        .child(ParameterGroup::new("filter.env", "Envelope"));
    let model = ParameterModel::new([ParameterRecord::numeric(
        "filter.cutoff",
        "Cutoff",
        "Hz",
        ParameterRange::new(20.0, 20_000.0, 1_000.0),
    )
    .group("filter.env")])
    .with_group(group);

    assert!(model.validate().is_ok());
    assert!(model.group_path("filter.env").is_some());

    let invalid = ParameterModel::new([ParameterRecord::boolean("Bad Id", "Enabled", true)]);
    let errors = invalid.validate().expect_err("invalid id should fail");
    assert_eq!(errors[0].code, "parameter.id-invalid");
}

#[test]
fn parameter_model_converts_normalized_values_and_display_text() {
    let parameter =
        ParameterRecord::numeric("gain", "Gain", "dB", ParameterRange::new(-60.0, 12.0, 0.0))
            .distribution(ParameterDistribution::Linear)
            .flags(ParameterFlags::automatable());

    let value = parameter
        .denormalize(0.5)
        .expect("normalized value should convert");

    assert_eq!(value, ParameterValue::Float(-24.0));
    assert_eq!(
        parameter.normalize(&ParameterValue::Float(-24.0)).unwrap(),
        0.5
    );
    assert_eq!(parameter.display_value(&value).unwrap(), "-24 dB");
    assert!(parameter.flags.automatable);
}

#[test]
fn parameter_model_supports_stepped_values_and_smoothing_metadata() {
    let parameter =
        ParameterRecord::numeric("mode", "Mode", "", ParameterRange::new(0.0, 3.0, 0.0))
            .steps(4)
            .smoothing(ParameterSmoothing::linear_ms(10.0));

    assert_eq!(
        parameter.denormalize(0.6).unwrap(),
        ParameterValue::Float(2.0)
    );
    assert_eq!(
        parameter.normalize(&ParameterValue::Float(2.0)).unwrap(),
        0.6666666666666666
    );
    assert_eq!(parameter.generated_metadata().steps, Some(4));
    assert_eq!(
        parameter.smoothing,
        Some(ParameterSmoothing::linear_ms(10.0))
    );
}

use hawk2ui_plugin::{AutomationBindingKind, AutomationEventKind, ParameterBinding};

#[test]
fn automation_events_accept_correct_gesture_ordering() {
    let mut sequence = AutomationSequence::default();

    sequence
        .push(AutomationEvent::begin_gesture("gain", AutomationOrigin::Ui))
        .unwrap();
    sequence
        .push(AutomationEvent::value_change(
            "gain",
            AutomationOrigin::Ui,
            0.75,
        ))
        .unwrap();
    sequence
        .push(AutomationEvent::end_gesture("gain", AutomationOrigin::Ui))
        .unwrap();
    sequence
        .push(AutomationEvent::host_update("gain", 0.5))
        .unwrap();

    assert_eq!(sequence.events()[0].kind, AutomationEventKind::BeginGesture);
    assert_eq!(sequence.events()[1].normalized_value, Some(0.75));
    assert_eq!(sequence.events()[3].origin, AutomationOrigin::Host);
}

#[test]
fn automation_events_reject_duplicate_open_gestures() {
    let mut sequence = AutomationSequence::default();
    sequence
        .push(AutomationEvent::begin_gesture("gain", AutomationOrigin::Ui))
        .unwrap();

    let error = sequence
        .push(AutomationEvent::begin_gesture("gain", AutomationOrigin::Ui))
        .expect_err("duplicate gesture must fail");

    assert_eq!(error.code, "automation.duplicate-gesture");
}

#[test]
fn automation_events_record_generated_and_custom_editor_bindings() {
    let generated = ParameterBinding::generated_editor("gain", "slider:generation");
    let custom = ParameterBinding::custom_editor("cutoff", "react:CutoffKnob");

    assert_eq!(generated.kind, AutomationBindingKind::GeneratedEditor);
    assert_eq!(custom.kind, AutomationBindingKind::CustomEditor);
    assert_eq!(generated.parameter_id, "gain");
}

use hawk2ui_plugin::{
    HostStateChunk, PluginPreset, PluginStateEnvelope, PresetKind, PresetMetadata, StateMigration,
    StateValue, UiPreferences,
};

#[test]
fn state_presets_separate_parameter_non_parameter_and_ui_state() {
    let state = PluginStateEnvelope::new(2)
        .parameter("gain", StateValue::Float(0.5))
        .non_parameter("oversampling", StateValue::Bool(true))
        .ui_preferences(
            UiPreferences::new()
                .window_size(800.0, 500.0)
                .theme("graphite"),
        )
        .host_chunk(HostStateChunk::new("vst3", vec![1, 2, 3]));

    assert_eq!(state.version, 2);
    assert_eq!(
        state.parameter_state.get("gain"),
        Some(&StateValue::Float(0.5))
    );
    assert_eq!(
        state.non_parameter_state.get("oversampling"),
        Some(&StateValue::Bool(true))
    );
    assert_eq!(state.ui_preferences.theme.as_deref(), Some("graphite"));
    assert_eq!(state.host_chunks[0].format, "vst3");
}

#[test]
fn state_presets_apply_migrations_in_order() {
    let state = PluginStateEnvelope::new(1).parameter("old_gain", StateValue::Float(0.25));
    let migrated = state
        .migrate([StateMigration::rename_parameter(1, 2, "old_gain", "gain")])
        .expect("migration should apply");

    assert_eq!(migrated.version, 2);
    assert_eq!(
        migrated.parameter_state.get("gain"),
        Some(&StateValue::Float(0.25))
    );
    assert!(!migrated.parameter_state.contains_key("old_gain"));
}

#[test]
fn state_presets_keep_factory_and_user_presets_separate() {
    let factory = PluginPreset::factory(
        PresetMetadata::new("factory.init", "Init", "Hawk2"),
        PluginStateEnvelope::new(1),
    );
    let user = PluginPreset::user(
        PresetMetadata::new("user.wide", "Wide", "Shawn"),
        PluginStateEnvelope::new(1),
    );

    assert_eq!(factory.kind, PresetKind::Factory);
    assert_eq!(user.kind, PresetKind::User);
    assert_ne!(factory.metadata.author, user.metadata.author);
}

use hawk2ui_plugin::{
    FrameDropPolicy, RealtimeChannelKind, RealtimeVisualFrameGate, RealtimeVisualPacket,
    RealtimeVisualTransport,
};

#[test]
fn realtime_visual_data_records_meters_analyzers_scopes_and_modulation() {
    let packets = [
        RealtimeVisualPacket::meter("out", 0.9),
        RealtimeVisualPacket::analyzer("fft", vec![0.1, 0.2]),
        RealtimeVisualPacket::scope("osc", vec![-0.5, 0.5]),
        RealtimeVisualPacket::modulation("lfo", 0.25),
    ];

    assert_eq!(packets[0].kind, RealtimeChannelKind::Meter);
    assert_eq!(packets[1].kind, RealtimeChannelKind::Analyzer);
    assert_eq!(packets[2].kind, RealtimeChannelKind::Scope);
    assert_eq!(packets[3].kind, RealtimeChannelKind::Modulation);
}

#[test]
fn realtime_visual_data_audio_thread_write_is_non_blocking_and_preallocated() {
    let mut transport = RealtimeVisualTransport::preallocated(2, FrameDropPolicy::DropOldest);

    assert!(
        transport
            .audio_thread_push(RealtimeVisualPacket::meter("out", 0.1))
            .accepted
    );
    assert!(
        transport
            .audio_thread_push(RealtimeVisualPacket::meter("out", 0.2))
            .accepted
    );
    let third = transport.audio_thread_push(RealtimeVisualPacket::meter("out", 0.3));

    assert!(third.accepted);
    assert_eq!(third.dropped_frames, 1);
    assert_eq!(transport.capacity(), 2);
    assert_eq!(transport.pending_len(), 2);
    assert_eq!(transport.allocation_count(), 0);
    assert_eq!(transport.blocking_wait_count(), 0);
}

#[test]
fn realtime_visual_data_ui_reads_do_not_block_audio_writes() {
    let mut transport = RealtimeVisualTransport::preallocated(4, FrameDropPolicy::DropNewest);
    let _ = transport.audio_thread_push(RealtimeVisualPacket::modulation("lfo", 0.5));

    let packets = transport.ui_drain();
    let write = transport.audio_thread_push(RealtimeVisualPacket::scope("osc", vec![0.0]));

    assert_eq!(packets.len(), 1);
    assert!(write.accepted);
    assert_eq!(transport.blocking_wait_count(), 0);
}

#[test]
fn realtime_visual_data_split_transport_moves_audio_writer_across_threads() {
    let (mut audio_writer, mut ui_reader) =
        RealtimeVisualTransport::split_preallocated(2, FrameDropPolicy::DropNewest);

    let handle = std::thread::spawn(move || {
        let first = audio_writer.audio_thread_push(RealtimeVisualPacket::meter("out", 0.1));
        let second = audio_writer.audio_thread_push(RealtimeVisualPacket::meter("out", 0.2));
        let third = audio_writer.audio_thread_push(RealtimeVisualPacket::meter("out", 0.3));
        (
            first,
            second,
            third,
            audio_writer.allocation_count(),
            audio_writer.blocking_wait_count(),
        )
    });

    let (first, second, third, allocations, blocking_waits) =
        handle.join().expect("audio writer thread should not panic");
    let packets = ui_reader.ui_drain();

    assert!(first.accepted);
    assert!(second.accepted);
    assert!(!third.accepted);
    assert_eq!(third.dropped_frames, 1);
    assert_eq!(allocations, 0);
    assert_eq!(blocking_waits, 0);
    assert_eq!(ui_reader.capacity(), 2);
    assert_eq!(packets.len(), 2);
    assert_eq!(packets[0].values, vec![0.1]);
    assert_eq!(packets[1].values, vec![0.2]);
}

#[test]
fn realtime_visual_data_ui_frame_gate_reduces_drain_cadence() {
    let (mut audio_writer, mut ui_reader) =
        RealtimeVisualTransport::split_preallocated(4, FrameDropPolicy::DropNewest);
    let mut frame_gate = RealtimeVisualFrameGate::new(30).expect("30hz visual gate is valid");

    assert_eq!(frame_gate.target_hz(), 30);
    assert_eq!(frame_gate.minimum_interval_ms(), 34);

    assert!(
        audio_writer
            .audio_thread_push(RealtimeVisualPacket::meter("out", 0.1))
            .accepted
    );
    let first_frame = ui_reader
        .ui_drain_due(0, &mut frame_gate)
        .expect("first frame should always be due");

    assert_eq!(first_frame.len(), 1);
    assert!(
        audio_writer
            .audio_thread_push(RealtimeVisualPacket::meter("out", 0.2))
            .accepted
    );
    assert!(ui_reader.ui_drain_due(16, &mut frame_gate).is_none());
    assert_eq!(ui_reader.pending_len(), 1);

    let reduced_frame = ui_reader
        .ui_drain_due(34, &mut frame_gate)
        .expect("30hz gate should present after the reduced interval");

    assert_eq!(reduced_frame.len(), 1);
    assert_eq!(reduced_frame[0].values, vec![0.2]);
    assert!(RealtimeVisualFrameGate::new(0).is_err());
}
