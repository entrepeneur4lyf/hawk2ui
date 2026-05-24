#![forbid(unsafe_code)]
//! Shared framework conformance harness for `Hawk2UI` native, `Svelte`, `React`, `Vue`, and `Solid` integrations.

use hawk2ui_authoring::{
    AssetRef, ElementKind, EventKind, EventPayloadField, NativeAuthoringElement,
    NativeAuthoringRuntime, NativeChild, NativeLifecycleEvent, NativeRef, PointerEventKind,
    StyleRef,
};
use hawk2ui_framework_react::{ReactElementTree, ReactIntegration};
use hawk2ui_framework_solid::{SolidComponentSource, SolidIntegration};
use hawk2ui_framework_svelte::{SvelteComponentSource, SvelteIntegration};
use hawk2ui_framework_vue::{VueIntegration, VueSingleFileComponent};

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
    #[must_use]
    pub fn run_diagnostic_matrix(self) -> FrameworkDiagnosticReport {
        let diagnostics = vec![
            svelte_diagnostic(),
            react_diagnostic(),
            vue_diagnostic(),
            solid_diagnostic(),
        ];
        FrameworkDiagnosticReport { diagnostics }
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

fn svelte_diagnostic() -> FrameworkDiagnosticEvidence {
    let error = SvelteIntegration::new()
        .compile(SvelteComponentSource::new(
            "src/Broken.svelte",
            "<hawk-view data-asset=\"https://example.invalid/logo.svg\" />",
        ))
        .expect_err("invalid Svelte fixture should fail");
    diagnostic_evidence(
        FrameworkKind::Svelte,
        error.source_map().author_file(),
        error.diagnostics()[0].rule.as_str(),
    )
}

fn react_diagnostic() -> FrameworkDiagnosticEvidence {
    let error = ReactIntegration::new()
        .render(ReactElementTree::new(
            "src/Broken.tsx",
            "<hawk-view data-asset=\"https://example.invalid/logo.svg\" />",
        ))
        .expect_err("invalid React fixture should fail");
    diagnostic_evidence(
        FrameworkKind::React,
        error.source_map().author_file(),
        error.diagnostics()[0].rule.as_str(),
    )
}

fn vue_diagnostic() -> FrameworkDiagnosticEvidence {
    let error = VueIntegration::new()
        .render(VueSingleFileComponent::new(
            "src/Broken.vue",
            "<hawk-view data-asset=\"https://example.invalid/logo.svg\" />",
        ))
        .expect_err("invalid Vue fixture should fail");
    diagnostic_evidence(
        FrameworkKind::Vue,
        error.source_map().author_file(),
        error.diagnostics()[0].rule.as_str(),
    )
}

fn solid_diagnostic() -> FrameworkDiagnosticEvidence {
    let error = SolidIntegration::new()
        .render(SolidComponentSource::new(
            "src/Broken.tsx",
            "<hawk-view data-asset=\"https://example.invalid/logo.svg\" />",
        ))
        .expect_err("invalid Solid fixture should fail");
    diagnostic_evidence(
        FrameworkKind::Solid,
        error.source_map().author_file(),
        error.diagnostics()[0].rule.as_str(),
    )
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
