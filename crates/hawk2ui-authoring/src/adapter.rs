//! Native renderer adapter contract for framework integrations.

use std::collections::{BTreeMap, BTreeSet};

use hawk2ui_api::Diagnostic;
use serde::{Deserialize, Serialize};

use crate::{
    AssetRef, ComponentInstance, CustomSurfaceDeclaration, ElementId, ElementKind, ElementNode,
    EventBinding, EventKind, EventPayloadField, HandlerRef, NativeAuthoringArtifact,
    NativeAuthoringElement, NativeAuthoringError, NativeAuthoringRuntime, NativeChild,
    NativeLifecycleEvent, NativeRef, PropValue, StyleRef,
};
use crate::{limits::MAX_AUTHORING_TREE_DEPTH, operation_keys};

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
    compiler: FrameworkCompilerMetadata,
    root: FrameworkNativeNode,
    reactivity: Vec<FrameworkReactiveBinding>,
    dynamic_bindings: Vec<FrameworkDynamicBinding>,
    initial_dynamic_values: Vec<FrameworkInitialDynamicValue>,
    event_handlers: Vec<FrameworkEventHandler>,
    list_templates: Vec<FrameworkListTemplate>,
}

/// Compiler provenance attached to a framework native program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameworkCompilerMetadata {
    framework: String,
    compiler: String,
    source_path: String,
    entrypoint: String,
}

/// Versioned native compiler artifact emitted by framework-specific compilers.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FrameworkNativeProgramWire {
    /// Wire schema version. Version `1` is the current compiler artifact format.
    pub schema_version: u32,
    /// Compiler provenance emitted by the framework adapter.
    #[serde(default)]
    pub compiler: FrameworkCompilerMetadataWire,
    /// Root node emitted by the framework compiler.
    pub root: FrameworkNativeNodeWire,
    /// Framework reactivity bindings emitted by the compiler.
    #[serde(default)]
    pub reactivity: Vec<FrameworkReactiveBindingWire>,
    /// Runtime-evaluated property bindings emitted by the compiler.
    #[serde(default)]
    pub dynamic_bindings: Vec<FrameworkDynamicBindingWire>,
    /// Initial dependency values available for first-frame dynamic binding evaluation.
    #[serde(default)]
    pub initial_dynamic_values: Vec<FrameworkInitialDynamicValueWire>,
    /// Executable event handlers emitted by the framework compiler.
    #[serde(default)]
    pub event_handlers: Vec<FrameworkEventHandlerWire>,
    /// Runtime keyed-list templates emitted by the compiler.
    #[serde(default)]
    pub list_templates: Vec<FrameworkListTemplateWire>,
}

/// Wire compiler provenance for source-fidelity checks.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FrameworkCompilerMetadataWire {
    /// Framework label that produced the native program.
    pub framework: String,
    /// Compiler package or adapter name that produced the native program.
    pub compiler: String,
    /// Workspace-relative author source path.
    pub source_path: String,
    /// Framework entrypoint lowered by the compiler.
    pub entrypoint: String,
}

impl Default for FrameworkCompilerMetadataWire {
    fn default() -> Self {
        Self {
            framework: "unknown".to_string(),
            compiler: "unknown".to_string(),
            source_path: "unknown".to_string(),
            entrypoint: "unknown".to_string(),
        }
    }
}

impl FrameworkNativeProgramWire {
    /// Current wire schema version.
    pub const SCHEMA_VERSION: u32 = 1;

    /// Parses a JSON compiler artifact.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError`] when the artifact is not valid JSON or does not match the wire schema.
    pub fn from_json(json: &str) -> Result<Self, AdapterError> {
        serde_json::from_str(json).map_err(|error| {
            AdapterError::with_rule(
                "framework-native-program.json-invalid",
                format!("framework native program artifact is invalid JSON: {error}"),
            )
        })
    }

    /// Serializes this compiler artifact to deterministic JSON.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError`] when the artifact cannot be serialized.
    pub fn to_json(&self) -> Result<String, AdapterError> {
        serde_json::to_string(self).map_err(|error| {
            AdapterError::with_rule(
                "framework-native-program.json-serialize-failed",
                format!("framework native program artifact could not be serialized: {error}"),
            )
        })
    }
}

/// Wire representation of a framework-emitted native node.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FrameworkNativeNodeWire {
    /// Stable native node id.
    pub id: String,
    /// Native element kind.
    pub kind: FrameworkNativeElementKindWire,
    /// Optional framework key for this node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// Ordered typed properties.
    #[serde(default)]
    pub props: Vec<FrameworkNativePropWire>,
    /// Ordered native refs.
    #[serde(default)]
    pub refs: Vec<String>,
    /// Ordered style registry refs.
    #[serde(default)]
    pub style_refs: Vec<String>,
    /// Ordered asset refs.
    #[serde(default)]
    pub asset_refs: Vec<FrameworkNativeAssetWire>,
    /// Ordered event bindings.
    #[serde(default)]
    pub events: Vec<FrameworkNativeEventWire>,
    /// Ordered lifecycle bindings.
    #[serde(default)]
    pub lifecycle: Vec<FrameworkNativeLifecycleWire>,
    /// Ordered child nodes.
    #[serde(default)]
    pub children: Vec<FrameworkNativeChildWire>,
}

/// Wire element kind names.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FrameworkNativeElementKindWire {
    /// Generic layout or grouping view.
    View,
    /// Text node.
    Text,
    /// Button control.
    Button,
    /// Host-rendered custom draw surface.
    CustomSurface,
}

/// Ordered wire property.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FrameworkNativePropWire {
    /// Property name.
    pub name: String,
    /// Typed property value.
    pub value: FrameworkNativePropValueWire,
}

/// Wire property value.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum FrameworkNativePropValueWire {
    /// String property value.
    String(String),
    /// Boolean property value.
    Bool(bool),
    /// Floating-point number property value.
    Number(f64),
}

/// Wire asset reference.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FrameworkNativeAssetWire {
    /// Stable asset registry name.
    pub name: String,
    /// Workspace-relative asset path.
    pub path: String,
}

/// Wire event binding.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FrameworkNativeEventWire {
    /// Stable event key, such as `pointer.press`.
    pub kind: String,
    /// Stable handler reference.
    pub handler: String,
    /// Requested payload fields.
    #[serde(default)]
    pub payload_fields: Vec<FrameworkNativePayloadFieldWire>,
}

/// Wire event payload field.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FrameworkNativePayloadFieldWire {
    /// Pointer or geometry position.
    Position,
    /// Movement delta.
    Delta,
    /// Text or control value.
    Value,
    /// Keyboard key identifier.
    Key,
}

/// Wire lifecycle binding.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FrameworkNativeLifecycleWire {
    /// Lifecycle event.
    pub event: FrameworkNativeLifecycleEventWire,
    /// Stable handler reference.
    pub handler: String,
}

/// Wire lifecycle event.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FrameworkNativeLifecycleEventWire {
    /// Node mounted into the native tree.
    Mounted,
    /// Node suspended while preserving native state.
    Suspended,
    /// Node resumed after a suspension.
    Resumed,
    /// Node reconciled after a hot-reload patch.
    HotReloaded,
    /// Node entered an error boundary.
    ErrorBoundary,
    /// Node received a shutdown notification before unmount.
    Shutdown,
    /// Node removed from the native tree.
    Unmounted,
}

/// Wire child node.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FrameworkNativeChildWire {
    /// Optional append key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// Child node.
    pub node: FrameworkNativeNodeWire,
}

