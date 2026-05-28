//! Native renderer adapter contract for framework integrations.

use std::collections::BTreeSet;

use crate::{
    AssetRef, ComponentInstance, CustomSurfaceDeclaration, ElementId, ElementKind, ElementNode,
    EventBinding, HandlerRef, NativeAuthoringArtifact, NativeAuthoringElement,
    NativeAuthoringError, NativeAuthoringRuntime, NativeChild, NativeLifecycleEvent, NativeRef,
    PropValue, StyleRef,
};

/// Typed native node emitted by a framework compiler boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameworkNativeNode {
    id: ElementId,
    kind: ElementKind,
    key: Option<String>,
    props: Vec<(String, PropValue)>,
    refs: Vec<NativeRef>,
    style_refs: Vec<StyleRef>,
    asset_refs: Vec<AssetRef>,
    events: Vec<EventBinding>,
    lifecycle: Vec<(NativeLifecycleEvent, HandlerRef)>,
    children: Vec<(Option<String>, FrameworkNativeNode)>,
}

impl FrameworkNativeNode {
    /// Creates a typed native framework node.
    #[must_use]
    pub fn new(id: impl Into<String>, kind: ElementKind) -> Self {
        Self {
            id: ElementId::new(id),
            kind,
            key: None,
            props: Vec::new(),
            refs: Vec::new(),
            style_refs: Vec::new(),
            asset_refs: Vec::new(),
            events: Vec::new(),
            lifecycle: Vec::new(),
            children: Vec::new(),
        }
    }

    /// Returns the stable node ID.
    #[must_use]
    pub const fn id(&self) -> &ElementId {
        &self.id
    }

    /// Returns the native element kind.
    #[must_use]
    pub const fn kind(&self) -> ElementKind {
        self.kind
    }

    /// Returns the optional framework key.
    #[must_use]
    pub fn key(&self) -> Option<&str> {
        self.key.as_deref()
    }

    /// Returns typed properties in compiler order.
    #[must_use]
    pub fn props(&self) -> &[(String, PropValue)] {
        &self.props
    }

    /// Returns refs in compiler order.
    #[must_use]
    pub fn refs(&self) -> &[NativeRef] {
        &self.refs
    }

    /// Returns style refs in compiler order.
    #[must_use]
    pub fn style_refs(&self) -> &[StyleRef] {
        &self.style_refs
    }

    /// Returns asset refs in compiler order.
    #[must_use]
    pub fn asset_refs(&self) -> &[AssetRef] {
        &self.asset_refs
    }

    /// Returns event bindings in compiler order.
    #[must_use]
    pub fn events(&self) -> &[EventBinding] {
        &self.events
    }

    /// Returns lifecycle handlers in compiler order.
    #[must_use]
    pub fn lifecycle(&self) -> &[(NativeLifecycleEvent, HandlerRef)] {
        &self.lifecycle
    }

    /// Returns child nodes in compiler order with their optional append keys.
    #[must_use]
    pub fn children(&self) -> &[(Option<String>, FrameworkNativeNode)] {
        &self.children
    }

    /// Sets the framework key for this node.
    #[must_use]
    pub fn with_key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// Adds a typed property.
    #[must_use]
    pub fn with_prop(mut self, name: impl Into<String>, value: PropValue) -> Self {
        self.props.push((name.into(), value));
        self
    }

    /// Adds a native ref.
    #[must_use]
    pub fn with_ref(mut self, reference: NativeRef) -> Self {
        self.refs.push(reference);
        self
    }

    /// Adds a style ref.
    #[must_use]
    pub fn with_style(mut self, style_ref: StyleRef) -> Self {
        self.style_refs.push(style_ref);
        self
    }

    /// Adds an asset ref.
    #[must_use]
    pub fn with_asset(mut self, asset_ref: AssetRef) -> Self {
        self.asset_refs.push(asset_ref);
        self
    }

    /// Adds an event binding.
    #[must_use]
    pub fn with_event(
        mut self,
        event: crate::EventKind,
        handler: HandlerRef,
        payload_fields: impl IntoIterator<Item = crate::EventPayloadField>,
    ) -> Self {
        let mut binding = EventBinding::new(self.id.clone(), event, handler);
        for field in payload_fields {
            binding = binding.with_payload(field);
        }
        self.events.push(binding);
        self
    }

    /// Adds a lifecycle handler.
    #[must_use]
    pub fn with_lifecycle(mut self, event: NativeLifecycleEvent, handler: HandlerRef) -> Self {
        self.lifecycle.push((event, handler));
        self
    }

    /// Adds a child node with a stable append key.
    #[must_use]
    pub fn with_child(mut self, key: impl Into<String>, child: FrameworkNativeNode) -> Self {
        self.children.push((Some(key.into()), child));
        self
    }

    /// Adds an unkeyed child node.
    #[must_use]
    pub fn with_unkeyed_child(mut self, child: FrameworkNativeNode) -> Self {
        self.children.push((None, child));
        self
    }
}

/// Typed native program emitted by framework compiler/runtime adapters.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameworkNativeProgram {
    root: FrameworkNativeNode,
}

impl FrameworkNativeProgram {
    /// Creates a framework native program with one root node.
    #[must_use]
    pub const fn new(root: FrameworkNativeNode) -> Self {
        Self { root }
    }

