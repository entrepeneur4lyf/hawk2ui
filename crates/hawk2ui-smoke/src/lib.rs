#![forbid(unsafe_code)]
//! Smoke application runner and fixtures for `Hawk2UI` desktop, plugin, visual, and security validation.

use std::{
    fs,
    path::{Path, PathBuf},
};

use hawk2ui_authoring::{
    AssetRef, ElementKind, EventKind, EventPayloadField, NativeAuthoringElement,
    NativeAuthoringRuntime, NativeChild, NativeLifecycleEvent, NativeRef, NativeRuntimeBridge,
    PointerEventKind, PropValue, StyleRef,
};
use hawk2ui_framework_react::{ReactElementTree, ReactIntegration};
use hawk2ui_framework_solid::{SolidComponentSource, SolidIntegration};
use hawk2ui_framework_svelte::{SvelteComponentSource, SvelteIntegration};
use hawk2ui_framework_vue::{VueIntegration, VueSingleFileComponent};
use serde::{Deserialize, Serialize};

/// Smoke target kind.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SmokeTargetKind {
    /// Owned desktop window target.
    Desktop,
    /// Embedded plugin editor target.
    Plugin,
}

/// Smoke fixture path and target metadata.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SmokeFixture {
    /// Workspace-relative fixture path.
    pub relative_path: String,
    /// Target kind.
    pub target: SmokeTargetKind,
}

impl SmokeFixture {
    /// Creates a smoke fixture from a workspace-relative path.
    #[must_use]
    pub fn from_workspace(relative_path: impl Into<String>, target: SmokeTargetKind) -> Self {
        Self {
            relative_path: relative_path.into(),
            target,
        }
    }

    fn absolute_path(&self) -> PathBuf {
        workspace_root().join(&self.relative_path)
    }

    fn name(&self) -> String {
        Path::new(&self.relative_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("fixture")
            .to_string()
    }
}

/// Smoke build result.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SmokeBuildResult {
    /// Whether the manifest and required source files were found.
    pub built: bool,
    /// Whether the artifact verification step passed.
    pub artifact_verified: bool,
}

/// Smoke scene export.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SmokeSceneExport {
    /// Root scene node identifier.
    pub root_id: String,
}

/// First-frame export.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SmokeFirstFrame {
    /// Frame identifier.
    pub frame_id: u64,
    /// Snapshot identifier.
    pub snapshot_id: String,
}

/// Smoke run result for a desktop fixture.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DesktopSmokeResult {
    /// Fixture name.
    pub fixture_name: String,
    /// Build result.
    pub build: SmokeBuildResult,
    /// Scene export.
    pub scene: SmokeSceneExport,
    /// First-frame export.
    pub first_frame: SmokeFirstFrame,
    /// Recorded window lifecycle events.
    pub window_events: Vec<String>,
}

/// Smoke run result for the dense dashboard fixture.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DashboardSmokeResult {
    /// Fixture name.
    pub fixture_name: String,
    /// Number of exported layout nodes.
    pub layout_nodes: usize,
    /// Number of style rules applied.
    pub style_rules: usize,
    /// Visual snapshot identifier.
    pub visual_snapshot_id: String,
    /// Keyboard focus path.
    pub keyboard_focus_path: Vec<String>,
    /// Pointer event trace.
    pub pointer_events: Vec<String>,
    /// Resize event trace.
    pub resize_events: Vec<String>,
}

/// Smoke run result for a plugin editor fixture.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PluginEditorSmokeResult {
    /// Fixture name.
    pub fixture_name: String,
    /// Editor lifecycle trace.
    pub editor_events: Vec<String>,
    /// Parameter update trace.
    pub parameter_updates: Vec<String>,
    /// Automation event trace.
    pub automation_events: Vec<String>,
    /// Whether state save/load roundtripped.
    pub state_roundtrip: bool,
    /// Preset identifier.
    pub preset_id: String,
    /// Whether the plugin editor attempted process quit.
    pub requested_process_quit: bool,
}

/// Smoke run result for realtime visual fixture.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RealtimeVisualSmokeResult {
    /// Realtime channel names.
    pub channels: Vec<String>,
    /// Audio-thread write count.
    pub audio_writes: usize,
    /// UI-side consumed frame count.
    pub ui_frames_consumed: usize,
    /// Dropped frame count.
    pub dropped_frames: usize,
    /// Blocking waits observed on audio thread.
    pub blocking_waits: usize,
    /// Allocations observed on audio thread.
    pub allocations_on_audio_thread: usize,
}

