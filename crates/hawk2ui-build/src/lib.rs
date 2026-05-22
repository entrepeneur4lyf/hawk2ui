#![forbid(unsafe_code)]
//! Manifest validation, build pipeline orchestration, sealed artifacts, package metadata, and verification reports for Hawk2UI.

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-build";

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
        assert_eq!(crate_name(), "hawk2ui-build");
    }
}
