#![forbid(unsafe_code)]
//! Script loading, host bindings, event dispatch, scheduling, lifecycle, and runtime safety for `Hawk2UI`.

pub mod bindings;
pub mod events;
pub mod lifecycle;
pub mod script;

pub use bindings::{
    BindingExecution, BindingLifecycleAvailability, BindingSchema, HostBindingCall,
    HostBindingError, HostBindingRecord, HostBindingRegistry,
};
pub use events::{
    RuntimeEvent, RuntimeEventDelivery, RuntimeEventDispatcher, RuntimeEventError,
    RuntimeEventKind, RuntimeEventPayload, RuntimeEventPropagation,
};
pub use lifecycle::{LifecycleHook, LifecyclePhase, LifecycleRegistry};
pub use script::{
    HostCallRecord, RuntimeCapability, RuntimeError, ScriptModuleKind, ScriptModuleRecord,
    StructuredValue,
};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-runtime";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-runtime");
    }
}
