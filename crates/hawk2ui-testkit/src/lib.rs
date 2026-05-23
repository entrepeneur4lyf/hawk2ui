#![forbid(unsafe_code)]
//! Shared deterministic fixtures, diagnostics assertions, visual helpers, security helpers, and benchmark helpers for `Hawk2UI` tests.

/// Fixture type used by deterministic `Hawk2UI` tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixtureKind {
    /// Manifest fixture.
    Manifest,
    /// Source fixture.
    Source,
    /// Asset fixture.
    Asset,
    /// Security fixture.
    Security,
    /// Visual fixture.
    Visual,
    /// Performance fixture.
    Performance,
}

/// Named test fixture record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestFixture {
    name: String,
    path: String,
    kind: FixtureKind,
}

impl TestFixture {
    /// Creates a named fixture record.
    #[must_use]
    pub fn new(name: impl Into<String>, path: impl Into<String>, kind: FixtureKind) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            kind,
        }
    }

    /// Returns the stable fixture name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the fixture path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the fixture kind.
    #[must_use]
    pub const fn kind(&self) -> FixtureKind {
        self.kind
    }
}

/// Fixture registry error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FixtureError {
    /// Requested fixture was not registered.
    MissingFixture(String),
}

/// Deterministic fixture registry for tests and conformance suites.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixtureRegistry {
    fixtures: Vec<TestFixture>,
}

impl FixtureRegistry {
    /// Creates a fixture registry from fixture records.
    #[must_use]
    pub fn new(fixtures: impl IntoIterator<Item = TestFixture>) -> Self {
        Self {
            fixtures: fixtures.into_iter().collect(),
        }
    }

    /// Returns a required fixture by name.
    ///
    /// # Errors
    ///
    /// Returns [`FixtureError::MissingFixture`] when no registered fixture has the requested name.
    pub fn require(&self, name: &str) -> Result<&TestFixture, FixtureError> {
        self.fixtures
            .iter()
            .find(|fixture| fixture.name == name)
            .ok_or_else(|| FixtureError::MissingFixture(name.to_string()))
    }
}

/// Recorded command invocation and exit status.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandRecord {
    program: String,
    args: Vec<String>,
    status: i32,
}

impl CommandRecord {
    /// Creates a command record.
    #[must_use]
    pub fn new(
        program: impl Into<String>,
        args: impl IntoIterator<Item = impl Into<String>>,
        status: i32,
    ) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
            status,
        }
    }

    /// Returns the command program.
    #[must_use]
    pub fn program(&self) -> &str {
        &self.program
    }

    /// Returns the command arguments.
    #[must_use]
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// Returns true when the recorded process status is successful.
    #[must_use]
    pub const fn succeeded(&self) -> bool {
        self.status == 0
    }
}

/// Test diagnostic severity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticSeverity {
    /// Release-blocking error.
    Error,
    /// Non-blocking warning.
    Warning,
}

/// Diagnostic record used by cross-crate tests without coupling to a concrete producer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticRecord {
    severity: DiagnosticSeverity,
    rule: String,
    message: String,
}

impl DiagnosticRecord {
    /// Creates an error diagnostic record.
    #[must_use]
    pub fn error(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(DiagnosticSeverity::Error, rule, message)
    }

    /// Creates a diagnostic record.
    #[must_use]
    pub fn new(
        severity: DiagnosticSeverity,
        rule: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            rule: rule.into(),
            message: message.into(),
        }
    }

    /// Returns the diagnostic severity.
    #[must_use]
    pub const fn severity(&self) -> DiagnosticSeverity {
        self.severity
    }

    /// Returns the stable diagnostic rule.
    #[must_use]
    pub fn rule(&self) -> &str {
        &self.rule
    }

    /// Returns the diagnostic message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Diagnostic assertion error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiagnosticAssertionError {
    /// Diagnostic did not match the expected rule or severity.
    Mismatch {
        /// Expected severity.
        expected_severity: DiagnosticSeverity,
        /// Expected rule.
        expected_rule: String,
        /// Actual severity.
        actual_severity: DiagnosticSeverity,
        /// Actual rule.
        actual_rule: String,
    },
}