/// Wire reactivity binding.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FrameworkReactiveBindingWire {
    /// Binding kind.
    pub kind: FrameworkReactiveBindingKindWire,
    /// Binding source/name.
    pub name: String,
}

/// Wire reactivity binding kind.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FrameworkReactiveBindingKindWire {
    /// A named signal/source value.
    Signal,
    /// A keyed list rendered from a named source value.
    KeyedForEach,
    /// A named effect/update group.
    Effect,
}

/// Wire runtime binding emitted by a framework compiler for non-static expressions.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FrameworkDynamicBindingWire {
    /// Target native node id.
    pub node_id: String,
    /// Target native property or text slot.
    pub target: FrameworkDynamicBindingTargetWire,
    /// Framework expression preserved by the compiler.
    pub expression: String,
    /// Runtime dependencies needed to re-evaluate the expression.
    #[serde(default)]
    pub dependencies: Vec<String>,
}

/// Wire initial value for a dynamic expression dependency.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FrameworkInitialDynamicValueWire {
    /// Dependency name projected into the expression scope.
    pub name: String,
    /// Projection mode used by the framework expression syntax.
    #[serde(default)]
    pub mode: FrameworkInitialDynamicValueModeWire,
    /// Literal dependency value.
    pub value: FrameworkDynamicValueWire,
}

/// Wire keyed-list template emitted by a framework compiler.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FrameworkListTemplateWire {
    /// Stable template id.
    pub id: String,
    /// Parent node that receives materialized list item roots.
    pub parent_id: String,
    /// Optional static sibling that materialized roots are inserted before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_before: Option<String>,
    /// Dynamic source expression/dependency name.
    pub source: String,
    /// Item binding name visible to template expressions.
    pub item: String,
    /// Key expression evaluated for each item.
    pub key: String,
    /// Root template node materialized for each item.
    pub node: FrameworkListTemplateNodeWire,
}

/// Wire node inside a keyed-list template.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FrameworkListTemplateNodeWire {
    /// Template expression or literal for the materialized node id.
    pub id: FrameworkTemplateScalarWire,
    /// Native element kind.
    pub kind: FrameworkNativeElementKindWire,
    /// Optional template expression or literal for the materialized key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<FrameworkTemplateScalarWire>,
    /// Ordered template props.
    #[serde(default)]
    pub props: Vec<FrameworkTemplatePropWire>,
    /// Ordered native refs.
    #[serde(default)]
    pub refs: Vec<String>,
    /// Ordered style refs.
    #[serde(default)]
    pub style_refs: Vec<String>,
    /// Ordered asset refs.
    #[serde(default)]
    pub asset_refs: Vec<FrameworkNativeAssetWire>,
    /// Ordered event bindings.
    #[serde(default)]
    pub events: Vec<FrameworkNativeEventWire>,
    /// Ordered lifecycle bindings.
    #[serde(default)]
    pub lifecycle: Vec<FrameworkNativeLifecycleWire>,
    /// Ordered child template nodes.
    #[serde(default)]
    pub children: Vec<FrameworkListTemplateNodeWire>,
}

/// Wire template property.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FrameworkTemplatePropWire {
    /// Property name.
    pub name: String,
    /// Literal or expression value.
    pub value: FrameworkTemplateScalarWire,
}

/// Wire scalar used by keyed-list templates.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FrameworkTemplateScalarWire {
    /// Literal scalar value.
    Literal {
        /// Literal scalar.
        value: FrameworkNativePropValueWire,
    },
    /// Expression evaluated with the item binding in scope.
    Expression {
        /// Framework expression.
        expression: String,
    },
}

/// Wire initial dynamic value projection mode.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FrameworkInitialDynamicValueModeWire {
    /// Plain `const name = value` binding.
    #[default]
    Value,
    /// Getter `const name = () => value` binding.
    Getter,
}

/// Wire literal value for initial dynamic expression dependencies.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum FrameworkDynamicValueWire {
    /// Null literal.
    Null,
    /// Boolean literal.
    Bool(bool),
    /// Finite numeric literal.
    Number(f64),
    /// String literal.
    String(String),
    /// Ordered array literal.
    Array(Vec<FrameworkDynamicValueWire>),
    /// Object literal.
    Object(BTreeMap<String, FrameworkDynamicValueWire>),
}

/// Wire executable event handler emitted by a framework compiler.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FrameworkEventHandlerWire {
    /// Stable handler name referenced by event and lifecycle bindings.
    pub name: String,
    /// Deterministic actions performed when the handler runs.
    #[serde(default)]
    pub actions: Vec<FrameworkEventHandlerActionWire>,
}

/// Wire event handler action.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FrameworkEventHandlerActionWire {
    /// Set a dynamic dependency directly to a literal value.
    SetDynamicValue {
        /// Dynamic dependency name to update.
        name: String,
        /// Literal value assigned to the dependency.
        value: FrameworkDynamicValueWire,
    },
    /// Set a dynamic dependency by evaluating a framework expression.
    SetDynamicExpression {
        /// Dynamic dependency name to update.
        name: String,
        /// Expression evaluated in the current dynamic environment.
        expression: String,
        /// Runtime dependencies required by the expression.
        #[serde(default)]
        dependencies: Vec<String>,
    },
}

/// Wire target for a runtime binding.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum FrameworkDynamicBindingTargetWire {
    /// Bind a native property.
    Prop {
        /// Native property name.
        name: String,
    },
    /// Bind the node text slot.
    Text,
}

impl FrameworkNativeProgram {
    /// Creates a framework native program with one root node.
    #[must_use]
    pub const fn new(root: FrameworkNativeNode) -> Self {
        Self {
            compiler: FrameworkCompilerMetadata::unknown(),
            root,
            reactivity: Vec::new(),
            dynamic_bindings: Vec::new(),
            initial_dynamic_values: Vec::new(),
            event_handlers: Vec::new(),
            list_templates: Vec::new(),
        }
    }

    /// Returns compiler provenance for this native program.
    #[must_use]
    pub const fn compiler(&self) -> &FrameworkCompilerMetadata {
        &self.compiler
    }

    /// Returns the root node.
    #[must_use]
    pub const fn root(&self) -> &FrameworkNativeNode {
        &self.root
    }

    /// Sets compiler provenance declared by the compiler boundary.
    #[must_use]
    pub fn with_compiler(mut self, compiler: FrameworkCompilerMetadata) -> Self {
        self.compiler = compiler;
        self
    }

    /// Adds a framework reactivity binding declared by the compiler boundary.
    #[must_use]
    pub fn with_reactive_binding(mut self, binding: FrameworkReactiveBinding) -> Self {
        self.reactivity.push(binding);
        self
    }

    /// Adds a runtime dynamic binding declared by the compiler boundary.
    #[must_use]
    pub fn with_dynamic_binding(mut self, binding: FrameworkDynamicBinding) -> Self {
        self.dynamic_bindings.push(binding);
        self
    }

    /// Adds an initial dependency value declared by the compiler boundary.
    #[must_use]
    pub fn with_initial_dynamic_value(mut self, value: FrameworkInitialDynamicValue) -> Self {
        self.initial_dynamic_values.push(value);
        self
    }

    /// Adds an executable framework event handler declared by the compiler boundary.
    #[must_use]
    pub fn with_event_handler(mut self, handler: FrameworkEventHandler) -> Self {
        self.event_handlers.push(handler);
        self
    }

