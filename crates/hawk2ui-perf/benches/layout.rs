use std::hint::black_box;

use hawk2ui_perf::{
    BenchmarkCase, BenchmarkKind, BenchmarkMeasurement, BenchmarkSuite, PerformanceBudgets,
};

const BUDGETS: &str = include_str!("../../../performance/budgets.toml");

fn main() {
    let budgets = PerformanceBudgets::parse(BUDGETS).expect("performance budgets parse");
    let suite = BenchmarkSuite::new("layout")
        .with_case(
            BenchmarkCase::new(
                "layout-pass",
                "examples/desktop-dashboard",
                BenchmarkKind::Layout,
            )
            .with_measurement(BenchmarkMeasurement::new(3)),
        )
        .with_case(
            BenchmarkCase::new(
                "text-measurement",
                "examples/desktop-dashboard",
                BenchmarkKind::Layout,
            )
            .with_measurement(BenchmarkMeasurement::new(400)),
        );

    suite
        .validate_against(&budgets)
        .expect("layout benchmarks must map to budgets");
    black_box(suite.cases.len());
}
