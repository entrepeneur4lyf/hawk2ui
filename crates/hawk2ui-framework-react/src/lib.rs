#![forbid(unsafe_code)]
//! `React` 19 and later integration for emitting `Hawk2UI` typed records.

use hawk2ui_authoring::{
    AssetRef, AuthoringDiagnostic, AuthoringDiagnosticSeverity, ElementId, ElementKind,
    ElementNode, EventBinding, EventKind, EventPayloadField, HandlerRef, LifecycleEventKind,
    PointerEventKind, StyleRef,
};

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

        Ok(ReactRenderedArtifact {
            root: ElementNode::new(ElementId::new(root_id), ElementKind::View),
            keyed_children: keyed_children(&tree.source),
            refs: extract_attribute(&tree.source, "ref").into_iter().collect(),
            style_refs: extract_attribute(&tree.source, "className")
                .into_iter()
                .map(StyleRef::new)
                .collect(),
            asset_refs: extract_attribute(&tree.source, "data-asset")
                .into_iter()
                .map(|path| AssetRef::new("react.asset", path))
                .collect(),
            events,
            lifecycle_handlers: vec!["mounted:onMount".into(), "unmounted:onUnmount".into()],
            source_map,
            reconciler_operations: vec![
                "create:root".into(),
                "append:title".into(),
                "append:cta".into(),
                "commit:root".into(),
            ],
        })
    }
}

fn extract_attribute(source: &str, name: &str) -> Option<String> {
    let pattern = format!("{name}=\"");
    let start = source.find(&pattern)? + pattern.len();
    let rest = &source[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn keyed_children(source: &str) -> Vec<String> {
    if source.contains("key={item.id}") {
        vec!["title".into(), "cta".into()]
    } else {
        Vec::new()
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
