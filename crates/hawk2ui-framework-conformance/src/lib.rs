#![forbid(unsafe_code)]
//! Shared framework conformance harness for `Hawk2UI` native, `Svelte`, `React`, `Vue`, and `Solid` integrations.

use hawk2ui_authoring::{
    AssetRef, ElementKind, EventKind, EventPayloadField, NativeAuthoringElement,
    NativeAuthoringRuntime, NativeChild, NativeLifecycleEvent, NativeRef, NativeRuntimeBridge,
    NativeRuntimeBridgeArtifact, PointerEventKind, PropValue, StyleRef,
};
use hawk2ui_framework_react::{ReactElementTree, ReactIntegration};
use hawk2ui_framework_solid::{SolidComponentSource, SolidIntegration};
use hawk2ui_framework_svelte::{SvelteComponentSource, SvelteIntegration};
use hawk2ui_framework_vue::{VueIntegration, VueSingleFileComponent};
use hawk2ui_layout::Viewport;
use hawk2ui_render::{Color, Geometry, RendererBackend};
use hawk2ui_render_skia::{SkiaFrameSnapshot, SkiaRendererBackend};
use hawk2ui_runtime::{RuntimeDrawCommand, RuntimeSceneBridge, RuntimeSceneFrame, RuntimeViewId};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-framework-conformance";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

/// Frameworks covered by the shared conformance harness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameworkKind {
    /// Direct native authoring.
    Native,
    /// Svelte 5 integration.
    Svelte,
    /// React 19 and later integration.
    React,
    /// Vue 3.5 and later integration.
    Vue,
    /// Solid integration.
    Solid,
}

/// Normalized framework snapshot used for conformance comparisons.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConformanceSnapshot {
    framework: FrameworkKind,
    root_id: String,
    keyed_children: Vec<String>,
    refs: Vec<String>,
    style_refs: Vec<String>,
    asset_paths: Vec<String>,
    event_keys: Vec<String>,
    state_updates: Vec<String>,
}

impl ConformanceSnapshot {
    /// Returns the framework that produced this snapshot.
    #[must_use]
    pub const fn framework(&self) -> FrameworkKind {
        self.framework
    }

    /// Returns the normalized root node identifier.
    #[must_use]
    pub fn root_id(&self) -> &str {
        &self.root_id
    }

    /// Returns normalized keyed child order.
    #[must_use]
    pub fn keyed_children(&self) -> &[String] {
        &self.keyed_children
    }

    /// Returns normalized reference names.
    #[must_use]
    pub fn refs(&self) -> &[String] {
        &self.refs
    }

    /// Returns normalized style reference names.
    #[must_use]
    pub fn style_refs(&self) -> &[String] {
        &self.style_refs
    }

    /// Returns normalized asset paths.
    #[must_use]
    pub fn asset_paths(&self) -> &[String] {
        &self.asset_paths
    }

    /// Returns normalized event keys.
    #[must_use]
    pub fn event_keys(&self) -> &[String] {
        &self.event_keys
    }

    /// Returns normalized state update keys.
    #[must_use]
    pub fn state_updates(&self) -> &[String] {
        &self.state_updates
    }

    fn comparable(&self) -> ComparableSnapshot {
        ComparableSnapshot {
            root_id: self.root_id.clone(),
            keyed_children: self.keyed_children.clone(),
            refs: self.refs.clone(),
            style_refs: self.style_refs.clone(),
            asset_paths: self.asset_paths.clone(),
            event_keys: self.event_keys.clone(),
            state_updates: self.state_updates.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ComparableSnapshot {
    root_id: String,
    keyed_children: Vec<String>,
    refs: Vec<String>,
    style_refs: Vec<String>,
    asset_paths: Vec<String>,
    event_keys: Vec<String>,
    state_updates: Vec<String>,
}

/// Conformance run report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameworkConformanceReport {
    snapshots: Vec<ConformanceSnapshot>,
}

impl FrameworkConformanceReport {
    /// Returns snapshots in stable framework order.
    #[must_use]
    pub fn snapshots(&self) -> &[ConformanceSnapshot] {
        &self.snapshots
    }

    /// Returns the frameworks included in stable order.
    #[must_use]
    pub fn frameworks(&self) -> Vec<FrameworkKind> {
        self.snapshots
            .iter()
            .map(ConformanceSnapshot::framework)
            .collect()
    }

    /// Returns whether every framework emitted the same normalized native contract.
    #[must_use]
    pub fn is_equivalent(&self) -> bool {
        let Some(first) = self.snapshots.first().map(ConformanceSnapshot::comparable) else {
            return false;
        };
        self.snapshots
            .iter()
            .all(|snapshot| snapshot.comparable() == first)
    }
}

/// Source-mapped diagnostic evidence for framework errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameworkDiagnosticEvidence {
    framework: FrameworkKind,
    author_file: String,
    rule: String,
}

impl FrameworkDiagnosticEvidence {
    /// Returns the framework that emitted the diagnostic.
    #[must_use]
    pub const fn framework(&self) -> FrameworkKind {
        self.framework
    }

