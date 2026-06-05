//! Benchmark suite records, observed measurements, and release-gate validation.

use std::{
    collections::BTreeSet,
    fs, io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use serde::Serialize;

use crate::{PerformanceBudgets, PerformanceCategory};

/// Evidence quality for a benchmark measurement.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MeasurementQuality {
    /// Deterministic measurement suitable for release gates.
    Deterministic,
    /// Wall-clock measurement retained for trend visibility only.
    AdvisoryWallClock,
}

/// Benchmark category used by a performance suite.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
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
    /// Memory working-set benchmarks.
    Memory,
    /// Package verification, size, and sealing benchmarks.
    Package,
    /// Desktop host lifecycle and event-loop benchmarks.
    Host,
    /// Plugin realtime safety benchmarks.
    Realtime,
}

impl BenchmarkKind {
    /// Returns the budget category that this benchmark kind is allowed to satisfy.
    #[must_use]
    pub const fn performance_category(self) -> PerformanceCategory {
        match self {
            Self::Startup => PerformanceCategory::Startup,
            Self::Style => PerformanceCategory::Style,
            Self::Layout => PerformanceCategory::Layout,
            Self::Rendering => PerformanceCategory::Rendering,
            Self::Runtime => PerformanceCategory::Runtime,
            Self::Script => PerformanceCategory::Script,
            Self::Assets => PerformanceCategory::Assets,
            Self::Memory => PerformanceCategory::Memory,
            Self::Package => PerformanceCategory::Package,
            Self::Host => PerformanceCategory::Host,
            Self::Realtime => PerformanceCategory::Realtime,
        }
    }
}

/// Runtime configuration for benchmark binaries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BenchmarkRunConfig {
    quick: bool,
    iterations: u64,
}

impl BenchmarkRunConfig {
    /// Creates a benchmark run configuration.
    #[must_use]
    pub const fn new(quick: bool, iterations: u64) -> Self {
        Self {
            quick,
            iterations: if iterations == 0 { 1 } else { iterations },
        }
    }

    /// Parses benchmark CLI arguments.
    ///
    /// Recognized arguments are `--quick` and `--iterations=N`. Unknown
    /// arguments are ignored so `cargo bench` harness flags remain harmless.
    #[must_use]
    pub fn from_args<I, S>(args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut quick = false;
        let mut explicit_iterations = None;
        for arg in args {
            let arg = arg.as_ref();
            if arg == "--quick" {
                quick = true;
            } else if let Some(value) = arg.strip_prefix("--iterations=") {
                explicit_iterations = value.parse::<u64>().ok().filter(|value| *value > 0);
            }
        }
        let iterations = explicit_iterations.unwrap_or(if quick { 1 } else { 64 });
        Self::new(quick, iterations)
    }

    /// Parses arguments from the current process environment.
    #[must_use]
    pub fn from_env_args() -> Self {
        Self::from_args(std::env::args().skip(1))
    }

    /// Returns whether this is a quick benchmark run.
    #[must_use]
    pub const fn quick(&self) -> bool {
        self.quick
    }

    /// Returns the number of repeated operations benchmarks should execute.
    #[must_use]
    pub const fn iterations(&self) -> u64 {
        self.iterations
    }
}

/// One benchmark case.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenchmarkCase {
    /// Budget name this case measures.
    pub budget_name: String,
    /// Fixture path used by the benchmark.
    pub fixture: String,
    /// Benchmark category.
    pub kind: BenchmarkKind,
    /// Optional observed measurement supplied by a benchmark gate.
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

/// Observed benchmark measurement in the same unit as the referenced budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BenchmarkMeasurement {
    /// Observed value.
    pub observed: u64,
    /// Evidence quality of the measurement.
    pub quality: MeasurementQuality,
}

impl BenchmarkMeasurement {
    /// Creates a benchmark measurement from an already-observed value.
    #[must_use]
    pub const fn new(observed: u64) -> Self {
        Self {
            observed,
            quality: MeasurementQuality::Deterministic,
        }
    }

    /// Creates a benchmark measurement with explicit evidence quality.
    #[must_use]
    pub const fn with_quality(observed: u64, quality: MeasurementQuality) -> Self {
        Self { observed, quality }
    }

    /// Returns whether this measurement can satisfy a release gate.
    #[must_use]
    pub const fn release_gate_eligible(self) -> bool {
        matches!(self.quality, MeasurementQuality::Deterministic)
    }

    /// Creates a millisecond duration measurement from a [`Duration`].
    #[must_use]
    pub fn from_duration_millis(duration: Duration) -> Self {
        Self::with_quality(
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
            MeasurementQuality::AdvisoryWallClock,
        )
    }