    /// Adds a keyed-list template declared by the compiler boundary.
    #[must_use]
    pub fn with_list_template(mut self, template: FrameworkListTemplate) -> Self {
        self.list_templates.push(template);
        self
    }

    /// Returns declared reactivity bindings in compiler order.
    #[must_use]
    pub fn reactivity(&self) -> &[FrameworkReactiveBinding] {
        &self.reactivity
    }

    /// Returns declared runtime dynamic bindings in compiler order.
    #[must_use]
    pub fn dynamic_bindings(&self) -> &[FrameworkDynamicBinding] {
        &self.dynamic_bindings
    }

    /// Returns declared initial dynamic dependency values in compiler order.
    #[must_use]
    pub fn initial_dynamic_values(&self) -> &[FrameworkInitialDynamicValue] {
        &self.initial_dynamic_values
    }

    /// Returns executable event handlers in compiler order.
    #[must_use]
    pub fn event_handlers(&self) -> &[FrameworkEventHandler] {
        &self.event_handlers
    }

    /// Returns keyed-list templates in compiler order.
    #[must_use]
    pub fn list_templates(&self) -> &[FrameworkListTemplate] {
        &self.list_templates
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
        emit_node_operations(&mut protocol, None, None, &self.root, 0)?;
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
            0,
        )?);
        for binding in &self.dynamic_bindings {
            runtime.bind_dynamic(binding.clone());
        }
        runtime.finish()
    }
}

impl FrameworkCompilerMetadata {
    /// Creates an explicit compiler provenance record.
    #[must_use]
    pub fn new(
        framework: impl Into<String>,
        compiler: impl Into<String>,
        source_path: impl Into<String>,
        entrypoint: impl Into<String>,
    ) -> Self {
        Self {
            framework: framework.into(),
            compiler: compiler.into(),
            source_path: source_path.into(),
            entrypoint: entrypoint.into(),
        }
    }

    /// Returns fallback provenance used for manually constructed native programs.
    #[must_use]
    pub const fn unknown() -> Self {
        Self {
            framework: String::new(),
            compiler: String::new(),
            source_path: String::new(),
            entrypoint: String::new(),
        }
    }

    /// Returns the framework label that produced the native program.
    #[must_use]
    pub fn framework(&self) -> &str {
        &self.framework
    }

    /// Returns the compiler package or adapter name.
    #[must_use]
    pub fn compiler(&self) -> &str {
        &self.compiler
    }

    /// Returns the workspace-relative source path.
    #[must_use]
    pub fn source_path(&self) -> &str {
        &self.source_path
    }

    /// Returns the framework entrypoint lowered by the compiler.
    #[must_use]
    pub fn entrypoint(&self) -> &str {
        &self.entrypoint
    }
}

impl TryFrom<FrameworkNativeProgramWire> for FrameworkNativeProgram {
    type Error = AdapterError;

    fn try_from(wire: FrameworkNativeProgramWire) -> Result<Self, Self::Error> {
        if wire.schema_version != FrameworkNativeProgramWire::SCHEMA_VERSION {
            return Err(AdapterError::with_rule(
                "framework-native-program.schema-version-unsupported",
                format!(
                    "unsupported framework native program schema version `{}`",
                    wire.schema_version
                ),
            ));
        }

        let mut program = Self::new(framework_native_node_from_wire(wire.root)?)
            .with_compiler(framework_compiler_metadata_from_wire(wire.compiler)?);
        for binding in wire.reactivity {
            program = program.with_reactive_binding(framework_reactivity_from_wire(binding)?);
        }
        for binding in wire.dynamic_bindings {
            program = program.with_dynamic_binding(framework_dynamic_binding_from_wire(binding)?);
        }
        let mut initial_names = BTreeSet::new();
        for value in wire.initial_dynamic_values {
            let value = framework_initial_dynamic_value_from_wire(value)?;
            if !initial_names.insert(value.name().to_string()) {
                return Err(AdapterError::with_rule(
                    "framework-native-program.initial-dynamic-value.duplicate",
                    format!(
                        "initial dynamic value `{}` is declared more than once",
                        value.name()
                    ),
                ));
            }
            program = program.with_initial_dynamic_value(value);
        }
        let mut handler_names = BTreeSet::new();
        for handler in wire.event_handlers {
            let handler = framework_event_handler_from_wire(handler)?;
            if !handler_names.insert(handler.name().to_string()) {
                return Err(AdapterError::with_rule(
                    "framework-native-program.event-handler.duplicate",
                    format!(
                        "event handler `{}` is declared more than once",
                        handler.name()
                    ),
                ));
            }
            program = program.with_event_handler(handler);
        }
        let mut template_ids = BTreeSet::new();
        for template in wire.list_templates {
            let template = framework_list_template_from_wire(template)?;
            if !template_ids.insert(template.id().to_string()) {
                return Err(AdapterError::with_rule(
                    "framework-native-program.list-template.duplicate",
                    format!(
                        "list template `{}` is declared more than once",
                        template.id()
                    ),
                ));
            }
            program = program.with_list_template(template);
        }
        Ok(program)
    }
}

fn framework_compiler_metadata_from_wire(
    wire: FrameworkCompilerMetadataWire,
) -> Result<FrameworkCompilerMetadata, AdapterError> {
    validate_non_empty(
        "framework-native-program.compiler.framework-invalid",
        "compiler framework",
        &wire.framework,
    )?;
    validate_non_empty(
        "framework-native-program.compiler.compiler-invalid",
        "compiler package",
        &wire.compiler,
    )?;
    validate_non_empty(
        "framework-native-program.compiler.source-path-invalid",
        "compiler source path",
        &wire.source_path,
    )?;
    validate_non_empty(
        "framework-native-program.compiler.entrypoint-invalid",
        "compiler entrypoint",
        &wire.entrypoint,
    )?;
    if wire.source_path.starts_with('/')
        || wire.source_path.contains("://")
        || wire.source_path.split('/').any(|segment| segment == "..")
    {
        return Err(AdapterError::with_rule(
            "framework-native-program.compiler.source-path-invalid",
            format!(
                "compiler source path `{}` must be workspace-relative",
                wire.source_path
            ),
        ));
    }
    Ok(FrameworkCompilerMetadata::new(
        wire.framework,
        wire.compiler,
        wire.source_path,
        wire.entrypoint,
    ))
}