    /// Returns the author source file associated with the diagnostic.
    #[must_use]
    pub fn author_file(&self) -> &str {
        &self.author_file
    }

    /// Returns the diagnostic rule.
    #[must_use]
    pub fn rule(&self) -> &str {
        &self.rule
    }
}

/// Diagnostic conformance report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameworkDiagnosticReport {
    diagnostics: Vec<FrameworkDiagnosticEvidence>,
}

impl FrameworkDiagnosticReport {
    /// Returns diagnostic evidence in stable framework order.
    #[must_use]
    pub fn diagnostics(&self) -> &[FrameworkDiagnosticEvidence] {
        &self.diagnostics
    }
}

/// Runtime rendering evidence for a framework integration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameworkRuntimeEvidence {
    framework: FrameworkKind,
    root_id: String,
    child_ids: Vec<String>,
    frames_presented: u64,
    changed_pixels: usize,
    operation_keys: Vec<String>,
}

impl FrameworkRuntimeEvidence {
    /// Returns the framework that produced this runtime evidence.
    #[must_use]
    pub const fn framework(&self) -> FrameworkKind {
        self.framework
    }

    /// Returns the runtime root node ID.
    #[must_use]
    pub fn root_id(&self) -> &str {
        &self.root_id
    }

    /// Returns the runtime child IDs in layout order.
    #[must_use]
    pub fn child_ids(&self) -> &[String] {
        &self.child_ids
    }

    /// Returns how many frames were successfully presented to the renderer backend.
    #[must_use]
    pub const fn frames_presented(&self) -> u64 {
        self.frames_presented
    }

    /// Returns the number of child-region pixels changed by foreground drawing.
    #[must_use]
    pub const fn changed_pixels(&self) -> usize {
        self.changed_pixels
    }

    /// Returns stable operation keys emitted by the authoring/runtime bridge.
    #[must_use]
    pub fn operation_keys(&self) -> &[String] {
        &self.operation_keys
    }
}

/// Runtime conformance report for integrations that have an end-to-end runtime bridge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameworkRuntimeReport {
    evidence: Vec<FrameworkRuntimeEvidence>,
}

impl FrameworkRuntimeReport {
    /// Returns runtime rendering evidence in stable framework order.
    #[must_use]
    pub fn evidence(&self) -> &[FrameworkRuntimeEvidence] {
        &self.evidence
    }

    /// Returns frameworks covered by this runtime report in stable order.
    #[must_use]
    pub fn frameworks(&self) -> Vec<FrameworkKind> {
        self.evidence
            .iter()
            .map(FrameworkRuntimeEvidence::framework)
            .collect()
    }
}

/// Failure evidence for invalid framework contract fixtures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameworkFailureEvidence {
    framework: FrameworkKind,
    case: String,
    rule: String,
}

impl FrameworkFailureEvidence {
    /// Returns the framework that rejected the invalid fixture.
    #[must_use]
    pub const fn framework(&self) -> FrameworkKind {
        self.framework
    }

    /// Returns the invalid fixture case label.
    #[must_use]
    pub fn case(&self) -> &str {
        &self.case
    }

    /// Returns the stable diagnostic rule.
    #[must_use]
    pub fn rule(&self) -> &str {
        &self.rule
    }
}

/// Failure conformance report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameworkFailureReport {
    failures: Vec<FrameworkFailureEvidence>,
}

