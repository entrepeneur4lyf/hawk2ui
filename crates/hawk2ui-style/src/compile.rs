//! Style source validation and lowering into typed style records.

use crate::{PropertyId, PropertyRegistry, Selector, StyleValue, ValidationError, ValueType};
use lightningcss::{
    printer::PrinterOptions,
    properties::Property,
    rules::CssRule,
    stylesheet::{ParserOptions, StyleSheet},
    traits::ToCss,
};

const DISALLOWED_CSS_FUNCTIONS: &[&str] = &["calc(", "var(", "min(", "max(", "clamp("];

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

/// Machine-readable description of the exact production CSS subset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StyleSubsetReference;

impl StyleSubsetReference {
    /// Returns the production CSS subset reference.
    #[must_use]
    pub const fn production() -> Self {
        Self
    }

    /// Returns supported selector forms.
    #[must_use]
    pub const fn selectors(&self) -> &'static [&'static str] {
        &[
            "element",
            "class",
            "id",
            "direct-child",
            "descendant",
            ":hawk(state)",
        ]
    }

    /// Returns supported style properties.
    #[must_use]
    pub const fn properties(&self) -> &'static [&'static str] {
        &[
            "display",
            "font-size",
            "color",
            "border-width",
            "border-radius",
            "box-shadow",
            "background-gradient-start",
            "background-gradient-end",
            "glow-radius",
            "glow-color",
            "transform",
            "opacity",
            "overflow",
            "--accent-color",
            "transition-duration",
            "background-color",
            "grid-template-columns",
            "grid-template-rows",
            "grid-auto-columns",
            "grid-auto-rows",
            "grid-auto-flow",
            "grid-column-start",
            "grid-column-end",
            "grid-row-start",
            "grid-row-end",
        ]
    }

    /// Returns supported unit forms.
    #[must_use]
    pub const fn units(&self) -> &'static [&'static str] {
        &["px", "fr", "unitless-zero", "unitless-number", "ms", "s"]
    }

    /// Returns supported CSS function forms.
    #[must_use]
    pub const fn functions(&self) -> &'static [&'static str] {
        &[
            "rgb()",
            "rgba()",
            "token()",
            "translateX()",
            "translateY()",
            "translate()",
            "scale()",
            "rotate()",
        ]
    }

    /// Returns rejected syntax classes.
    #[must_use]
    pub const fn rejected_syntax(&self) -> &'static [&'static str] {
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

    if let Some(function) = first_disallowed_source_function(source) {
        diagnostics.push(StyleCompileDiagnostic::new(
            "style.function.unsupported",
            format!("style function `{function}` is not supported"),
        ));
        return Err(StyleCompileError::new(diagnostics));
    }

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
        let style_rule = match rule {
            CssRule::Style(style_rule) => style_rule,
            CssRule::Keyframes(_) => {
                diagnostics.push(StyleCompileDiagnostic::new(
                    "style.keyframes.unsupported",
                    "keyframes are not supported in the Hawk2UI CSS subset",
                ));
                continue;
            }
            CssRule::Media(_)
            | CssRule::Supports(_)
            | CssRule::Container(_)
            | CssRule::Scope(_)
            | CssRule::LayerBlock(_)
            | CssRule::LayerStatement(_) => {
                diagnostics.push(StyleCompileDiagnostic::new(
                    "style.at-rule.unsupported",
                    "conditional and grouping at-rules are not supported in Hawk2UI stylesheets",
                ));
                continue;
            }
            _ => {
                diagnostics.push(StyleCompileDiagnostic::new(
                    "style.rule.unsupported",
                    "only style rules are supported in Hawk2UI stylesheets",
                ));
                continue;
            }
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
        if is_unsupported_shorthand(property.as_str()) {
            diagnostics.push(StyleCompileDiagnostic::new(
                "style.shorthand.unsupported",
                format!(
                    "style shorthand `{}` is not supported; use explicit longhand properties",
                    property.as_str()
                ),
            ));
            continue;
        }
        let Some(metadata) = registry.metadata(&property) else {
            diagnostics.push(StyleCompileDiagnostic::new(
                "style.property.unknown",
                format!("style property `{}` is not supported", property.as_str()),
            ));
            continue;
        };
        let Some(value) = parse_value(&property, raw_value.trim(), metadata.value_type()) else {
            diagnostics.push(diagnostic_from_unsupported_value(
                raw_value.trim(),
                metadata.value_type(),
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

fn is_unsupported_shorthand(property: &str) -> bool {
    matches!(
        property,
        "margin" | "padding" | "border" | "background" | "transition" | "animation"
    )
}

fn first_disallowed_source_function(source: &str) -> Option<&'static str> {
    let source_without_comments = strip_css_comments(source);
    DISALLOWED_CSS_FUNCTIONS
        .iter()
        .find(|function| source_without_comments.contains(**function))
        .map(|function| function.trim_end_matches('('))
}

fn strip_css_comments(source: &str) -> String {
    let mut stripped = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '/' && chars.peek() == Some(&'*') {
            let _ = chars.next();
            while let Some(comment_ch) = chars.next() {
                if comment_ch == '*' && chars.peek() == Some(&'/') {
                    let _ = chars.next();
                    break;
                }
            }
        } else {
            stripped.push(ch);
        }
    }
    stripped
}

fn diagnostic_from_unsupported_value(
    raw_value: &str,
    value_type: ValueType,
) -> StyleCompileDiagnostic {
    if raw_value.contains("var(") || contains_unsupported_function(raw_value, value_type) {
        return StyleCompileDiagnostic::new(
            "style.function.unsupported",
            format!("style function in `{raw_value}` is not supported"),
        );
    }
    if has_unsupported_unit(raw_value, value_type) {
        return StyleCompileDiagnostic::new(
            "style.unit.unsupported",
            format!("style unit in `{raw_value}` is not supported"),
        );
    }
    StyleCompileDiagnostic::new(
        "style.value.unsupported",
        format!("style value `{raw_value}` is not supported"),
    )
}

fn contains_unsupported_function(raw_value: &str, value_type: ValueType) -> bool {
    if contains_disallowed_nested_function(raw_value) {
        return true;
    }
    raw_value.contains('(')
        && !matches!(value_type, ValueType::Shadow | ValueType::Transform)
        && !raw_value.starts_with("rgb(")
        && !raw_value.starts_with("rgba(")
        && !raw_value.starts_with("token(")
}

fn contains_disallowed_nested_function(raw_value: &str) -> bool {
    DISALLOWED_CSS_FUNCTIONS
        .iter()
        .any(|function| raw_value.contains(function))
}

fn has_unsupported_unit(raw_value: &str, value_type: ValueType) -> bool {
    match value_type {
        ValueType::Length => raw_value != "0" && raw_value.chars().any(char::is_alphabetic),
        ValueType::Duration => raw_value.chars().any(char::is_alphabetic),
        _ => false,
    }
}

fn parse_value(
    property: &PropertyId,
    raw_value: &str,
    value_type: ValueType,
) -> Option<StyleValue> {
    match value_type {
        ValueType::Keyword => parse_keyword(property.as_str(), raw_value).map(StyleValue::Keyword),
        ValueType::Length => parse_px(raw_value).map(StyleValue::LengthPx),
        ValueType::Number => raw_value.parse::<f32>().ok().map(StyleValue::Number),
        ValueType::Color => parse_color(raw_value),
        ValueType::Shadow => parse_shadow(raw_value).map(StyleValue::Shadow),
        ValueType::Transform => parse_transform(raw_value).map(StyleValue::Transform),
        ValueType::Duration => parse_duration(raw_value).map(StyleValue::DurationMs),
        ValueType::TokenReference => parse_token_ref(raw_value).map(StyleValue::TokenRef),
        ValueType::GridTrackList => parse_grid_track_list(raw_value).map(StyleValue::GridTrackList),
        ValueType::GridPlacement => parse_grid_placement(raw_value).map(StyleValue::GridPlacement),
        ValueType::GridAutoFlow => parse_grid_auto_flow(raw_value).map(StyleValue::GridAutoFlow),
    }
}

fn parse_keyword(property: &str, raw_value: &str) -> Option<String> {
    match property {
        "display" => matches!(raw_value, "flex" | "grid" | "none").then(|| raw_value.to_string()),
        "overflow" => matches!(raw_value, "visible" | "hidden" | "clip" | "scroll" | "auto")
            .then(|| raw_value.to_string()),
        _ => None,
    }
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

fn parse_grid_track_list(raw_value: &str) -> Option<String> {
    if raw_value == "none" {
        return Some(raw_value.to_string());
    }
    let tracks: Vec<_> = raw_value.split_whitespace().collect();
    if tracks.is_empty() || tracks.iter().any(|track| !is_grid_track(track)) {
        None
    } else {
        Some(tracks.join(" "))
    }
}

fn is_grid_track(raw_value: &str) -> bool {
    matches!(raw_value, "auto" | "min-content" | "max-content")
        || parse_non_negative_unit(raw_value, "px").is_some()
        || parse_non_negative_unit(raw_value, "fr").is_some()
}

fn parse_grid_placement(raw_value: &str) -> Option<String> {
    if raw_value == "auto" {
        return Some(raw_value.to_string());
    }
    if parse_positive_i16(raw_value).is_some() {
        return Some(raw_value.to_string());
    }
    let span = raw_value.strip_prefix("span ").map(str::trim)?;
    parse_positive_u16(span).map(|_| format!("span {span}"))
}

fn parse_grid_auto_flow(raw_value: &str) -> Option<String> {
    match raw_value {
        "row" | "column" | "row dense" | "column dense" | "dense" => Some(raw_value.to_string()),
        _ => None,
    }
}

fn parse_non_negative_unit(raw_value: &str, suffix: &str) -> Option<f32> {
    let value = raw_value.strip_suffix(suffix)?.parse::<f32>().ok()?;
    (value.is_finite() && value >= 0.0).then_some(value)
}

fn parse_positive_i16(raw_value: &str) -> Option<i16> {
    let value = raw_value.parse::<i16>().ok()?;
    (value > 0).then_some(value)
}

fn parse_positive_u16(raw_value: &str) -> Option<u16> {
    let value = raw_value.parse::<u16>().ok()?;
    (value > 0).then_some(value)
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
        3 => (
            parse_short_hex_channel(&hex[0..1])?,
            parse_short_hex_channel(&hex[1..2])?,
            parse_short_hex_channel(&hex[2..3])?,
            255,
        ),
        4 => (
            parse_short_hex_channel(&hex[0..1])?,
            parse_short_hex_channel(&hex[1..2])?,
            parse_short_hex_channel(&hex[2..3])?,
            parse_short_hex_channel(&hex[3..4])?,
        ),
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

fn parse_short_hex_channel(value: &str) -> Option<u8> {
    let doubled = format!("{value}{value}");
    parse_hex_channel(&doubled)
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

fn parse_shadow(raw_value: &str) -> Option<String> {
    if raw_value == "none" {
        return Some(raw_value.to_string());
    }
    if raw_value.contains(',') {
        return None;
    }
    let parts: Vec<_> = raw_value.split_whitespace().collect();
    match parts.as_slice() {
        [offset_x, offset_y, blur, color] => {
            parse_px(offset_x)?;
            parse_px(offset_y)?;
            parse_non_negative_px(blur)?;
            parse_color(color)?;
            Some(raw_value.to_string())
        }
        [offset_x, offset_y, blur, spread, color] => {
            parse_px(offset_x)?;
            parse_px(offset_y)?;
            parse_non_negative_px(blur)?;
            parse_non_negative_px(spread)?;
            parse_color(color)?;
            Some(raw_value.to_string())
        }
        _ => None,
    }
}

fn parse_non_negative_px(raw_value: &str) -> Option<f32> {
    let value = parse_px(raw_value)?;
    (value.is_finite() && value >= 0.0).then_some(value)
}

fn parse_transform(raw_value: &str) -> Option<String> {
    if raw_value == "none" {
        return Some(raw_value.to_string());
    }
    if let Some(argument) = function_argument(raw_value, "translateX") {
        parse_px(argument)?;
        return Some(raw_value.to_string());
    }
    if let Some(argument) = function_argument(raw_value, "translateY") {
        parse_px(argument)?;
        return Some(raw_value.to_string());
    }
    if let Some(argument) = function_argument(raw_value, "scale") {
        let scale = argument.parse::<f32>().ok()?;
        return (scale.is_finite() && scale > 0.0).then(|| raw_value.to_string());
    }
    if let Some(argument) = function_argument(raw_value, "rotate") {
        parse_degrees(argument)?;
        return Some(raw_value.to_string());
    }
    if let Some(arguments) = function_argument(raw_value, "translate") {
        let parts: Vec<_> = arguments.split(',').map(str::trim).collect();
        if parts.len() == 2 && parts.iter().all(|part| parse_px(part).is_some()) {
            return Some(raw_value.to_string());
        }
    }
    None
}

fn function_argument<'a>(raw_value: &'a str, name: &str) -> Option<&'a str> {
    raw_value
        .strip_prefix(name)?
        .strip_prefix('(')?
        .strip_suffix(')')
        .map(str::trim)
        .filter(|argument| !argument.is_empty())
}

fn parse_degrees(raw_value: &str) -> Option<f32> {
    let degrees = raw_value.strip_suffix("deg")?.parse::<f32>().ok()?;
    degrees.is_finite().then_some(degrees)
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
