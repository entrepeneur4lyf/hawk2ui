//! Accessibility action dispatch records.

use hawk2ui_api::Diagnostic;
use serde::{Deserialize, Serialize};

use crate::{A11yAction, A11yRole, A11yTree, CheckedState};

/// Maximum retained action events per dispatcher.
pub const A11Y_ACTION_EVENT_HISTORY_LIMIT: usize = 1024;

/// Accessibility action event.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct A11yActionEvent {
    /// Target node identifier.
    pub node_id: String,
    /// Action to dispatch.
    pub action: A11yAction,
}

impl A11yActionEvent {
    /// Creates a focus action event.
    #[must_use]
    pub fn focus(node_id: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            action: A11yAction::Focus,
        }
    }

    /// Creates a press action event.
    #[must_use]
    pub fn press(node_id: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            action: A11yAction::Press,
        }
    }

    /// Creates an increment action event.
    #[must_use]
    pub fn increment(node_id: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            action: A11yAction::Increment,
        }
    }

    /// Creates a decrement action event.
    #[must_use]
    pub fn decrement(node_id: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            action: A11yAction::Decrement,
        }
    }

    /// Creates a set-value action event.
    #[must_use]
    pub fn set_value(node_id: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            action: A11yAction::SetValue(value.into()),
        }
    }

    /// Creates a custom action event.
    #[must_use]
    pub fn custom(node_id: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            action: A11yAction::Custom(action.into()),
        }
    }
}

/// Accessibility action dispatch error.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct A11yActionDispatchError {
    /// Stable error code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
}

impl From<A11yActionDispatchError> for Diagnostic {
    fn from(error: A11yActionDispatchError) -> Self {
        Self::error(error.code, error.message)
    }
}

/// Accessibility action dispatcher.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct A11yActionDispatcher {
    tree: A11yTree,
    events: Vec<A11yActionEvent>,
}

impl A11yActionDispatcher {
    /// Creates an action dispatcher.
    #[must_use]
    pub fn new(tree: A11yTree) -> Self {
        Self {
            tree,
            events: Vec::new(),
        }
    }

    /// Dispatches an accessibility action event.
    ///
    /// # Errors
    ///
    /// Returns [`A11yActionDispatchError`] when the target node is missing.
    pub fn dispatch(&mut self, event: A11yActionEvent) -> Result<(), A11yActionDispatchError> {
        let Some(node) = self.tree.find(&event.node_id) else {
            return Err(missing_target_error(&event.node_id));
        };
        if node.disabled {
            return Err(A11yActionDispatchError {
                code: "a11y.action-target-disabled".into(),
                message: format!("accessibility action target is disabled: {}", event.node_id),
            });
        }
        if !action_supported(&node.actions, &event.action) {
            return Err(A11yActionDispatchError {
                code: "a11y.action-unsupported".into(),
                message: format!(
                    "accessibility action is not supported by target {}",
                    event.node_id
                ),
            });
        }
        match &event.action {
            A11yAction::Focus => {
                self.tree.clear_focus();
                let Some(node) = self.tree.find_mut(&event.node_id) else {
                    return Err(missing_target_error(&event.node_id));
                };
                node.focused = true;
            }
            A11yAction::Press => {
                let Some(node) = self.tree.find_mut(&event.node_id) else {
                    return Err(missing_target_error(&event.node_id));
                };
                if node.role == A11yRole::Checkbox {
                    node.checked = Some(match node.checked.unwrap_or(CheckedState::Unchecked) {
                        CheckedState::Checked | CheckedState::Mixed => CheckedState::Unchecked,
                        CheckedState::Unchecked => CheckedState::Checked,
                    });
                }
            }
            A11yAction::Increment => self.adjust_numeric_value(&event.node_id, 1.0)?,
            A11yAction::Decrement => self.adjust_numeric_value(&event.node_id, -1.0)?,
            A11yAction::SetValue(value) => {
                let Some(node) = self.tree.find_mut(&event.node_id) else {
                    return Err(missing_target_error(&event.node_id));
                };
                if let Some(numeric) = node.numeric_value.as_mut() {
                    let (parsed, suffix) =
                        parse_numeric_text(value).ok_or_else(|| A11yActionDispatchError {
                            code: "a11y.action-invalid-value".into(),
                            message: format!(
                                "accessibility numeric value is invalid: {}",
                                event.node_id
                            ),
                        })?;
                    let adjusted = clamp_numeric(parsed, numeric.min, numeric.max);
                    numeric.value = adjusted;
                    node.value = Some(format_numeric_value(adjusted, suffix.as_deref()));
                } else {
                    node.value = Some(value.clone());
                }
            }
            A11yAction::Custom(_) => {}
        }
        self.push_event(event);
        Ok(())
    }

