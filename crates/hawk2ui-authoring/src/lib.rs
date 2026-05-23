#![forbid(unsafe_code)]
//! Typed authoring records, component model, event binding, state records, and framework adapter contracts for `Hawk2UI`.

pub mod compile;
pub mod component;
pub mod element;
pub mod events;
pub mod state;

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
