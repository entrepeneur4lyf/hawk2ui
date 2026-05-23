use hawk2ui_style::{
    PropertyGroup, PropertyId, PropertyRegistry, PropertyRequirement, StyleValue, UnitHandling,
    ValueType,
};

#[test]
fn property_registry_exposes_required_property_families() {
    let registry = PropertyRegistry::production();

    for (name, group) in [
        ("display", PropertyGroup::Layout),
        ("font-size", PropertyGroup::Typography),
        ("color", PropertyGroup::Color),
        ("border-width", PropertyGroup::Border),
        ("border-radius", PropertyGroup::Radius),
        ("box-shadow", PropertyGroup::Shadow),
        ("transform", PropertyGroup::Transform),
        ("opacity", PropertyGroup::Compositing),
        ("overflow", PropertyGroup::Overflow),
        ("--accent-color", PropertyGroup::Custom),
        ("transition-duration", PropertyGroup::Transition),
        ("background-color", PropertyGroup::TokenReference),
    ] {
        let metadata = registry
            .metadata(&PropertyId::new(name))
            .expect("required property metadata must exist");
        assert_eq!(metadata.group(), group);
    }
}

#[test]
fn property_registry_tracks_defaults_inheritance_units_and_requirements() {
    let registry = PropertyRegistry::production();
    let font_size = registry
        .metadata(&PropertyId::new("font-size"))
        .expect("font-size metadata exists");
    let opacity = registry
        .metadata(&PropertyId::new("opacity"))
        .expect("opacity metadata exists");

    assert_eq!(font_size.value_type(), ValueType::Length);
    assert_eq!(font_size.default_value(), &StyleValue::LengthPx(16.0));
    assert!(font_size.inherited());
    assert_eq!(font_size.unit_handling(), UnitHandling::PxOnly);
    assert!(font_size.requires(PropertyRequirement::Layout));

    assert_eq!(opacity.value_type(), ValueType::Number);
    assert_eq!(opacity.default_value(), &StyleValue::Number(1.0));
    assert!(!opacity.inherited());
    assert!(opacity.requires(PropertyRequirement::Renderer));
}

#[test]
fn property_registry_validates_typed_values() {
    let registry = PropertyRegistry::production();

    assert!(
        registry
            .validate(&PropertyId::new("opacity"), &StyleValue::Number(0.5))
            .is_ok()
    );
    assert!(
        registry
            .validate(&PropertyId::new("opacity"), &StyleValue::Number(1.5))
            .is_err()
    );
    assert!(
        registry
            .validate(
                &PropertyId::new("background-color"),
                &StyleValue::TokenRef("color.surface".to_string()),
            )
            .is_ok()
    );
}

#[test]
fn selector_subset_accepts_supported_forms() {
    let selectors = [
        ("button", "element(button)"),
        (".primary", "class(primary)"),
        ("#submit", "id(submit)"),
        ("panel > button", "element(panel)>element(button)"),
        ("panel button", "element(panel) element(button)"),
        ("button:hawk(active)", "element(button):state(active)"),
    ];

    for (source, key) in selectors {
        let selector = hawk2ui_style::Selector::parse(source).expect("selector must parse");
        assert_eq!(selector.stable_key(), key);
    }
}

#[test]
fn selector_subset_rejects_unsupported_forms_with_diagnostics() {
    for (source, rule) in [
        ("button + label", "selector.combinator.unsupported"),
        ("button:hover", "selector.state.unsupported"),
        ("[aria-label]", "selector.attribute.unsupported"),
        ("button, label", "selector.list.unsupported"),
    ] {
        let error = hawk2ui_style::Selector::parse(source).expect_err("selector must be rejected");
        assert_eq!(error.diagnostic().rule(), rule);
    }
}
