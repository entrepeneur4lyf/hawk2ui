#![forbid(unsafe_code)]
//! `React` 19 and later integration for emitting `Hawk2UI` typed records.

use hawk2ui_authoring::{
    AssetRef, AuthoringDiagnostic, AuthoringDiagnosticSeverity, CustomRendererOperation,
    CustomRendererProtocol, ElementId, ElementKind, ElementNode, EventBinding, EventKind,
    EventPayloadField, FrameworkNativeProgram, HandlerRef, LifecycleEventKind,
    NativeAuthoringElement, NativeAuthoringRuntime, NativeChild, NativeLifecycleEvent, NativeRef,
    NativeRuntimeBridge, NativeRuntimeBridgeArtifact, NativeRuntimeBridgeError, PointerEventKind,
    PropValue, StyleRef,
};
use hawk2ui_runtime::RuntimeViewTree;
use hawk2ui_style::{CompiledStyleSheet, TokenSet};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-framework-react";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

/// React element tree source submitted to the custom renderer bridge.
#[derive(Clone, Debug, PartialEq)]
pub struct ReactElementTree {
    author_file: String,
    source: String,
    native_program: Option<FrameworkNativeProgram>,
}

impl ReactElementTree {
    /// Creates a React source unit with its author-visible path.
    #[must_use]
    pub fn new(author_file: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            author_file: author_file.into(),
            source: source.into(),
            native_program: None,
        }
    }

    /// Creates a React source unit from explicit native compiler output.
    #[must_use]
    pub fn from_native_program(
        author_file: impl Into<String>,
        native_program: FrameworkNativeProgram,
    ) -> Self {
        Self {
            author_file: author_file.into(),
            source: String::new(),
            native_program: Some(native_program),
        }
    }
}

/// Source map boundary returned by the React integration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReactSourceMap {
    author_file: String,
}

impl ReactSourceMap {
    /// Returns the author source file associated with diagnostics and records.
    #[must_use]
    pub fn author_file(&self) -> &str {
        &self.author_file
    }
}

/// Rendered React artifact expressed as typed `Hawk2UI` records.
#[derive(Clone, Debug, PartialEq)]
pub struct ReactRenderedArtifact {
    root: ElementNode,
    keyed_children: Vec<String>,
    refs: Vec<String>,
    style_refs: Vec<StyleRef>,
    asset_refs: Vec<AssetRef>,
    events: Vec<EventBinding>,
    lifecycle_handlers: Vec<String>,
    native_program: Option<FrameworkNativeProgram>,
    source_map: ReactSourceMap,
    reconciler_operations: Vec<String>,
}

/// Runtime-ready React artifact.
#[derive(Clone, Debug, PartialEq)]
pub struct ReactRuntimeArtifact {
    rendered: ReactRenderedArtifact,
    runtime: NativeRuntimeBridgeArtifact,
}

impl ReactRuntimeArtifact {
    /// Returns the typed rendered React artifact.
    #[must_use]
    pub const fn rendered(&self) -> &ReactRenderedArtifact {
        &self.rendered
    }

    /// Returns the runtime view tree produced through the native bridge.
    #[must_use]
    pub const fn runtime_tree(&self) -> &RuntimeViewTree {
        self.runtime.runtime_tree()
    }

    /// Returns bridge metadata for a runtime node ID.
    #[must_use]
    pub fn metadata_for(
        &self,
        node_id: &str,
    ) -> Option<&hawk2ui_authoring::NativeRuntimeNodeMetadata> {
        self.runtime.metadata_for(node_id)
    }

    /// Returns stable native operation keys.
    #[must_use]
    pub fn operation_keys(&self) -> &[String] {
        self.runtime.operation_keys()
    }
}

