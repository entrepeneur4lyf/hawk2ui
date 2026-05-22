use std::hint::black_box;

use hawk2ui_perf::{
    BenchmarkCase, BenchmarkKind, BenchmarkSuite, PerformanceBudgets, RuntimeStabilityFixture,
};

const BUDGETS: &str = include_str!("../../../performance/budgets.toml");

fn main() {
    let budgets = PerformanceBudgets::parse(BUDGETS).expect("performance budgets parse");
    let suite = BenchmarkSuite::new("runtime").with_case(BenchmarkCase::new(
        "runtime-event-dispatch",
        "examples/desktop-basic",
        BenchmarkKind::Runtime,
    ));
    let stability = RuntimeStabilityFixture::new("runtime-event-dispatch", 10_000)
        .with_failures(0)
        .with_allowed_failures(0);

    suite
        .validate_against(&budgets)
        .expect("runtime benchmarks must map to budgets");
    stability
        .validate()
        .expect("runtime stability fixture must satisfy failure limits");
    black_box(suite.cases.len());
}
