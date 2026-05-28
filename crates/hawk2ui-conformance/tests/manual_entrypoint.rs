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

fn section_body<'manual>(manual: &'manual str, heading: &str) -> &'manual str {
    let start = manual
        .find(heading)
        .unwrap_or_else(|| panic!("manual must contain `{heading}`"))
        + heading.len();
    let tail = &manual[start..];
    tail.find("\n## ")
        .map_or(tail.trim(), |next_heading| tail[..next_heading].trim())
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
        if heading.starts_with("## ") {
            let body = section_body(&manual, heading);
            assert!(
                body.contains("](") && body.len() >= 48,
                "manual section `{heading}` must contain explanatory copy and link to concrete user-facing manual pages"
            );
        }
    }
}