fn framework_native_node_from_wire(
    wire: FrameworkNativeNodeWire,
) -> Result<FrameworkNativeNode, AdapterError> {
    validate_non_empty(
        "framework-native-program.node.id-invalid",
        "node id",
        &wire.id,
    )?;
    let mut node = FrameworkNativeNode::new(wire.id, element_kind_from_wire(wire.kind));
    if let Some(key) = wire.key {
        validate_non_empty(
            "framework-native-program.node.key-invalid",
            "node key",
            &key,
        )?;
        node = node.with_key(key);
    }
    for prop in wire.props {
        validate_non_empty(
            "framework-native-program.prop.name-invalid",
            "property name",
            &prop.name,
        )?;
        node = node.with_prop(prop.name, prop_value_from_wire(prop.value)?);
    }
    for reference in wire.refs {
        validate_non_empty(
            "framework-native-program.ref.name-invalid",
            "ref name",
            &reference,
        )?;
        node = node.with_ref(NativeRef::new(reference));
    }
    for style_ref in wire.style_refs {
        validate_non_empty(
            "framework-native-program.style-ref.name-invalid",
            "style ref",
            &style_ref,
        )?;
        node = node.with_style(StyleRef::new(style_ref));
    }
    for asset in wire.asset_refs {
        validate_non_empty(
            "framework-native-program.asset.name-invalid",
            "asset name",
            &asset.name,
        )?;
        validate_non_empty(
            "framework-native-program.asset.path-invalid",
            "asset path",
            &asset.path,
        )?;
        node = node.with_asset(AssetRef::new(asset.name, asset.path));
    }
    for event in wire.events {
        validate_non_empty(
            "framework-native-program.event.handler-invalid",
            "event handler",
            &event.handler,
        )?;
        let event_kind = event.kind.parse::<EventKind>().map_err(|()| {
            AdapterError::with_rule(
                "framework-native-program.event.kind-invalid",
                format!(
                    "framework compiler emitted unsupported event kind `{}`",
                    event.kind
                ),
            )
        })?;
        node = node.with_event(
            event_kind,
            HandlerRef::new(event.handler),
            event
                .payload_fields
                .into_iter()
                .map(event_payload_field_from_wire),
        );
    }
    for lifecycle in wire.lifecycle {
        node = with_lifecycle_from_wire(node, lifecycle)?;
    }
    for child in wire.children {
        node = with_child_from_wire(node, child)?;
    }
    Ok(node)
}

fn with_lifecycle_from_wire(
    node: FrameworkNativeNode,
    lifecycle: FrameworkNativeLifecycleWire,
) -> Result<FrameworkNativeNode, AdapterError> {
    validate_non_empty(
        "framework-native-program.lifecycle.handler-invalid",
        "lifecycle handler",
        &lifecycle.handler,
    )?;
    Ok(node.with_lifecycle(
        lifecycle_event_from_wire(lifecycle.event),
        HandlerRef::new(lifecycle.handler),
    ))
}

fn with_child_from_wire(
    node: FrameworkNativeNode,
    child: FrameworkNativeChildWire,
) -> Result<FrameworkNativeNode, AdapterError> {
    let child_node = framework_native_node_from_wire(child.node)?;
    let Some(key) = child.key else {
        return Ok(node.with_unkeyed_child(child_node));
    };
    validate_non_empty(
        "framework-native-program.child.key-invalid",
        "child key",
        &key,
    )?;
    Ok(node.with_child(key, child_node))
}

fn element_kind_from_wire(kind: FrameworkNativeElementKindWire) -> ElementKind {
    match kind {
        FrameworkNativeElementKindWire::View => ElementKind::View,
        FrameworkNativeElementKindWire::Text => ElementKind::Text,
        FrameworkNativeElementKindWire::Button => ElementKind::Button,
        FrameworkNativeElementKindWire::CustomSurface => ElementKind::CustomSurface,
    }
}

fn prop_value_from_wire(value: FrameworkNativePropValueWire) -> Result<PropValue, AdapterError> {
    match value {
        FrameworkNativePropValueWire::String(value) => Ok(PropValue::String(value)),
        FrameworkNativePropValueWire::Bool(value) => Ok(PropValue::Bool(value)),
        FrameworkNativePropValueWire::Number(value) if value.is_finite() => {
            Ok(PropValue::Number(value))
        }
        FrameworkNativePropValueWire::Number(value) => Err(AdapterError::with_rule(
            "framework-native-program.prop.number-invalid",
            format!("framework compiler emitted non-finite numeric property value `{value}`"),
        )),
    }
}

const fn event_payload_field_from_wire(
    field: FrameworkNativePayloadFieldWire,
) -> EventPayloadField {
    match field {
        FrameworkNativePayloadFieldWire::Position => EventPayloadField::Position,
        FrameworkNativePayloadFieldWire::Delta => EventPayloadField::Delta,
        FrameworkNativePayloadFieldWire::Value => EventPayloadField::Value,
        FrameworkNativePayloadFieldWire::Key => EventPayloadField::Key,
    }
}

const fn lifecycle_event_from_wire(
    event: FrameworkNativeLifecycleEventWire,
) -> NativeLifecycleEvent {
    match event {
        FrameworkNativeLifecycleEventWire::Mounted => NativeLifecycleEvent::Mounted,
        FrameworkNativeLifecycleEventWire::Suspended => NativeLifecycleEvent::Suspended,
        FrameworkNativeLifecycleEventWire::Resumed => NativeLifecycleEvent::Resumed,
        FrameworkNativeLifecycleEventWire::HotReloaded => NativeLifecycleEvent::HotReloaded,
        FrameworkNativeLifecycleEventWire::ErrorBoundary => NativeLifecycleEvent::ErrorBoundary,
        FrameworkNativeLifecycleEventWire::Shutdown => NativeLifecycleEvent::Shutdown,
        FrameworkNativeLifecycleEventWire::Unmounted => NativeLifecycleEvent::Unmounted,
    }
}

fn framework_reactivity_from_wire(
    binding: FrameworkReactiveBindingWire,
) -> Result<FrameworkReactiveBinding, AdapterError> {
    validate_non_empty(
        "framework-native-program.reactivity.name-invalid",
        "reactivity binding name",
        &binding.name,
    )?;
    Ok(match binding.kind {
        FrameworkReactiveBindingKindWire::Signal => FrameworkReactiveBinding::Signal(binding.name),
        FrameworkReactiveBindingKindWire::KeyedForEach => {
            FrameworkReactiveBinding::KeyedForEach(binding.name)
        }
        FrameworkReactiveBindingKindWire::Effect => FrameworkReactiveBinding::Effect(binding.name),
    })
}

fn framework_dynamic_binding_from_wire(
    binding: FrameworkDynamicBindingWire,
) -> Result<FrameworkDynamicBinding, AdapterError> {
    validate_non_empty(
        "framework-native-program.dynamic-binding.node-id-invalid",
        "dynamic binding node id",
        &binding.node_id,
    )?;
    validate_non_empty(
        "framework-native-program.dynamic-binding.expression-invalid",
        "dynamic binding expression",
        &binding.expression,
    )?;
    for dependency in &binding.dependencies {
        validate_non_empty(
            "framework-native-program.dynamic-binding.dependency-invalid",
            "dynamic binding dependency",
            dependency,
        )?;
    }
    let target = match binding.target {
        FrameworkDynamicBindingTargetWire::Prop { name } => {
            validate_non_empty(
                "framework-native-program.dynamic-binding.target-invalid",
                "dynamic binding target property",
                &name,
            )?;
            FrameworkDynamicBindingTarget::Prop { name }
        }
        FrameworkDynamicBindingTargetWire::Text => FrameworkDynamicBindingTarget::Text,
    };
    Ok(FrameworkDynamicBinding {
        node_id: binding.node_id,
        target,
        expression: binding.expression,
        dependencies: binding.dependencies,
    })
}

fn framework_initial_dynamic_value_from_wire(
    value: FrameworkInitialDynamicValueWire,
) -> Result<FrameworkInitialDynamicValue, AdapterError> {
    validate_non_empty(
        "framework-native-program.initial-dynamic-value.name-invalid",
        "initial dynamic value name",
        &value.name,
    )?;
    let dynamic_value = framework_dynamic_value_from_wire(value.value)?;
    Ok(match value.mode {
        FrameworkInitialDynamicValueModeWire::Value => {
            FrameworkInitialDynamicValue::value(value.name, dynamic_value)
        }
        FrameworkInitialDynamicValueModeWire::Getter => {
            FrameworkInitialDynamicValue::getter(value.name, dynamic_value)
        }
    })
}

