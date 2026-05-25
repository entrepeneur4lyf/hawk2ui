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
    BenchmarkCase, BenchmarkError, BenchmarkKind, BenchmarkMeasurement, BenchmarkSuite,
};
pub use realtime::{RealtimeContext, RealtimeGuard, RealtimeGuardError, RealtimeOperation};
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
            "scene-export",
            "frame-render",
            "text-measurement",
            "runtime-event-dispatch",
            "memory-working-set",
            "package-size",
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
}
