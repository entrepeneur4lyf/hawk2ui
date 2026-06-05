#![forbid(unsafe_code)]
//! `Svelte` 5 integration for emitting `Hawk2UI` typed records.

use hawk2ui_authoring::{
    AdapterError, AssetRef, AuthoringDiagnostic, AuthoringDiagnosticSeverity, ElementNode,
    EventBinding, EventKind, FrameworkDynamicBinding, FrameworkNativeProgram,
    FrameworkNativeProgramWire, HandlerRef, LifecycleEventKind, NativeLifecycleEvent,
    NativeRuntimeBridge, NativeRuntimeBridgeArtifact, NativeRuntimeBridgeError, StyleRef,
};
use hawk2ui_runtime::RuntimeViewTree;
use hawk2ui_style::{CompiledStyleSheet, TokenSet};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-framework-svelte";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

/// Svelte component source submitted to the native compile integration.
#[derive(Clone, Debug, PartialEq)]
pub struct SvelteComponentSource {
    author_file: String,
    source: String,
    native_program: Option<FrameworkNativeProgram>,
}

impl SvelteComponentSource {
    /// Creates a Svelte source unit with its author-visible path.
    #[must_use]
    pub fn new(author_file: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            author_file: author_file.into(),
            source: source.into(),
            native_program: None,
        }
    }

    /// Creates a Svelte source unit from explicit native compiler output.
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

    /// Creates a Svelte source unit from a versioned native compiler JSON artifact.
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
    renderer_operations: Vec<String>,
    native_program: Option<FrameworkNativeProgram>,
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

    /// Returns runtime dynamic bindings in compiler declaration order.
    #[must_use]
    pub fn dynamic_bindings(&self) -> &[FrameworkDynamicBinding] {
        self.runtime.dynamic_bindings()
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

    /// Returns custom renderer operation keys in deterministic order.
    #[must_use]
    pub fn renderer_operations(&self) -> &[String] {
        &self.renderer_operations
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

    /// Compiles typed Svelte compiler output into typed `Hawk2UI` records.
    ///
    /// Raw `.svelte` source is not accepted here. Production use must enter through
    /// [`SvelteComponentSource::from_native_program`] or
    /// [`SvelteComponentSource::from_compiler_json`] after the Svelte compiler adapter has produced
    /// the native program artifact.
    ///
    /// # Errors
    ///
    /// Returns [`SvelteCompileError`] when the Svelte source violates the native integration contract.
    pub fn compile(
        self,
        source: SvelteComponentSource,
    ) -> Result<SvelteCompiledArtifact, SvelteCompileError> {
        let source_map = SvelteSourceMap {
            author_file: source.author_file.clone(),
        };
        match source.native_program {
            Some(native_program) => svelte_artifact_from_native_program(source_map, native_program),
            None => Err(compiler_artifact_required_error(source_map)),
        }
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
        let compiled = self.compile(source)?;
        let native_artifact = native_artifact_from_svelte(author_file.as_str(), &compiled)?;
        let runtime = NativeRuntimeBridge::new()
            .bridge_artifact(&native_artifact)
            .map_err(|error| bridge_error(author_file.as_str(), &error))?;
        Ok(SvelteRuntimeArtifact { compiled, runtime })
    }

    /// Compiles Svelte author source into a runtime artifact with compiled style references applied.
    ///
    /// # Errors
    ///
    /// Returns [`SvelteCompileError`] when source validation, native authoring finalization, style
    /// resolution, or runtime bridging fails.
    pub fn compile_to_runtime_with_styles(
        self,
        source: SvelteComponentSource,
        sheet: &CompiledStyleSheet,
        tokens: &TokenSet,
    ) -> Result<SvelteRuntimeArtifact, SvelteCompileError> {
        let author_file = source.author_file.clone();
        let compiled = self.compile(source)?;
        let native_artifact =
            native_artifact_from_svelte_with_defaults(author_file.as_str(), &compiled, false)?;
        let runtime = NativeRuntimeBridge::new()
            .bridge_artifact_with_styles(&native_artifact, sheet, tokens)
            .map_err(|error| bridge_error(author_file.as_str(), &error))?;
        Ok(SvelteRuntimeArtifact { compiled, runtime })
    }

    /// Compiles Svelte author source into a themed runtime artifact.
    ///
    /// # Errors
    ///
    /// Returns [`SvelteCompileError`] when source validation, native authoring finalization, theme
    /// token resolution, style resolution, or runtime bridging fails.
    pub fn compile_to_runtime_with_theme(
        self,
        source: SvelteComponentSource,
        sheet: &CompiledStyleSheet,
        tokens: &TokenSet,
        theme: &str,
    ) -> Result<SvelteRuntimeArtifact, SvelteCompileError> {
        let author_file = source.author_file.clone();
        let compiled = self.compile(source)?;
        let native_artifact =
            native_artifact_from_svelte_with_defaults(author_file.as_str(), &compiled, false)?;
        let runtime = NativeRuntimeBridge::new()
            .bridge_artifact_with_theme(&native_artifact, sheet, tokens, theme)
            .map_err(|error| bridge_error(author_file.as_str(), &error))?;
        Ok(SvelteRuntimeArtifact { compiled, runtime })
    }
}

fn compiler_artifact_required_error(source_map: SvelteSourceMap) -> SvelteCompileError {
    SvelteCompileError {
        diagnostics: vec![AuthoringDiagnostic::new(
            AuthoringDiagnosticSeverity::Error,
            "svelte.compiler-artifact.required",
            "Svelte source must be compiled by the Svelte compiler adapter before entering the Rust renderer",
        )],
        source_map,
    }
}

fn native_artifact_from_svelte(
    author_file: &str,
    compiled: &SvelteCompiledArtifact,
) -> Result<hawk2ui_authoring::NativeAuthoringArtifact, SvelteCompileError> {
    native_artifact_from_svelte_with_defaults(author_file, compiled, true)
}

fn native_artifact_from_svelte_with_defaults(
    author_file: &str,
    compiled: &SvelteCompiledArtifact,
    include_default_visual_props: bool,
) -> Result<hawk2ui_authoring::NativeAuthoringArtifact, SvelteCompileError> {
    if let Some(native_program) = &compiled.native_program {
        return native_program
            .to_native_authoring_artifact(author_file, include_default_visual_props)
            .map_err(|error| SvelteCompileError {
                diagnostics: error.diagnostics().to_vec(),
                source_map: SvelteSourceMap {
                    author_file: author_file.to_string(),
                },
            });
    }
    Err(compiler_artifact_required_error(SvelteSourceMap {
        author_file: author_file.to_string(),
    }))
}

fn svelte_artifact_from_native_program(
    source_map: SvelteSourceMap,
    native_program: FrameworkNativeProgram,
) -> Result<SvelteCompiledArtifact, SvelteCompileError> {
    let events = framework_program_events(&native_program);
    let renderer_operations = native_program
        .custom_renderer_operation_keys("svelte")
        .map_err(|error| custom_renderer_error(source_map.author_file(), &error))?;
    Ok(SvelteCompiledArtifact {
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
        diagnostics: Vec::new(),
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
) -> SvelteCompileError {
    SvelteCompileError {
        diagnostics: vec![AuthoringDiagnostic::new(
            AuthoringDiagnosticSeverity::Error,
            "svelte.custom-renderer.failed",
            format!("{}: {}", error.rule(), error.message()),
        )],
        source_map: SvelteSourceMap {
            author_file: author_file.to_string(),
        },
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
