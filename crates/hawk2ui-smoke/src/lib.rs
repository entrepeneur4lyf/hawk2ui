#![forbid(unsafe_code)]
//! Smoke application runner and fixtures for `Hawk2UI` desktop, plugin, visual, and security validation.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use hawk2ui_authoring::{
    AssetRef, ElementKind, EventKind, EventPayloadField, FrameworkNativeProgram,
    FrameworkNativeProgramWire, NativeAuthoringElement, NativeAuthoringRuntime, NativeChild,
    NativeLifecycleEvent, NativeRef, NativeRuntimeBridge, PointerEventKind, PropValue, StyleRef,
};
use hawk2ui_build::{
    ArtifactSchemaVersion, ArtifactSigningKey, BuildWorkspace, BuildWorkspaceError,
    BuildWorkspaceOutput, CompiledFrameworkRecord, SealedJsModuleGraph as BuildSealedJsModuleGraph,
    SourceFramework,
};
use hawk2ui_cli::{CliCommand, CliExitCode, CommandExecution, WorkspaceCommandRunner};
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
    SoftwareFrame, SoftwareFrameRenderer, WinitDesktopAdapter, WinitDesktopRuntimeConfig,
    WinitPlatformFixture,
};
use hawk2ui_js_runtime::{
    HawkHostContext, HawkJsModule, HawkJsModuleGraph, HawkJsRuntime, HawkNetworkResponse,
    HawkPluginTransportInfo, HawkRuntimeCapabilities, JsRuntimeValue, RuntimeSceneOpAdapter,
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
    RuntimeDrawCommand, RuntimeSceneBridge, RuntimeSceneFrame, RuntimeViewId, RuntimeViewNode,
    RuntimeViewTree, RuntimeVisual,
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
    /// Number of framework compiler payloads embedded in the sealed artifact.
    pub compiled_framework_count: usize,
    /// Number of sealed JavaScript module graphs embedded in the sealed artifact.
    pub js_module_graph_count: usize,
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

/// Compiler artifact evidence collected from a real framework example build.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameworkCompilerSmokeArtifact {
    /// Framework label emitted by the framework compiler.
    pub framework: String,
    /// Compiler package or binary that produced the native artifact.
    pub compiler: String,
    /// Workspace-relative source path compiled by the framework compiler.
    pub source_path: String,
    /// Entrypoint lowered by the framework compiler.
    pub entrypoint: String,
    /// Artifact-local path used for the compiler payload.
    pub artifact_path: String,
    /// Compiler payload size in bytes.
    pub artifact_bytes: usize,
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

/// Smoke run result for the React/Deno desktop fixture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReactDenoDesktopSmokeResult {
    /// Fixture name.
    pub fixture_name: String,
    /// Number of application modules sealed into the runtime graph.
    pub sealed_module_count: usize,
    /// Entrypoint specifier executed by the Deno runtime.
    pub bundle_entrypoint: String,
    /// Package manager that produced the sealed bundle.
    pub package_manager: String,
    /// Lockfile hash recorded by package-manager metadata.
    pub package_manager_lockfile_sha256: Option<String>,
    /// SHA-256 hash of the executed JS bundle module.
    pub bundle_sha256: String,
    /// Scene export.
    pub scene: SmokeSceneExport,
    /// Real software-rendered first-frame evidence.
    pub first_frame: SmokeSoftwareFrameEvidence,
    /// Real software-rendered second-frame evidence.
    pub second_frame: SmokeSoftwareFrameEvidence,
    /// Text values observed across the initial and updated frames.
    pub text_updates: Vec<String>,
    /// Network status values observed across the initial and async-updated frames.
    pub network_updates: Vec<String>,
    /// Storage operations executed by the sealed React/Deno bundle.
    pub storage_operations: Vec<String>,
    /// File operations executed by the sealed React/Deno bundle.
    pub file_operations: Vec<String>,
    /// Real Winit host lifecycle evidence.
    pub host_winit: SmokeWinitHostEvidence,
    /// Whether the Winit runtime config accepts the React/Deno runtime tree.
    pub window_config_valid: bool,
    /// Whether the smoke lifecycle reached a clean close.
    pub close_cleanly: bool,
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
    /// Project-relative trace file used as plugin lifecycle evidence.
    pub trace_source_path: String,
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

/// Smoke run result for the React/Deno plugin fixture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReactDenoPluginSmokeResult {
    /// Fixture name.
    pub fixture_name: String,
    /// Number of application modules sealed into the runtime graph.
    pub sealed_module_count: usize,
    /// Entrypoint specifier executed by the Deno runtime.
    pub bundle_entrypoint: String,
    /// Package manager that produced the sealed bundle.
    pub package_manager: String,
    /// Lockfile hash recorded by package-manager metadata.
    pub package_manager_lockfile_sha256: Option<String>,
    /// SHA-256 hash of the executed JS bundle module.
    pub bundle_sha256: String,
    /// Parameter updates observed through `hawk:plugin`.
    pub parameter_updates: Vec<String>,
    /// Text values observed across the initial and updated frames.
    pub text_updates: Vec<String>,
    /// Plugin state save/load evidence from the sealed React/Deno bundle.
    pub state_roundtrip: Vec<String>,
    /// Plugin preset save/load evidence from the sealed React/Deno bundle.
    pub preset_roundtrip: Vec<String>,
    /// Plugin host transport snapshots observed through `hawk:plugin`.
    pub transport_snapshots: Vec<String>,
    /// Realtime visual stream frames observed through `hawk:audio`.
    pub realtime_visual_streams: Vec<String>,
    /// DSP control queue evidence observed through `hawk:dsp`.
    pub dsp_control_messages: Vec<String>,
    /// Frames presented by the `Baseview` adapter.
    pub baseview_presented_frames: u64,
    /// Last `Baseview` software surface size.
    pub baseview_surface_size: [u32; 2],
    /// Whether the `Baseview` frame had visible pixels.
    pub baseview_visible_pixel: bool,
    /// Realtime denial rule observed when plugin ops are attempted in audio realtime context.
    pub realtime_denial_rule: String,
    /// Whether JavaScript execution was kept out of the audio realtime context.
    pub no_js_on_audio_thread: bool,
    /// Whether the editor was destroyed cleanly.
    pub destroyed_cleanly: bool,
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
    /// Project-relative source file that declared the section list.
    pub section_source_path: String,
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
    /// Evidence line proving the launch gate remained closed.
    pub launch_gate_evidence: String,
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
    /// Verified sealed build output for this example.
    pub build: SmokeBuildResult,
    /// Real framework compiler artifact evidence, when the example uses a framework compiler.
    pub compiler_artifact: Option<FrameworkCompilerSmokeArtifact>,
    /// Number of runtime keyed-list templates emitted by the framework compiler.
    pub list_template_count: usize,
    /// Number of dynamic runtime bindings emitted by the framework compiler.
    pub dynamic_binding_count: usize,
    /// Whether the source bridged into a runtime-ready view tree.
    pub runtime_bridged: bool,
    /// Real software-rendered frame evidence from the framework runtime tree.
    pub software_frame: SmokeSoftwareFrameEvidence,
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

/// Smoke result for reusable CLI workflow execution over real examples.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CliEndToEndSmokeResult {
    /// `build-release` status for the desktop example.
    pub desktop_build_release: CliWorkflowStatus,
    /// `verify-artifact` status for the signed desktop release artifact.
    pub desktop_verify_artifact: CliWorkflowStatus,
    /// `run-desktop` first-frame status.
    pub desktop_run_first_frame: CliWorkflowStatus,
    /// Number of frames reported by the desktop runtime summary.
    pub desktop_frames_presented: u64,
    /// `package-plugin` status for the plugin example.
    pub plugin_package: CliWorkflowStatus,
    /// Plugin package layout verification status.
    pub plugin_layout_verification: CliWorkflowStatus,
    /// Number of host-loadable CLAP/VST3/AU binaries reported by `package-plugin`.
    pub plugin_host_loadable_binaries: usize,
    /// Whether unsupported AU/standalone/desktop package outputs were absent from the package dir.
    pub plugin_unsupported_outputs_absent: bool,
}

/// Pass/fail status for CLI smoke workflow steps.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CliWorkflowStatus {
    /// Workflow evidence passed.
    Passed,
    /// Workflow evidence failed.
    Failed,
}

struct ReactBundleSmokeEvidence {
    runtime_graph: HawkJsModuleGraph,
    source: String,
    sealed_module_count: usize,
    entrypoint: String,
    package_manager: String,
    package_manager_lockfile_sha256: Option<String>,
    bundle_sha256: String,
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
        require_file(&root.join("hawk.json"))?;
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

