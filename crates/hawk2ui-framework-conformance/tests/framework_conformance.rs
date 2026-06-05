use hawk2ui_framework_conformance::{FrameworkConformanceHarness, FrameworkKind};

#[test]
fn framework_conformance_outputs_equivalent_records_for_lifecycle_events_refs_keyed_children_styles_and_assets()
 {
    let report = FrameworkConformanceHarness::new()
        .run_all()
        .expect("all framework integrations should compile conformance fixture");

    assert_eq!(report.snapshots().len(), 5);
    assert!(
        report.is_equivalent(),
        "snapshots must normalize to the same native record contract"
    );
    assert_eq!(
        report.frameworks(),
        [
            FrameworkKind::Native,
            FrameworkKind::Svelte,
            FrameworkKind::React,
            FrameworkKind::Vue,
            FrameworkKind::Solid,
        ]
    );

    for snapshot in report.snapshots() {
        assert_eq!(snapshot.root_id(), "root");
        assert_eq!(snapshot.keyed_children(), ["title", "cta"]);
        assert_eq!(snapshot.refs(), ["root_ref"]);
        assert_eq!(snapshot.style_refs(), ["surface.card"]);
        assert_eq!(snapshot.asset_paths(), ["assets/logo.svg"]);
        assert_eq!(
            snapshot.event_keys(),
            ["pointer.press", "lifecycle.mounted", "lifecycle.unmounted"]
        );
    }
}

#[test]
fn framework_conformance_diagnostics_point_to_author_source_files() {
    let report = FrameworkConformanceHarness::new()
        .run_diagnostic_matrix()
        .expect("invalid framework fixtures should produce diagnostics");

    assert_eq!(report.diagnostics().len(), 4);
    for diagnostic in report.diagnostics() {
        assert!(diagnostic.author_file().starts_with("src/Broken"));
        assert!(diagnostic.rule().contains("compiler-artifact.required"));
    }
}

#[test]
fn framework_conformance_runtime_bridge_renders_visible_pixels_for_reference_frameworks() {
    let report = FrameworkConformanceHarness::new()
        .run_runtime_matrix()
        .expect("all framework reference fixtures should render through runtime and Skia");

    assert_eq!(
        report.frameworks(),
        [
            FrameworkKind::Native,
            FrameworkKind::Svelte,
            FrameworkKind::React,
            FrameworkKind::Vue,
            FrameworkKind::Solid,
        ]
    );
    for evidence in report.evidence() {
        assert_eq!(evidence.root_id(), "root");
        assert_eq!(evidence.child_ids(), ["title", "cta"]);
        assert!(evidence.frames_presented() > 0);
        assert!(evidence.changed_pixels() > 0);
        assert!(
            evidence
                .operation_keys()
                .contains(&"mount-element:root".to_string())
        );
        assert!(
            evidence
                .operation_keys()
                .contains(&"bind-event:root:pointer.press".to_string())
        );
    }
}

#[test]
fn framework_conformance_failure_matrix_rejects_invalid_contracts() {
    let report = FrameworkConformanceHarness::new()
        .run_failure_matrix()
        .expect("invalid contract fixtures should produce structured failures");

    assert!(report.has_failure(
        FrameworkKind::Native,
        "duplicate-keyed-child",
        "native.child-key.duplicate"
    ));
    assert!(report.has_failure(
        FrameworkKind::Svelte,
        "invalid-asset-path",
        "native.asset.path-invalid"
    ));
    assert!(report.has_failure(
        FrameworkKind::Svelte,
        "invalid-layout-number",
        "svelte.runtime-bridge.failed"
    ));
    assert!(report.has_failure(
        FrameworkKind::Svelte,
        "unsupported-event",
        "svelte.compiler-artifact.required"
    ));
    assert!(report.has_failure(
        FrameworkKind::React,
        "invalid-asset-path",
        "native.asset.path-invalid"
    ));
    assert!(report.has_failure(
        FrameworkKind::React,
        "unsupported-event",
        "react.compiler-artifact.required"
    ));
    assert!(report.has_failure(
        FrameworkKind::React,
        "invalid-layout-number",
        "react.runtime-bridge.failed"
    ));
    assert!(report.has_failure(
        FrameworkKind::Vue,
        "invalid-asset-path",
        "native.asset.path-invalid"
    ));
    assert!(report.has_failure(
        FrameworkKind::Vue,
        "unsupported-event",
        "vue.compiler-artifact.required"
    ));
    assert!(report.has_failure(
        FrameworkKind::Vue,
        "invalid-layout-number",
        "vue.runtime-bridge.failed"
    ));
    assert!(report.has_failure(
        FrameworkKind::Solid,
        "invalid-asset-path",
        "native.asset.path-invalid"
    ));
    assert!(report.has_failure(
        FrameworkKind::Solid,
        "unsupported-event",
        "solid.compiler-artifact.required"
    ));
    assert!(report.has_failure(
        FrameworkKind::Solid,
        "invalid-layout-number",
        "solid.runtime-bridge.failed"
    ));
}

#[test]
fn framework_conformance_workspace_filter_marker() {
    assert_eq!(
        hawk2ui_framework_conformance::crate_name(),
        "hawk2ui-framework-conformance"
    );
}