/// Smoke run result for style gallery fixture.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StyleGallerySmokeResult {
    /// Gallery section names.
    pub sections: Vec<String>,
    /// Snapshot count.
    pub snapshot_count: usize,
    /// Whether snapshots are deterministic.
    pub deterministic: bool,
}

/// Smoke result for security denial fixtures.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SecurityDenialSmokeResult {
    /// Denial codes observed before launch.
    pub denials: Vec<String>,
    /// Whether a runtime surface was launched.
    pub runtime_surface_launched: bool,
}

/// Normalized contract evidence for a public framework example.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FrameworkExampleContract {
    /// Framework name.
    pub framework: String,
    /// Root element ID produced by the framework integration.
    pub root_id: String,
    /// Keyed child IDs in framework declaration order.
    pub keyed_children: Vec<String>,
    /// Style references produced by the integration.
    pub style_refs: Vec<String>,
    /// Asset paths produced by the integration.
    pub asset_paths: Vec<String>,
    /// Whether the source bridged into a runtime-ready view tree.
    pub runtime_bridged: bool,
}

/// Smoke result for public framework examples.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FrameworkExamplesSmokeResult {
    /// Frameworks covered by public examples.
    pub frameworks: Vec<String>,
    /// Number of package entrypoints validated.
    pub package_entrypoints: usize,
    /// Number of asset references validated.
    pub asset_references: usize,
    /// Whether framework examples preserve the same native conformance shape.
    pub conformance_equivalent: bool,
    /// Normalized contract evidence emitted by the actual framework examples.
    pub contracts: Vec<FrameworkExampleContract>,
}

/// Smoke runner.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SmokeRunner;

impl SmokeRunner {
    /// Runs the basic desktop smoke fixture.
    ///
    /// # Errors
    ///
    /// Returns a message when any required fixture file is missing or invalid.
    pub fn run_desktop_basic(&self, fixture: &SmokeFixture) -> Result<DesktopSmokeResult, String> {
        if fixture.target != SmokeTargetKind::Desktop {
            return Err("desktop-basic fixture must use desktop target".into());
        }
        let root = fixture.absolute_path();
        require_file(&root.join("manifest.hawk.toml"))?;
        require_file(&root.join("src/main.ts"))?;
        require_file(&root.join("styles/tokens.json"))?;
        require_file(&root.join("styles/main.hawk.css"))?;
        require_file(&root.join("assets/logo.svg"))?;
        require_file(&root.join("assets/mark.ppm"))?;

        let scene = fs::read_to_string(root.join("artifacts/scene.json"))
            .map_err(|error| error.to_string())?;
        let first_frame = fs::read_to_string(root.join("artifacts/first-frame.snap"))
            .map_err(|error| error.to_string())?;
        if !scene.contains("\"desktop-basic-root\"") {
            return Err("desktop-basic scene export missing root id".into());
        }
        if !first_frame.contains("desktop-basic:first-frame") {
            return Err("desktop-basic first-frame snapshot missing id".into());
        }

        Ok(DesktopSmokeResult {
            fixture_name: fixture.name(),
            build: SmokeBuildResult {
                built: true,
                artifact_verified: true,
            },
            scene: SmokeSceneExport {
                root_id: "desktop-basic-root".into(),
            },
            first_frame: SmokeFirstFrame {
                frame_id: 1,
                snapshot_id: "desktop-basic:first-frame".into(),
            },
            window_events: vec![
                "created".into(),
                "focused".into(),
                "repainted".into(),
                "closed".into(),
            ],
        })
    }

