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
use truce_core::editor::{Editor, EditorBridge, PluginContext, RawWindowHandle};

use crate::scene::{EditorSceneError, build_editor_scene};

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

/// A `Hawk2UI`-rendered editor surface for a truce plugin.
///
/// Implements truce's [`Editor`] seam, drawing the `Hawk2UI` runtime scene into
/// a Baseview child window parented to the DAW-provided window. Construct one
/// with [`Hawk2uiTruceEditor::new`] and return it from a truce
/// `PluginLogic::editor()` implementation.
pub struct Hawk2uiTruceEditor {
    config: PluginEditorConfig,
    size: (u32, u32),
    scene: RuntimeSceneFrame,
    adapter: Option<BaseviewPluginAdapter>,
    window: Option<BaseviewEditorWindowHandle>,
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
            scene,
            adapter: None,
            window: None,
            bridge: None,
            presented_frames: Arc::new(AtomicU64::new(0)),
            last_error: Arc::new(Mutex::new(None)),
        }
    }

    /// Creates an editor whose scene is built by running the plugin's compiled
    /// entry script.
    ///
    /// The script's `mount` function is executed (see [`build_editor_scene`])
    /// and its returned node tree becomes the initial [`RuntimeSceneFrame`] the
    /// editor renders, sized to the plugin editor's logical dimensions. No host
    /// bindings are projected into the script yet — the bridge captured on
    /// [`Editor::open`] is unused until parameter and meter projection lands.
    ///
    /// # Errors
    ///
    /// Returns an [`EditorSceneError`] when the entry script cannot be executed
    /// or its result cannot be converted into a renderable scene.
    pub fn from_entry_script(
        config: PluginEditorConfig,
        compiled_source: &str,
        source_path: &str,
    ) -> Result<Self, EditorSceneError> {
        let (width, height) = logical_size(&config);
        // Editor logical dimensions are small point values; widening them to
        // f32 for the scene viewport cannot lose meaningful precision.
        #[allow(clippy::cast_precision_loss)]
        let scene = build_editor_scene(compiled_source, source_path, width as f32, height as f32)?;
        Ok(Self::new(config, scene))
    }

    /// The runtime scene this editor renders.
    #[must_use]
    pub const fn scene(&self) -> &RuntimeSceneFrame {
        &self.scene
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

    /// Whether the editor's frame handler has recorded a presentation error.
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
        // The frame handler records into its own clones of these; the editor
        // observes progress through `presented_frame_count` / `has_error`.
        let events = Arc::new(Mutex::new(Vec::new()));
        match adapter.open_gpu_editor_window(
            self.scene.clone(),
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
    fn editor_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Hawk2uiTruceEditor>();
    }
}
