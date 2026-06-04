//! Build-time source validation diagnostic evidence.
//!
//! Concrete validators in the build, style, script, and asset crates decide whether input is
//! accepted. This module records those decisions in a stable security vocabulary.

use crate::{SecurityDiagnostic, SecuritySeverity};

/// Build-time source validation rejection rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceValidationRule {
    /// Style syntax is unsupported.
    UnsupportedStyleSyntax,
    /// Script syntax is unsupported.
    UnsupportedScriptSyntax,
    /// Vector content is unsafe.
    UnsafeVectorContent,
    /// Declared asset is missing.
    MissingAsset,
    /// Capability is not declared.
    UndeclaredCapability,
    /// Manifest is malformed.
    MalformedManifest,
    /// Plugin metadata is invalid.
    InvalidPluginMetadata,
    /// Package target is invalid.
    InvalidPackageTarget,
}

impl SourceValidationRule {
    fn diagnostic(self) -> SecurityDiagnostic {
        let (rule, message) = match self {
            Self::UnsupportedStyleSyntax => ("style.unsupported", "style syntax is unsupported"),
            Self::UnsupportedScriptSyntax => ("script.unsupported", "script syntax is unsupported"),
            Self::UnsafeVectorContent => ("asset.vector.unsafe", "vector content is unsafe"),
            Self::MissingAsset => ("asset.missing", "declared asset is missing"),
            Self::UndeclaredCapability => ("capability.undeclared", "capability is not declared"),
            Self::MalformedManifest => ("manifest.malformed", "manifest is malformed"),
            Self::InvalidPluginMetadata => {
                ("plugin.metadata.invalid", "plugin metadata is invalid")
            }
            Self::InvalidPackageTarget => ("target.invalid", "package target is invalid"),
        };
        SecurityDiagnostic::new(SecuritySeverity::Error, rule, message)
    }
}

/// Source validation rejection record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceValidationRecord {
    /// Rejection rule.
    pub rule: SourceValidationRule,
    /// Source path associated with the rejection.
    pub path: String,
    /// Structured diagnostic.
    pub diagnostic: SecurityDiagnostic,
}

impl SourceValidationRecord {
    /// Returns a stable diagnostic label for the rejection.
    #[must_use]
    pub fn diagnostic_label(&self) -> String {
        format!("source.{}:{}", self.diagnostic.rule, self.path)
    }
}

/// Source validation evidence factory.
pub struct SourceValidationPolicy;

impl SourceValidationPolicy {
    /// Produces a rejection evidence record for a source validation rule.
    #[must_use]
    pub fn reject(rule: SourceValidationRule, path: impl Into<String>) -> SourceValidationRecord {
        SourceValidationRecord {
            rule,
            path: path.into(),
            diagnostic: rule.diagnostic(),
        }
    }
}
