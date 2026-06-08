use std::fmt::Write as _;
use std::path::PathBuf;

use hawk2ui_build::{
    JsBundleError, PackageManagerKind, PackageManagerMetadata, SealedJsDependencyOrigin,
    SealedJsModule, SealedJsModuleGraph, SealedJsSourceMap,
};

#[test]
fn js_bundle_validates_entrypoint_modules_imports_hashes_and_metadata() {
    let entry_source = "import('./counter.js'); export const title = 'Hawk';";
    let counter_source = "export const count = 1;";
    let package_manager = package_manager_metadata();
    let graph = SealedJsModuleGraph::new("app:///src/main.js", package_manager.clone())
        .with_module(
            SealedJsModule::new(
                "app:///src/main.js",
                entry_source,
                sha256_hex(entry_source.as_bytes()),
            )
            .with_static_import("hawk:runtime")
            .with_dynamic_import("app:///src/counter.js")
            .with_dependency_origin(SealedJsDependencyOrigin::workspace("src/main.tsx"))
            .with_source_map(SealedJsSourceMap::inline("{}"))
            .with_chunk("entry"),
        )
        .with_module(
            SealedJsModule::new(
                "app:///src/counter.js",
                counter_source,
                sha256_hex(counter_source.as_bytes()),
            )
            .with_dependency_origin(SealedJsDependencyOrigin::package(
                "@hawk2ui/example-counter",
                Some("1.0.0"),
            ))
            .with_chunk("counter"),
        );

    graph.validate().expect("sealed JS module graph validates");

    assert_eq!(graph.entrypoint(), "app:///src/main.js");
    assert_eq!(graph.package_manager(), &package_manager);
    assert_eq!(graph.modules().len(), 2);
    assert_eq!(
        graph
            .module("app:///src/main.js")
            .expect("entry module exists")
            .dynamic_imports(),
        ["app:///src/counter.js"]
    );
    let entry_module = graph
        .module("app:///src/main.js")
        .expect("entry module exists");
    assert_eq!(
        entry_module.dependency_origin(),
        &SealedJsDependencyOrigin::workspace("src/main.tsx")
    );
    assert_eq!(
        entry_module
            .source_map()
            .expect("source map exists")
            .sha256(),
        sha256_hex(b"{}")
    );
    assert_eq!(
        graph
            .module("app:///src/counter.js")
            .expect("counter module exists")
            .dependency_origin(),
        &SealedJsDependencyOrigin::package("@hawk2ui/example-counter", Some("1.0.0"))
    );
}

#[test]
fn js_bundle_rejects_missing_dynamic_import_target() {
    let source = "import('./missing.js');";
    let graph = SealedJsModuleGraph::new("app:///src/main.js", package_manager_metadata())
        .with_module(
            SealedJsModule::new("app:///src/main.js", source, sha256_hex(source.as_bytes()))
                .with_dynamic_import("app:///src/missing.js"),
        );

    let error = graph
        .validate()
        .expect_err("dynamic imports must point at sealed graph modules");

    assert_rule(&error, "build.js-bundle.dynamic-import-missing");
    assert!(error.message().contains("app:///src/missing.js"));
}

#[test]
fn js_bundle_rejects_missing_static_import_target_except_host_modules() {
    let source = "import './missing.js'; import { network } from 'hawk:runtime';";
    let graph = SealedJsModuleGraph::new("app:///src/main.js", package_manager_metadata())
        .with_module(
            SealedJsModule::new("app:///src/main.js", source, sha256_hex(source.as_bytes()))
                .with_static_import("app:///src/missing.js")
                .with_static_import("hawk:runtime"),
        );

    let error = graph
        .validate()
        .expect_err("static imports must point at sealed graph modules or host modules");

    assert_rule(&error, "build.js-bundle.static-import-missing");
    assert!(error.message().contains("app:///src/missing.js"));
    assert!(!error.message().contains("hawk:runtime"));
}

#[test]
fn js_bundle_rejects_hash_mismatch() {
    let graph = SealedJsModuleGraph::new("app:///src/main.js", package_manager_metadata())
        .with_module(SealedJsModule::new(
            "app:///src/main.js",
            "export const title = 'Hawk';",
            "not-a-real-hash",
        ));

    let error = graph
        .validate()
        .expect_err("module hashes must match sealed source");

    assert_rule(&error, "build.js-bundle.hash-mismatch");
    assert!(error.message().contains("app:///src/main.js"));
}

#[test]
fn js_bundle_rejects_missing_entrypoint() {
    let graph = SealedJsModuleGraph::new("app:///src/main.js", package_manager_metadata())
        .with_module(SealedJsModule::new(
            "app:///src/other.js",
            "export const other = true;",
            sha256_hex(b"export const other = true;"),
        ));

    let error = graph
        .validate()
        .expect_err("entrypoint must exist in graph modules");

    assert_rule(&error, "build.js-bundle.entrypoint-missing");
    assert!(error.message().contains("app:///src/main.js"));
}

fn package_manager_metadata() -> PackageManagerMetadata {
    PackageManagerMetadata::new(
        PackageManagerKind::Bun,
        Some(PathBuf::from("bun.lock")),
        Some("lock-hash".to_owned()),
    )
}

fn assert_rule(error: &JsBundleError, expected: &str) {
    assert_eq!(error.rule(), expected);
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};

    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}
