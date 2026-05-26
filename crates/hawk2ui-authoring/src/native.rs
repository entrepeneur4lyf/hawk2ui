//! Direct native authoring runtime for producing typed `Hawk2UI` records.

use std::collections::BTreeSet;

use crate::{
    AuthoringDiagnostic, AuthoringDiagnosticSeverity, ElementId, ElementKind, ElementNode,
    EventBinding, EventKind, HandlerRef, LifecycleEventKind, PropValue,
};

/// Stable style reference emitted by native authoring code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleRef {
    name: String,
}

impl StyleRef {
    /// Creates a style reference by registry name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    /// Returns the style reference name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Stable asset reference emitted by native authoring code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetRef {
    name: String,
    path: String,
}

impl AssetRef {
    /// Creates an asset reference with a stable name and workspace-relative path.
    #[must_use]
    pub fn new(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
        }
    }

    /// Returns the asset registry name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the workspace-relative asset path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }
}

/// Stable native reference emitted by direct authoring code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeRef {
    name: String,
}

impl NativeRef {
    /// Creates a native reference by stable author name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    /// Returns the native reference name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Lifecycle events exposed by direct native authoring.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeLifecycleEvent {
    /// Node mounted into the native tree.
    Mounted,
    /// Node removed from the native tree.
    Unmounted,
}

impl NativeLifecycleEvent {
    const fn event_kind(self) -> LifecycleEventKind {
        match self {
            Self::Mounted => LifecycleEventKind::Mounted,
            Self::Unmounted => LifecycleEventKind::Unmounted,
        }
    }

    const fn operation_key(self) -> &'static str {
        match self {
            Self::Mounted => "mounted",
            Self::Unmounted => "unmounted",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LifecycleBinding {
    event: NativeLifecycleEvent,
    handler: HandlerRef,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NativeEventBinding {
    event: EventKind,
    handler: HandlerRef,
    payload_fields: Vec<crate::EventPayloadField>,
}

/// Child element emitted by direct native authoring.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeChild {
    key: Option<String>,
    element: NativeAuthoringElement,
}

impl NativeChild {
    /// Creates an ordered child without a stable key.
    #[must_use]
    pub const fn ordered(element: NativeAuthoringElement) -> Self {
        Self { key: None, element }
    }

    /// Creates a child with a stable author-provided key.
    #[must_use]
    pub fn keyed(key: impl Into<String>, element: NativeAuthoringElement) -> Self {
        Self {
            key: Some(key.into()),
            element,
        }
    }

    /// Returns the optional stable child key.
    #[must_use]
    pub fn key(&self) -> Option<&str> {
        self.key.as_deref()
    }

    /// Returns the child element.
    #[must_use]
    pub const fn element(&self) -> &NativeAuthoringElement {
        &self.element
    }
}

/// Native element authoring node before it is finalized into typed records.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeAuthoringElement {
    node: ElementNode,
    children: Vec<NativeChild>,
    refs: Vec<NativeRef>,
    style_refs: Vec<StyleRef>,
    asset_refs: Vec<AssetRef>,
    events: Vec<NativeEventBinding>,
    lifecycle: Vec<LifecycleBinding>,
}

impl NativeAuthoringElement {
    /// Creates a native authoring element.
    #[must_use]
    pub fn new(id: impl Into<String>, kind: ElementKind) -> Self {
        Self {
            node: ElementNode::new(ElementId::new(id), kind),
            children: Vec::new(),
            refs: Vec::new(),
            style_refs: Vec::new(),
            asset_refs: Vec::new(),
            events: Vec::new(),
            lifecycle: Vec::new(),
        }
    }

    /// Adds or replaces a typed property on the element.
    #[must_use]
    pub fn with_prop(mut self, name: impl Into<String>, value: PropValue) -> Self {
        self.node = self.node.with_prop(name, value);
        self
    }

    /// Adds a child element in declaration order.
    #[must_use]
    pub fn with_child(mut self, child: NativeChild) -> Self {
        self.children.push(child);
        self
    }