    /// Creates a microsecond duration measurement from a [`Duration`].
    #[must_use]
    pub fn from_duration_micros(duration: Duration) -> Self {
        Self::with_quality(
            u64::try_from(duration.as_micros()).unwrap_or(u64::MAX),
            MeasurementQuality::AdvisoryWallClock,
        )
    }

    /// Measures a closure and records elapsed milliseconds.
    #[must_use]
    pub fn measure_millis(operation: impl FnOnce()) -> Self {
        let start = Instant::now();
        operation();
        Self::from_duration_millis(start.elapsed())
    }

    /// Measures a closure and records elapsed microseconds.
    #[must_use]
    pub fn measure_micros(operation: impl FnOnce()) -> Self {
        let start = Instant::now();
        operation();
        Self::from_duration_micros(start.elapsed())
    }

    /// Creates a byte-size measurement.
    #[must_use]
    pub const fn from_bytes(bytes: u64) -> Self {
        Self::new(bytes)
    }

    /// Creates a count measurement.
    #[must_use]
    pub const fn from_count(count: u64) -> Self {
        Self::new(count)
    }
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
    /// Evidence quality, when a measurement was reported.
    pub quality: Option<MeasurementQuality>,
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

    /// Returns a deterministic TOML payload suitable for release evidence.
    #[must_use]
    pub fn artifact_payload(&self) -> String {
        let artifact = BenchmarkReportArtifact {
            suite: &self.suite_name,
            case_count: self.case_count(),
            failed_count: self.failed_count(),
            accepted: self.accepted(),
            benchmarks: self
                .entries
                .iter()
                .map(|entry| BenchmarkReportArtifactEntry {
                    budget: &entry.budget_name,
                    fixture: &entry.fixture,
                    kind: entry.kind,
                    observed: entry.observed,
                    quality: entry.quality,
                    maximum: entry.maximum,
                    accepted: entry.accepted,
                    failure: entry.failure.as_deref(),
                })
                .collect(),
        };
        toml::to_string(&artifact).unwrap_or_else(|error| {
            format!(
                "suite = \"benchmark-report-serialization-failed\"\ncase_count = 0\nfailed_count = 1\naccepted = false\nerror = {:?}\n",
                error.to_string()
            )
        })
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

#[derive(Serialize)]
struct BenchmarkReportArtifact<'entry> {
    suite: &'entry str,
    case_count: usize,
    failed_count: usize,
    accepted: bool,
    benchmarks: Vec<BenchmarkReportArtifactEntry<'entry>>,
}

#[derive(Serialize)]
struct BenchmarkReportArtifactEntry<'entry> {
    budget: &'entry str,
    fixture: &'entry str,
    kind: BenchmarkKind,
    observed: Option<u64>,
    quality: Option<MeasurementQuality>,
    maximum: Option<u64>,
    accepted: bool,
    failure: Option<&'entry str>,
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

    /// Validates every benchmark case against the configured budgets.
    ///
    /// # Errors
    ///
    /// Returns [`BenchmarkError`] when a case has no matching budget, uses the
    /// wrong fixture or category, lacks a measurement, or exceeds its budget.
    pub fn validate_against(&self, budgets: &PerformanceBudgets) -> Result<(), BenchmarkError> {
        for case in &self.cases {
            validate_case(case, budgets)?;
        }
        Ok(())
    }

    /// Verifies every release-gating budget is covered by at least one suite case.
    ///
    /// # Errors
    ///
    /// Returns [`BenchmarkError::MissingReleaseGateCase`] for the first
    /// release-gating budget with no corresponding benchmark case.
    pub fn validate_release_gate_coverage<'suite>(
        budgets: &PerformanceBudgets,
        suites: impl IntoIterator<Item = &'suite BenchmarkSuite>,
    ) -> Result<(), BenchmarkError> {
        let mut covered = BTreeSet::new();
        for suite in suites {
            for case in &suite.cases {
                covered.insert(case.budget_name.as_str());
            }
        }
        for budget in budgets.release_gates() {
            if !covered.contains(budget.name.as_str()) {
                return Err(BenchmarkError::MissingReleaseGateCase(budget.name.clone()));
            }
        }
        Ok(())
    }

    /// Evaluates every benchmark case and returns a complete release evidence report.
    #[must_use]
    pub fn evaluate_against(&self, budgets: &PerformanceBudgets) -> BenchmarkReport {
        BenchmarkReport::new(
            self.name.clone(),
            self.cases
                .iter()
                .map(|case| report_entry(case, budgets))
                .collect(),
        )
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
    /// A release-gating budget has no benchmark case.
    MissingReleaseGateCase(String),
    /// A benchmark case uses a fixture different from its budget fixture.
    FixtureMismatch {
        /// Budget name.
        budget_name: String,
        /// Fixture configured by the budget.
        expected: String,
        /// Fixture declared by the benchmark case.
        actual: String,
    },
    /// A benchmark case kind does not match the budget category.
    CategoryMismatch {
        /// Budget name.
        budget_name: String,
        /// Category configured by the budget.
        expected: PerformanceCategory,
        /// Benchmark kind declared by the case.
        actual: BenchmarkKind,
    },
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
    /// Advisory wall-clock evidence was used for a release-gating budget.
    AdvisoryMeasurementUsedForReleaseGate(String),
}

