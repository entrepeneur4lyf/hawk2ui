mod common;

use hawk2ui_perf::{BenchmarkCase, BenchmarkKind, BenchmarkSuite};

fn main() {
    let budgets = common::budgets();
    let config = common::config();
    let fixture = "examples/desktop-dashboard";
    let suite = BenchmarkSuite::new("layout")
        .with_case(
            BenchmarkCase::new("layout-pass", fixture, BenchmarkKind::Layout)
                .with_measurement(common::measure_read_tree_millis(fixture, config)),
        )
        .with_case(
            BenchmarkCase::new("text-measurement", fixture, BenchmarkKind::Layout)
                .with_measurement(common::measure_counter_micros(config, 8)),
        );

    common::finish_suite(&suite, &budgets);
}
