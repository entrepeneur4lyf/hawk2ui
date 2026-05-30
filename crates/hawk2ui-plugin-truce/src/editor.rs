//! The `Hawk2UI` editor surface for a truce plugin.
//!
//! [`Hawk2uiTruceEditor`] implements truce's [`Editor`] seam: on
//! [`Editor::open`] it attaches a Baseview child window to the DAW-provided
//! parent and renders the `Hawk2UI` runtime scene into it through
//! `hawk2ui-host-baseview`'s Skia Ganesh GPU presentation path. The
//! window-owning machinery and the raw-window-handle FFI live in
//! `hawk2ui-host-baseview`
//! (the workspace's sole `unsafe`-permitting crate); this module is
//! `unsafe`-free — [`Hawk2uiTruceEditor`] is `Send` because every field is
//! already `Send`, including the [`BaseviewEditorWindowHandle`] that carries
//! the contained `unsafe impl Send`.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use hawk2ui_host::{HostPlatformHandle, PluginEditorConfig};
use hawk2ui_host_baseview::{
    BaseviewEditorWindowHandle, BaseviewHostError, BaseviewParentFixture, BaseviewPluginAdapter,
};
use hawk2ui_runtime::RuntimeSceneFrame;
use hawk2ui_script::HostSnapshot;
use truce_core::editor::{Editor, EditorBridge, PluginContext, RawWindowHandle};

use crate::scene::{EditorSceneError, build_editor_scene, build_error_scene};

/// Stable parent-fixture identifier for the truce editor surface.
const EDITOR_FIXTURE_ID: &str = "hawk2ui-truce-editor";

/// Placeholder X11 display handle.
///
/// Truce's [`RawWindowHandle::X11`] carries only the parent window id, whereas
/// `Hawk2UI`'s [`HostPlatformHandle::LinuxX11`] also records a display handle.
/// Baseview opens its own X11 connection from `$DISPLAY` and parents off the
/// window id — for both the GLX context and X11 — so the recorded display is a
/// non-zero placeholder rather than a live pointer.
const X11_PLACEHOLDER_DISPLAY: u64 = 1;

/// Converts a truce parent window handle into `Hawk2UI`'s host platform handle.
///
/// Pure data repackaging: truce captures the native handle as a raw pointer
/// (`Win32`) or an X11 window id, and `Hawk2UI` stores it as an integer record.
/// No pointer is dereferenced, so the conversion stays `unsafe`-free.
///
/// # Errors
///
/// Returns [`BaseviewHostError`] for handle variants `Hawk2UI` cannot yet
/// embed: macOS `AppKit` (truce exposes only an `NSView`, but Baseview
/// attachment also needs the owning `NSWindow`) and iOS `UiKit` (not a
/// `Hawk2UI` target).
fn host_handle_from_truce(
    parent: RawWindowHandle,
) -> Result<HostPlatformHandle, BaseviewHostError> {
    match parent {
        RawWindowHandle::Win32(hwnd) => Ok(HostPlatformHandle::windows_hwnd(hwnd.addr() as u64)),
        RawWindowHandle::X11(window) => Ok(HostPlatformHandle::linux_x11(
            X11_PLACEHOLDER_DISPLAY,
            window,
        )),
        RawWindowHandle::AppKit(_) => Err(BaseviewHostError::new(
            "hawk2ui-truce.parent.appkit-unsupported",
            "macOS AppKit parenting needs the owning NSWindow alongside the NSView that truce exposes; supported with the GPU presentation path",
        )),
        RawWindowHandle::UiKit(_) => Err(BaseviewHostError::new(
            "hawk2ui-truce.parent.uikit-unsupported",
            "iOS UIKit editor surfaces are not a Hawk2UI target",
        )),
    }
}

/// Logical editor size in points, derived from the plugin editor metrics.
fn logical_size(config: &PluginEditorConfig) -> (u32, u32) {
    let metrics = config.metrics;
    // Editor logical dimensions are small, non-negative point values; rounding
    // to the nearest integer point cannot truncate or lose sign in practice.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        (
            metrics.logical_width.round() as u32,
            metrics.logical_height.round() as u32,
        )
    }
}

/// Editor scene dimensions in points: the logical size widened to `f32` for the
/// scene viewport.
fn scene_dimensions(config: &PluginEditorConfig) -> (f32, f32) {
    let (width, height) = logical_size(config);
    // Editor logical dimensions are small point values; widening them to f32
    // cannot lose meaningful precision.
    #[allow(clippy::cast_precision_loss)]
    {
        (width as f32, height as f32)
    }
}