fn framework_list_template_from_wire(
    template: FrameworkListTemplateWire,
) -> Result<FrameworkListTemplate, AdapterError> {
    validate_non_empty(
        "framework-native-program.list-template.id-invalid",
        "list template id",
        &template.id,
    )?;
    validate_non_empty(
        "framework-native-program.list-template.parent-id-invalid",
        "list template parent id",
        &template.parent_id,
    )?;
    if let Some(anchor_before) = template.anchor_before.as_ref() {
        validate_non_empty(
            "framework-native-program.list-template.anchor-before-invalid",
            "list template anchor id",
            anchor_before,
        )?;
    }
    validate_non_empty(
        "framework-native-program.list-template.source-invalid",
        "list template source",
        &template.source,
    )?;
    validate_non_empty(
        "framework-native-program.list-template.item-invalid",
        "list template item binding",
        &template.item,
    )?;
    validate_non_empty(
        "framework-native-program.list-template.key-invalid",
        "list template key expression",
        &template.key,
    )?;
    let mut list_template = FrameworkListTemplate::new(
        template.id,
        template.parent_id,
        template.source,
        template.item,
        template.key,
        framework_list_template_node_from_wire(template.node)?,
    );
    if let Some(anchor_before) = template.anchor_before {
        list_template = list_template.with_anchor_before(anchor_before);
    }
    Ok(list_template)
}

fn framework_list_template_node_from_wire(
    node: FrameworkListTemplateNodeWire,
) -> Result<FrameworkListTemplateNode, AdapterError> {
    let mut typed = FrameworkListTemplateNode::new(
        framework_template_scalar_from_wire(node.id)?,
        element_kind_from_wire(node.kind),
    );
    if let Some(key) = node.key {
        typed = typed.with_key(framework_template_scalar_from_wire(key)?);
    }
    for prop in node.props {
        validate_non_empty(
            "framework-native-program.list-template.prop.name-invalid",
            "list template property name",
            &prop.name,
        )?;
        typed = typed.with_prop(prop.name, framework_template_scalar_from_wire(prop.value)?);
    }
    for reference in node.refs {
        validate_non_empty(
            "framework-native-program.list-template.ref.name-invalid",
            "list template ref name",
            &reference,
        )?;
        typed = typed.with_ref(NativeRef::new(reference));
    }
    for style_ref in node.style_refs {
        validate_non_empty(
            "framework-native-program.list-template.style-ref.name-invalid",
            "list template style ref",
            &style_ref,
        )?;
        typed = typed.with_style(StyleRef::new(style_ref));
    }
    for asset in node.asset_refs {
        validate_non_empty(
            "framework-native-program.list-template.asset.name-invalid",
            "list template asset name",
            &asset.name,
        )?;
        validate_non_empty(
            "framework-native-program.list-template.asset.path-invalid",
            "list template asset path",
            &asset.path,
        )?;
        typed = typed.with_asset(AssetRef::new(asset.name, asset.path));
    }
    for event in node.events {
        validate_non_empty(
            "framework-native-program.list-template.event.handler-invalid",
            "list template event handler",
            &event.handler,
        )?;
        let event_kind = event.kind.parse::<EventKind>().map_err(|()| {
            AdapterError::with_rule(
                "framework-native-program.list-template.event.kind-invalid",
                format!(
                    "framework compiler emitted unsupported list template event kind `{}`",
                    event.kind
                ),
            )
        })?;
        typed = typed.with_event(
            event_kind,
            HandlerRef::new(event.handler),
            event
                .payload_fields
                .into_iter()
                .map(event_payload_field_from_wire)
                .collect(),
        );
    }
    for lifecycle in node.lifecycle {
        validate_non_empty(
            "framework-native-program.list-template.lifecycle.handler-invalid",
            "list template lifecycle handler",
            &lifecycle.handler,
        )?;
        typed = typed.with_lifecycle(
            lifecycle_event_from_wire(lifecycle.event),
            HandlerRef::new(lifecycle.handler),
        );
    }
    for child in node.children {
        typed = typed.with_child(framework_list_template_node_from_wire(child)?);
    }
    Ok(typed)
}

fn framework_template_scalar_from_wire(
    scalar: FrameworkTemplateScalarWire,
) -> Result<FrameworkTemplateScalar, AdapterError> {
    match scalar {
        FrameworkTemplateScalarWire::Literal { value } => {
            prop_value_from_wire(value).map(FrameworkTemplateScalar::Literal)
        }
        FrameworkTemplateScalarWire::Expression { expression } => {
            validate_non_empty(
                "framework-native-program.list-template.expression-invalid",
                "list template expression",
                &expression,
            )?;
            Ok(FrameworkTemplateScalar::Expression(expression))
        }
    }
}

fn framework_event_handler_from_wire(
    handler: FrameworkEventHandlerWire,
) -> Result<FrameworkEventHandler, AdapterError> {
    validate_non_empty(
        "framework-native-program.event-handler.name-invalid",
        "event handler name",
        &handler.name,
    )?;
    if handler.actions.is_empty() {
        return Err(AdapterError::with_rule(
            "framework-native-program.event-handler.actions-empty",
            format!(
                "event handler `{}` must contain at least one action",
                handler.name
            ),
        ));
    }
    let mut typed = FrameworkEventHandler::new(handler.name);
    for action in handler.actions {
        typed = typed.with_action(framework_event_handler_action_from_wire(action)?);
    }
    Ok(typed)
}

fn framework_event_handler_action_from_wire(
    action: FrameworkEventHandlerActionWire,
) -> Result<FrameworkEventHandlerAction, AdapterError> {
    match action {
        FrameworkEventHandlerActionWire::SetDynamicValue { name, value } => {
            validate_non_empty(
                "framework-native-program.event-handler.action.name-invalid",
                "event handler action dependency name",
                &name,
            )?;
            Ok(FrameworkEventHandlerAction::set_dynamic_value(
                name,
                framework_dynamic_value_from_wire(value)?,
            ))
        }
        FrameworkEventHandlerActionWire::SetDynamicExpression {
            name,
            expression,
            dependencies,
        } => {
            validate_non_empty(
                "framework-native-program.event-handler.action.name-invalid",
                "event handler action dependency name",
                &name,
            )?;
            validate_non_empty(
                "framework-native-program.event-handler.action.expression-invalid",
                "event handler action expression",
                &expression,
            )?;
            for dependency in &dependencies {
                validate_non_empty(
                    "framework-native-program.event-handler.action.dependency-invalid",
                    "event handler action dependency",
                    dependency,
                )?;
            }
            Ok(FrameworkEventHandlerAction::set_dynamic_expression(
                name,
                expression,
                dependencies,
            ))
        }
    }
}

fn framework_dynamic_value_from_wire(
    value: FrameworkDynamicValueWire,
) -> Result<FrameworkDynamicValue, AdapterError> {
    match value {
        FrameworkDynamicValueWire::Null => Ok(FrameworkDynamicValue::Null),
        FrameworkDynamicValueWire::Bool(value) => Ok(FrameworkDynamicValue::Bool(value)),
        FrameworkDynamicValueWire::Number(value) if value.is_finite() => {
            Ok(FrameworkDynamicValue::Number(value))
        }
        FrameworkDynamicValueWire::Number(_) => Err(AdapterError::with_rule(
            "framework-native-program.initial-dynamic-value.number-invalid",
            "initial dynamic numeric value must be finite",
        )),
        FrameworkDynamicValueWire::String(value) => Ok(FrameworkDynamicValue::String(value)),
        FrameworkDynamicValueWire::Array(values) => values
            .into_iter()
            .map(framework_dynamic_value_from_wire)
            .collect::<Result<Vec<_>, _>>()
            .map(FrameworkDynamicValue::Array),
        FrameworkDynamicValueWire::Object(values) => values
            .into_iter()
            .map(|(key, value)| framework_dynamic_value_from_wire(value).map(|value| (key, value)))
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map(FrameworkDynamicValue::Object),
    }
}

