#![forbid(unsafe_code)]
//! Machine-readable compatibility matrices and unsupported-target diagnostics for `Hawk2UI`.

pub mod matrix;

pub use matrix::{
    CompatibilityMatrix, CoverageStatus, GraphicsBackendCompatibility, GraphicsCompatibilityMatrix,
    HostCompatibilityMatrix, MatrixError, PackageCompatibility, PackageCompatibilityMatrix,
    PluginHostCompatibility, ReleaseStatus, SurfaceKind, TargetCompatibility,
};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-compat";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

#[cfg(test)]
mod tests {
    use super::*;

    const MATRIX: &str = include_str!("../../../compatibility/matrix.toml");

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-compat");
    }

    #[test]
    fn compat_workspace_filter_marker() {
        assert_eq!(CRATE_NAME, "hawk2ui-compat");
    }

    #[test]
    fn loads_supported_targets_from_matrix() {
        let matrix = CompatibilityMatrix::parse(MATRIX).expect("compatibility matrix parses");

        assert!(matrix.contains_target("windows-desktop"));
        assert!(matrix.contains_target("macos-desktop"));
        assert!(matrix.contains_target("linux-wayland-desktop"));
        assert!(matrix.contains_target("linux-x11-plugin"));
    }

    #[test]
    fn rejects_duplicate_target_names() {
        let duplicate = r#"
            [[targets]]
            name = "linux-wayland-desktop"
            family = "linux"
            os_version = "Ubuntu 24.04 LTS or newer"
            surface = "desktop"
            architecture = "x86_64"
            windowing = "wayland"
            accessibility = "native"
            packaging = "desktop-bundle"
            release = "supported"
            ci_coverage = true

            [[targets]]
            name = "linux-wayland-desktop"
            family = "linux"
            os_version = "Ubuntu 24.04 LTS or newer"
            surface = "desktop"
            architecture = "x86_64"
            windowing = "wayland"
            accessibility = "native"
            packaging = "desktop-bundle"
            release = "supported"
            ci_coverage = true
        "#;

        let error = CompatibilityMatrix::parse(duplicate).expect_err("duplicate target must fail");
        assert_eq!(
            error,
            MatrixError::DuplicateTarget("linux-wayland-desktop".to_owned())
        );
    }

    #[test]
    fn reports_unsupported_target_diagnostic() {
        let matrix = CompatibilityMatrix::parse(MATRIX).expect("compatibility matrix parses");

        let diagnostic = matrix
            .unsupported_target_diagnostic("plan9-desktop")
            .expect("missing target returns diagnostic");

        assert!(diagnostic.contains("unsupported target 'plan9-desktop'"));
        assert!(diagnostic.contains("windows-desktop"));
    }

    #[test]
    fn rejects_declared_but_blocked_targets() {
        let matrix = CompatibilityMatrix::parse(
            r#"
            [[targets]]
            name = "supported-target"
            family = "linux"
            os_version = "Ubuntu 26.04 or newer"
            surface = "desktop"
            architecture = "x86_64"
            windowing = "wayland"
            accessibility = "native"
            packaging = "desktop-bundle"
            release = "supported"
            ci_coverage = true

            [[targets]]
            name = "blocked-target"
            family = "linux"
            os_version = "Ubuntu 26.04 or newer"
            surface = "desktop"
            architecture = "x86_64"
            windowing = "wayland"
            accessibility = "native"
            packaging = "desktop-bundle"
            release = "blocked"
            ci_coverage = false
            "#,
        )
        .expect("compatibility matrix parses");

        assert!(matrix.contains_target("supported-target"));
        assert!(!matrix.contains_target("blocked-target"));
        let diagnostic = matrix
            .unsupported_target_diagnostic("blocked-target")
            .expect("blocked target should return diagnostic");
        assert!(diagnostic.contains("declared as blocked"));
        assert!(diagnostic.contains("supported-target"));
        assert!(!diagnostic.contains("blocked-target. Supported"));
        assert_eq!(
            matrix
                .unsupported_target_shared_diagnostic("blocked-target")
                .expect("blocked target should produce shared diagnostic")
                .rule
                .as_str(),
            "compat.target.unsupported"
        );
    }
}
