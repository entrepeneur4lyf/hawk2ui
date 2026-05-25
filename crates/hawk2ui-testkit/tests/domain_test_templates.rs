use hawk2ui_testkit::templates::{DomainTemplateKind, DomainTemplateSuite, DomainTestTemplate};

#[test]
fn domain_test_templates_provide_every_required_domain_template() {
    let suite = DomainTemplateSuite::production_baseline();

    for kind in [
        DomainTemplateKind::Parser,
        DomainTemplateKind::Validator,
        DomainTemplateKind::StyleProperty,
        DomainTemplateKind::LayoutCalculation,
        DomainTemplateKind::SceneMutation,
        DomainTemplateKind::RendererCommandGeneration,
        DomainTemplateKind::RuntimeScheduling,
        DomainTemplateKind::ManifestHandling,
        DomainTemplateKind::PluginParameterBehavior,
    ] {
        assert!(
            suite.template(kind).is_some(),
            "missing domain template {kind:?}"
        );
    }
}

#[test]
fn domain_test_templates_are_deterministic_compile_fixtures() {
    let template = DomainTestTemplate::new(
        DomainTemplateKind::RendererCommandGeneration,
        "renderer emits stable paint commands",
        "render::paint_commands",
    );

    assert_eq!(
        template.kind(),
        DomainTemplateKind::RendererCommandGeneration
    );
    assert_eq!(template.name(), "renderer emits stable paint commands");
    assert_eq!(template.fixture_key(), "render::paint_commands");
}
