//! Deterministic benchmark suite records and validation.

use crate::PerformanceBudgets;

/// Benchmark category used by a performance suite.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BenchmarkKind {
    /// Startup, manifest, artifact, and first-frame benchmarks.
    Startup,
    /// Layout and text benchmarks.
    Layout,
    /// Rendering and scene export benchmarks.
    Rendering,
    /// Runtime scheduler and event benchmarks.
    Runtime,
    /// Plugin realtime safety benchmarks.
    Realtime,
}

/// One deterministic benchmark case.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenchmarkCase {
    /// Budget name this case measures.
    pub budget_name: String,
    /// Fixture path used by the benchmark.
    pub fixture: String,
    /// Benchmark category.
    pub kind: BenchmarkKind,
}

impl BenchmarkCase {
    /// Creates a benchmark case tied to a named performance budget.
    #[must_use]
    pub fn new(
        budget_name: impl Into<String>,
        fixture: impl Into<String>,
        kind: BenchmarkKind,
    ) -> Self {
        Self {
            budget_name: budget_name.into(),
            fixture: fixture.into(),
            kind,
        }
    }
}

/// Collection of benchmark cases for one gate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenchmarkSuite {
    /// Stable suite name.
    pub name: String,
    /// Benchmark cases in deterministic execution order.
    pub cases: Vec<BenchmarkCase>,
}

impl BenchmarkSuite {
    /// Creates an empty benchmark suite.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            cases: Vec::new(),
        }
    }

    /// Appends one benchmark case.
    #[must_use]
    pub fn with_case(mut self, case: BenchmarkCase) -> Self {
        self.cases.push(case);
        self
    }

    /// Validates that every benchmark case maps to a configured budget.
    ///
    /// # Errors
    ///
    /// Returns [`BenchmarkError`] when a case has no matching budget.
    pub fn validate_against(&self, budgets: &PerformanceBudgets) -> Result<(), BenchmarkError> {
        for case in &self.cases {
            if !budgets.contains(&case.budget_name) {
                return Err(BenchmarkError::MissingBudget(case.budget_name.clone()));
            }
        }
        Ok(())
    }
}

/// Benchmark suite validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BenchmarkError {
    /// A benchmark case references a budget that does not exist.
    MissingBudget(String),
}
