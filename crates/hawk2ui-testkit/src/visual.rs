//! Visual regression suite helpers.

use crate::VisualSnapshot;

/// Visual regression case with a baseline and candidate snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisualRegressionCase {
    name: String,
    baseline: VisualSnapshot,
    candidate: VisualSnapshot,
}

impl VisualRegressionCase {
    /// Creates a visual regression case.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        baseline: VisualSnapshot,
        candidate: VisualSnapshot,
    ) -> Self {
        Self {
            name: name.into(),
            baseline,
            candidate,
        }
    }

    /// Returns the case name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns true when baseline and candidate metadata match.
    #[must_use]
    pub fn matches_baseline(&self) -> bool {
        self.baseline == self.candidate
    }
}

/// Collection of visual regression cases.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VisualRegressionSuite {
    cases: Vec<VisualRegressionCase>,
}

impl VisualRegressionSuite {
    /// Creates an empty visual regression suite.
    #[must_use]
    pub const fn new() -> Self {
        Self { cases: Vec::new() }
    }

    /// Adds a visual regression case.
    #[must_use]
    pub fn with_case(mut self, case: VisualRegressionCase) -> Self {
        self.cases.push(case);
        self
    }

    /// Returns all visual regression cases.
    #[must_use]
    pub fn cases(&self) -> &[VisualRegressionCase] {
        &self.cases
    }

    /// Returns whether every case matches its baseline.
    #[must_use]
    pub fn all_match(&self) -> bool {
        self.cases
            .iter()
            .all(VisualRegressionCase::matches_baseline)
    }
}