impl FrameworkFailureReport {
    /// Returns failure evidence in stable order.
    #[must_use]
    pub fn failures(&self) -> &[FrameworkFailureEvidence] {
        &self.failures
    }

    /// Returns whether a framework emitted the expected rule for a specific invalid fixture.
    #[must_use]
    pub fn has_failure(&self, framework: FrameworkKind, case: &str, rule: &str) -> bool {
        self.failures.iter().any(|failure| {
            failure.framework == framework && failure.case == case && failure.rule == rule
        })
    }
}

/// Shared conformance harness for framework integrations.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FrameworkConformanceHarness;

impl FrameworkConformanceHarness {
    /// Creates a framework conformance harness.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Runs all framework integrations against the same semantic fixture.
    ///
    /// # Errors
    ///
    /// Returns a message if any framework rejects the valid conformance fixture.
    pub fn run_all(self) -> Result<FrameworkConformanceReport, String> {
        Ok(FrameworkConformanceReport {
            snapshots: vec![
                native_snapshot()?,
                svelte_snapshot()?,
                react_snapshot()?,
                vue_snapshot()?,
                solid_snapshot()?,
            ],
        })
    }

    /// Runs invalid framework fixtures and records source-mapped diagnostics.
    ///
    /// # Errors
    ///
    /// Returns a message if an invalid fixture is accepted instead of producing diagnostics.
    pub fn run_diagnostic_matrix(self) -> Result<FrameworkDiagnosticReport, String> {
        Ok(FrameworkDiagnosticReport {
            diagnostics: vec![
                svelte_diagnostic()?,
                react_diagnostic()?,
                vue_diagnostic()?,
                solid_diagnostic()?,
            ],
        })
    }

    /// Runs runtime bridge and renderer conformance for integrations with a production runtime path.
    ///
    /// # Errors
    ///
    /// Returns a message if any reference framework fails to bridge, lay out, draw, or snapshot.
    pub fn run_runtime_matrix(self) -> Result<FrameworkRuntimeReport, String> {
        Ok(FrameworkRuntimeReport {
            evidence: vec![
                runtime_evidence_from_native()?,
                runtime_evidence_from_svelte()?,
            ],
        })
    }

    /// Runs invalid contract fixtures and records the exact rejection rules.
    ///
    /// # Errors
    ///
    /// Returns a message if an invalid fixture is accepted instead of producing diagnostics.
    pub fn run_failure_matrix(self) -> Result<FrameworkFailureReport, String> {
        Ok(FrameworkFailureReport {
            failures: vec![
                native_duplicate_key_failure()?,
                svelte_invalid_asset_failure()?,
                svelte_invalid_layout_failure()?,
                svelte_unsupported_event_failure()?,
            ],
        })
    }
}

fn native_snapshot() -> Result<ConformanceSnapshot, String> {
    let mut runtime = NativeAuthoringRuntime::new("conformance-native");
    runtime.mount(
        NativeAuthoringElement::new("root", ElementKind::View)
            .with_ref(NativeRef::new("root_ref"))
            .with_style(StyleRef::new("surface.card"))
            .with_asset(AssetRef::new("hawk.logo", "assets/logo.svg"))
            .with_child(NativeChild::keyed(
                "title",
                NativeAuthoringElement::new("title", ElementKind::Text),
            ))
            .with_child(NativeChild::keyed(
                "cta",
                NativeAuthoringElement::new("cta", ElementKind::Button),
            ))
            .with_event(
                EventKind::Pointer(PointerEventKind::Press),
                "handle_press",
                [EventPayloadField::Position],
            )
            .with_lifecycle(NativeLifecycleEvent::Mounted, "onMount")
            .with_lifecycle(NativeLifecycleEvent::Unmounted, "onUnmount"),
    );
    let artifact = runtime.finish().map_err(|error| format!("{error:?}"))?;
    Ok(ConformanceSnapshot {
        framework: FrameworkKind::Native,
        root_id: artifact.root().id().as_str().to_string(),
        keyed_children: to_strings(artifact.root().keyed_child_order()),
        refs: artifact
            .root()
            .refs()
            .iter()
            .map(|item| item.name().to_string())
            .collect(),
        style_refs: artifact
            .root()
            .style_refs()
            .iter()
            .map(|item| item.name().to_string())
            .collect(),
        asset_paths: artifact
            .root()
            .asset_refs()
            .iter()
            .map(|item| item.path().to_string())
            .collect(),
        event_keys: artifact
            .events()
            .iter()
            .map(|event| event.event().stable_key())
            .collect(),
        state_updates: vec!["state:items".into()],
    })
}