impl ReactRenderedArtifact {
    /// Returns the framework label.
    #[must_use]
    pub const fn framework(&self) -> &'static str {
        "react"
    }

    /// Returns the supported framework version requirement.
    #[must_use]
    pub const fn framework_version_requirement(&self) -> &'static str {
        ">=19"
    }

    /// Returns the rendered root element.
    #[must_use]
    pub const fn root(&self) -> &ElementNode {
        &self.root
    }

    /// Returns keyed children in React reconciliation order.
    #[must_use]
    pub fn keyed_children(&self) -> &[String] {
        &self.keyed_children
    }

    /// Returns native refs in declaration order.
    #[must_use]
    pub fn refs(&self) -> &[String] {
        &self.refs
    }

    /// Returns style reference names in declaration order.
    #[must_use]
    pub fn style_refs(&self) -> Vec<&str> {
        self.style_refs.iter().map(StyleRef::name).collect()
    }

    /// Returns asset references in declaration order.
    #[must_use]
    pub fn asset_refs(&self) -> &[AssetRef] {
        &self.asset_refs
    }

    /// Returns event bindings in declaration order.
    #[must_use]
    pub fn events(&self) -> &[EventBinding] {
        &self.events
    }

    /// Returns lifecycle handler labels in normalized order.
    #[must_use]
    pub fn lifecycle_handlers(&self) -> &[String] {
        &self.lifecycle_handlers
    }

    /// Returns the source map for diagnostics.
    #[must_use]
    pub const fn source_map(&self) -> &ReactSourceMap {
        &self.source_map
    }

    /// Returns the custom renderer reconciler operations in commit order.
    #[must_use]
    pub fn reconciler_operations(&self) -> &[String] {
        &self.reconciler_operations
    }
}

/// React renderer error with source-mapped diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReactRenderError {
    diagnostics: Vec<AuthoringDiagnostic>,
    source_map: ReactSourceMap,
}

impl ReactRenderError {
    /// Returns diagnostics emitted for the author source.
    #[must_use]
    pub fn diagnostics(&self) -> &[AuthoringDiagnostic] {
        &self.diagnostics
    }

    /// Returns source map context for diagnostics.
    #[must_use]
    pub const fn source_map(&self) -> &ReactSourceMap {
        &self.source_map
    }
}

/// React 19 custom renderer integration.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReactIntegration;

