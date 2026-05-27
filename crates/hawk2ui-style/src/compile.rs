//! Style source validation and lowering into typed style records.

use crate::{PropertyId, PropertyRegistry, Selector, StyleValue, ValidationError};

/// Style compiler diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleCompileDiagnostic {
    rule: String,
    message: String,
}

impl StyleCompileDiagnostic {
    /// Creates a style compiler diagnostic.
    #[must_use]
    pub fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            rule: rule.into(),
            message: message.into(),
        }
    }

    /// Returns the stable diagnostic rule.
    #[must_use]
    pub fn rule(&self) -> &str {
        &self.rule
    }

    /// Returns the human-readable diagnostic message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Style compiler error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleCompileError {
    diagnostics: Vec<StyleCompileDiagnostic>,
}

impl StyleCompileError {
    /// Creates a style compiler error.
    #[must_use]
    pub fn new(diagnostics: Vec<StyleCompileDiagnostic>) -> Self {
        Self { diagnostics }
    }

    /// Returns compiler diagnostics.
    #[must_use]
    pub fn diagnostics(&self) -> &[StyleCompileDiagnostic] {
        &self.diagnostics
    }
}

/// Compiled typed style declaration.
#[derive(Clone, Debug, PartialEq)]
pub struct CompiledDeclaration {
    property: PropertyId,
    value: StyleValue,
}

impl CompiledDeclaration {
    /// Creates a compiled declaration.
    #[must_use]
    pub const fn new(property: PropertyId, value: StyleValue) -> Self {
        Self { property, value }
    }

    /// Returns the declaration property.
    #[must_use]
    pub const fn property(&self) -> &PropertyId {
        &self.property
    }

    /// Returns the typed declaration value.
    #[must_use]
    pub const fn value(&self) -> &StyleValue {
        &self.value
    }
}

/// Compiled style rule.
#[derive(Clone, Debug, PartialEq)]
pub struct CompiledStyleRule {
    selector: Selector,
    declarations: Vec<CompiledDeclaration>,
}

impl CompiledStyleRule {
    /// Creates a compiled style rule.
    #[must_use]
    pub fn new(selector: Selector, declarations: Vec<CompiledDeclaration>) -> Self {
        Self {
            selector,
            declarations,
        }
    }

    /// Returns the compiled selector.
    #[must_use]
    pub const fn selector(&self) -> &Selector {
        &self.selector
    }

    /// Returns a declaration by property.
    #[must_use]
    pub fn declaration(&self, property: &PropertyId) -> Option<&CompiledDeclaration> {
        self.declarations
            .iter()
            .find(|decl| decl.property.as_str() == property.as_str())
    }

    /// Returns declarations in source order.
    #[must_use]
    pub fn declarations(&self) -> &[CompiledDeclaration] {
        &self.declarations
    }
}

/// Compiled style sheet.
#[derive(Clone, Debug, PartialEq)]
pub struct CompiledStyleSheet {
    rules: Vec<CompiledStyleRule>,
}

impl CompiledStyleSheet {
    /// Creates a compiled style sheet.
    #[must_use]
    pub fn new(rules: Vec<CompiledStyleRule>) -> Self {
        Self { rules }
    }

    /// Returns a rule by selector stable key.
    #[must_use]
    pub fn rule(&self, stable_key: &str) -> Option<&CompiledStyleRule> {
        self.rules
            .iter()
            .find(|rule| rule.selector.stable_key() == stable_key)
    }

    /// Returns all compiled rules.
    #[must_use]
    pub fn rules(&self) -> &[CompiledStyleRule] {
        &self.rules
    }
}