/// Reusable diagnostic assertion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticAssertion {
    severity: DiagnosticSeverity,
    rule: String,
}

impl DiagnosticAssertion {
    /// Creates an assertion for an error diagnostic.
    #[must_use]
    pub fn error(rule: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            rule: rule.into(),
        }
    }

    /// Checks a diagnostic record against this assertion.
    ///
    /// # Errors
    ///
    /// Returns [`DiagnosticAssertionError::Mismatch`] when the rule or severity differs.
    pub fn assert_matches(
        &self,
        diagnostic: &DiagnosticRecord,
    ) -> Result<(), DiagnosticAssertionError> {
        if diagnostic.severity == self.severity && diagnostic.rule == self.rule {
            Ok(())
        } else {
            Err(DiagnosticAssertionError::Mismatch {
                expected_severity: self.severity,
                expected_rule: self.rule.clone(),
                actual_severity: diagnostic.severity,
                actual_rule: diagnostic.rule.clone(),
            })
        }
    }
}

/// Sealed artifact metadata used by tests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactRecord {
    name: String,
    hash: String,
}

impl ArtifactRecord {
    /// Creates an artifact metadata record.
    #[must_use]
    pub fn new(name: impl Into<String>, hash: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            hash: hash.into(),
        }
    }

    /// Returns the artifact name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the artifact hash.
    #[must_use]
    pub fn hash(&self) -> &str {
        &self.hash
    }
}

/// Artifact assertion error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArtifactAssertionError {
    /// Artifact hash did not match the expected hash.
    HashMismatch {
        /// Expected artifact hash.
        expected: String,
        /// Actual artifact hash.
        actual: String,
    },
}

/// Reusable artifact assertion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactAssertion {
    expected_hash: String,
}

impl ArtifactAssertion {
    /// Creates an artifact hash assertion.
    #[must_use]
    pub fn hash(expected_hash: impl Into<String>) -> Self {
        Self {
            expected_hash: expected_hash.into(),
        }
    }

    /// Checks an artifact record against this assertion.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactAssertionError::HashMismatch`] when the artifact hash differs.
    pub fn assert_matches(&self, artifact: &ArtifactRecord) -> Result<(), ArtifactAssertionError> {
        if artifact.hash == self.expected_hash {
            Ok(())
        } else {
            Err(ArtifactAssertionError::HashMismatch {
                expected: self.expected_hash.clone(),
                actual: artifact.hash.clone(),
            })
        }
    }
}

/// Visual snapshot metadata used by rendering tests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisualSnapshot {
    name: String,
    width: u32,
    height: u32,
    commands: Vec<String>,
}

impl VisualSnapshot {
    /// Creates visual snapshot metadata.
    #[must_use]
    pub fn new(name: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            name: name.into(),
            width,
            height,
            commands: Vec::new(),
        }
    }

    /// Adds a recorded drawing command.
    #[must_use]
    pub fn with_command(mut self, command: impl Into<String>) -> Self {
        self.commands.push(command.into());
        self
    }

    /// Returns the snapshot name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the snapshot dimensions.
    #[must_use]
    pub const fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Returns the recorded drawing commands.
    #[must_use]
    pub fn commands(&self) -> &[String] {
        &self.commands
    }
}

/// Security rejection case used by authority and sandbox tests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityRejection {
    capability: String,
    fixture: String,
    diagnostic_rule: String,
}

impl SecurityRejection {
    /// Creates a security rejection case.
    #[must_use]
    pub fn new(
        capability: impl Into<String>,
        fixture: impl Into<String>,
        diagnostic_rule: impl Into<String>,
    ) -> Self {
        Self {
            capability: capability.into(),
            fixture: fixture.into(),
            diagnostic_rule: diagnostic_rule.into(),
        }
    }

    /// Returns the rejected capability.
    #[must_use]
    pub fn capability(&self) -> &str {
        &self.capability
    }

    /// Returns the rejection fixture path.
    #[must_use]
    pub fn fixture(&self) -> &str {
        &self.fixture
    }