impl ReactIntegration {
    /// Creates a React integration instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Renders a React element tree into typed `Hawk2UI` records.
    ///
    /// # Source grammar (raw-source path)
    ///
    /// When the tree carries raw author source (rather than a
    /// [`ReactElementTree::from_native_program`] boundary), this method is an intentional substring
    /// heuristic, **not** a React/TSX parser. It models only a single root `View` element plus a
    /// flat list of `<hawk-text>` / keyed children; it does not parse nested component trees,
    /// non-text leaf kinds, or handler binding expressions (handler identifiers on this path are
    /// fixed labels — e.g. `onPointerDown` always records `handlePress` — not parsed from the
    /// `={…}` expression). High-fidelity authoring is expected to arrive through
    /// [`ReactElementTree::from_native_program`], which carries explicit native-compiler output and
    /// derives handlers, lifecycle, and the nested element tree from that program.
    ///
    /// # Errors
    ///
    /// Returns [`ReactRenderError`] when the source violates the renderer contract.
    pub fn render(self, tree: ReactElementTree) -> Result<ReactRenderedArtifact, ReactRenderError> {
        let source_map = ReactSourceMap {
            author_file: tree.author_file.clone(),
        };
        if let Some(native_program) = tree.native_program {
            return react_artifact_from_native_program(source_map, native_program);
        }
        let mut diagnostics = Vec::new();
        if let Some(asset) = extract_attribute(&tree.source, "data-asset")
            && (asset.contains("://") || asset.starts_with('/') || asset.contains(".."))
        {
            diagnostics.push(AuthoringDiagnostic::new(
                AuthoringDiagnosticSeverity::Error,
                "react.asset.path-invalid",
                "React asset references must use workspace-relative paths",
            ));
        }
        for event in unsupported_react_events(&tree.source) {
            diagnostics.push(AuthoringDiagnostic::new(
                AuthoringDiagnosticSeverity::Error,
                "react.event.unsupported",
                format!("React event `{event}` is not part of the native event contract"),
            ));
        }
        if tree.source.contains("<Missing") {
            diagnostics.push(AuthoringDiagnostic::new(
                AuthoringDiagnosticSeverity::Error,
                "react.reconciler.unresolved-component",
                "React custom renderer could not resolve component",
            ));
        }
        if !diagnostics.is_empty() {
            return Err(ReactRenderError {
                diagnostics,
                source_map,
            });
        }

        let root_id = extract_attribute(&tree.source, "id").unwrap_or_else(|| "root".to_string());
        let mut events = Vec::new();
        if tree.source.contains("onPointerDown") {
            events.push(
                EventBinding::new(
                    ElementId::new(root_id.clone()),
                    EventKind::Pointer(PointerEventKind::Press),
                    HandlerRef::new("handlePress"),
                )
                .with_payload(EventPayloadField::Position),
            );
        }
        if tree.source.contains("onMount") {
            events.push(EventBinding::new(
                ElementId::new(root_id.clone()),
                EventKind::Lifecycle(LifecycleEventKind::Mounted),
                HandlerRef::new("onMount"),
            ));
        }
        if tree.source.contains("onUnmount") {
            events.push(EventBinding::new(
                ElementId::new(root_id.clone()),
                EventKind::Lifecycle(LifecycleEventKind::Unmounted),
                HandlerRef::new("onUnmount"),
            ));
        }

        // Lifecycle handler labels are gated on the same `source.contains(…)` checks as the
        // lifecycle `events` above, so the two public surfaces agree for a given source instead of
        // reporting canned hooks the source never declared. The `from_native_program` path derives
        // these from the program's declared lifecycle in `react_artifact_from_native_program`.
        let mut lifecycle_handlers = Vec::new();
        if tree.source.contains("onMount") {
            lifecycle_handlers.push("mounted:onMount".to_string());
        }
        if tree.source.contains("onUnmount") {
            lifecycle_handlers.push("unmounted:onUnmount".to_string());
        }

        let keyed_children = keyed_children(&tree.source);
        let refs: Vec<_> = extract_attribute(&tree.source, "ref").into_iter().collect();
        let style_refs = style_refs_from_attribute(&tree.source, "className");
        let asset_refs: Vec<_> = extract_attribute(&tree.source, "data-asset")
            .into_iter()
            .map(|path| AssetRef::new("react.asset", path))
            .collect();
        let reconciler_operations = react_protocol_operations(ReactProtocolInput {
            author_file: source_map.author_file(),
            source_text: tree.source.as_str(),
            root_id: root_id.as_str(),
            refs: &refs,
            style_refs: &style_refs,
            asset_refs: &asset_refs,
            events: &events,
            keyed_children: &keyed_children,
        })?;

        Ok(ReactRenderedArtifact {
            root: ElementNode::new(ElementId::new(root_id), ElementKind::View),
            keyed_children,
            refs,
            style_refs,
            asset_refs,
            events,
            lifecycle_handlers,
            native_program: None,
            source_map,
            reconciler_operations,
        })
    }

    /// Renders a React element tree into a runtime-ready native bridge artifact.
    ///
    /// # Errors
    ///
    /// Returns [`ReactRenderError`] when source validation, native authoring finalization, or
    /// runtime bridging fails.
    pub fn render_to_runtime(
        self,
        tree: ReactElementTree,
    ) -> Result<ReactRuntimeArtifact, ReactRenderError> {
        let author_file = tree.author_file.clone();
        let source_text = tree.source.clone();
        let rendered = self.render(tree)?;
        let native_artifact =
            native_artifact_from_react(author_file.as_str(), source_text.as_str(), &rendered)?;
        let runtime = NativeRuntimeBridge::new()
            .bridge_artifact(&native_artifact)
            .map_err(|error| bridge_error(author_file.as_str(), &error))?;
        Ok(ReactRuntimeArtifact { rendered, runtime })
    }

