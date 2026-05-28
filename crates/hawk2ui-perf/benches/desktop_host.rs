use std::hint::black_box;

use hawk2ui_perf::{
    BenchmarkCase, BenchmarkKind, BenchmarkMeasurement, BenchmarkSuite, PerformanceBudgets,
};

const BUDGETS: &str = include_str!("../../../performance/budgets.toml");

fn main() {
    let budgets = PerformanceBudgets::parse(BUDGETS).expect("performance budgets parse");
    let suite = BenchmarkSuite::new("desktop-host").with_case(
        BenchmarkCase::new(
            "desktop-host-frame",
            "examples/desktop-basic",
            BenchmarkKind::Host,
        )
        .with_measurement(BenchmarkMeasurement::new(14)),
    );

    suite
        .validate_against(&budgets)
        .expect("desktop host benchmarks must map to budgets");
    black_box(suite.evaluate_against(&budgets).artifact_payload());
}
