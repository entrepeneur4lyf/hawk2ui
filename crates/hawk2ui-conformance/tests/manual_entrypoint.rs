use std::{fs, path::PathBuf};

fn workspace_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn read_workspace_file(path: &str) -> String {
    fs::read_to_string(workspace_path(path)).unwrap_or_else(|error| {
        panic!("required manual entrypoint `{path}` must be readable: {error}")
    })
}

#[test]
fn manual_entrypoint_covers_required_product_domains() {
    let manual = read_workspace_file("manual/README.md");

    for heading in [
        "# Hawk2UI Manual",
        "## Desktop Applications",
        "## Plugin Editors",
        "## Style System",
        "## Runtime APIs",
        "## Packaging",
        "## Troubleshooting",
    ] {
        assert!(manual.contains(heading), "manual must contain `{heading}`");
    }
}
