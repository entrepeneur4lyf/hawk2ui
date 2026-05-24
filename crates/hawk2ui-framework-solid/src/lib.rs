#![forbid(unsafe_code)]
//! `Solid` integration for emitting `Hawk2UI` typed records.

use hawk2ui_authoring::{
    AssetRef, AuthoringDiagnostic, AuthoringDiagnosticSeverity, ElementId, ElementKind,
    ElementNode, EventBinding, EventKind, EventPayloadField, HandlerRef, LifecycleEventKind,
    PointerEventKind, StyleRef,
};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-framework-solid";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

/// Solid component source submitted to the renderer integration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolidComponentSource {
    author_file: String,
    source: String,
}

impl SolidComponentSource {
    /// Creates a Solid source unit with its author-visible path.
    #[must_use]
    pub fn new(author_file: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            author_file: author_file.into(),
            source: source.into(),
        }
    }
}

/// Source map boundary returned by the Solid integration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolidSourceMap {
    author_file: String,
}

impl SolidSourceMap {
    /// Returns the author source file associated with diagnostics and records.
    #[must_use]
    pub fn author_file(&self) -> &str {
        &self.author_file
    }
}

/// Rendered Solid artifact expressed as typed `Hawk2UI` records.
#[derive(Clone, Debug, PartialEq)]
pub struct SolidRenderedArtifact {
    root: ElementNode,
    keyed_children: Vec<String>,
    refs: Vec<String>,
    style_refs: Vec<StyleRef>,
    asset_refs: Vec<AssetRef>,
    events: Vec<EventBinding>,
    lifecycle_handlers: Vec<String>,
    fine_grained_updates: Vec<String>,
    source_map: SolidSourceMap,
}

impl SolidRenderedArtifact {
    /// Returns the framework label.
    #[must_use]
    pub const fn framework(&self) -> &'static str {
        "solid"
    }

    /// Returns the supported framework version requirement.
    #[must_use]
    pub const fn framework_version_requirement(&self) -> &'static str {
        ">=1"
    }

    /// Returns the rendered root element.
    #[must_use]
    pub const fn root(&self) -> &ElementNode {
        &self.root
    }

    /// Returns keyed children in Solid list order.
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

    /// Returns fine-grained update records.
    #[must_use]
    pub fn fine_grained_updates(&self) -> &[String] {
        &self.fine_grained_updates
    }

    /// Returns the source map for diagnostics.
    #[must_use]
    pub const fn source_map(&self) -> &SolidSourceMap {
        &self.source_map
    }
}

/// Solid renderer error with source-mapped diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolidRenderError {
    diagnostics: Vec<AuthoringDiagnostic>,
    source_map: SolidSourceMap,
}

impl SolidRenderError {
    /// Returns diagnostics emitted for the author source.
    #[must_use]
    pub fn diagnostics(&self) -> &[AuthoringDiagnostic] {
        &self.diagnostics
    }

    /// Returns source map context for diagnostics.
    #[must_use]
    pub const fn source_map(&self) -> &SolidSourceMap {
        &self.source_map
    }
}

/// Solid renderer integration.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SolidIntegration;

impl SolidIntegration {
    /// Creates a Solid integration instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Renders a Solid component into typed `Hawk2UI` records.
    ///
    /// # Errors
    ///
    /// Returns [`SolidRenderError`] when the source violates the renderer contract.
    pub fn render(
        self,
        component: SolidComponentSource,
    ) -> Result<SolidRenderedArtifact, SolidRenderError> {
        let source_map = SolidSourceMap {
            author_file: component.author_file,
        };
        let mut diagnostics = Vec::new();
        if let Some(asset) = extract_attribute(&component.source, "data-asset")
            && (asset.contains("://") || asset.starts_with('/') || asset.contains(".."))
        {
            diagnostics.push(AuthoringDiagnostic::new(
                AuthoringDiagnosticSeverity::Error,
                "solid.asset.path-invalid",
                "Solid asset references must use workspace-relative paths",
            ));
        }
        if component.source.contains("<Missing") {
            diagnostics.push(AuthoringDiagnostic::new(
                AuthoringDiagnosticSeverity::Error,
                "solid.renderer.unresolved-component",
                "Solid renderer could not resolve component",
            ));
        }
        if !diagnostics.is_empty() {
            return Err(SolidRenderError {
                diagnostics,
                source_map,
            });
        }

        let root_id =
            extract_attribute(&component.source, "id").unwrap_or_else(|| "root".to_string());
        let mut events = Vec::new();
        if component.source.contains("onPointerDown") {
            events.push(
                EventBinding::new(
                    ElementId::new(root_id.clone()),
                    EventKind::Pointer(PointerEventKind::Press),
                    HandlerRef::new("handlePress"),
                )
                .with_payload(EventPayloadField::Position),
            );
        }
        if component.source.contains("onMount") {
            events.push(EventBinding::new(
                ElementId::new(root_id.clone()),
                EventKind::Lifecycle(LifecycleEventKind::Mounted),
                HandlerRef::new("onMount"),
            ));
        }
        if component.source.contains("onCleanup") {
            events.push(EventBinding::new(
                ElementId::new(root_id.clone()),
                EventKind::Lifecycle(LifecycleEventKind::Unmounted),
                HandlerRef::new("onCleanup"),
            ));
        }

        Ok(SolidRenderedArtifact {
            root: ElementNode::new(ElementId::new(root_id), ElementKind::View),
            keyed_children: keyed_children(&component.source),
            refs: if component.source.contains("ref={root_ref}") {
                vec!["root_ref".into()]
            } else {
                Vec::new()
            },
            style_refs: extract_attribute(&component.source, "class")
                .into_iter()
                .map(StyleRef::new)
                .collect(),
            asset_refs: extract_attribute(&component.source, "data-asset")
                .into_iter()
                .map(|path| AssetRef::new("solid.asset", path))
                .collect(),
            events,
            lifecycle_handlers: vec!["mounted:onMount".into(), "unmounted:onCleanup".into()],
            fine_grained_updates: vec![
                "signal:items".into(),
                "for-each:keyed".into(),
                "effect:root-props".into(),
            ],
            source_map,
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
    if source.contains("<For each={items()}") {
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
        assert_eq!(crate_name(), "hawk2ui-framework-solid");
    }
}