    /// Renders a React element tree into a runtime artifact with compiled style references applied.
    ///
    /// # Errors
    ///
    /// Returns [`ReactRenderError`] when source validation, native authoring finalization, style
    /// resolution, or runtime bridging fails.
    pub fn render_to_runtime_with_styles(
        self,
        tree: ReactElementTree,
        sheet: &CompiledStyleSheet,
        tokens: &TokenSet,
    ) -> Result<ReactRuntimeArtifact, ReactRenderError> {
        let author_file = tree.author_file.clone();
        let source_text = tree.source.clone();
        let rendered = self.render(tree)?;
        let native_artifact = native_artifact_from_react_with_defaults(
            author_file.as_str(),
            source_text.as_str(),
            &rendered,
            false,
        )?;
        let runtime = NativeRuntimeBridge::new()
            .bridge_artifact_with_styles(&native_artifact, sheet, tokens)
            .map_err(|error| bridge_error(author_file.as_str(), &error))?;
        Ok(ReactRuntimeArtifact { rendered, runtime })
    }

    /// Renders a React element tree into a themed runtime artifact.
    ///
    /// # Errors
    ///
    /// Returns [`ReactRenderError`] when source validation, native authoring finalization, theme
    /// token resolution, style resolution, or runtime bridging fails.
    pub fn render_to_runtime_with_theme(
        self,
        tree: ReactElementTree,
        sheet: &CompiledStyleSheet,
        tokens: &TokenSet,
        theme: &str,
    ) -> Result<ReactRuntimeArtifact, ReactRenderError> {
        let author_file = tree.author_file.clone();
        let source_text = tree.source.clone();
        let rendered = self.render(tree)?;
        let native_artifact = native_artifact_from_react_with_defaults(
            author_file.as_str(),
            source_text.as_str(),
            &rendered,
            false,
        )?;
        let runtime = NativeRuntimeBridge::new()
            .bridge_artifact_with_theme(&native_artifact, sheet, tokens, theme)
            .map_err(|error| bridge_error(author_file.as_str(), &error))?;
        Ok(ReactRuntimeArtifact { rendered, runtime })
    }
}

fn native_artifact_from_react(
    author_file: &str,
    source_text: &str,
    rendered: &ReactRenderedArtifact,
) -> Result<hawk2ui_authoring::NativeAuthoringArtifact, ReactRenderError> {
    native_artifact_from_react_with_defaults(author_file, source_text, rendered, true)
}

fn native_artifact_from_react_with_defaults(
    author_file: &str,
    source_text: &str,
    rendered: &ReactRenderedArtifact,
    include_default_visual_props: bool,
) -> Result<hawk2ui_authoring::NativeAuthoringArtifact, ReactRenderError> {
    if let Some(native_program) = &rendered.native_program {
        return native_program
            .to_native_authoring_artifact(author_file, include_default_visual_props)
            .map_err(|error| ReactRenderError {
                diagnostics: error.diagnostics().to_vec(),
                source_map: ReactSourceMap {
                    author_file: author_file.to_string(),
                },
            });
    }
    let mut runtime = NativeAuthoringRuntime::new(author_file);
    let mut root = NativeAuthoringElement::new(rendered.root().id().as_str(), ElementKind::View);
    if include_default_visual_props {
        root = root.with_prop("background", PropValue::String("#080a0e".to_string()));
    }
    for reference in rendered.refs() {
        root = root.with_ref(NativeRef::new(reference));
    }
    for style in rendered.style_refs() {
        root = root.with_style(StyleRef::new(style));
    }
    for asset in rendered.asset_refs() {
        root = root.with_asset(AssetRef::new(asset.name(), asset.path()));
    }
    if source_text.contains("onPointerDown") {
        root = root.with_event(
            EventKind::Pointer(PointerEventKind::Press),
            "handlePress",
            [EventPayloadField::Position],
        );
    }
    if source_text.contains("onMount") {
        root = root.with_lifecycle(NativeLifecycleEvent::Mounted, "onMount");
    }
    if source_text.contains("onUnmount") {
        root = root.with_lifecycle(NativeLifecycleEvent::Unmounted, "onUnmount");
    }
    let font_size = extract_number_attribute(source_text, "data-font-size").unwrap_or(18.0);
    for child_id in rendered.keyed_children() {
        root = root.with_child(NativeChild::keyed(
            child_id,
            NativeAuthoringElement::new(child_id, ElementKind::Text)
                .with_prop(
                    "text",
                    PropValue::String(
                        static_hawk_text_content(source_text, child_id)
                            .unwrap_or_else(|| child_id.clone()),
                    ),
                )
                .with_prop("font_size", PropValue::Number(font_size))
                .with_prop("color", PropValue::String("#ffffff".to_string()))
                .with_prop("width", PropValue::Number(160.0))
                .with_prop("height", PropValue::Number(32.0)),
        ));
    }
    runtime.mount(root);
    runtime.finish().map_err(|error| ReactRenderError {
        diagnostics: error.diagnostics().to_vec(),
        source_map: ReactSourceMap {
            author_file: author_file.to_string(),
        },
    })
}

