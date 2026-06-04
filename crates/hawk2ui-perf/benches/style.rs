mod common;

use hawk2ui_perf::{BenchmarkCase, BenchmarkKind, BenchmarkSuite};

fn main() {
    let budgets = common::budgets();
    let config = common::config();
    let fixture = "examples/style-gallery";
    let suite = BenchmarkSuite::new("style").with_case(
        BenchmarkCase::new("style-compile", fixture, BenchmarkKind::Style)
            .with_measurement(common::measure_read_tree_millis(fixture, config)),
    );

    common::finish_suite(&suite, &budgets);
}
