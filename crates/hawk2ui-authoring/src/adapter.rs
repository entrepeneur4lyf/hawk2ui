//! Native renderer adapter contract for framework integrations.

use std::collections::BTreeSet;

use crate::{
    AssetRef, ComponentInstance, CustomSurfaceDeclaration, ElementId, ElementKind, ElementNode,
    EventBinding, HandlerRef, NativeLifecycleEvent, NativeRef, PropValue, StyleRef,
};

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

/// Public operation protocol implemented by framework custom renderers.
#[derive(Clone, Debug, PartialEq)]
pub enum CustomRendererOperation {
    /// Creates a node.
    CreateNode {
        /// Stable node ID.
        id: ElementId,
        /// Native element kind.
        kind: ElementKind,
    },
    /// Updates or inserts a node property.
    SetProp {
        /// Target node ID.
        id: ElementId,
        /// Property name.
        name: String,
        /// Typed property value.
        value: PropValue,
    },
    /// Adds a style reference to a node.
    SetStyleRef {
        /// Target node ID.
        id: ElementId,
        /// Style reference.
        style_ref: StyleRef,
    },
    /// Adds an asset reference to a node.
    SetAssetRef {
        /// Target node ID.
        id: ElementId,
        /// Asset reference.
        asset_ref: AssetRef,
    },
    /// Adds a framework/native ref to a node.
    SetRef {
        /// Target node ID.
        id: ElementId,
        /// Native ref.
        reference: NativeRef,
    },
    /// Binds an event to a node.
    BindEvent {
        /// Event binding.
        binding: EventBinding,
    },
    /// Binds a lifecycle hook to a node.
    BindLifecycle {
        /// Target node ID.
        id: ElementId,
        /// Lifecycle event.
        event: NativeLifecycleEvent,
        /// Handler reference.
        handler: HandlerRef,
    },
    /// Appends a child to a parent with an optional key.
    AppendChild {
        /// Parent node ID.
        parent: ElementId,
        /// Child node ID.
        child: ElementId,
        /// Optional keyed child identity.
        key: Option<String>,
    },
    /// Registers an error boundary for a node.
    EnterErrorBoundary {
        /// Boundary node ID.
        id: ElementId,
        /// Error handler.
        handler: HandlerRef,
    },
    /// Commits a completed renderer transaction.
    Commit {
        /// Root node ID.
        root: ElementId,
    },
    /// Removes a node.
    RemoveNode {
        /// Node ID.
        id: ElementId,
    },
}

impl CustomRendererOperation {
    fn stable_key(&self) -> String {
        match self {
            Self::CreateNode { id, kind } => {
                format!("create-node:{}:{}", id.as_str(), element_kind_key(*kind))
            }
            Self::SetProp { id, name, .. } => format!("set-prop:{}:{name}", id.as_str()),
            Self::SetStyleRef { id, style_ref } => {
                format!("set-style:{}:{}", id.as_str(), style_ref.name())
            }
            Self::SetAssetRef { id, asset_ref } => {
                format!("set-asset:{}:{}", id.as_str(), asset_ref.path())
            }
            Self::SetRef { id, reference } => {
                format!("set-ref:{}:{}", id.as_str(), reference.name())
            }
            Self::BindEvent { binding } => format!(
                "bind-event:{}:{}",
                binding.target().as_str(),
                binding.event().stable_key()
            ),
            Self::BindLifecycle { id, event, handler } => format!(
                "bind-lifecycle:{}:{}:{}",
                id.as_str(),
                lifecycle_key(*event),
                handler.as_str()
            ),
            Self::AppendChild { parent, child, key } => match key {
                Some(key) => format!(
                    "append-child:{}:{}:key:{key}",
                    parent.as_str(),
                    child.as_str()
                ),
                None => format!("append-child:{}:{}", parent.as_str(), child.as_str()),
            },
            Self::EnterErrorBoundary { id, handler } => {
                format!("error-boundary:{}:{}", id.as_str(), handler.as_str())
            }
            Self::Commit { root } => format!("commit:{}", root.as_str()),
            Self::RemoveNode { id } => format!("remove-node:{}", id.as_str()),
        }
    }
}

