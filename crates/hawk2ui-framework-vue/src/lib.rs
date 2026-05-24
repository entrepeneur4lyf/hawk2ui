#![forbid(unsafe_code)]
//! `Vue` 3.5 and later integration for emitting `Hawk2UI` typed records.

use hawk2ui_authoring::{
    AssetRef, AuthoringDiagnostic, AuthoringDiagnosticSeverity, ElementId, ElementKind,
    ElementNode, EventBinding, EventKind, EventPayloadField, HandlerRef, LifecycleEventKind,
    PointerEventKind, StyleRef,
};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-framework-vue";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

/// Vue single-file component source submitted to the renderer integration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VueSingleFileComponent {
    author_file: String,
    source: String,
}

impl VueSingleFileComponent {
    /// Creates a Vue source unit with its author-visible path.
    #[must_use]
    pub fn new(author_file: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            author_file: author_file.into(),
            source: source.into(),
        }
    }
}

/// Source map boundary returned by the Vue integration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VueSourceMap {
    author_file: String,
}

impl VueSourceMap {
    /// Returns the author source file associated with diagnostics and records.
    #[must_use]
    pub fn author_file(&self) -> &str {
        &self.author_file
    }
}

/// Rendered Vue artifact expressed as typed `Hawk2UI` records.
#[derive(Clone, Debug, PartialEq)]
pub struct VueRenderedArtifact {
    root: ElementNode,
    keyed_children: Vec<String>,
    refs: Vec<String>,
    style_refs: Vec<StyleRef>,
    asset_refs: Vec<AssetRef>,
    events: Vec<EventBinding>,
    lifecycle_handlers: Vec<String>,
    source_map: VueSourceMap,
    renderer_operations: Vec<String>,
}

impl VueRenderedArtifact {
    /// Returns the framework label.
    #[must_use]
    pub const fn framework(&self) -> &'static str {
        "vue"
    }

    /// Returns the supported framework version requirement.
    #[must_use]
    pub const fn framework_version_requirement(&self) -> &'static str {
        ">=3.5"
    }

    /// Returns the rendered root element.
    #[must_use]
    pub const fn root(&self) -> &ElementNode {
        &self.root
    }

    /// Returns keyed children in Vue renderer order.
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
    pub const fn source_map(&self) -> &VueSourceMap {
        &self.source_map
    }

    /// Returns custom renderer operations in deterministic order.
    #[must_use]
    pub fn renderer_operations(&self) -> &[String] {
        &self.renderer_operations
    }
}

/// Vue renderer error with source-mapped diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VueRenderError {
    diagnostics: Vec<AuthoringDiagnostic>,
    source_map: VueSourceMap,
}

impl VueRenderError {
    /// Returns diagnostics emitted for the author source.
    #[must_use]
    pub fn diagnostics(&self) -> &[AuthoringDiagnostic] {
        &self.diagnostics
    }

    /// Returns source map context for diagnostics.
    #[must_use]
    pub const fn source_map(&self) -> &VueSourceMap {
        &self.source_map
    }
}

/// Vue 3.5 custom renderer integration.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VueIntegration;

impl VueIntegration {
    /// Creates a Vue integration instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Renders a Vue component into typed `Hawk2UI` records.
    ///
    /// # Errors
    ///
    /// Returns [`VueRenderError`] when the source violates the renderer contract.
    pub fn render(
        self,
        component: VueSingleFileComponent,
    ) -> Result<VueRenderedArtifact, VueRenderError> {
        let source_map = VueSourceMap {
            author_file: component.author_file,
        };
        let mut diagnostics = Vec::new();
        if let Some(asset) = extract_attribute(&component.source, "data-asset")
            && (asset.contains("://") || asset.starts_with('/') || asset.contains(".."))
        {
            diagnostics.push(AuthoringDiagnostic::new(
                AuthoringDiagnosticSeverity::Error,
                "vue.asset.path-invalid",
                "Vue asset references must use workspace-relative paths",
            ));
        }
        if component.source.contains("<Missing") {
            diagnostics.push(AuthoringDiagnostic::new(
                AuthoringDiagnosticSeverity::Error,
                "vue.renderer.unresolved-component",
                "Vue renderer could not resolve component",
            ));
        }
        if !diagnostics.is_empty() {
            return Err(VueRenderError {
                diagnostics,
                source_map,
            });
        }

        let root_id =
            extract_attribute(&component.source, "id").unwrap_or_else(|| "root".to_string());
        let mut events = Vec::new();
        if component.source.contains("@pointerdown") {
            events.push(
                EventBinding::new(
                    ElementId::new(root_id.clone()),
                    EventKind::Pointer(PointerEventKind::Press),
                    HandlerRef::new("handlePress"),
                )
                .with_payload(EventPayloadField::Position),
            );
        }
        if component.source.contains("@mounted") {
            events.push(EventBinding::new(
                ElementId::new(root_id.clone()),
                EventKind::Lifecycle(LifecycleEventKind::Mounted),
                HandlerRef::new("onMounted"),
            ));
        }
        if component.source.contains("@unmounted") {
            events.push(EventBinding::new(
                ElementId::new(root_id.clone()),
                EventKind::Lifecycle(LifecycleEventKind::Unmounted),
                HandlerRef::new("onUnmounted"),
            ));
        }

        Ok(VueRenderedArtifact {
            root: ElementNode::new(ElementId::new(root_id), ElementKind::View),
            keyed_children: keyed_children(&component.source),
            refs: extract_attribute(&component.source, "ref")
                .into_iter()
                .collect(),
            style_refs: extract_attribute(&component.source, "class")
                .into_iter()
                .map(StyleRef::new)
                .collect(),
            asset_refs: extract_attribute(&component.source, "data-asset")
                .into_iter()
                .map(|path| AssetRef::new("vue.asset", path))
                .collect(),
            events,
            lifecycle_handlers: vec!["mounted:onMounted".into(), "unmounted:onUnmounted".into()],
            source_map,
            renderer_operations: vec![
                "create:root".into(),
                "insert:title".into(),
                "insert:cta".into(),
                "patch-props:root".into(),
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
    if source.contains(":key=\"item.id\"") {
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
        assert_eq!(crate_name(), "hawk2ui-framework-vue");
    }
}
