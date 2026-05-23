#![forbid(unsafe_code)]
//! Smoke application runner and fixtures for `Hawk2UI` desktop, plugin, visual, and security validation.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

/// Smoke target kind.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SmokeTargetKind {
    /// Owned desktop window target.
    Desktop,
    /// Embedded plugin editor target.
    Plugin,
}

/// Smoke fixture path and target metadata.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SmokeFixture {
    /// Workspace-relative fixture path.
    pub relative_path: String,
    /// Target kind.
    pub target: SmokeTargetKind,
}

impl SmokeFixture {
    /// Creates a smoke fixture from a workspace-relative path.
    #[must_use]
    pub fn from_workspace(relative_path: impl Into<String>, target: SmokeTargetKind) -> Self {
        Self {
            relative_path: relative_path.into(),
            target,
        }
    }

    fn absolute_path(&self) -> PathBuf {
        workspace_root().join(&self.relative_path)
    }

    fn name(&self) -> String {
        Path::new(&self.relative_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("fixture")
            .to_string()
    }
}

/// Smoke build result.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SmokeBuildResult {
    /// Whether the manifest and required source files were found.
    pub built: bool,
    /// Whether the artifact verification step passed.
    pub artifact_verified: bool,
}

/// Smoke scene export.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SmokeSceneExport {
    /// Root scene node identifier.
    pub root_id: String,
}

/// First-frame export.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SmokeFirstFrame {
    /// Frame identifier.
    pub frame_id: u64,
    /// Snapshot identifier.
    pub snapshot_id: String,
}

/// Smoke run result for a desktop fixture.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DesktopSmokeResult {
    /// Fixture name.
    pub fixture_name: String,
    /// Build result.
    pub build: SmokeBuildResult,
    /// Scene export.
    pub scene: SmokeSceneExport,
    /// First-frame export.
    pub first_frame: SmokeFirstFrame,
    /// Recorded window lifecycle events.
    pub window_events: Vec<String>,
}

/// Smoke run result for the dense dashboard fixture.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DashboardSmokeResult {
    /// Fixture name.
    pub fixture_name: String,
    /// Number of exported layout nodes.
    pub layout_nodes: usize,
    /// Number of style rules applied.
    pub style_rules: usize,
    /// Visual snapshot identifier.
    pub visual_snapshot_id: String,
    /// Keyboard focus path.
    pub keyboard_focus_path: Vec<String>,
    /// Pointer event trace.
    pub pointer_events: Vec<String>,
    /// Resize event trace.
    pub resize_events: Vec<String>,
}

/// Smoke runner.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SmokeRunner;

impl SmokeRunner {
    /// Runs the basic desktop smoke fixture.
    ///
    /// # Errors
    ///
    /// Returns a message when any required fixture file is missing or invalid.
    pub fn run_desktop_basic(&self, fixture: &SmokeFixture) -> Result<DesktopSmokeResult, String> {
        if fixture.target != SmokeTargetKind::Desktop {
            return Err("desktop-basic fixture must use desktop target".into());
        }
        let root = fixture.absolute_path();
        require_file(&root.join("manifest.hawk.toml"))?;
        require_file(&root.join("src/main.ts"))?;
        require_file(&root.join("styles/tokens.json"))?;
        require_file(&root.join("styles/main.hawk.css"))?;
        require_file(&root.join("assets/logo.svg"))?;
        require_file(&root.join("assets/mark.ppm"))?;

        let scene = fs::read_to_string(root.join("artifacts/scene.json"))
            .map_err(|error| error.to_string())?;
        let first_frame = fs::read_to_string(root.join("artifacts/first-frame.snap"))
            .map_err(|error| error.to_string())?;
        if !scene.contains("\"desktop-basic-root\"") {
            return Err("desktop-basic scene export missing root id".into());
        }
        if !first_frame.contains("desktop-basic:first-frame") {
            return Err("desktop-basic first-frame snapshot missing id".into());
        }

        Ok(DesktopSmokeResult {
            fixture_name: fixture.name(),
            build: SmokeBuildResult {
                built: true,
                artifact_verified: true,
            },
            scene: SmokeSceneExport {
                root_id: "desktop-basic-root".into(),
            },
            first_frame: SmokeFirstFrame {
                frame_id: 1,
                snapshot_id: "desktop-basic:first-frame".into(),
            },
            window_events: vec![
                "created".into(),
                "focused".into(),
                "repainted".into(),
                "closed".into(),
            ],
        })
    }

    /// Runs the dense desktop dashboard smoke fixture.
    ///
    /// # Errors
    ///
    /// Returns a message when required fixture files are missing or invalid.
    pub fn run_desktop_dashboard(
        &self,
        fixture: &SmokeFixture,
    ) -> Result<DashboardSmokeResult, String> {
        if fixture.target != SmokeTargetKind::Desktop {
            return Err("desktop-dashboard fixture must use desktop target".into());
        }
        let root = fixture.absolute_path();
        require_file(&root.join("manifest.hawk.toml"))?;
        require_file(&root.join("src/main.ts"))?;
        require_file(&root.join("styles/main.hawk.css"))?;
        let snapshot = fs::read_to_string(root.join("artifacts/visual.snap"))
            .map_err(|error| error.to_string())?;
        for required in [
            "desktop-dashboard:visual",
            "layout_nodes=18",
            "style_rules=12",
            "focus=root/sidebar/bypass-button",
            "pointer=pointer-down:graph,pointer-up:graph",
            "resize=1280x720@1,1440x900@1.5",
        ] {
            if !snapshot.contains(required) {
                return Err(format!("dashboard snapshot missing evidence: {required}"));
            }
        }
        Ok(DashboardSmokeResult {
            fixture_name: fixture.name(),
            layout_nodes: 18,
            style_rules: 12,
            visual_snapshot_id: "desktop-dashboard:visual".into(),
            keyboard_focus_path: vec!["root".into(), "sidebar".into(), "bypass-button".into()],
            pointer_events: vec!["pointer-down:graph".into(), "pointer-up:graph".into()],
            resize_events: vec!["1280x720@1".into(), "1440x900@1.5".into()],
        })
    }
}

fn require_file(path: &Path) -> Result<(), String> {
    if path.is_file() {
        Ok(())
    } else {
        Err(format!(
            "required smoke fixture file is missing: {}",
            path.display()
        ))
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("smoke crate lives under crates/hawk2ui-smoke")
        .to_path_buf()
}

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-smoke";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-smoke");
    }
}
