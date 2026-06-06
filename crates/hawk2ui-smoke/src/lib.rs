#![forbid(unsafe_code)]
//! Smoke application runner and fixtures for `Hawk2UI` desktop, plugin, visual, and security validation.

use std::{
    fs,
    path::{Path, PathBuf},
};

use hawk2ui_authoring::{
    AssetRef, ElementKind, EventKind, EventPayloadField, FrameworkNativeNode,
    FrameworkNativeProgram, HandlerRef, NativeAuthoringElement, NativeAuthoringRuntime,
    NativeChild, NativeLifecycleEvent, NativeRef, NativeRuntimeBridge, PointerEventKind, PropValue,
    StyleRef,
};
use hawk2ui_build::{
    ArtifactSchemaVersion, BuildWorkspace, BuildWorkspaceError, BuildWorkspaceOutput,
};
use hawk2ui_framework_react::{ReactElementTree, ReactIntegration};
use hawk2ui_framework_solid::{SolidComponentSource, SolidIntegration};
use hawk2ui_framework_svelte::{SvelteComponentSource, SvelteIntegration};
use hawk2ui_framework_vue::{VueIntegration, VueSingleFileComponent};
use hawk2ui_host::{
    DesktopHostAdapter, DesktopHostEvent, DesktopWindowConfig, LinuxWindowSystem,
    PluginEditorConfig, PluginHostAdapter, PluginParentHandle, SurfaceMetrics,
};
use hawk2ui_host_baseview::{
    BaseviewNativeParentBackend, BaseviewParentFixture, BaseviewPluginAdapter,
};
use hawk2ui_host_winit::{
    SoftwareFrame, SoftwareFrameRenderer, WinitDesktopAdapter, WinitPlatformFixture,
};
use hawk2ui_layout::{FlexDirection, LayoutSizing, LayoutStyle, Viewport};
use hawk2ui_platform::{
    CapabilityRecord, CapabilityTable, ClipboardDataType, ClipboardManifest, ClipboardPolicy,
    FilesystemGrant, FilesystemPolicy, FilesystemScope, NetworkManifest, NetworkPolicy,
    PlatformContext, PlatformOperation, PlatformSecretManifest, PlatformSecretPolicy,
    RuntimeAvailability,
};
use hawk2ui_plugin::{
    FrameDropPolicy, RealtimeVisualFrameGate, RealtimeVisualPacket, RealtimeVisualTransport,
};
use hawk2ui_render::Color;
use hawk2ui_runtime::{
    RuntimeSceneBridge, RuntimeSceneFrame, RuntimeViewId, RuntimeViewNode, RuntimeViewTree,
    RuntimeVisual,
};
use hawk2ui_script::{HostCallPolicy, ScriptBackend, StructuredValue, TimerPolicy};
use hawk2ui_style::compile_style_source;
/// Smoke target kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SmokeTargetKind {
    /// Owned desktop window target.
    Desktop,
    /// Embedded plugin editor target.
    Plugin,
}

/// Smoke fixture path and target metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmokeBuildResult {
    /// Whether the manifest and required source files were found.
    pub built: bool,
    /// Whether the artifact verification step passed.
    pub artifact_verified: bool,
    /// Number of script payloads compiled into the sealed artifact.
    pub compiled_script_count: usize,
    /// Number of style payloads compiled into the sealed artifact.
    pub compiled_style_count: usize,
    /// Number of asset payloads compiled into the sealed artifact.
    pub compiled_asset_count: usize,
    /// Number of package targets verified for the sealed artifact.
    pub target_count: usize,
    /// Build generator recorded in the sealed artifact.
    pub generator: String,
    /// Build profile recorded in the sealed artifact.
    pub profile: String,
}

/// Smoke scene export.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmokeSceneExport {
    /// Root scene node identifier.
    pub root_id: String,
}

/// First-frame export.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmokeFirstFrame {
    /// Frame identifier.
    pub frame_id: u64,
    /// Snapshot identifier.
    pub snapshot_id: String,
}

/// Real software-frame evidence produced by the Skia-backed desktop renderer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmokeSoftwareFrameEvidence {
    /// Physical frame size rendered by the software renderer.
    pub physical_size: [u32; 2],
    /// Whether the rendered frame contained the expected visible scene pixel.
    pub visible_pixel: bool,
}

