use std::hint::black_box;

use hawk2ui_perf::{BenchmarkCase, BenchmarkKind, BenchmarkSuite, PerformanceBudgets};

const BUDGETS: &str = include_str!("../../../performance/budgets.toml");

fn main() {
    let budgets = PerformanceBudgets::parse(BUDGETS).expect("performance budgets parse");
    let suite = BenchmarkSuite::new("startup")
        .with_case(BenchmarkCase::new(
            "cold-start",
            "examples/desktop-basic",
            BenchmarkKind::Startup,
        ))
        .with_case(BenchmarkCase::new(
            "artifact-load",
            "examples/desktop-basic",
            BenchmarkKind::Startup,
        ))
        .with_case(BenchmarkCase::new(
            "first-frame",
            "examples/desktop-basic",
            BenchmarkKind::Startup,
        ));

    suite
        .validate_against(&budgets)
        .expect("startup benchmarks must map to budgets");
    black_box(suite.cases.len());
}