/// Compiles style source into typed style records.
///
/// # Errors
///
/// Returns [`StyleCompileError`] when selectors, properties, or values are unsupported.
pub fn compile_style_source(source: &str) -> Result<CompiledStyleSheet, StyleCompileError> {
    let registry = PropertyRegistry::production();
    let mut rules = Vec::new();
    let mut diagnostics = Vec::new();

    for block in source
        .split('}')
        .map(str::trim)
        .filter(|block| !block.is_empty())
    {
        let Some((selector_source, body)) = block.split_once('{') else {
            diagnostics.push(StyleCompileDiagnostic::new(
                "style.syntax.invalid",
                "style block is missing '{'",
            ));
            continue;
        };
        let selector = match Selector::parse(selector_source.trim()) {
            Ok(selector) => selector,
            Err(error) => {
                diagnostics.push(StyleCompileDiagnostic::new(
                    error.diagnostic().rule(),
                    error.diagnostic().message(),
                ));
                continue;
            }
        };
        let declarations = compile_declarations(body, &registry, &mut diagnostics);
        if !declarations.is_empty() {
            rules.push(CompiledStyleRule::new(selector, declarations));
        }
    }

    if diagnostics.is_empty() {
        Ok(CompiledStyleSheet::new(rules))
    } else {
        Err(StyleCompileError::new(diagnostics))
    }
}

fn compile_declarations(
    body: &str,
    registry: &PropertyRegistry,
    diagnostics: &mut Vec<StyleCompileDiagnostic>,
) -> Vec<CompiledDeclaration> {
    let mut declarations = Vec::new();
    for raw_decl in body
        .split(';')
        .map(str::trim)
        .filter(|decl| !decl.is_empty())
    {
        let Some((name, raw_value)) = raw_decl.split_once(':') else {
            diagnostics.push(StyleCompileDiagnostic::new(
                "style.declaration.invalid",
                "style declaration is missing ':'",
            ));
            continue;
        };
        let property = PropertyId::new(name.trim());
        if registry.metadata(&property).is_none() {
            diagnostics.push(StyleCompileDiagnostic::new(
                "style.property.unknown",
                format!("style property `{}` is not supported", property.as_str()),
            ));
            continue;
        }
        let Some(value) = parse_value(raw_value.trim()) else {
            diagnostics.push(StyleCompileDiagnostic::new(
                "style.value.unsupported",
                format!("style value `{}` is not supported", raw_value.trim()),
            ));
            continue;
        };
        if let Err(error) = registry.validate(&property, &value) {
            diagnostics.push(diagnostic_from_validation(error));
            continue;
        }
        declarations.push(CompiledDeclaration::new(property, value));
    }
    declarations
}

fn parse_value(raw_value: &str) -> Option<StyleValue> {
    if let Some(token) = raw_value
        .strip_prefix("token(")
        .and_then(|value| value.strip_suffix(')'))
    {
        Some(StyleValue::TokenRef(token.to_string()))
    } else if let Some(px) = raw_value.strip_suffix("px") {
        px.parse::<f32>().ok().map(StyleValue::LengthPx)
    } else if let Ok(number) = raw_value.parse::<f32>() {
        Some(StyleValue::Number(number))
    } else if is_keyword(raw_value) {
        Some(StyleValue::Keyword(raw_value.to_string()))
    } else {
        None
    }
}

fn is_keyword(raw_value: &str) -> bool {
    raw_value
        .chars()
        .all(|character| character.is_ascii_alphabetic() || character == '-')
}

fn diagnostic_from_validation(error: ValidationError) -> StyleCompileDiagnostic {
    match error {
        ValidationError::UnknownProperty(property) => StyleCompileDiagnostic::new(
            "style.property.unknown",
            format!("style property `{property}` is not supported"),
        ),
        ValidationError::WrongValueType { property, .. } => StyleCompileDiagnostic::new(
            "style.value.type-mismatch",
            format!("style property `{property}` received an incompatible value"),
        ),
        ValidationError::NumberOutOfRange { property } => StyleCompileDiagnostic::new(
            "style.value.range",
            format!("style property `{property}` received an out-of-range number"),
        ),
    }
}
