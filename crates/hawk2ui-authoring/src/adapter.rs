//! Native renderer adapter contract for framework integrations.

use crate::{ComponentInstance, CustomSurfaceDeclaration, ElementNode, EventBinding};

/// Node operation accepted by native renderer adapters.
#[derive(Clone, Debug, PartialEq)]
pub enum NodeOperation {
    /// Mount a native element node.
    MountElement(ElementNode),
    /// Mount a component instance.
    MountComponent(ComponentInstance),
    /// Declare a custom surface.
    DeclareSurface(CustomSurfaceDeclaration),
    /// Bind a native event.
    BindEvent(EventBinding),
}

impl NodeOperation {
    fn stable_key(&self) -> String {
        match self {
            Self::MountElement(node) => format!("mount-element:{}", node.id().as_str()),
            Self::MountComponent(component) => {
                format!("mount-component:{}", component.id().as_str())
            }
            Self::DeclareSurface(surface) => format!("declare-surface:{}", surface.id().as_str()),
            Self::BindEvent(binding) => format!(
                "bind-event:{}:{}",
                binding.target().as_str(),
                binding.event().stable_key()
            ),
        }
    }
}

/// Native renderer adapter error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterError {
    message: String,
}

impl AdapterError {
    /// Creates an adapter error.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Returns the adapter error message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Contract implemented by framework adapters that emit native renderer operations.
pub trait NativeRendererAdapter {
    /// Applies a typed node operation.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError`] when the adapter rejects the operation.
    fn apply(&mut self, operation: NodeOperation) -> Result<(), AdapterError>;
}

/// Recording adapter used by conformance and framework contract tests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordingNativeRendererAdapter {
    framework_label: String,
    operation_keys: Vec<String>,
}

impl RecordingNativeRendererAdapter {
    /// Creates a recording adapter for a framework label.
    #[must_use]
    pub fn new(framework_label: impl Into<String>) -> Self {
        Self {
            framework_label: framework_label.into(),
            operation_keys: Vec::new(),
        }
    }

    /// Applies a typed node operation and records its stable key.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError`] when the adapter rejects the operation.
    pub fn apply(&mut self, operation: NodeOperation) -> Result<(), AdapterError> {
        <Self as NativeRendererAdapter>::apply(self, operation)
    }

    /// Returns the framework label associated with this recording adapter.
    #[must_use]
    pub fn framework_label(&self) -> &str {
        &self.framework_label
    }

    /// Returns stable operation keys in application order.
    #[must_use]
    pub fn operation_keys(&self) -> &[String] {
        &self.operation_keys
    }
}

impl NativeRendererAdapter for RecordingNativeRendererAdapter {
    fn apply(&mut self, operation: NodeOperation) -> Result<(), AdapterError> {
        self.operation_keys.push(operation.stable_key());
        Ok(())
    }
}
