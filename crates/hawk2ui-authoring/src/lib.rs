#![forbid(unsafe_code)]
//! Typed authoring records, component model, event binding, state records, and framework adapter contracts for `Hawk2UI`.

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-authoring";

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
        assert_eq!(crate_name(), "hawk2ui-authoring");
    }
}