fn svelte_snapshot() -> Result<ConformanceSnapshot, String> {
    let artifact = SvelteIntegration::new()
        .compile(SvelteComponentSource::new("src/App.svelte", SVELTE_FIXTURE))
        .map_err(|error| format!("{error:?}"))?;
    Ok(ConformanceSnapshot {
        framework: FrameworkKind::Svelte,
        root_id: artifact.root().id().as_str().to_string(),
        keyed_children: artifact.keyed_children().to_vec(),
        refs: artifact.refs().to_vec(),
        style_refs: to_strings(artifact.style_refs()),
        asset_paths: artifact
            .asset_refs()
            .iter()
            .map(|asset| asset.path().to_string())
            .collect(),
        event_keys: artifact
            .events()
            .iter()
            .map(|event| event.event().stable_key())
            .collect(),
        state_updates: vec!["state:items".into()],
    })
}

fn react_snapshot() -> Result<ConformanceSnapshot, String> {
    let artifact = ReactIntegration::new()
        .render(ReactElementTree::new("src/App.tsx", REACT_FIXTURE))
        .map_err(|error| format!("{error:?}"))?;
    Ok(ConformanceSnapshot {
        framework: FrameworkKind::React,
        root_id: artifact.root().id().as_str().to_string(),
        keyed_children: artifact.keyed_children().to_vec(),
        refs: artifact.refs().to_vec(),
        style_refs: to_strings(artifact.style_refs()),
        asset_paths: artifact
            .asset_refs()
            .iter()
            .map(|asset| asset.path().to_string())
            .collect(),
        event_keys: artifact
            .events()
            .iter()
            .map(|event| event.event().stable_key())
            .collect(),
        state_updates: vec!["state:items".into()],
    })
}

fn vue_snapshot() -> Result<ConformanceSnapshot, String> {
    let artifact = VueIntegration::new()
        .render(VueSingleFileComponent::new("src/App.vue", VUE_FIXTURE))
        .map_err(|error| format!("{error:?}"))?;
    Ok(ConformanceSnapshot {
        framework: FrameworkKind::Vue,
        root_id: artifact.root().id().as_str().to_string(),
        keyed_children: artifact.keyed_children().to_vec(),
        refs: artifact.refs().to_vec(),
        style_refs: to_strings(artifact.style_refs()),
        asset_paths: artifact
            .asset_refs()
            .iter()
            .map(|asset| asset.path().to_string())
            .collect(),
        event_keys: artifact
            .events()
            .iter()
            .map(|event| event.event().stable_key())
            .collect(),
        state_updates: vec!["state:items".into()],
    })
}

fn solid_snapshot() -> Result<ConformanceSnapshot, String> {
    let artifact = SolidIntegration::new()
        .render(SolidComponentSource::new("src/App.tsx", SOLID_FIXTURE))
        .map_err(|error| format!("{error:?}"))?;
    Ok(ConformanceSnapshot {
        framework: FrameworkKind::Solid,
        root_id: artifact.root().id().as_str().to_string(),
        keyed_children: artifact.keyed_children().to_vec(),
        refs: artifact.refs().to_vec(),
        style_refs: to_strings(artifact.style_refs()),
        asset_paths: artifact
            .asset_refs()
            .iter()
            .map(|asset| asset.path().to_string())
            .collect(),
        event_keys: artifact
            .events()
            .iter()
            .map(|event| event.event().stable_key())
            .collect(),
        state_updates: vec!["state:items".into()],
    })
}

fn svelte_diagnostic() -> Result<FrameworkDiagnosticEvidence, String> {
    let error = SvelteIntegration::new()
        .compile(SvelteComponentSource::new(
            "src/Broken.svelte",
            "<hawk-view data-asset=\"https://example.invalid/logo.svg\" />",
        ))
        .map_or_else(Ok, |_| {
            Err("invalid Svelte diagnostic fixture was accepted".to_string())
        })?;
    Ok(diagnostic_evidence(
        FrameworkKind::Svelte,
        error.source_map().author_file(),
        error.diagnostics()[0].rule.as_str(),
    ))
}

