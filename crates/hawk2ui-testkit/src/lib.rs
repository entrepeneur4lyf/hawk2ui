#![forbid(unsafe_code)]
//! Shared deterministic fixtures, diagnostics assertions, visual helpers, security helpers, and benchmark helpers for Hawk2UI tests.

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-testkit";

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
        assert_eq!(crate_name(), "hawk2ui-testkit");
    }
}