/// A `Hawk2UI`-rendered editor surface for a truce plugin.
///
/// Implements truce's [`Editor`] seam, drawing the `Hawk2UI` runtime scene into
/// a Baseview child window parented to the DAW-provided window. Construct one
/// with [`Hawk2uiTruceEditor::from_entry_script`] and return it from a truce
/// `PluginLogic::editor()` implementation.
pub struct Hawk2uiTruceEditor {
    config: PluginEditorConfig,
    size: (u32, u32),
    scene: Option<RuntimeSceneFrame>,
    adapter: Option<BaseviewPluginAdapter>,
    window: Option<BaseviewEditorWindowHandle>,
    /// INVARIANT (Decision 0003 D4 / Lock 3): the editor holds the host **bridge
    /// only** — never truce's typed parameter store. The bridge's parameter reads
    /// (`get_param` / `get_param_plain`) are the non-advancing "host→GUI sync"
    /// path; the typed store would instead expose a `FloatParam` whose advancing
    /// `read()` (a smoother step meant for `process()`) could perturb the audio
    /// thread from a GUI repaint. Reading only through the bridge makes that
    /// advancing read **unreachable**. Rust cannot assert "this struct has no
    /// such field" at compile time, so a source-pattern conformance gate enforces
    /// it: `hawk2ui-conformance/tests/source_hygiene.rs`,
    /// `truce_editor_crate_never_captures_a_param_store`.
    bridge: Option<Arc<dyn EditorBridge>>,
    presented_frames: Arc<AtomicU64>,
    last_error: Arc<Mutex<Option<BaseviewHostError>>>,
}

impl Hawk2uiTruceEditor {
    /// Creates an editor that renders a pre-built `scene` for the plugin
    /// described by `config`.
    ///
    /// Crate-internal: the public entry point is [`Self::from_entry_script`],
    /// which builds the scene from the author's compiled entry script. This
    /// lower-level constructor is retained for the in-crate tests that supply a
    /// hand-built scene.
    #[must_use]
    pub(crate) fn new(config: PluginEditorConfig, scene: RuntimeSceneFrame) -> Self {
        let size = logical_size(&config);
        Self {
            config,
            size,
            scene: Some(scene),
            adapter: None,
            window: None,
            bridge: None,
            presented_frames: Arc::new(AtomicU64::new(0)),
            last_error: Arc::new(Mutex::new(None)),
        }
    }

    /// Creates an editor that renders the plugin's compiled entry script,
    /// falling back to a visible error panel if the script cannot build a scene.
    ///
    /// This is the production entry point. Truce's `PluginLogic::editor()`
    /// returns a `Box<dyn Editor>` and so cannot propagate a failure, and a
    /// plugin editor embedded in a DAW must never crash or blank the host. So
    /// this constructor is **infallible**: a broken author script yields an
    /// editor that presents a legible error scene and records the error
    /// (observable through [`Self::has_error`]) instead of failing construction.
    /// Use [`Self::try_from_entry_script`] when the caller wants to inspect or
    /// handle the build error itself.
    ///
    /// `snapshot` is projected into the script's `host` (parameters and meters,
    /// read by string key); pass an empty [`HostSnapshot`] for a paramless
    /// editor. It carries the model's declared defaults today — re-projecting it
    /// from the live truce `EditorBridge` is task 0009.4.
    #[must_use]
    pub fn from_entry_script(
        config: PluginEditorConfig,
        compiled_source: &str,
        source_path: &str,
        snapshot: &HostSnapshot,
    ) -> Self {
        let (width, height) = scene_dimensions(&config);
        match build_editor_scene(compiled_source, source_path, snapshot, width, height) {
            Ok(scene) => Self::new(config, scene),
            Err(error) => {
                // The script failed; still present something diagnosable. If even
                // the error panel cannot build (unreachable in practice), degrade
                // to no scene — `open` then records and presents nothing rather
                // than panicking.
                let fallback = build_error_scene(&error.to_string(), width, height).ok();
                Self::errored(config, fallback, &error)
            }
        }
    }

    /// Creates an editor whose scene is built by running the plugin's compiled
    /// entry script, returning the build error to the caller.
    ///
    /// The script's `mount` function is executed (see [`build_editor_scene`])
    /// and its returned node tree becomes the initial [`RuntimeSceneFrame`] the
    /// editor renders, sized to the plugin editor's logical dimensions.
    /// `snapshot` is projected into the script's `host` (parameters and meters,
    /// read by string key), carrying the model's declared defaults; re-projecting
    /// it from the live [`Editor::open`] bridge is task 0009.4.
    ///
    /// Prefer [`Self::from_entry_script`] at the truce `editor()` boundary, which
    /// cannot propagate a `Result`; reach for this when the caller wants the
    /// error (tooling, tests, a custom fallback policy).
    ///
    /// # Errors
    ///
    /// Returns an [`EditorSceneError`] when the entry script cannot be executed
    /// or its result cannot be converted into a renderable scene.
    pub fn try_from_entry_script(
        config: PluginEditorConfig,
        compiled_source: &str,
        source_path: &str,
        snapshot: &HostSnapshot,
    ) -> Result<Self, EditorSceneError> {
        let (width, height) = scene_dimensions(&config);
        let scene = build_editor_scene(compiled_source, source_path, snapshot, width, height)?;
        Ok(Self::new(config, scene))
    }

