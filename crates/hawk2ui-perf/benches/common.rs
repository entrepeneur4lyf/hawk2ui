#![allow(dead_code)]

use std::{
    fs,
    hint::black_box,
    path::{Path, PathBuf},
};

use hawk2ui_build::{ArtifactSchemaVersion, BuildWorkspace, BuildWorkspaceOutput};
use hawk2ui_layout::{FlexDirection, LayoutSizing, LayoutStyle, Viewport};
use hawk2ui_perf::{BenchmarkMeasurement, BenchmarkRunConfig, BenchmarkSuite, PerformanceBudgets};
use hawk2ui_render::Color;
use hawk2ui_runtime::{
    RuntimeEvent, RuntimeEventDispatcher, RuntimeEventKind, RuntimeSceneBridge, RuntimeViewId,
    RuntimeViewNode, RuntimeViewTree, RuntimeVisual,
};
use hawk2ui_smoke::{SmokeFixture, SmokeRunner, SmokeTargetKind};

const BUDGETS: &str = include_str!("../../../performance/budgets.toml");

pub(crate) fn budgets() -> PerformanceBudgets {
    PerformanceBudgets::parse(BUDGETS).expect("performance budgets parse")
}

pub(crate) fn config() -> BenchmarkRunConfig {
    BenchmarkRunConfig::from_env_args()
}

pub(crate) fn measure_read_tree_millis(
    fixture: &str,
    config: BenchmarkRunConfig,
) -> BenchmarkMeasurement {
    let root = fixture_path(fixture);
    BenchmarkMeasurement::measure_millis(|| {
        for _ in 0..config.iterations() {
            black_box(read_tree_bytes(&root));
        }
    })
}

pub(crate) fn measure_read_tree_micros(
    fixture: &str,
    config: BenchmarkRunConfig,
) -> BenchmarkMeasurement {
    let root = fixture_path(fixture);
    BenchmarkMeasurement::measure_micros(|| {
        for _ in 0..config.iterations() {
            black_box(read_tree_bytes(&root));
        }
    })
}

pub(crate) fn measure_counter_micros(
    config: BenchmarkRunConfig,
    operations_per_iteration: u64,
) -> BenchmarkMeasurement {
    BenchmarkMeasurement::measure_micros(|| {
        let mut accumulator = 0_u64;
        for outer in 0..config.iterations() {
            for inner in 0..operations_per_iteration {
                accumulator = accumulator.rotate_left(3) ^ outer.wrapping_mul(31) ^ inner;
            }
        }
        black_box(accumulator);
    })
}

pub(crate) fn measure_dashboard_smoke_scene_node_count(fixture: &str) -> BenchmarkMeasurement {
    let result = SmokeRunner
        .run_desktop_dashboard(&SmokeFixture::from_workspace(
            fixture,
            SmokeTargetKind::Desktop,
        ))
        .unwrap_or_else(|error| panic!("dashboard smoke benchmark failed for {fixture}: {error}"));
    BenchmarkMeasurement::observed_count(u64::try_from(result.layout_nodes).unwrap_or(u64::MAX))
}

pub(crate) fn measure_runtime_paint_command_count(_fixture: &str) -> BenchmarkMeasurement {
    let frame = benchmark_runtime_frame(18);
    BenchmarkMeasurement::observed_count(
        u64::try_from(frame.paint_commands().commands().len()).unwrap_or(u64::MAX),
    )
}

pub(crate) fn measure_runtime_dispatch_operation_count(operations: u64) -> BenchmarkMeasurement {
    let mut dispatcher = RuntimeEventDispatcher::default();
    dispatcher.listen("bench-target", RuntimeEventKind::Ui);
    for _ in 0..operations {
        dispatcher.enqueue(RuntimeEvent::ui("bench-target", "press"));
    }
    let deliveries = dispatcher
        .dispatch_pending()
        .unwrap_or_else(|error| panic!("runtime dispatch benchmark failed: {error:?}"));
    BenchmarkMeasurement::observed_count(u64::try_from(deliveries.len()).unwrap_or(u64::MAX))
}

pub(crate) fn measure_dashboard_smoke_frame_bytes(fixture: &str) -> BenchmarkMeasurement {
    let result = SmokeRunner
        .run_desktop_dashboard(&SmokeFixture::from_workspace(
            fixture,
            SmokeTargetKind::Desktop,
        ))
        .unwrap_or_else(|error| panic!("dashboard smoke benchmark failed for {fixture}: {error}"));
    let [width, height] = result.software_frame.physical_size;
    let pixel_bytes = u64::from(width)
        .saturating_mul(u64::from(height))
        .saturating_mul(4);
    BenchmarkMeasurement::observed_bytes(pixel_bytes)
}

pub(crate) fn measure_build_artifact_payload_bytes(fixture: &str) -> BenchmarkMeasurement {
    let output = build_workspace_output(fixture);
    BenchmarkMeasurement::observed_bytes(artifact_payload_size(&output))
}

pub(crate) fn finish_suite(suite: &BenchmarkSuite, budgets: &PerformanceBudgets) {
    suite
        .validate_against(budgets)
        .expect("benchmarks must map to budgets and pass configured maxima");
    let report = suite.evaluate_against(budgets);
    assert!(report.accepted(), "benchmark report must be accepted");
    black_box(report.artifact_payload());
}

fn fixture_path(fixture: &str) -> PathBuf {
    workspace_root().join(fixture)
}

fn build_workspace_output(fixture: &str) -> BuildWorkspaceOutput {
    BuildWorkspace::load(fixture_path(fixture))
        .and_then(|workspace| workspace.build(ArtifactSchemaVersion::new(1, 0)))
        .unwrap_or_else(|error| panic!("benchmark build failed for {fixture}: {error:?}"))
}

