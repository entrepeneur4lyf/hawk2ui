#![forbid(unsafe_code)]
//! Typed authoring records, component model, event binding, state records, and framework adapter contracts for `Hawk2UI`.

pub mod adapter;
pub mod compile;
pub mod component;
pub mod element;
pub mod events;
mod limits;
pub mod native;
mod operation_keys;
pub mod runtime_bridge;
pub mod state;

pub use adapter::{
    AdapterError, CustomRendererError, CustomRendererOperation, CustomRendererProtocol,
    FrameworkDynamicBinding, FrameworkDynamicBindingTarget, FrameworkDynamicBindingTargetWire,
    FrameworkDynamicBindingWire, FrameworkDynamicValue, FrameworkDynamicValueWire,
    FrameworkInitialDynamicValue, FrameworkInitialDynamicValueMode,
    FrameworkInitialDynamicValueModeWire, FrameworkInitialDynamicValueWire,
    FrameworkNativeAssetWire, FrameworkNativeChildWire, FrameworkNativeElementKindWire,
    FrameworkNativeEventWire, FrameworkNativeLifecycleEventWire, FrameworkNativeLifecycleWire,
    FrameworkNativeNode, FrameworkNativeNodeWire, FrameworkNativePayloadFieldWire,
    FrameworkNativeProgram, FrameworkNativeProgramWire, FrameworkNativePropValueWire,
    FrameworkNativePropWire, FrameworkReactiveBinding, FrameworkReactiveBindingKindWire,
    FrameworkReactiveBindingWire, NativeRendererAdapter, NodeOperation,
    RecordingNativeRendererAdapter,
};
pub use compile::{
    AuthoringArtifact, AuthoringDiagnostic, AuthoringDiagnosticSeverity, compile_authoring_source,
};
pub use component::{
    ComponentId, ComponentInstance, CustomSurfaceDeclaration, SurfaceId, SurfacePurpose,
};
pub use element::{
    ChildList, DuplicateChildKeyError, ElementId, ElementKind, ElementNode, KeyedChild, PropValue,
};
pub use events::{
    EventBinding, EventKind, EventPayloadField, FocusEventKind, HandlerRef, InputEventKind,
    KeyboardEventKind, LifecycleEventKind, PointerEventKind,
};
pub use native::{
    AssetRef, NativeAuthoringArtifact, NativeAuthoringElement, NativeAuthoringError,
    NativeAuthoringRuntime, NativeChild, NativeLifecycleEvent, NativeRef, StyleRef,
};
pub use runtime_bridge::{
    NativeRuntimeBridge, NativeRuntimeBridgeArtifact, NativeRuntimeBridgeError,
    NativeRuntimeNodeMetadata,
};
pub use state::{
    BatchedUpdate, StateId, StateScope, StateScopeKind, StateSubscription, StateUpdate,
    SubscriptionId, TeardownPlan, TeardownStep,
};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-authoring";

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
        assert_eq!(crate_name(), "hawk2ui-authoring");
    }
}
