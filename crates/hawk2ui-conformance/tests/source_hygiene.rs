use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("conformance crate lives under crates/hawk2ui-conformance")
        .to_path_buf()
}

fn read_source(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| {
        panic!(
            "production source `{}` must be readable: {error}",
            path.display()
        )
    })
}

fn collect_rust_sources(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|error| {
        panic!(
            "source directory `{}` must be readable: {error}",
            dir.display()
        )
    }) {
        let path = entry
            .unwrap_or_else(|error| panic!("source directory entry must be readable: {error}"))
            .path();

        if path.is_dir() {
            collect_rust_sources(&path, files);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            files.push(path);
        }
    }
}

fn collect_workspace_production_sources(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(root.join("crates"))
        .unwrap_or_else(|error| panic!("crates directory must be readable: {error}"))
    {
        let crate_dir = entry
            .unwrap_or_else(|error| panic!("crate directory entry must be readable: {error}"))
            .path();
        let src_dir = crate_dir.join("src");
        if src_dir.is_dir() {
            collect_rust_sources(&src_dir, &mut files);
        }
    }
    collect_rust_sources(&root.join("xtask/src"), &mut files);
    files
}

fn production_source(source: &str) -> &str {
    source
        .find("\n#[cfg(test)]")
        .map_or(source, |test_module_start| &source[..test_module_start])
}

#[test]
fn production_source_does_not_use_panic_style_fallible_assumptions() {
    let root = workspace_root();
    for source_path in collect_workspace_production_sources(&root) {
        let source = read_source(&source_path);
        let production_source = production_source(&source);

        for forbidden in [
            ".expect(",
            ".unwrap(",
            "panic!(",
            "todo!(",
            "unimplemented!(",
        ] {
            assert!(
                !production_source.contains(forbidden),
                "`{}` must not contain `{forbidden}` in production code",
                source_path.display()
            );
        }
    }
}