fn validate_non_empty(
    rule: &'static str,
    label: &'static str,
    value: &str,
) -> Result<(), AdapterError> {
    if value.trim().is_empty() {
        Err(AdapterError::with_rule(
            rule,
            format!("framework compiler emitted empty {label}"),
        ))
    } else {
        Ok(())
    }
}

/// Runtime dynamic binding declared by a framework compiler boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameworkDynamicBinding {
    node_id: String,
    target: FrameworkDynamicBindingTarget,
    expression: String,
    dependencies: Vec<String>,
}

impl FrameworkDynamicBinding {
    /// Creates a property binding for a framework expression.
    #[must_use]
    pub fn prop(
        node_id: impl Into<String>,
        name: impl Into<String>,
        expression: impl Into<String>,
        dependencies: impl Into<Vec<String>>,
    ) -> Self {
        Self {
            node_id: node_id.into(),
            target: FrameworkDynamicBindingTarget::Prop { name: name.into() },
            expression: expression.into(),
            dependencies: dependencies.into(),
        }
    }

    /// Creates a text-slot binding for a framework expression.
    #[must_use]
    pub fn text(
        node_id: impl Into<String>,
        expression: impl Into<String>,
        dependencies: impl Into<Vec<String>>,
    ) -> Self {
        Self {
            node_id: node_id.into(),
            target: FrameworkDynamicBindingTarget::Text,
            expression: expression.into(),
            dependencies: dependencies.into(),
        }
    }

    /// Returns the target node id.
    #[must_use]
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Returns the binding target.
    #[must_use]
    pub const fn target(&self) -> &FrameworkDynamicBindingTarget {
        &self.target
    }

    /// Returns the preserved framework expression.
    #[must_use]
    pub fn expression(&self) -> &str {
        &self.expression
    }

    /// Returns runtime dependency names in compiler order.
    #[must_use]
    pub fn dependencies(&self) -> &[String] {
        &self.dependencies
    }

    /// Returns the stable key used by diagnostics and runtime bridge records.
    #[must_use]
    pub fn stable_key(&self) -> String {
        match &self.target {
            FrameworkDynamicBindingTarget::Prop { name } => {
                format!("{}:prop:{name}={}", self.node_id, self.expression)
            }
            FrameworkDynamicBindingTarget::Text => {
                format!("{}:text={}", self.node_id, self.expression)
            }
        }
    }
}

/// Target for a runtime dynamic binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrameworkDynamicBindingTarget {
    /// Bind a named native property.
    Prop {
        /// Native property name.
        name: String,
    },
    /// Bind the node text slot.
    Text,
}

/// Keyed-list template declared by a framework compiler boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameworkListTemplate {
    id: String,
    parent_id: String,
    anchor_before: Option<String>,
    source: String,
    item: String,
    key: String,
    node: FrameworkListTemplateNode,
}

impl FrameworkListTemplate {
    /// Creates a keyed-list template.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        parent_id: impl Into<String>,
        source: impl Into<String>,
        item: impl Into<String>,
        key: impl Into<String>,
        node: FrameworkListTemplateNode,
    ) -> Self {
        Self {
            id: id.into(),
            parent_id: parent_id.into(),
            anchor_before: None,
            source: source.into(),
            item: item.into(),
            key: key.into(),
            node,
        }
    }

    /// Sets the optional static sibling that materialized roots are inserted before.
    #[must_use]
    pub fn with_anchor_before(mut self, anchor_before: impl Into<String>) -> Self {
        self.anchor_before = Some(anchor_before.into());
        self
    }

    /// Returns the stable template id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the parent node id.
    #[must_use]
    pub fn parent_id(&self) -> &str {
        &self.parent_id
    }

    /// Returns the optional static sibling that materialized roots are inserted before.
    #[must_use]
    pub fn anchor_before(&self) -> Option<&str> {
        self.anchor_before.as_deref()
    }

    /// Returns the dynamic list source.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Returns the item binding name.
    #[must_use]
    pub fn item(&self) -> &str {
        &self.item
    }

    /// Returns the item key expression.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Returns the root template node.
    #[must_use]
    pub const fn node(&self) -> &FrameworkListTemplateNode {
        &self.node
    }
}

/// Node template materialized for each item in a keyed list.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameworkListTemplateNode {
    id: FrameworkTemplateScalar,
    kind: ElementKind,
    key: Option<FrameworkTemplateScalar>,
    props: Vec<(String, FrameworkTemplateScalar)>,
    refs: Vec<NativeRef>,
    style_refs: Vec<StyleRef>,
    asset_refs: Vec<AssetRef>,
    events: Vec<(EventKind, HandlerRef, Vec<EventPayloadField>)>,
    lifecycle: Vec<(NativeLifecycleEvent, HandlerRef)>,
    children: Vec<FrameworkListTemplateNode>,
}

impl FrameworkListTemplateNode {
    /// Creates a list template node.
    #[must_use]
    pub fn new(id: FrameworkTemplateScalar, kind: ElementKind) -> Self {
        Self {
            id,
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

    /// Sets the materialized key expression or literal.
    #[must_use]
    pub fn with_key(mut self, key: FrameworkTemplateScalar) -> Self {
        self.key = Some(key);
        self
    }

    /// Adds a template property.
    #[must_use]
    pub fn with_prop(mut self, name: impl Into<String>, value: FrameworkTemplateScalar) -> Self {
        self.props.push((name.into(), value));
        self
    }

    /// Adds a native ref copied to materialized nodes.
    #[must_use]
    pub fn with_ref(mut self, reference: NativeRef) -> Self {
        self.refs.push(reference);
        self
    }

    /// Adds a style ref copied to materialized nodes.
    #[must_use]
    pub fn with_style(mut self, style_ref: StyleRef) -> Self {
        self.style_refs.push(style_ref);
        self
    }

    /// Adds an asset ref copied to materialized nodes.
    #[must_use]
    pub fn with_asset(mut self, asset_ref: AssetRef) -> Self {
        self.asset_refs.push(asset_ref);
        self
    }

    /// Adds an event copied to materialized nodes.
    #[must_use]
    pub fn with_event(
        mut self,
        event: EventKind,
        handler: HandlerRef,
        payload_fields: Vec<EventPayloadField>,
    ) -> Self {
        self.events.push((event, handler, payload_fields));
        self
    }

    /// Adds a lifecycle binding copied to materialized nodes.
    #[must_use]
    pub fn with_lifecycle(mut self, event: NativeLifecycleEvent, handler: HandlerRef) -> Self {
        self.lifecycle.push((event, handler));
        self
    }

    /// Adds a child template node.
    #[must_use]
    pub fn with_child(mut self, child: FrameworkListTemplateNode) -> Self {
        self.children.push(child);
        self
    }

    /// Returns the id scalar.
    #[must_use]
    pub const fn id(&self) -> &FrameworkTemplateScalar {
        &self.id
    }

    /// Returns the element kind.
    #[must_use]
    pub const fn kind(&self) -> ElementKind {
        self.kind
    }

    /// Returns the optional key scalar.
    #[must_use]
    pub const fn key(&self) -> Option<&FrameworkTemplateScalar> {
        self.key.as_ref()
    }

    /// Returns template properties.
    #[must_use]
    pub fn props(&self) -> &[(String, FrameworkTemplateScalar)] {
        &self.props
    }

    /// Returns refs.
    #[must_use]
    pub fn refs(&self) -> &[NativeRef] {
        &self.refs
    }

    /// Returns style refs.
    #[must_use]
    pub fn style_refs(&self) -> &[StyleRef] {
        &self.style_refs
    }

    /// Returns asset refs.
    #[must_use]
    pub fn asset_refs(&self) -> &[AssetRef] {
        &self.asset_refs
    }

    /// Returns template events.
    #[must_use]
    pub fn events(&self) -> &[(EventKind, HandlerRef, Vec<EventPayloadField>)] {
        &self.events
    }

    /// Returns lifecycle bindings.
    #[must_use]
    pub fn lifecycle(&self) -> &[(NativeLifecycleEvent, HandlerRef)] {
        &self.lifecycle
    }

    /// Returns child template nodes.
    #[must_use]
    pub fn children(&self) -> &[FrameworkListTemplateNode] {
        &self.children
    }
}

/// Literal or expression scalar inside a framework list template.
#[derive(Clone, Debug, PartialEq)]
pub enum FrameworkTemplateScalar {
    /// Literal scalar.
    Literal(PropValue),
    /// Framework expression evaluated against the item binding.
    Expression(String),
}

/// Initial value for a dynamic expression dependency.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameworkInitialDynamicValue {
    name: String,
    mode: FrameworkInitialDynamicValueMode,
    value: FrameworkDynamicValue,
}

impl FrameworkInitialDynamicValue {
    /// Creates a plain value dependency.
    #[must_use]
    pub fn value(name: impl Into<String>, value: FrameworkDynamicValue) -> Self {
        Self {
            name: name.into(),
            mode: FrameworkInitialDynamicValueMode::Value,
            value,
        }
    }

