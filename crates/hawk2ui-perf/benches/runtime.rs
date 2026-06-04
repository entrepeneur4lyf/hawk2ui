mod common;

use hawk2ui_perf::{BenchmarkCase, BenchmarkKind, BenchmarkSuite, RuntimeStabilityFixture};

fn main() {
    let budgets = common::budgets();
    let config = common::config();
    let runtime_fixture = "examples/desktop-basic";
    let memory_fixture = "examples/desktop-dashboard";
    let suite = BenchmarkSuite::new("runtime")
        .with_case(
            BenchmarkCase::new(
                "runtime-event-dispatch",
                runtime_fixture,
                BenchmarkKind::Runtime,
            )
            .with_measurement(common::measure_counter_micros(config, 8)),
        )
        .with_case(
            BenchmarkCase::new("memory-working-set", memory_fixture, BenchmarkKind::Memory)
                .with_measurement(common::measure_directory_bytes(memory_fixture)),
        );
    let stability = RuntimeStabilityFixture::run(
        "runtime-event-dispatch",
        config.iterations().saturating_mul(128),
        |_| true,
    )
    .with_allowed_failures(0);

    common::finish_suite(&suite, &budgets);
    stability
        .validate()
        .expect("runtime stability fixture must satisfy failure limits");
}
