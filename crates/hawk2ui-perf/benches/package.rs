mod common;

use hawk2ui_perf::{BenchmarkCase, BenchmarkKind, BenchmarkSuite};

fn main() {
    let budgets = common::budgets();
    let config = common::config();
    let fixture = "examples/desktop-basic";
    let suite = BenchmarkSuite::new("package")
        .with_case(
            BenchmarkCase::new("package-size", fixture, BenchmarkKind::Package)
                .with_measurement(common::measure_directory_bytes(fixture)),
        )
        .with_case(
            BenchmarkCase::new("package-verify", fixture, BenchmarkKind::Package)
                .with_measurement(common::measure_read_tree_millis(fixture, config)),
        );

    common::finish_suite(&suite, &budgets);
}
