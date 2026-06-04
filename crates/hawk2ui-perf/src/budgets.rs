//! Performance budget records and validation.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// Unit used by a performance budget.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BudgetUnit {
    /// Millisecond duration budget.
    Milliseconds,
    /// Microsecond duration budget.
    Microseconds,
    /// Byte-size budget.
    Bytes,
    /// Count budget.
    Count,
}

/// Performance budget category.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PerformanceCategory {
    /// Startup and initialization budgets.
    Startup,
    /// Layout calculation budgets.
    Layout,
    /// Style parsing and style resolution budgets.
    Style,
    /// Rendering budgets.
    Rendering,
    /// Runtime scheduling and event budgets.
    Runtime,
    /// Script parsing, evaluation, and bridge budgets.
    Script,
    /// Asset decoding and cache budgets.
    Assets,
    /// Memory usage budgets.
    Memory,
    /// Package size budgets.
    Package,
    /// Desktop host and window event-loop budgets.
    Host,
    /// Realtime audio safety budgets.
    Realtime,
}

/// One performance budget row.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PerformanceBudget {
    /// Stable budget name.
    pub name: String,
    /// Budget category.
    pub category: PerformanceCategory,
    /// Measurement unit.
    pub unit: BudgetUnit,
    /// Target value for healthy operation.
    pub target: u64,
    /// Maximum allowed value before release gate failure.
    pub maximum: u64,
    /// Whether this budget blocks release readiness.
    pub release_gate: bool,
    /// Fixture used to measure this budget.
    pub fixture: String,
}

/// Collection of performance budgets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PerformanceBudgets {
    /// Budget rows.
    pub budgets: Vec<PerformanceBudget>,
}

#[derive(Debug, Deserialize)]
struct RawBudgets {
    budgets: Vec<PerformanceBudget>,
}

impl PerformanceBudgets {
    /// Parses and validates performance budgets from TOML.
    ///
    /// # Errors
    ///
    /// Returns [`PerformanceError`] when TOML parsing fails, when names are duplicated,
    /// or when a target value is greater than its maximum release value.
    pub fn parse(input: &str) -> Result<Self, PerformanceError> {
        let raw: RawBudgets =
            toml::from_str(input).map_err(|error| PerformanceError::Parse(error.to_string()))?;
        let budgets = Self {
            budgets: raw.budgets,
        };
        budgets.validate()?;
        Ok(budgets)
    }

    /// Returns true when a budget name exists.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.budgets.iter().any(|budget| budget.name == name)
    }

    /// Returns a budget by stable name.
    #[must_use]
    pub fn budget(&self, name: &str) -> Option<&PerformanceBudget> {
        self.budgets.iter().find(|budget| budget.name == name)
    }

    /// Iterates over release-gating budgets.
    pub fn release_gates(&self) -> impl Iterator<Item = &PerformanceBudget> {
        self.budgets.iter().filter(|budget| budget.release_gate)
    }

    fn validate(&self) -> Result<(), PerformanceError> {
        let mut names = BTreeSet::new();
        for budget in &self.budgets {
            if !names.insert(budget.name.clone()) {
                return Err(PerformanceError::DuplicateBudget(budget.name.clone()));
            }
            if budget.target > budget.maximum {
                return Err(PerformanceError::TargetExceedsMaximum(budget.name.clone()));
            }
        }
        Ok(())
    }
}

/// Performance budget validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PerformanceError {
    /// TOML parsing failed.
    Parse(String),
    /// Two or more budgets use the same stable name.
    DuplicateBudget(String),
    /// Budget target exceeds the maximum release value.
    TargetExceedsMaximum(String),
}
