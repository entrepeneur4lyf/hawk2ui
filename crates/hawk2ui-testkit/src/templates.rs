//! Shared domain test templates for production conformance tests.

/// Required domain test template category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DomainTemplateKind {
    /// Parser behavior template.
    Parser,
    /// Validator behavior template.
    Validator,
    /// Style property behavior template.
    StyleProperty,
    /// Layout calculation behavior template.
    LayoutCalculation,
    /// Scene mutation behavior template.
    SceneMutation,
    /// Renderer command generation behavior template.
    RendererCommandGeneration,
    /// Runtime scheduling behavior template.
    RuntimeScheduling,
    /// Manifest handling behavior template.
    ManifestHandling,
    /// Plugin parameter behavior template.
    PluginParameterBehavior,
}

/// Deterministic test template shared by domain crates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DomainTestTemplate {
    kind: DomainTemplateKind,
    name: String,
    fixture_key: String,
}

impl DomainTestTemplate {
    /// Creates a domain test template.
    #[must_use]
    pub fn new(
        kind: DomainTemplateKind,
        name: impl Into<String>,
        fixture_key: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            name: name.into(),
            fixture_key: fixture_key.into(),
        }
    }

    /// Returns the template kind.
    #[must_use]
    pub const fn kind(&self) -> DomainTemplateKind {
        self.kind
    }

    /// Returns the template name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the fixture key used by deterministic tests.
    #[must_use]
    pub fn fixture_key(&self) -> &str {
        &self.fixture_key
    }
}

/// Production baseline suite of domain test templates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DomainTemplateSuite {
    templates: Vec<DomainTestTemplate>,
}

impl DomainTemplateSuite {
    /// Creates a template suite.
    #[must_use]
    pub fn new(templates: impl IntoIterator<Item = DomainTestTemplate>) -> Self {
        Self {
            templates: templates.into_iter().collect(),
        }
    }

    /// Creates the production baseline suite.
    #[must_use]
    pub fn production_baseline() -> Self {
        use DomainTemplateKind::{
            LayoutCalculation, ManifestHandling, Parser, PluginParameterBehavior,
            RendererCommandGeneration, RuntimeScheduling, SceneMutation, StyleProperty, Validator,
        };

        Self::new([
            DomainTestTemplate::new(
                Parser,
                "parser accepts valid source",
                "parser::valid-source",
            ),
            DomainTestTemplate::new(
                Validator,
                "validator rejects invalid source",
                "validator::invalid-source",
            ),
            DomainTestTemplate::new(
                StyleProperty,
                "style property registry validates values",
                "style::property-values",
            ),
            DomainTestTemplate::new(
                LayoutCalculation,
                "layout calculation is deterministic",
                "layout::deterministic-tree",
            ),
            DomainTestTemplate::new(
                SceneMutation,
                "scene mutation preserves stable node identity",
                "scene::stable-node-identity",
            ),
            DomainTestTemplate::new(
                RendererCommandGeneration,
                "renderer emits stable paint commands",
                "render::paint-commands",
            ),
            DomainTestTemplate::new(
                RuntimeScheduling,
                "runtime scheduler preserves job ordering",
                "runtime::job-ordering",
            ),
            DomainTestTemplate::new(
                ManifestHandling,
                "manifest parser preserves package targets",
                "manifest::package-targets",
            ),
            DomainTestTemplate::new(
                PluginParameterBehavior,
                "plugin parameters preserve automation semantics",
                "plugin::parameter-automation",
            ),
        ])
    }

    /// Returns all templates.
    #[must_use]
    pub fn templates(&self) -> &[DomainTestTemplate] {
        &self.templates
    }

    /// Returns a template by kind.
    #[must_use]
    pub fn template(&self, kind: DomainTemplateKind) -> Option<&DomainTestTemplate> {
        self.templates.iter().find(|template| template.kind == kind)
    }
}