    /// Adds a named native reference.
    #[must_use]
    pub fn with_ref(mut self, reference: NativeRef) -> Self {
        self.refs.push(reference);
        self
    }

    /// Adds a style registry reference.
    #[must_use]
    pub fn with_style(mut self, style: StyleRef) -> Self {
        self.style_refs.push(style);
        self
    }

    /// Adds an asset reference.
    #[must_use]
    pub fn with_asset(mut self, asset: AssetRef) -> Self {
        self.asset_refs.push(asset);
        self
    }

    /// Adds a native event binding.
    #[must_use]
    pub fn with_event(
        mut self,
        event: EventKind,
        handler: impl Into<String>,
        payload_fields: impl IntoIterator<Item = crate::EventPayloadField>,
    ) -> Self {
        self.events.push(NativeEventBinding {
            event,
            handler: HandlerRef::new(handler),
            payload_fields: payload_fields.into_iter().collect(),
        });
        self
    }

    /// Adds a lifecycle binding.
    #[must_use]
    pub fn with_lifecycle(
        mut self,
        event: NativeLifecycleEvent,
        handler: impl Into<String>,
    ) -> Self {
        self.lifecycle.push(LifecycleBinding {
            event,
            handler: HandlerRef::new(handler),
        });
        self
    }

    /// Returns the element identifier.
    #[must_use]
    pub const fn id(&self) -> &ElementId {
        self.node.id()
    }

    /// Returns the typed element node.
    #[must_use]
    pub const fn node(&self) -> &ElementNode {
        &self.node
    }

    /// Returns child elements in declaration order.
    #[must_use]
    pub fn children(&self) -> &[NativeChild] {
        &self.children
    }

    /// Returns style references in declaration order.
    #[must_use]
    pub fn style_refs(&self) -> &[StyleRef] {
        &self.style_refs
    }

    /// Returns asset references in declaration order.
    #[must_use]
    pub fn asset_refs(&self) -> &[AssetRef] {
        &self.asset_refs
    }

    /// Returns native references in declaration order.
    #[must_use]
    pub fn refs(&self) -> &[NativeRef] {
        &self.refs
    }

    /// Returns keyed child names in declaration order, skipping unkeyed children.
    #[must_use]
    pub fn keyed_child_order(&self) -> Vec<&str> {
        self.children
            .iter()
            .filter_map(|child| child.key.as_deref())
            .collect()
    }
}

/// Finalized direct native authoring artifact.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeAuthoringArtifact {
    name: String,
    root: NativeAuthoringElement,
    events: Vec<EventBinding>,
    diagnostics: Vec<AuthoringDiagnostic>,
    operation_keys: Vec<String>,
}

impl NativeAuthoringArtifact {
    /// Returns the artifact name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the root authoring element.
    #[must_use]
    pub const fn root(&self) -> &NativeAuthoringElement {
        &self.root
    }

    /// Returns finalized event bindings in deterministic order.
    #[must_use]
    pub fn events(&self) -> &[EventBinding] {
        &self.events
    }

    /// Returns non-fatal diagnostics collected during finalization.
    #[must_use]
    pub fn diagnostics(&self) -> &[AuthoringDiagnostic] {
        &self.diagnostics
    }

    /// Returns stable operation keys for conformance testing.
    #[must_use]
    pub fn operation_keys(&self) -> &[String] {
        &self.operation_keys
    }
}

/// Direct native authoring finalization error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeAuthoringError {
    diagnostics: Vec<AuthoringDiagnostic>,
}

impl NativeAuthoringError {
    /// Returns diagnostics that caused native authoring finalization to fail.
    #[must_use]
    pub fn diagnostics(&self) -> &[AuthoringDiagnostic] {
        &self.diagnostics
    }
}

/// Direct native authoring runtime that owns an authoring root until finalization.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeAuthoringRuntime {
    name: String,
    root: Option<NativeAuthoringElement>,
}

