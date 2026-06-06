#![forbid(unsafe_code)]
//! Truce export fixture whose [`PluginLogic::editor`] returns the `Hawk2UI`
//! truce editor.
//!
//! The fixture composes the editor into a truce [`Plugin`] through the real
//! `truce::plugin!` export macro. That keeps the production artifact-to-editor
//! seam covered by compiled code: truce must accept
//! `Box<dyn Editor> = Hawk2uiTruceEditor`, and the editor entry script must
//! build a clean scene.
//!
//! The [`FixtureParams`] struct and `ENTRY_SOURCE` intentionally mirror the
//! codegen shape emitted for packaged plugins: a `#[derive(Params)]` parameter
//! struct, manifest-derived host projection records, and an editor source pulled
//! through [`Hawk2uiTruceEditor::from_entry_script`].

use std::sync::Arc;

use truce::prelude::*;

use hawk2ui_host::{PluginEditorConfig, PluginParentHandle, SurfaceMetrics};
use hawk2ui_plugin_truce::{EditRouting, Hawk2uiTruceEditor, HostSnapshot};

/// One continuous control, mirroring what `emit_truce_params_struct` produces
/// for a single float parameter (`#[param(id = N, ...)]`). truce's `Params`
/// trait is sealed — only `#[derive(Params)]` can satisfy it — so production
/// codegen emits exactly this shape from the manifest.
#[derive(Params)]
pub struct FixtureParams {
    #[param(
        id = 0,
        name = "Level",
        range = "linear(0, 1)",
        unit = "none",
        default = 0.5
    )]
    pub level: FloatParam,
}

/// Editor-only passthrough plugin. `Hawk2UI` authors write the editor, not the
/// DSP, so `process` is a straight input->output copy; the covered behavior is
/// [`PluginLogic::editor`].
pub struct FixturePlugin {
    params: Arc<FixtureParams>,
}

impl FixturePlugin {
    /// Builds the plugin around its parameter struct. truce's export macro calls
    /// this with the `Arc<FixtureParams>` it owns.
    #[must_use]
    pub fn new(params: Arc<FixtureParams>) -> Self {
        Self { params }
    }
}

impl PluginLogic for FixturePlugin {
    fn reset(&mut self, sample_rate: f64, _max_block_size: usize) {
        self.params.set_sample_rate(sample_rate);
    }

    fn process(
        &mut self,
        buffer: &mut AudioBuffer,
        _events: &EventList,
        _context: &mut ProcessContext,
    ) -> ProcessStatus {
        for ch in 0..buffer.channels() {
            let (input, output) = buffer.io(ch);
            output.copy_from_slice(input);
        }
        ProcessStatus::Normal
    }

    fn editor(&self) -> Box<dyn Editor> {
        // `from_entry_script` is the infallible production entry point: a broken
        // script yields a legible error scene rather than failing construction,
        // which is exactly what `editor()` needs since it cannot return a Result.
        Box::new(Hawk2uiTruceEditor::from_entry_script(
            editor_config(),
            ENTRY_SOURCE,
            "src/editor.js",
            &HostSnapshot::default(),
            &EditRouting::default(),
        ))
    }
}

truce::plugin! {
    logic: FixturePlugin,
    params: FixtureParams,
}

/// The editor surface configuration: a fixed 320x180 logical editor at 1.0
/// scale. The parent handle is opaque here — the real handle arrives through
/// `Editor::open` from the host.
fn editor_config() -> PluginEditorConfig {
    PluginEditorConfig::new(
        "hawk2ui-plugin-truce-fixture",
        PluginParentHandle::opaque("truce-fixture-parent"),
        SurfaceMetrics::new(320.0, 180.0, 1.0),
    )
}

/// A minimal compiled entry whose `mount` returns the editor's root view.
const ENTRY_SOURCE: &str = r##"
export function mount(host) {
    return {
        id: "truce-fixture-root",
        type: "view",
        props: { backgroundColor: "#2060b4" },
        children: [
            { id: "truce-fixture-title", type: "text", text: "Hawk2UI plugin editor" }
        ]
    };
}
"##;

#[cfg(test)]
mod tests {
    use super::*;

    /// The composition proof: `truce::plugin!` only produces a `Plugin` that
    /// implements `PluginExport` if it accepted everything `PluginLogic`
    /// returned — including `editor()` handing back a
    /// `Box<dyn Editor> = Hawk2uiTruceEditor`. Constructing one exercises that.
    #[test]
    fn export_macro_accepts_the_hawk2ui_editor() {
        let _plugin: Plugin = <Plugin as truce_core::export::PluginExport>::create();
    }

    /// The editor truce will hand the host is a clean, sized `Hawk2UI` editor.
    ///
    /// `size()` alone is a weak check: it returns the configured size whether the
    /// scene built or `from_entry_script` fell back to an error scene. So this
    /// builds the concrete editor `editor()` returns and asserts `!has_error()` —
    /// proving the embedded entry source actually builds a real scene — then
    /// confirms the same construction reached through the `PluginLogic::editor`
    /// seam truce calls is a sized `Box<dyn Editor>`.
    #[test]
    fn editor_builds_clean_and_sizes_through_the_trait() {
        let editor = Hawk2uiTruceEditor::from_entry_script(
            editor_config(),
            ENTRY_SOURCE,
            "src/editor.js",
            &HostSnapshot::default(),
            &EditRouting::default(),
        );
        assert!(
            !editor.has_error(),
            "embedded entry source must build a clean scene, not the error fallback"
        );
        assert_eq!(editor.size(), (320, 180));

        let boxed = PluginLogic::editor(&FixturePlugin::new(Arc::new(FixtureParams::default())));
        assert_eq!(boxed.size(), (320, 180));
    }
}