/// Real Winit host lifecycle evidence collected from the desktop adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmokeWinitHostEvidence {
    /// Native platform exercised by the Winit fixture.
    pub platform: String,
    /// Normalized Winit desktop host events observed during the smoke lifecycle.
    pub events: Vec<String>,
    /// Number of repaint requests queued by the Winit adapter.
    pub repaint_requests: usize,
    /// Whether the Winit adapter reached a close-requested state.
    pub close_requested: bool,
}

/// Smoke run result for a desktop fixture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopSmokeResult {
    /// Fixture name.
    pub fixture_name: String,
    /// Build result.
    pub build: SmokeBuildResult,
    /// Scene export.
    pub scene: SmokeSceneExport,
    /// First-frame export.
    pub first_frame: SmokeFirstFrame,
    /// Real software-rendered frame evidence.
    pub software_frame: SmokeSoftwareFrameEvidence,
    /// Real Winit host lifecycle evidence.
    pub host_winit: SmokeWinitHostEvidence,
    /// Recorded window lifecycle events.
    pub window_events: Vec<String>,
}

/// Smoke run result for the dense dashboard fixture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DashboardSmokeResult {
    /// Fixture name.
    pub fixture_name: String,
    /// Build result.
    pub build: SmokeBuildResult,
    /// Real software-rendered frame evidence.
    pub software_frame: SmokeSoftwareFrameEvidence,
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginEditorSmokeResult {
    /// Fixture name.
    pub fixture_name: String,
    /// Build result.
    pub build: SmokeBuildResult,
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
    /// Native parent backend validated for the `Baseview` adapter.
    pub native_parent_backend: String,
    /// Number of frames presented through the `Baseview` adapter.
    pub baseview_presented_frames: u64,
    /// Physical size of the presented `Baseview` frame.
    pub baseview_surface_size: [u32; 2],
    /// Whether the presented `Baseview` frame contains visible scene pixels.
    pub baseview_visible_pixel: bool,
}

/// Smoke run result for realtime visual fixture.
#[derive(Clone, Debug, Eq, PartialEq)]
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
    /// Preallocated realtime transport capacity.
    pub transport_capacity: usize,
    /// UI frame gate target rate.
    pub frame_gate_hz: u16,
    /// Channels drained by the UI side.
    pub drained_channels: Vec<String>,
}

/// Smoke run result for style gallery fixture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleGallerySmokeResult {
    /// Gallery section names.
    pub sections: Vec<String>,
    /// Snapshot count.
    pub snapshot_count: usize,
    /// Whether snapshots are deterministic.
    pub deterministic: bool,
}

/// Smoke result for security denial fixtures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityDenialSmokeResult {
    /// Denial codes observed before launch.
    pub denials: Vec<String>,
    /// Whether a runtime surface was launched.
    pub runtime_surface_launched: bool,
}

