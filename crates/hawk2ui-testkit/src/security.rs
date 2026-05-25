//! Security rejection matrix helpers.

use crate::SecurityRejection;

/// Error returned when a security rejection matrix is incomplete.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SecurityRejectionMatrixError {
    /// A required capability has no rejection case.
    MissingCapability(String),
}

/// Security rejection matrix for capability-boundary tests.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SecurityRejectionMatrix {
    cases: Vec<SecurityRejection>,
}

impl SecurityRejectionMatrix {
    /// Creates a security rejection matrix.
    #[must_use]
    pub fn new(cases: impl IntoIterator<Item = SecurityRejection>) -> Self {
        Self {
            cases: cases.into_iter().collect(),
        }
    }

    /// Returns all rejection cases.
    #[must_use]
    pub fn cases(&self) -> &[SecurityRejection] {
        &self.cases
    }

    /// Verifies every required capability has a rejection case.
    ///
    /// # Errors
    ///
    /// Returns [`SecurityRejectionMatrixError::MissingCapability`] when a
    /// required capability is not covered.
    pub fn require_capabilities(
        &self,
        capabilities: &[&str],
    ) -> Result<(), SecurityRejectionMatrixError> {
        for capability in capabilities {
            if !self
                .cases
                .iter()
                .any(|case| case.capability() == *capability)
            {
                return Err(SecurityRejectionMatrixError::MissingCapability(
                    (*capability).to_string(),
                ));
            }
        }
        Ok(())
    }
}