    /// Builds an editor already in an error state: it presents `scene` (a
    /// fallback error panel, or nothing if even that could not build) and
    /// reports `error` through [`Self::has_error`].
    fn errored(
        config: PluginEditorConfig,
        scene: Option<RuntimeSceneFrame>,
        error: &EditorSceneError,
    ) -> Self {
        let size = logical_size(&config);
        Self {
            config,
            size,
            scene,
            adapter: None,
            window: None,
            bridge: None,
            presented_frames: Arc::new(AtomicU64::new(0)),
            last_error: Arc::new(Mutex::new(Some(BaseviewHostError::new(
                error.rule(),
                error.message(),
            )))),
        }
    }

    /// The runtime scene this editor renders, or `None` when scene construction
    /// failed catastrophically and the editor has nothing to present.
    #[must_use]
    pub fn scene(&self) -> Option<&RuntimeSceneFrame> {
        self.scene.as_ref()
    }

    /// The host bridge captured on [`Editor::open`], or `None` before open.
    #[must_use]
    pub fn bridge(&self) -> Option<&Arc<dyn EditorBridge>> {
        self.bridge.as_ref()
    }

    /// Number of scene frames presented into the editor window so far.
    #[must_use]
    pub fn presented_frame_count(&self) -> u64 {
        self.presented_frames.load(Ordering::SeqCst)
    }

    /// Whether the editor has recorded an error — either during scene
    /// construction (a failed entry script) or during presentation.
    #[must_use]
    pub fn has_error(&self) -> bool {
        self.last_error.lock().is_ok_and(|guard| guard.is_some())
    }

