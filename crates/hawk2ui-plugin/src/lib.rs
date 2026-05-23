#![forbid(unsafe_code)]
//! Format-neutral plugin metadata, editor, parameter, automation, state, preset, and realtime visual data records for `Hawk2UI`.

pub mod automation;
pub mod editor;
pub mod format;
pub mod parameter;

pub use automation::{
    AutomationBindingKind, AutomationEvent, AutomationEventError, AutomationEventKind,
    AutomationOrigin, AutomationSequence, ParameterBinding,
};
pub use editor::{
    EditorEvent, EditorKind, EditorParent, PluginEditor, PluginEditorLifecycle, PluginEditorSize,
};
pub use format::{
    BundleOutput, FormatMetadata, FormatValidationError, PackageTarget, PluginFormat,
    PluginFormatTarget,
};
pub use parameter::{
    GeneratedParameterMetadata, ParameterDistribution, ParameterFlags, ParameterGroup,
    ParameterModel, ParameterRange, ParameterRecord, ParameterSmoothing, ParameterValidationError,
    ParameterValue,
};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-plugin";

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
        assert_eq!(crate_name(), "hawk2ui-plugin");
    }
}
