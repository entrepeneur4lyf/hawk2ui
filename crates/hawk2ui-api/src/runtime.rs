//! Runtime API contracts.

/// Manifest capability key required by a host binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityKey(String);

impl CapabilityKey {
    /// Creates a capability key.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the key as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Runtime lifecycle phase when a binding is available.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimePhase {
    /// Runtime is loading modules and artifacts.
    Loading,
    /// Runtime is running application code.
    Running,
    /// Runtime is tearing down and cancelling work.
    Teardown,
}

/// Typed host binding contract exposed to scripts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostBindingContract {
    /// Binding name exposed to runtime code.
    pub name: String,
    /// Capability required to call the binding.
    pub required_capability: CapabilityKey,
    /// Earliest runtime phase where the binding may be called.
    pub available_from: RuntimePhase,
}

impl HostBindingContract {
    /// Creates a host binding contract.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        required_capability: CapabilityKey,
        available_from: RuntimePhase,
    ) -> Self {
        Self {
            name: name.into(),
            required_capability,
            available_from,
        }
    }
}
