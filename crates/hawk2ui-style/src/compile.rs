//! Style source validation and lowering into typed style records.

use crate::{PropertyId, PropertyRegistry, Selector, StyleValue, ValidationError, ValueType};
use lightningcss::{
    printer::PrinterOptions,
    properties::Property,
    rules::CssRule,
    stylesheet::{ParserOptions, StyleSheet},
    traits::ToCss,
};

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

    let stylesheet = match StyleSheet::parse(source, ParserOptions::default()) {
        Ok(stylesheet) => stylesheet,
        Err(error) => {
            diagnostics.push(StyleCompileDiagnostic::new(
                "style.syntax.invalid",
                format!("style source is not valid CSS: {error}"),
            ));
            return Err(StyleCompileError::new(diagnostics));
        }
    };

    for rule in stylesheet.rules.0 {
        let CssRule::Style(style_rule) = rule else {
            diagnostics.push(StyleCompileDiagnostic::new(
                "style.rule.unsupported",
                "only style rules are supported in Hawk2UI stylesheets",
            ));
            continue;
        };
        let Ok(selector_source) = style_rule
            .selectors
            .to_css_string(PrinterOptions::default())
        else {
            diagnostics.push(StyleCompileDiagnostic::new(
                "style.selector.serialize-failed",
                "style selector could not be serialized for Hawk2UI validation",
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
        let declarations = compile_declarations(
            &style_rule.declarations.declarations,
            &registry,
            &mut diagnostics,
        );
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
    properties: &[Property<'_>],
    registry: &PropertyRegistry,
    diagnostics: &mut Vec<StyleCompileDiagnostic>,
) -> Vec<CompiledDeclaration> {
    let mut declarations = Vec::new();
    for property in properties {
        let Ok(raw_decl) = property.to_css_string(false, PrinterOptions::default()) else {
            diagnostics.push(StyleCompileDiagnostic::new(
                "style.declaration.serialize-failed",
                "style declaration could not be serialized for Hawk2UI validation",
            ));
            continue;
        };
        let Some((name, raw_value)) = raw_decl.split_once(':') else {
            diagnostics.push(StyleCompileDiagnostic::new(
                "style.declaration.invalid",
                "style declaration is missing ':'",
            ));
            continue;
        };
        let property = PropertyId::new(name.trim());
        let Some(metadata) = registry.metadata(&property) else {
            diagnostics.push(StyleCompileDiagnostic::new(
                "style.property.unknown",
                format!("style property `{}` is not supported", property.as_str()),
            ));
            continue;
        };
        let Some(value) = parse_value(raw_value.trim(), metadata.value_type()) else {
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

fn parse_value(raw_value: &str, value_type: ValueType) -> Option<StyleValue> {
    match value_type {
        ValueType::Keyword => {
            is_keyword(raw_value).then(|| StyleValue::Keyword(raw_value.to_string()))
        }
        ValueType::Length => parse_px(raw_value).map(StyleValue::LengthPx),
        ValueType::Number => raw_value.parse::<f32>().ok().map(StyleValue::Number),
        ValueType::Color => parse_color(raw_value),
        ValueType::Shadow => parse_expression(raw_value).map(StyleValue::Shadow),
        ValueType::Transform => parse_expression(raw_value).map(StyleValue::Transform),
        ValueType::Duration => parse_duration(raw_value).map(StyleValue::DurationMs),
        ValueType::TokenReference => parse_token_ref(raw_value).map(StyleValue::TokenRef),
    }
}

fn is_keyword(raw_value: &str) -> bool {
    raw_value
        .chars()
        .all(|character| character.is_ascii_alphabetic() || character == '-')
}

fn parse_token_ref(raw_value: &str) -> Option<String> {
    raw_value
        .strip_prefix("token(")
        .and_then(|value| value.strip_suffix(')'))
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(ToOwned::to_owned)
}

fn parse_px(raw_value: &str) -> Option<f32> {
    if raw_value == "0" {
        Some(0.0)
    } else {
        raw_value.strip_suffix("px")?.parse::<f32>().ok()
    }
}

fn parse_duration(raw_value: &str) -> Option<u32> {
    parse_ms(raw_value).or_else(|| parse_seconds(raw_value))
}

fn parse_ms(raw_value: &str) -> Option<u32> {
    raw_value.strip_suffix("ms")?.parse::<u32>().ok()
}

fn parse_seconds(raw_value: &str) -> Option<u32> {
    let seconds = raw_value.strip_suffix('s')?.parse::<f32>().ok()?;
    if seconds.is_finite() && seconds >= 0.0 {
        format!("{:.0}", seconds * 1000.0).parse::<u32>().ok()
    } else {
        None
    }
}

fn parse_color(raw_value: &str) -> Option<StyleValue> {
    parse_hex_color(raw_value).or_else(|| parse_function_color(raw_value))
}

fn parse_hex_color(raw_value: &str) -> Option<StyleValue> {
    let hex = raw_value.strip_prefix('#')?;
    let (red, green, blue, alpha) = match hex.len() {
        6 => (
            parse_hex_channel(&hex[0..2])?,
            parse_hex_channel(&hex[2..4])?,
            parse_hex_channel(&hex[4..6])?,
            255,
        ),
        8 => (
            parse_hex_channel(&hex[0..2])?,
            parse_hex_channel(&hex[2..4])?,
            parse_hex_channel(&hex[4..6])?,
            parse_hex_channel(&hex[6..8])?,
        ),
        _ => return None,
    };
    Some(StyleValue::ColorRgba(red, green, blue, alpha))
}

fn parse_hex_channel(value: &str) -> Option<u8> {
    u8::from_str_radix(value, 16).ok()
}

fn parse_function_color(raw_value: &str) -> Option<StyleValue> {
    if let Some(args) = raw_value
        .strip_prefix("rgba(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let channels: Vec<_> = args.split(',').map(str::trim).collect();
        if channels.len() != 4 {
            return None;
        }
        return Some(StyleValue::ColorRgba(
            parse_u8_channel(channels[0])?,
            parse_u8_channel(channels[1])?,
            parse_u8_channel(channels[2])?,
            parse_alpha_channel(channels[3])?,
        ));
    }
    let args = raw_value
        .strip_prefix("rgb(")
        .and_then(|value| value.strip_suffix(')'))?;
    let channels: Vec<_> = args.split(',').map(str::trim).collect();
    if channels.len() != 3 {
        return None;
    }
    Some(StyleValue::ColorRgba(
        parse_u8_channel(channels[0])?,
        parse_u8_channel(channels[1])?,
        parse_u8_channel(channels[2])?,
        255,
    ))
}

fn parse_u8_channel(value: &str) -> Option<u8> {
    value.parse::<u8>().ok()
}

fn parse_alpha_channel(value: &str) -> Option<u8> {
    if let Ok(alpha) = value.parse::<u8>() {
        return Some(alpha);
    }
    let alpha = value.parse::<f32>().ok()?;
    if (0.0..=1.0).contains(&alpha) {
        format!("{:.0}", alpha * 255.0).parse::<u8>().ok()
    } else {
        None
    }
}

fn parse_expression(raw_value: &str) -> Option<String> {
    (!raw_value.is_empty()
        && !raw_value.contains('{')
        && !raw_value.contains('}')
        && !raw_value.contains(';'))
    .then(|| raw_value.to_string())
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