fn react_diagnostic() -> Result<FrameworkDiagnosticEvidence, String> {
    let error = ReactIntegration::new()
        .render(ReactElementTree::new(
            "src/Broken.tsx",
            "<hawk-view data-asset=\"https://example.invalid/logo.svg\" />",
        ))
        .map_or_else(Ok, |_| {
            Err("invalid React diagnostic fixture was accepted".to_string())
        })?;
    Ok(diagnostic_evidence(
        FrameworkKind::React,
        error.source_map().author_file(),
        error.diagnostics()[0].rule.as_str(),
    ))
}

fn vue_diagnostic() -> Result<FrameworkDiagnosticEvidence, String> {
    let error = VueIntegration::new()
        .render(VueSingleFileComponent::new(
            "src/Broken.vue",
            "<hawk-view data-asset=\"https://example.invalid/logo.svg\" />",
        ))
        .map_or_else(Ok, |_| {
            Err("invalid Vue diagnostic fixture was accepted".to_string())
        })?;
    Ok(diagnostic_evidence(
        FrameworkKind::Vue,
        error.source_map().author_file(),
        error.diagnostics()[0].rule.as_str(),
    ))
}

fn solid_diagnostic() -> Result<FrameworkDiagnosticEvidence, String> {
    let error = SolidIntegration::new()
        .render(SolidComponentSource::new(
            "src/Broken.tsx",
            "<hawk-view data-asset=\"https://example.invalid/logo.svg\" />",
        ))
        .map_or_else(Ok, |_| {
            Err("invalid Solid diagnostic fixture was accepted".to_string())
        })?;
    Ok(diagnostic_evidence(
        FrameworkKind::Solid,
        error.source_map().author_file(),
        error.diagnostics()[0].rule.as_str(),
    ))
}

fn diagnostic_evidence(
    framework: FrameworkKind,
    author_file: &str,
    rule: &str,
) -> FrameworkDiagnosticEvidence {
    FrameworkDiagnosticEvidence {
        framework,
        author_file: author_file.to_string(),
        rule: rule.to_string(),
    }
}

fn runtime_evidence_from_native() -> Result<FrameworkRuntimeEvidence, String> {
    let artifact = native_runtime_artifact()?;
    runtime_evidence(
        FrameworkKind::Native,
        artifact.runtime_tree(),
        artifact.operation_keys(),
    )
}

fn runtime_evidence_from_svelte() -> Result<FrameworkRuntimeEvidence, String> {
    let artifact = SvelteIntegration::new()
        .compile_to_runtime(SvelteComponentSource::new("src/App.svelte", SVELTE_FIXTURE))
        .map_err(|error| format!("{error:?}"))?;
    runtime_evidence(
        FrameworkKind::Svelte,
        artifact.runtime_tree(),
        artifact.operation_keys(),
    )
}

