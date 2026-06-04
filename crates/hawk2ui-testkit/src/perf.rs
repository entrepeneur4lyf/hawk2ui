//! Performance budget suite helpers.

use std::time::Duration;

use crate::BenchmarkExpectation;

/// Error returned when an observed benchmark duration violates a budget.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PerformanceBudgetError {
    /// The requested budget name is not registered.
    MissingExpectation(String),
    /// The registered expectation does not declare an enforceable budget.
    MissingBudget(String),
    /// The observed duration exceeds the registered budget.
    BudgetExceeded {
        /// Budget name.
        budget_name: String,
        /// Observed duration in milliseconds.
        elapsed_millis: u64,
        /// Maximum accepted duration in milliseconds.
        max_millis: u64,
    },
}

/// Successful performance budget evaluation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PerformanceBudgetReport {
    budget_name: String,
    fixture: String,
    elapsed_millis: u64,
    max_millis: u64,
}

impl PerformanceBudgetReport {
    fn new(expectation: &BenchmarkExpectation, elapsed_millis: u64, max_millis: u64) -> Self {
        Self {
            budget_name: expectation.budget_name().to_string(),
            fixture: expectation.fixture().to_string(),
            elapsed_millis,
            max_millis,
        }
    }

    /// Returns the evaluated budget name.
    #[must_use]
    pub fn budget_name(&self) -> &str {
        &self.budget_name
    }

    /// Returns the fixture associated with the budget.
    #[must_use]
    pub fn fixture(&self) -> &str {
        &self.fixture
    }

    /// Returns the observed duration in milliseconds.
    #[must_use]
    pub const fn elapsed_millis(&self) -> u64 {
        self.elapsed_millis
    }

    /// Returns the maximum accepted duration in milliseconds.
    #[must_use]
    pub const fn max_millis(&self) -> u64 {
        self.max_millis
    }
}

/// Performance expectation suite.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PerformanceSuite {
    expectations: Vec<BenchmarkExpectation>,
}

impl PerformanceSuite {
    /// Creates a performance suite.
    #[must_use]
    pub fn new(expectations: impl IntoIterator<Item = BenchmarkExpectation>) -> Self {
        Self {
            expectations: expectations.into_iter().collect(),
        }
    }

    /// Returns all benchmark expectations.
    #[must_use]
    pub fn expectations(&self) -> &[BenchmarkExpectation] {
        &self.expectations
    }

    /// Returns the expectation for a budget name.
    #[must_use]
    pub fn expectation(&self, budget_name: &str) -> Option<&BenchmarkExpectation> {
        self.expectations
            .iter()
            .find(|expectation| expectation.budget_name() == budget_name)
    }

    /// Verifies an observed duration against a registered benchmark budget.
    ///
    /// # Errors
    ///
    /// Returns [`PerformanceBudgetError`] when the budget is missing, not
    /// enforceable, or exceeded.
    pub fn assert_within_budget(
        &self,
        budget_name: &str,
        elapsed: Duration,
    ) -> Result<PerformanceBudgetReport, PerformanceBudgetError> {
        let expectation = self
            .expectation(budget_name)
            .ok_or_else(|| PerformanceBudgetError::MissingExpectation(budget_name.to_string()))?;
        let max_millis = expectation
            .max_millis()
            .ok_or_else(|| PerformanceBudgetError::MissingBudget(budget_name.to_string()))?;
        let elapsed_millis = duration_millis(elapsed);
        if elapsed_millis > max_millis {
            return Err(PerformanceBudgetError::BudgetExceeded {
                budget_name: budget_name.to_string(),
                elapsed_millis,
                max_millis,
            });
        }
        Ok(PerformanceBudgetReport::new(
            expectation,
            elapsed_millis,
            max_millis,
        ))
    }
}

fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