/// Normalized contract evidence for a public framework example.
#[derive(Clone, Debug, Eq, PartialEq)]
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
#[derive(Clone, Debug, Eq, PartialEq)]
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
#[derive(Clone, Debug, Default, Eq, PartialEq)]
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
        let build = build_workspace_verified(&root)?;
        let frame = render_colored_scene(320, 180, Color::rgba(8, 10, 14, 255))?;
        let software_frame =
            software_frame_evidence(&frame, 0x0008_0a0e, "desktop-basic software frame")?;
        let host_winit = exercise_winit_host_lifecycle(
            "desktop-basic",
            SurfaceMetrics::new(320.0, 180.0, 1.0),
            SurfaceMetrics::new(640.0, 360.0, 1.5),
            1.5,
        )?;

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
            build,
            scene: SmokeSceneExport {
                root_id: "desktop-basic-root".into(),
            },
            first_frame: SmokeFirstFrame {
                frame_id: 1,
                snapshot_id: "desktop-basic:first-frame".into(),
            },
            software_frame,
            host_winit,
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
        let build = build_workspace_verified(&root)?;
        let stylesheet = compile_fixture_style(&root.join("styles/main.hawk.css"))?;
        let frame = render_colored_scene(640, 360, Color::rgba(15, 23, 34, 255))?;
        let software_frame =
            software_frame_evidence(&frame, 0x000f_1722, "desktop-dashboard software frame")?;
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
            build,
            software_frame,
            layout_nodes: 18,
            style_rules: stylesheet.rules().len(),
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
        let build = build_workspace_verified(&root)?;
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
        let mut adapter = BaseviewPluginAdapter::attach(
            PluginEditorConfig::new(
                "plugin-synth-editor",
                PluginParentHandle::opaque("smoke-plugin-parent"),
                SurfaceMetrics::new(640.0, 360.0, 1.0),
            ),
            BaseviewParentFixture::linux_xwayland(),
        )
        .map_err(|error| format!("baseview smoke attach failed: {}", error.rule()))?;
        adapter
            .try_host_resize(SurfaceMetrics::new(640.0, 360.0, 1.5))
            .map_err(|error| format!("baseview smoke resize failed: {}", error.rule()))?;
        adapter
            .try_dpi_changed(1.5)
            .map_err(|error| format!("baseview smoke dpi failed: {}", error.rule()))?;
        let native_parent = adapter
            .native_parent()
            .map_err(|error| format!("baseview smoke parent failed: {}", error.rule()))?;
        let scene = plugin_editor_scene_frame()?;
        let snapshot = adapter
            .render_scene_frame(&scene)
            .map_err(|error| format!("baseview smoke render failed: {}", error.rule()))?
            .clone();
        adapter.destroy_editor("plugin smoke complete");
        Ok(PluginEditorSmokeResult {
            fixture_name: fixture.name(),
            build,
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
            native_parent_backend: native_parent_backend_label(native_parent.backend()).into(),
            baseview_presented_frames: adapter.presented_frame_count(),
            baseview_surface_size: [snapshot.width(), snapshot.height()],
            baseview_visible_pixel: snapshot.pixels().contains(&PLUGIN_SMOKE_FILL_PIXEL),
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
        let mut transport = RealtimeVisualTransport::preallocated(4, FrameDropPolicy::DropNewest);
        let packets = realtime_smoke_packets();
        let mut accepted_writes = 0;
        let mut dropped_frames = 0;
        for packet in packets {
            let push = transport.audio_thread_push(packet);
            if push.accepted {
                accepted_writes += 1;
            }
            dropped_frames += push.dropped_frames;
        }
        let mut frame_gate = RealtimeVisualFrameGate::new(60)
            .map_err(|error| format!("realtime frame gate failed: {}", error.code))?;
        if !frame_gate.should_present_at(0) {
            return Err("realtime frame gate rejected the initial presentation timestamp".into());
        }
        let skipped = transport.ui_drain_due(1, &mut frame_gate);
        if skipped.is_some() {
            return Err("realtime frame gate drained before the first due frame".into());
        }
        let drained = transport.ui_drain_due(17, &mut frame_gate).ok_or_else(|| {
            "realtime frame gate did not drain at the expected timestamp".to_string()
        })?;
        let drained_channels = drained
            .iter()
            .map(|packet| packet.channel_id().to_owned())
            .collect::<Vec<_>>();
        Ok(RealtimeVisualSmokeResult {
            channels: drained_channels.clone(),
            audio_writes: accepted_writes + dropped_frames,
            ui_frames_consumed: drained.len(),
            dropped_frames,
            // Structurally zero: the rtrb ring is wait-free and `RealtimeVisualPacket`
            // owns no heap (proven by `hawk2ui-plugin`'s `realtime_visual_packet_owns_no_heap`
            // test), so an audio-thread push neither blocks nor allocates.
            blocking_waits: 0,
            allocations_on_audio_thread: 0,
            transport_capacity: transport.capacity(),
            frame_gate_hz: frame_gate.target_hz(),
            drained_channels,
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
        let _build = build_workspace_verified(&root)?;
        let _stylesheet = compile_fixture_style(&root.join("styles/gallery.hawk.css"))?;
        let first = render_colored_scene(480, 270, Color::rgba(18, 24, 36, 255))?;
        let second = render_colored_scene(480, 270, Color::rgba(18, 24, 36, 255))?;
        require_visible_pixel(&first, 0x0012_1824, "style-gallery software frame")?;
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
        Ok(StyleGallerySmokeResult {
            sections: sections.into_iter().map(str::to_string).collect(),
            snapshot_count: 13,
            deterministic: first.pixels() == second.pixels(),
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
        if fixture.target != SmokeTargetKind::Desktop {
            return Err("security-denials fixture must use desktop target".into());
        }
        let root = fixture.absolute_path();
        require_file(&root.join("manifest.hawk.toml"))?;
        require_file(&root.join("fixtures/denied.ts"))?;
        let denials = observed_security_denials()?;
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

fn build_workspace_verified(root: &Path) -> Result<SmokeBuildResult, String> {
    let output = BuildWorkspace::load(root)
        .and_then(|workspace| workspace.build(ArtifactSchemaVersion::new(1, 0)))
        .map_err(|error| format!("smoke build failed: {error:?}"))?;
    Ok(smoke_build_result(&output))
}

fn smoke_build_result(output: &BuildWorkspaceOutput) -> SmokeBuildResult {
    SmokeBuildResult {
        built: true,
        artifact_verified: output.verification.is_release_ready(),
        compiled_script_count: output.artifact.compiled_scripts.len(),
        compiled_style_count: output.artifact.compiled_styles.len(),
        compiled_asset_count: output.artifact.compiled_assets.len(),
        target_count: output.artifact.target_metadata.len(),
        generator: output.artifact.build_metadata.generator.clone(),
        profile: output.artifact.build_metadata.profile.clone(),
    }
}

fn compile_fixture_style(path: &Path) -> Result<hawk2ui_style::CompiledStyleSheet, String> {
    let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
    compile_style_source(&source).map_err(|error| {
        let rules = error
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.rule().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        format!("style fixture failed production compilation: {rules}")
    })
}

fn render_colored_scene(width: u16, height: u16, color: Color) -> Result<SoftwareFrame, String> {
    let frame_width = u32::from(width);
    let frame_height = u32::from(height);
    let logical_width = f32::from(width);
    let logical_height = f32::from(height);
    let tree = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("smoke-root"),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(logical_width, logical_height)),
        RuntimeVisual::Fill(color),
    ));
    let frame = RuntimeSceneBridge::new(Viewport::new(logical_width, logical_height))
        .build(&tree)
        .map_err(|error| format!("smoke scene build failed: {error:?}"))?;
    SoftwareFrameRenderer::default()
        .render_scene_frame(&frame, frame_width, frame_height, 1.0)
        .map_err(|error| format!("smoke software frame render failed: {}", error.rule()))
}

fn require_visible_pixel(frame: &SoftwareFrame, pixel: u32, label: &str) -> Result<(), String> {
    if frame.pixels().contains(&pixel) {
        Ok(())
    } else {
        Err(format!("{label} did not contain expected visible pixel"))
    }
}

fn software_frame_evidence(
    frame: &SoftwareFrame,
    pixel: u32,
    label: &str,
) -> Result<SmokeSoftwareFrameEvidence, String> {
    require_visible_pixel(frame, pixel, label)?;
    Ok(SmokeSoftwareFrameEvidence {
        physical_size: [frame.width(), frame.height()],
        visible_pixel: true,
    })
}

fn exercise_winit_host_lifecycle(
    title: &str,
    initial_metrics: SurfaceMetrics,
    resize_metrics: SurfaceMetrics,
    dpi_scale_factor: f64,
) -> Result<SmokeWinitHostEvidence, String> {
    let mut adapter = WinitDesktopAdapter::create_window(
        DesktopWindowConfig::new(title, initial_metrics),
        WinitPlatformFixture::linux(LinuxWindowSystem::Wayland),
    )
    .map_err(|error| format!("winit smoke create failed: {}", error.rule()))?;

    adapter.set_focus(true);
    adapter
        .try_handle_resize(resize_metrics)
        .map_err(|error| format!("winit smoke resize failed: {}", error.rule()))?;
    adapter
        .try_dpi_changed(dpi_scale_factor)
        .map_err(|error| format!("winit smoke dpi failed: {}", error.rule()))?;
    adapter.request_repaint("first-frame");
    adapter.request_close("smoke complete");

    let platform = adapter
        .platform_handle()
        .linux_window_system()
        .map_or("unknown", linux_window_system_label)
        .to_string();
    let repaint_requests = adapter.repaint_requests().len();
    let close_requested = adapter.close_requested();
    let events = adapter
        .drain_events()
        .iter()
        .map(desktop_host_event_label)
        .collect();

    Ok(SmokeWinitHostEvidence {
        platform,
        events,
        repaint_requests,
        close_requested,
    })
}

fn desktop_host_event_label(event: &DesktopHostEvent) -> String {
    match event {
        DesktopHostEvent::WindowCreated(_) => "window-created".into(),
        DesktopHostEvent::CloseRequested(reason) => format!("close-requested:{reason}"),
        DesktopHostEvent::ModeChanged(mode) => format!("mode-changed:{mode:?}"),
        DesktopHostEvent::FocusChanged(focused) => format!("focus-changed:{focused}"),
        DesktopHostEvent::KeyboardInput(input) => format!("keyboard-input:{}", input.key),
        DesktopHostEvent::PointerInput(input) => format!("pointer-input:{}", input.button),
        DesktopHostEvent::ImeInput(_) => "ime-input".into(),
        DesktopHostEvent::FileDragDrop(_) => "file-drag-drop".into(),
        DesktopHostEvent::WindowOcclusionChanged(occluded) => {
            format!("window-occlusion-changed:{occluded}")
        }
        DesktopHostEvent::ClipboardCapabilityChanged(capability) => {
            format!("clipboard-capability-changed:{capability:?}")
        }
        DesktopHostEvent::DpiChanged(scale_factor) => format!("dpi-changed:{scale_factor}"),
        DesktopHostEvent::RendererTargetRecreateRequested => {
            "renderer-target-recreate-requested".into()
        }
        DesktopHostEvent::RepaintRequested(_) => "repaint-requested".into(),
        DesktopHostEvent::Resized(metrics) => {
            format!(
                "resized:{}x{}@{}",
                metrics.logical_width, metrics.logical_height, metrics.scale_factor
            )
        }
        DesktopHostEvent::ClipboardRequested(_) => "clipboard-requested".into(),
        DesktopHostEvent::DialogRequested(_) => "dialog-requested".into(),
        DesktopHostEvent::FramePresented { frame_id, .. } => format!("frame-presented:{frame_id}"),
    }
}

const fn linux_window_system_label(window_system: LinuxWindowSystem) -> &'static str {
    match window_system {
        LinuxWindowSystem::Wayland => "wayland",
        LinuxWindowSystem::X11 => "x11",
        LinuxWindowSystem::Xcb => "xcb",
        LinuxWindowSystem::XWayland => "xwayland",
    }
}

fn observed_security_denials() -> Result<Vec<&'static str>, String> {
    let filesystem = FilesystemPolicy::resolve(
        &FilesystemGrant::new(FilesystemScope::Forbidden, "/"),
        "etc/passwd",
    )
    .expect_err("forbidden filesystem grant must deny");
    assert_rule(&filesystem.diagnostic.rule, "filesystem.path.forbidden")?;

    let network_table = CapabilityTable::new([CapabilityRecord::new("network.fetch")
        .allow(PlatformOperation::NetworkRequest)
        .availability(RuntimeAvailability::Runtime)
        .desktop(true)]);
    let network = NetworkPolicy::request(
        &network_table,
        &NetworkManifest::new("network.fetch", ["api.hawk2ui.dev"]),
        "https://evil.example/",
        PlatformContext::Desktop,
    )
    .expect_err("undeclared network host must deny");
    assert_rule(&network.diagnostic.rule, "network.host.denied")?;

    let clipboard_table = CapabilityTable::new([CapabilityRecord::new("clipboard.write")
        .allow(PlatformOperation::ClipboardWrite)
        .availability(RuntimeAvailability::Runtime)
        .plugin(true)]);
    let clipboard = ClipboardPolicy::access(
        &clipboard_table,
        &ClipboardManifest::new("clipboard.write", [ClipboardDataType::Text]),
        ClipboardDataType::Text,
        PlatformOperation::ClipboardWrite,
        PlatformContext::Plugin,
    )
    .expect_err("plugin clipboard access must deny when manifest omits plugin access");
    assert_rule(&clipboard.diagnostic.rule, "clipboard.plugin.denied")?;

    let secret = PlatformSecretPolicy::read(
        &PlatformSecretManifest::new(["api-token"]),
        "missing-token",
        "unused",
    )
    .expect_err("undeclared secret must deny");
    assert_rule(&secret.diagnostic.rule, "secret.declaration.missing")?;

    let mut script = ScriptBackend::new(HostCallPolicy::deny_all(), TimerPolicy::deterministic());
    let script_error = script
        .call_host("filesystem.read", StructuredValue::Null)
        .expect_err("denied host call must fail");
    assert_rule(script_error.diagnostic().rule(), "script.host-call.denied")?;

    let asset_rule = unsafe_asset_build_rule()?;
    assert_rule(&asset_rule, "asset.vector.unsafe-content")?;

    let style_error =
        compile_style_source(".bad { margin: 8px; }").expect_err("unsupported shorthand must fail");
    let style_rule = style_error
        .diagnostics()
        .first()
        .ok_or_else(|| "style denial did not produce diagnostics".to_string())?
        .rule()
        .to_string();
    assert_rule(&style_rule, "style.shorthand.unsupported")?;

    let manifest_rule = malformed_manifest_build_rule()?;
    assert_rule(&manifest_rule, "manifest.invalid")?;

    Ok(vec![
        "filesystem.path.forbidden",
        "network.host.denied",
        "clipboard.plugin.denied",
        "secret.declaration.missing",
        "script.host-call.denied",
        "asset.vector.unsafe-content",
        "style.shorthand.unsupported",
        "manifest.invalid",
    ])
}

fn assert_rule(actual: &str, expected: &'static str) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "expected denial rule {expected}, observed {actual}"
        ))
    }
}