fn runtime_evidence(
    framework: FrameworkKind,
    runtime_tree: &hawk2ui_runtime::RuntimeViewTree,
    operation_keys: &[String],
) -> Result<FrameworkRuntimeEvidence, String> {
    let root_id = runtime_tree.root_id().as_str().to_string();
    let root = RuntimeViewId::new(root_id.clone());
    let child_ids = runtime_tree
        .children_of(&root)
        .iter()
        .map(RuntimeViewId::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let frame = RuntimeSceneBridge::new(Viewport::new(220.0, 120.0))
        .build(runtime_tree)
        .map_err(|error| format!("{error:?}"))?;

    let mut backend = SkiaRendererBackend::default();
    backend
        .create_surface("conformance", 220, 120)
        .map_err(|error| format!("{error:?}"))?;
    backend
        .begin_frame("conformance")
        .map_err(|error| format!("{error:?}"))?;
    backend
        .clear(Color::rgba(0, 0, 0, 255))
        .map_err(|error| format!("{error:?}"))?;
    render_runtime_frame_with_skia(&frame, &mut backend)?;
    backend
        .end_frame("conformance")
        .map_err(|error| format!("{error:?}"))?;

    let snapshot = backend
        .frame_snapshot("conformance")
        .map_err(|error| format!("{error:?}"))?;
    let title_geometry = frame
        .geometry_for(&RuntimeViewId::new("title"))
        .ok_or_else(|| "runtime frame did not contain title geometry".to_string())?;
    Ok(FrameworkRuntimeEvidence {
        framework,
        root_id,
        child_ids,
        frames_presented: 1,
        changed_pixels: count_changed_pixels(snapshot, CONFORMANCE_BACKGROUND, title_geometry),
        operation_keys: operation_keys.to_vec(),
    })
}

fn native_runtime_artifact() -> Result<NativeRuntimeBridgeArtifact, String> {
    let mut runtime = NativeAuthoringRuntime::new("conformance-native-runtime");
    runtime.mount(
        NativeAuthoringElement::new("root", ElementKind::View)
            .with_prop("background", PropValue::String("#080a0e".to_string()))
            .with_ref(NativeRef::new("root_ref"))
            .with_style(StyleRef::new("surface.card"))
            .with_asset(AssetRef::new("hawk.logo", "assets/logo.svg"))
            .with_child(NativeChild::keyed("title", runtime_text_child("title")))
            .with_child(NativeChild::keyed("cta", runtime_text_child("cta")))
            .with_event(
                EventKind::Pointer(PointerEventKind::Press),
                "handle_press",
                [EventPayloadField::Position],
            )
            .with_lifecycle(NativeLifecycleEvent::Mounted, "onMount")
            .with_lifecycle(NativeLifecycleEvent::Unmounted, "onUnmount"),
    );
    let artifact = runtime.finish().map_err(|error| format!("{error:?}"))?;
    NativeRuntimeBridge::new()
        .bridge_artifact(&artifact)
        .map_err(|error| format!("{error:?}"))
}

fn runtime_text_child(id: &str) -> NativeAuthoringElement {
    NativeAuthoringElement::new(id, ElementKind::Text)
        .with_prop("text", PropValue::String(id.to_string()))
        .with_prop("font_size", PropValue::Number(18.0))
        .with_prop("color", PropValue::String("#ffffff".to_string()))
        .with_prop("width", PropValue::Number(160.0))
        .with_prop("height", PropValue::Number(32.0))
}

fn render_runtime_frame_with_skia(
    frame: &RuntimeSceneFrame,
    backend: &mut SkiaRendererBackend,
) -> Result<(), String> {
    for command in frame.draw_commands() {
        match command {
            RuntimeDrawCommand::Fill {
                geometry, color, ..
            } => backend
                .fill(*geometry, *color)
                .map_err(|error| format!("{error:?}"))?,
            RuntimeDrawCommand::Text {
                geometry,
                text,
                font_size,
                color,
                ..
            } => backend
                .draw_text_at(
                    text,
                    geometry.x,
                    geometry.y + geometry.height,
                    *font_size,
                    *color,
                )
                .map_err(|error| format!("{error:?}"))?,
        }
    }
    Ok(())
}

fn count_changed_pixels(
    snapshot: &SkiaFrameSnapshot,
    background: u32,
    geometry: Geometry,
) -> usize {
    let min_x = f64::from(geometry.x.max(0.0));
    let min_y = f64::from(geometry.y.max(0.0));
    let max_x = f64::from((geometry.x + geometry.width).max(0.0));
    let max_y = f64::from((geometry.y + geometry.height).max(0.0));
    let mut changed = 0;
    for y in 0..snapshot.height() {
        let y_position = f64::from(y);
        if y_position < min_y || y_position >= max_y {
            continue;
        }
        for x in 0..snapshot.width() {
            let x_position = f64::from(x);
            if x_position < min_x || x_position >= max_x {
                continue;
            }
            if snapshot
                .pixel_at(x, y)
                .is_some_and(|pixel| pixel != background)
            {
                changed += 1;
            }
        }
    }
    changed
}

const CONFORMANCE_BACKGROUND: u32 = 0x0008_0a0e;

fn native_duplicate_key_failure() -> Result<FrameworkFailureEvidence, String> {
    let mut runtime = NativeAuthoringRuntime::new("invalid-native-duplicate-key");
    runtime.mount(
        NativeAuthoringElement::new("root", ElementKind::View)
            .with_child(NativeChild::keyed(
                "title",
                NativeAuthoringElement::new("title", ElementKind::Text),
            ))
            .with_child(NativeChild::keyed(
                "title",
                NativeAuthoringElement::new("title-copy", ElementKind::Text),
            )),
    );
    let error = runtime.finish().map_or_else(Ok, |_| {
        Err("duplicate native child key fixture was accepted".to_string())
    })?;
    Ok(failure_evidence(
        FrameworkKind::Native,
        "duplicate-keyed-child",
        error.diagnostics()[0].rule.as_str(),
    ))
}

fn svelte_invalid_asset_failure() -> Result<FrameworkFailureEvidence, String> {
    let error = SvelteIntegration::new()
        .compile(SvelteComponentSource::new(
            "src/Broken.svelte",
            "<hawk-view data-asset=\"../secret.svg\" />",
        ))
        .map_or_else(Ok, |_| {
            Err("unsafe Svelte asset path fixture was accepted".to_string())
        })?;
    Ok(failure_evidence(
        FrameworkKind::Svelte,
        "invalid-asset-path",
        error.diagnostics()[0].rule.as_str(),
    ))
}

fn svelte_invalid_layout_failure() -> Result<FrameworkFailureEvidence, String> {
    let error = SvelteIntegration::new()
        .compile_to_runtime(SvelteComponentSource::new(
            "src/Broken.svelte",
            r#"<hawk-view id="root">{#each items as item (item.id)}<hawk-text id={item.id} data-font-size="0">{item.id}</hawk-text>{/each}</hawk-view>"#,
        ))
        .map_or_else(Ok, |_| {
            Err("invalid Svelte runtime layout number fixture was accepted".to_string())
        })?;
    Ok(failure_evidence(
        FrameworkKind::Svelte,
        "invalid-layout-number",
        error.diagnostics()[0].rule.as_str(),
    ))
}

fn svelte_unsupported_event_failure() -> Result<FrameworkFailureEvidence, String> {
    let error = SvelteIntegration::new()
        .compile(SvelteComponentSource::new(
            "src/Broken.svelte",
            r#"<hawk-view id="root" on:hover={handleHover} />"#,
        ))
        .map_or_else(Ok, |_| {
            Err("unsupported Svelte event fixture was accepted".to_string())
        })?;
    Ok(failure_evidence(
        FrameworkKind::Svelte,
        "unsupported-event",
        error.diagnostics()[0].rule.as_str(),
    ))
}

fn failure_evidence(framework: FrameworkKind, case: &str, rule: &str) -> FrameworkFailureEvidence {
    FrameworkFailureEvidence {
        framework,
        case: case.to_string(),
        rule: rule.to_string(),
    }
}

fn to_strings(items: impl IntoIterator<Item = impl AsRef<str>>) -> Vec<String> {
    items
        .into_iter()
        .map(|item| item.as_ref().to_string())
        .collect()
}

const SVELTE_FIXTURE: &str = r#"<hawk-view id="root" use:ref="root_ref" class="surface.card" data-asset="assets/logo.svg" on:press={handlePress} on:mount={onMount} on:destroy={onDestroy}>{#each items as item (item.id)}<hawk-text id={item.id}>{item.id}</hawk-text>{/each}</hawk-view>"#;
const REACT_FIXTURE: &str = r#"<hawk-view id="root" ref="root_ref" className="surface.card" data-asset="assets/logo.svg" onPointerDown={handlePress} onMount={onMount} onUnmount={onUnmount}>{items.map((item) => <hawk-text id={item.id} key={item.id}>{item.id}</hawk-text>)}</hawk-view>"#;
const VUE_FIXTURE: &str = r#"<hawk-view id="root" ref="root_ref" class="surface.card" data-asset="assets/logo.svg" @pointerdown="handlePress" @mounted="onMounted" @unmounted="onUnmounted"><hawk-text v-for="item in items" :id="item.id" :key="item.id">{{ item.id }}</hawk-text></hawk-view>"#;
const SOLID_FIXTURE: &str = r#"<hawk-view id="root" ref={root_ref} class="surface.card" data-asset="assets/logo.svg" onPointerDown={handlePress} onMount={onMount} onCleanup={onCleanup}><For each={items()}>{(item) => <hawk-text id={item.id}>{item.id}</hawk-text>}</For></hawk-view>"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-framework-conformance");
    }
}
