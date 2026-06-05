//! Plugin state and preset envelopes.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::parameter::{ParameterModel, ParameterValue};

/// Serializable state value.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum StateValue {
    /// Floating-point state value.
    Float(f64),
    /// Boolean state value.
    Bool(bool),
    /// Indexed-choice state value.
    Choice(u32),
    /// Integer state value.
    Int(i64),
    /// String state value.
    String(String),
}

/// UI preference state.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct UiPreferences {
    /// Optional window width.
    pub window_width: Option<f64>,
    /// Optional window height.
    pub window_height: Option<f64>,
    /// Optional theme identifier.
    pub theme: Option<String>,
}

impl UiPreferences {
    /// Creates UI preferences.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets window size.
    #[must_use]
    pub const fn window_size(mut self, width: f64, height: f64) -> Self {
        self.window_width = Some(width);
        self.window_height = Some(height);
        self
    }

    /// Sets theme.
    #[must_use]
    pub fn theme(mut self, theme: impl Into<String>) -> Self {
        self.theme = Some(theme.into());
        self
    }
}

/// Host-specific opaque state chunk.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HostStateChunk {
    /// Plugin format or host key.
    pub format: String,
    /// Opaque host chunk bytes.
    pub bytes: Vec<u8>,
}

impl HostStateChunk {
    /// Creates a host state chunk.
    #[must_use]
    pub fn new(format: impl Into<String>, bytes: Vec<u8>) -> Self {
        Self {
            format: format.into(),
            bytes,
        }
    }
}

/// Versioned plugin state envelope.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PluginStateEnvelope {
    /// State schema version.
    pub version: u32,
    /// Parameter state keyed by stable parameter id.
    pub parameter_state: BTreeMap<String, StateValue>,
    /// Non-parameter plugin state.
    pub non_parameter_state: BTreeMap<String, StateValue>,
    /// UI preferences.
    pub ui_preferences: UiPreferences,
    /// Host chunks.
    pub host_chunks: Vec<HostStateChunk>,
}

/// Parameter state validation error.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StateValidationError {
    /// Stable validation error code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
}

impl StateValidationError {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl PluginStateEnvelope {
    /// Creates a state envelope.
    #[must_use]
    pub fn new(version: u32) -> Self {
        Self {
            version,
            parameter_state: BTreeMap::new(),
            non_parameter_state: BTreeMap::new(),
            ui_preferences: UiPreferences::default(),
            host_chunks: Vec::new(),
        }
    }

    /// Adds parameter state.
    #[must_use]
    pub fn parameter(mut self, parameter_id: impl Into<String>, value: StateValue) -> Self {
        self.parameter_state.insert(parameter_id.into(), value);
        self
    }

    /// Adds non-parameter state.
    #[must_use]
    pub fn non_parameter(mut self, key: impl Into<String>, value: StateValue) -> Self {
        self.non_parameter_state.insert(key.into(), value);
        self
    }

    /// Sets UI preferences.
    #[must_use]
    pub fn ui_preferences(mut self, preferences: UiPreferences) -> Self {
        self.ui_preferences = preferences;
        self
    }

    /// Adds a host chunk.
    #[must_use]
    pub fn host_chunk(mut self, chunk: HostStateChunk) -> Self {
        self.host_chunks.push(chunk);
        self
    }

