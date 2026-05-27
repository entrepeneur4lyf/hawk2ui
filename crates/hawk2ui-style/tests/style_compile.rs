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
            .validate(&PropertyId::new("font-size"), &StyleValue::LengthPx(-1.0))
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
fn style_subset_reference_lists_exact_supported_css_surface() {
    let reference = hawk2ui_style::StyleSubsetReference::production();
    let registry = PropertyRegistry::production();

    assert_eq!(
        reference.selectors(),
        &[
            "element",
            "class",
            "id",
            "direct-child",
            "descendant",
            ":hawk(state)",
        ]
    );
    assert_eq!(
        reference.units(),
        &["px", "unitless-zero", "unitless-number", "ms", "s"]
    );
    assert_eq!(reference.functions(), &["rgb()", "rgba()", "token()"]);
    assert_eq!(
        reference.rejected_syntax(),
        &[
            "selector-list",
            "attribute-selector",
            "sibling-combinator",
            "non-hawk-pseudo-class",
            "shorthand-property",
            "css-var-function",
            "keyframes",
            "conditional-at-rule",
        ]
    );

    for property in reference.properties() {
        assert!(
            registry.metadata(&PropertyId::new(*property)).is_some(),
            "subset reference listed property missing from registry: {property}"
        );
    }
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
        (".", "selector.syntax.invalid"),
        ("#", "selector.syntax.invalid"),
        ("button:hawk()", "selector.state.invalid"),
    ] {
        let error = hawk2ui_style::Selector::parse(source).expect_err("selector must be rejected");
        assert_eq!(error.diagnostic().rule(), rule);
    }
}

#[test]
fn token_records_resolve_supported_token_families() {
    let tokens = hawk2ui_style::TokenSet::production()
        .with_color("color.surface", 12, 14, 18, 255)
        .with_spacing("space.md", 16.0)
        .with_radius("radius.card", 12.0)
        .with_typography("type.body", "Atkinson Hyperlegible", 16.0)
        .with_motion("motion.fast", 120)
        .with_preference_hook("preference.reduced-motion", "motion.none");

    assert_eq!(
        tokens.resolve("color.surface").unwrap().value(),
        &hawk2ui_style::TokenValue::ColorRgba(12, 14, 18, 255)
    );
    assert_eq!(
        tokens.resolve("space.md").unwrap().value(),
        &hawk2ui_style::TokenValue::LengthPx(16.0)
    );
    assert_eq!(
        tokens.resolve("type.body").unwrap().kind(),
        hawk2ui_style::TokenKind::Typography
    );
    assert_eq!(
        tokens.resolve("preference.reduced-motion").unwrap().kind(),
        hawk2ui_style::TokenKind::PreferenceHook
    );

    let invalid_name = hawk2ui_style::TokenSet::production().with_spacing("space bad", 8.0);
    assert_eq!(
        invalid_name
            .resolve("space bad")
            .expect_err("invalid token name must fail")
            .diagnostic()
            .rule(),
        "token.name.invalid"
    );

    let invalid_length = hawk2ui_style::TokenSet::production().with_spacing("space.bad", -8.0);
    assert_eq!(
        invalid_length
            .resolve("space.bad")
            .expect_err("invalid token length must fail")
            .diagnostic()
            .rule(),
        "token.value.invalid"
    );
}

#[test]
fn token_records_report_missing_tokens_and_select_theme_variants() {
    let dark_theme = hawk2ui_style::ThemeVariant::new("dark").with_token(
        "color.surface",
        hawk2ui_style::TokenValue::ColorRgba(8, 10, 14, 255),
    );
    let light_theme = hawk2ui_style::ThemeVariant::new("light").with_token(
        "color.surface",
        hawk2ui_style::TokenValue::ColorRgba(245, 243, 238, 255),
    );
    let tokens = hawk2ui_style::TokenSet::production()
        .with_theme(dark_theme)
        .with_theme(light_theme);

    let missing = tokens
        .resolve("color.missing")
        .expect_err("missing token must fail");
    assert_eq!(missing.diagnostic().rule(), "token.missing");

    assert_eq!(
        tokens
            .resolve_for_theme("color.surface", "light")
            .unwrap()
            .value(),
        &hawk2ui_style::TokenValue::ColorRgba(245, 243, 238, 255)
    );
}

