#![forbid(unsafe_code)]
//! Reusable command definitions and orchestration for the `Hawk2UI` CLI.

pub mod commands;
pub mod diagnostics;

pub use commands::{CliCommand, CliError, CliExitCode, CommandCatalog};
pub use diagnostics::{CliDiagnostic, DiagnosticSeverity, SourceSpan};

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
