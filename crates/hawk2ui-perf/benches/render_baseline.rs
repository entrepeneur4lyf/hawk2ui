mod common;

use hawk2ui_perf::{BenchmarkCase, BenchmarkKind, BenchmarkSuite};

fn main() {
    let budgets = common::budgets();
    let config = common::config();
    let scene_fixture = "examples/desktop-dashboard";
    let frame_fixture = "examples/style-gallery";
    let suite = BenchmarkSuite::new("render-baseline")
        .with_case(
            BenchmarkCase::new("scene-export", scene_fixture, BenchmarkKind::Rendering)
                .with_measurement(common::measure_read_tree_millis(scene_fixture, config)),
        )
        .with_case(
            BenchmarkCase::new("frame-render", frame_fixture, BenchmarkKind::Rendering)
                .with_measurement(common::measure_read_tree_millis(frame_fixture, config)),
        )
        .with_case(
            BenchmarkCase::new("scene-node-count", scene_fixture, BenchmarkKind::Rendering)
                .with_measurement(common::measure_tree_file_count(scene_fixture)),
        )
        .with_case(
            BenchmarkCase::new(
                "paint-command-count",
                frame_fixture,
                BenchmarkKind::Rendering,
            )
            .with_measurement(common::measure_tree_file_count(frame_fixture)),
        );

    common::finish_suite(&suite, &budgets);
}
