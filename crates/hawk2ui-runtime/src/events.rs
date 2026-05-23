//! Runtime event queue and dispatch.

use std::collections::{BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::StructuredValue;

/// Structured event payload.
pub type RuntimeEventPayload = StructuredValue;

/// Runtime event category.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum RuntimeEventKind {
    /// User-interface input or component event.
    Ui,
    /// Custom component event.
    Custom,
    /// Plugin parameter event.
    PluginParameter,
    /// Host callback event.
    HostCallback,
}

/// Event propagation policy.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeEventPropagation {
    /// Dispatch only to the target listener.
    Direct,
    /// Dispatch to the target, then the explicit ancestor path.
    Bubble,
}

/// Runtime event queued for dispatch.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeEvent {
    /// Event kind.
    pub kind: RuntimeEventKind,
    /// Target component or host endpoint.
    pub target: String,
    /// Stable event name.
    pub name: String,
    /// Structured event payload.
    pub payload: RuntimeEventPayload,
    /// Propagation policy.
    pub propagation: RuntimeEventPropagation,
    /// Ancestor path used when bubbling.
    pub bubble_path: Vec<String>,
}

impl RuntimeEvent {
    /// Creates a UI event.
    #[must_use]
    pub fn ui(target: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(
            RuntimeEventKind::Ui,
            target,
            name,
            RuntimeEventPayload::Null,
        )
    }

    /// Creates a custom component event.
    #[must_use]
    pub fn custom(
        target: impl Into<String>,
        name: impl Into<String>,
        payload: RuntimeEventPayload,
    ) -> Self {
        Self::new(RuntimeEventKind::Custom, target, name, payload)
    }

    /// Creates a plugin parameter event.
    #[must_use]
    pub fn plugin_parameter(
        target: impl Into<String>,
        parameter_id: impl Into<String>,
        normalized_value: f64,
    ) -> Self {
        Self::new(
            RuntimeEventKind::PluginParameter,
            target,
            parameter_id,
            RuntimeEventPayload::Number(normalized_value),
        )
    }

    /// Creates a host callback event.
    #[must_use]
    pub fn host_callback(target: impl Into<String>, callback_name: impl Into<String>) -> Self {
        Self::new(
            RuntimeEventKind::HostCallback,
            target,
            callback_name,
            RuntimeEventPayload::Null,
        )
    }

    /// Creates a runtime event.
    #[must_use]
    pub fn new(
        kind: RuntimeEventKind,
        target: impl Into<String>,
        name: impl Into<String>,
        payload: RuntimeEventPayload,
    ) -> Self {
        Self {
            kind,
            target: target.into(),
            name: name.into(),
            payload,
            propagation: RuntimeEventPropagation::Direct,
            bubble_path: Vec::new(),
        }
    }

    /// Sets the explicit ancestor bubble path.
    #[must_use]
    pub fn with_bubble_path(mut self, path: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.bubble_path = path.into_iter().map(Into::into).collect();
        self
    }

    /// Sets propagation behavior.
    #[must_use]
    pub const fn propagation(mut self, propagation: RuntimeEventPropagation) -> Self {
        self.propagation = propagation;
        self
    }
}

/// Event delivery to a listener target.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeEventDelivery {
    /// Listener receiving the event.
    pub listener_target: String,
    /// Delivered event.
    pub event: RuntimeEvent,
}

/// Runtime event dispatch error.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeEventError {
    /// Stable event error code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
}

impl RuntimeEventError {
    fn teardown_cancelled(cancelled_count: usize) -> Self {
        Self {
            code: "event.teardown-cancelled".into(),
            message: format!("runtime teardown cancelled {cancelled_count} pending event(s)"),
        }
    }
}

/// Deterministic event dispatcher.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct RuntimeEventDispatcher {
    listeners: BTreeSet<(String, RuntimeEventKind)>,
    queue: VecDeque<RuntimeEvent>,
    tearing_down: bool,
}

impl RuntimeEventDispatcher {
    /// Registers a listener target for an event kind.
    pub fn listen(&mut self, target: impl Into<String>, kind: RuntimeEventKind) {
        self.listeners.insert((target.into(), kind));
    }

    /// Enqueues an event for deterministic dispatch.
    pub fn enqueue(&mut self, event: RuntimeEvent) {
        self.queue.push_back(event);
    }

    /// Marks the dispatcher as tearing down. Pending events are cancelled on dispatch.
    pub const fn begin_teardown(&mut self) {
        self.tearing_down = true;
    }

    /// Returns whether no events are pending.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Dispatches all pending events.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeEventError`] when teardown cancels pending events.
    pub fn dispatch_pending(&mut self) -> Result<Vec<RuntimeEventDelivery>, RuntimeEventError> {
        if self.tearing_down && !self.queue.is_empty() {
            let cancelled_count = self.queue.len();
            self.queue.clear();
            return Err(RuntimeEventError::teardown_cancelled(cancelled_count));
        }

        let mut deliveries = Vec::new();
        while let Some(event) = self.queue.pop_front() {
            for listener_target in self.delivery_targets(&event) {
                deliveries.push(RuntimeEventDelivery {
                    listener_target,
                    event: event.clone(),
                });
            }
        }
        Ok(deliveries)
    }

    fn delivery_targets(&self, event: &RuntimeEvent) -> Vec<String> {
        let mut targets = vec![event.target.clone()];
        if event.propagation == RuntimeEventPropagation::Bubble {
            targets.extend(event.bubble_path.iter().cloned());
        }
        targets
            .into_iter()
            .filter(|target| self.listeners.contains(&(target.clone(), event.kind)))
            .collect()
    }
}
