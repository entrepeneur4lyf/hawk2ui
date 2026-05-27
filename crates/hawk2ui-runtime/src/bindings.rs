//! Capability-scoped host binding registry.

use std::collections::BTreeMap;

use hawk2ui_api::{Diagnostic, RelatedContext};
use serde::{Deserialize, Serialize};

use crate::{LifecyclePhase, RuntimeCapability, StructuredValue};

/// Host binding execution behavior.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum BindingExecution {
    /// Completes before returning to script.
    Synchronous,
    /// Completes asynchronously through the script engine promise boundary.
    Asynchronous,
}

/// Lifecycle availability for a binding.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum BindingLifecycleAvailability {
    /// Available during every lifecycle phase.
    Always,
    /// Available after initialization.
    AfterMount,
    /// Available only while the UI is mounted.
    MountedOnly,
}

impl BindingLifecycleAvailability {
    fn allows(self, phase: LifecyclePhase) -> bool {
        match self {
            Self::Always => true,
            Self::AfterMount => phase != LifecyclePhase::Initialize,
            Self::MountedOnly => matches!(phase, LifecyclePhase::Mount | LifecyclePhase::Update),
        }
    }
}

/// Host binding schema labels.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BindingSchema {
    /// Input schema label.
    pub input: String,
    /// Output schema label.
    pub output: String,
    /// Error schema label.
    pub error: String,
}

impl BindingSchema {
    /// Creates host binding schema labels.
    #[must_use]
    pub fn new(
        input: impl Into<String>,
        output: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        Self {
            input: input.into(),
            output: output.into(),
            error: error.into(),
        }
    }
}

/// Registered host binding declaration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HostBindingRecord {
    /// Stable host binding name.
    pub name: String,
    /// Input/output/error schema labels.
    pub schema: BindingSchema,
    /// Capability required to call this binding.
    pub required_capability: Option<RuntimeCapability>,
    /// Sync or async execution behavior.
    pub execution: BindingExecution,
    /// Lifecycle availability.
    pub lifecycle_availability: BindingLifecycleAvailability,
}

impl HostBindingRecord {
    /// Creates a host binding record.
    #[must_use]
    pub fn new(name: impl Into<String>, schema: BindingSchema) -> Self {
        Self {
            name: name.into(),
            schema,
            required_capability: None,
            execution: BindingExecution::Asynchronous,
            lifecycle_availability: BindingLifecycleAvailability::Always,
        }
    }

    /// Sets the required capability.
    #[must_use]
    pub const fn requires(mut self, capability: RuntimeCapability) -> Self {
        self.required_capability = Some(capability);
        self
    }

    /// Sets the execution behavior.
    #[must_use]
    pub const fn execution(mut self, execution: BindingExecution) -> Self {
        self.execution = execution;
        self
    }

    /// Sets lifecycle availability.
    #[must_use]
    pub const fn available_during(mut self, availability: BindingLifecycleAvailability) -> Self {
        self.lifecycle_availability = availability;
        self
    }
}

/// Accepted host binding call.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HostBindingCall {
    /// Host binding name.
    pub binding_name: String,
    /// Structured call payload.
    pub payload: StructuredValue,
    /// Capability required by the binding.
    pub required_capability: Option<RuntimeCapability>,
    /// Output schema label for the result path.
    pub output_schema: String,
    /// Binding execution behavior.
    pub execution: BindingExecution,
}

/// Host binding validation error.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HostBindingError {
    /// Stable binding error code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// Related binding name.
    pub binding_name: String,
    /// Related capability.
    pub capability: Option<RuntimeCapability>,
}

impl HostBindingError {
    fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        binding_name: impl Into<String>,
        capability: Option<RuntimeCapability>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            binding_name: binding_name.into(),
            capability,
        }
    }
}

impl From<HostBindingError> for Diagnostic {
    fn from(error: HostBindingError) -> Self {
        let mut diagnostic = Self::error(error.code, error.message)
            .with_related(RelatedContext::new("binding", error.binding_name));
        if let Some(capability) = error.capability {
            diagnostic = diagnostic
                .with_related(RelatedContext::new("capability", format!("{capability:?}")));
        }
        diagnostic
    }
}

/// Host binding registry.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct HostBindingRegistry {
    bindings: BTreeMap<String, HostBindingRecord>,
}

impl HostBindingRegistry {
    /// Creates a registry from binding records.
    #[must_use]
    pub fn new(bindings: impl IntoIterator<Item = HostBindingRecord>) -> Self {
        let mut records = BTreeMap::new();
        for binding in bindings {
            records.entry(binding.name.clone()).or_insert(binding);
        }
        Self { bindings: records }
    }

    /// Validates and records a host binding call.
    ///
    /// # Errors
    ///
    /// Returns [`HostBindingError`] when the binding is unknown, unavailable during the lifecycle phase,
    /// denied by capability, or receives a payload that does not match its schema label.
    pub fn call(
        &self,
        binding_name: &str,
        payload: StructuredValue,
        capabilities: impl IntoIterator<Item = RuntimeCapability>,
        phase: LifecyclePhase,
    ) -> Result<HostBindingCall, HostBindingError> {
        let Some(binding) = self.bindings.get(binding_name) else {
            return Err(HostBindingError::new(
                "binding.unknown",
                format!("host binding is not registered: {binding_name}"),
                binding_name,
                None,
            ));
        };

        if !binding.lifecycle_availability.allows(phase) {
            return Err(HostBindingError::new(
                "binding.lifecycle-unavailable",
                format!("host binding is unavailable during {phase:?}: {binding_name}"),
                binding_name,
                binding.required_capability,
            ));
        }

        let available_capabilities: Vec<_> = capabilities.into_iter().collect();
        if let Some(required) = binding.required_capability
            && !available_capabilities.contains(&required)
        {
            return Err(HostBindingError::new(
                "binding.capability-denied",
                format!("host binding requires undeclared capability {required:?}: {binding_name}"),
                binding_name,
                Some(required),
            ));
        }

        if !schema_matches(&binding.schema.input, &payload) {
            return Err(HostBindingError::new(
                "binding.schema-mismatch",
                format!(
                    "host binding payload schema mismatch for {binding_name}: expected {}, received {}",
                    binding.schema.input,
                    payload.schema_label()
                ),
                binding_name,
                binding.required_capability,
            ));
        }

        Ok(HostBindingCall {
            binding_name: binding.name.clone(),
            payload,
            required_capability: binding.required_capability,
            output_schema: binding.schema.output.clone(),
            execution: binding.execution,
        })
    }
}

fn schema_matches(schema: &str, value: &StructuredValue) -> bool {
    schema == "any" || schema == value.schema_label()
}
