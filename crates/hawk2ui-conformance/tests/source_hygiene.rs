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

fn production_source(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;

    while let Some(cfg_relative_start) = source[cursor..].find("\n#[cfg(test)]") {
        let cfg_start = cursor + cfg_relative_start;
        output.push_str(&source[cursor..cfg_start]);

        let attr_start = cfg_start + 1;
        let Some(mod_relative_start) = source[attr_start..].find("mod tests") else {
            cursor = attr_start;
            continue;
        };
        let mod_start = attr_start + mod_relative_start;
        let Some(open_relative_brace) = source[mod_start..].find('{') else {
            cursor = attr_start;
            continue;
        };
        let open_brace = mod_start + open_relative_brace;
        let Some(close_brace) = matching_brace(source, open_brace) else {
            cursor = attr_start;
            continue;
        };
        cursor = close_brace + 1;
    }

    output.push_str(&source[cursor..]);
    output
}

fn matching_brace(source: &str, open_brace: usize) -> Option<usize> {
    let mut depth = 0_usize;
    for (offset, byte) in source[open_brace..].bytes().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open_brace + offset);
                }
            }
            _ => {}
        }
    }
    None
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
            ".unwrap_err(",
            "panic!(",
            "unreachable!(",
            "assert!(",
            "debug_assert!(",
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

#[test]
fn production_source_strips_test_modules_without_dropping_later_production_code() {
    let source = r#"
pub fn before() {}

#[cfg(test)]
mod tests {
    #[test]
    fn test_only() {
        panic!("test panic is outside production");
    }
}

pub fn after() {
    panic!("production panic remains visible");
}
"#;

    let production = production_source(source);

    assert!(production.contains("pub fn before()"));
    assert!(production.contains("pub fn after()"));
    assert!(!production.contains("fn test_only()"));
    assert!(production.contains("panic!(\"production panic remains visible\")"));
}
