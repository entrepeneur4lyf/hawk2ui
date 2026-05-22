#![forbid(unsafe_code)]
//! Product conformance checks that keep Hawk2UI requirements represented in code and examples.

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-conformance";

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
        assert_eq!(crate_name(), "hawk2ui-conformance");
    }
}
