#![forbid(unsafe_code)]
//! Performance budgets, benchmark helpers, and stability gates for `Hawk2UI`.

pub mod budgets;
pub mod harness;
pub mod realtime;
pub mod stability;

pub use budgets::{
    BudgetUnit, PerformanceBudget, PerformanceBudgets, PerformanceCategory, PerformanceError,
};
pub use harness::{
    BenchmarkArtifactSet, BenchmarkCase, BenchmarkError, BenchmarkKind, BenchmarkMeasurement,
    BenchmarkReport, BenchmarkReportEntry, BenchmarkRunConfig, BenchmarkSuite, MeasurementQuality,
};
pub use realtime::{
    RealtimeContext, RealtimeGuard, RealtimeGuardError, RealtimeLockPolicy, RealtimeOperation,
    RealtimeSafetyReport, RealtimeSafetyTelemetry,
};
pub use stability::{RuntimeStabilityFixture, StabilityError};

#[cfg(test)]
mod tests {
    use super::*;

    const BUDGETS: &str = include_str!("../../../performance/budgets.toml");

    #[test]
    fn loads_release_gate_budgets() {
        let budgets = PerformanceBudgets::parse(BUDGETS).expect("performance budgets parse");

        assert!(budgets.contains("cold-start"));
        assert!(budgets.contains("layout-pass"));
        assert!(budgets.release_gates().all(|budget| budget.release_gate));
    }

    #[test]
    fn performance_budget_file_covers_required_production_domains() {
        let budgets = PerformanceBudgets::parse(BUDGETS).expect("performance budgets parse");

        for name in [
            "cold-start",
            "artifact-load",
            "first-frame",
            "layout-pass",
            "style-compile",
            "scene-export",
            "frame-render",
            "scene-node-count",
            "paint-command-count",
            "text-measurement",
            "runtime-event-dispatch",
            "runtime-dispatch-operation-count",
            "js-evaluate",
            "asset-decode",
            "memory-working-set",
            "package-size",
            "package-verify",
            "desktop-host-frame",
            "plugin-audio-allocation",
        ] {
            assert!(budgets.contains(name), "missing budget {name}");
        }
    }

    #[test]
    fn rejects_duplicate_budget_names() {
        let duplicate = r#"
            [[budgets]]
            name = "frame-render"
            category = "rendering"
            unit = "milliseconds"
            target = 8
            maximum = 16
            release_gate = true
            fixture = "examples/style-gallery"

            [[budgets]]
            name = "frame-render"
            category = "rendering"
            unit = "milliseconds"
            target = 8
            maximum = 16
            release_gate = true
            fixture = "examples/style-gallery"
        "#;

        let error = PerformanceBudgets::parse(duplicate).expect_err("duplicate budget must fail");
        assert_eq!(
            error,
            PerformanceError::DuplicateBudget("frame-render".to_owned())
        );
    }

    #[test]
    fn rejects_target_above_maximum() {
        let invalid = r#"
            [[budgets]]
            name = "layout-pass"
            category = "layout"
            unit = "milliseconds"
            target = 12
            maximum = 8
            release_gate = true
            fixture = "examples/desktop-dashboard"
        "#;

        let error = PerformanceBudgets::parse(invalid).expect_err("target above max must fail");
        assert_eq!(
            error,
            PerformanceError::TargetExceedsMaximum("layout-pass".to_owned())
        );
    }

    #[test]
    fn benchmark_suite_rejects_cases_without_matching_budget() {
        let budgets = PerformanceBudgets::parse(BUDGETS).expect("performance budgets parse");
        let suite = BenchmarkSuite::new("startup").with_case(BenchmarkCase::new(
            "missing-budget",
            "examples/desktop-basic",
            BenchmarkKind::Startup,
        ));

        let error = suite
            .validate_against(&budgets)
            .expect_err("case without budget must fail");

        assert_eq!(
            error,
            BenchmarkError::MissingBudget("missing-budget".to_owned())
        );
    }