    fn record_error(&self, error: BaseviewHostError) {
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = Some(error);
        }
    }

    #[cfg(target_os = "linux")]
    fn open_window(&mut self, adapter: &BaseviewPluginAdapter) {
        let Some(scene) = self.scene.clone() else {
            self.record_error(BaseviewHostError::new(
                "hawk2ui-truce.present.no-scene",
                "editor has no renderable scene to present; scene construction failed",
            ));
            return;
        };
        // The frame handler records into its own clones of these; the editor
        // observes progress through `presented_frame_count` / `has_error`.
        let events = Arc::new(Mutex::new(Vec::new()));
        match adapter.open_gpu_editor_window(
            scene,
            Arc::clone(&self.presented_frames),
            Arc::clone(&self.last_error),
            events,
        ) {
            Ok(window) => self.window = Some(window),
            Err(error) => self.record_error(error),
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn open_window(&mut self, _adapter: &BaseviewPluginAdapter) {
        self.record_error(BaseviewHostError::new(
            "hawk2ui-truce.present.unsupported-platform",
            "Hawk2UI truce editor GPU presentation is currently wired for Linux; macOS and Windows arrive once their Baseview parent-handle conversion lands",
        ));
    }
}

impl Editor for Hawk2uiTruceEditor {
    fn size(&self) -> (u32, u32) {
        self.size
    }

    fn open(&mut self, parent: RawWindowHandle, context: PluginContext) {
        self.bridge = Some(Arc::clone(context.bridge()));

        let host_handle = match host_handle_from_truce(parent) {
            Ok(handle) => handle,
            Err(error) => {
                self.record_error(error);
                return;
            }
        };
        let fixture = BaseviewParentFixture::from_platform_handle(EDITOR_FIXTURE_ID, host_handle);
        let adapter = match BaseviewPluginAdapter::attach(self.config.clone(), fixture) {
            Ok(adapter) => adapter,
            Err(error) => {
                self.record_error(error);
                return;
            }
        };
        self.open_window(&adapter);
        self.adapter = Some(adapter);
    }

    fn close(&mut self) {
        if let Some(mut window) = self.window.take() {
            window.close();
        }
    }

    fn set_scale_factor(&mut self, factor: f64) {
        // Keep the adapter's metrics in sync; the software path applies the new
        // scale on its next presented frame.
        let result = self
            .adapter
            .as_mut()
            .map(|adapter| adapter.try_dpi_changed(factor));
        if let Some(Err(error)) = result {
            self.record_error(error);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ptr;

    use hawk2ui_host::{PluginParentHandle, SurfaceMetrics};
    use hawk2ui_layout::{FlexDirection, LayoutSizing, LayoutStyle, Viewport};
    use hawk2ui_render::Color;
    use hawk2ui_runtime::{
        RuntimeSceneBridge, RuntimeViewId, RuntimeViewNode, RuntimeViewTree, RuntimeVisual,
    };

    use super::*;

    fn test_scene() -> RuntimeSceneFrame {
        let tree = RuntimeViewTree::new(RuntimeViewNode::new(
            RuntimeViewId::new("truce-editor-root"),
            LayoutStyle::flex_container(FlexDirection::Column)
                .with_size(LayoutSizing::fixed(320.0, 180.0)),
            RuntimeVisual::Fill(Color::rgba(32, 96, 180, 255)),
        ));
        RuntimeSceneBridge::new(Viewport::new(320.0, 180.0))
            .build(&tree)
            .expect("truce editor test scene builds")
    }

    fn test_config() -> PluginEditorConfig {
        PluginEditorConfig::new(
            "truce-editor-test",
            PluginParentHandle::opaque("truce-test-parent"),
            SurfaceMetrics::new(320.0, 180.0, 1.0),
        )
    }

    #[test]
    fn converts_win32_parent_handle_to_integer_record() {
        let handle =
            host_handle_from_truce(RawWindowHandle::Win32(ptr::without_provenance_mut(0x1234)))
                .expect("win32 handle converts");
        assert_eq!(handle, HostPlatformHandle::windows_hwnd(0x1234));
    }

    #[test]
    fn converts_x11_parent_handle_with_placeholder_display() {
        let handle =
            host_handle_from_truce(RawWindowHandle::X11(0xABCD)).expect("x11 handle converts");
        assert_eq!(
            handle,
            HostPlatformHandle::linux_x11(X11_PLACEHOLDER_DISPLAY, 0xABCD)
        );
    }

    #[test]
    fn rejects_appkit_and_uikit_parent_handles() {
        let appkit =
            host_handle_from_truce(RawWindowHandle::AppKit(ptr::without_provenance_mut(1)))
                .expect_err("appkit is rejected");
        assert_eq!(appkit.rule(), "hawk2ui-truce.parent.appkit-unsupported");

        let uikit = host_handle_from_truce(RawWindowHandle::UiKit(ptr::without_provenance_mut(1)))
            .expect_err("uikit is rejected");
        assert_eq!(uikit.rule(), "hawk2ui-truce.parent.uikit-unsupported");
    }

    #[test]
    fn reports_logical_size_and_initial_state() {
        let editor = Hawk2uiTruceEditor::new(test_config(), test_scene());
        assert_eq!(Editor::size(&editor), (320, 180));
        assert_eq!(editor.presented_frame_count(), 0);
        assert!(!editor.has_error());
        assert!(editor.bridge().is_none());
    }

    #[test]
    fn from_entry_script_falls_back_to_a_visible_error_panel_on_a_broken_script() {
        // truce's `editor()` cannot propagate a Result, so the production
        // constructor is infallible: a broken author script must still yield an
        // editor that presents a (legible error) scene and records the error —
        // never a failed construction that the caller cannot handle.
        let editor = Hawk2uiTruceEditor::from_entry_script(
            test_config(),
            "const broken = 1;",
            "src/editor.js",
            &HostSnapshot::default(),
        );
        assert!(
            editor.scene().is_some(),
            "a fallback error scene must be present"
        );
        assert!(
            editor.has_error(),
            "the construction failure must be recorded"
        );
    }

    #[test]
    fn from_entry_script_renders_a_valid_script_without_error() {
        let editor = Hawk2uiTruceEditor::from_entry_script(
            test_config(),
            "export function mount(host) { return { id: \"root\", type: \"view\" }; }",
            "src/editor.js",
            &HostSnapshot::default(),
        );
        assert!(editor.scene().is_some());
        assert!(!editor.has_error());
    }

    #[test]
    fn try_from_entry_script_surfaces_the_build_error() {
        let result = Hawk2uiTruceEditor::try_from_entry_script(
            test_config(),
            "const broken = 1;",
            "src/editor.js",
            &HostSnapshot::default(),
        );
        // `Hawk2uiTruceEditor` is not `Debug` (it holds an `Arc<dyn EditorBridge>`),
        // so destructure rather than `expect_err`.
        let Err(error) = result else {
            panic!("a script without a mount function must fail");
        };
        assert_eq!(error.rule(), "hawk2ui-truce.editor.no-mount");
    }

    #[test]
    fn editor_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Hawk2uiTruceEditor>();
    }
}