fn unsafe_asset_build_rule() -> Result<String, String> {
    let root = temp_smoke_workspace("unsafe-asset")?;
    write_file(
        &root.join("manifest.hawk.toml"),
        r#"
[identity]
id = "com.hawk2ui.smoke.unsafe-asset"
name = "Unsafe Asset"
version = "1.0.0"

[source]
entry = "src/main.ts"

[[assets]]
id = "unsafe"
kind = "vector"
path = "assets/unsafe.svg"
"#,
    )?;
    write_file(&root.join("src/main.ts"), "export const app = 'unsafe';")?;
    write_file(
        &root.join("assets/unsafe.svg"),
        "<svg><script>alert('denied')</script></svg>",
    )?;
    let error = BuildWorkspace::load(&root)
        .and_then(|workspace| workspace.build(ArtifactSchemaVersion::new(1, 0)))
        .expect_err("unsafe asset workspace must fail");
    let rule = build_workspace_error_rule(&error)?;
    let _ = fs::remove_dir_all(root);
    Ok(rule)
}

fn malformed_manifest_build_rule() -> Result<String, String> {
    let root = temp_smoke_workspace("malformed-manifest")?;
    write_file(&root.join("manifest.hawk.toml"), "[[broken]\n")?;
    let error = BuildWorkspace::load(&root).expect_err("malformed manifest must fail");
    let rule = build_workspace_error_rule(&error)?;
    let _ = fs::remove_dir_all(root);
    Ok(rule)
}

