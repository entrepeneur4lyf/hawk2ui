mod common;

use hawk2ui_perf::{BenchmarkCase, BenchmarkKind, BenchmarkSuite};

fn main() {
    let budgets = common::budgets();
    let config = common::config();
    let fixture = "examples/desktop-basic";
    let suite = BenchmarkSuite::new("startup")
        .with_case(
            BenchmarkCase::new("cold-start", fixture, BenchmarkKind::Startup)
                .with_measurement(common::measure_read_tree_millis(fixture, config)),
        )
        .with_case(
            BenchmarkCase::new("artifact-load", fixture, BenchmarkKind::Startup)
                .with_measurement(common::measure_read_tree_millis(fixture, config)),
        )
        .with_case(
            BenchmarkCase::new("first-frame", fixture, BenchmarkKind::Startup)
                .with_measurement(common::measure_read_tree_millis(fixture, config)),
        );

    common::finish_suite(&suite, &budgets);
}