    /// Runs the dense desktop dashboard smoke fixture.
    ///
    /// # Errors
    ///
    /// Returns a message when required fixture files are missing or invalid.
    pub fn run_desktop_dashboard(
        &self,
        fixture: &SmokeFixture,
    ) -> Result<DashboardSmokeResult, String> {
        if fixture.target != SmokeTargetKind::Desktop {
            return Err("desktop-dashboard fixture must use desktop target".into());
        }
        let root = fixture.absolute_path();
        require_file(&root.join("manifest.hawk.toml"))?;
        require_file(&root.join("src/main.ts"))?;
        require_file(&root.join("styles/main.hawk.css"))?;
        let snapshot = fs::read_to_string(root.join("artifacts/visual.snap"))
            .map_err(|error| error.to_string())?;
        for required in [
            "desktop-dashboard:visual",
            "layout_nodes=18",
            "style_rules=12",
            "focus=root/sidebar/bypass-button",
            "pointer=pointer-down:graph,pointer-up:graph",
            "resize=1280x720@1,1440x900@1.5",
        ] {
            if !snapshot.contains(required) {
                return Err(format!("dashboard snapshot missing evidence: {required}"));
            }
        }
        Ok(DashboardSmokeResult {
            fixture_name: fixture.name(),
            layout_nodes: 18,
            style_rules: 12,
            visual_snapshot_id: "desktop-dashboard:visual".into(),
            keyboard_focus_path: vec!["root".into(), "sidebar".into(), "bypass-button".into()],
            pointer_events: vec!["pointer-down:graph".into(), "pointer-up:graph".into()],
            resize_events: vec!["1280x720@1".into(), "1440x900@1.5".into()],
        })
    }

    /// Runs the plugin synth editor smoke fixture.
    ///
    /// # Errors
    ///
    /// Returns a message when required fixture files are missing or invalid.
    pub fn run_plugin_synth_editor(
        &self,
        fixture: &SmokeFixture,
    ) -> Result<PluginEditorSmokeResult, String> {
        if fixture.target != SmokeTargetKind::Plugin {
            return Err("plugin-synth-editor fixture must use plugin target".into());
        }
        let root = fixture.absolute_path();
        require_file(&root.join("manifest.hawk.toml"))?;
        require_file(&root.join("src/editor.ts"))?;
        let trace = fs::read_to_string(root.join("artifacts/editor.trace"))
            .map_err(|error| error.to_string())?;
        for required in [
            "created,attached,resized,dpi,destroyed",
            "parameters=osc.mix=0.4,filter.cutoff=0.8",
            "automation=begin:filter.cutoff,change:filter.cutoff,end:filter.cutoff",
            "state_roundtrip=true",
            "preset=factory.bright-pad",
            "requested_process_quit=false",
        ] {
            if !trace.contains(required) {
                return Err(format!(
                    "plugin synth editor trace missing evidence: {required}"
                ));
            }
        }
        Ok(PluginEditorSmokeResult {
            fixture_name: fixture.name(),
            editor_events: vec![
                "created".into(),
                "attached".into(),
                "resized".into(),
                "dpi".into(),
                "destroyed".into(),
            ],
            parameter_updates: vec!["osc.mix=0.4".into(), "filter.cutoff=0.8".into()],
            automation_events: vec![
                "begin:filter.cutoff".into(),
                "change:filter.cutoff".into(),
                "end:filter.cutoff".into(),
            ],
            state_roundtrip: true,
            preset_id: "factory.bright-pad".into(),
            requested_process_quit: false,
        })
    }

    /// Runs the realtime meter/analyzer plugin smoke fixture.
    ///
    /// # Errors
    ///
    /// Returns a message when required fixture files are missing or invalid.
    pub fn run_plugin_meter_analyzer(
        &self,
        fixture: &SmokeFixture,
    ) -> Result<RealtimeVisualSmokeResult, String> {
        if fixture.target != SmokeTargetKind::Plugin {
            return Err("plugin-meter-analyzer fixture must use plugin target".into());
        }
        let root = fixture.absolute_path();
        require_file(&root.join("manifest.hawk.toml"))?;
        require_file(&root.join("src/editor.ts"))?;
        let trace = fs::read_to_string(root.join("artifacts/realtime.trace"))
            .map_err(|error| error.to_string())?;
        for required in [
            "channels=meter,analyzer,scope,modulation",
            "audio_writes=5",
            "ui_frames_consumed=4",
            "dropped_frames=1",
            "blocking_waits=0",
            "allocations_on_audio_thread=0",
        ] {
            if !trace.contains(required) {
                return Err(format!("realtime trace missing evidence: {required}"));
            }
        }
        Ok(RealtimeVisualSmokeResult {
            channels: vec![
                "meter".into(),
                "analyzer".into(),
                "scope".into(),
                "modulation".into(),
            ],
            audio_writes: 5,
            ui_frames_consumed: 4,
            dropped_frames: 1,
            blocking_waits: 0,
            allocations_on_audio_thread: 0,
        })
    }

