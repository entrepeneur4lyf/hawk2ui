use std::hint::black_box;

use hawk2ui_perf::{
    BenchmarkCase, BenchmarkKind, BenchmarkMeasurement, BenchmarkSuite, PerformanceBudgets,
};

const BUDGETS: &str = include_str!("../../../performance/budgets.toml");

fn main() {
    let budgets = PerformanceBudgets::parse(BUDGETS).expect("performance budgets parse");
    let suite = BenchmarkSuite::new("style").with_case(
        BenchmarkCase::new(
            "style-compile",
            "examples/style-gallery",
            BenchmarkKind::Style,
        )
        .with_measurement(BenchmarkMeasurement::new(7)),
    );

    suite
        .validate_against(&budgets)
        .expect("style benchmarks must map to budgets");
    black_box(suite.evaluate_against(&budgets).artifact_payload());
}
