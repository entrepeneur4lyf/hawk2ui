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