fn react_artifact_from_native_program(
    source_map: ReactSourceMap,
    native_program: FrameworkNativeProgram,
) -> Result<ReactRenderedArtifact, ReactRenderError> {
    let events = framework_program_events(&native_program);
    let reconciler_operations = native_program
        .custom_renderer_operation_keys("react")
        .map_err(|error| custom_renderer_error(source_map.author_file(), &error))?
        .iter()
        .map(|key| react_public_operation_key(key))
        .collect();
    Ok(ReactRenderedArtifact {
        root: ElementNode::new(
            native_program.root().id().clone(),
            native_program.root().kind(),
        ),
        keyed_children: native_program.keyed_child_order(),
        refs: native_program
            .root()
            .refs()
            .iter()
            .map(|reference| reference.name().to_string())
            .collect(),
        style_refs: native_program.root().style_refs().to_vec(),
        asset_refs: native_program.root().asset_refs().to_vec(),
        events,
        lifecycle_handlers: native_program
            .root()
            .lifecycle()
            .iter()
            .map(|(event, handler)| lifecycle_handler_label(*event, handler.as_str()))
            .collect(),
        native_program: Some(native_program),
        source_map,
        reconciler_operations,
    })
}

fn framework_program_events(native_program: &FrameworkNativeProgram) -> Vec<EventBinding> {
    let mut events = native_program.root().events().to_vec();
    events.extend(
        native_program
            .root()
            .lifecycle()
            .iter()
            .map(|(event, handler)| {
                EventBinding::new(
                    native_program.root().id().clone(),
                    lifecycle_event_kind(*event),
                    HandlerRef::new(handler.as_str()),
                )
            }),
    );
    events
}

fn lifecycle_event_kind(event: NativeLifecycleEvent) -> EventKind {
    EventKind::Lifecycle(match event {
        NativeLifecycleEvent::Mounted => LifecycleEventKind::Mounted,
        NativeLifecycleEvent::Suspended => LifecycleEventKind::Suspended,
        NativeLifecycleEvent::Resumed => LifecycleEventKind::Resumed,
        NativeLifecycleEvent::HotReloaded => LifecycleEventKind::HotReloaded,
        NativeLifecycleEvent::ErrorBoundary => LifecycleEventKind::ErrorBoundary,
        NativeLifecycleEvent::Shutdown => LifecycleEventKind::Shutdown,
        NativeLifecycleEvent::Unmounted => LifecycleEventKind::Unmounted,
    })
}

fn bridge_error(author_file: &str, error: &NativeRuntimeBridgeError) -> ReactRenderError {
    ReactRenderError {
        diagnostics: vec![AuthoringDiagnostic::new(
            AuthoringDiagnosticSeverity::Error,
            "react.runtime-bridge.failed",
            error.message().to_string(),
        )],
        source_map: ReactSourceMap {
            author_file: author_file.to_string(),
        },
    }
}

fn lifecycle_handler_label(event: NativeLifecycleEvent, handler: &str) -> String {
    match event {
        NativeLifecycleEvent::Mounted => format!("mounted:{handler}"),
        NativeLifecycleEvent::Suspended => format!("suspended:{handler}"),
        NativeLifecycleEvent::Resumed => format!("resumed:{handler}"),
        NativeLifecycleEvent::HotReloaded => format!("hot-reloaded:{handler}"),
        NativeLifecycleEvent::ErrorBoundary => format!("error-boundary:{handler}"),
        NativeLifecycleEvent::Shutdown => format!("shutdown:{handler}"),
        NativeLifecycleEvent::Unmounted => format!("unmounted:{handler}"),
    }
}