    /// Validates parameter state values against a parameter model.
    ///
    /// # Errors
    ///
    /// Returns all state validation errors when the envelope references unknown
    /// parameters, stores a value with the wrong kind, or persists a value that
    /// falls outside the current parameter's valid domain.
    pub fn validate_parameter_state(
        &self,
        model: &ParameterModel,
    ) -> Result<(), Vec<StateValidationError>> {
        let mut errors = Vec::new();
        for (parameter_id, value) in &self.parameter_state {
            let Some(parameter) = model
                .parameters
                .iter()
                .find(|parameter| parameter.id == *parameter_id)
            else {
                errors.push(StateValidationError::new(
                    "state.parameter.unknown",
                    format!("state references unknown parameter `{parameter_id}`"),
                ));
                continue;
            };

            match (&parameter.default_value, value) {
                (ParameterValue::Float(_), StateValue::Float(value)) => {
                    if !value.is_finite() {
                        errors.push(StateValidationError::new(
                            "state.parameter.non-finite",
                            format!("float state for parameter `{parameter_id}` must be finite"),
                        ));
                        continue;
                    }
                    if let Some(range) = parameter.range
                        && (value < &range.min || value > &range.max)
                    {
                        errors.push(StateValidationError::new(
                            "state.parameter.float-out-of-range",
                            format!(
                                "float state {value} for parameter `{parameter_id}` is outside {}..={}",
                                range.min, range.max
                            ),
                        ));
                    }
                }
                (ParameterValue::Int(_), StateValue::Int(value)) => {
                    if let Some(range) = parameter.range {
                        #[allow(clippy::cast_precision_loss)]
                        let value = *value as f64;
                        if value < range.min || value > range.max {
                            errors.push(StateValidationError::new(
                                "state.parameter.int-out-of-range",
                                format!(
                                    "integer state {value} for parameter `{parameter_id}` is outside {}..={}",
                                    range.min, range.max
                                ),
                            ));
                        }
                    }
                }
                (ParameterValue::Bool(_), StateValue::Bool(_)) => {}
                (ParameterValue::Choice(_), StateValue::Choice(index)) => {
                    if usize::try_from(*index)
                        .ok()
                        .is_none_or(|index| index >= parameter.variants.len())
                    {
                        errors.push(StateValidationError::new(
                            "state.parameter.choice-out-of-range",
                            format!(
                                "choice state index {index} is outside parameter `{parameter_id}` variant range"
                            ),
                        ));
                    }
                }
                _ => errors.push(StateValidationError::new(
                    "state.parameter.type-mismatch",
                    format!(
                        "state value for parameter `{parameter_id}` does not match parameter kind"
                    ),
                )),
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Applies state migrations in order.
    ///
    /// # Errors
    ///
    /// Returns [`StateMigrationError`] when a migration source version does not match the envelope version.
    pub fn migrate(
        mut self,
        migrations: impl IntoIterator<Item = StateMigration>,
    ) -> Result<Self, StateMigrationError> {
        for migration in migrations {
            if migration.from_version != self.version {
                return Err(StateMigrationError {
                    code: "state.migration-version-mismatch".into(),
                    message: format!(
                        "migration expected version {}, envelope has {}",
                        migration.from_version, self.version
                    ),
                });
            }
            match migration.kind {
                StateMigrationKind::RenameParameter { from, to } => {
                    if let Some(value) = self.parameter_state.remove(&from) {
                        self.parameter_state.insert(to, value);
                    }
                }
            }
            self.version = migration.to_version;
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum StateMigrationKind {
    RenameParameter { from: String, to: String },
}

/// State migration record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StateMigration {
    /// Source version.
    pub from_version: u32,
    /// Destination version.
    pub to_version: u32,
    kind: StateMigrationKind,
}

impl StateMigration {
    /// Creates a parameter rename migration.
    #[must_use]
    pub fn rename_parameter(
        from_version: u32,
        to_version: u32,
        from: impl Into<String>,
        to: impl Into<String>,
    ) -> Self {
        Self {
            from_version,
            to_version,
            kind: StateMigrationKind::RenameParameter {
                from: from.into(),
                to: to.into(),
            },
        }
    }
}

/// State migration error.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StateMigrationError {
    /// Stable migration error code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
}

/// Preset kind.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum PresetKind {
    /// Factory preset shipped with the plugin.
    Factory,
    /// User-authored preset.
    User,
}

/// Preset metadata.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PresetMetadata {
    /// Stable preset identifier.
    pub id: String,
    /// Preset display name.
    pub name: String,
    /// Preset author.
    pub author: String,
}

impl PresetMetadata {
    /// Creates preset metadata.
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>, author: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            author: author.into(),
        }
    }
}

/// Plugin preset envelope.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PluginPreset {
    /// Preset kind.
    pub kind: PresetKind,
    /// Preset metadata.
    pub metadata: PresetMetadata,
    /// Preset state.
    pub state: PluginStateEnvelope,
}

impl PluginPreset {
    /// Creates a factory preset.
    #[must_use]
    pub const fn factory(metadata: PresetMetadata, state: PluginStateEnvelope) -> Self {
        Self {
            kind: PresetKind::Factory,
            metadata,
            state,
        }
    }

    /// Creates a user preset.
    #[must_use]
    pub const fn user(metadata: PresetMetadata, state: PluginStateEnvelope) -> Self {
        Self {
            kind: PresetKind::User,
            metadata,
            state,
        }
    }
}
