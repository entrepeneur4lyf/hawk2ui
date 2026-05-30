#![forbid(unsafe_code)]
//! Truce audio-plugin framework binding for `Hawk2UI`.
//!
//! `Hawk2UI`'s multi-format audio-plugin backend is built on the
//! [`truce`](https://github.com/truce-audio/truce) framework. This crate
//! implements truce's [`truce_core::editor::Editor`] seam by re-hosting the
//! existing baseview + Skia editor render path owned by `hawk2ui-host-baseview`
//! inside a truce plugin — so the editor surface is drawn by `Hawk2UI`'s own
//! renderer while truce supplies the CLAP / VST3 / AU / AAX / LV2 / standalone
//! format wrappers, the parameter system, and the host bridge.
//!
//! The window-owning machinery and the raw-window-handle FFI stay inside
//! `hawk2ui-host-baseview` (the workspace's sole `unsafe`-permitting crate);
//! this crate is `unsafe`-free and drives that machinery through safe APIs.

/// Truce's plugin-editor seam. `Hawk2UI`'s editor type implements this so a
/// truce `PluginLogic::editor()` can return a `Hawk2UI`-rendered surface.
pub use truce_core::editor::Editor as TruceEditor;

/// Truce's parameter trait, backing the `PluginContext<P>` handed to the
/// editor on open.
pub use truce_params::Params as TruceParams;

/// The C3 editor protocol types from `hawk2ui-script`: the parameter/meter read
/// projection an editor's `mount(host)` reads ([`HostSnapshot`] and friends), the
/// ordered write edits it emits ([`HostEdit`]), and the host-side write
/// [`EditRouting`]. Build the snapshot and routing from a parameter model with
/// `hawk2ui-build`'s `host_snapshot_from_model` / `edit_routing_from_model`, or by
/// hand from these types. Re-exported so a caller can name the editor's full
/// construction and replay surface from this crate alone.
pub use hawk2ui_script::{
    EditRouting, HostEdit, HostMeter, HostParam, HostParamKind, HostParamValue, HostSnapshot,
    ParamRoute,
};

pub mod editor;
mod replay;
mod scene;

pub use editor::Hawk2uiTruceEditor;
pub use replay::{EditReplayDiagnostic, replay_edits};
pub use scene::EditorSceneError;

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-plugin-truce";

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
        assert_eq!(crate_name(), "hawk2ui-plugin-truce");
    }
}