#[test]
fn style_compile_lowers_supported_declarations_to_typed_records() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures/style/basic.hawk.css"),
    )
    .expect("style fixture must be readable");

    let sheet = hawk2ui_style::compile_style_source(&source).expect("style source must compile");
    let rule = sheet
        .rule("class(primary)")
        .expect("compiled class rule must exist");

    assert_eq!(
        rule.declaration(&PropertyId::new("font-size"))
            .unwrap()
            .value(),
        &StyleValue::LengthPx(18.0)
    );
    assert_eq!(
        rule.declaration(&PropertyId::new("background-color"))
            .unwrap()
            .value(),
        &StyleValue::TokenRef("color.surface".to_string())
    );
    assert_eq!(
        rule.declaration(&PropertyId::new("opacity"))
            .unwrap()
            .value(),
        &StyleValue::Number(0.9)
    );
}

#[test]
fn style_compile_uses_css_parser_semantics_for_comments() {
    let sheet = hawk2ui_style::compile_style_source(
        r#"
/* author-facing stylesheet comment */
.primary {
  /* declaration comment */
  font-size: 18px;
  color: rgb(240, 245, 255);
}
"#,
    )
    .expect("CSS comments must not affect supported style compilation");

    let rule = sheet
        .rule("class(primary)")
        .expect("commented class rule must compile");

    assert_eq!(
        rule.declaration(&PropertyId::new("font-size"))
            .unwrap()
            .value(),
        &StyleValue::LengthPx(18.0)
    );
    assert_eq!(
        rule.declaration(&PropertyId::new("color")).unwrap().value(),
        &StyleValue::ColorRgba(240, 245, 255, 255)
    );
}

#[test]
fn style_compile_accepts_css_unitless_zero_for_lengths() {
    let sheet = hawk2ui_style::compile_style_source(".zero { font-size: 0; }")
        .expect("CSS unitless zero is a valid length token");
    let rule = sheet.rule("class(zero)").expect("zero rule exists");

    assert_eq!(
        rule.declaration(&PropertyId::new("font-size"))
            .unwrap()
            .value(),
        &StyleValue::LengthPx(0.0)
    );
}

#[test]
fn style_compile_lowers_color_duration_shadow_and_transform_values() {
    let sheet = hawk2ui_style::compile_style_source(
        r#"
.visual {
  color: #f0f5ff;
  transition-duration: 120ms;
  box-shadow: 0px 8px 24px rgba(0,0,0,0.35);
  transform: translateX(12px);
}
.overlay {
  color: rgba(240, 245, 255, 0.5);
}
"#,
    )
    .expect("production visual values must compile");

    let visual = sheet.rule("class(visual)").expect("visual rule exists");
    assert_eq!(
        visual
            .declaration(&PropertyId::new("color"))
            .unwrap()
            .value(),
        &StyleValue::ColorRgba(240, 245, 255, 255)
    );
    assert_eq!(
        visual
            .declaration(&PropertyId::new("transition-duration"))
            .unwrap()
            .value(),
        &StyleValue::DurationMs(120)
    );
    assert_eq!(
        visual
            .declaration(&PropertyId::new("box-shadow"))
            .unwrap()
            .value(),
        &StyleValue::Shadow("0 8px 24px #00000059".to_string())
    );
    assert_eq!(
        visual
            .declaration(&PropertyId::new("transform"))
            .unwrap()
            .value(),
        &StyleValue::Transform("translateX(12px)".to_string())
    );

    let overlay = sheet.rule("class(overlay)").expect("overlay rule exists");
    assert_eq!(
        overlay
            .declaration(&PropertyId::new("color"))
            .unwrap()
            .value(),
        &StyleValue::ColorRgba(240, 245, 255, 128)
    );
}

