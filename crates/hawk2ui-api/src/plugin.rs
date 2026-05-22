//! Plugin API contracts.

/// Stable plugin parameter identifier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParameterId(String);

impl ParameterId {
    /// Creates a parameter identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Automation gesture emitted by a plugin editor.
#[derive(Clone, Debug, PartialEq)]
pub enum AutomationGesture {
    /// Begin parameter gesture.
    Begin(ParameterId),
    /// Change parameter value in normalized 0..1 space.
    Change {
        parameter: ParameterId,
        normalized: f32,
    },
    /// End parameter gesture.
    End(ParameterId),
}

/// Public plugin parameter contract.
#[derive(Clone, Debug, PartialEq)]
pub struct PluginParameterContract {
    /// Stable parameter identifier.
    pub id: ParameterId,
    /// Display name.
    pub name: String,
    /// Default normalized value.
    pub default_normalized: f32,
    /// Whether host automation is allowed.
    pub automatable: bool,
}

impl PluginParameterContract {
    /// Creates a plugin parameter contract.
    #[must_use]
    pub fn new(
        id: ParameterId,
        name: impl Into<String>,
        default_normalized: f32,
        automatable: bool,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            default_normalized,
            automatable,
        }
    }
}

/// Public plugin editor contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PluginEditorContract {
    /// Default logical width.
    pub default_width: u32,
    /// Default logical height.
    pub default_height: u32,
    /// Minimum logical width.
    pub min_width: u32,
    /// Minimum logical height.
    pub min_height: u32,
}

impl PluginEditorContract {
    /// Creates a plugin editor contract.
    #[must_use]
    pub const fn new(
        default_width: u32,
        default_height: u32,
        min_width: u32,
        min_height: u32,
    ) -> Self {
        Self {
            default_width,
            default_height,
            min_width,
            min_height,
        }
    }
}