/// Custom renderer protocol error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomRendererError {
    rule: String,
    message: String,
}

impl CustomRendererError {
    /// Creates a custom renderer protocol error.
    #[must_use]
    pub fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            rule: rule.into(),
            message: message.into(),
        }
    }

    /// Returns the stable diagnostic rule.
    #[must_use]
    pub fn rule(&self) -> &str {
        &self.rule
    }

    /// Returns the diagnostic message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Public custom renderer protocol used by framework integrations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomRendererProtocol {
    framework_label: String,
    live_nodes: BTreeSet<ElementId>,
    operation_keys: Vec<String>,
}

impl CustomRendererProtocol {
    /// Creates a protocol recorder for a framework label.
    #[must_use]
    pub fn new(framework_label: impl Into<String>) -> Self {
        Self {
            framework_label: framework_label.into(),
            live_nodes: BTreeSet::new(),
            operation_keys: Vec::new(),
        }
    }

    /// Applies one renderer operation after validating node identity relationships.
    ///
    /// # Errors
    ///
    /// Returns [`CustomRendererError`] when a node is duplicated or referenced before creation.
    pub fn apply(&mut self, operation: CustomRendererOperation) -> Result<(), CustomRendererError> {
        self.validate_operation(&operation)?;
        let operation_key = operation.stable_key();
        match operation {
            CustomRendererOperation::CreateNode { id, .. } => {
                self.live_nodes.insert(id);
            }
            CustomRendererOperation::RemoveNode { id } => {
                self.live_nodes.remove(&id);
            }
            _ => {}
        }
        self.operation_keys.push(operation_key);
        Ok(())
    }

    /// Returns the framework label.
    #[must_use]
    pub fn framework_label(&self) -> &str {
        &self.framework_label
    }

    /// Returns stable operation keys in application order.
    #[must_use]
    pub fn operation_keys(&self) -> &[String] {
        &self.operation_keys
    }

    fn validate_operation(
        &self,
        operation: &CustomRendererOperation,
    ) -> Result<(), CustomRendererError> {
        match operation {
            CustomRendererOperation::CreateNode { id, .. } if self.live_nodes.contains(id) => {
                Err(CustomRendererError::new(
                    "custom-renderer.node.duplicate",
                    format!("custom renderer node `{}` already exists", id.as_str()),
                ))
            }
            CustomRendererOperation::SetProp { id, .. }
            | CustomRendererOperation::SetStyleRef { id, .. }
            | CustomRendererOperation::SetAssetRef { id, .. }
            | CustomRendererOperation::SetRef { id, .. }
            | CustomRendererOperation::BindLifecycle { id, .. }
            | CustomRendererOperation::EnterErrorBoundary { id, .. }
            | CustomRendererOperation::Commit { root: id }
            | CustomRendererOperation::RemoveNode { id }
                if !self.live_nodes.contains(id) =>
            {
                Err(missing_node(id))
            }
            CustomRendererOperation::BindEvent { binding }
                if !self.live_nodes.contains(binding.target()) =>
            {
                Err(missing_node(binding.target()))
            }
            CustomRendererOperation::AppendChild { parent, child, .. } => {
                if !self.live_nodes.contains(parent) {
                    Err(missing_node(parent))
                } else if !self.live_nodes.contains(child) {
                    Err(missing_node(child))
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
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

fn missing_node(id: &ElementId) -> CustomRendererError {
    CustomRendererError::new(
        "custom-renderer.node.missing",
        format!("custom renderer node `{}` does not exist", id.as_str()),
    )
}

fn element_kind_key(kind: ElementKind) -> &'static str {
    match kind {
        ElementKind::View => "view",
        ElementKind::Text => "text",
        ElementKind::Button => "button",
    }
}

fn lifecycle_key(event: NativeLifecycleEvent) -> &'static str {
    match event {
        NativeLifecycleEvent::Mounted => "mounted",
        NativeLifecycleEvent::Unmounted => "unmounted",
    }
}