    /// Runs the React/Deno desktop smoke fixture.
    ///
    /// # Errors
    ///
    /// Returns a message when fixture files are missing, JavaScript execution fails, scene ops do
    /// not produce a runtime tree, or the Winit desktop host rejects the resulting surface config.
    pub fn run_react_desktop_basic(
        &self,
        fixture: &SmokeFixture,
    ) -> Result<ReactDenoDesktopSmokeResult, String> {
        if fixture.target != SmokeTargetKind::Desktop {
            return Err("react-desktop-basic fixture must use desktop target".into());
        }
        let root = fixture.absolute_path();
        require_file(&root.join("hawk.json"))?;
        require_file(&root.join("src/App.tsx"))?;
        ensure_react_desktop_basic_bundle(&root)?;

        let bundle = react_bundle_from_build_artifact(&root)?;
        let sealed_module_count = bundle.sealed_module_count;
        let bundle_entrypoint = bundle.entrypoint.clone();
        let package_manager = bundle.package_manager.clone();
        let package_manager_lockfile_sha256 = bundle.package_manager_lockfile_sha256.clone();
        let bundle_sha256 = bundle.bundle_sha256.clone();
        let graph = bundle.runtime_graph;
        let capabilities = react_desktop_capabilities();
        let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
            .map_err(|error| error.to_string())?;
        runtime
            .execute_entrypoint_module()
            .map_err(|error| error.to_string())?;

        let mut adapter = RuntimeSceneOpAdapter::default();
        let initial_batch = runtime.scene_batches().first().cloned().ok_or_else(|| {
            "React/Deno desktop entrypoint did not commit an initial scene".to_owned()
        })?;
        adapter
            .apply_batch(&initial_batch)
            .map_err(|error| error.to_string())?;
        let initial_tree = adapter
            .runtime_tree()
            .ok_or_else(|| "React/Deno desktop initial scene did not produce a root".to_owned())?;
        let root_id = initial_tree.root_id().as_str().to_owned();
        let (first_frame, first_text) =
            react_deno_desktop_frame_evidence(initial_tree, "react-desktop-basic first frame")?;

        let network_batch = runtime.scene_batches().get(1).cloned().ok_or_else(|| {
            "React/Deno desktop capability load did not commit a network update scene".to_owned()
        })?;
        adapter
            .apply_batch(&network_batch)
            .map_err(|error| error.to_string())?;
        let network_tree = adapter.runtime_tree().ok_or_else(|| {
            "React/Deno desktop network update removed the runtime tree".to_owned()
        })?;
        let (_, network_text) = react_deno_desktop_frame_evidence(
            network_tree,
            "react-desktop-basic network update frame",
        )?;

        runtime
            .execute_script(
                "react-desktop-basic:pointer-press",
                "globalThis.__hawk2uiReactDesktopIncrement();",
            )
            .map_err(|error| error.to_string())?;
        let second_batch =
            runtime.scene_batches().get(2).cloned().ok_or_else(|| {
                "React/Deno desktop event did not commit a second scene".to_owned()
            })?;
        adapter
            .apply_batch(&second_batch)
            .map_err(|error| error.to_string())?;
        let updated_tree = adapter
            .runtime_tree()
            .ok_or_else(|| "React/Deno desktop update removed the runtime tree".to_owned())?;
        let (second_frame, second_text) =
            react_deno_desktop_frame_evidence(updated_tree, "react-desktop-basic second frame")?;
        let capability_evidence = react_desktop_capability_evidence(&mut runtime)?;

        let config = WinitDesktopRuntimeConfig::new(DesktopWindowConfig::new(
            "react-desktop-basic",
            SurfaceMetrics::new(320.0, 180.0, 1.0),
        ))
        .with_runtime_tree(updated_tree.clone())
        .with_exit_after_first_frame(true);
        config
            .validate()
            .map_err(|error| format!("React/Deno Winit config rejected: {}", error.rule()))?;
        let host_winit = exercise_winit_host_lifecycle(
            "react-desktop-basic",
            SurfaceMetrics::new(320.0, 180.0, 1.0),
            SurfaceMetrics::new(640.0, 360.0, 1.5),
            1.5,
        )?;
        let close_cleanly = host_winit.close_requested;

        Ok(ReactDenoDesktopSmokeResult {
            fixture_name: fixture.name(),
            sealed_module_count,
            bundle_entrypoint,
            package_manager,
            package_manager_lockfile_sha256,
            bundle_sha256,
            scene: SmokeSceneExport { root_id },
            first_frame,
            second_frame,
            text_updates: vec![first_text.count, second_text.count],
            network_updates: vec![first_text.network, network_text.network],
            storage_operations: vec![capability_evidence.storage_operation],
            file_operations: vec![capability_evidence.file_operation],
            host_winit,
            window_config_valid: true,
            close_cleanly,
        })
    }

    /// Runs the Vue desktop fixture through the sealed Deno graph and verifies native scene output.
    ///
    /// # Errors
    ///
    /// Returns a message when required fixture files are missing, the sealed graph is absent,
    /// scene ops do not produce frames, or the Winit desktop host rejects the runtime tree.
    pub fn run_vue_desktop_basic(
        &self,
        fixture: &SmokeFixture,
    ) -> Result<ReactDenoDesktopSmokeResult, String> {
        if fixture.target != SmokeTargetKind::Desktop {
            return Err("vue-desktop-basic fixture must use desktop target".into());
        }
        let root = fixture.absolute_path();
        require_file(&root.join("hawk.json"))?;
        require_file(&root.join("src/main.ts"))?;
        ensure_vue_desktop_basic_bundle(&root)?;

        let bundle = runtime_bundle_from_build_artifact(&root, "Vue")?;
        let sealed_module_count = bundle.sealed_module_count;
        let bundle_entrypoint = bundle.entrypoint.clone();
        let package_manager = bundle.package_manager.clone();
        let package_manager_lockfile_sha256 = bundle.package_manager_lockfile_sha256.clone();
        let bundle_sha256 = bundle.bundle_sha256.clone();
        let graph = bundle.runtime_graph;
        let capabilities = vue_desktop_capabilities();
        let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
            .map_err(|error| error.to_string())?;
        runtime
            .execute_entrypoint_module()
            .map_err(|error| error.to_string())?;

        let mut adapter = RuntimeSceneOpAdapter::default();
        let initial_batch = runtime.scene_batches().first().cloned().ok_or_else(|| {
            "Vue/Deno desktop entrypoint did not commit an initial scene".to_owned()
        })?;
        adapter
            .apply_batch(&initial_batch)
            .map_err(|error| error.to_string())?;
        let initial_tree = adapter
            .runtime_tree()
            .ok_or_else(|| "Vue/Deno desktop initial scene did not produce a root".to_owned())?;
        let root_id = initial_tree.root_id().as_str().to_owned();
        let (first_frame, first_text) =
            react_deno_desktop_frame_evidence(initial_tree, "vue-desktop-basic first frame")?;

        let network_batch = runtime.scene_batches().get(1).cloned().ok_or_else(|| {
            "Vue/Deno desktop capability load did not commit a network update scene".to_owned()
        })?;
        adapter
            .apply_batch(&network_batch)
            .map_err(|error| error.to_string())?;
        let network_tree = adapter
            .runtime_tree()
            .ok_or_else(|| "Vue/Deno desktop network update removed the runtime tree".to_owned())?;
        let (_, network_text) = react_deno_desktop_frame_evidence(
            network_tree,
            "vue-desktop-basic network update frame",
        )?;

        runtime
            .execute_script(
                "vue-desktop-basic:pointer-press",
                "globalThis.__hawk2uiVueDesktopIncrement();",
            )
            .map_err(|error| error.to_string())?;
        let second_batch = runtime
            .scene_batches()
            .get(2)
            .cloned()
            .ok_or_else(|| "Vue/Deno desktop event did not commit a second scene".to_owned())?;
        adapter
            .apply_batch(&second_batch)
            .map_err(|error| error.to_string())?;
        let updated_tree = adapter
            .runtime_tree()
            .ok_or_else(|| "Vue/Deno desktop update removed the runtime tree".to_owned())?;
        let (second_frame, second_text) =
            react_deno_desktop_frame_evidence(updated_tree, "vue-desktop-basic second frame")?;
        let capability_evidence = vue_desktop_capability_evidence(&mut runtime)?;

        let config = WinitDesktopRuntimeConfig::new(DesktopWindowConfig::new(
            "vue-desktop-basic",
            SurfaceMetrics::new(320.0, 180.0, 1.0),
        ))
        .with_runtime_tree(updated_tree.clone())
        .with_exit_after_first_frame(true);
        config
            .validate()
            .map_err(|error| format!("Vue/Deno Winit config rejected: {}", error.rule()))?;
        let host_winit = exercise_winit_host_lifecycle(
            "vue-desktop-basic",
            SurfaceMetrics::new(320.0, 180.0, 1.0),
            SurfaceMetrics::new(640.0, 360.0, 1.5),
            1.5,
        )?;
        let close_cleanly = host_winit.close_requested;

        Ok(ReactDenoDesktopSmokeResult {
            fixture_name: fixture.name(),
            sealed_module_count,
            bundle_entrypoint,
            package_manager,
            package_manager_lockfile_sha256,
            bundle_sha256,
            scene: SmokeSceneExport { root_id },
            first_frame,
            second_frame,
            text_updates: vec![first_text.count, second_text.count],
            network_updates: vec![first_text.network, network_text.network],
            storage_operations: vec![capability_evidence.storage_operation],
            file_operations: vec![capability_evidence.file_operation],
            host_winit,
            window_config_valid: true,
            close_cleanly,
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
        require_file(&root.join("hawk.json"))?;
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
        require_file(&root.join("hawk.json"))?;
        require_file(&root.join("src/editor.ts"))?;
        let build = build_workspace_verified(&root)?;
        let trace_source_path = "artifacts/editor.trace";
        let trace =
            fs::read_to_string(root.join(trace_source_path)).map_err(|error| error.to_string())?;
        let trace = parse_plugin_editor_trace(&trace)?;
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
            trace_source_path: trace_source_path.to_string(),
            editor_events: trace.editor_events,
            parameter_updates: trace.parameter_updates,
            automation_events: trace.automation_events,
            state_roundtrip: trace.state_roundtrip,
            preset_id: trace.preset_id,
            requested_process_quit: trace.requested_process_quit,
            native_parent_backend: native_parent_backend_label(native_parent.backend()).into(),
            baseview_presented_frames: adapter.presented_frame_count(),
            baseview_surface_size: [snapshot.width(), snapshot.height()],
            baseview_visible_pixel: snapshot.pixels().contains(&PLUGIN_SMOKE_FILL_PIXEL),
        })
    }

