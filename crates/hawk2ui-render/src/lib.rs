#![forbid(unsafe_code)]
//! Retained scene records, paint layers, renderer backend boundary, text contracts, and custom draw surfaces for Hawk2UI.

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-render";

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
        assert_eq!(crate_name(), "hawk2ui-render");
    }
}