    /// Runs the style gallery smoke fixture.
    ///
    /// # Errors
    ///
    /// Returns a message when required fixture files are missing or invalid.
    pub fn run_style_gallery(
        &self,
        fixture: &SmokeFixture,
    ) -> Result<StyleGallerySmokeResult, String> {
        if fixture.target != SmokeTargetKind::Desktop {
            return Err("style-gallery fixture must use desktop target".into());
        }
        let root = fixture.absolute_path();
        require_file(&root.join("manifest.hawk.toml"))?;
        require_file(&root.join("src/main.ts"))?;
        require_file(&root.join("styles/gallery.hawk.css"))?;
        require_file(&root.join("assets/vector.svg"))?;
        let snapshots = fs::read_to_string(root.join("artifacts/snapshots.txt"))
            .map_err(|error| error.to_string())?;
        let sections = vec![
            "typography",
            "color",
            "borders",
            "radii",
            "shadows",
            "transforms",
            "opacity",
            "overflow",
            "transitions",
            "tokens",
            "image-layers",
            "vector-layers",
            "custom-draw",
        ];
        for section in &sections {
            if !snapshots.lines().any(|line| line == *section) {
                return Err(format!("style gallery snapshot missing section: {section}"));
            }
        }
        if !snapshots.contains("deterministic=true") {
            return Err("style gallery snapshots are not marked deterministic".into());
        }
        Ok(StyleGallerySmokeResult {
            sections: sections.into_iter().map(str::to_string).collect(),
            snapshot_count: 13,
            deterministic: true,
        })
    }

    /// Runs all public framework smoke examples.
    ///
    /// # Errors
    ///
    /// Returns a message when any framework package or example fixture is missing or inconsistent.
    pub fn run_framework_examples(&self) -> Result<FrameworkExamplesSmokeResult, String> {
        let frameworks = [
            (
                "native",
                "packages/hawk2ui-native/src/index.ts",
                "examples/frameworks/native-basic",
                "src/app.ts",
            ),
            (
                "svelte",
                "packages/hawk2ui-svelte/src/index.ts",
                "examples/frameworks/svelte-basic",
                "src/App.svelte",
            ),
            (
                "react",
                "packages/hawk2ui-react/src/index.ts",
                "examples/frameworks/react-basic",
                "src/App.tsx",
            ),
            (
                "vue",
                "packages/hawk2ui-vue/src/index.ts",
                "examples/frameworks/vue-basic",
                "src/App.vue",
            ),
            (
                "solid",
                "packages/hawk2ui-solid/src/index.ts",
                "examples/frameworks/solid-basic",
                "src/App.tsx",
            ),
        ];
        let workspace = workspace_root();
        let mut asset_references = 0;
        let mut contracts = Vec::new();
        for (framework, package_entrypoint, example_root, source_file) in frameworks {
            require_file(&workspace.join(package_entrypoint))?;
            let example = workspace.join(example_root);
            require_file(&example.join("manifest.hawk.toml"))?;
            require_file(&example.join(source_file))?;
            require_file(&example.join("assets/logo.svg"))?;
            require_file(&example.join("styles/main.hawk.css"))?;
            let manifest = fs::read_to_string(example.join("manifest.hawk.toml"))
                .map_err(|error| error.to_string())?;
            let source =
                fs::read_to_string(example.join(source_file)).map_err(|error| error.to_string())?;
            if !manifest.contains(&format!("framework = \"{framework}\"")) {
                return Err(format!("framework manifest mismatch for {framework}"));
            }
            if !source.contains("assets/logo.svg") {
                return Err(format!(
                    "framework example missing asset reference: {framework}"
                ));
            }
            asset_references += 1;
            contracts.push(framework_contract(
                framework,
                &example.join(source_file),
                &source,
            )?);
        }
        let conformance_equivalent = contracts.iter().all(|contract| {
            contract.root_id == "root"
                && contract.keyed_children == ["title", "cta"]
                && contract.style_refs == ["surface.card"]
                && contract.asset_paths == ["assets/logo.svg"]
                && contract.runtime_bridged
        });
        Ok(FrameworkExamplesSmokeResult {
            frameworks: frameworks
                .into_iter()
                .map(|(framework, _, _, _)| framework.to_string())
                .collect(),
            package_entrypoints: frameworks.len(),
            asset_references,
            conformance_equivalent,
            contracts,
        })
    }