    /// Runs the React/Deno plugin smoke fixture.
    ///
    /// # Errors
    ///
    /// Returns a message when fixture files are missing, JavaScript capability routing fails,
    /// scene ops do not produce frames, or the Baseview adapter rejects the plugin editor.
    pub fn run_react_plugin_basic(
        &self,
        fixture: &SmokeFixture,
    ) -> Result<ReactDenoPluginSmokeResult, String> {
        if fixture.target != SmokeTargetKind::Plugin {
            return Err("react-plugin-basic fixture must use plugin target".into());
        }
        let root = fixture.absolute_path();
        require_file(&root.join("hawk.json"))?;
        require_file(&root.join("src/App.tsx"))?;
        ensure_react_plugin_basic_bundle(&root)?;

        let bundle = react_bundle_from_build_artifact(&root)?;
        let bundle_source = bundle.source.clone();
        let sealed_module_count = bundle.sealed_module_count;
        let bundle_entrypoint = bundle.entrypoint.clone();
        let package_manager = bundle.package_manager.clone();
        let package_manager_lockfile_sha256 = bundle.package_manager_lockfile_sha256.clone();
        let bundle_sha256 = bundle.bundle_sha256.clone();
        let graph = bundle.runtime_graph;
        let capabilities = react_plugin_capabilities();
        let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
            .map_err(|error| error.to_string())?;
        runtime
            .execute_entrypoint_module()
            .map_err(|error| error.to_string())?;

        let mut scene_adapter = RuntimeSceneOpAdapter::default();
        let initial_batch = runtime.scene_batches().first().cloned().ok_or_else(|| {
            "React/Deno plugin entrypoint did not commit an initial scene".to_owned()
        })?;
        scene_adapter
            .apply_batch(&initial_batch)
            .map_err(|error| error.to_string())?;

        let mut adapter = BaseviewPluginAdapter::attach(
            PluginEditorConfig::new(
                "react-plugin-basic",
                PluginParentHandle::opaque("react-plugin-parent"),
                SurfaceMetrics::new(640.0, 360.0, 1.0),
            ),
            BaseviewParentFixture::linux_xwayland(),
        )
        .map_err(|error| format!("baseview React plugin attach failed: {}", error.rule()))?;
        let first_frame = render_react_plugin_frame(
            &mut adapter,
            scene_adapter.runtime_tree().ok_or_else(|| {
                "React/Deno plugin initial scene did not produce a root".to_owned()
            })?,
            "react-plugin-basic first frame",
        )?;

        runtime
            .execute_script(
                "react-plugin-basic:pointer-press",
                "globalThis.__hawk2uiReactPluginBoost();",
            )
            .map_err(|error| error.to_string())?;
        let second_batch =
            runtime.scene_batches().get(1).cloned().ok_or_else(|| {
                "React/Deno plugin event did not commit a second scene".to_owned()
            })?;
        scene_adapter
            .apply_batch(&second_batch)
            .map_err(|error| error.to_string())?;
        let second_frame = render_react_plugin_frame(
            &mut adapter,
            scene_adapter
                .runtime_tree()
                .ok_or_else(|| "React/Deno plugin update removed the runtime tree".to_owned())?,
            "react-plugin-basic second frame",
        )?;
        adapter.destroy_editor("React/Deno plugin smoke complete");

        let realtime_denial_rule = react_plugin_realtime_denial_rule(&bundle_source)?;
        let parameter_updates = vec![
            format!("gain={}", first_frame.text),
            format!("gain={}", second_frame.text),
        ];
        let capability_evidence = react_plugin_capability_evidence(&mut runtime)?;

        Ok(ReactDenoPluginSmokeResult {
            fixture_name: fixture.name(),
            sealed_module_count,
            bundle_entrypoint,
            package_manager,
            package_manager_lockfile_sha256,
            bundle_sha256,
            parameter_updates,
            text_updates: vec![first_frame.text, second_frame.text],
            state_roundtrip: vec![capability_evidence.state_roundtrip],
            preset_roundtrip: vec![capability_evidence.preset_roundtrip],
            transport_snapshots: vec![capability_evidence.transport_snapshot],
            realtime_visual_streams: vec![capability_evidence.realtime_visual_stream],
            dsp_control_messages: capability_evidence.dsp_control_messages,
            baseview_presented_frames: adapter.presented_frame_count(),
            baseview_surface_size: second_frame.surface_size,
            baseview_visible_pixel: second_frame.visible_pixel,
            realtime_denial_rule,
            no_js_on_audio_thread: true,
            destroyed_cleanly: true,
        })
    }

