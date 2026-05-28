use hawk2ui_vst3::{
    Vst3ClassCategory, Vst3ClassId, Vst3EditorHostParent, Vst3FactoryInfo, Vst3NormalizedValue,
    Vst3PluginClassInfo, Vst3String,
};

#[test]
fn vst3_safe_records_validate_ids_metadata_values_and_parent_handles() {
    let class_id = Vst3ClassId::from_u32s(0x6e33_2252, 0x5422_4a00, 0xaa69_301a, 0xf318_797d);
    assert_eq!(class_id.to_hex(), "6e33225254224a00aa69301af318797d");
    assert_eq!(class_id.as_tuid().len(), 16);

    assert!(Vst3ClassId::from_hex("6e33225254224a00aa69301af318797d").is_ok());
    assert!(Vst3ClassId::from_hex("not-a-class-id").is_err());

    let normalized = Vst3NormalizedValue::new(0.75).expect("value is in VST3 normalized range");
    assert_eq!(normalized.get(), 0.75);
    assert!(Vst3NormalizedValue::new(-0.01).is_err());
    assert!(Vst3NormalizedValue::new(1.01).is_err());

    let factory = Vst3FactoryInfo::new(
        Vst3String::new("Hawk2UI").expect("vendor string validates"),
        Some(Vst3String::new("https://hawk2ui.local").expect("url validates")),
        Some(Vst3String::new("dev@hawk2ui.local").expect("email validates")),
    );
    assert_eq!(factory.vendor().as_str(), "Hawk2UI");

    let class = Vst3PluginClassInfo::new(
        class_id,
        Vst3ClassCategory::AudioModule,
        Vst3String::new("Hawk2UI Demo").expect("class name validates"),
    )
    .expect("class info validates");
    assert_eq!(class.category().as_vst3_category(), "Audio Module Class");

    assert!(Vst3EditorHostParent::from_raw(0).is_err());
    let parent = Vst3EditorHostParent::from_raw(42).expect("nonzero host parent validates");
    assert_eq!(parent.raw_handle(), 42);
}
