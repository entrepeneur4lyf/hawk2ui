use std::hint::black_box;

use hawk2ui_perf::{
    BenchmarkCase, BenchmarkKind, BenchmarkMeasurement, BenchmarkSuite, PerformanceBudgets,
};

const BUDGETS: &str = include_str!("../../../performance/budgets.toml");

fn main() {
    let budgets = PerformanceBudgets::parse(BUDGETS).expect("performance budgets parse");
    let suite = BenchmarkSuite::new("package").with_case(
        BenchmarkCase::new(
            "package-verify",
            "examples/release-package",
            BenchmarkKind::Package,
        )
        .with_measurement(BenchmarkMeasurement::new(120)),
    );

    suite
        .validate_against(&budgets)
        .expect("package benchmarks must map to budgets");
    black_box(suite.evaluate_against(&budgets).artifact_payload());
}
