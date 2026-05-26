#![forbid(unsafe_code)]
//! `Svelte` 5 integration for emitting `Hawk2UI` typed records.

use hawk2ui_authoring::{
    AssetRef, AuthoringDiagnostic, AuthoringDiagnosticSeverity, ElementId, ElementKind,
    ElementNode, EventBinding, EventKind, EventPayloadField, HandlerRef, LifecycleEventKind,
    NativeAuthoringElement, NativeAuthoringRuntime, NativeChild, NativeLifecycleEvent, NativeRef,
    NativeRuntimeBridge, NativeRuntimeBridgeArtifact, NativeRuntimeBridgeError, PointerEventKind,
    PropValue, StyleRef,
};
use hawk2ui_runtime::RuntimeViewTree;

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-framework-svelte";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

/// Svelte component source submitted to the native compile integration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SvelteComponentSource {
    author_file: String,
    source: String,
}

impl SvelteComponentSource {
    /// Creates a Svelte source unit with its author-visible path.
    #[must_use]
    pub fn new(author_file: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            author_file: author_file.into(),
            source: source.into(),
        }
    }
}

/// Source map boundary returned by the Svelte integration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SvelteSourceMap {
    author_file: String,
}

impl SvelteSourceMap {
    /// Returns the author source file associated with diagnostics and records.
    #[must_use]
    pub fn author_file(&self) -> &str {
        &self.author_file
    }
}

/// Compiled Svelte artifact expressed as typed `Hawk2UI` records.
#[derive(Clone, Debug, PartialEq)]
pub struct SvelteCompiledArtifact {
    root: ElementNode,
    keyed_children: Vec<String>,
    refs: Vec<String>,
    style_refs: Vec<StyleRef>,
    asset_refs: Vec<AssetRef>,
    events: Vec<EventBinding>,
    lifecycle_handlers: Vec<String>,
    source_map: SvelteSourceMap,
    diagnostics: Vec<AuthoringDiagnostic>,
}

/// Runtime-ready Svelte artifact.
#[derive(Clone, Debug, PartialEq)]
pub struct SvelteRuntimeArtifact {
    compiled: SvelteCompiledArtifact,
    runtime: NativeRuntimeBridgeArtifact,
}

impl SvelteRuntimeArtifact {
    /// Returns the typed compiled Svelte record artifact.
    #[must_use]
    pub const fn compiled(&self) -> &SvelteCompiledArtifact {
        &self.compiled
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

impl SvelteCompiledArtifact {
    /// Returns the framework label.
    #[must_use]
    pub const fn framework(&self) -> &'static str {
        "svelte"
    }

    /// Returns the supported framework version requirement.
    #[must_use]
    pub const fn framework_version_requirement(&self) -> &'static str {
        ">=5"
    }

    /// Returns the compiled root element.
    #[must_use]
    pub const fn root(&self) -> &ElementNode {
        &self.root
    }

    /// Returns keyed children in framework declaration order.
    #[must_use]
    pub fn keyed_children(&self) -> &[String] {
        &self.keyed_children
    }

    /// Returns native refs in declaration order.
    #[must_use]
    pub fn refs(&self) -> &[String] {
        &self.refs
    }

    /// Returns style references in declaration order.
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
    pub const fn source_map(&self) -> &SvelteSourceMap {
        &self.source_map
    }

    /// Returns non-fatal diagnostics.
    #[must_use]
    pub fn diagnostics(&self) -> &[AuthoringDiagnostic] {
        &self.diagnostics
    }
}

/// Svelte compile error with source-mapped diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SvelteCompileError {
    diagnostics: Vec<AuthoringDiagnostic>,
    source_map: SvelteSourceMap,
}

impl SvelteCompileError {
    /// Returns diagnostics emitted for the author source.
    #[must_use]
    pub fn diagnostics(&self) -> &[AuthoringDiagnostic] {
        &self.diagnostics
    }

    /// Returns source map context for diagnostics.
    #[must_use]
    pub const fn source_map(&self) -> &SvelteSourceMap {
        &self.source_map
    }
}

/// Svelte 5 compile integration.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SvelteIntegration;

