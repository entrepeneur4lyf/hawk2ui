use std::fs;
use std::path::Path;

const PUBLIC_MODULES: &[(&str, &str)] = &[
    ("artifact", "src/artifact.rs"),
    ("diagnostic", "src/diagnostic.rs"),
    ("inventory", "src/inventory.rs"),
    ("plugin", "src/plugin.rs"),
    ("runtime", "src/runtime.rs"),
    ("surface", "src/surface.rs"),
];

#[test]
fn docs_public_modules_have_module_level_stability_sections() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    for (module, path) in PUBLIC_MODULES {
        let source = fs::read_to_string(manifest_dir.join(path)).expect("module source");
        assert!(
            source.contains("//! ## Stability"),
            "{module} module is missing a module-level stability section"
        );
    }
}
