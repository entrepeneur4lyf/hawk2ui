//! Accessibility action dispatch records.

use hawk2ui_api::Diagnostic;
use serde::{Deserialize, Serialize};

use crate::{A11yAction, A11yTree};

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
        let Some(node) = self.tree.find_mut(&event.node_id) else {
            return Err(A11yActionDispatchError {
                code: "a11y.action-target-missing".into(),
                message: format!("accessibility action target is missing: {}", event.node_id),
            });
        };
        match &event.action {
            A11yAction::Focus => node.focused = true,
            A11yAction::SetValue(value) => node.value = Some(value.clone()),
            A11yAction::Press
            | A11yAction::Increment
            | A11yAction::Decrement
            | A11yAction::Custom(_) => {}
        }
        self.events.push(event);
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
}
