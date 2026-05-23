//! Plugin parameter model.

use serde::{Deserialize, Serialize};

/// Typed parameter value.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum ParameterValue {
    /// Floating-point value.
    Float(f64),
    /// Boolean value.
    Bool(bool),
    /// Indexed choice value.
    Choice(u32),
}

/// Parameter numeric range.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct ParameterRange {
    /// Minimum value.
    pub min: f64,
    /// Maximum value.
    pub max: f64,
    /// Default value.
    pub default: f64,
}

impl ParameterRange {
    /// Creates a parameter range.
    #[must_use]
    pub const fn new(min: f64, max: f64, default: f64) -> Self {
        Self { min, max, default }
    }
}

/// Parameter distribution.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ParameterDistribution {
    /// Linear distribution.
    Linear,
    /// Exponential distribution.
    Exponential,
}

/// Parameter smoothing metadata.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum ParameterSmoothing {
    /// Linear smoothing over milliseconds.
    LinearMs(f64),
}

impl ParameterSmoothing {
    /// Creates linear smoothing metadata.
    #[must_use]
    pub const fn linear_ms(milliseconds: f64) -> Self {
        Self::LinearMs(milliseconds)
    }
}

/// Parameter host flags.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ParameterFlags {
    /// Host may automate the parameter.
    pub automatable: bool,
    /// Parameter is readonly.
    pub readonly: bool,
    /// Parameter should be hidden from generic editors.
    pub hidden: bool,
}

impl ParameterFlags {
    /// Automatable parameter flags.
    #[must_use]
    pub const fn automatable() -> Self {
        Self {
            automatable: true,
            readonly: false,
            hidden: false,
        }
    }
}

/// Generated parameter metadata for host adapters and generated editors.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GeneratedParameterMetadata {
    /// Stable parameter identifier.
    pub id: String,
    /// Display name.
    pub display_name: String,
    /// Unit label.
    pub unit: String,
    /// Number of steps when discrete.
    pub steps: Option<u32>,
}

/// Plugin parameter record.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ParameterRecord {
    /// Stable string identifier.
    pub id: String,
    /// Display name.
    pub display_name: String,
    /// Unit label.
    pub unit: String,
    /// Parameter range for numeric values.
    pub range: Option<ParameterRange>,
    /// Default value.
    pub default_value: ParameterValue,
    /// Distribution metadata.
    pub distribution: ParameterDistribution,
    /// Number of discrete steps.
    pub steps: Option<u32>,
    /// Smoothing metadata.
    pub smoothing: Option<ParameterSmoothing>,
    /// Host flags.
    pub flags: ParameterFlags,
    /// Optional group identifier.
    pub group_id: Option<String>,
}

