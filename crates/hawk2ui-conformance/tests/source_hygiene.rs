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

fn collect_crate_production_sources(root: &Path, crate_name: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let src_dir = root.join("crates").join(crate_name).join("src");
    if src_dir.is_dir() {
        collect_rust_sources(&src_dir, &mut files);
    }
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

fn panic_style_forbidden_tokens() -> &'static [&'static str] {
    &[
        ".expect(",
        ".unwrap(",
        ".unwrap_err(",
        "panic!(",
        "unreachable!(",
        "assert!(",
        "assert_eq!(",
        "assert_ne!(",
        "debug_assert!(",
        "debug_assert_eq!(",
        "debug_assert_ne!(",
        "todo!(",
        "unimplemented!(",
    ]
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

        for forbidden in panic_style_forbidden_tokens() {
            assert!(
                !production_source.contains(forbidden),
                "`{}` must not contain `{forbidden}` in production code",
                source_path.display()
            );
        }
    }
}

#[test]
fn panic_style_gate_covers_assertion_macro_variants() {
    for token in [
        "assert!(",
        "assert_eq!(",
        "assert_ne!(",
        "debug_assert!(",
        "debug_assert_eq!(",
        "debug_assert_ne!(",
    ] {
        assert!(
            panic_style_forbidden_tokens().contains(&token),
            "panic-style gate must reject {token}"
        );
    }
}

#[test]
fn truce_editor_crate_never_captures_a_param_store() {
    // Decision 0003 D4 (Lock 3): the truce editor reads parameters ONLY through
    // the non-advancing `EditorBridge`, never by capturing truce's typed param
    // store — a captured store exposes a `FloatParam` whose advancing `read()`
    // could perturb the audio thread from a GUI repaint. Rust can't assert "this
    // struct lacks a field of type X", so this is a source-pattern gate, the same
    // enforcement class as the panic-style check above and the `unsafe_code`
    // boundary. The two patterns reach the store: capturing it (the param
    // accessor call) or storing it (the trait-object field).
    let root = workspace_root();
    let sources = collect_crate_production_sources(&root, "hawk2ui-plugin-truce");
    assert!(
        !sources.is_empty(),
        "hawk2ui-plugin-truce/src must contain production sources to scan"
    );
    for source_path in sources {
        let source = read_source(&source_path);
        let production_source = production_source(&source);
        for forbidden in [".params()", "dyn Params"] {
            assert!(
                !production_source.contains(forbidden),
                "`{}` must not contain `{forbidden}`: the truce editor must read parameters only through the non-advancing EditorBridge, never a captured truce param store (Decision 0003 D4)",
                source_path.display()
            );
        }
    }
}

#[test]
fn the_param_store_gate_detects_a_capture_outside_test_modules() {
    // Guard against a vacuous gate: a capture in production source is caught,
    // while the same capture inside a stripped test module is not.
    assert!(production_source("let store = context.params();").contains(".params()"));
    assert!(
        !production_source("\n#[cfg(test)]\nmod tests {\n    let _ = context.params();\n}\n")
            .contains(".params()")
    );
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