    /// Creates a getter dependency.
    #[must_use]
    pub fn getter(name: impl Into<String>, value: FrameworkDynamicValue) -> Self {
        Self {
            name: name.into(),
            mode: FrameworkInitialDynamicValueMode::Getter,
            value,
        }
    }

    /// Returns the dependency name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the projection mode.
    #[must_use]
    pub const fn mode(&self) -> FrameworkInitialDynamicValueMode {
        self.mode
    }

    /// Returns the dependency value.
    #[must_use]
    pub const fn value_ref(&self) -> &FrameworkDynamicValue {
        &self.value
    }
}

/// Initial dynamic dependency projection mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameworkInitialDynamicValueMode {
    /// Plain value binding.
    Value,
    /// Getter binding.
    Getter,
}

/// Literal value for initial dynamic expression dependencies.
#[derive(Clone, Debug, PartialEq)]
pub enum FrameworkDynamicValue {
    /// Null literal.
    Null,
    /// Boolean literal.
    Bool(bool),
    /// Finite numeric literal.
    Number(f64),
    /// String literal.
    String(String),
    /// Ordered array literal.
    Array(Vec<FrameworkDynamicValue>),
    /// Object literal.
    Object(BTreeMap<String, FrameworkDynamicValue>),
}

/// Executable event handler declared by a framework compiler boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameworkEventHandler {
    name: String,
    actions: Vec<FrameworkEventHandlerAction>,
}

impl FrameworkEventHandler {
    /// Creates an event handler with a stable compiler-emitted name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            actions: Vec::new(),
        }
    }

    /// Adds an action to this handler.
    #[must_use]
    pub fn with_action(mut self, action: FrameworkEventHandlerAction) -> Self {
        self.actions.push(action);
        self
    }

    /// Returns the stable handler name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns handler actions in compiler order.
    #[must_use]
    pub fn actions(&self) -> &[FrameworkEventHandlerAction] {
        &self.actions
    }
}

/// Deterministic action performed by a framework event handler.
#[derive(Clone, Debug, PartialEq)]
pub enum FrameworkEventHandlerAction {
    /// Set a dynamic dependency directly to a literal value.
    SetDynamicValue {
        /// Dynamic dependency name to update.
        name: String,
        /// Literal value assigned to the dependency.
        value: FrameworkDynamicValue,
    },
    /// Set a dynamic dependency from an evaluated framework expression.
    SetDynamicExpression {
        /// Dynamic dependency name to update.
        name: String,
        /// Expression evaluated in the current dynamic environment.
        expression: String,
        /// Runtime dependencies required by the expression.
        dependencies: Vec<String>,
    },
}

impl FrameworkEventHandlerAction {
    /// Creates a literal dynamic dependency assignment.
    #[must_use]
    pub fn set_dynamic_value(name: impl Into<String>, value: FrameworkDynamicValue) -> Self {
        Self::SetDynamicValue {
            name: name.into(),
            value,
        }
    }

    /// Creates an expression-backed dynamic dependency assignment.
    #[must_use]
    pub fn set_dynamic_expression(
        name: impl Into<String>,
        expression: impl Into<String>,
        dependencies: impl Into<Vec<String>>,
    ) -> Self {
        Self::SetDynamicExpression {
            name: name.into(),
            expression: expression.into(),
            dependencies: dependencies.into(),
        }
    }

    /// Returns the dynamic dependency updated by this action.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::SetDynamicValue { name, .. } | Self::SetDynamicExpression { name, .. } => name,
        }
    }

    /// Returns the literal value for direct assignments.
    #[must_use]
    pub const fn value(&self) -> Option<&FrameworkDynamicValue> {
        match self {
            Self::SetDynamicValue { value, .. } => Some(value),
            Self::SetDynamicExpression { .. } => None,
        }
    }

    /// Returns the preserved framework expression for expression assignments.
    #[must_use]
    pub fn expression(&self) -> Option<&str> {
        match self {
            Self::SetDynamicValue { .. } => None,
            Self::SetDynamicExpression { expression, .. } => Some(expression),
        }
    }

    /// Returns expression dependencies in compiler order.
    #[must_use]
    pub fn dependencies(&self) -> &[String] {
        match self {
            Self::SetDynamicValue { .. } => &[],
            Self::SetDynamicExpression { dependencies, .. } => dependencies,
        }
    }

    /// Returns the stable diagnostic key for this action.
    #[must_use]
    pub fn stable_key(&self) -> String {
        match self {
            Self::SetDynamicValue { name, .. } => format!("set:{name}"),
            Self::SetDynamicExpression {
                name, expression, ..
            } => {
                format!("set:{name}={expression}")
            }
        }
    }
}

/// Reactive primitive declared by a framework compiler boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrameworkReactiveBinding {
    /// A named signal/source value.
    Signal(String),
    /// A keyed list rendered from a named source value.
    KeyedForEach(String),
    /// A named effect/update group.
    Effect(String),
}

impl FrameworkReactiveBinding {
    /// Creates a signal binding.
    #[must_use]
    pub fn signal(name: impl Into<String>) -> Self {
        Self::Signal(name.into())
    }

    /// Creates a keyed list binding.
    #[must_use]
    pub fn keyed_for_each(source: impl Into<String>) -> Self {
        Self::KeyedForEach(source.into())
    }