    /// Returns the expected diagnostic rule.
    #[must_use]
    pub fn diagnostic_rule(&self) -> &str {
        &self.diagnostic_rule
    }
}

/// Performance benchmark expectation used by deterministic performance tests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenchmarkExpectation {
    budget_name: String,
    fixture: String,
    max_millis: Option<u64>,
}

impl BenchmarkExpectation {
    /// Creates a benchmark expectation.
    #[must_use]
    pub fn new(budget_name: impl Into<String>, fixture: impl Into<String>) -> Self {
        Self {
            budget_name: budget_name.into(),
            fixture: fixture.into(),
            max_millis: None,
        }
    }

    /// Adds an upper bound in milliseconds.
    #[must_use]
    pub const fn with_max_millis(mut self, max_millis: u64) -> Self {
        self.max_millis = Some(max_millis);
        self
    }

    /// Returns the performance budget name.
    #[must_use]
    pub fn budget_name(&self) -> &str {
        &self.budget_name
    }

    /// Returns the benchmark fixture path.
    #[must_use]
    pub fn fixture(&self) -> &str {
        &self.fixture
    }

    /// Returns the optional maximum runtime in milliseconds.
    #[must_use]
    pub const fn max_millis(&self) -> Option<u64> {
        self.max_millis
    }
}

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-testkit";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-testkit");
    }

    #[test]
    fn fixture_helpers_load_registered_fixture() {
        let registry = FixtureRegistry::new([TestFixture::new(
            "desktop-basic",
            "examples/desktop-basic/manifest.hawk.toml",
            FixtureKind::Manifest,
        )]);

        let fixture = registry.require("desktop-basic").unwrap();

        assert_eq!(fixture.path(), "examples/desktop-basic/manifest.hawk.toml");
        assert_eq!(fixture.kind(), FixtureKind::Manifest);
    }

    #[test]
    fn fixture_helpers_reject_missing_fixture() {
        let registry = FixtureRegistry::new([]);

        assert_eq!(
            registry.require("missing"),
            Err(FixtureError::MissingFixture("missing".to_string()))
        );
    }

    #[test]
    fn command_runner_records_command_and_status() {
        let record = CommandRecord::new("cargo", ["test", "-p", "hawk2ui-testkit"], 0);

        assert_eq!(record.program(), "cargo");
        assert_eq!(record.args(), &["test", "-p", "hawk2ui-testkit"]);
        assert!(record.succeeded());
    }

    #[test]
    fn diagnostic_assertion_matches_rule_and_severity() {
        let diagnostic = DiagnosticRecord::error(
            "manifest.identity.missing",
            "manifest is missing product identity",
        );

        DiagnosticAssertion::error("manifest.identity.missing")
            .assert_matches(&diagnostic)
            .unwrap();
    }

    #[test]
    fn artifact_assertion_matches_hash() {
        let artifact = ArtifactRecord::new("hawk2ui.app", "fnv1a64:0123456789abcdef");

        ArtifactAssertion::hash("fnv1a64:0123456789abcdef")
            .assert_matches(&artifact)
            .unwrap();
    }

    #[test]
    fn visual_helper_records_snapshot_metadata() {
        let snapshot = VisualSnapshot::new("main-window", 1280, 720).with_command("draw-rect");

        assert_eq!(snapshot.name(), "main-window");
        assert_eq!(snapshot.size(), (1280, 720));
        assert_eq!(snapshot.commands(), &["draw-rect"]);
    }

    #[test]
    fn security_helper_requires_rejection_case() {
        let rejection = SecurityRejection::new(
            "fs.read",
            "fixtures/security/capability-fs-read.toml",
            "security.capability.denied",
        );

        assert_eq!(rejection.capability(), "fs.read");
        assert_eq!(rejection.diagnostic_rule(), "security.capability.denied");
    }

    #[test]
    fn benchmark_helper_records_budget_name() {
        let expectation = BenchmarkExpectation::new("cold-start", "fixtures/perf/cold-start.toml")
            .with_max_millis(16);

        assert_eq!(expectation.budget_name(), "cold-start");
        assert_eq!(expectation.max_millis(), Some(16));
    }
}