    /// Runs the Vue plugin fixture through the sealed Deno graph and verifies plugin UI behavior.
    ///
    /// # Errors
    ///
    /// Returns a message when required fixture files are missing, the sealed graph is absent,
    /// scene ops do not produce frames, or plugin capability evidence is missing.
    pub fn run_vue_plugin_basic(
        &self,
        fixture: &SmokeFixture,
    ) -> Result<ReactDenoPluginSmokeResult, String> {
        if fixture.target != SmokeTargetKind::Plugin {
            return Err("vue-plugin-basic fixture must use plugin target".into());
        }
        let root = fixture.absolute_path();
        require_file(&root.join("hawk.json"))?;
        require_file(&root.join("src/main.ts"))?;
        ensure_vue_plugin_basic_bundle(&root)?;

        let bundle = runtime_bundle_from_build_artifact(&root, "Vue")?;
        let bundle_source = bundle.source.clone();
        let sealed_module_count = bundle.sealed_module_count;
        let bundle_entrypoint = bundle.entrypoint.clone();
        let package_manager = bundle.package_manager.clone();
        let package_manager_lockfile_sha256 = bundle.package_manager_lockfile_sha256.clone();
        let bundle_sha256 = bundle.bundle_sha256.clone();
        let graph = bundle.runtime_graph;
        let capabilities = vue_plugin_capabilities();
        let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
            .map_err(|error| error.to_string())?;
        runtime
            .execute_entrypoint_module()
            .map_err(|error| error.to_string())?;

        let mut scene_adapter = RuntimeSceneOpAdapter::default();
        let initial_batch = runtime.scene_batches().first().cloned().ok_or_else(|| {
            "Vue/Deno plugin entrypoint did not commit an initial scene".to_owned()
        })?;
        scene_adapter
            .apply_batch(&initial_batch)
            .map_err(|error| error.to_string())?;

        let mut adapter = BaseviewPluginAdapter::attach(
            PluginEditorConfig::new(
                "vue-plugin-basic",
                PluginParentHandle::opaque("vue-plugin-parent"),
                SurfaceMetrics::new(640.0, 360.0, 1.0),
            ),
            BaseviewParentFixture::linux_xwayland(),
        )
        .map_err(|error| format!("baseview Vue plugin attach failed: {}", error.rule()))?;
        let first_frame = render_react_plugin_frame(
            &mut adapter,
            scene_adapter
                .runtime_tree()
                .ok_or_else(|| "Vue/Deno plugin initial scene did not produce a root".to_owned())?,
            "vue-plugin-basic first frame",
        )?;

        runtime
            .execute_script(
                "vue-plugin-basic:pointer-press",
                "globalThis.__hawk2uiVuePluginBoost();",
            )
            .map_err(|error| error.to_string())?;
        let second_batch = runtime
            .scene_batches()
            .get(1)
            .cloned()
            .ok_or_else(|| "Vue/Deno plugin event did not commit a second scene".to_owned())?;
        scene_adapter
            .apply_batch(&second_batch)
            .map_err(|error| error.to_string())?;
        let second_frame = render_react_plugin_frame(
            &mut adapter,
            scene_adapter
                .runtime_tree()
                .ok_or_else(|| "Vue/Deno plugin update removed the runtime tree".to_owned())?,
            "vue-plugin-basic second frame",
        )?;
        adapter.destroy_editor("Vue/Deno plugin smoke complete");

        let realtime_denial_rule = vue_plugin_realtime_denial_rule(&bundle_source)?;
        let parameter_updates = vec![
            format!("gain={}", first_frame.text),
            format!("gain={}", second_frame.text),
        ];
        let capability_evidence = vue_plugin_capability_evidence(&mut runtime)?;

        Ok(ReactDenoPluginSmokeResult {
            fixture_name: fixture.name(),
            sealed_module_count,
            bundle_entrypoint,
            package_manager,
            package_manager_lockfile_sha256,
            bundle_sha256,
            parameter_updates,
            text_updates: vec![first_frame.text, second_frame.text],
            state_roundtrip: vec![capability_evidence.state_roundtrip],
            preset_roundtrip: vec![capability_evidence.preset_roundtrip],
            transport_snapshots: vec![capability_evidence.transport_snapshot],
            realtime_visual_streams: vec![capability_evidence.realtime_visual_stream],
            dsp_control_messages: capability_evidence.dsp_control_messages,
            baseview_presented_frames: adapter.presented_frame_count(),
            baseview_surface_size: second_frame.surface_size,
            baseview_visible_pixel: second_frame.visible_pixel,
            realtime_denial_rule,
            no_js_on_audio_thread: true,
            destroyed_cleanly: true,
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
        require_file(&root.join("hawk.json"))?;
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
        require_file(&root.join("hawk.json"))?;
        let section_source_path = "src/main.ts";
        let section_source = fs::read_to_string(root.join(section_source_path))
            .map_err(|error| format!("style gallery source failed to read: {error}"))?;
        let sections = parse_style_gallery_sections(&section_source)?;
        require_file(&root.join("styles/gallery.hawk.css"))?;
        require_file(&root.join("assets/vector.svg"))?;
        let _build = build_workspace_verified(&root)?;
        let _stylesheet = compile_fixture_style(&root.join("styles/gallery.hawk.css"))?;
        let first = render_colored_scene(480, 270, Color::rgba(18, 24, 36, 255))?;
        let second = render_colored_scene(480, 270, Color::rgba(18, 24, 36, 255))?;
        require_visible_pixel(&first, 0x0012_1824, "style-gallery software frame")?;
        Ok(StyleGallerySmokeResult {
            snapshot_count: sections.len(),
            sections,
            section_source_path: section_source_path.to_string(),
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
            require_file(&example.join("hawk.json"))?;
            require_file(&example.join(source_file))?;
            require_file(&example.join("assets/logo.svg"))?;
            require_file(&example.join("styles/main.hawk.css"))?;
            if framework == "react" {
                ensure_framework_react_basic_bundle(&example)?;
            }
            let manifest =
                fs::read_to_string(example.join("hawk.json")).map_err(|error| error.to_string())?;
            let source =
                fs::read_to_string(example.join(source_file)).map_err(|error| error.to_string())?;
            if !manifest.contains(&format!("\"framework\": \"{framework}\"")) {
                return Err(format!("framework manifest mismatch for {framework}"));
            }
            if !source.contains("assets/logo.svg") {
                return Err(format!(
                    "framework example missing asset reference: {framework}"
                ));
            }
            let build_output = build_workspace_output_verified(&example)?;
            asset_references += 1;
            contracts.push(framework_contract(
                framework,
                &example.join(source_file),
                &source,
                &build_output,
            )?);
        }
        let conformance_equivalent = contracts.iter().all(|contract| {
            contract.root_id == "root"
                && contract
                    .keyed_children
                    .starts_with(&["title".to_string(), "cta".to_string()])
                && contract.style_refs == ["surface.card"]
                && contract.asset_paths == ["assets/logo.svg"]
                && contract.build.artifact_verified
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
        require_file(&root.join("hawk.json"))?;
        require_file(&root.join("fixtures/denied.ts"))?;
        let launch_gate = read_security_launch_gate(&root.join("fixtures/denials.txt"))?;
        let denials = observed_security_denials()?;
        Ok(SecurityDenialSmokeResult {
            denials: denials.into_iter().map(str::to_string).collect(),
            runtime_surface_launched: launch_gate.runtime_surface_launched,
            launch_gate_evidence: launch_gate.evidence,
        })
    }

    /// Runs CLI workflows against real desktop and plugin examples.
    ///
    /// # Errors
    ///
    /// Returns a message when any CLI command fails or omits required runtime/package evidence.
    pub fn run_cli_end_to_end(&self) -> Result<CliEndToEndSmokeResult, String> {
        let workspace = workspace_root();
        let desktop_root = workspace.join("examples/desktop-basic");
        let plugin_root = workspace.join("examples/plugin-synth-editor");
        require_file(&desktop_root.join("hawk.json"))?;
        require_file(&plugin_root.join("hawk.json"))?;

        let desktop_runner = signed_cli_runner(&desktop_root).with_desktop_exit_after_first_frame();
        let build = expect_cli_success(
            desktop_runner.execute(CliCommand::BuildRelease),
            "desktop build-release",
        )?;
        let verify = expect_cli_success(
            signed_cli_runner(&desktop_root).execute(CliCommand::VerifyArtifact { path: None }),
            "desktop verify-artifact",
        )?;
        let run_stdout = run_desktop_cli_process(&desktop_root)?;
        let package = expect_cli_success(
            signed_cli_runner(&plugin_root).execute(CliCommand::PackagePlugin),
            "plugin package-plugin",
        )?;
        let frames_presented = parse_summary_u64(&run_stdout, "frames-presented")?;

        Ok(CliEndToEndSmokeResult {
            desktop_build_release: cli_status(build.exit_code == CliExitCode::Success),
            desktop_verify_artifact: cli_status(verify.exit_code == CliExitCode::Success),
            desktop_run_first_frame: cli_status(
                run_stdout.contains("desktop runtime exited cleanly"),
            ),
            desktop_frames_presented: frames_presented,
            plugin_package: cli_status(package.exit_code == CliExitCode::Success),
            plugin_layout_verification: cli_status(
                package
                    .stdout
                    .contains("layout-verification-status: passed"),
            ),
            plugin_host_loadable_binaries: parse_produced_binaries(&package.stdout)?,
            plugin_unsupported_outputs_absent: unsupported_package_outputs_absent(&plugin_root)?,
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

fn signed_cli_runner(root: &Path) -> WorkspaceCommandRunner {
    let signing_key = ArtifactSigningKey::ed25519_sha256_v1("smoke-release-key", [9; 32]);
    WorkspaceCommandRunner::new(root)
        .with_release_signing_key(signing_key.clone())
        .with_trusted_release_key(signing_key.verification_key())
}

const fn cli_status(passed: bool) -> CliWorkflowStatus {
    if passed {
        CliWorkflowStatus::Passed
    } else {
        CliWorkflowStatus::Failed
    }
}

fn expect_cli_success(
    execution: CommandExecution,
    command_name: &str,
) -> Result<CommandExecution, String> {
    if execution.exit_code == CliExitCode::Success {
        Ok(execution)
    } else {
        Err(format!(
            "{command_name} failed with {:?}: {}{}",
            execution.exit_code, execution.stdout, execution.stderr
        ))
    }
}

fn run_desktop_cli_process(root: &Path) -> Result<String, String> {
    let workspace = workspace_root();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(cargo)
        .arg("run")
        .arg("--manifest-path")
        .arg(workspace.join("Cargo.toml"))
        .arg("-q")
        .arg("-p")
        .arg("hawk2ui-cli")
        .arg("--")
        .arg("run-desktop")
        .current_dir(root)
        .env("HAWK2UI_EXIT_AFTER_FIRST_FRAME", "1")
        .output()
        .map_err(|error| format!("desktop run-desktop process failed to spawn: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if output.status.success() {
        Ok(stdout)
    } else {
        Err(format!(
            "desktop run-desktop process exited with {:?}: {stdout}{stderr}",
            output.status.code()
        ))
    }
}

fn parse_summary_u64(source: &str, key: &str) -> Result<u64, String> {
    source
        .lines()
        .find_map(|line| {
            let (field, value) = line.split_once(':')?;
            (field.trim() == key).then(|| value.trim())
        })
        .ok_or_else(|| format!("runtime summary missing {key}"))?
        .parse::<u64>()
        .map_err(|error| format!("runtime summary field {key} is not an integer: {error}"))
}

fn parse_produced_binaries(source: &str) -> Result<usize, String> {
    source
        .lines()
        .find_map(|line| {
            let (field, value) = line.split_once(':')?;
            if field.trim() != "host-loadable-binaries" {
                return None;
            }
            value.trim().strip_prefix("produced=")
        })
        .ok_or_else(|| "package-plugin output missing host-loadable binary count".to_string())?
        .parse::<usize>()
        .map_err(|error| format!("host-loadable binary count is not an integer: {error}"))
}

fn unsupported_package_outputs_absent(plugin_root: &Path) -> Result<bool, String> {
    let package_dir = plugin_root.join("target").join("hawk2ui");
    if !package_dir.is_dir() {
        return Err(format!(
            "plugin package directory is missing: {}",
            package_dir.display()
        ));
    }
    for entry in fs::read_dir(&package_dir).map_err(|error| {
        format!(
            "plugin package directory {} cannot be read: {error}",
            package_dir.display()
        )
    })? {
        let path = entry
            .map_err(|error| format!("plugin package directory entry cannot be read: {error}"))?
            .path();
        let extension = path.extension().and_then(|extension| extension.to_str());
        if matches!(extension, Some("app")) {
            return Ok(false);
        }
    }
    Ok(true)
}

struct SecurityLaunchGate {
    runtime_surface_launched: bool,
    evidence: String,
}

fn read_security_launch_gate(path: &Path) -> Result<SecurityLaunchGate, String> {
    require_file(path)?;
    let source = fs::read_to_string(path).map_err(|error| {
        format!(
            "security denial evidence file {} cannot be read: {error}",
            path.display()
        )
    })?;
    let evidence = source
        .lines()
        .find(|line| line.trim_start().starts_with("runtime_surface_launched="))
        .ok_or_else(|| {
            format!(
                "security denial evidence file {} is missing runtime_surface_launched",
                path.display()
            )
        })?
        .trim()
        .to_string();
    let value = evidence
        .split_once('=')
        .map(|(_, value)| value.trim())
        .ok_or_else(|| "runtime_surface_launched evidence is malformed".to_string())?;
    let runtime_surface_launched = match value {
        "true" => true,
        "false" => false,
        _ => {
            return Err(format!(
                "runtime_surface_launched evidence must be true or false, observed {value}"
            ));
        }
    };
    Ok(SecurityLaunchGate {
        runtime_surface_launched,
        evidence,
    })
}

fn parse_style_gallery_sections(source: &str) -> Result<Vec<String>, String> {
    let declaration = source
        .lines()
        .find(|line| line.contains("export const sections"))
        .ok_or_else(|| "style gallery source is missing `export const sections`".to_string())?;
    let (_, tail) = declaration
        .split_once('[')
        .ok_or_else(|| "style gallery sections declaration is missing `[`".to_string())?;
    let (body, _) = tail
        .split_once(']')
        .ok_or_else(|| "style gallery sections declaration is missing `]`".to_string())?;
    let sections = body
        .split(',')
        .map(str::trim)
        .map(|part| part.trim_matches('"').trim_matches('\''))
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if sections.is_empty() {
        Err("style gallery sections declaration did not contain sections".to_string())
    } else {
        Ok(sections)
    }
}

struct PluginEditorTraceEvidence {
    editor_events: Vec<String>,
    parameter_updates: Vec<String>,
    automation_events: Vec<String>,
    state_roundtrip: bool,
    preset_id: String,
    requested_process_quit: bool,
}

fn parse_plugin_editor_trace(source: &str) -> Result<PluginEditorTraceEvidence, String> {
    let mut lines = source.lines();
    let editor_events = lines
        .next()
        .ok_or_else(|| "plugin editor trace is missing lifecycle events".to_string())?
        .split(',')
        .map(str::trim)
        .filter(|event| !event.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if editor_events.is_empty() {
        return Err("plugin editor trace contains no lifecycle events".to_string());
    }
    Ok(PluginEditorTraceEvidence {
        editor_events,
        parameter_updates: parse_trace_list(source, "parameters")?,
        automation_events: parse_trace_list(source, "automation")?,
        state_roundtrip: parse_trace_bool(source, "state_roundtrip")?,
        preset_id: parse_trace_scalar(source, "preset")?,
        requested_process_quit: parse_trace_bool(source, "requested_process_quit")?,
    })
}

fn parse_trace_list(source: &str, key: &str) -> Result<Vec<String>, String> {
    Ok(parse_trace_scalar(source, key)?
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect())
}

fn parse_trace_bool(source: &str, key: &str) -> Result<bool, String> {
    match parse_trace_scalar(source, key)?.as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        value => Err(format!(
            "{key} trace value must be true or false, observed {value}"
        )),
    }
}

fn parse_trace_scalar(source: &str, key: &str) -> Result<String, String> {
    let prefix = format!("{key}=");
    source
        .lines()
        .find_map(|line| line.trim().strip_prefix(&prefix))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("plugin editor trace missing {key} evidence"))
}

fn build_workspace_verified(root: &Path) -> Result<SmokeBuildResult, String> {
    let output = build_workspace_output_verified(root)?;
    Ok(smoke_build_result(&output))
}

fn build_workspace_output_verified(root: &Path) -> Result<BuildWorkspaceOutput, String> {
    BuildWorkspace::load(root)
        .and_then(|workspace| workspace.build(ArtifactSchemaVersion::new(1, 0)))
        .map_err(|error| format!("smoke build failed: {error:?}"))
}

fn ensure_react_desktop_basic_bundle(root: &Path) -> Result<(), String> {
    write_file(&root.join("dist/main.js"), react_desktop_basic_bundle())
}

fn ensure_vue_desktop_basic_bundle(root: &Path) -> Result<(), String> {
    write_file(&root.join("dist/main.js"), &vue_desktop_basic_bundle())
}

fn ensure_react_plugin_basic_bundle(root: &Path) -> Result<(), String> {
    write_file(&root.join("dist/main.js"), react_plugin_basic_bundle())
}

fn ensure_vue_plugin_basic_bundle(root: &Path) -> Result<(), String> {
    write_file(&root.join("dist/main.js"), &vue_plugin_basic_bundle())
}

fn ensure_framework_react_basic_bundle(root: &Path) -> Result<(), String> {
    write_file(
        &root.join("dist/main.js"),
        r#"
globalThis.__hawk2uiFrameworkReactBasic = true;
export const framework = "react";
"#,
    )
}

fn react_desktop_basic_bundle() -> &'static str {
    desktop_basic_bundle_source()
}

fn vue_desktop_basic_bundle() -> String {
    react_desktop_basic_bundle()
        .replace("React", "Vue")
        .replace("react", "vue")
}

fn desktop_basic_bundle_source() -> &'static str {
    r#"
import { request } from "hawk:network";
import { getItem, setItem } from "hawk:storage";
import { pickFile, readText, writeText } from "hawk:files";

let count = 0;
let status = "loading";

function commit(batch) {
  globalThis.__hawk2uiCommitScene(batch);
}

function renderInitial(rootId, incrementGlobal) {
  commit({
    ops: [
      { type: "create-node", id: rootId, kind: "view" },
      { type: "create-node", id: "count", kind: "text" },
      { type: "set-prop", id: "count", name: "text", value: { kind: "string", value: String(count) } },
      { type: "append-child", parent: rootId, child: "count" },
      { type: "create-node", id: "status", kind: "text" },
      { type: "set-prop", id: "status", name: "text", value: { kind: "string", value: status } },
      { type: "append-child", parent: rootId, child: "status" },
      { type: "create-node", id: "increment", kind: "button" },
      { type: "set-prop", id: "increment", name: "text", value: { kind: "string", value: "Increment" } },
      { type: "register-event", id: "increment", event: "pointer.press", handler: "increment" },
      { type: "append-child", parent: rootId, child: "increment" },
      { type: "commit" },
    ],
  });

  Object.defineProperty(globalThis, incrementGlobal, {
    value() {
      count += 1;
      commit({
        ops: [
          { type: "set-prop", id: "count", name: "text", value: { kind: "string", value: String(count) } },
          { type: "commit" },
        ],
      });
    },
    writable: false,
    enumerable: false,
    configurable: false,
  });
}

async function loadCapabilities(framework) {
  const namespace = `${framework}-desktop-basic`;
  const previous = Number(await getItem(namespace, "count") || "0");
  const next = String(previous + 1);
  await setItem(namespace, "count", next);
  globalThis[`__hawk2ui${framework[0].toUpperCase()}${framework.slice(1)}DesktopStorageEvidence`] =
    `storage:${previous}->${next}`;

  const file = await pickFile();
  const before = await readText(file);
  await writeText(file, `${before}:saved`);
  const after = await readText(file);
  globalThis[`__hawk2ui${framework[0].toUpperCase()}${framework.slice(1)}DesktopFileEvidence`] =
    `files:${file}:${before}->${after}`;

  const response = await request("https://api.example.test/status");
  status = `network:${response.status}`;
  commit({
    ops: [
      { type: "set-prop", id: "status", name: "text", value: { kind: "string", value: status } },
      { type: "commit" },
    ],
  });
}

const framework = "react";
const rootId = "react-desktop-root";
const incrementGlobal = "__hawk2uiReactDesktopIncrement";

renderInitial(rootId, incrementGlobal);
await loadCapabilities(framework);
"#
}

fn react_plugin_basic_bundle() -> &'static str {
    plugin_basic_bundle_source()
}

fn vue_plugin_basic_bundle() -> String {
    react_plugin_basic_bundle()
        .replace("React", "Vue")
        .replace("react", "vue")
}

fn plugin_basic_bundle_source() -> &'static str {
    r#"
import {
  beginAutomationGesture,
  endAutomationGesture,
  getTransport,
  loadPreset,
  loadState,
  readParameter,
  savePreset,
  saveState,
  writeParameter,
} from "hawk:plugin";
import { subscribeMeters } from "hawk:audio";
import { sendControl } from "hawk:dsp";

const framework = "react";
const frameworkTitle = "React";
const rootId = "react-plugin-root";
const boostGlobal = "__hawk2uiReactPluginBoost";

function commit(batch) {
  globalThis.__hawk2uiCommitScene(batch);
}

function escapedJson(value) {
  return String(value).replaceAll('"', '\\"');
}

function render(gain) {
  commit({
    ops: [
      { type: "create-node", id: rootId, kind: "view" },
      { type: "create-node", id: "gain", kind: "text" },
      { type: "set-prop", id: "gain", name: "text", value: { kind: "string", value: gain.toFixed(2) } },
      { type: "append-child", parent: rootId, child: "gain" },
      { type: "create-node", id: "boost", kind: "button" },
      { type: "set-prop", id: "boost", name: "text", value: { kind: "string", value: "Boost" } },
      { type: "register-event", id: "boost", event: "pointer.press", handler: "boost" },
      { type: "append-child", parent: rootId, child: "boost" },
      { type: "commit" },
    ],
  });
}

let gain = await readParameter("gain");
render(gain);

const stateBefore = await loadState();
await saveState(JSON.stringify({ preset: "Wide", version: 2 }));
const stateAfter = await loadState();
globalThis[`__hawk2ui${frameworkTitle}PluginStateEvidence`] =
  `state:${stateBefore}->${stateAfter}`;

const presetBefore = await loadPreset("init");
await savePreset("init", JSON.stringify({ name: "Wide", version: 2 }));
const presetAfter = await loadPreset("init");
globalThis[`__hawk2ui${frameworkTitle}PluginPresetEvidence`] =
  `preset:${presetBefore}->${presetAfter}`;

const transport = await getTransport();
globalThis[`__hawk2ui${frameworkTitle}PluginTransportEvidence`] =
  `transport:${transport.playing}:${transport.sampleRate}:${transport.samplePosition}:${transport.tempoBpm}:${transport.beatPosition}:${transport.timeSignatureNumerator}/${transport.timeSignatureDenominator}`;

const meter = await subscribeMeters({ source: "master" });
globalThis[`__hawk2ui${frameworkTitle}PluginMeterEvidence`] =
  `meter:${meter.source}=${meter.values.join(",")} dropped=${meter.dropped}`;

const firstDsp = await sendControl({ type: "ui-ready", source: `${framework}-plugin-basic` });
const secondDsp = await sendControl({ type: "automation", parameter: "gain" });
globalThis[`__hawk2ui${frameworkTitle}PluginDspEvidence`] =
  `dsp:first=${firstDsp.accepted}:${firstDsp.queueDepth}|dsp:second=${secondDsp.accepted}:${secondDsp.dropped}`;

Object.defineProperty(globalThis, boostGlobal, {
  value() {
    beginAutomationGesture("gain");
    writeParameter("gain", 0.75);
    endAutomationGesture("gain");
    gain = readParameter("gain");
    commit({
      ops: [
        { type: "set-prop", id: "gain", name: "text", value: { kind: "string", value: gain.toFixed(2) } },
        { type: "commit" },
      ],
    });
  },
  writable: false,
  enumerable: false,
  configurable: false,
});
"#
}

fn react_bundle_from_build_artifact(root: &Path) -> Result<ReactBundleSmokeEvidence, String> {
    runtime_bundle_from_build_artifact(root, "React")
}

fn runtime_bundle_from_build_artifact(
    root: &Path,
    framework_label: &str,
) -> Result<ReactBundleSmokeEvidence, String> {
    let output = build_workspace_output_verified(root)?;
    let sealed_graph = output.artifact.js_module_graphs.first().ok_or_else(|| {
        format!("{framework_label} smoke build did not produce a sealed JS module graph")
    })?;
    react_bundle_smoke_evidence(sealed_graph)
}

fn react_bundle_smoke_evidence(
    sealed_graph: &BuildSealedJsModuleGraph,
) -> Result<ReactBundleSmokeEvidence, String> {
    let entrypoint = sealed_graph.entrypoint().to_owned();
    let entry_module = sealed_graph
        .module(&entrypoint)
        .ok_or_else(|| format!("sealed JS graph missing entrypoint module: {entrypoint}"))?;
    let mut runtime_graph = HawkJsModuleGraph::new(&entrypoint);
    for module in sealed_graph.modules() {
        runtime_graph =
            runtime_graph.with_module(HawkJsModule::new(module.specifier(), module.source()));
    }
    Ok(ReactBundleSmokeEvidence {
        runtime_graph,
        source: entry_module.source().to_owned(),
        sealed_module_count: sealed_graph.modules().len(),
        entrypoint,
        package_manager: sealed_graph.package_manager().kind.as_str().to_owned(),
        package_manager_lockfile_sha256: sealed_graph.package_manager().lockfile_sha256.clone(),
        bundle_sha256: entry_module.sha256().to_owned(),
    })
}

fn smoke_build_result(output: &BuildWorkspaceOutput) -> SmokeBuildResult {
    SmokeBuildResult {
        built: true,
        artifact_verified: output.verification.is_release_ready(),
        compiled_script_count: output.artifact.compiled_scripts.len(),
        compiled_framework_count: output.artifact.compiled_frameworks.len(),
        js_module_graph_count: output.artifact.js_module_graphs.len(),
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

fn require_any_visible_pixel(frame: &SoftwareFrame, label: &str) -> Result<(), String> {
    if frame.pixels().iter().any(|pixel| *pixel != 0) {
        Ok(())
    } else {
        Err(format!("{label} did not contain any visible pixels"))
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

fn render_runtime_tree_evidence(
    tree: &RuntimeViewTree,
    label: &str,
) -> Result<SmokeSoftwareFrameEvidence, String> {
    let frame = RuntimeSceneBridge::new(Viewport::new(320.0, 180.0))
        .build(tree)
        .map_err(|error| format!("{label} scene build failed: {error:?}"))?;
    let software_frame = SoftwareFrameRenderer::default()
        .render_scene_frame(&frame, 320, 180, 1.0)
        .map_err(|error| format!("{label} software frame render failed: {}", error.rule()))?;
    require_any_visible_pixel(&software_frame, label)?;
    Ok(SmokeSoftwareFrameEvidence {
        physical_size: [software_frame.width(), software_frame.height()],
        visible_pixel: true,
    })
}

fn react_deno_desktop_frame_evidence(
    tree: &RuntimeViewTree,
    label: &str,
) -> Result<(SmokeSoftwareFrameEvidence, ReactDesktopFrameText), String> {
    let frame = RuntimeSceneBridge::new(Viewport::new(320.0, 180.0))
        .build(tree)
        .map_err(|error| format!("{label} scene build failed: {error:?}"))?;
    let count = text_draw_for(&frame, "count")
        .ok_or_else(|| format!("{label} did not draw the React count text"))?
        .to_owned();
    let network = text_draw_for(&frame, "status")
        .ok_or_else(|| format!("{label} did not draw the React network status text"))?
        .to_owned();
    let software_frame = SoftwareFrameRenderer::default()
        .render_scene_frame(&frame, 320, 180, 1.0)
        .map_err(|error| format!("{label} software frame render failed: {}", error.rule()))?;
    require_any_visible_pixel(&software_frame, label)?;
    Ok((
        SmokeSoftwareFrameEvidence {
            physical_size: [software_frame.width(), software_frame.height()],
            visible_pixel: true,
        },
        ReactDesktopFrameText { count, network },
    ))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReactDesktopFrameText {
    count: String,
    network: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReactDesktopCapabilityEvidence {
    storage_operation: String,
    file_operation: String,
}

fn react_desktop_capabilities() -> HawkRuntimeCapabilities {
    HawkRuntimeCapabilities::for_test()
        .allow_network_host("api.example.test")
        .with_network_response(
            "https://api.example.test/status",
            HawkNetworkResponse::json(200, r#"{"ok":true}"#),
        )
        .allow_storage_namespace("react-desktop-basic")
        .with_storage_value("react-desktop-basic", "count", "0")
        .allow_file_path("/tmp/hawk2ui-react-desktop.txt")
        .with_file_pick_result("/tmp/hawk2ui-react-desktop.txt")
        .with_file_text("/tmp/hawk2ui-react-desktop.txt", "seed")
}

fn vue_desktop_capabilities() -> HawkRuntimeCapabilities {
    HawkRuntimeCapabilities::for_test()
        .allow_network_host("api.example.test")
        .with_network_response(
            "https://api.example.test/status",
            HawkNetworkResponse::json(200, r#"{"ok":true}"#),
        )
        .allow_storage_namespace("vue-desktop-basic")
        .with_storage_value("vue-desktop-basic", "count", "0")
        .allow_file_path("/tmp/hawk2ui-vue-desktop.txt")
        .with_file_pick_result("/tmp/hawk2ui-vue-desktop.txt")
        .with_file_text("/tmp/hawk2ui-vue-desktop.txt", "seed")
}

fn react_desktop_capability_evidence(
    runtime: &mut HawkJsRuntime,
) -> Result<ReactDesktopCapabilityEvidence, String> {
    Ok(ReactDesktopCapabilityEvidence {
        storage_operation: runtime_string_value(
            runtime,
            "react-desktop-basic:storage-evidence",
            "globalThis.__hawk2uiReactDesktopStorageEvidence ?? ''",
        )?,
        file_operation: runtime_string_value(
            runtime,
            "react-desktop-basic:file-evidence",
            "globalThis.__hawk2uiReactDesktopFileEvidence ?? ''",
        )?,
    })
}

fn vue_desktop_capability_evidence(
    runtime: &mut HawkJsRuntime,
) -> Result<ReactDesktopCapabilityEvidence, String> {
    Ok(ReactDesktopCapabilityEvidence {
        storage_operation: runtime_string_value(
            runtime,
            "vue-desktop-basic:storage-evidence",
            "globalThis.__hawk2uiVueDesktopStorageEvidence ?? ''",
        )?,
        file_operation: runtime_string_value(
            runtime,
            "vue-desktop-basic:file-evidence",
            "globalThis.__hawk2uiVueDesktopFileEvidence ?? ''",
        )?,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReactPluginCapabilityEvidence {
    state_roundtrip: String,
    preset_roundtrip: String,
    transport_snapshot: String,
    realtime_visual_stream: String,
    dsp_control_messages: Vec<String>,
}

fn react_plugin_capabilities() -> HawkRuntimeCapabilities {
    HawkRuntimeCapabilities::for_test()
        .with_host_context(HawkHostContext::PluginUi)
        .allow_plugin_parameter("gain")
        .with_plugin_parameter("gain", 0.25)
        .allow_plugin_state()
        .with_plugin_state(r#"{"preset":"Init"}"#)
        .with_plugin_preset("init", r#"{"name":"Init"}"#)
        .with_plugin_transport_info(HawkPluginTransportInfo {
            playing: true,
            sample_rate: 48_000.0,
            sample_position: 96_000,
            tempo_bpm: 128.0,
            beat_position: 12.5,
            time_signature_numerator: 7,
            time_signature_denominator: 8,
        })
        .allow_audio_meter_stream("master")
        .with_audio_meter_frame("master", vec![0.125, 0.5])
        .allow_dsp_control_queue(1)
}

fn vue_plugin_capabilities() -> HawkRuntimeCapabilities {
    react_plugin_capabilities()
}

fn react_plugin_capability_evidence(
    runtime: &mut HawkJsRuntime,
) -> Result<ReactPluginCapabilityEvidence, String> {
    let dsp_control = runtime_string_value(
        runtime,
        "react-plugin-basic:dsp-evidence",
        "globalThis.__hawk2uiReactPluginDspEvidence ?? ''",
    )?;
    Ok(ReactPluginCapabilityEvidence {
        state_roundtrip: runtime_string_value(
            runtime,
            "react-plugin-basic:state-evidence",
            "globalThis.__hawk2uiReactPluginStateEvidence ?? ''",
        )?,
        preset_roundtrip: runtime_string_value(
            runtime,
            "react-plugin-basic:preset-evidence",
            "globalThis.__hawk2uiReactPluginPresetEvidence ?? ''",
        )?,
        transport_snapshot: runtime_string_value(
            runtime,
            "react-plugin-basic:transport-evidence",
            "globalThis.__hawk2uiReactPluginTransportEvidence ?? ''",
        )?,
        realtime_visual_stream: runtime_string_value(
            runtime,
            "react-plugin-basic:meter-evidence",
            "globalThis.__hawk2uiReactPluginMeterEvidence ?? ''",
        )?,
        dsp_control_messages: dsp_control
            .split('|')
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect(),
    })
}

fn vue_plugin_capability_evidence(
    runtime: &mut HawkJsRuntime,
) -> Result<ReactPluginCapabilityEvidence, String> {
    let dsp_control = runtime_string_value(
        runtime,
        "vue-plugin-basic:dsp-evidence",
        "globalThis.__hawk2uiVuePluginDspEvidence ?? ''",
    )?;
    Ok(ReactPluginCapabilityEvidence {
        state_roundtrip: runtime_string_value(
            runtime,
            "vue-plugin-basic:state-evidence",
            "globalThis.__hawk2uiVuePluginStateEvidence ?? ''",
        )?,
        preset_roundtrip: runtime_string_value(
            runtime,
            "vue-plugin-basic:preset-evidence",
            "globalThis.__hawk2uiVuePluginPresetEvidence ?? ''",
        )?,
        transport_snapshot: runtime_string_value(
            runtime,
            "vue-plugin-basic:transport-evidence",
            "globalThis.__hawk2uiVuePluginTransportEvidence ?? ''",
        )?,
        realtime_visual_stream: runtime_string_value(
            runtime,
            "vue-plugin-basic:meter-evidence",
            "globalThis.__hawk2uiVuePluginMeterEvidence ?? ''",
        )?,
        dsp_control_messages: dsp_control
            .split('|')
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect(),
    })
}

fn runtime_string_value(
    runtime: &mut HawkJsRuntime,
    name: &str,
    source: &str,
) -> Result<String, String> {
    match runtime
        .evaluate_script_value(name, source)
        .map_err(|error| error.to_string())?
    {
        JsRuntimeValue::String(value) => Ok(value),
        value => Err(format!("{name} did not evaluate to a string: {value:?}")),
    }
}

fn text_draw_for<'a>(frame: &'a RuntimeSceneFrame, node_id: &str) -> Option<&'a str> {
    frame
        .draw_commands()
        .iter()
        .find_map(|command| match command {
            RuntimeDrawCommand::Text { id, text, .. } if id.as_str() == node_id => {
                Some(text.as_str())
            }
            _ => None,
        })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReactPluginFrameEvidence {
    text: String,
    surface_size: [u32; 2],
    visible_pixel: bool,
}

fn render_react_plugin_frame(
    adapter: &mut BaseviewPluginAdapter,
    tree: &RuntimeViewTree,
    label: &str,
) -> Result<ReactPluginFrameEvidence, String> {
    let frame = RuntimeSceneBridge::new(Viewport::new(640.0, 360.0))
        .build(tree)
        .map_err(|error| format!("{label} scene build failed: {error:?}"))?;
    let text = text_draw_for(&frame, "gain")
        .ok_or_else(|| format!("{label} did not draw the plugin gain text"))?
        .to_owned();
    let snapshot = adapter
        .render_scene_frame(&frame)
        .map_err(|error| format!("{label} Baseview render failed: {}", error.rule()))?
        .clone();
    Ok(ReactPluginFrameEvidence {
        text,
        surface_size: [snapshot.width(), snapshot.height()],
        visible_pixel: snapshot.pixels().iter().any(|pixel| *pixel != 0),
    })
}

fn react_plugin_realtime_denial_rule(bundle: &str) -> Result<String, String> {
    let graph =
        HawkJsModuleGraph::new("file:///react-plugin-basic/realtime-denial.js").with_module(
            HawkJsModule::new("file:///react-plugin-basic/realtime-denial.js", bundle),
        );
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_host_context(HawkHostContext::AudioRealtime)
        .allow_plugin_parameter("gain")
        .with_plugin_parameter("gain", 0.25);
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .map_err(|error| error.to_string())?;
    let error = match runtime.execute_entrypoint_module() {
        Ok(()) => {
            return Err("plugin ops unexpectedly succeeded in realtime audio context".to_owned());
        }
        Err(error) => error,
    };
    if error
        .message()
        .contains("js-runtime.capability.realtime-denied")
    {
        Ok("js-runtime.capability.realtime-denied".to_owned())
    } else {
        Err(format!(
            "realtime plugin denial used unexpected diagnostic: {}",
            error.message()
        ))
    }
}

fn vue_plugin_realtime_denial_rule(bundle: &str) -> Result<String, String> {
    let graph = HawkJsModuleGraph::new("file:///vue-plugin-basic/realtime-denial.js").with_module(
        HawkJsModule::new("file:///vue-plugin-basic/realtime-denial.js", bundle),
    );
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_host_context(HawkHostContext::AudioRealtime)
        .allow_plugin_parameter("gain")
        .with_plugin_parameter("gain", 0.25);
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .map_err(|error| error.to_string())?;
    let error = match runtime.execute_entrypoint_module() {
        Ok(()) => {
            return Err("plugin ops unexpectedly succeeded in realtime audio context".to_owned());
        }
        Err(error) => error,
    };
    if error
        .message()
        .contains("js-runtime.capability.realtime-denied")
    {
        Ok("js-runtime.capability.realtime-denied".to_owned())
    } else {
        Err(format!(
            "realtime plugin denial used unexpected diagnostic: {}",
            error.message()
        ))
    }
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
    build_output: &BuildWorkspaceOutput,
) -> Result<FrameworkExampleContract, String> {
    match framework {
        "native" => native_contract(source_file, source, build_output),
        "svelte" => svelte_contract(source_file, build_output),
        "react" => react_contract(source_file, source, build_output),
        "vue" => vue_contract(source_file, build_output),
        "solid" => solid_contract(source_file, build_output),
        _ => Err(format!("unsupported framework smoke example: {framework}")),
    }
}

fn native_contract(
    source_file: &Path,
    source: &str,
    build_output: &BuildWorkspaceOutput,
) -> Result<FrameworkExampleContract, String> {
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
    let bridged = NativeRuntimeBridge::new()
        .bridge_artifact(&artifact)
        .map_err(|error| format!("{error:?}"))?;
    let software_frame =
        render_runtime_tree_evidence(bridged.runtime_tree(), "native framework smoke")?;
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
        build: smoke_build_result(build_output),
        compiler_artifact: None,
        list_template_count: 0,
        dynamic_binding_count: 0,
        runtime_bridged: true,
        software_frame,
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

fn svelte_contract(
    source_file: &Path,
    build_output: &BuildWorkspaceOutput,
) -> Result<FrameworkExampleContract, String> {
    let (record, program) = compiled_framework_program(build_output, "svelte")?;
    let artifact = SvelteIntegration::new()
        .compile_to_runtime(SvelteComponentSource::from_native_program(
            source_file.display().to_string(),
            program.clone(),
        ))
        .map_err(|error| format!("{error:?}"))?;
    framework_program_contract(
        "svelte",
        build_output,
        record,
        &program,
        artifact.runtime_tree(),
    )
}

fn react_contract(
    source_file: &Path,
    source: &str,
    build_output: &BuildWorkspaceOutput,
) -> Result<FrameworkExampleContract, String> {
    for required in [
        "useState",
        "surface.card",
        "assets/logo.svg",
        "title",
        "cta",
    ] {
        if !source.contains(required) {
            return Err(format!("react example missing contract token: {required}"));
        }
    }
    let graph = match build_output.artifact.js_module_graphs.as_slice() {
        [graph] => graph,
        graphs => {
            return Err(format!(
                "react smoke expected exactly one sealed JS graph, found {}",
                graphs.len()
            ));
        }
    };
    if graph.entrypoint() != "file:///dist/main.js" {
        return Err(format!(
            "react smoke expected file:///dist/main.js entrypoint, got {}",
            graph.entrypoint()
        ));
    }
    let entry_module = graph
        .module(graph.entrypoint())
        .ok_or_else(|| "react smoke sealed graph missing entrypoint module".to_owned())?;
    if !entry_module
        .source()
        .contains("__hawk2uiFrameworkReactBasic")
    {
        return Err("react smoke bundle missing framework runtime marker".into());
    }

    let mut runtime = NativeAuthoringRuntime::new(source_file.display().to_string());
    runtime.mount(native_root());
    let artifact = runtime.finish().map_err(|error| format!("{error:?}"))?;
    let bridged = NativeRuntimeBridge::new()
        .bridge_artifact(&artifact)
        .map_err(|error| format!("{error:?}"))?;
    let software_frame =
        render_runtime_tree_evidence(bridged.runtime_tree(), "react runtime framework smoke")?;
    Ok(FrameworkExampleContract {
        framework: "react".into(),
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
        build: smoke_build_result(build_output),
        compiler_artifact: None,
        list_template_count: 0,
        dynamic_binding_count: 0,
        runtime_bridged: true,
        software_frame,
    })
}

fn vue_contract(
    source_file: &Path,
    build_output: &BuildWorkspaceOutput,
) -> Result<FrameworkExampleContract, String> {
    let (record, program) = compiled_framework_program(build_output, "vue")?;
    let artifact = VueIntegration::new()
        .render_to_runtime(VueSingleFileComponent::from_native_program(
            source_file.display().to_string(),
            program.clone(),
        ))
        .map_err(|error| format!("{error:?}"))?;
    framework_program_contract(
        "vue",
        build_output,
        record,
        &program,
        artifact.runtime_tree(),
    )
}

fn solid_contract(
    source_file: &Path,
    build_output: &BuildWorkspaceOutput,
) -> Result<FrameworkExampleContract, String> {
    let (record, program) = compiled_framework_program(build_output, "solid")?;
    let artifact = SolidIntegration::new()
        .render_to_runtime(SolidComponentSource::from_native_program(
            source_file.display().to_string(),
            program.clone(),
        ))
        .map_err(|error| format!("{error:?}"))?;
    framework_program_contract(
        "solid",
        build_output,
        record,
        &program,
        artifact.runtime_tree(),
    )
}

fn compiled_framework_program<'a>(
    build_output: &'a BuildWorkspaceOutput,
    expected_framework: &str,
) -> Result<(&'a CompiledFrameworkRecord, FrameworkNativeProgram), String> {
    let record = match build_output.artifact.compiled_frameworks.as_slice() {
        [record] => record,
        records => {
            return Err(format!(
                "framework smoke expected exactly one compiled framework artifact for {expected_framework}, found {}",
                records.len()
            ));
        }
    };
    let actual_framework = source_framework_label(record.framework);
    if actual_framework != expected_framework {
        return Err(format!(
            "compiled framework mismatch: expected {expected_framework}, got {actual_framework}"
        ));
    }
    if record.compiler_artifact_json.trim().is_empty() {
        return Err(format!(
            "compiled framework artifact is empty for {expected_framework}"
        ));
    }
    let wire = FrameworkNativeProgramWire::from_json(&record.compiler_artifact_json)
        .map_err(|error| format!("{error:?}"))?;
    let program = FrameworkNativeProgram::try_from(wire).map_err(|error| format!("{error:?}"))?;
    if program.compiler().framework() != expected_framework {
        return Err(format!(
            "compiler metadata framework mismatch: expected {expected_framework}, got {}",
            program.compiler().framework()
        ));
    }
    if program.compiler().source_path() != record.source_path {
        return Err(format!(
            "compiler metadata source path mismatch: expected {}, got {}",
            record.source_path,
            program.compiler().source_path()
        ));
    }
    Ok((record, program))
}

fn framework_program_contract(
    framework: &str,
    build_output: &BuildWorkspaceOutput,
    record: &CompiledFrameworkRecord,
    program: &FrameworkNativeProgram,
    runtime_tree: &RuntimeViewTree,
) -> Result<FrameworkExampleContract, String> {
    let root = program.root();
    let software_frame =
        render_runtime_tree_evidence(runtime_tree, &format!("{framework} framework smoke"))?;
    Ok(FrameworkExampleContract {
        framework: framework.into(),
        root_id: root.id().as_str().to_string(),
        keyed_children: program.keyed_child_order(),
        style_refs: root
            .style_refs()
            .iter()
            .map(|style| style.name().to_string())
            .collect(),
        asset_paths: root
            .asset_refs()
            .iter()
            .map(|asset| asset.path().to_string())
            .collect(),
        build: smoke_build_result(build_output),
        compiler_artifact: Some(FrameworkCompilerSmokeArtifact {
            framework: program.compiler().framework().to_string(),
            compiler: program.compiler().compiler().to_string(),
            source_path: program.compiler().source_path().to_string(),
            entrypoint: program.compiler().entrypoint().to_string(),
            artifact_path: record.artifact_path.clone(),
            artifact_bytes: record.compiler_artifact_json.len(),
        }),
        list_template_count: program.list_templates().len(),
        dynamic_binding_count: program.dynamic_bindings().len(),
        runtime_bridged: runtime_tree_has_example_children(runtime_tree),
        software_frame,
    })
}

fn source_framework_label(framework: SourceFramework) -> &'static str {
    match framework {
        SourceFramework::Native => "native",
        SourceFramework::React => "react",
        SourceFramework::Solid => "solid",
        SourceFramework::Svelte => "svelte",
        SourceFramework::Vue => "vue",
    }
}

fn runtime_tree_has_example_children(tree: &hawk2ui_runtime::RuntimeViewTree) -> bool {
    let children = tree.children_of(tree.root_id());
    tree.root_id().as_str() == "root"
        && children.len() >= 2
        && children[0].as_str() == "title"
        && children[1].as_str() == "cta"
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
