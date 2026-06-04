mod common;

use hawk2ui_perf::{BenchmarkCase, BenchmarkKind, BenchmarkSuite};

fn main() {
    let budgets = common::budgets();
    let config = common::config();
    let fixture = "examples/desktop-basic";
    let suite = BenchmarkSuite::new("desktop-host").with_case(
        BenchmarkCase::new("desktop-host-frame", fixture, BenchmarkKind::Host)
            .with_measurement(common::measure_read_tree_millis(fixture, config)),
    );

    common::finish_suite(&suite, &budgets);
}
