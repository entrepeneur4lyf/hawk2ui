#![forbid(unsafe_code)]
//! Production script backend for `Hawk2UI` `JavaScript` and `TypeScript` execution.

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-script";

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
        assert_eq!(crate_name(), "hawk2ui-script");
    }
}
