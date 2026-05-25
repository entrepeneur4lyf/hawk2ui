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

#[test]
fn docs_api_stability_policy_covers_all_public_modules() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let policy_path = workspace_root.join("docs/development/api-stability.md");
    let policy = fs::read_to_string(&policy_path).expect("api stability policy");

    for (module, _) in PUBLIC_MODULES {
        let heading = format!("### `{module}`");
        assert!(
            policy.contains(&heading),
            "api stability policy is missing {heading}"
        );
    }

    for required in [
        "Source Compatibility",
        "Artifact Compatibility",
        "Feature Flags",
        "Deprecation Windows",
        "Breaking-Change Process",
    ] {
        assert!(
            policy.contains(required),
            "api stability policy is missing {required}"
        );
    }
}
