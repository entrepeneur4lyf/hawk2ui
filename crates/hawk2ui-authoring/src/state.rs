//! State, subscription, batched update, and teardown records.

use crate::{ElementId, HandlerRef, PropValue};

/// Stable state identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StateId(String);

impl StateId {
    /// Creates a state identifier.
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

/// State scope with any scope-specific binding metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StateScope {
    /// Application-level state.
    App,
    /// Component-local state.
    Component(ElementId),
    /// User interface preference state.
    UiPreference(String),
    /// Plugin parameter or host state binding.
    PluginBinding(String),
}

impl StateScope {
    /// Returns the coarse state scope kind.
    #[must_use]
    pub const fn kind(&self) -> StateScopeKind {
        match self {
            Self::App => StateScopeKind::App,
            Self::Component(_) => StateScopeKind::Component,
            Self::UiPreference(_) => StateScopeKind::UiPreference,
            Self::PluginBinding(_) => StateScopeKind::PluginBinding,
        }
    }
}

/// Coarse state scope kind used for grouping.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StateScopeKind {
    /// Application-level state.
    App,
    /// Component-local state.
    Component,
    /// User interface preference state.
    UiPreference,
    /// Plugin parameter or host state binding.
    PluginBinding,
}

/// State update record.
#[derive(Clone, Debug, PartialEq)]
pub struct StateUpdate {
    id: StateId,
    scope: StateScope,
    value: PropValue,
}

impl StateUpdate {
    /// Creates a state update.
    #[must_use]
    pub const fn new(id: StateId, scope: StateScope, value: PropValue) -> Self {
        Self { id, scope, value }
    }

    /// Returns the state identifier.
    #[must_use]
    pub const fn id(&self) -> &StateId {
        &self.id
    }

    /// Returns the state scope.
    #[must_use]
    pub const fn scope(&self) -> &StateScope {
        &self.scope
    }

    /// Returns the state value.
    #[must_use]
    pub const fn value(&self) -> &PropValue {
        &self.value
    }
}

/// Batched state update record.
#[derive(Clone, Debug, PartialEq)]
pub struct BatchedUpdate {
    name: String,
    updates: Vec<StateUpdate>,
}

impl BatchedUpdate {
    /// Creates a batched update.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            updates: Vec::new(),
        }
    }

    /// Adds an update to the batch in author-declared order.
    #[must_use]
    pub fn with_update(mut self, update: StateUpdate) -> Self {
        self.updates.push(update);
        self
    }

    /// Returns the batch name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns all updates for a coarse scope kind.
    #[must_use]
    pub fn updates_for_scope(&self, scope: StateScopeKind) -> Vec<&StateUpdate> {
        self.updates
            .iter()
            .filter(|update| update.scope.kind() == scope)
            .collect()
    }
}

/// Stable subscription identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SubscriptionId(String);

impl SubscriptionId {
    /// Creates a subscription identifier.
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

/// State subscription record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateSubscription {
    id: SubscriptionId,
    state: StateId,
    handler: HandlerRef,
}

impl StateSubscription {
    /// Creates a state subscription.
    #[must_use]
    pub const fn new(id: SubscriptionId, state: StateId, handler: HandlerRef) -> Self {
        Self { id, state, handler }
    }

    /// Returns the subscription identifier.
    #[must_use]
    pub const fn id(&self) -> &SubscriptionId {
        &self.id
    }

    /// Returns the subscribed state identifier.
    #[must_use]
    pub const fn state(&self) -> &StateId {
        &self.state
    }

    /// Returns the subscription handler reference.
    #[must_use]
    pub const fn handler(&self) -> &HandlerRef {
        &self.handler
    }
}

/// Teardown step record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TeardownStep {
    /// Release a state subscription.
    ReleaseSubscription(SubscriptionId),
    /// Detach a plugin binding.
    DetachPluginBinding(String),
    /// Clear component-local state.
    ClearComponentState(ElementId),
    /// Clear user interface preference state.
    ClearUiPreference(String),
}

impl TeardownStep {
    fn stable_key(&self) -> String {
        match self {
            Self::ReleaseSubscription(id) => format!("release-subscription:{}", id.as_str()),
            Self::DetachPluginBinding(parameter) => format!("detach-plugin-binding:{parameter}"),
            Self::ClearComponentState(id) => format!("clear-component-state:{}", id.as_str()),
            Self::ClearUiPreference(name) => format!("clear-ui-preference:{name}"),
        }
    }
}

/// Deterministic teardown plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TeardownPlan {
    steps: Vec<TeardownStep>,
}

impl TeardownPlan {
    /// Creates an empty teardown plan.
    #[must_use]
    pub const fn new() -> Self {
        Self { steps: Vec::new() }
    }

    /// Adds a teardown step in deterministic execution order.
    #[must_use]
    pub fn with_step(mut self, step: TeardownStep) -> Self {
        self.steps.push(step);
        self
    }

    /// Returns deterministic step keys in execution order.
    #[must_use]
    pub fn step_keys(&self) -> Vec<String> {
        self.steps.iter().map(TeardownStep::stable_key).collect()
    }
}

impl Default for TeardownPlan {
    fn default() -> Self {
        Self::new()
    }
}