impl ParameterRecord {
    /// Creates a numeric parameter.
    #[must_use]
    pub fn numeric(
        id: impl Into<String>,
        display_name: impl Into<String>,
        unit: impl Into<String>,
        range: ParameterRange,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            unit: unit.into(),
            range: Some(range),
            default_value: ParameterValue::Float(range.default),
            distribution: ParameterDistribution::Linear,
            steps: None,
            smoothing: None,
            flags: ParameterFlags::default(),
            group_id: None,
        }
    }

    /// Creates a boolean parameter.
    #[must_use]
    pub fn boolean(
        id: impl Into<String>,
        display_name: impl Into<String>,
        default_value: bool,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            unit: String::new(),
            range: None,
            default_value: ParameterValue::Bool(default_value),
            distribution: ParameterDistribution::Linear,
            steps: Some(2),
            smoothing: None,
            flags: ParameterFlags::default(),
            group_id: None,
        }
    }

    /// Sets distribution metadata.
    #[must_use]
    pub const fn distribution(mut self, distribution: ParameterDistribution) -> Self {
        self.distribution = distribution;
        self
    }

    /// Sets host flags.
    #[must_use]
    pub const fn flags(mut self, flags: ParameterFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Sets discrete step count.
    #[must_use]
    pub const fn steps(mut self, steps: u32) -> Self {
        self.steps = Some(steps);
        self
    }

    /// Sets smoothing metadata.
    #[must_use]
    pub const fn smoothing(mut self, smoothing: ParameterSmoothing) -> Self {
        self.smoothing = Some(smoothing);
        self
    }

    /// Assigns a parameter group.
    #[must_use]
    pub fn group(mut self, group_id: impl Into<String>) -> Self {
        self.group_id = Some(group_id.into());
        self
    }

    /// Converts normalized value to typed value.
    ///
    /// # Errors
    ///
    /// Returns a message when the parameter has no numeric range.
    pub fn denormalize(&self, normalized: f64) -> Result<ParameterValue, String> {
        let range = self
            .range
            .ok_or_else(|| "parameter has no numeric range".to_string())?;
        let normalized = normalized.clamp(0.0, 1.0);
        let mut value = range.min + ((range.max - range.min) * normalized);
        if let Some(steps) = self.steps {
            let max_index = f64::from(steps.saturating_sub(1));
            let index = (normalized * max_index).round();
            value = range.min + ((range.max - range.min) * (index / max_index.max(1.0)));
        }
        Ok(ParameterValue::Float(value))
    }

    /// Converts typed value to normalized value.
    ///
    /// # Errors
    ///
    /// Returns a message when the value type or range is incompatible.
    pub fn normalize(&self, value: &ParameterValue) -> Result<f64, String> {
        match (self.range, value) {
            (Some(range), ParameterValue::Float(value)) => {
                Ok(((value - range.min) / (range.max - range.min)).clamp(0.0, 1.0))
            }
            (None, ParameterValue::Bool(value)) => Ok(f64::from(u8::from(*value))),
            _ => Err("parameter value type is incompatible".into()),
        }
    }

    /// Converts typed value to host display text.
    ///
    /// # Errors
    ///
    /// Returns a message when the value type is incompatible.
    pub fn display_value(&self, value: &ParameterValue) -> Result<String, String> {
        match value {
            ParameterValue::Float(value) if self.unit.is_empty() => Ok(display_float(*value)),
            ParameterValue::Float(value) => Ok(format!("{} {}", display_float(*value), self.unit)),
            ParameterValue::Bool(value) => Ok(value.to_string()),
            ParameterValue::Choice(value) => Ok(value.to_string()),
        }
    }

    /// Returns generated parameter metadata.
    #[must_use]
    pub fn generated_metadata(&self) -> GeneratedParameterMetadata {
        GeneratedParameterMetadata {
            id: self.id.clone(),
            display_name: self.display_name.clone(),
            unit: self.unit.clone(),
            steps: self.steps,
        }
    }
}

/// Parameter group with nested children.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ParameterGroup {
    /// Group identifier.
    pub id: String,
    /// Group display name.
    pub display_name: String,
    /// Nested child groups.
    pub children: Vec<ParameterGroup>,
}

impl ParameterGroup {
    /// Creates a parameter group.
    #[must_use]
    pub fn new(id: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            children: Vec::new(),
        }
    }

    /// Adds a child group.
    #[must_use]
    pub fn child(mut self, child: ParameterGroup) -> Self {
        self.children.push(child);
        self
    }

    fn find(&self, id: &str) -> Option<&ParameterGroup> {
        if self.id == id {
            return Some(self);
        }
        self.children.iter().find_map(|child| child.find(id))
    }
}

/// Parameter validation error.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ParameterValidationError {
    /// Stable validation code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
}

impl ParameterValidationError {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

/// Parameter model.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ParameterModel {
    /// Parameters in stable order.
    pub parameters: Vec<ParameterRecord>,
    /// Top-level groups.
    pub groups: Vec<ParameterGroup>,
}

impl ParameterModel {
    /// Creates a parameter model.
    #[must_use]
    pub fn new(parameters: impl IntoIterator<Item = ParameterRecord>) -> Self {
        Self {
            parameters: parameters.into_iter().collect(),
            groups: Vec::new(),
        }
    }

    /// Adds a top-level group.
    #[must_use]
    pub fn with_group(mut self, group: ParameterGroup) -> Self {
        self.groups.push(group);
        self
    }

    /// Validates parameter IDs and group references.
    ///
    /// # Errors
    ///
    /// Returns all validation errors.
    pub fn validate(&self) -> Result<(), Vec<ParameterValidationError>> {
        let mut errors = Vec::new();
        for parameter in &self.parameters {
            if !is_stable_id(&parameter.id) {
                errors.push(ParameterValidationError::new(
                    "parameter.id-invalid",
                    format!("parameter id is invalid: {}", parameter.id),
                ));
            }
            if let Some(group_id) = &parameter.group_id
                && self.group_path(group_id).is_none()
            {
                errors.push(ParameterValidationError::new(
                    "parameter.group-missing",
                    format!("parameter group does not exist: {group_id}"),
                ));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Finds a group by nested path identifier.
    #[must_use]
    pub fn group_path(&self, id: &str) -> Option<&ParameterGroup> {
        self.groups.iter().find_map(|group| group.find(id))
    }
}

fn is_stable_id(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|ch| {
            ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '.' || ch == '_' || ch == '-'
        })
}

fn display_float(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}
