#![forbid(unsafe_code)]
//! `Vue` 3.5 and later integration for emitting `Hawk2UI` typed records.

use hawk2ui_authoring::{
    AdapterError, AssetRef, AuthoringDiagnostic, AuthoringDiagnosticSeverity, ElementNode,
    EventBinding, EventKind, FrameworkDynamicBinding, FrameworkNativeProgram,
    FrameworkNativeProgramWire, HandlerRef, LifecycleEventKind, NativeLifecycleEvent,
    NativeRuntimeBridge, NativeRuntimeBridgeArtifact, NativeRuntimeBridgeError, StyleRef,
};
use hawk2ui_runtime::RuntimeViewTree;
use hawk2ui_style::{CompiledStyleSheet, TokenSet};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-framework-vue";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

/// Vue single-file component source submitted to the renderer integration.
#[derive(Clone, Debug, PartialEq)]
pub struct VueSingleFileComponent {
    author_file: String,
    source: String,
    native_program: Option<FrameworkNativeProgram>,
}

impl VueSingleFileComponent {
    /// Creates a Vue source unit with its author-visible path.
    #[must_use]
    pub fn new(author_file: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            author_file: author_file.into(),
            source: source.into(),
            native_program: None,
        }
    }

    /// Creates a Vue source unit from explicit native compiler output.
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

    /// Creates a Vue source unit from a versioned native compiler JSON artifact.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError`] when the JSON wire artifact is malformed, unsupported, or violates
    /// the native authoring contract.
    pub fn from_compiler_json(
        author_file: impl Into<String>,
        compiler_json: &str,
    ) -> Result<Self, AdapterError> {
        let wire = FrameworkNativeProgramWire::from_json(compiler_json)?;
        let native_program = FrameworkNativeProgram::try_from(wire)?;
        Ok(Self::from_native_program(author_file, native_program))
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
    native_program: Option<FrameworkNativeProgram>,
    source_map: VueSourceMap,
    renderer_operations: Vec<String>,
}

/// Runtime-ready Vue artifact.
#[derive(Clone, Debug, PartialEq)]
pub struct VueRuntimeArtifact {
    rendered: VueRenderedArtifact,
    runtime: NativeRuntimeBridgeArtifact,
}

