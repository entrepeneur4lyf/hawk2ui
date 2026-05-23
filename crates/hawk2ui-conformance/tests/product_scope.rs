use std::{fs, path::PathBuf};

use hawk2ui_build::{HawkManifest, PackageTarget};

fn workspace_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn read_workspace_file(path: &str) -> String {
    fs::read_to_string(workspace_path(path)).unwrap_or_else(|error| {
        panic!("required conformance fixture `{path}` must be readable: {error}")
    })
}

fn assert_required_sections(input: &str, sections: &[&str]) {
    for section in sections {
        assert!(
            input.contains(section),
            "manifest fixture must contain `{section}`"
        );
    }
}

#[test]
fn product_scope_desktop_manifest_declares_native_window_target() {
    let input = read_workspace_file("examples/desktop-basic/manifest.hawk.toml");

    assert_required_sections(&input, &["[identity]", "[source]", "[capabilities]", "[[targets]]"]);

    let manifest = HawkManifest::parse(&input).expect("desktop manifest must parse");

    assert_eq!(manifest.identity.id, "com.hawk2ui.examples.desktop-basic");
    assert!(manifest.has_capability("native-windowing"));
    assert!(manifest.has_capability("sealed-artifacts"));
    assert!(manifest.has_target(PackageTarget::Desktop));
}

#[test]
fn product_scope_plugin_manifest_declares_editor_and_parameters() {
    let input = read_workspace_file("examples/plugin-basic/manifest.hawk.toml");

    assert_required_sections(
        &input,
        &[
            "[identity]",
            "[source]",
            "[capabilities]",
            "[[targets]]",
            "[plugin]",
            "[editor]",
            "[[parameters]]",
        ],
    );

    let manifest = HawkManifest::parse(&input).expect("plugin manifest must parse");

    assert_eq!(manifest.identity.id, "com.hawk2ui.examples.plugin-basic");
    assert!(manifest.has_capability("plugin-editor"));
    assert!(manifest.has_capability("sealed-artifacts"));
    assert!(manifest.has_target(PackageTarget::Plugin));
    assert_eq!(manifest.parameters.len(), 2);
}
