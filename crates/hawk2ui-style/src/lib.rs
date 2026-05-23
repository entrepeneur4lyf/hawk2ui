#![forbid(unsafe_code)]
//! Typed style property registry, selector subset, design tokens, compiler, and runtime style tables for `Hawk2UI`.

pub mod property;
pub mod selector;
pub mod token;

pub use property::{
    PropertyGroup, PropertyId, PropertyMetadata, PropertyRegistry, PropertyRequirement, StyleValue,
    UnitHandling, ValidationError, ValueType,
};
pub use selector::{Selector, SelectorDiagnostic, SelectorParseError, SelectorPart};
pub use token::{
    ThemeVariant, TokenDiagnostic, TokenError, TokenKind, TokenRecord, TokenSet, TokenValue,
};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-style";

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
        assert_eq!(crate_name(), "hawk2ui-style");
    }
}
