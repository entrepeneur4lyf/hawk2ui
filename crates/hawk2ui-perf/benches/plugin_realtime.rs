use std::hint::black_box;

use hawk2ui_perf::{
    BenchmarkCase, BenchmarkKind, BenchmarkMeasurement, BenchmarkSuite, PerformanceBudgets,
    RealtimeGuard, RealtimeOperation,
};

const BUDGETS: &str = include_str!("../../../performance/budgets.toml");

fn main() {
    let budgets = PerformanceBudgets::parse(BUDGETS).expect("performance budgets parse");
    let suite = BenchmarkSuite::new("plugin-realtime").with_case(
        BenchmarkCase::new(
            "plugin-audio-allocation",
            "examples/plugin-meter-analyzer",
            BenchmarkKind::Realtime,
        )
        .with_measurement(BenchmarkMeasurement::new(0)),
    );
    let guard = RealtimeGuard::audio_thread();

    suite
        .validate_against(&budgets)
        .expect("plugin realtime benchmarks must map to budgets");
    guard
        .check(RealtimeOperation::PreallocatedWrite)
        .expect("preallocated realtime writes must be permitted");
    black_box(suite.cases.len());
}
