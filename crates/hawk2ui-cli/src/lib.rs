#![forbid(unsafe_code)]
//! Reusable command definitions and orchestration for the `Hawk2UI` CLI.

pub mod commands;
pub mod dev_loop;
pub mod diagnostics;
pub mod executor;
pub mod testkit;

mod framework_packages;

pub use commands::{
    CliCommand, CliError, CliExitCode, CliPackageManager, CliPresentationBackend,
    CliProjectTemplate, CommandCatalog,
};
pub use dev_loop::{
    DevChangeBatch, DevChangeClassifier, DevErrorOverlay, DevLoop, DevLoopEvent, DevLoopReport,
    DevPatchKind, DevPatchPlan, DevReloadAcknowledgement, DevSurfaceReloader, DevWatchKind,
    DevWatchedPath, DevWatcherError, FileSystemWatcher, NotifyFileSystemWatcher,
};
pub use diagnostics::{CliDiagnostic, DiagnosticSeverity, SourceSpan};
pub use executor::{
    CommandExecution, WorkspaceCommandRunner, run_packaged_desktop_from_default_location,
    run_packaged_desktop_from_descriptor_path,
};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-cli";

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
        assert_eq!(crate_name(), "hawk2ui-cli");
    }
}
