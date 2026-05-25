//! Runtime API contracts.
//!
//! ## Stability
//!
//! Runtime records are source-compatible within a major crate version. Host
//! binding, lifecycle, and job records may add optional fields, but existing
//! phases, directions, and statuses are compatibility commitments.

use serde::{Deserialize, Serialize};

/// Manifest capability key required by a host binding.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
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
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimePhase {
    /// Runtime is loading modules and artifacts.
    Loading,
    /// Runtime is running application code.
    Running,
    /// Runtime is tearing down and cancelling work.
    Teardown,
}

/// Direction of a host binding call.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum BindingDirection {
    /// Runtime code calls into host-provided functionality.
    RuntimeToHost,
    /// Host code sends data or commands into the runtime.
    HostToRuntime,
}

/// Typed host binding contract exposed to scripts.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HostBindingContract {
    /// Binding name exposed to runtime code.
    pub name: String,
    /// Capability required to call the binding.
    pub required_capability: CapabilityKey,
    /// Earliest runtime phase where the binding may be called.
    pub available_from: RuntimePhase,
    /// Direction of calls crossing the runtime/host boundary.
    pub direction: BindingDirection,
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
            direction: BindingDirection::RuntimeToHost,
        }
    }

    /// Sets the binding direction.
    #[must_use]
    pub const fn with_direction(mut self, direction: BindingDirection) -> Self {
        self.direction = direction;
        self
    }
}

/// Runtime lifecycle hook registration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeLifecycleHook {
    /// Runtime phase where the hook runs.
    pub phase: RuntimePhase,
    /// Hook name.
    pub name: String,
}

impl RuntimeLifecycleHook {
    /// Creates a lifecycle hook record.
    #[must_use]
    pub fn new(phase: RuntimePhase, name: impl Into<String>) -> Self {
        Self {
            phase,
            name: name.into(),
        }
    }
}

/// Stable runtime job identifier.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct RuntimeJobId(String);

impl RuntimeJobId {
    /// Creates a runtime job identifier.
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

/// Runtime job category.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeJobKind {
    /// Load a sealed artifact.
    LoadArtifact,
    /// Run a script module.
    RunScriptModule,
    /// Invoke a host binding.
    InvokeHostBinding,
    /// Render a surface frame.
    RenderFrame,
    /// Dispose a surface and related runtime state.
    DisposeSurface,
}

/// Runtime job status.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeJobStatus {
    /// Job is queued and has not started.
    Pending,
    /// Job is actively running.
    Running,
    /// Job completed successfully.
    Completed,
    /// Job was cancelled.
    Cancelled,
    /// Job failed.
    Failed,
}

/// Runtime job record shared by host, runtime, and test infrastructure.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeJob {
    /// Stable job identifier.
    pub id: RuntimeJobId,
    /// Runtime job category.
    pub kind: RuntimeJobKind,
    /// Runtime phase associated with the job.
    pub phase: RuntimePhase,
    /// Current job status.
    pub status: RuntimeJobStatus,
    /// Capability required by the job, if any.
    pub required_capability: Option<CapabilityKey>,
}

impl RuntimeJob {
    /// Creates a pending runtime job.
    #[must_use]
    pub const fn new(id: RuntimeJobId, kind: RuntimeJobKind, phase: RuntimePhase) -> Self {
        Self {
            id,
            kind,
            phase,
            status: RuntimeJobStatus::Pending,
            required_capability: None,
        }
    }

    /// Attaches a required capability to the job.
    #[must_use]
    pub fn with_capability(mut self, capability: CapabilityKey) -> Self {
        self.required_capability = Some(capability);
        self
    }

    /// Sets the current job status.
    #[must_use]
    pub const fn with_status(mut self, status: RuntimeJobStatus) -> Self {
        self.status = status;
        self
    }
}