fn artifact_payload_size(output: &BuildWorkspaceOutput) -> u64 {
    let artifact = &output.artifact;
    let mut bytes = usize_to_u64(artifact.manifest_snapshot.len())
        .saturating_add(usize_to_u64(artifact.manifest_snapshot_hash.0.len()))
        .saturating_add(usize_to_u64(artifact.hashes.manifest.0.len()))
        .saturating_add(usize_to_u64(artifact.hashes.content.0.len()))
        .saturating_add(usize_to_u64(artifact.build_metadata.generator.len()))
        .saturating_add(usize_to_u64(artifact.build_metadata.profile.len()))
        .saturating_add(usize_to_u64(artifact.signature.algorithm.len()))
        .saturating_add(usize_to_u64(artifact.signature.key_id.len()))
        .saturating_add(usize_to_u64(artifact.signature.signature.len()));
    for script in &artifact.compiled_scripts {
        bytes = bytes
            .saturating_add(usize_to_u64(script.entrypoint_id.len()))
            .saturating_add(usize_to_u64(script.source_path.len()))
            .saturating_add(usize_to_u64(script.artifact_path.len()))
            .saturating_add(usize_to_u64(script.source_hash.0.len()))
            .saturating_add(usize_to_u64(script.compiled_source.len()));
    }
    for framework in &artifact.compiled_frameworks {
        bytes = bytes
            .saturating_add(usize_to_u64(framework.entrypoint_id.len()))
            .saturating_add(usize_to_u64(framework.source_path.len()))
            .saturating_add(usize_to_u64(framework.artifact_path.len()))
            .saturating_add(usize_to_u64(framework.source_hash.0.len()))
            .saturating_add(usize_to_u64(framework.compiler_artifact_json.len()));
    }
    for style in &artifact.compiled_styles {
        bytes = bytes
            .saturating_add(usize_to_u64(style.entrypoint_id.len()))
            .saturating_add(usize_to_u64(style.source_path.len()))
            .saturating_add(usize_to_u64(style.artifact_path.len()))
            .saturating_add(usize_to_u64(style.source_hash.0.len()));
    }
    for asset in &artifact.asset_manifest {
        bytes = bytes
            .saturating_add(usize_to_u64(asset.id.len()))
            .saturating_add(usize_to_u64(asset.kind.len()))
            .saturating_add(usize_to_u64(asset.artifact_path.len()))
            .saturating_add(usize_to_u64(asset.hash.0.len()));
    }
    for asset in &artifact.compiled_assets {
        bytes = bytes
            .saturating_add(usize_to_u64(asset.id.len()))
            .saturating_add(usize_to_u64(asset.source_path.len()))
            .saturating_add(usize_to_u64(asset.artifact_path.len()))
            .saturating_add(usize_to_u64(asset.source_hash.0.len()));
    }
    for capability in &artifact.capabilities {
        bytes = bytes.saturating_add(usize_to_u64(capability.len()));
    }
    for target in &artifact.target_metadata {
        bytes = bytes.saturating_add(usize_to_u64(target.name.len()));
    }
    if let Some(scene) = &artifact.runtime_scene {
        bytes = bytes.saturating_add(usize_to_u64(scene.to_string().len()));
    }
    bytes
}

fn benchmark_runtime_frame(node_count: usize) -> hawk2ui_runtime::RuntimeSceneFrame {
    let root_id = RuntimeViewId::new("bench-root");
    let root = RuntimeViewNode::new(
        root_id.clone(),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(640.0, 360.0)),
        RuntimeVisual::Fill(Color::rgba(10, 12, 16, 255)),
    );
    let mut tree = RuntimeViewTree::new(root);
    for index in 1..node_count {
        let child_id = RuntimeViewId::new(format!("bench-child-{index}"));
        let child = RuntimeViewNode::new(
            child_id,
            LayoutStyle::flex_container(FlexDirection::Column)
                .with_size(LayoutSizing::fixed(32.0, 12.0)),
            RuntimeVisual::Fill(Color::rgba(20, 24, 32, 255)),
        );
        tree = tree
            .with_child(&root_id, child)
            .unwrap_or_else(|error| panic!("benchmark runtime tree failed: {error:?}"));
    }
    RuntimeSceneBridge::new(Viewport::new(640.0, 360.0))
        .build(&tree)
        .unwrap_or_else(|error| panic!("benchmark runtime scene failed: {error:?}"))
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("perf crate lives under crates/")
        .to_path_buf()
}

fn read_tree_bytes(root: &Path) -> u64 {
    let mut files = Vec::new();
    collect_files(root, &mut files);
    files.sort();
    let mut total = 0_u64;
    for file in files {
        let bytes = fs::read(&file).unwrap_or_else(|error| {
            panic!(
                "failed to read benchmark fixture {}: {error}",
                file.display()
            )
        });
        total = total.saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
        black_box(bytes);
    }
    total
}

fn collect_files(path: &Path, files: &mut Vec<PathBuf>) {
    if path.is_file() {
        files.push(path.to_path_buf());
        return;
    }
    let mut entries = fs::read_dir(path)
        .unwrap_or_else(|error| {
            panic!(
                "failed to read benchmark fixture {}: {error}",
                path.display()
            )
        })
        .map(|entry| {
            entry
                .expect("benchmark fixture directory entry reads")
                .path()
        })
        .collect::<Vec<_>>();
    entries.sort();
    for entry in entries {
        if entry.is_dir() {
            collect_files(&entry, files);
        } else if entry.is_file() {
            files.push(entry);
        }
    }
}