    /// Creates an effect binding.
    #[must_use]
    pub fn effect(name: impl Into<String>) -> Self {
        Self::Effect(name.into())
    }

    /// Returns the stable reactivity key used by diagnostics and Solid integration output.
    #[must_use]
    pub fn stable_key(&self) -> String {
        match self {
            Self::Signal(name) => format!("signal:{name}"),
            Self::KeyedForEach(source) => format!("for-each:keyed:{source}"),
            Self::Effect(name) => format!("effect:{name}"),
        }
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
    pub(crate) fn stable_key(&self) -> String {
        match self {
            Self::MountElement(node) => operation_keys::mount_element_key(node.id()),
            Self::MountComponent(component) => {
                operation_keys::mount_component_key(component.id().as_str())
            }
            Self::DeclareSurface(surface) => {
                operation_keys::declare_surface_key(surface.id().as_str())
            }
            Self::BindEvent(binding) => operation_keys::bind_event_key(binding),
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
            Self::CreateNode { id, kind } => operation_keys::create_node_key(id, *kind),
            Self::SetProp { id, name, .. } => operation_keys::set_prop_key(id, name),
            Self::SetStyleRef { id, style_ref } => {
                operation_keys::set_style_key(id, style_ref.name())
            }
            Self::SetAssetRef { id, asset_ref } => {
                operation_keys::set_asset_key(id, asset_ref.path())
            }
            Self::SetRef { id, reference } => operation_keys::set_ref_key(id, reference.name()),
            Self::BindEvent { binding } => operation_keys::bind_event_key(binding),
            Self::BindLifecycle { id, event, handler } => {
                operation_keys::bind_lifecycle_key(id, *event, handler)
            }
            Self::AppendChild { parent, child, key } => {
                operation_keys::append_child_key(parent, child, key.as_deref())
            }
            Self::EnterErrorBoundary { id, handler } => {
                operation_keys::error_boundary_key(id, handler)
            }
            Self::Commit { root } => operation_keys::commit_key(root),
            Self::RemoveNode { id } => operation_keys::remove_node_key(id),
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

impl From<CustomRendererError> for Diagnostic {
    fn from(error: CustomRendererError) -> Self {
        Self::error(error.rule, error.message)
    }
}

/// Public custom renderer protocol used by framework integrations.
#[derive(Clone, Debug, PartialEq)]
pub struct CustomRendererProtocol {
    framework_label: String,
    live_nodes: BTreeSet<ElementId>,
    keyed_children: BTreeMap<(ElementId, String), ElementId>,
    operations: Vec<CustomRendererOperation>,
    operation_keys: Vec<String>,
}

impl CustomRendererProtocol {
    /// Creates a protocol recorder for a framework label.
    #[must_use]
    pub fn new(framework_label: impl Into<String>) -> Self {
        Self {
            framework_label: framework_label.into(),
            live_nodes: BTreeSet::new(),
            keyed_children: BTreeMap::new(),
            operations: Vec::new(),
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
        match &operation {
            CustomRendererOperation::CreateNode { id, .. } => {
                self.live_nodes.insert(id.clone());
            }
            CustomRendererOperation::RemoveNode { id } => {
                self.live_nodes.remove(id);
                self.remove_child_bindings_for(id);
            }
            CustomRendererOperation::AppendChild {
                parent,
                child,
                key: Some(key),
            } => {
                self.keyed_children
                    .insert((parent.clone(), key.clone()), child.clone());
            }
            _ => {}
        }
        self.operations.push(operation);
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

    /// Returns typed operations in application order.
    #[must_use]
    pub fn operations(&self) -> &[CustomRendererOperation] {
        &self.operations
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
                } else if let CustomRendererOperation::AppendChild { key: Some(key), .. } =
                    operation
                {
                    if let Some(existing_child) =
                        self.keyed_children.get(&(parent.clone(), key.clone()))
                    {
                        Err(CustomRendererError::new(
                            "custom-renderer.child-key.duplicate",
                            format!(
                                "custom renderer parent `{}` already has keyed child `{key}` bound to `{}`",
                                parent.as_str(),
                                existing_child.as_str()
                            ),
                        ))
                    } else {
                        Ok(())
                    }
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        }
    }

    fn remove_child_bindings_for(&mut self, id: &ElementId) {
        self.keyed_children
            .retain(|(parent, _), child| parent != id && child != id);
    }
}

/// Native renderer adapter error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterError {
    rule: String,
    message: String,
}

impl AdapterError {
    /// Creates an adapter error.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            rule: "adapter.error".to_string(),
            message: message.into(),
        }
    }

    /// Creates an adapter error with a stable diagnostic rule.
    #[must_use]
    pub fn with_rule(rule: impl Into<String>, message: impl Into<String>) -> Self {
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

    /// Returns the adapter error message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl From<AdapterError> for Diagnostic {
    fn from(error: AdapterError) -> Self {
        Self::error(error.rule, error.message)
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

fn emit_node_operations(
    protocol: &mut CustomRendererProtocol,
    parent: Option<&ElementId>,
    append_key: Option<&str>,
    node: &FrameworkNativeNode,
    depth: usize,
) -> Result<(), CustomRendererError> {
    if depth > MAX_AUTHORING_TREE_DEPTH {
        return Err(CustomRendererError::new(
            "custom-renderer.tree.depth-exceeded",
            format!("framework node tree exceeds maximum depth of {MAX_AUTHORING_TREE_DEPTH}"),
        ));
    }
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
        if *event == NativeLifecycleEvent::ErrorBoundary {
            protocol.apply(CustomRendererOperation::EnterErrorBoundary {
                id: node.id.clone(),
                handler: HandlerRef::new(handler.as_str()),
            })?;
        } else {
            protocol.apply(CustomRendererOperation::BindLifecycle {
                id: node.id.clone(),
                event: *event,
                handler: HandlerRef::new(handler.as_str()),
            })?;
        }
    }
    if let Some(parent) = parent {
        protocol.apply(CustomRendererOperation::AppendChild {
            parent: parent.clone(),
            child: node.id.clone(),
            key: append_key.map(str::to_string).or_else(|| node.key.clone()),
        })?;
    }
    for (child_key, child) in &node.children {
        emit_node_operations(
            protocol,
            Some(&node.id),
            child_key.as_deref(),
            child,
            depth + 1,
        )?;
    }
    Ok(())
}

fn framework_node_to_native_element(
    node: &FrameworkNativeNode,
    include_default_visual_props: bool,
    is_root: bool,
    depth: usize,
) -> Result<NativeAuthoringElement, NativeAuthoringError> {
    if depth > MAX_AUTHORING_TREE_DEPTH {
        return Err(NativeAuthoringError::from_diagnostics(vec![
            crate::AuthoringDiagnostic::new(
                crate::AuthoringDiagnosticSeverity::Error,
                "native.tree.depth-exceeded",
                format!(
                    "native authoring tree exceeds maximum depth of {MAX_AUTHORING_TREE_DEPTH}"
                ),
            ),
        ]));
    }
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
        let child_element = framework_node_to_native_element(child, false, false, depth + 1)?;
        element = element.with_child(match key.as_deref().or_else(|| child.key()) {
            Some(key) => NativeChild::keyed(key, child_element),
            None => NativeChild::ordered(child_element),
        });
    }
    Ok(element)
}

fn missing_node(id: &ElementId) -> CustomRendererError {
    CustomRendererError::new(
        "custom-renderer.node.missing",
        format!("custom renderer node `{}` does not exist", id.as_str()),
    )
}
