mod common;

use hawk2ui_perf::{BenchmarkCase, BenchmarkKind, BenchmarkSuite};

fn main() {
    let budgets = common::budgets();
    let config = common::config();
    let fixture = "examples/frameworks/svelte-basic";
    let suite = BenchmarkSuite::new("script").with_case(
        BenchmarkCase::new("js-evaluate", fixture, BenchmarkKind::Script)
            .with_measurement(common::measure_read_tree_millis(fixture, config)),
    );

    common::finish_suite(&suite, &budgets);
}