    #[test]
    fn benchmark_suite_rejects_cases_without_measurements() {
        let budgets = PerformanceBudgets::parse(BUDGETS).expect("performance budgets parse");
        let suite = BenchmarkSuite::new("render").with_case(BenchmarkCase::new(
            "frame-render",
            "examples/style-gallery",
            BenchmarkKind::Rendering,
        ));

        let error = suite
            .validate_against(&budgets)
            .expect_err("case without measurement must fail");

        assert_eq!(
            error,
            BenchmarkError::MissingMeasurement("frame-render".to_owned())
        );
    }

    #[test]
    fn benchmark_suite_rejects_measurements_above_budget_maximum() {
        let budgets = PerformanceBudgets::parse(BUDGETS).expect("performance budgets parse");
        let suite = BenchmarkSuite::new("render").with_case(
            BenchmarkCase::new(
                "frame-render",
                "examples/style-gallery",
                BenchmarkKind::Rendering,
            )
            .with_measurement(BenchmarkMeasurement::new(17)),
        );

        let error = suite
            .validate_against(&budgets)
            .expect_err("measurement above maximum must fail");

        assert_eq!(
            error,
            BenchmarkError::BudgetExceeded {
                budget_name: "frame-render".to_owned(),
                observed: 17,
                maximum: 16,
            }
        );
    }

    #[test]
    fn benchmark_suite_rejects_advisory_wall_clock_measurements_for_release_gates() {
        let budgets = PerformanceBudgets::parse(
            r#"
            [[budgets]]
            name = "frame-render"
            category = "rendering"
            unit = "milliseconds"
            target = 8
            maximum = 16
            release_gate = true
            fixture = "examples/style-gallery"
        "#,
        )
        .expect("performance budgets parse");
        let suite = BenchmarkSuite::new("render").with_case(
            BenchmarkCase::new(
                "frame-render",
                "examples/style-gallery",
                BenchmarkKind::Rendering,
            )
            .with_measurement(BenchmarkMeasurement::measure_millis(|| {})),
        );

        assert_eq!(
            suite.validate_against(&budgets),
            Err(BenchmarkError::AdvisoryMeasurementUsedForReleaseGate(
                "frame-render".to_owned()
            ))
        );
        let report = suite.evaluate_against(&budgets);
        assert_eq!(report.failed_count(), 1);
        assert!(
            report
                .artifact_payload()
                .contains("failure = \"advisory-measurement\"")
        );
    }

    #[test]
    fn release_gate_budget_file_uses_only_deterministic_units() {
        let budgets = PerformanceBudgets::parse(BUDGETS).expect("performance budgets parse");

        for budget in budgets.release_gates() {
            assert!(
                matches!(budget.unit, BudgetUnit::Bytes | BudgetUnit::Count),
                "release gate `{}` must use bytes/count, not {:?}",
                budget.name,
                budget.unit
            );
        }
    }

    #[test]
    fn benchmark_suite_rejects_fixture_mismatch() {
        let budgets = PerformanceBudgets::parse(BUDGETS).expect("performance budgets parse");
        let suite = BenchmarkSuite::new("render").with_case(
            BenchmarkCase::new(
                "frame-render",
                "examples/desktop-basic",
                BenchmarkKind::Rendering,
            )
            .with_measurement(BenchmarkMeasurement::new(1)),
        );

        let error = suite
            .validate_against(&budgets)
            .expect_err("fixture mismatch must fail");

        assert_eq!(
            error,
            BenchmarkError::FixtureMismatch {
                budget_name: "frame-render".to_owned(),
                expected: "examples/style-gallery".to_owned(),
                actual: "examples/desktop-basic".to_owned(),
            }
        );
    }