#[derive(Clone, Copy)]
struct ReactProtocolInput<'a> {
    author_file: &'a str,
    source_text: &'a str,
    root_id: &'a str,
    refs: &'a [String],
    style_refs: &'a [StyleRef],
    asset_refs: &'a [AssetRef],
    events: &'a [EventBinding],
    keyed_children: &'a [String],
}

fn react_protocol_operations(
    input: ReactProtocolInput<'_>,
) -> Result<Vec<String>, ReactRenderError> {
    let root_element_id = ElementId::new(input.root_id);
    let mut protocol = CustomRendererProtocol::new("react");
    apply_react_protocol_operation(
        input.author_file,
        &mut protocol,
        CustomRendererOperation::CreateNode {
            id: root_element_id.clone(),
            kind: ElementKind::View,
        },
    )?;
    for style_ref in input.style_refs {
        apply_react_protocol_operation(
            input.author_file,
            &mut protocol,
            CustomRendererOperation::SetStyleRef {
                id: root_element_id.clone(),
                style_ref: StyleRef::new(style_ref.name()),
            },
        )?;
    }
    for asset_ref in input.asset_refs {
        apply_react_protocol_operation(
            input.author_file,
            &mut protocol,
            CustomRendererOperation::SetAssetRef {
                id: root_element_id.clone(),
                asset_ref: AssetRef::new(asset_ref.name(), asset_ref.path()),
            },
        )?;
    }
    for reference in input.refs {
        apply_react_protocol_operation(
            input.author_file,
            &mut protocol,
            CustomRendererOperation::SetRef {
                id: root_element_id.clone(),
                reference: NativeRef::new(reference),
            },
        )?;
    }
    for event in input.events {
        if !matches!(event.event(), EventKind::Lifecycle(_)) {
            apply_react_protocol_operation(
                input.author_file,
                &mut protocol,
                CustomRendererOperation::BindEvent {
                    binding: event.clone(),
                },
            )?;
        }
    }
    apply_react_lifecycle_protocol_operations(
        input.author_file,
        input.source_text,
        input.root_id,
        &mut protocol,
    )?;
    for child in input.keyed_children {
        let child_id = ElementId::new(child);
        apply_react_protocol_operation(
            input.author_file,
            &mut protocol,
            CustomRendererOperation::CreateNode {
                id: child_id.clone(),
                kind: ElementKind::Text,
            },
        )?;
        apply_react_protocol_operation(
            input.author_file,
            &mut protocol,
            CustomRendererOperation::AppendChild {
                parent: root_element_id.clone(),
                child: child_id,
                key: Some(child.clone()),
            },
        )?;
    }
    apply_react_protocol_operation(
        input.author_file,
        &mut protocol,
        CustomRendererOperation::Commit {
            root: root_element_id,
        },
    )?;
    Ok(protocol
        .operation_keys()
        .iter()
        .map(|key| react_public_operation_key(key))
        .collect())
}

fn apply_react_lifecycle_protocol_operations(
    author_file: &str,
    source_text: &str,
    root_id: &str,
    protocol: &mut CustomRendererProtocol,
) -> Result<(), ReactRenderError> {
    if source_text.contains("onMount") {
        apply_react_protocol_operation(
            author_file,
            protocol,
            CustomRendererOperation::BindLifecycle {
                id: ElementId::new(root_id),
                event: NativeLifecycleEvent::Mounted,
                handler: HandlerRef::new("onMount"),
            },
        )?;
    }
    if source_text.contains("onUnmount") {
        apply_react_protocol_operation(
            author_file,
            protocol,
            CustomRendererOperation::BindLifecycle {
                id: ElementId::new(root_id),
                event: NativeLifecycleEvent::Unmounted,
                handler: HandlerRef::new("onUnmount"),
            },
        )?;
    }
    Ok(())
}

fn apply_react_protocol_operation(
    author_file: &str,
    protocol: &mut CustomRendererProtocol,
    operation: CustomRendererOperation,
) -> Result<(), ReactRenderError> {
    protocol
        .apply(operation)
        .map_err(|error| custom_renderer_error(author_file, &error))
}

