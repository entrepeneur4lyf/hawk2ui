use hawk2ui_testkit::fixtures::{FixtureCatalog, FixtureCatalogError, TempProject};
use hawk2ui_testkit::{FixtureKind, TestFixture};
use std::io;

#[test]
fn fixture_helpers_temp_project_writes_files_and_cleans_up_on_drop() {
    let project_root;

    {
        let project = TempProject::new("fixture-cleanup").expect("temp project");
        project_root = project.root().to_path_buf();

        project
            .write_file("src/App.hawk", "component App {}")
            .expect("write source fixture");

        assert_eq!(
            project
                .read_to_string("src/App.hawk")
                .expect("read source fixture"),
            "component App {}"
        );
        assert!(project.root().join("src/App.hawk").is_file());
    }

    assert!(
        !project_root.exists(),
        "temp project root must be removed on drop"
    );
}

#[test]
fn fixture_helpers_temp_project_rejects_paths_that_escape_root() {
    let project = TempProject::new("fixture-escape").expect("temp project");

    let error = project
        .write_file("../escaped.txt", "nope")
        .expect_err("parent traversal must fail");

    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn fixture_helpers_catalog_resolves_required_fixtures_by_kind() {
    let catalog = FixtureCatalog::new([
        TestFixture::new("desktop", "examples/desktop-basic", FixtureKind::Manifest),
        TestFixture::new("visual-card", "fixtures/visual/card", FixtureKind::Visual),
    ]);

    let visual = catalog
        .require_kind("visual-card", FixtureKind::Visual)
        .expect("visual fixture");

    assert_eq!(visual.name(), "visual-card");
    assert_eq!(visual.path(), "fixtures/visual/card");
}

#[test]
fn fixture_helpers_catalog_rejects_kind_mismatches() {
    let catalog = FixtureCatalog::new([TestFixture::new(
        "desktop",
        "examples/desktop-basic",
        FixtureKind::Manifest,
    )]);

    assert_eq!(
        catalog.require_kind("desktop", FixtureKind::Visual),
        Err(FixtureCatalogError::KindMismatch {
            name: "desktop".to_string(),
            expected: FixtureKind::Visual,
            actual: FixtureKind::Manifest,
        })
    );
}