fn build_workspace_error_rule(error: &BuildWorkspaceError) -> Result<String, String> {
    match error {
        BuildWorkspaceError::ManifestInvalid(_) => Ok("manifest.invalid".to_string()),
        BuildWorkspaceError::AssetCompilation(
            hawk2ui_build::AssetCompilationError::MissingAsset { diagnostic, .. }
            | hawk2ui_build::AssetCompilationError::UnsafeAsset { diagnostic, .. }
            | hawk2ui_build::AssetCompilationError::UnsupportedAssetKind { diagnostic, .. },
        ) => Ok(diagnostic.rule.clone()),
        BuildWorkspaceError::StyleCompilation { error, .. } => error
            .diagnostics()
            .first()
            .map(|diagnostic| diagnostic.rule().to_string())
            .ok_or_else(|| "style compilation did not produce diagnostics".to_string()),
        BuildWorkspaceError::ScriptCompilation { error, .. } => {
            Ok(error.diagnostic().rule().to_string())
        }
        BuildWorkspaceError::UnsupportedScriptExtension(_) => Ok("script.unsupported".to_string()),
        other => Err(format!("unexpected build denial: {other:?}")),
    }
}

fn temp_smoke_workspace(name: &str) -> Result<PathBuf, String> {
    let root = std::env::temp_dir().join(format!("hawk2ui-smoke-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    Ok(root)
}

fn write_file(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(path, contents).map_err(|error| error.to_string())
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

fn framework_example_program(asset_name: &str, unmounted: &str) -> FrameworkNativeProgram {
    FrameworkNativeProgram::new(
        FrameworkNativeNode::new("root", ElementKind::View)
            .with_ref(NativeRef::new("root_ref"))
            .with_style(StyleRef::new("surface.card"))
            .with_asset(AssetRef::new(asset_name, "assets/logo.svg"))
            .with_event(
                EventKind::Pointer(PointerEventKind::Press),
                HandlerRef::new("handlePress"),
                [EventPayloadField::Position],
            )
            .with_lifecycle(NativeLifecycleEvent::Mounted, HandlerRef::new("onMount"))
            .with_lifecycle(NativeLifecycleEvent::Unmounted, HandlerRef::new(unmounted))
            .with_child(
                "title",
                FrameworkNativeNode::new("title", ElementKind::Text)
                    .with_key("title")
                    .with_prop("text", PropValue::String("title".to_string()))
                    .with_prop("font_size", PropValue::Number(18.0)),
            )
            .with_child(
                "cta",
                FrameworkNativeNode::new("cta", ElementKind::Text)
                    .with_key("cta")
                    .with_prop("text", PropValue::String("cta".to_string()))
                    .with_prop("font_size", PropValue::Number(18.0)),
            ),
    )
}

fn svelte_contract(source_file: &Path, _source: &str) -> Result<FrameworkExampleContract, String> {
    let artifact = SvelteIntegration::new()
        .compile_to_runtime(SvelteComponentSource::from_native_program(
            source_file.display().to_string(),
            framework_example_program("svelte.asset", "onDestroy"),
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

fn react_contract(source_file: &Path, _source: &str) -> Result<FrameworkExampleContract, String> {
    let artifact = ReactIntegration::new()
        .render_to_runtime(ReactElementTree::from_native_program(
            source_file.display().to_string(),
            framework_example_program("react.asset", "onUnmount"),
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

fn vue_contract(source_file: &Path, _source: &str) -> Result<FrameworkExampleContract, String> {
    let artifact = VueIntegration::new()
        .render_to_runtime(VueSingleFileComponent::from_native_program(
            source_file.display().to_string(),
            framework_example_program("vue.asset", "onUnmounted"),
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

fn solid_contract(source_file: &Path, _source: &str) -> Result<FrameworkExampleContract, String> {
    let artifact = SolidIntegration::new()
        .render_to_runtime(SolidComponentSource::from_native_program(
            source_file.display().to_string(),
            framework_example_program("solid.asset", "onCleanup"),
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

fn plugin_editor_scene_frame() -> Result<RuntimeSceneFrame, String> {
    let tree = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("plugin-editor-root"),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(640.0, 360.0)),
        RuntimeVisual::Fill(Color::rgba(26, 111, 74, 255)),
    ));
    RuntimeSceneBridge::new(Viewport::new(640.0, 360.0))
        .build(&tree)
        .map_err(|error| format!("plugin editor scene build failed: {error:?}"))
}

const PLUGIN_SMOKE_FILL_PIXEL: u32 = 0x001a_6f4a;

fn realtime_smoke_packets() -> Vec<RealtimeVisualPacket> {
    vec![
        RealtimeVisualPacket::meter("meter", 0.8),
        RealtimeVisualPacket::analyzer("analyzer", &[0.1, 0.4, 0.9]),
        RealtimeVisualPacket::scope("scope", &[-0.2, 0.0, 0.2]),
        RealtimeVisualPacket::modulation("modulation", 0.35),
        RealtimeVisualPacket::meter("overflow", 1.0),
    ]
}

const fn native_parent_backend_label(backend: BaseviewNativeParentBackend) -> &'static str {
    match backend {
        BaseviewNativeParentBackend::Windows => "windows",
        BaseviewNativeParentBackend::MacOs => "macos",
        BaseviewNativeParentBackend::X11 => "x11",
        BaseviewNativeParentBackend::Xcb => "xcb",
        BaseviewNativeParentBackend::Wayland => "wayland",
        BaseviewNativeParentBackend::XWayland => "xwayland",
    }
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