    /// Returns current tree state.
    #[must_use]
    pub const fn tree(&self) -> &A11yTree {
        &self.tree
    }

    /// Returns dispatched event history.
    #[must_use]
    pub fn events(&self) -> &[A11yActionEvent] {
        &self.events
    }

    fn adjust_numeric_value(
        &mut self,
        node_id: &str,
        direction: f64,
    ) -> Result<(), A11yActionDispatchError> {
        let node = self
            .tree
            .find_mut(node_id)
            .ok_or_else(|| missing_target_error(node_id))?;
        let step = node
            .numeric_value
            .as_ref()
            .and_then(|numeric| numeric.step)
            .filter(|step| step.is_finite() && *step > 0.0)
            .unwrap_or(1.0);
        let (current, suffix) = numeric_value_and_suffix(node.value.as_deref(), node.numeric_value)
            .ok_or_else(|| A11yActionDispatchError {
                code: "a11y.action-invalid-value".into(),
                message: format!("accessibility numeric value is invalid: {node_id}"),
            })?;
        let adjusted = if let Some(numeric) = node.numeric_value.as_mut() {
            let value = clamp_numeric(current + (direction * step), numeric.min, numeric.max);
            numeric.value = value;
            value
        } else {
            current + (direction * step)
        };
        node.value = Some(format_numeric_value(adjusted, suffix.as_deref()));
        Ok(())
    }

    fn push_event(&mut self, event: A11yActionEvent) {
        while self.events.len() >= A11Y_ACTION_EVENT_HISTORY_LIMIT {
            self.events.remove(0);
        }
        self.events.push(event);
    }
}

fn missing_target_error(node_id: &str) -> A11yActionDispatchError {
    A11yActionDispatchError {
        code: "a11y.action-target-missing".into(),
        message: format!("accessibility action target is missing: {node_id}"),
    }
}

fn action_supported(supported: &[A11yAction], requested: &A11yAction) -> bool {
    supported.iter().any(|action| match (action, requested) {
        (A11yAction::SetValue(_), A11yAction::SetValue(_)) => true,
        (A11yAction::Custom(left), A11yAction::Custom(right)) => left == right,
        _ => action == requested,
    })
}

fn numeric_value_and_suffix(
    value: Option<&str>,
    numeric: Option<crate::A11yNumericValue>,
) -> Option<(f64, Option<String>)> {
    if let Some(text) = value {
        return parse_numeric_text(text);
    }
    numeric.map(|numeric| (numeric.value, None))
}

fn parse_numeric_text(text: &str) -> Option<(f64, Option<String>)> {
    let trimmed = text.trim_start();
    let leading_whitespace = text.len().saturating_sub(trimmed.len());
    let mut end = 0;
    for (index, character) in trimmed.char_indices() {
        if character.is_ascii_digit() || matches!(character, '+' | '-' | '.' | 'e' | 'E') {
            end = index + character.len_utf8();
        } else {
            break;
        }
    }
    if end == 0 {
        return None;
    }
    let number = trimmed[..end].parse::<f64>().ok()?;
    if !number.is_finite() {
        return None;
    }
    let suffix_start = leading_whitespace + end;
    let suffix = text[suffix_start..].trim_start();
    let suffix = (!suffix.is_empty()).then(|| suffix.to_owned());
    Some((number, suffix))
}

fn clamp_numeric(value: f64, min: Option<f64>, max: Option<f64>) -> f64 {
    let value = min.map_or(value, |min| value.max(min));
    max.map_or(value, |max| value.min(max))
}

fn format_numeric_value(value: f64, suffix: Option<&str>) -> String {
    let mut text = value.to_string();
    if text.ends_with(".0") {
        text.truncate(text.len() - 2);
    }
    if let Some(suffix) = suffix {
        text.push(' ');
        text.push_str(suffix);
    }
    text
}