#[test]
fn style_compile_accepts_exact_units_functions_tokens_and_transitions() {
    let sheet = hawk2ui_style::compile_style_source(
        r#"
.exact {
  font-size: 20px;
  border-width: 0;
  opacity: 0.75;
  color: rgb(12, 24, 36);
  background-color: token(color.surface);
  transition-duration: 0.25s;
}
"#,
    )
    .expect("exact supported CSS subset must compile");
    let rule = sheet.rule("class(exact)").expect("exact rule exists");

    assert_eq!(
        rule.declaration(&PropertyId::new("font-size"))
            .unwrap()
            .value(),
        &StyleValue::LengthPx(20.0)
    );
    assert_eq!(
        rule.declaration(&PropertyId::new("border-width"))
            .unwrap()
            .value(),
        &StyleValue::LengthPx(0.0)
    );
    assert_eq!(
        rule.declaration(&PropertyId::new("color")).unwrap().value(),
        &StyleValue::ColorRgba(12, 24, 36, 255)
    );
    assert_eq!(
        rule.declaration(&PropertyId::new("background-color"))
            .unwrap()
            .value(),
        &StyleValue::TokenRef("color.surface".to_string())
    );
    assert_eq!(
        rule.declaration(&PropertyId::new("transition-duration"))
            .unwrap()
            .value(),
        &StyleValue::DurationMs(250)
    );
}

#[test]
fn style_compile_rejects_every_documented_unsupported_css_class() {
    for (source, rule) in [
        (".card { margin: 8px; }", "style.shorthand.unsupported"),
        (
            ".card { transition: opacity 120ms; }",
            "style.shorthand.unsupported",
        ),
        (".card { font-size: 1rem; }", "style.unit.unsupported"),
        (
            ".card { color: var(--accent-color); }",
            "style.function.unsupported",
        ),
        (
            "@keyframes pulse { from { opacity: 0; } to { opacity: 1; } }",
            "style.keyframes.unsupported",
        ),
        (
            "@media (min-width: 400px) { .card { opacity: 1; } }",
            "style.at-rule.unsupported",
        ),
    ] {
        let error = hawk2ui_style::compile_style_source(source)
            .expect_err("unsupported CSS class must fail");
        assert_eq!(
            error.diagnostics()[0].rule(),
            rule,
            "unexpected diagnostic for {source}"
        );
    }
}

#[test]
fn style_compile_rejects_unsupported_syntax_with_diagnostics() {
    let error = hawk2ui_style::compile_style_source("button + label { opacity: 0.5; }")
        .expect_err("unsupported selector must fail");

    assert_eq!(
        error.diagnostics()[0].rule(),
        "selector.combinator.unsupported"
    );

    let error = hawk2ui_style::compile_style_source(".card { unknown-property: 1px; }")
        .expect_err("unknown property must fail");

    assert_eq!(error.diagnostics()[0].rule(), "style.property.unknown");

    let error = hawk2ui_style::compile_style_source(".card { font-size: -1px; }")
        .expect_err("negative lengths must fail");

    assert_eq!(error.diagnostics()[0].rule(), "style.value.range");
}

#[test]
fn runtime_style_table_returns_typed_values_by_node_and_property() {
    let table = hawk2ui_style::RuntimeStyleTable::new()
        .with_value(
            "button-1",
            PropertyId::new("opacity"),
            StyleValue::Number(0.8),
        )
        .with_value(
            "button-1",
            PropertyId::new("font-size"),
            StyleValue::LengthPx(18.0),
        );

    assert_eq!(
        table.typed_value("button-1", &PropertyId::new("opacity")),
        Some(&StyleValue::Number(0.8))
    );
    assert_eq!(
        table.typed_value("button-1", &PropertyId::new("font-size")),
        Some(&StyleValue::LengthPx(18.0))
    );
    assert_eq!(
        table.typed_value("missing", &PropertyId::new("opacity")),
        None
    );
}

