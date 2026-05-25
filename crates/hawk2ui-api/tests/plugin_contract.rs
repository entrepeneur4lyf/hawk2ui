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