    /// Returns the root node.
    #[must_use]
    pub const fn root(&self) -> &FrameworkNativeNode {
        &self.root
    }

    /// Returns keyed direct children in compiler order.
    #[must_use]
    pub fn keyed_child_order(&self) -> Vec<String> {
        self.root
            .children()
            .iter()
            .filter_map(|(key, child)| key.clone().or_else(|| child.key.clone()))
            .collect()
    }

    /// Records this program as custom renderer protocol operation keys.
    ///
    /// # Errors
    ///
    /// Returns [`CustomRendererError`] when the program references invalid node relationships.
    pub fn custom_renderer_operation_keys(
        &self,
        framework_label: &str,
    ) -> Result<Vec<String>, CustomRendererError> {
        let mut protocol = CustomRendererProtocol::new(framework_label);
        emit_node_operations(&mut protocol, None, None, &self.root)?;
        protocol.apply(CustomRendererOperation::Commit {
            root: self.root.id.clone(),
        })?;
        Ok(protocol.operation_keys().to_vec())
    }

    /// Converts the compiler output to a finalized native authoring artifact.
    ///
    /// # Errors
    ///
    /// Returns [`NativeAuthoringError`] when native authoring validation rejects the program.
    pub fn to_native_authoring_artifact(
        &self,
        author_file: &str,
        include_default_visual_props: bool,
    ) -> Result<NativeAuthoringArtifact, NativeAuthoringError> {
        let mut runtime = NativeAuthoringRuntime::new(author_file);
        runtime.mount(framework_node_to_native_element(
            &self.root,
            include_default_visual_props,
            true,
        ));
        runtime.finish()
    }
}

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

fn emit_node_operations(
    protocol: &mut CustomRendererProtocol,
    parent: Option<&ElementId>,
    append_key: Option<&str>,
    node: &FrameworkNativeNode,
) -> Result<(), CustomRendererError> {
    protocol.apply(CustomRendererOperation::CreateNode {
        id: node.id.clone(),
        kind: node.kind,
    })?;
    for (name, value) in &node.props {
        protocol.apply(CustomRendererOperation::SetProp {
            id: node.id.clone(),
            name: name.clone(),
            value: value.clone(),
        })?;
    }
    for style_ref in &node.style_refs {
        protocol.apply(CustomRendererOperation::SetStyleRef {
            id: node.id.clone(),
            style_ref: StyleRef::new(style_ref.name()),
        })?;
    }
    for asset_ref in &node.asset_refs {
        protocol.apply(CustomRendererOperation::SetAssetRef {
            id: node.id.clone(),
            asset_ref: AssetRef::new(asset_ref.name(), asset_ref.path()),
        })?;
    }
    for reference in &node.refs {
        protocol.apply(CustomRendererOperation::SetRef {
            id: node.id.clone(),
            reference: NativeRef::new(reference.name()),
        })?;
    }
    for event in &node.events {
        protocol.apply(CustomRendererOperation::BindEvent {
            binding: event.clone(),
        })?;
    }
    for (event, handler) in &node.lifecycle {
        protocol.apply(CustomRendererOperation::BindLifecycle {
            id: node.id.clone(),
            event: *event,
            handler: HandlerRef::new(handler.as_str()),
        })?;
    }
    if let Some(parent) = parent {
        protocol.apply(CustomRendererOperation::AppendChild {
            parent: parent.clone(),
            child: node.id.clone(),
            key: append_key.map(str::to_string).or_else(|| node.key.clone()),
        })?;
    }
    for (child_key, child) in &node.children {
        emit_node_operations(protocol, Some(&node.id), child_key.as_deref(), child)?;
    }
    Ok(())
}

fn framework_node_to_native_element(
    node: &FrameworkNativeNode,
    include_default_visual_props: bool,
    is_root: bool,
) -> NativeAuthoringElement {
    let mut element = NativeAuthoringElement::new(node.id.as_str(), node.kind);
    if include_default_visual_props && is_root {
        element = element.with_prop("background", PropValue::String("#080a0e".to_string()));
    }
    for (name, value) in &node.props {
        element = element.with_prop(name, value.clone());
    }
    for reference in &node.refs {
        element = element.with_ref(NativeRef::new(reference.name()));
    }
    for style in &node.style_refs {
        element = element.with_style(StyleRef::new(style.name()));
    }
    for asset in &node.asset_refs {
        element = element.with_asset(AssetRef::new(asset.name(), asset.path()));
    }
    for event in &node.events {
        element = element.with_event(
            event.event().clone(),
            event.handler().as_str(),
            event.payload_fields().iter().copied(),
        );
    }
    for (event, handler) in &node.lifecycle {
        element = element.with_lifecycle(*event, handler.as_str());
    }
    for (key, child) in &node.children {
        let child_element = framework_node_to_native_element(child, false, false);
        element = element.with_child(match key.as_deref().or_else(|| child.key()) {
            Some(key) => NativeChild::keyed(key, child_element),
            None => NativeChild::ordered(child_element),
        });
    }
    element
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
        NativeLifecycleEvent::Suspended => "suspended",
        NativeLifecycleEvent::Resumed => "resumed",
        NativeLifecycleEvent::HotReloaded => "hot-reloaded",
        NativeLifecycleEvent::ErrorBoundary => "error-boundary",
        NativeLifecycleEvent::Shutdown => "shutdown",
        NativeLifecycleEvent::Unmounted => "unmounted",
    }
}