#[test]
fn runtime_style_table_resolves_ordered_style_refs_with_later_precedence() {
    let sheet = hawk2ui_style::compile_style_source(
        r#"
.surface {
  opacity: 0.6;
  font-size: 16px;
}
.intent-primary {
  opacity: 0.9;
}
"#,
    )
    .expect("style source must compile");

    let table = hawk2ui_style::RuntimeStyleTable::from_style_refs(
        "button-1",
        &sheet,
        ["surface", "intent-primary"],
    )
    .expect("style refs must resolve");

    assert_eq!(
        table.typed_value("button-1", &PropertyId::new("font-size")),
        Some(&StyleValue::LengthPx(16.0))
    );
    assert_eq!(
        table.typed_value("button-1", &PropertyId::new("opacity")),
        Some(&StyleValue::Number(0.9))
    );
}

#[test]
fn runtime_style_table_reports_missing_style_refs() {
    let sheet = hawk2ui_style::compile_style_source(".surface { opacity: 0.6; }")
        .expect("style source must compile");

    let error = hawk2ui_style::RuntimeStyleTable::from_style_refs(
        "button-1",
        &sheet,
        ["surface", "missing"],
    )
    .expect_err("missing style refs must fail");

    assert_eq!(error.diagnostic().rule(), "runtime-style.ref.missing");
    assert!(error.diagnostic().message().contains("missing"));
}

#[test]
fn runtime_style_table_resolves_token_backed_declarations() {
    let sheet =
        hawk2ui_style::compile_style_source(".surface { background-color: token(color.surface); }")
            .expect("style source must compile");
    let tokens = hawk2ui_style::TokenSet::production().with_color("color.surface", 8, 10, 14, 255);

    let table = hawk2ui_style::RuntimeStyleTable::from_style_refs_with_tokens(
        "panel-1",
        &sheet,
        ["surface"],
        &tokens,
    )
    .expect("style refs and tokens must resolve");

    assert_eq!(
        table.typed_value("panel-1", &PropertyId::new("background-color")),
        Some(&StyleValue::ColorRgba(8, 10, 14, 255))
    );
}

#[test]
fn runtime_style_table_resolves_theme_token_overrides() {
    let sheet =
        hawk2ui_style::compile_style_source(".surface { background-color: token(color.surface); }")
            .expect("style source must compile");
    let tokens = hawk2ui_style::TokenSet::production()
        .with_color("color.surface", 8, 10, 14, 255)
        .with_theme(hawk2ui_style::ThemeVariant::new("light").with_token(
            "color.surface",
            hawk2ui_style::TokenValue::ColorRgba(245, 243, 238, 255),
        ));

    let table = hawk2ui_style::RuntimeStyleTable::from_style_refs_for_theme(
        "panel-1",
        &sheet,
        ["surface"],
        &tokens,
        "light",
    )
    .expect("theme style refs and tokens must resolve");

    assert_eq!(
        table.typed_value("panel-1", &PropertyId::new("background-color")),
        Some(&StyleValue::ColorRgba(245, 243, 238, 255))
    );
}

#[test]
fn runtime_style_table_reports_missing_tokens() {
    let sheet =
        hawk2ui_style::compile_style_source(".surface { background-color: token(color.missing); }")
            .expect("style source must compile");

    let error = hawk2ui_style::RuntimeStyleTable::from_style_refs_with_tokens(
        "panel-1",
        &sheet,
        ["surface"],
        &hawk2ui_style::TokenSet::production(),
    )
    .expect_err("missing token must fail");

    assert_eq!(error.diagnostic().rule(), "runtime-style.token.missing");
    assert!(error.diagnostic().message().contains("color.missing"));
}

#[test]
fn runtime_style_table_rejects_raw_string_values() {
    let error = hawk2ui_style::RuntimeStyleTable::new()
        .try_with_raw_value("button-1", "opacity", "0.8")
        .expect_err("raw style values must be rejected");

    assert_eq!(
        error.diagnostic().rule(),
        "runtime-style.raw-value.rejected"
    );
}
