use hawk2ui_smoke::{SmokeFixture, SmokeRunner, SmokeTargetKind};

#[test]
fn desktop_basic_builds_verifies_exports_scene_first_frame_and_window_lifecycle() {
    let fixture = SmokeFixture::from_workspace("examples/desktop-basic", SmokeTargetKind::Desktop);
    let runner = SmokeRunner::default();

    let result = runner
        .run_desktop_basic(&fixture)
        .expect("desktop smoke app should run");

    assert_eq!(result.fixture_name, "desktop-basic");
    assert!(result.build.artifact_verified);
    assert_eq!(result.scene.root_id, "desktop-basic-root");
    assert_eq!(result.first_frame.frame_id, 1);
    assert_eq!(result.first_frame.snapshot_id, "desktop-basic:first-frame");
    assert_eq!(
        result.window_events,
        vec!["created", "focused", "repainted", "closed"]
    );
}

#[test]
fn desktop_dashboard_exercises_layout_style_snapshot_focus_pointer_and_resize() {
    let fixture =
        SmokeFixture::from_workspace("examples/desktop-dashboard", SmokeTargetKind::Desktop);
    let runner = SmokeRunner::default();

    let result = runner
        .run_desktop_dashboard(&fixture)
        .expect("dashboard smoke app should run");

    assert_eq!(result.fixture_name, "desktop-dashboard");
    assert_eq!(result.layout_nodes, 18);
    assert_eq!(result.style_rules, 12);
    assert_eq!(result.visual_snapshot_id, "desktop-dashboard:visual");
    assert_eq!(
        result.keyboard_focus_path,
        vec!["root", "sidebar", "bypass-button"]
    );
    assert_eq!(
        result.pointer_events,
        vec!["pointer-down:graph", "pointer-up:graph"]
    );
    assert_eq!(result.resize_events, vec!["1280x720@1", "1440x900@1.5"]);
}
