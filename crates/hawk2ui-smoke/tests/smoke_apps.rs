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

#[test]
fn plugin_synth_editor_exercises_editor_parameters_automation_state_and_destroy() {
    let fixture =
        SmokeFixture::from_workspace("examples/plugin-synth-editor", SmokeTargetKind::Plugin);
    let runner = SmokeRunner::default();

    let result = runner
        .run_plugin_synth_editor(&fixture)
        .expect("plugin synth editor smoke fixture should run");

    assert_eq!(result.fixture_name, "plugin-synth-editor");
    assert_eq!(
        result.editor_events,
        vec!["created", "attached", "resized", "dpi", "destroyed"]
    );
    assert_eq!(
        result.parameter_updates,
        vec!["osc.mix=0.4", "filter.cutoff=0.8"]
    );
    assert_eq!(
        result.automation_events,
        vec![
            "begin:filter.cutoff",
            "change:filter.cutoff",
            "end:filter.cutoff"
        ]
    );
    assert!(result.state_roundtrip);
    assert_eq!(result.preset_id, "factory.bright-pad");
    assert!(!result.requested_process_quit);
}

#[test]
fn plugin_meter_analyzer_proves_non_blocking_transport_and_ui_consumption() {
    let fixture =
        SmokeFixture::from_workspace("examples/plugin-meter-analyzer", SmokeTargetKind::Plugin);
    let runner = SmokeRunner::default();

    let result = runner
        .run_plugin_meter_analyzer(&fixture)
        .expect("realtime visual fixture should run");

    assert_eq!(
        result.channels,
        vec!["meter", "analyzer", "scope", "modulation"]
    );
    assert_eq!(result.audio_writes, 5);
    assert_eq!(result.ui_frames_consumed, 4);
    assert_eq!(result.dropped_frames, 1);
    assert_eq!(result.blocking_waits, 0);
    assert_eq!(result.allocations_on_audio_thread, 0);
}

#[test]
fn style_gallery_exports_deterministic_snapshots_for_all_sections() {
    let fixture = SmokeFixture::from_workspace("examples/style-gallery", SmokeTargetKind::Desktop);
    let runner = SmokeRunner::default();

    let result = runner
        .run_style_gallery(&fixture)
        .expect("style gallery should run");

    assert_eq!(
        result.sections,
        vec![
            "typography",
            "color",
            "borders",
            "radii",
            "shadows",
            "transforms",
            "opacity",
            "overflow",
            "transitions",
            "tokens",
            "image-layers",
            "vector-layers",
            "custom-draw"
        ]
    );
    assert_eq!(result.snapshot_count, 13);
    assert!(result.deterministic);
}

#[test]
fn security_denials_fail_before_runtime_surface_launch() {
    let fixture =
        SmokeFixture::from_workspace("examples/security-denials", SmokeTargetKind::Desktop);
    let runner = SmokeRunner::default();

    let result = runner
        .run_security_denials(&fixture)
        .expect("security denials should be evaluated");

    assert_eq!(
        result.denials,
        vec![
            "filesystem.undeclared",
            "network.denied",
            "clipboard.denied",
            "secret.redacted",
            "asset.unsafe",
            "style.unsupported",
            "manifest.invalid",
        ]
    );
    assert!(!result.runtime_surface_launched);
}

#[test]
fn framework_examples_cover_all_public_framework_fixtures() {
    let runner = SmokeRunner::default();

    let result = runner
        .run_framework_examples()
        .expect("framework examples should be validated");

    assert_eq!(
        result.frameworks,
        vec!["native", "svelte", "react", "vue", "solid"]
    );
    assert_eq!(result.package_entrypoints, 5);
    assert_eq!(result.asset_references, 5);
    assert!(result.conformance_equivalent);
    assert_eq!(result.contracts.len(), 5);
    for contract in &result.contracts {
        assert_eq!(contract.root_id, "root");
        assert_eq!(contract.keyed_children, vec!["title", "cta"]);
        assert_eq!(contract.style_refs, vec!["surface.card"]);
        assert_eq!(contract.asset_paths, vec!["assets/logo.svg"]);
    }
    assert!(
        result
            .contracts
            .iter()
            .all(|contract| contract.runtime_bridged)
    );
}
