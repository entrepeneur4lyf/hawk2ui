#![forbid(unsafe_code)]
//! Machine-readable compatibility matrices and unsupported-target diagnostics for `Hawk2UI`.

pub mod matrix;

pub use matrix::{
    CompatibilityMatrix, MatrixError, ReleaseStatus, SurfaceKind, TargetCompatibility,
};

#[cfg(test)]
mod tests {
    use super::*;

    const MATRIX: &str = include_str!("../../../compatibility/matrix.toml");

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
            surface = "desktop"
            architecture = "x86_64"
            windowing = "wayland"
            accessibility = "native"
            packaging = "desktop-bundle"
            release = "supported"

            [[targets]]
            name = "linux-wayland-desktop"
            family = "linux"
            surface = "desktop"
            architecture = "x86_64"
            windowing = "wayland"
            accessibility = "native"
            packaging = "desktop-bundle"
            release = "supported"
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
}