impl VueRuntimeArtifact {
    /// Returns the typed rendered Vue artifact.
    #[must_use]
    pub const fn rendered(&self) -> &VueRenderedArtifact {
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

    /// Returns runtime dynamic bindings in compiler declaration order.
    #[must_use]
    pub fn dynamic_bindings(&self) -> &[FrameworkDynamicBinding] {
        self.runtime.dynamic_bindings()
    }
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

    /// Renders typed Vue compiler output into typed `Hawk2UI` records.
    ///
    /// Raw `.vue` source is not accepted here. Production use must enter through
    /// [`VueSingleFileComponent::from_native_program`] or
    /// [`VueSingleFileComponent::from_compiler_json`] after the Vue compiler adapter has produced
    /// the native program artifact.
    ///
    /// # Errors
    ///
    /// Returns [`VueRenderError`] when the source violates the renderer contract.
    pub fn render(
        self,
        component: VueSingleFileComponent,
    ) -> Result<VueRenderedArtifact, VueRenderError> {
        let source_map = VueSourceMap {
            author_file: component.author_file.clone(),
        };
        match component.native_program {
            Some(native_program) => vue_artifact_from_native_program(source_map, native_program),
            None => Err(compiler_artifact_required_error(source_map)),
        }
    }

    /// Renders a Vue component into a runtime-ready native bridge artifact.
    ///
    /// # Errors
    ///
    /// Returns [`VueRenderError`] when source validation, native authoring finalization, or runtime
    /// bridging fails.
    pub fn render_to_runtime(
        self,
        component: VueSingleFileComponent,
    ) -> Result<VueRuntimeArtifact, VueRenderError> {
        let author_file = component.author_file.clone();
        let rendered = self.render(component)?;
        let native_artifact = native_artifact_from_vue(author_file.as_str(), &rendered)?;
        let runtime = NativeRuntimeBridge::new()
            .bridge_artifact(&native_artifact)
            .map_err(|error| bridge_error(author_file.as_str(), &error))?;
        Ok(VueRuntimeArtifact { rendered, runtime })
    }

    /// Renders a Vue component into a runtime artifact with compiled style references applied.
    ///
    /// # Errors
    ///
    /// Returns [`VueRenderError`] when source validation, native authoring finalization, style
    /// resolution, or runtime bridging fails.
    pub fn render_to_runtime_with_styles(
        self,
        component: VueSingleFileComponent,
        sheet: &CompiledStyleSheet,
        tokens: &TokenSet,
    ) -> Result<VueRuntimeArtifact, VueRenderError> {
        let author_file = component.author_file.clone();
        let rendered = self.render(component)?;
        let native_artifact =
            native_artifact_from_vue_with_defaults(author_file.as_str(), &rendered, false)?;
        let runtime = NativeRuntimeBridge::new()
            .bridge_artifact_with_styles(&native_artifact, sheet, tokens)
            .map_err(|error| bridge_error(author_file.as_str(), &error))?;
        Ok(VueRuntimeArtifact { rendered, runtime })
    }

    /// Renders a Vue component into a themed runtime artifact.
    ///
    /// # Errors
    ///
    /// Returns [`VueRenderError`] when source validation, native authoring finalization, theme
    /// token resolution, style resolution, or runtime bridging fails.
    pub fn render_to_runtime_with_theme(
        self,
        component: VueSingleFileComponent,
        sheet: &CompiledStyleSheet,
        tokens: &TokenSet,
        theme: &str,
    ) -> Result<VueRuntimeArtifact, VueRenderError> {
        let author_file = component.author_file.clone();
        let rendered = self.render(component)?;
        let native_artifact =
            native_artifact_from_vue_with_defaults(author_file.as_str(), &rendered, false)?;
        let runtime = NativeRuntimeBridge::new()
            .bridge_artifact_with_theme(&native_artifact, sheet, tokens, theme)
            .map_err(|error| bridge_error(author_file.as_str(), &error))?;
        Ok(VueRuntimeArtifact { rendered, runtime })
    }
}

fn compiler_artifact_required_error(source_map: VueSourceMap) -> VueRenderError {
    VueRenderError {
        diagnostics: vec![AuthoringDiagnostic::new(
            AuthoringDiagnosticSeverity::Error,
            "vue.compiler-artifact.required",
            "Vue source must be compiled by the Vue compiler adapter before entering the Rust renderer",
        )],
        source_map,
    }
}

fn native_artifact_from_vue(
    author_file: &str,
    rendered: &VueRenderedArtifact,
) -> Result<hawk2ui_authoring::NativeAuthoringArtifact, VueRenderError> {
    native_artifact_from_vue_with_defaults(author_file, rendered, true)
}

fn native_artifact_from_vue_with_defaults(
    author_file: &str,
    rendered: &VueRenderedArtifact,
    include_default_visual_props: bool,
) -> Result<hawk2ui_authoring::NativeAuthoringArtifact, VueRenderError> {
    if let Some(native_program) = &rendered.native_program {
        return native_program
            .to_native_authoring_artifact(author_file, include_default_visual_props)
            .map_err(|error| VueRenderError {
                diagnostics: error.diagnostics().to_vec(),
                source_map: VueSourceMap {
                    author_file: author_file.to_string(),
                },
            });
    }
    Err(compiler_artifact_required_error(VueSourceMap {
        author_file: author_file.to_string(),
    }))
}

fn vue_artifact_from_native_program(
    source_map: VueSourceMap,
    native_program: FrameworkNativeProgram,
) -> Result<VueRenderedArtifact, VueRenderError> {
    let events = framework_program_events(&native_program);
    let renderer_operations = native_program
        .custom_renderer_operation_keys("vue")
        .map_err(|error| custom_renderer_error(source_map.author_file(), &error))?;
    Ok(VueRenderedArtifact {
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
        renderer_operations,
        native_program: Some(native_program),
        source_map,
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

fn bridge_error(author_file: &str, error: &NativeRuntimeBridgeError) -> VueRenderError {
    VueRenderError {
        diagnostics: vec![AuthoringDiagnostic::new(
            AuthoringDiagnosticSeverity::Error,
            "vue.runtime-bridge.failed",
            error.message().to_string(),
        )],
        source_map: VueSourceMap {
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

fn custom_renderer_error(
    author_file: &str,
    error: &hawk2ui_authoring::CustomRendererError,
) -> VueRenderError {
    VueRenderError {
        diagnostics: vec![AuthoringDiagnostic::new(
            AuthoringDiagnosticSeverity::Error,
            "vue.custom-renderer.failed",
            format!("{}: {}", error.rule(), error.message()),
        )],
        source_map: VueSourceMap {
            author_file: author_file.to_string(),
        },
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
