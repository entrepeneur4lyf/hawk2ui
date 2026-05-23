//! Plugin parameter automation events.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// Automation event origin.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AutomationOrigin {
    /// Host-originated automation update.
    Host,
    /// UI-originated automation update.
    Ui,
}

/// Automation event kind.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AutomationEventKind {
    /// Begin automation gesture.
    BeginGesture,
    /// Parameter value changed.
    ValueChange,
    /// End automation gesture.
    EndGesture,
    /// Host-originated update outside a UI gesture.
    HostUpdate,
}

/// Parameter automation event.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AutomationEvent {
    /// Parameter identifier.
    pub parameter_id: String,
    /// Event kind.
    pub kind: AutomationEventKind,
    /// Event origin.
    pub origin: AutomationOrigin,
    /// Optional normalized value.
    pub normalized_value: Option<f64>,
}

impl AutomationEvent {
    /// Creates a begin gesture event.
    #[must_use]
    pub fn begin_gesture(parameter_id: impl Into<String>, origin: AutomationOrigin) -> Self {
        Self {
            parameter_id: parameter_id.into(),
            kind: AutomationEventKind::BeginGesture,
            origin,
            normalized_value: None,
        }
    }

    /// Creates a value change event.
    #[must_use]
    pub fn value_change(
        parameter_id: impl Into<String>,
        origin: AutomationOrigin,
        normalized_value: f64,
    ) -> Self {
        Self {
            parameter_id: parameter_id.into(),
            kind: AutomationEventKind::ValueChange,
            origin,
            normalized_value: Some(normalized_value.clamp(0.0, 1.0)),
        }
    }

    /// Creates an end gesture event.
    #[must_use]
    pub fn end_gesture(parameter_id: impl Into<String>, origin: AutomationOrigin) -> Self {
        Self {
            parameter_id: parameter_id.into(),
            kind: AutomationEventKind::EndGesture,
            origin,
            normalized_value: None,
        }
    }

    /// Creates a host-originated update.
    #[must_use]
    pub fn host_update(parameter_id: impl Into<String>, normalized_value: f64) -> Self {
        Self {
            parameter_id: parameter_id.into(),
            kind: AutomationEventKind::HostUpdate,
            origin: AutomationOrigin::Host,
            normalized_value: Some(normalized_value.clamp(0.0, 1.0)),
        }
    }
}

/// Automation event validation error.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AutomationEventError {
    /// Stable error code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// Related parameter identifier.
    pub parameter_id: String,
}

impl AutomationEventError {
    fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        parameter_id: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            parameter_id: parameter_id.into(),
        }
    }
}

/// Ordered automation event sequence.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct AutomationSequence {
    events: Vec<AutomationEvent>,
    open_gestures: BTreeSet<String>,
}

impl AutomationSequence {
    /// Appends an automation event, enforcing gesture ordering.
    ///
    /// # Errors
    ///
    /// Returns [`AutomationEventError`] when gestures are duplicated or closed out of order.
    pub fn push(&mut self, event: AutomationEvent) -> Result<(), AutomationEventError> {
        match event.kind {
            AutomationEventKind::BeginGesture => {
                if !self.open_gestures.insert(event.parameter_id.clone()) {
                    return Err(AutomationEventError::new(
                        "automation.duplicate-gesture",
                        "automation gesture is already open",
                        event.parameter_id,
                    ));
                }
            }
            AutomationEventKind::EndGesture => {
                if !self.open_gestures.remove(&event.parameter_id) {
                    return Err(AutomationEventError::new(
                        "automation.gesture-not-open",
                        "automation gesture is not open",
                        event.parameter_id,
                    ));
                }
            }
            AutomationEventKind::ValueChange | AutomationEventKind::HostUpdate => {}
        }
        self.events.push(event);
        Ok(())
    }

    /// Returns events in accepted order.
    #[must_use]
    pub fn events(&self) -> &[AutomationEvent] {
        &self.events
    }
}

/// Automation binding kind.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AutomationBindingKind {
    /// Generated editor binding.
    GeneratedEditor,
    /// Custom editor binding.
    CustomEditor,
}

/// Parameter binding from an editor control to a parameter.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ParameterBinding {
    /// Parameter identifier.
    pub parameter_id: String,
    /// Editor control identifier.
    pub control_id: String,
    /// Binding kind.
    pub kind: AutomationBindingKind,
}

impl ParameterBinding {
    /// Creates a generated editor binding.
    #[must_use]
    pub fn generated_editor(
        parameter_id: impl Into<String>,
        control_id: impl Into<String>,
    ) -> Self {
        Self {
            parameter_id: parameter_id.into(),
            control_id: control_id.into(),
            kind: AutomationBindingKind::GeneratedEditor,
        }
    }

    /// Creates a custom editor binding.
    #[must_use]
    pub fn custom_editor(parameter_id: impl Into<String>, control_id: impl Into<String>) -> Self {
        Self {
            parameter_id: parameter_id.into(),
            control_id: control_id.into(),
            kind: AutomationBindingKind::CustomEditor,
        }
    }
}
