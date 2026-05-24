use hawk2ui_framework_conformance::{FrameworkConformanceHarness, FrameworkKind};

#[test]
fn framework_conformance_outputs_equivalent_records_for_lifecycle_state_events_refs_keyed_children_styles_and_assets()
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
        assert_eq!(snapshot.state_updates(), ["state:items"]);
    }
}

#[test]
fn framework_conformance_diagnostics_point_to_author_source_files() {
    let report = FrameworkConformanceHarness::new().run_diagnostic_matrix();

    assert_eq!(report.diagnostics().len(), 4);
    for diagnostic in report.diagnostics() {
        assert!(diagnostic.author_file().starts_with("src/Broken"));
        assert!(diagnostic.rule().contains("asset.path-invalid"));
    }
}

#[test]
fn framework_conformance_workspace_filter_marker() {
    assert_eq!(
        hawk2ui_framework_conformance::crate_name(),
        "hawk2ui-framework-conformance"
    );
}
