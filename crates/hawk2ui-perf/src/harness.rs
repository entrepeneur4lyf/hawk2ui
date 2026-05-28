//! Deterministic benchmark suite records and validation.

use std::{
    fmt::Write as _,
    fs, io,
    path::{Path, PathBuf},
};

use crate::PerformanceBudgets;

/// Benchmark category used by a performance suite.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BenchmarkKind {
    /// Startup, manifest, artifact, and first-frame benchmarks.
    Startup,
    /// Style compilation and matching benchmarks.
    Style,
    /// Layout and text benchmarks.
    Layout,
    /// Rendering and scene export benchmarks.
    Rendering,
    /// Runtime scheduler and event benchmarks.
    Runtime,
    /// Script runtime and bridge benchmarks.
    Script,
    /// Asset decode and cache benchmarks.
    Assets,
    /// Package verification and sealing benchmarks.
    Package,
    /// Desktop host lifecycle and event-loop benchmarks.
    Host,
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

/// One evaluated benchmark report row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenchmarkReportEntry {
    /// Budget name this row evaluates.
    pub budget_name: String,
    /// Fixture used by the benchmark.
    pub fixture: String,
    /// Benchmark category.
    pub kind: BenchmarkKind,
    /// Observed measurement, when reported.
    pub observed: Option<u64>,
    /// Maximum accepted measurement, when the budget exists.
    pub maximum: Option<u64>,
    /// Whether the row passed its budget.
    pub accepted: bool,
    /// Stable failure reason for diagnostics and release evidence.
    pub failure: Option<String>,
}

/// Evaluated benchmark report for release evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenchmarkReport {
    /// Stable suite name.
    pub suite_name: String,
    /// Evaluated benchmark rows.
    pub entries: Vec<BenchmarkReportEntry>,
}

impl BenchmarkReport {
    fn new(suite_name: String, entries: Vec<BenchmarkReportEntry>) -> Self {
        Self {
            suite_name,
            entries,
        }
    }

    /// Returns the number of evaluated benchmark rows.
    #[must_use]
    pub fn case_count(&self) -> usize {
        self.entries.len()
    }

    /// Returns the number of failing benchmark rows.
    #[must_use]
    pub fn failed_count(&self) -> usize {
        self.entries.iter().filter(|entry| !entry.accepted).count()
    }

    /// Returns true when all benchmark rows passed.
    #[must_use]
    pub fn accepted(&self) -> bool {
        self.failed_count() == 0
    }

    /// Returns a deterministic text payload suitable for release evidence.
    #[must_use]
    pub fn artifact_payload(&self) -> String {
        let mut payload = format!(
            "suite = \"{}\"\ncase_count = {}\nfailed_count = {}\naccepted = {}\n",
            escape_toml_string(&self.suite_name),
            self.case_count(),
            self.failed_count(),
            self.accepted()
        );
        for entry in &self.entries {
            payload.push_str("\n[[benchmarks]]\n");
            let _ = write!(
                payload,
                "budget = \"{}\"\nfixture = \"{}\"\nkind = \"{:?}\"\nobserved = {}\nmaximum = {}\naccepted = {}\n",
                escape_toml_string(&entry.budget_name),
                escape_toml_string(&entry.fixture),
                entry.kind,
                optional_u64(entry.observed),
                optional_u64(entry.maximum),
                entry.accepted
            );
            if let Some(failure) = &entry.failure {
                let _ = writeln!(payload, "failure = \"{}\"", escape_toml_string(failure));
            }
        }
        payload
    }

    /// Writes this report as deterministic release evidence.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the report cannot be written.
    pub fn write_artifact(&self, path: impl AsRef<Path>) -> io::Result<()> {
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.artifact_payload())
    }
}

/// Files written by benchmark release evidence export.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenchmarkArtifactSet {
    root: PathBuf,
    files: Vec<PathBuf>,
}

impl BenchmarkArtifactSet {
    fn new(root: PathBuf, files: Vec<PathBuf>) -> Self {
        Self { root, files }
    }

    /// Returns the artifact root directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns written evidence files.
    #[must_use]
    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }
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

    /// Evaluates every benchmark case and returns a complete release evidence report.
    #[must_use]
    pub fn evaluate_against(&self, budgets: &PerformanceBudgets) -> BenchmarkReport {
        let mut entries = Vec::with_capacity(self.cases.len());
        for case in &self.cases {
            let Some(budget) = budgets
                .budgets
                .iter()
                .find(|budget| budget.name == case.budget_name)
            else {
                entries.push(BenchmarkReportEntry {
                    budget_name: case.budget_name.clone(),
                    fixture: case.fixture.clone(),
                    kind: case.kind,
                    observed: case.measurement.map(|measurement| measurement.observed),
                    maximum: None,
                    accepted: false,
                    failure: Some("missing-budget".to_string()),
                });
                continue;
            };
            let Some(measurement) = case.measurement else {
                entries.push(BenchmarkReportEntry {
                    budget_name: case.budget_name.clone(),
                    fixture: case.fixture.clone(),
                    kind: case.kind,
                    observed: None,
                    maximum: Some(budget.maximum),
                    accepted: false,
                    failure: Some("missing-measurement".to_string()),
                });
                continue;
            };
            let accepted = measurement.observed <= budget.maximum;
            entries.push(BenchmarkReportEntry {
                budget_name: case.budget_name.clone(),
                fixture: case.fixture.clone(),
                kind: case.kind,
                observed: Some(measurement.observed),
                maximum: Some(budget.maximum),
                accepted,
                failure: (!accepted).then(|| "budget-exceeded".to_string()),
            });
        }
        BenchmarkReport::new(self.name.clone(), entries)
    }

    /// Writes benchmark release evidence for this suite.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the evidence file cannot be written.
    pub fn write_report_artifact(
        &self,
        budgets: &PerformanceBudgets,
        directory: impl AsRef<Path>,
    ) -> io::Result<BenchmarkArtifactSet> {
        let root = directory.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;
        let report_path = root.join(format!("{}-benchmark-report.toml", self.name));
        self.evaluate_against(budgets)
            .write_artifact(&report_path)?;
        Ok(BenchmarkArtifactSet::new(root, vec![report_path]))
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

fn optional_u64(value: Option<u64>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

fn escape_toml_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch => escaped.push(ch),
        }
    }
    escaped
}
