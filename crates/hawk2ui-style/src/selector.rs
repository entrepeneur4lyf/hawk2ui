//! Supported selector subset and diagnostics.

/// Structured selector diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectorDiagnostic {
    rule: String,
    message: String,
}

impl SelectorDiagnostic {
    /// Creates a selector diagnostic.
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

/// Selector parse error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectorParseError {
    diagnostic: SelectorDiagnostic,
}

impl SelectorParseError {
    /// Creates a selector parse error.
    #[must_use]
    pub fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            diagnostic: SelectorDiagnostic::new(rule, message),
        }
    }

    /// Returns the structured diagnostic.
    #[must_use]
    pub const fn diagnostic(&self) -> &SelectorDiagnostic {
        &self.diagnostic
    }
}

/// Selector part in the supported subset.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelectorPart {
    /// Element selector.
    Element(String),
    /// Class selector.
    Class(String),
    /// ID selector.
    Id(String),
    /// Direct child combinator.
    DirectChild,
    /// Descendant combinator.
    Descendant,
    /// `Hawk2UI`-owned state selector.
    State(String),
}

impl SelectorPart {
    fn stable_key(&self) -> String {
        match self {
            Self::Element(value) => format!("element({value})"),
            Self::Class(value) => format!("class({value})"),
            Self::Id(value) => format!("id({value})"),
            Self::DirectChild => ">".to_string(),
            Self::Descendant => " ".to_string(),
            Self::State(value) => format!(":state({value})"),
        }
    }
}

/// Parsed selector in the supported subset.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Selector {
    parts: Vec<SelectorPart>,
}

impl Selector {
    /// Parses a selector from the supported subset.
    ///
    /// # Errors
    ///
    /// Returns [`SelectorParseError`] when the selector uses unsupported syntax.
    pub fn parse(source: &str) -> Result<Self, SelectorParseError> {
        reject_unsupported(source)?;
        let mut parts = Vec::new();
        if source.contains(" > ") {
            let Some((parent, child)) = source.split_once(" > ") else {
                return Err(SelectorParseError::new(
                    "selector.syntax.invalid",
                    "direct child selector is invalid",
                ));
            };
            parts.push(parse_simple(parent)?);
            parts.push(SelectorPart::DirectChild);
            parts.push(parse_simple(child)?);
        } else if source.contains(' ') {
            let Some((ancestor, descendant)) = source.split_once(' ') else {
                return Err(SelectorParseError::new(
                    "selector.syntax.invalid",
                    "descendant selector is invalid",
                ));
            };
            parts.push(parse_simple(ancestor)?);
            parts.push(SelectorPart::Descendant);
            parts.push(parse_simple(descendant)?);
        } else {
            let (base, state) = split_hawk_state(source)?;
            parts.push(parse_simple(base)?);
            if let Some(state) = state {
                parts.push(SelectorPart::State(state.to_string()));
            }
        }
        Ok(Self { parts })
    }

    /// Returns stable selector key used by tests and compiled style records.
    #[must_use]
    pub fn stable_key(&self) -> String {
        self.parts
            .iter()
            .map(SelectorPart::stable_key)
            .collect::<String>()
    }

    /// Returns selector parts.
    #[must_use]
    pub fn parts(&self) -> &[SelectorPart] {
        &self.parts
    }
}

fn reject_unsupported(source: &str) -> Result<(), SelectorParseError> {
    if source.contains(',') {
        return Err(SelectorParseError::new(
            "selector.list.unsupported",
            "selector lists are not supported",
        ));
    }
    if source.contains('[') || source.contains(']') {
        return Err(SelectorParseError::new(
            "selector.attribute.unsupported",
            "attribute selectors are not supported",
        ));
    }
    if source.contains('+') || source.contains('~') {
        return Err(SelectorParseError::new(
            "selector.combinator.unsupported",
            "only direct child and descendant combinators are supported",
        ));
    }
    if let Some((_, state)) = source.split_once(':')
        && !state.starts_with("hawk(")
    {
        return Err(SelectorParseError::new(
            "selector.state.unsupported",
            "only :hawk(state) selectors are supported",
        ));
    }
    Ok(())
}

fn parse_simple(source: &str) -> Result<SelectorPart, SelectorParseError> {
    let source = source.trim();
    if let Some(class_name) = source.strip_prefix('.') {
        if is_valid_name(class_name) {
            Ok(SelectorPart::Class(class_name.to_string()))
        } else {
            Err(invalid_selector_syntax())
        }
    } else if let Some(id) = source.strip_prefix('#') {
        if is_valid_name(id) {
            Ok(SelectorPart::Id(id.to_string()))
        } else {
            Err(invalid_selector_syntax())
        }
    } else if is_valid_name(source) {
        Ok(SelectorPart::Element(source.to_string()))
    } else {
        Err(invalid_selector_syntax())
    }
}

fn split_hawk_state(source: &str) -> Result<(&str, Option<&str>), SelectorParseError> {
    let Some((base, raw_state)) = source.split_once(":hawk(") else {
        return Ok((source, None));
    };
    let Some(state) = raw_state.strip_suffix(')') else {
        return Err(SelectorParseError::new(
            "selector.state.invalid",
            "hawk state selector must close with ')'",
        ));
    };
    if !is_valid_name(state) {
        return Err(SelectorParseError::new(
            "selector.state.invalid",
            "hawk state selector must contain a supported state name",
        ));
    }
    Ok((base, Some(state)))
}

fn invalid_selector_syntax() -> SelectorParseError {
    SelectorParseError::new("selector.syntax.invalid", "selector syntax is invalid")
}

fn is_valid_name(value: &str) -> bool {
    !value.is_empty() && value.chars().all(is_name_char)
}

fn is_name_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '-'
}