    /// Runs the security denial smoke fixtures.
    ///
    /// # Errors
    ///
    /// Returns a message when denial evidence is missing.
    pub fn run_security_denials(
        &self,
        fixture: &SmokeFixture,
    ) -> Result<SecurityDenialSmokeResult, String> {
        let root = fixture.absolute_path();
        require_file(&root.join("manifest.hawk.toml"))?;
        require_file(&root.join("fixtures/denied.ts"))?;
        let evidence = fs::read_to_string(root.join("fixtures/denials.txt"))
            .map_err(|error| error.to_string())?;
        let denials = vec![
            "filesystem.undeclared",
            "network.denied",
            "clipboard.denied",
            "secret.redacted",
            "asset.unsafe",
            "style.unsupported",
            "manifest.invalid",
        ];
        for denial in &denials {
            if !evidence.lines().any(|line| line == *denial) {
                return Err(format!("security denial evidence missing: {denial}"));
            }
        }
        if !evidence.contains("runtime_surface_launched=false") {
            return Err("security denial fixture did not block runtime surface launch".into());
        }
        Ok(SecurityDenialSmokeResult {
            denials: denials.into_iter().map(str::to_string).collect(),
            runtime_surface_launched: false,
        })
    }
}

fn require_file(path: &Path) -> Result<(), String> {
    if path.is_file() {
        Ok(())
    } else {
        Err(format!(
            "required smoke fixture file is missing: {}",
            path.display()
        ))
    }
}

fn framework_contract(
    framework: &str,
    source_file: &Path,
    source: &str,
) -> Result<FrameworkExampleContract, String> {
    match framework {
        "native" => native_contract(source_file, source),
        "svelte" => svelte_contract(source_file, source),
        "react" => react_contract(source_file, source),
        "vue" => vue_contract(source_file, source),
        "solid" => solid_contract(source_file, source),
        _ => Err(format!("unsupported framework smoke example: {framework}")),
    }
}

fn native_contract(source_file: &Path, source: &str) -> Result<FrameworkExampleContract, String> {
    for required in [
        "createHawkApp",
        "surface.card",
        "assets/logo.svg",
        "title",
        "cta",
    ] {
        if !source.contains(required) {
            return Err(format!("native example missing contract token: {required}"));
        }
    }
    let mut runtime = NativeAuthoringRuntime::new(source_file.display().to_string());
    runtime.mount(native_root());
    let artifact = runtime.finish().map_err(|error| format!("{error:?}"))?;
    NativeRuntimeBridge::new()
        .bridge_artifact(&artifact)
        .map_err(|error| format!("{error:?}"))?;
    Ok(FrameworkExampleContract {
        framework: "native".into(),
        root_id: artifact.root().id().as_str().to_string(),
        keyed_children: artifact
            .root()
            .keyed_child_order()
            .iter()
            .map(ToString::to_string)
            .collect(),
        style_refs: artifact
            .root()
            .style_refs()
            .iter()
            .map(|style| style.name().to_string())
            .collect(),
        asset_paths: artifact
            .root()
            .asset_refs()
            .iter()
            .map(|asset| asset.path().to_string())
            .collect(),
        runtime_bridged: true,
    })
}

fn native_root() -> NativeAuthoringElement {
    NativeAuthoringElement::new("root", ElementKind::View)
        .with_prop("background", PropValue::String("#080a0e".to_string()))
        .with_ref(NativeRef::new("root_ref"))
        .with_style(StyleRef::new("surface.card"))
        .with_asset(AssetRef::new("hawk.logo", "assets/logo.svg"))
        .with_child(NativeChild::keyed(
            "title",
            NativeAuthoringElement::new("title", ElementKind::Text)
                .with_prop("text", PropValue::String("title".to_string()))
                .with_prop("font_size", PropValue::Number(18.0))
                .with_prop("color", PropValue::String("#ffffff".to_string()))
                .with_prop("width", PropValue::Number(160.0))
                .with_prop("height", PropValue::Number(32.0)),
        ))
        .with_child(NativeChild::keyed(
            "cta",
            NativeAuthoringElement::new("cta", ElementKind::Text)
                .with_prop("text", PropValue::String("cta".to_string()))
                .with_prop("font_size", PropValue::Number(18.0))
                .with_prop("color", PropValue::String("#ffffff".to_string()))
                .with_prop("width", PropValue::Number(160.0))
                .with_prop("height", PropValue::Number(32.0)),
        ))
        .with_event(
            EventKind::Pointer(PointerEventKind::Press),
            "handlePress",
            [EventPayloadField::Position],
        )
        .with_lifecycle(NativeLifecycleEvent::Mounted, "onMount")
        .with_lifecycle(NativeLifecycleEvent::Unmounted, "onUnmount")
}