    #[test]
    fn benchmark_suite_rejects_category_mismatch() {
        let budgets = PerformanceBudgets::parse(BUDGETS).expect("performance budgets parse");
        let suite = BenchmarkSuite::new("render").with_case(
            BenchmarkCase::new(
                "frame-render",
                "examples/style-gallery",
                BenchmarkKind::Style,
            )
            .with_measurement(BenchmarkMeasurement::new(1)),
        );

        let error = suite
            .validate_against(&budgets)
            .expect_err("category mismatch must fail");

        assert_eq!(
            error,
            BenchmarkError::CategoryMismatch {
                budget_name: "frame-render".to_owned(),
                expected: PerformanceCategory::Rendering,
                actual: BenchmarkKind::Style,
            }
        );
    }

    #[test]
    fn benchmark_suite_requires_release_gate_coverage() {
        let budgets = PerformanceBudgets::parse(BUDGETS).expect("performance budgets parse");
        let partial = BenchmarkSuite::new("partial").with_case(
            BenchmarkCase::new(
                "cold-start",
                "examples/desktop-basic",
                BenchmarkKind::Startup,
            )
            .with_measurement(BenchmarkMeasurement::new(1)),
        );

        assert_eq!(
            BenchmarkSuite::validate_release_gate_coverage(&budgets, [&partial]),
            Err(BenchmarkError::MissingReleaseGateCase(
                "scene-node-count".to_owned()
            ))
        );

        let mut complete = BenchmarkSuite::new("complete");
        for budget in budgets.release_gates() {
            complete = complete.with_case(
                BenchmarkCase::new(
                    budget.name.clone(),
                    budget.fixture.clone(),
                    kind_for_category(budget.category),
                )
                .with_measurement(BenchmarkMeasurement::new(budget.target)),
            );
        }
        BenchmarkSuite::validate_release_gate_coverage(&budgets, [&complete])
            .expect("complete release gate coverage passes");
    }

    #[test]
    fn benchmark_run_config_parses_quick_and_iterations() {
        let quick = BenchmarkRunConfig::from_args(["--quick"]);
        let explicit = BenchmarkRunConfig::from_args(["--quick", "--iterations=7"]);

        assert!(quick.quick());
        assert_eq!(quick.iterations(), 1);
        assert!(explicit.quick());
        assert_eq!(explicit.iterations(), 7);
    }

    #[test]
    fn benchmark_measurement_executes_observed_operation() {
        let mut executed = false;
        let measurement = BenchmarkMeasurement::measure_micros(|| {
            executed = true;
        });

        assert!(executed);
        let _ = measurement.observed;
    }

