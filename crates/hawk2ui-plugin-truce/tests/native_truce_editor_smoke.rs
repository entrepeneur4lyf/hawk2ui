#![cfg(target_os = "linux")]
//! Gated native smoke: builds the editor scene from a compiled entry script
//! (the `from_entry_script` path — boa runs the script's `mount`), drives
//! [`Hawk2uiTruceEditor::open`] against a real X11 parent, and asserts the
//! editor presents at least one frame and captures the host bridge. Mirrors
//! `hawk2ui-host-baseview`'s `native_parented_smoke`; set
//! `HAWK2UI_NATIVE_BASEVIEW_SMOKE=1` (with a live X server) to actually run it.

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use hawk2ui_host::{PluginEditorConfig, PluginParentHandle, SurfaceMetrics};
use hawk2ui_plugin_truce::{Hawk2uiTruceEditor, HostSnapshot};
use truce::prelude::{FloatParam, Params};
use truce_core::editor::{Editor, RawWindowHandle, for_test_params};
use x11rb::{
    COPY_DEPTH_FROM_PARENT,
    connection::Connection,
    protocol::xproto::{ConnectionExt, CreateWindowAux, EventMask, WindowClass},
    rust_connection::RustConnection,
};

/// Minimal param set for the editor smoke. The sealed `Params` trait can only
/// be produced by `#[derive(Params)]`, which requires at least one parameter
/// field; the editor render path here reads no parameters, so a single unused
/// control is sufficient to build the `PluginContext` that `open` requires.
#[derive(Params)]
struct TestParams {
    #[param(
        name = "Level",
        range = "linear(0, 1)",
        unit = "none",
        smooth = "exp(5)"
    )]
    level: FloatParam,
}

#[test]
fn truce_editor_opens_real_x11_window_and_presents_when_smoke_enabled() {
    if std::env::var("HAWK2UI_NATIVE_BASEVIEW_SMOKE")
        .ok()
        .as_deref()
        != Some("1")
    {
        eprintln!("skipping native truce editor smoke; set HAWK2UI_NATIVE_BASEVIEW_SMOKE=1");
        return;
    }

    let parent = X11ParentWindow::open().expect("x11 parent window opens");
    let mut editor = Hawk2uiTruceEditor::try_from_entry_script(
        editor_config(),
        ENTRY_SOURCE,
        "src/editor.js",
        &HostSnapshot::default(),
    )
    .expect("editor builds a scene from the entry script");

    let params: Arc<dyn truce_params::Params> = Arc::new(TestParams::default());
    editor.open(
        RawWindowHandle::X11(u64::from(parent.window)),
        for_test_params(params),
    );

    let deadline = Instant::now() + Duration::from_secs(3);
    while editor.presented_frame_count() == 0 && !editor.has_error() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }

    editor.close();

    assert!(
        !editor.has_error(),
        "truce editor recorded a presentation error during open"
    );
    assert!(
        editor.presented_frame_count() >= 1,
        "expected the truce editor to present at least one frame"
    );
    assert!(
        editor.bridge().is_some(),
        "open should capture the host editor bridge"
    );
}

fn editor_config() -> PluginEditorConfig {
    PluginEditorConfig::new(
        "native-truce-editor-smoke",
        PluginParentHandle::opaque("x11-parent"),
        SurfaceMetrics::new(320.0, 180.0, 1.0),
    )
}

/// Entry script whose `mount` returns the editor's root view — a blue fill with
/// a title — driven through the real `from_entry_script` → boa → scene path so
/// the smoke renders a from-script scene rather than a hand-built one.
const ENTRY_SOURCE: &str = r##"
export function mount(host) {
    return {
        id: "native-truce-editor-root",
        type: "view",
        props: { backgroundColor: "#2060b4" },
        children: [
            { id: "native-truce-editor-title", type: "text", text: "Hawk2UI plugin editor" }
        ]
    };
}
"##;

struct X11ParentWindow {
    connection: RustConnection,
    window: u32,
}

impl X11ParentWindow {
    fn open() -> Result<Self, String> {
        let (connection, screen_number) =
            x11rb::connect(None).map_err(|error| format!("x11 connect failed: {error}"))?;
        let screen = &connection.setup().roots[screen_number];
        let window = connection
            .generate_id()
            .map_err(|error| format!("x11 window id allocation failed: {error}"))?;
        connection
            .create_window(
                COPY_DEPTH_FROM_PARENT,
                window,
                screen.root,
                0,
                0,
                320,
                180,
                0,
                WindowClass::INPUT_OUTPUT,
                0,
                &CreateWindowAux::new()
                    .background_pixel(screen.black_pixel)
                    .event_mask(EventMask::EXPOSURE | EventMask::STRUCTURE_NOTIFY),
            )
            .map_err(|error| format!("x11 parent window creation failed: {error}"))?;
        connection
            .map_window(window)
            .map_err(|error| format!("x11 parent window map failed: {error}"))?;
        connection
            .flush()
            .map_err(|error| format!("x11 parent window flush failed: {error}"))?;
        Ok(Self { connection, window })
    }
}

impl Drop for X11ParentWindow {
    fn drop(&mut self) {
        let _ = self.connection.destroy_window(self.window);
        let _ = self.connection.flush();
    }
}