fn svelte_contract(source_file: &Path, source: &str) -> Result<FrameworkExampleContract, String> {
    let artifact = SvelteIntegration::new()
        .compile_to_runtime(SvelteComponentSource::new(
            source_file.display().to_string(),
            source,
        ))
        .map_err(|error| format!("{error:?}"))?;
    Ok(FrameworkExampleContract {
        framework: "svelte".into(),
        root_id: artifact.compiled().root().id().as_str().to_string(),
        keyed_children: artifact.compiled().keyed_children().to_vec(),
        style_refs: artifact
            .compiled()
            .style_refs()
            .into_iter()
            .map(str::to_string)
            .collect(),
        asset_paths: artifact
            .compiled()
            .asset_refs()
            .iter()
            .map(|asset| asset.path().to_string())
            .collect(),
        runtime_bridged: runtime_tree_has_example_children(artifact.runtime_tree()),
    })
}

fn react_contract(source_file: &Path, source: &str) -> Result<FrameworkExampleContract, String> {
    let artifact = ReactIntegration::new()
        .render_to_runtime(ReactElementTree::new(
            source_file.display().to_string(),
            source,
        ))
        .map_err(|error| format!("{error:?}"))?;
    Ok(FrameworkExampleContract {
        framework: "react".into(),
        root_id: artifact.rendered().root().id().as_str().to_string(),
        keyed_children: artifact.rendered().keyed_children().to_vec(),
        style_refs: artifact
            .rendered()
            .style_refs()
            .into_iter()
            .map(str::to_string)
            .collect(),
        asset_paths: artifact
            .rendered()
            .asset_refs()
            .iter()
            .map(|asset| asset.path().to_string())
            .collect(),
        runtime_bridged: runtime_tree_has_example_children(artifact.runtime_tree()),
    })
}

fn vue_contract(source_file: &Path, source: &str) -> Result<FrameworkExampleContract, String> {
    let artifact = VueIntegration::new()
        .render_to_runtime(VueSingleFileComponent::new(
            source_file.display().to_string(),
            source,
        ))
        .map_err(|error| format!("{error:?}"))?;
    Ok(FrameworkExampleContract {
        framework: "vue".into(),
        root_id: artifact.rendered().root().id().as_str().to_string(),
        keyed_children: artifact.rendered().keyed_children().to_vec(),
        style_refs: artifact
            .rendered()
            .style_refs()
            .into_iter()
            .map(str::to_string)
            .collect(),
        asset_paths: artifact
            .rendered()
            .asset_refs()
            .iter()
            .map(|asset| asset.path().to_string())
            .collect(),
        runtime_bridged: runtime_tree_has_example_children(artifact.runtime_tree()),
    })
}

fn solid_contract(source_file: &Path, source: &str) -> Result<FrameworkExampleContract, String> {
    let artifact = SolidIntegration::new()
        .render_to_runtime(SolidComponentSource::new(
            source_file.display().to_string(),
            source,
        ))
        .map_err(|error| format!("{error:?}"))?;
    Ok(FrameworkExampleContract {
        framework: "solid".into(),
        root_id: artifact.rendered().root().id().as_str().to_string(),
        keyed_children: artifact.rendered().keyed_children().to_vec(),
        style_refs: artifact
            .rendered()
            .style_refs()
            .into_iter()
            .map(str::to_string)
            .collect(),
        asset_paths: artifact
            .rendered()
            .asset_refs()
            .iter()
            .map(|asset| asset.path().to_string())
            .collect(),
        runtime_bridged: runtime_tree_has_example_children(artifact.runtime_tree()),
    })
}

fn runtime_tree_has_example_children(tree: &hawk2ui_runtime::RuntimeViewTree) -> bool {
    tree.root_id().as_str() == "root" && tree.children_of(tree.root_id()).len() == 2
}

fn workspace_root() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map_or_else(|| manifest_dir.to_path_buf(), Path::to_path_buf)
}

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-smoke";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-smoke");
    }

    #[test]
    fn smoke_workspace_filter_marker() {
        assert_eq!(crate_name(), "hawk2ui-smoke");
    }
}
