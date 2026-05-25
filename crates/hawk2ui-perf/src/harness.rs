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
    /// Optional deterministic measurement supplied by a benchmark gate.
    pub measurement: Option<BenchmarkMeasurement>,
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
            measurement: None,
        }
    }

    /// Attaches an observed measurement to this case.
    #[must_use]
    pub const fn with_measurement(mut self, measurement: BenchmarkMeasurement) -> Self {
        self.measurement = Some(measurement);
        self
    }
}

/// Deterministic benchmark measurement in the same unit as the referenced budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BenchmarkMeasurement {
    /// Observed value.
    pub observed: u64,
}

impl BenchmarkMeasurement {
    /// Creates a benchmark measurement.
    #[must_use]
    pub const fn new(observed: u64) -> Self {
        Self { observed }
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
            let Some(budget) = budgets
                .budgets
                .iter()
                .find(|budget| budget.name == case.budget_name)
            else {
                return Err(BenchmarkError::MissingBudget(case.budget_name.clone()));
            };
            let Some(measurement) = case.measurement else {
                return Err(BenchmarkError::MissingMeasurement(case.budget_name.clone()));
            };
            if measurement.observed > budget.maximum {
                return Err(BenchmarkError::BudgetExceeded {
                    budget_name: case.budget_name.clone(),
                    observed: measurement.observed,
                    maximum: budget.maximum,
                });
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
    /// A benchmark case did not report a measurement.
    MissingMeasurement(String),
    /// A benchmark measurement exceeded the configured release maximum.
    BudgetExceeded {
        /// Budget name.
        budget_name: String,
        /// Observed value.
        observed: u64,
        /// Maximum allowed value.
        maximum: u64,
    },
}
