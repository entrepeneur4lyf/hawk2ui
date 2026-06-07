//! Test doubles for authoring adapter conformance.

use crate::adapter::{AdapterError, NativeRendererAdapter, NodeOperation};

/// Recording adapter used by conformance and framework contract tests.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordingNativeRendererAdapter {
    framework_label: String,
    operations: Vec<NodeOperation>,
    operation_keys: Vec<String>,
}

impl RecordingNativeRendererAdapter {
    /// Creates a recording adapter for a framework label.
    #[must_use]
    pub fn new(framework_label: impl Into<String>) -> Self {
        Self {
            framework_label: framework_label.into(),
            operations: Vec::new(),
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

    /// Returns typed operations in application order.
    #[must_use]
    pub fn operations(&self) -> &[NodeOperation] {
        &self.operations
    }
}

impl NativeRendererAdapter for RecordingNativeRendererAdapter {
    fn apply(&mut self, operation: NodeOperation) -> Result<(), AdapterError> {
        self.operations.push(operation.clone());
        self.operation_keys.push(operation.stable_key());
        Ok(())
    }
}
