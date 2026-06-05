#![forbid(unsafe_code)]
//! Runtime core for `Hawk2UI`: host-binding validation, event dispatch, scheduling, runtime safety,
//! state persistence, and the view→render bridge. Script engines and lifecycle hooks are modeled
//! here as boundaries (a recording engine and a hook registry); script execution and lifecycle
//! sequencing are supplied by the host, not performed in this crate.

pub mod bindings;
pub mod entry_tree;
pub mod events;
pub mod lifecycle;
pub mod persistence;
pub mod safety;
pub mod scene_payload;
pub mod scheduler;
pub mod script;
pub mod view;

pub use bindings::{
    BindingExecution, BindingLifecycleAvailability, BindingSchema, HostBindingCall,
    HostBindingError, HostBindingRecord, HostBindingRegistry,
};
pub use entry_tree::{EntryNode, EntryNodeKind, EntryNodeProps};
pub use events::{
    RuntimeEvent, RuntimeEventDelivery, RuntimeEventDispatcher, RuntimeEventError,
    RuntimeEventKind, RuntimeEventPayload, RuntimeEventPropagation,
};
pub use lifecycle::{LifecycleHook, LifecyclePhase, LifecycleRegistry};
pub use persistence::{
    RuntimeHostStateChunk, RuntimePersistenceStore, RuntimeStateEntry, RuntimeStateMigration,
    RuntimeStatePersistenceError, RuntimeStateScope, RuntimeStateSnapshot, RuntimeStoragePath,
};
pub use safety::{
    RuntimeExecutionContext, RuntimeGuardDenial, RuntimeGuardOperation, RuntimeSafetyGuard,
};
pub use scene_payload::{
    RuntimeScenePayload, RuntimeScenePayloadError, RuntimeScenePayloadNode,
    RuntimeScenePayloadText, RuntimeScenePayloadVisual, RuntimeSceneViewport,
};
pub use scheduler::{
    AnimationCadencePolicy, AnimationFrameScheduler, AnimationFrameTick, RuntimeScheduleBatch,
    RuntimeScheduleError, RuntimeScheduler, TimerJob,
};
pub use script::{
    HostCallRecord, PromiseId, RecordingScriptEngine, RuntimeCapability, RuntimeError,
    ScriptEngine, ScriptEngineError, ScriptEngineOperation, ScriptModuleKind, ScriptModuleRecord,
    StructuredValue,
};
pub use view::{
    RuntimeCustomSurfaceVisual, RuntimeDrawCommand, RuntimeGlowEffect, RuntimeLinearGradient,
    RuntimeSceneBridge, RuntimeSceneError, RuntimeSceneFrame, RuntimeSceneUpdate,
    RuntimeShadowEffect, RuntimeStyledBoxVisual, RuntimeTextVisual, RuntimeViewId, RuntimeViewNode,
    RuntimeViewTree, RuntimeVisual,
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
