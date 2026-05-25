use std::hint::black_box;

use hawk2ui_perf::{BenchmarkCase, BenchmarkKind, BenchmarkSuite, PerformanceBudgets};

const BUDGETS: &str = include_str!("../../../performance/budgets.toml");

fn main() {
    let budgets = PerformanceBudgets::parse(BUDGETS).expect("performance budgets parse");
    let suite = BenchmarkSuite::new("render-baseline")
        .with_case(BenchmarkCase::new(
            "scene-export",
            "examples/desktop-dashboard",
            BenchmarkKind::Rendering,
        ))
        .with_case(BenchmarkCase::new(
            "frame-render",
            "examples/style-gallery",
            BenchmarkKind::Rendering,
        ));

    suite
        .validate_against(&budgets)
        .expect("render baseline benchmarks must map to budgets");
    black_box(suite.cases.len());
}
