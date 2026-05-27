//! Runtime typed style table.

use crate::{CompiledStyleSheet, PropertyId, StyleValue, TokenSet, TokenValue};

/// Runtime style diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeStyleDiagnostic {
    rule: String,
    message: String,
}

impl RuntimeStyleDiagnostic {
    /// Creates a runtime style diagnostic.
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

/// Runtime style error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeStyleError {
    diagnostic: RuntimeStyleDiagnostic,
}

impl RuntimeStyleError {
    /// Creates a runtime style error.
    #[must_use]
    pub fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            diagnostic: RuntimeStyleDiagnostic::new(rule, message),
        }
    }

    /// Returns the structured diagnostic.
    #[must_use]
    pub const fn diagnostic(&self) -> &RuntimeStyleDiagnostic {
        &self.diagnostic
    }
}

/// Runtime style table keyed by node identity and property ID.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeStyleTable {
    values: Vec<NodeStyleValues>,
}

impl RuntimeStyleTable {
    /// Creates an empty runtime style table.
    #[must_use]
    pub const fn new() -> Self {
        Self { values: Vec::new() }
    }

    /// Resolves ordered style references for a node into typed runtime values.
    ///
    /// Later style references have higher precedence when they declare the same property.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeStyleError`] when any referenced style rule is missing.
    pub fn from_style_refs(
        node_id: impl Into<String>,
        sheet: &CompiledStyleSheet,
        refs: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Self, RuntimeStyleError> {
        Self::from_style_refs_with_value_map(node_id, sheet, refs, |value| Ok(value.clone()))
    }

    /// Resolves ordered style references and token-backed declarations for a node.
    ///
    /// Token references are converted to renderer-ready typed runtime values.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeStyleError`] when any referenced style rule or token is missing.
    pub fn from_style_refs_with_tokens(
        node_id: impl Into<String>,
        sheet: &CompiledStyleSheet,
        refs: impl IntoIterator<Item = impl AsRef<str>>,
        tokens: &TokenSet,
    ) -> Result<Self, RuntimeStyleError> {
        Self::from_style_refs_with_value_map(node_id, sheet, refs, |value| {
            resolve_runtime_value(value, tokens)
        })
    }

    fn from_style_refs_with_value_map(
        node_id: impl Into<String>,
        sheet: &CompiledStyleSheet,
        refs: impl IntoIterator<Item = impl AsRef<str>>,
        mut map_value: impl FnMut(&StyleValue) -> Result<StyleValue, RuntimeStyleError>,
    ) -> Result<Self, RuntimeStyleError> {
        let node_id = node_id.into();
        let mut table = Self::new();
        for style_ref in refs {
            let style_ref = style_ref.as_ref();
            let selector_key = format!("class({style_ref})");
            let Some(rule) = sheet.rule(&selector_key) else {
                return Err(RuntimeStyleError::new(
                    "runtime-style.ref.missing",
                    format!("style reference `{style_ref}` was not found in the compiled sheet"),
                ));
            };
            for declaration in rule.declarations() {
                table.set_value(
                    node_id.clone(),
                    declaration.property().clone(),
                    map_value(declaration.value())?,
                );
            }
        }
        Ok(table)
    }

    /// Adds or replaces a typed style value.
    #[must_use]
    pub fn with_value(
        mut self,
        node_id: impl Into<String>,
        property: PropertyId,
        value: StyleValue,
    ) -> Self {
        self.set_value(node_id.into(), property, value);
        self
    }

    /// Rejects raw string values at the runtime boundary.
    ///
    /// # Errors
    ///
    /// Always returns [`RuntimeStyleError`] because runtime style values must be typed before
    /// insertion.
    pub fn try_with_raw_value(
        self,
        _node_id: impl Into<String>,
        _property: impl Into<String>,
        _value: impl Into<String>,
    ) -> Result<Self, RuntimeStyleError> {
        Err(RuntimeStyleError::new(
            "runtime-style.raw-value.rejected",
            "runtime style table accepts typed values only",
        ))
    }

    /// Returns a typed style value by node identity and property ID.
    #[must_use]
    pub fn typed_value(&self, node_id: &str, property: &PropertyId) -> Option<&StyleValue> {
        self.values
            .iter()
            .find(|node| node.node_id == node_id)
            .and_then(|node| {
                node.values
                    .iter()
                    .find(|(entry_property, _)| entry_property.as_str() == property.as_str())
                    .map(|(_, value)| value)
            })
    }

    fn set_value(&mut self, node_id: String, property: PropertyId, value: StyleValue) {
        let Some(node) = self.values.iter_mut().find(|node| node.node_id == node_id) else {
            self.values.push(NodeStyleValues {
                node_id,
                values: vec![(property, value)],
            });
            return;
        };
        if let Some((_, existing)) = node
            .values
            .iter_mut()
            .find(|(entry_property, _)| entry_property.as_str() == property.as_str())
        {
            *existing = value;
        } else {
            node.values.push((property, value));
        }
    }
}

impl Default for RuntimeStyleTable {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq)]
struct NodeStyleValues {
    node_id: String,
    values: Vec<(PropertyId, StyleValue)>,
}

fn resolve_runtime_value(
    value: &StyleValue,
    tokens: &TokenSet,
) -> Result<StyleValue, RuntimeStyleError> {
    let StyleValue::TokenRef(token_name) = value else {
        return Ok(value.clone());
    };
    let token = tokens.resolve(token_name).map_err(|error| {
        RuntimeStyleError::new(
            "runtime-style.token.missing",
            error.diagnostic().message().to_string(),
        )
    })?;
    match token.value() {
        TokenValue::ColorRgba(r, g, b, a) => Ok(StyleValue::ColorRgba(*r, *g, *b, *a)),
        TokenValue::LengthPx(value) => Ok(StyleValue::LengthPx(*value)),
        TokenValue::DurationMs(value) => Ok(StyleValue::DurationMs(*value)),
        TokenValue::Typography { size_px, .. } => Ok(StyleValue::LengthPx(*size_px)),
        TokenValue::PreferenceHook(target) => Ok(StyleValue::TokenRef(target.clone())),
    }
}
