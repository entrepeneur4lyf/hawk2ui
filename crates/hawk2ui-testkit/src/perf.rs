//! Performance budget suite helpers.

use crate::BenchmarkExpectation;

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
}