/// Rewrites the authoring layer's internal `append-child:{parent}:{child}[:key:{key}]` operation
/// key into the public `append:{child}` form.
///
/// This is parent-agnostic: it does not assume the parent is `"root"`, so a custom root id
/// (`id="app"`) or a nested `from_native_program` tree (whose grandchildren are parented at a
/// non-root id) no longer leaks the raw internal key shape into `reconciler_operations()`.
fn react_public_operation_key(key: &str) -> String {
    let Some(rest) = key.strip_prefix("append-child:") else {
        return key.to_string();
    };
    let descriptor = rest.split(":key:").next().unwrap_or(rest);
    descriptor
        .rsplit(':')
        .next()
        .map_or_else(|| key.to_string(), |child| format!("append:{child}"))
}

fn custom_renderer_error(
    author_file: &str,
    error: &hawk2ui_authoring::CustomRendererError,
) -> ReactRenderError {
    ReactRenderError {
        diagnostics: vec![AuthoringDiagnostic::new(
            AuthoringDiagnosticSeverity::Error,
            "react.custom-renderer.failed",
            format!("{}: {}", error.rule(), error.message()),
        )],
        source_map: ReactSourceMap {
            author_file: author_file.to_string(),
        },
    }
}

fn extract_attribute(source: &str, name: &str) -> Option<String> {
    let pattern = format!("{name}=\"");
    let start = source.find(&pattern)? + pattern.len();
    let rest = &source[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn extract_number_attribute(source: &str, name: &str) -> Option<f64> {
    extract_attribute(source, name)?.parse().ok()
}

fn style_refs_from_attribute(source: &str, name: &str) -> Vec<StyleRef> {
    extract_attribute(source, name).map_or_else(Vec::new, |classes| {
        classes
            .split_ascii_whitespace()
            .map(StyleRef::new)
            .collect()
    })
}

fn unsupported_react_events(source: &str) -> Vec<String> {
    ["onClick", "onHover", "onMouseEnter", "onKeyDown"]
        .into_iter()
        .filter(|event| source.contains(event))
        .map(str::to_string)
        .collect()
}

fn keyed_children(source: &str) -> Vec<String> {
    let mut ids = if source.contains("key={item.id}") {
        declared_item_ids(source)
    } else {
        Vec::new()
    };
    for id in static_hawk_text_ids(source) {
        push_unique(&mut ids, id);
    }
    ids
}

fn static_hawk_text_ids(source: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut rest = source;
    while let Some(index) = rest.find("<hawk-text") {
        let after = &rest[index..];
        let segment = after.split('>').next().unwrap_or(after);
        if let Some(id) = extract_attribute(segment, "id") {
            push_unique(&mut ids, id);
        }
        rest = &after["<hawk-text".len()..];
    }
    ids
}

fn static_hawk_text_content(source: &str, child_id: &str) -> Option<String> {
    let mut rest = source;
    while let Some(index) = rest.find("<hawk-text") {
        let after = &rest[index..];
        let tag_end = after.find('>')?;
        let tag = &after[..tag_end];
        if extract_attribute(tag, "id").as_deref() == Some(child_id) {
            let body = &after[tag_end + 1..];
            let end = body.find("</hawk-text>")?;
            let text = body[..end].trim();
            return (!text.is_empty()).then(|| text.to_string());
        }
        rest = &after["<hawk-text".len()..];
    }
    None
}

fn declared_item_ids(source: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut rest = source;
    while let Some(index) = rest.find("id:") {
        let after = rest[index + "id:".len()..].trim_start();
        let Some(quote) = after
            .chars()
            .next()
            .filter(|quote| matches!(quote, '\'' | '"'))
        else {
            rest = after;
            continue;
        };
        let value = &after[quote.len_utf8()..];
        let Some(end) = value.find(quote) else {
            break;
        };
        let id = &value[..end];
        push_unique(&mut ids, id.to_string());
        rest = &value[end + quote.len_utf8()..];
    }
    ids
}

fn push_unique(ids: &mut Vec<String>, id: String) {
    if !id.is_empty() && !ids.iter().any(|existing| existing == &id) {
        ids.push(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-framework-react");
    }
}
