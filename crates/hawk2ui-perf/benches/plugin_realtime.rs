mod common;

use hawk2ui_perf::{
    BenchmarkCase, BenchmarkKind, BenchmarkMeasurement, BenchmarkSuite, RealtimeGuard,
    RealtimeOperation,
};

fn main() {
    let budgets = common::budgets();
    let config = common::config();
    let fixture = "examples/plugin-meter-analyzer";
    let guard = RealtimeGuard::audio_thread();
    let report =
        guard.audit((0..config.iterations()).map(|_| RealtimeOperation::PreallocatedWrite));
    let suite = BenchmarkSuite::new("plugin-realtime").with_case(
        BenchmarkCase::new("plugin-audio-allocation", fixture, BenchmarkKind::Realtime)
            .with_measurement(BenchmarkMeasurement::observed_count(
                u64::try_from(report.telemetry.allocation_attempts).unwrap_or(u64::MAX),
            )),
    );

    common::finish_suite(&suite, &budgets);
    assert_eq!(report.denied_count(), 0);
}
