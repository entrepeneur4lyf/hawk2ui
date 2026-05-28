use std::hint::black_box;

use hawk2ui_perf::{
    BenchmarkCase, BenchmarkKind, BenchmarkMeasurement, BenchmarkSuite, PerformanceBudgets,
};

const BUDGETS: &str = include_str!("../../../performance/budgets.toml");

fn main() {
    let budgets = PerformanceBudgets::parse(BUDGETS).expect("performance budgets parse");
    let suite = BenchmarkSuite::new("assets").with_case(
        BenchmarkCase::new(
            "asset-decode",
            "examples/style-gallery",
            BenchmarkKind::Assets,
        )
        .with_measurement(BenchmarkMeasurement::new(9)),
    );

    suite
        .validate_against(&budgets)
        .expect("asset benchmarks must map to budgets");
    black_box(suite.evaluate_against(&budgets).artifact_payload());
}
