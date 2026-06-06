mod common;

use hawk2ui_perf::{
    BenchmarkCase, BenchmarkKind, BenchmarkMeasurement, BenchmarkSuite, RealtimeGuard,
    RealtimeOperation,
};

fn main() {
    let budgets = common::budgets();
    let config = common::config();
    let runtime_fixture = "examples/desktop-basic";
    let dashboard_fixture = "examples/desktop-dashboard";
    let style_fixture = "examples/style-gallery";
    let plugin_fixture = "examples/plugin-meter-analyzer";
    let realtime_report = RealtimeGuard::audio_thread()
        .audit((0..config.iterations()).map(|_| RealtimeOperation::PreallocatedWrite));

    let suite = BenchmarkSuite::new("release-gates")
        .with_case(
            BenchmarkCase::new(
                "scene-node-count",
                dashboard_fixture,
                BenchmarkKind::Rendering,
            )
            .with_measurement(common::measure_tree_file_count(dashboard_fixture)),
        )
        .with_case(
            BenchmarkCase::new(
                "paint-command-count",
                style_fixture,
                BenchmarkKind::Rendering,
            )
            .with_measurement(common::measure_tree_file_count(style_fixture)),
        )
        .with_case(
            BenchmarkCase::new(
                "runtime-dispatch-operation-count",
                runtime_fixture,
                BenchmarkKind::Runtime,
            )
            .with_measurement(common::measure_operation_count(1024)),
        )
        .with_case(
            BenchmarkCase::new(
                "memory-working-set",
                dashboard_fixture,
                BenchmarkKind::Memory,
            )
            .with_measurement(common::measure_directory_bytes(dashboard_fixture)),
        )
        .with_case(
            BenchmarkCase::new("package-size", runtime_fixture, BenchmarkKind::Package)
                .with_measurement(common::measure_directory_bytes(runtime_fixture)),
        )
        .with_case(
            BenchmarkCase::new(
                "plugin-audio-allocation",
                plugin_fixture,
                BenchmarkKind::Realtime,
            )
            .with_measurement(BenchmarkMeasurement::from_count(
                u64::try_from(realtime_report.telemetry.allocation_attempts).unwrap_or(u64::MAX),
            )),
        );

    BenchmarkSuite::validate_release_gate_coverage(&budgets, [&suite])
        .expect("release gate benchmark suite must cover every release-gating budget");
    common::finish_suite(&suite, &budgets);
    assert_eq!(realtime_report.denied_count(), 0);
}
