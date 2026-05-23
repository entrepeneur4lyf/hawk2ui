//! Runtime typed style table.

use crate::{PropertyId, StyleValue};

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
