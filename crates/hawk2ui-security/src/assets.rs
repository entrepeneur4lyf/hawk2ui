//! Asset security diagnostic evidence records.
//!
//! These records describe asset decisions made by the concrete asset/build validators. This module
//! does not decode assets, compare hashes, or enforce limits by itself.

use crate::{SecurityDiagnostic, SecuritySeverity};

/// Image metadata stripping status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetImageMetadataStatus {
    /// No metadata was present.
    NotPresent,
    /// Metadata was stripped before packaging.
    Stripped,
}

/// Vector asset safety status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VectorSafetyStatus {
    /// Vector content passed safety validation.
    Safe,
    /// Vector content failed safety validation.
    Unsafe,
}

/// Asset hash verification status.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AssetHashVerification {
    /// Actual hash matched the declared hash.
    Verified {
        /// Verified hash.
        hash: String,
    },
}

impl AssetHashVerification {
    /// Creates a verified hash status.
    #[must_use]
    pub fn verified(hash: impl Into<String>) -> Self {
        Self::Verified { hash: hash.into() }
    }
}

/// Accepted asset security record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetSecurityRecord {
    /// Asset path.
    pub path: String,
    /// Optional image metadata status.
    pub metadata_status: Option<AssetImageMetadataStatus>,
    /// Optional vector safety status.
    pub vector_status: Option<VectorSafetyStatus>,
    /// Hash verification status.
    pub hash: AssetHashVerification,
}

/// Asset security rejection rule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AssetSecurityRule {
    /// Asset exceeds declared size limit.
    Oversized {
        /// Actual byte length.
        actual_bytes: u64,
        /// Maximum allowed byte length.
        max_bytes: u64,
    },
    /// Asset format is unsupported.
    UnsupportedFormat {
        /// Unsupported format label.
        format: String,
    },
    /// Vector content failed safety validation.
    UnsafeVector,
    /// Asset hash does not match declared hash.
    HashMismatch {
        /// Expected hash.
        expected: String,
        /// Actual hash.
        actual: String,
    },
}

impl AssetSecurityRule {
    fn diagnostic(&self) -> SecurityDiagnostic {
        let (rule, message) = match self {
            Self::Oversized { .. } => ("asset.size.exceeded", "asset exceeds declared size limit"),
            Self::UnsupportedFormat { .. } => {
                ("asset.format.unsupported", "asset format is unsupported")
            }
            Self::UnsafeVector => (
                "asset.vector.unsafe",
                "vector asset failed safety validation",
            ),
            Self::HashMismatch { .. } => (
                "asset.hash.mismatch",
                "asset hash does not match declared hash",
            ),
        };
        SecurityDiagnostic::new(SecuritySeverity::Error, rule, message)
    }
}

/// Asset security rejection record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetSecurityRejection {
    /// Rejection rule.
    pub rule: AssetSecurityRule,
    /// Asset path.
    pub path: String,
    /// Structured diagnostic.
    pub diagnostic: SecurityDiagnostic,
}

impl AssetSecurityRejection {
    /// Returns a stable diagnostic label for the asset rejection.
    #[must_use]
    pub fn diagnostic_label(&self) -> String {
        format!("asset.{}:{}", self.diagnostic.rule, self.path)
    }
}

/// Asset security evidence factory.
pub struct AssetSecurityPolicy;

impl AssetSecurityPolicy {
    /// Creates an accepted image asset security record.
    #[must_use]
    pub fn image_record(
        path: impl Into<String>,
        metadata_status: AssetImageMetadataStatus,
        hash: AssetHashVerification,
    ) -> AssetSecurityRecord {
        AssetSecurityRecord {
            path: path.into(),
            metadata_status: Some(metadata_status),
            vector_status: None,
            hash,
        }
    }

    /// Creates an accepted vector asset security record.
    #[must_use]
    pub fn vector_record(
        path: impl Into<String>,
        vector_status: VectorSafetyStatus,
        hash: AssetHashVerification,
    ) -> AssetSecurityRecord {
        AssetSecurityRecord {
            path: path.into(),
            metadata_status: None,
            vector_status: Some(vector_status),
            hash,
        }
    }

    /// Produces an asset security rejection.
    #[must_use]
    pub fn reject(rule: AssetSecurityRule, path: impl Into<String>) -> AssetSecurityRejection {
        let diagnostic = rule.diagnostic();
        AssetSecurityRejection {
            rule,
            path: path.into(),
            diagnostic,
        }
    }
}
