use hawk2ui_api::{
    AutomationGesture, ParameterId, PluginEditorContract, PluginEditorKind,
    PluginParameterContract, PluginPresetContract, PluginStateContract, PluginStateFormat,
    RealtimeDataContract, RealtimeDataDirection, RealtimeDataKind,
};

#[test]
fn plugin_contract_downstream_code_uses_parameter_editor_and_automation_records() {
    let parameter = PluginParameterContract::new(ParameterId::new("gain"), "Gain", 0.5, true)
        .with_unit("dB")
        .with_normalized_range(0.0, 1.0);
    let editor = PluginEditorContract::new(900, 560, 640, 360)
        .with_kind(PluginEditorKind::Generated)
        .with_resizable(true);
    let gesture = AutomationGesture::Change {
        parameter: ParameterId::new("gain"),
        normalized: 0.75,
    };

    assert_eq!(parameter.id.as_str(), "gain");
    assert_eq!(parameter.unit.as_deref(), Some("dB"));
    assert!(parameter.accepts_normalized(0.75));
    assert_eq!(editor.kind, PluginEditorKind::Generated);
    assert!(editor.resizable);
    assert_eq!(gesture.parameter_id().as_str(), "gain");
}

#[test]
fn plugin_parameter_contract_sanitizes_invalid_normalized_values() {
    let parameter = PluginParameterContract::new(ParameterId::new("gain"), "Gain", f32::NAN, true)
        .with_normalized_range(0.25, 0.75);
    let invalid_range = PluginParameterContract::new(ParameterId::new("mix"), "Mix", 2.0, true)
        .with_normalized_range(f32::INFINITY, 0.25);

    assert!((parameter.default_normalized - 0.25).abs() < f32::EPSILON);
    assert!(!parameter.accepts_normalized(f32::NAN));
    assert!(!parameter.accepts_normalized(0.1));
    assert!(parameter.accepts_normalized(0.5));
    assert!((invalid_range.default_normalized - 1.0).abs() < f32::EPSILON);
    assert!((invalid_range.normalized_min - 0.0).abs() < f32::EPSILON);
    assert!((invalid_range.normalized_max - 1.0).abs() < f32::EPSILON);
}

#[test]
fn plugin_contract_downstream_code_uses_state_and_preset_records() {
    let state = PluginStateContract::new(PluginStateFormat::Json, "application/json")
        .with_entry("gain", "0.5")
        .with_entry("mix", "1.0");
    let preset = PluginPresetContract::new("factory.default", "Default").with_state(state.clone());

    assert_eq!(state.entry("gain"), Some("0.5"));
    assert_eq!(preset.id, "factory.default");
    assert_eq!(preset.state.as_ref().unwrap().entry("mix"), Some("1.0"));
}

#[test]
fn plugin_contract_downstream_code_uses_realtime_data_records() {
    let contract = RealtimeDataContract::new(
        "spectrum",
        RealtimeDataKind::F32Frames,
        RealtimeDataDirection::ProcessorToEditor,
    )
    .with_capacity(2048)
    .with_channel_count(2);

    assert_eq!(contract.name, "spectrum");
    assert_eq!(contract.capacity_frames, 2048);
    assert_eq!(contract.channel_count, 2);
}

#[test]
fn plugin_contract_records_are_stable_json_contracts() {
    let state = PluginStateContract::new(PluginStateFormat::Json, "application/json")
        .with_entry("gain", "0.5");
    let value = serde_json::to_value(&state).expect("state json");

    assert_eq!(
        value,
        serde_json::json!({
            "format": "Json",
            "media_type": "application/json",
            "entries": [{"key": "gain", "value": "0.5"}]
        })
    );
}