impl NativeAuthoringRuntime {
    /// Creates a native authoring runtime for one app or editor surface.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            root: None,
        }
    }

    /// Sets the root element to be finalized.
    pub fn mount(&mut self, root: NativeAuthoringElement) {
        self.root = Some(root);
    }

    /// Finalizes the mounted root into deterministic typed records.
    ///
    /// # Errors
    ///
    /// Returns [`NativeAuthoringError`] when validation finds release-blocking diagnostics.
    pub fn finish(self) -> Result<NativeAuthoringArtifact, NativeAuthoringError> {
        let Some(root) = self.root else {
            return Err(NativeAuthoringError {
                diagnostics: vec![AuthoringDiagnostic::new(
                    AuthoringDiagnosticSeverity::Error,
                    "native.root.missing",
                    "native authoring runtime requires a mounted root element",
                )],
            });
        };

        let mut diagnostics = Vec::new();
        validate_element(&root, &mut diagnostics);
        if !diagnostics.is_empty() {
            return Err(NativeAuthoringError { diagnostics });
        }

        let mut events = Vec::new();
        let mut operation_keys = Vec::new();
        collect_lifecycle(
            &root,
            NativeLifecycleEvent::Mounted,
            &mut events,
            &mut operation_keys,
        );
        collect_mounts(&root, &mut operation_keys);
        collect_events(&root, &mut events, &mut operation_keys);
        collect_lifecycle(
            &root,
            NativeLifecycleEvent::Unmounted,
            &mut events,
            &mut operation_keys,
        );
        events.sort_by_key(|event| matches!(event.event(), EventKind::Lifecycle(_)));

        Ok(NativeAuthoringArtifact {
            name: self.name,
            root,
            events,
            diagnostics,
            operation_keys,
        })
    }
}

fn validate_element(element: &NativeAuthoringElement, diagnostics: &mut Vec<AuthoringDiagnostic>) {
    let mut keys = BTreeSet::new();
    for child in &element.children {
        if let Some(key) = &child.key
            && !keys.insert(key.clone())
        {
            diagnostics.push(AuthoringDiagnostic::new(
                AuthoringDiagnosticSeverity::Error,
                "native.child-key.duplicate",
                format!("duplicate native child key `{key}`"),
            ));
        }
        validate_element(&child.element, diagnostics);
    }

    for asset in &element.asset_refs {
        if asset.path.contains("://") || asset.path.starts_with('/') || asset.path.contains("..") {
            diagnostics.push(AuthoringDiagnostic::new(
                AuthoringDiagnosticSeverity::Error,
                "native.asset.path-invalid",
                format!(
                    "asset `{}` must use a workspace-relative safe path",
                    asset.name()
                ),
            ));
        }
    }
}

fn collect_lifecycle(
    element: &NativeAuthoringElement,
    event: NativeLifecycleEvent,
    events: &mut Vec<EventBinding>,
    operation_keys: &mut Vec<String>,
) {
    for binding in element
        .lifecycle
        .iter()
        .filter(|binding| binding.event == event)
    {
        events.push(EventBinding::new(
            element.node.id().clone(),
            EventKind::Lifecycle(event.event_kind()),
            binding.handler.clone(),
        ));
        operation_keys.push(format!(
            "lifecycle:{}:{}:{}",
            event.operation_key(),
            element.node.id().as_str(),
            binding.handler.as_str()
        ));
    }
}

fn collect_mounts(element: &NativeAuthoringElement, operation_keys: &mut Vec<String>) {
    operation_keys.push(format!("mount-element:{}", element.node.id().as_str()));
    for child in &element.children {
        collect_mounts(&child.element, operation_keys);
    }
}

fn collect_events(
    element: &NativeAuthoringElement,
    events: &mut Vec<EventBinding>,
    operation_keys: &mut Vec<String>,
) {
    for binding in &element.events {
        let mut event = EventBinding::new(
            element.node.id().clone(),
            binding.event.clone(),
            binding.handler.clone(),
        );
        for field in &binding.payload_fields {
            event = event.with_payload(*field);
        }
        operation_keys.push(format!(
            "bind-event:{}:{}",
            element.node.id().as_str(),
            binding.event.stable_key()
        ));
        events.push(event);
    }
    for child in &element.children {
        collect_events(&child.element, events, operation_keys);
    }
}