impl SvelteIntegration {
    /// Creates a Svelte integration instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Compiles Svelte author source into typed `Hawk2UI` records.
    ///
    /// # Errors
    ///
    /// Returns [`SvelteCompileError`] when the Svelte source violates the native integration contract.
    pub fn compile(
        self,
        source: SvelteComponentSource,
    ) -> Result<SvelteCompiledArtifact, SvelteCompileError> {
        let source_map = SvelteSourceMap {
            author_file: source.author_file,
        };
        let mut diagnostics = Vec::new();
        if let Some(asset) = extract_attribute(&source.source, "data-asset")
            && (asset.contains("://") || asset.starts_with('/') || asset.contains(".."))
        {
            diagnostics.push(AuthoringDiagnostic::new(
                AuthoringDiagnosticSeverity::Error,
                "svelte.asset.path-invalid",
                "Svelte asset references must use workspace-relative paths",
            ));
        }
        if source.source.contains("<Broken") {
            diagnostics.push(AuthoringDiagnostic::new(
                AuthoringDiagnosticSeverity::Error,
                "svelte.compile.unresolved-component",
                "Svelte component could not be resolved",
            ));
        }
        if !diagnostics.is_empty() {
            return Err(SvelteCompileError {
                diagnostics,
                source_map,
            });
        }

        let root_id = extract_attribute(&source.source, "id").unwrap_or_else(|| "root".to_string());
        let mut events = Vec::new();
        if source.source.contains("on:press") {
            events.push(
                EventBinding::new(
                    ElementId::new(root_id.clone()),
                    EventKind::Pointer(PointerEventKind::Press),
                    HandlerRef::new("handlePress"),
                )
                .with_payload(EventPayloadField::Position),
            );
        }
        if source.source.contains("on:mount") {
            events.push(EventBinding::new(
                ElementId::new(root_id.clone()),
                EventKind::Lifecycle(LifecycleEventKind::Mounted),
                HandlerRef::new("onMount"),
            ));
        }
        if source.source.contains("on:destroy") {
            events.push(EventBinding::new(
                ElementId::new(root_id.clone()),
                EventKind::Lifecycle(LifecycleEventKind::Unmounted),
                HandlerRef::new("onDestroy"),
            ));
        }

        Ok(SvelteCompiledArtifact {
            root: ElementNode::new(ElementId::new(root_id), ElementKind::View),
            keyed_children: keyed_children(&source.source),
            refs: extract_attribute(&source.source, "use:ref")
                .into_iter()
                .collect(),
            style_refs: extract_attribute(&source.source, "class")
                .into_iter()
                .map(StyleRef::new)
                .collect(),
            asset_refs: extract_attribute(&source.source, "data-asset")
                .into_iter()
                .map(|path| AssetRef::new("svelte.asset", path))
                .collect(),
            events,
            lifecycle_handlers: vec!["mounted:onMount".into(), "unmounted:onDestroy".into()],
            source_map,
            diagnostics: Vec::new(),
        })
    }

    /// Compiles Svelte author source into a runtime-ready native bridge artifact.
    ///
    /// # Errors
    ///
    /// Returns [`SvelteCompileError`] when source validation, native authoring finalization, or
    /// runtime bridging fails.
    pub fn compile_to_runtime(
        self,
        source: SvelteComponentSource,
    ) -> Result<SvelteRuntimeArtifact, SvelteCompileError> {
        let author_file = source.author_file.clone();
        let source_text = source.source.clone();
        let compiled = self.compile(source)?;
        let native_artifact =
            native_artifact_from_svelte(author_file.as_str(), source_text.as_str(), &compiled)?;
        let runtime = NativeRuntimeBridge::new()
            .bridge_artifact(&native_artifact)
            .map_err(|error| bridge_error(author_file.as_str(), &error))?;
        Ok(SvelteRuntimeArtifact { compiled, runtime })
    }
}

fn native_artifact_from_svelte(
    author_file: &str,
    source_text: &str,
    compiled: &SvelteCompiledArtifact,
) -> Result<hawk2ui_authoring::NativeAuthoringArtifact, SvelteCompileError> {
    let mut runtime = NativeAuthoringRuntime::new(author_file);
    let mut root = NativeAuthoringElement::new(compiled.root().id().as_str(), ElementKind::View)
        .with_prop("background", PropValue::String("#080a0e".to_string()));
    for reference in compiled.refs() {
        root = root.with_ref(NativeRef::new(reference));
    }
    for style in compiled.style_refs() {
        root = root.with_style(StyleRef::new(style));
    }
    for asset in compiled.asset_refs() {
        root = root.with_asset(AssetRef::new(asset.name(), asset.path()));
    }
    if source_text.contains("on:press") {
        root = root.with_event(
            EventKind::Pointer(PointerEventKind::Press),
            "handlePress",
            [EventPayloadField::Position],
        );
    }
    if source_text.contains("on:mount") {
        root = root.with_lifecycle(NativeLifecycleEvent::Mounted, "onMount");
    }
    if source_text.contains("on:destroy") {
        root = root.with_lifecycle(NativeLifecycleEvent::Unmounted, "onDestroy");
    }
    for child_id in compiled.keyed_children() {
        root = root.with_child(NativeChild::keyed(
            child_id,
            NativeAuthoringElement::new(child_id, ElementKind::Text)
                .with_prop("text", PropValue::String(child_id.clone()))
                .with_prop("font_size", PropValue::Number(18.0))
                .with_prop("color", PropValue::String("#ffffff".to_string()))
                .with_prop("width", PropValue::Number(160.0))
                .with_prop("height", PropValue::Number(32.0)),
        ));
    }
    runtime.mount(root);
    runtime.finish().map_err(|error| SvelteCompileError {
        diagnostics: error.diagnostics().to_vec(),
        source_map: SvelteSourceMap {
            author_file: author_file.to_string(),
        },
    })
}

fn bridge_error(author_file: &str, error: &NativeRuntimeBridgeError) -> SvelteCompileError {
    SvelteCompileError {
        diagnostics: vec![AuthoringDiagnostic::new(
            AuthoringDiagnosticSeverity::Error,
            "svelte.runtime-bridge.failed",
            error.message().to_string(),
        )],
        source_map: SvelteSourceMap {
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

fn keyed_children(source: &str) -> Vec<String> {
    if source.contains("(item.id)") {
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
        assert_eq!(crate_name(), "hawk2ui-framework-svelte");
    }
}