fn validate_case(case: &BenchmarkCase, budgets: &PerformanceBudgets) -> Result<(), BenchmarkError> {
    let budget = budgets
        .budget(&case.budget_name)
        .ok_or_else(|| BenchmarkError::MissingBudget(case.budget_name.clone()))?;
    if case.fixture != budget.fixture {
        return Err(BenchmarkError::FixtureMismatch {
            budget_name: case.budget_name.clone(),
            expected: budget.fixture.clone(),
            actual: case.fixture.clone(),
        });
    }
    if case.kind.performance_category() != budget.category {
        return Err(BenchmarkError::CategoryMismatch {
            budget_name: case.budget_name.clone(),
            expected: budget.category,
            actual: case.kind,
        });
    }
    let Some(measurement) = case.measurement else {
        return Err(BenchmarkError::MissingMeasurement(case.budget_name.clone()));
    };
    if budget.release_gate && !measurement.release_gate_eligible() {
        return Err(BenchmarkError::AdvisoryMeasurementUsedForReleaseGate(
            case.budget_name.clone(),
        ));
    }
    if matches!(measurement.quality, MeasurementQuality::AdvisoryWallClock) {
        return Ok(());
    }
    if measurement.observed > budget.maximum {
        return Err(BenchmarkError::BudgetExceeded {
            budget_name: case.budget_name.clone(),
            observed: measurement.observed,
            maximum: budget.maximum,
        });
    }
    Ok(())
}

fn report_entry(case: &BenchmarkCase, budgets: &PerformanceBudgets) -> BenchmarkReportEntry {
    let Some(budget) = budgets.budget(&case.budget_name) else {
        return BenchmarkReportEntry {
            budget_name: case.budget_name.clone(),
            fixture: case.fixture.clone(),
            kind: case.kind,
            observed: case.measurement.map(|measurement| measurement.observed),
            quality: case.measurement.map(|measurement| measurement.quality),
            maximum: None,
            accepted: false,
            failure: Some("missing-budget".to_string()),
        };
    };
    if case.fixture != budget.fixture {
        return BenchmarkReportEntry {
            budget_name: case.budget_name.clone(),
            fixture: case.fixture.clone(),
            kind: case.kind,
            observed: case.measurement.map(|measurement| measurement.observed),
            quality: case.measurement.map(|measurement| measurement.quality),
            maximum: Some(budget.maximum),
            accepted: false,
            failure: Some("fixture-mismatch".to_string()),
        };
    }
    if case.kind.performance_category() != budget.category {
        return BenchmarkReportEntry {
            budget_name: case.budget_name.clone(),
            fixture: case.fixture.clone(),
            kind: case.kind,
            observed: case.measurement.map(|measurement| measurement.observed),
            quality: case.measurement.map(|measurement| measurement.quality),
            maximum: Some(budget.maximum),
            accepted: false,
            failure: Some("category-mismatch".to_string()),
        };
    }
    let Some(measurement) = case.measurement else {
        return BenchmarkReportEntry {
            budget_name: case.budget_name.clone(),
            fixture: case.fixture.clone(),
            kind: case.kind,
            observed: None,
            quality: None,
            maximum: Some(budget.maximum),
            accepted: false,
            failure: Some("missing-measurement".to_string()),
        };
    };
    if budget.release_gate && !measurement.release_gate_eligible() {
        return BenchmarkReportEntry {
            budget_name: case.budget_name.clone(),
            fixture: case.fixture.clone(),
            kind: case.kind,
            observed: Some(measurement.observed),
            quality: Some(measurement.quality),
            maximum: Some(budget.maximum),
            accepted: false,
            failure: Some("advisory-measurement".to_string()),
        };
    }
    let accepted = measurement.quality == MeasurementQuality::AdvisoryWallClock
        || measurement.observed <= budget.maximum;
    BenchmarkReportEntry {
        budget_name: case.budget_name.clone(),
        fixture: case.fixture.clone(),
        kind: case.kind,
        observed: Some(measurement.observed),
        quality: Some(measurement.quality),
        maximum: Some(budget.maximum),
        accepted,
        failure: (!accepted).then(|| "budget-exceeded".to_string()),
    }
}
