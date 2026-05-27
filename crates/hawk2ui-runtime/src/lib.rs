#![forbid(unsafe_code)]
//! Script loading, host bindings, event dispatch, scheduling, lifecycle, and runtime safety for `Hawk2UI`.

pub mod bindings;
pub mod events;
pub mod lifecycle;
pub mod safety;
pub mod scheduler;
pub mod script;
pub mod view;

pub use bindings::{
    BindingExecution, BindingLifecycleAvailability, BindingSchema, HostBindingCall,
    HostBindingError, HostBindingRecord, HostBindingRegistry,
};
pub use events::{
    RuntimeEvent, RuntimeEventDelivery, RuntimeEventDispatcher, RuntimeEventError,
    RuntimeEventKind, RuntimeEventPayload, RuntimeEventPropagation,
};
pub use lifecycle::{LifecycleHook, LifecyclePhase, LifecycleRegistry};
pub use safety::{
    RuntimeExecutionContext, RuntimeGuardDenial, RuntimeGuardOperation, RuntimeSafetyGuard,
};
pub use scheduler::{RuntimeScheduleBatch, RuntimeScheduleError, RuntimeScheduler, TimerJob};
pub use script::{
    HostCallRecord, PromiseId, RecordingScriptEngine, RuntimeCapability, RuntimeError,
    ScriptEngine, ScriptEngineError, ScriptEngineOperation, ScriptModuleKind, ScriptModuleRecord,
    StructuredValue,
};
pub use view::{
    RuntimeDrawCommand, RuntimeSceneBridge, RuntimeSceneError, RuntimeSceneFrame,
    RuntimeSceneUpdate, RuntimeTextVisual, RuntimeViewId, RuntimeViewNode, RuntimeViewTree,
    RuntimeVisual,
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
