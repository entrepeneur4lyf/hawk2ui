use std::hint::black_box;

use hawk2ui_perf::{
    BenchmarkCase, BenchmarkKind, BenchmarkMeasurement, BenchmarkSuite, PerformanceBudgets,
};

const BUDGETS: &str = include_str!("../../../performance/budgets.toml");

fn main() {
    let budgets = PerformanceBudgets::parse(BUDGETS).expect("performance budgets parse");
    let suite = BenchmarkSuite::new("render")
        .with_case(
            BenchmarkCase::new(
                "scene-export",
                "examples/desktop-dashboard",
                BenchmarkKind::Rendering,
            )
            .with_measurement(BenchmarkMeasurement::new(3)),
        )
        .with_case(
            BenchmarkCase::new(
                "frame-render",
                "examples/style-gallery",
                BenchmarkKind::Rendering,
            )
            .with_measurement(BenchmarkMeasurement::new(7)),
        );

    suite
        .validate_against(&budgets)
        .expect("render benchmarks must map to budgets");
    black_box(suite.cases.len());
}
