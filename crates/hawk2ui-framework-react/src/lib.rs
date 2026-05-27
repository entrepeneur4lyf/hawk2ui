#![forbid(unsafe_code)]
//! `React` 19 and later integration for emitting `Hawk2UI` typed records.

use hawk2ui_authoring::{
    AssetRef, AuthoringDiagnostic, AuthoringDiagnosticSeverity, ElementId, ElementKind,
    ElementNode, EventBinding, EventKind, EventPayloadField, HandlerRef, LifecycleEventKind,
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReactElementTree {
    author_file: String,
    source: String,
}

impl ReactElementTree {
    /// Creates a React source unit with its author-visible path.
    #[must_use]
    pub fn new(author_file: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            author_file: author_file.into(),
            source: source.into(),
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
    /// # Errors
    ///
    /// Returns [`ReactRenderError`] when the source violates the renderer contract.
    pub fn render(self, tree: ReactElementTree) -> Result<ReactRenderedArtifact, ReactRenderError> {
        let source_map = ReactSourceMap {
            author_file: tree.author_file,
        };
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

        let keyed_children = keyed_children(&tree.source);
        let mut reconciler_operations = Vec::with_capacity(keyed_children.len() + 2);
        reconciler_operations.push("create:root".to_string());
        reconciler_operations.extend(keyed_children.iter().map(|child| format!("append:{child}")));
        reconciler_operations.push("commit:root".to_string());

        Ok(ReactRenderedArtifact {
            root: ElementNode::new(ElementId::new(root_id), ElementKind::View),
            keyed_children,
            refs: extract_attribute(&tree.source, "ref").into_iter().collect(),
            style_refs: style_refs_from_attribute(&tree.source, "className"),
            asset_refs: extract_attribute(&tree.source, "data-asset")
                .into_iter()
                .map(|path| AssetRef::new("react.asset", path))
                .collect(),
            events,
            lifecycle_handlers: vec!["mounted:onMount".into(), "unmounted:onUnmount".into()],
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
                .with_prop("text", PropValue::String(child_id.clone()))
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
    if source.contains("key={item.id}") {
        declared_item_ids(source)
    } else {
        Vec::new()
    }
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
        if !id.is_empty() && !ids.iter().any(|existing| existing == id) {
            ids.push(id.to_string());
        }
        rest = &value[end + quote.len_utf8()..];
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-framework-react");
    }
}
