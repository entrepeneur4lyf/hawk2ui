#![allow(dead_code)]

use std::{
    fs,
    hint::black_box,
    path::{Path, PathBuf},
};

use hawk2ui_perf::{BenchmarkMeasurement, BenchmarkRunConfig, BenchmarkSuite, PerformanceBudgets};

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

pub(crate) fn measure_directory_bytes(fixture: &str) -> BenchmarkMeasurement {
    BenchmarkMeasurement::from_bytes(read_tree_bytes(&fixture_path(fixture)))
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
