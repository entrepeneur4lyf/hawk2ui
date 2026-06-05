#![forbid(unsafe_code)]
//! `React` 19 and later integration for emitting `Hawk2UI` typed records.

use hawk2ui_authoring::{
    AdapterError, AssetRef, AuthoringDiagnostic, AuthoringDiagnosticSeverity, ElementNode,
    EventBinding, EventKind, FrameworkNativeProgram, FrameworkNativeProgramWire, HandlerRef,
    LifecycleEventKind, NativeLifecycleEvent, NativeRuntimeBridge, NativeRuntimeBridgeArtifact,
    NativeRuntimeBridgeError, StyleRef,
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

    /// Creates a React source unit from versioned compiler JSON.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError`] when the compiler JSON is malformed or fails native program
    /// validation.
    pub fn from_compiler_json(
        author_file: impl Into<String>,
        compiler_json: &str,
    ) -> Result<Self, AdapterError> {
        let native_program = FrameworkNativeProgram::try_from(
            FrameworkNativeProgramWire::from_json(compiler_json)?,
        )?;
        Ok(Self::from_native_program(author_file, native_program))
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
    /// Raw TSX source is rejected by this layer. Production rendering requires either
    /// [`ReactElementTree::from_native_program`] or [`ReactElementTree::from_compiler_json`], both of
    /// which carry explicit output from a React compiler/runtime adapter.
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
        Err(compiler_artifact_required_error(source_map))
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
        let rendered = self.render(tree)?;
        let native_artifact = native_artifact_from_react(author_file.as_str(), &rendered)?;
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
        let rendered = self.render(tree)?;
        let native_artifact =
            native_artifact_from_react_with_defaults(author_file.as_str(), &rendered, false)?;
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
        let rendered = self.render(tree)?;
        let native_artifact =
            native_artifact_from_react_with_defaults(author_file.as_str(), &rendered, false)?;
        let runtime = NativeRuntimeBridge::new()
            .bridge_artifact_with_theme(&native_artifact, sheet, tokens, theme)
            .map_err(|error| bridge_error(author_file.as_str(), &error))?;
        Ok(ReactRuntimeArtifact { rendered, runtime })
    }
}

fn native_artifact_from_react(
    author_file: &str,
    rendered: &ReactRenderedArtifact,
) -> Result<hawk2ui_authoring::NativeAuthoringArtifact, ReactRenderError> {
    native_artifact_from_react_with_defaults(author_file, rendered, true)
}

fn native_artifact_from_react_with_defaults(
    author_file: &str,
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
    Err(compiler_artifact_required_error(ReactSourceMap {
        author_file: author_file.to_string(),
    }))
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

fn compiler_artifact_required_error(source_map: ReactSourceMap) -> ReactRenderError {
    ReactRenderError {
        diagnostics: vec![AuthoringDiagnostic::new(
            AuthoringDiagnosticSeverity::Error,
            "react.compiler-artifact.required",
            "React source must be compiled by the React compiler adapter before entering the Rust renderer",
        )],
        source_map,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-framework-react");
    }
}
