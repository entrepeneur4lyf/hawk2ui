use std::{error::Error, fmt};

/// Structured JavaScript runtime diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsRuntimeError {
    rule: String,
    message: String,
}

impl JsRuntimeError {
    /// Creates a structured JavaScript runtime diagnostic.
    #[must_use]
    pub fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            rule: rule.into(),
            message: message.into(),
        }
    }

    /// Returns the stable machine-readable diagnostic rule.
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

impl fmt::Display for JsRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.rule, self.message)
    }
}

impl Error for JsRuntimeError {}
