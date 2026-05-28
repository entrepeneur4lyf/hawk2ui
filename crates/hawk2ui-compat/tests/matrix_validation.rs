use std::fs;
use std::path::PathBuf;

use hawk2ui_compat::{
    CompatibilityMatrix, CoverageStatus, GraphicsCompatibilityMatrix, HostCompatibilityMatrix,
    MatrixError, PackageCompatibilityMatrix, ReleaseStatus, SurfaceKind,
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("compat crate lives under crates/hawk2ui-compat")
        .to_path_buf()
}

fn read_workspace_file(path: &str) -> String {
    let path = workspace_root().join(path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("required compatibility file `{}`: {error}", path.display()))
}

#[test]
fn os_matrix_declares_supported_platforms_with_ci_and_package_coverage() {
    let matrix = CompatibilityMatrix::parse(&read_workspace_file("compatibility/matrix.toml"))
        .expect("OS compatibility matrix parses");

    for target in &matrix.targets {
        assert!(!target.name.trim().is_empty(), "target name is required");
        assert!(
            !target.family.trim().is_empty(),
            "target family is required"
        );
        assert!(
            !target.os_version.trim().is_empty(),
            "target OS version is required"
        );
        assert!(
            !target.architecture.trim().is_empty(),
            "target architecture is required"
        );
        assert!(
            !target.windowing.trim().is_empty(),
            "target windowing is required"
        );
        assert!(
            !target.accessibility.trim().is_empty(),
            "target accessibility path is required"
        );
        assert!(
            !target.packaging.trim().is_empty(),
            "target packaging is required"
        );
        if target.release == ReleaseStatus::Supported {
            assert!(target.ci_coverage, "supported targets require CI coverage");
        }
    }

    assert!(matrix.contains_target("windows-desktop"));
    assert!(matrix.contains_target("macos-desktop"));
    assert!(matrix.contains_target("linux-wayland-desktop"));
    assert!(
        matrix
            .targets
            .iter()
            .any(|target| target.surface == SurfaceKind::Plugin)
    );
}

#[test]
fn graphics_matrix_maps_every_render_feature_to_a_supported_backend() {
    let matrix =
        GraphicsCompatibilityMatrix::parse(&read_workspace_file("compatibility/graphics.toml"))
            .expect("graphics matrix parses");

    for feature in [
        "cpu-raster",
        "high-dpi",
        "text-shaping",
        "image-layers",
        "vector-layers",
        "effects",
        "dirty-regions",
    ] {
        assert!(
            matrix.supports_feature(feature),
            "missing supported graphics feature {feature}"
        );
    }

    let diagnostic = matrix
        .unsupported_feature_diagnostic("skia-cpu-raster", "gpu-rendering")
        .expect("unsupported feature should produce diagnostic");
    assert_eq!(diagnostic.rule.as_str(), "backend.capability.unsupported");
    assert!(diagnostic.message.contains("gpu-rendering"));
}

#[test]
fn plugin_host_matrix_declares_editor_lifecycle_state_and_realtime_coverage() {
    let matrix = HostCompatibilityMatrix::parse(&read_workspace_file("compatibility/hosts.toml"))
        .expect("host matrix parses");

    for format in ["clap", "vst3", "au", "standalone"] {
        let host = matrix
            .host(format)
            .unwrap_or_else(|| panic!("missing host compatibility row for {format}"));
        assert!(
            host.host_attachment.is_covered(),
            "{format} missing host attachment"
        );
        assert!(host.resize.is_covered(), "{format} missing resize coverage");
        assert!(host.dpi.is_covered(), "{format} missing DPI coverage");
        assert!(
            host.keyboard_focus.is_covered(),
            "{format} missing keyboard focus coverage"
        );
        assert!(
            host.accessibility.is_covered(),
            "{format} missing accessibility coverage"
        );
        assert!(host.state.is_covered(), "{format} missing state coverage");
        assert!(
            host.automation.is_covered(),
            "{format} missing automation coverage"
        );
        assert!(
            host.realtime_visual_data.is_covered(),
            "{format} missing realtime visual data coverage"
        );
        assert!(
            matrix.missing_coverage_diagnostics(format).is_empty(),
            "{format} should not have missing coverage diagnostics"
        );
    }
}

#[test]
fn plugin_host_matrix_reports_missing_coverage() {
    let matrix = HostCompatibilityMatrix {
        hosts: vec![hawk2ui_compat::PluginHostCompatibility {
            format: "clap".into(),
            host_attachment: CoverageStatus::Covered,
            resize: CoverageStatus::Missing,
            dpi: CoverageStatus::Covered,
            keyboard_focus: CoverageStatus::Missing,
            accessibility: CoverageStatus::Covered,
            state: CoverageStatus::Covered,
            automation: CoverageStatus::Covered,
            realtime_visual_data: CoverageStatus::Covered,
        }],
    };

    let diagnostics = matrix.missing_coverage_diagnostics("clap");
    assert_eq!(diagnostics.len(), 2);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("resize"))
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule.as_str() == "compat.host.coverage-missing")
    );
    assert_eq!(
        matrix.missing_coverage_diagnostics("unknown")[0]
            .rule
            .as_str(),
        "compat.host.missing"
    );
}

#[test]
fn packaging_matrix_declares_outputs_and_verification_commands() {
    let matrix =
        PackageCompatibilityMatrix::parse(&read_workspace_file("compatibility/packages.toml"))
            .expect("package matrix parses");

    for output in [
        "desktop-linux",
        "desktop-windows",
        "desktop-macos",
        "plugin-clap",
        "plugin-vst3",
        "plugin-au",
        "sealed-artifact",
        "debug-package",
        "release-package",
    ] {
        let package = matrix
            .package(output)
            .unwrap_or_else(|| panic!("missing package output {output}"));
        assert!(
            !package.verify_command.trim().is_empty(),
            "{output} missing verification command"
        );
    }

    let missing = matrix
        .missing_package_diagnostic("plugin-aax")
        .expect("missing package output should produce diagnostic");
    assert_eq!(missing.rule.as_str(), "compat.package.missing");
}

#[test]
fn non_target_duplicate_keys_report_precise_matrix_errors() {
    let duplicate_graphics = r#"
        [[backends]]
        backend = "skia-cpu-raster"
        supported = true
        features = ["cpu-raster"]
        diagnostic = "backend.unsupported"

        [[backends]]
        backend = "skia-cpu-raster"
        supported = true
        features = ["cpu-raster"]
        diagnostic = "backend.unsupported"
    "#;

    let error = GraphicsCompatibilityMatrix::parse(duplicate_graphics)
        .expect_err("duplicate graphics backend should fail");
    assert_eq!(
        error,
        MatrixError::DuplicateKey {
            kind: "graphics backend",
            key: "skia-cpu-raster".into()
        }
    );
    let diagnostic: hawk2ui_api::Diagnostic = error.into();
    assert_eq!(diagnostic.rule.as_str(), "compat.matrix.duplicate-key");
}

#[test]
fn compatibility_manual_documents_local_commands_and_release_coverage() {
    let manual = read_workspace_file("docs/development/compatibility.md");

    for required in [
        "Operating System Matrix",
        "Graphics Matrix",
        "Plugin Host Matrix",
        "Packaging Matrix",
        "Unsupported Target Diagnostics",
        "rtk cargo test -p hawk2ui-compat",
    ] {
        assert!(manual.contains(required), "manual missing {required}");
    }
}
