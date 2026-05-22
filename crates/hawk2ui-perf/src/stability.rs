//! Runtime stability fixture records.

/// Deterministic runtime stability fixture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeStabilityFixture {
    /// Stable fixture name.
    pub name: String,
    /// Number of iterations the fixture represents.
    pub iterations: u64,
    /// Observed failure count.
    pub failures: u64,
    /// Maximum allowed failures.
    pub allowed_failures: u64,
}

impl RuntimeStabilityFixture {
    /// Creates a stability fixture with zero allowed and observed failures.
    #[must_use]
    pub fn new(name: impl Into<String>, iterations: u64) -> Self {
        Self {
            name: name.into(),
            iterations,
            failures: 0,
            allowed_failures: 0,
        }
    }

    /// Sets the observed failure count.
    #[must_use]
    pub fn with_failures(mut self, failures: u64) -> Self {
        self.failures = failures;
        self
    }

    /// Sets the allowed failure count.
    #[must_use]
    pub fn with_allowed_failures(mut self, allowed_failures: u64) -> Self {
        self.allowed_failures = allowed_failures;
        self
    }

    /// Validates the fixture against its stability limits.
    ///
    /// # Errors
    ///
    /// Returns [`StabilityError`] when observed failures exceed the allowed limit.
    pub fn validate(&self) -> Result<(), StabilityError> {
        if self.failures > self.allowed_failures {
            Err(StabilityError::FailureLimitExceeded {
                name: self.name.clone(),
                failures: self.failures,
                allowed: self.allowed_failures,
            })
        } else {
            Ok(())
        }
    }
}

/// Runtime stability validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StabilityError {
    /// Observed failures exceeded the fixture limit.
    FailureLimitExceeded {
        /// Fixture name.
        name: String,
        /// Observed failure count.
        failures: u64,
        /// Allowed failure count.
        allowed: u64,
    },
}