    #[test]
    fn benchmark_suite_writes_release_evidence_report() {
        let budgets = PerformanceBudgets::parse(BUDGETS).expect("performance budgets parse");
        let suite = BenchmarkSuite::new("production-matrix")
            .with_case(
                BenchmarkCase::new(
                    "style-compile",
                    "examples/style-gallery",
                    BenchmarkKind::Style,
                )
                .with_measurement(BenchmarkMeasurement::new(7)),
            )
            .with_case(
                BenchmarkCase::new(
                    "js-evaluate",
                    "examples/frameworks/svelte-basic",
                    BenchmarkKind::Script,
                )
                .with_measurement(BenchmarkMeasurement::new(25)),
            );

        let report = suite.evaluate_against(&budgets);
        assert_eq!(report.case_count(), 2);
        assert_eq!(report.failed_count(), 1);
        assert!(!report.accepted());
        assert!(
            report
                .artifact_payload()
                .contains("budget = \"style-compile\"")
        );
        assert!(
            report
                .artifact_payload()
                .contains("failure = \"budget-exceeded\"")
        );
        toml::from_str::<toml::Value>(&report.artifact_payload())
            .expect("benchmark evidence is valid TOML");

        let root =
            std::env::temp_dir().join(format!("hawk2ui-benchmark-evidence-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let artifacts = suite
            .write_report_artifact(&budgets, &root)
            .expect("benchmark evidence writes");
        assert_eq!(artifacts.root(), root.as_path());
        assert_eq!(artifacts.files().len(), 1);
        let evidence =
            std::fs::read_to_string(&artifacts.files()[0]).expect("benchmark evidence reads");
        assert!(evidence.contains("suite = \"production-matrix\""));
        assert!(evidence.contains("failed_count = 1"));
        std::fs::remove_dir_all(root).expect("benchmark evidence temp directory cleans up");
    }

    #[test]
    fn runtime_stability_fixture_detects_failure_rate_above_limit() {
        let fixture = RuntimeStabilityFixture::new("event-dispatch", 100)
            .with_failures(3)
            .with_allowed_failures(2);

        let error = fixture
            .validate()
            .expect_err("failure count above limit must fail");

        assert_eq!(
            error,
            StabilityError::FailureLimitExceeded {
                name: "event-dispatch".to_owned(),
                failures: 3,
                allowed: 2
            }
        );
    }

    #[test]
    fn runtime_stability_fixture_runs_observed_iterations() {
        let fixture = RuntimeStabilityFixture::run("event-dispatch", 5, |index| index % 2 == 0)
            .with_allowed_failures(2);

        assert_eq!(fixture.iterations, 5);
        assert_eq!(fixture.failures, 2);
        fixture.validate().expect("observed failures fit limit");
    }

    #[test]
    fn realtime_guard_denies_audio_thread_unsafe_operations() {
        let guard = RealtimeGuard::audio_thread();

        assert_eq!(
            guard.check(RealtimeOperation::Allocation),
            Err(RealtimeGuardError::Denied {
                context: RealtimeContext::AudioThread,
                operation: RealtimeOperation::Allocation
            })
        );
        assert!(guard.check(RealtimeOperation::PreallocatedWrite).is_ok());
    }

    #[test]
    fn realtime_guard_reports_audio_thread_policy_and_telemetry() {
        let guard = RealtimeGuard::audio_thread();
        let report = guard.audit([
            RealtimeOperation::PreallocatedWrite,
            RealtimeOperation::Allocation,
            RealtimeOperation::BlockingWait,
        ]);

        assert_eq!(guard.lock_policy(), RealtimeLockPolicy::NoBlockingLocks);
        assert_eq!(report.context, RealtimeContext::AudioThread);
        assert_eq!(report.allowed_count(), 1);
        assert_eq!(report.denied_count(), 2);
        assert_eq!(report.telemetry.allocation_attempts, 1);
        assert_eq!(report.telemetry.blocking_wait_attempts, 1);
        assert!(
            report
                .denied_operations
                .contains(&RealtimeOperation::Allocation)
        );
        assert!(
            report
                .denied_operations
                .contains(&RealtimeOperation::BlockingWait)
        );
        assert!(RealtimeOperation::Allocation.is_denied_on_audio_thread());
        assert!(!RealtimeOperation::PreallocatedWrite.is_denied_on_audio_thread());
    }

    fn kind_for_category(category: PerformanceCategory) -> BenchmarkKind {
        match category {
            PerformanceCategory::Startup => BenchmarkKind::Startup,
            PerformanceCategory::Layout => BenchmarkKind::Layout,
            PerformanceCategory::Style => BenchmarkKind::Style,
            PerformanceCategory::Rendering => BenchmarkKind::Rendering,
            PerformanceCategory::Runtime => BenchmarkKind::Runtime,
            PerformanceCategory::Script => BenchmarkKind::Script,
            PerformanceCategory::Assets => BenchmarkKind::Assets,
            PerformanceCategory::Memory => BenchmarkKind::Memory,
            PerformanceCategory::Package => BenchmarkKind::Package,
            PerformanceCategory::Host => BenchmarkKind::Host,
            PerformanceCategory::Realtime => BenchmarkKind::Realtime,
        }
    }
}
