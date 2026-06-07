use std::{fs, path::PathBuf};

use hawk2ui_api::{ApiInventory, ApiModule, ApiTypeEntry, ApiTypeStatus};
use hawk2ui_assets::{AssetBackend, AssetHash, AssetLimits};
use hawk2ui_build::{HawkManifest, PackageTarget};
use hawk2ui_cli::CommandCatalog;
use hawk2ui_compat::{
    CompatibilityMatrix, GraphicsCompatibilityMatrix, HostCompatibilityMatrix,
    PackageCompatibilityMatrix,
};
use hawk2ui_script::{HostCallPolicy, ScriptBackend, ScriptModule, TimerPolicy};
use hawk2ui_security::{SourceValidationPolicy, SourceValidationRecord, SourceValidationRule};
use hawk2ui_style::{PropertyId, PropertyRegistry, compile_style_source};

fn workspace_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn read_workspace_file(path: &str) -> String {
    fs::read_to_string(workspace_path(path))
        .unwrap_or_else(|error| panic!("required source-of-truth file `{path}`: {error}"))
}

fn manual(path: &str) -> String {
    read_workspace_file(path)
}

#[test]
fn readme_states_non_negotiable_production_baseline() {
    let readme = manual("README.md");

    for required in [
        "The baseline is stable, production-ready, and feature-complete.",
        "No MVP scope, candidate backend, partial framework compiler, placeholder runtime path, or deferred compatibility target satisfies the baseline.",
        "Any missing feature, unsupported production platform, stub, TODO, or untested integration is a release blocker until code, tests, manual coverage, and release evidence prove it complete.",
    ] {
        assert!(
            readme.contains(required),
            "README must state non-negotiable production baseline sentence: {required}"
        );
    }

    for stale_reference in [
        "docs/specs/",
        "docs/decisions/",
        "docs/technical/",
        "current limitations",
    ] {
        assert!(
            !readme.contains(stale_reference),
            "README must not point users at removed planning docs or soften baseline with `{stale_reference}`"
        );
    }
}

#[test]
fn readme_states_desktop_and_baseview_wayland_gpu_backends_are_remediated() {
    let readme = manual("README.md");

    for required in [
        "Desktop Wayland GPU backend",
        "Remediated",
        "HAWK2UI_NATIVE_WAYLAND_GPU_SMOKE=1",
        "Baseview native Wayland plugin embedding",
        "vendored Baseview adapter now accepts native Wayland parent handles",
        "reports GL creation failures as hard diagnostics",
        "CLAP Wayland parent ABI",
    ] {
        assert!(
            readme.contains(required),
            "README must preserve GPU/Wayland production status evidence: {required}"
        );
    }

    for stale in [
        "baseview-truce` remains X11-only",
        "DAW-owned plugin editor embedding on native Wayland still requires",
        "Baseview plugin editor embedding on native Wayland remains blocked",
    ] {
        assert!(
            !readme.contains(stale),
            "README must not preserve stale Wayland blocker wording: {stale}"
        );
    }
}

fn read_workspace_bytes(path: &str) -> Vec<u8> {
    fs::read(workspace_path(path))
        .unwrap_or_else(|error| panic!("required source-of-truth file `{path}`: {error}"))
}

fn assert_rejection_record(
    record: &SourceValidationRecord,
    expected_rule: SourceValidationRule,
    expected_diagnostic: &str,
    expected_path: &str,
) {
    assert_eq!(record.rule, expected_rule);
    assert_eq!(record.path, expected_path);
    assert_eq!(record.diagnostic.rule, expected_diagnostic);
    assert_eq!(
        record.diagnostic_label(),
        format!("source.{expected_diagnostic}:{expected_path}")
    );
}

fn assert_script_fixture_validator_rejects(script_fixture: &str) {
    let script_error = ScriptBackend::new(HostCallPolicy::deny_all(), TimerPolicy::deterministic())
        .execute_module(ScriptModule::for_source_path(
            script_fixture,
            read_workspace_file(script_fixture),
        ))
        .expect_err("unsupported script fixture must be rejected by the script backend");
    assert_eq!(script_error.diagnostic().rule(), "script.eval.failed");
}

fn assert_security_fixture_validators_reject_adversarial_inputs() {
    let style_fixture = "fixtures/security/unsupported-style.css";
    let style_error = compile_style_source(&read_workspace_file(style_fixture))
        .expect_err("unsupported style fixture must be rejected by the style compiler");
    assert!(
        style_error
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.rule() == "style.property.unknown"),
        "unsupported style fixture must emit style.property.unknown"
    );
    assert_rejection_record(
        &SourceValidationPolicy::reject(
            SourceValidationRule::UnsupportedStyleSyntax,
            style_fixture,
        ),
        SourceValidationRule::UnsupportedStyleSyntax,
        "style.unsupported",
        style_fixture,
    );

    let vector_fixture = "fixtures/security/unsafe-vector.svg";
    let vector_bytes = read_workspace_bytes(vector_fixture);
    let vector_error = AssetBackend::new(AssetLimits::default())
        .compile_vector(
            "unsafe-vector",
            vector_fixture,
            &vector_bytes,
            &AssetHash::sha256_bytes(&vector_bytes),
        )
        .expect_err("unsafe vector fixture must be rejected by the asset backend");
    assert_eq!(
        vector_error.diagnostic().rule(),
        "asset.vector.unsafe-content"
    );
    assert_rejection_record(
        &SourceValidationPolicy::reject(SourceValidationRule::UnsafeVectorContent, vector_fixture),
        SourceValidationRule::UnsafeVectorContent,
        "asset.vector.unsafe",
        vector_fixture,
    );

    let oversized_fixture = "fixtures/security/oversized-asset.manifest";
    let oversized_bytes = vec![0_u8; 16];
    let oversized_error = AssetBackend::new(AssetLimits::default().with_max_bytes(8))
        .compile_image(
            "oversized",
            oversized_fixture,
            &oversized_bytes,
            &AssetHash::sha256_bytes(&oversized_bytes),
        )
        .expect_err("oversized asset fixture must be rejected by the asset backend");
    assert_eq!(
        oversized_error.diagnostic().rule(),
        "asset.limit.bytes-exceeded"
    );

    let hash_fixture = "fixtures/security/hash-mismatch.manifest";
    let hash_bytes = read_workspace_bytes(hash_fixture);
    let hash_error = AssetBackend::new(AssetLimits::default())
        .compile_image(
            "hash-mismatch",
            hash_fixture,
            &hash_bytes,
            &AssetHash::new("sha256:bad"),
        )
        .expect_err("hash mismatch fixture must be rejected before decode");
    assert_eq!(hash_error.diagnostic().rule(), "asset.hash.mismatch");

    let missing_fixture = "fixtures/security/missing-asset.manifest";
    assert!(
        !workspace_path("fixtures/security/does-not-exist.png").is_file(),
        "missing asset fixture must point at an absent asset"
    );
    assert_rejection_record(
        &SourceValidationPolicy::reject(SourceValidationRule::MissingAsset, missing_fixture),
        SourceValidationRule::MissingAsset,
        "asset.missing",
        missing_fixture,
    );

    let malformed_fixture = "fixtures/security/malformed-manifest.toml";
    assert!(
        HawkManifest::parse(&read_workspace_file(malformed_fixture)).is_err(),
        "malformed manifest fixture must be rejected by manifest parsing"
    );
    assert_rejection_record(
        &SourceValidationPolicy::reject(SourceValidationRule::MalformedManifest, malformed_fixture),
        SourceValidationRule::MalformedManifest,
        "manifest.malformed",
        malformed_fixture,
    );

    let script_fixture = "fixtures/security/unsupported-script.ts";
    assert_script_fixture_validator_rejects(script_fixture);
    assert_rejection_record(
        &SourceValidationPolicy::reject(
            SourceValidationRule::UnsupportedScriptSyntax,
            script_fixture,
        ),
        SourceValidationRule::UnsupportedScriptSyntax,
        "script.unsupported",
        script_fixture,
    );
}

fn assert_security_manual_documents_release_and_evidence_boundaries(security: &str) {
    for security_claim in [
        "HAWK2UI_RELEASE_SIGNING_KEY_ID",
        "HAWK2UI_RELEASE_SIGNING_KEY_HEX",
        "HAWK2UI_TRUSTED_RELEASE_KEYS",
        "build-release",
        "package-plugin",
        "verify-artifact",
        "evidence vocabulary",
        "concrete validators",
        "hawk2ui-build",
        "hawk2ui-assets",
        "hawk2ui-script",
        "hawk2ui-platform",
        "hawk2ui-security-model",
    ] {
        assert!(
            security.contains(security_claim),
            "security manual missing release/security claim {security_claim}"
        );
    }
}

#[test]
fn manual_links_summary_links_resolve() {
    let summary = manual("manual/SUMMARY.md");

    for required in [
        "getting-started.md",
        "desktop-apps.md",
        "plugin-editors.md",
        "style-reference.md",
        "layout-reference.md",
        "rendering-reference.md",
        "runtime-apis.md",
        "packaging.md",
        "security.md",
        "troubleshooting.md",
        "examples.md",
    ] {
        assert!(summary.contains(required), "summary missing {required}");
        assert!(
            workspace_path(&format!("manual/{required}")).is_file(),
            "summary link target missing: manual/{required}"
        );
    }
}

#[test]
fn manual_desktop_commands_document_implemented_cli_commands() {
    let catalog = CommandCatalog;
    let combined = [
        manual("manual/getting-started.md"),
        manual("manual/desktop-apps.md"),
        manual("manual/packaging.md"),
        manual("manual/troubleshooting.md"),
    ]
    .join("\n");

    for command in [
        "new",
        "validate",
        "build-dev",
        "build-release",
        "verify-artifact",
        "run-desktop",
        "package-plugin",
        "diagnostics",
    ] {
        catalog.parse(["hawk2ui", command]).unwrap_or_else(|error| {
            panic!("test command `{command}` is not implemented: {error:?}")
        });
        assert!(
            combined.contains(&format!("hawk2ui {command}")),
            "manuals must document implemented CLI command hawk2ui {command}"
        );
    }
}

#[test]
fn manual_plugin_examples_reference_code_backed_plugin_fixtures() {
    let guide = manual("manual/plugin-editors.md");

    for path in [
        "examples/plugin-basic/hawk.json",
        "examples/plugin-synth-editor/hawk.json",
        "examples/plugin-meter-analyzer/hawk.json",
    ] {
        let manifest = HawkManifest::parse(&read_workspace_file(path))
            .unwrap_or_else(|error| panic!("plugin fixture `{path}` must parse: {error:?}"));
        assert!(
            manifest.has_target(PackageTarget::Plugin),
            "{path} must remain a plugin fixture"
        );
        assert!(
            guide.contains(path),
            "plugin guide missing fixture path {path}"
        );
        for parameter in &manifest.parameters {
            assert!(
                guide.contains(&parameter.id),
                "plugin guide missing parameter id {} from {path}",
                parameter.id
            );
            assert!(
                parameter.param_id.is_some(),
                "plugin fixture `{path}` parameter `{}` must pin param_id for stable automation/state",
                parameter.id
            );
        }
    }
}

#[test]
fn manual_reference_links_cover_style_layout_and_rendering_source_truth() {
    let style = manual("manual/style-reference.md");
    let layout = manual("manual/layout-reference.md");
    let rendering = manual("manual/rendering-reference.md");
    let registry = PropertyRegistry::production();

    for property in [
        "display",
        "font-size",
        "color",
        "border-width",
        "border-radius",
        "box-shadow",
        "transform",
        "opacity",
        "overflow",
        "--accent-color",
        "transition-duration",
        "background-color",
    ] {
        assert!(
            registry.metadata(&PropertyId::new(property)).is_some(),
            "test property must exist in production registry: {property}"
        );
        assert!(style.contains(property), "style manual missing {property}");
    }

    for selector_rule in [
        "selector.combinator.unsupported",
        "selector.state.unsupported",
        "selector.attribute.unsupported",
        "selector.list.unsupported",
        "style.shorthand.unsupported",
        "style.unit.unsupported",
        "style.function.unsupported",
        "style.keyframes.unsupported",
        "style.at-rule.unsupported",
    ] {
        assert!(
            style.contains(selector_rule),
            "style manual missing selector diagnostic {selector_rule}"
        );
    }

    for style_term in [
        "Supported Units",
        "Supported Functions",
        "Rejected CSS",
        "manual/css-subset-reference.md",
    ] {
        assert!(
            style.contains(style_term),
            "style manual missing {style_term}"
        );
    }

    let css_subset = manual("manual/css-subset-reference.md");
    for required_term in [
        "Selectors",
        "Properties",
        "Units",
        "Functions",
        "Tokens",
        "Inheritance",
        "Shorthands",
        "Transitions",
        "Keyframes",
        "Diagnostics",
    ] {
        assert!(
            css_subset.contains(required_term),
            "CSS subset reference missing {required_term}"
        );
    }

    for layout_term in [
        "LayoutTree",
        "LayoutStyle",
        "FlexDirection",
        "PluginEditorConstraints",
        "SceneGeometryAttachment",
        "TextMeasureInput",
    ] {
        assert!(
            layout.contains(layout_term),
            "layout manual missing {layout_term}"
        );
    }

    for render_term in [
        "SceneGraph",
        "PaintCommandList",
        "LayerStack",
        "RendererBackend",
        "CustomDrawSurface",
        "BackendDiagnostic",
    ] {
        assert!(
            rendering.contains(render_term),
            "rendering manual missing {render_term}"
        );
    }
}

#[test]
fn manual_runtime_security_and_packaging_match_machine_readable_gates() {
    let runtime = manual("manual/runtime-apis.md");
    let security = manual("manual/security.md");
    let packaging = manual("manual/packaging.md");

    let inventory = ApiInventory::production_baseline();
    for module in inventory.root_modules() {
        let module_name = match module {
            ApiModule::Artifact => "Artifact",
            ApiModule::Diagnostic => "Diagnostic",
            ApiModule::Inventory => "Inventory",
            ApiModule::Plugin => "Plugin",
            ApiModule::Runtime => "Runtime",
            ApiModule::Surface => "Surface",
        };
        assert!(
            runtime.contains(module_name) || packaging.contains(module_name),
            "manuals missing API module {module_name}"
        );
    }

    for public_type in inventory
        .types()
        .iter()
        .filter(|entry| entry.status() == ApiTypeStatus::Public)
        .map(ApiTypeEntry::name)
    {
        assert!(
            runtime.contains(public_type)
                || packaging.contains(public_type)
                || manual("manual/plugin-editors.md").contains(public_type),
            "manuals missing public API type {public_type}"
        );
    }

    let os_matrix = CompatibilityMatrix::parse(&read_workspace_file("compatibility/matrix.toml"))
        .expect("compatibility matrix parses");
    for target in &os_matrix.targets {
        assert!(
            packaging.contains(&target.name),
            "packaging manual missing compatibility target {}",
            target.name
        );
    }

    let package_matrix =
        PackageCompatibilityMatrix::parse(&read_workspace_file("compatibility/packages.toml"))
            .expect("package matrix parses");
    for package in &package_matrix.packages {
        assert!(
            packaging.contains(&package.output),
            "packaging manual missing package output {}",
            package.output
        );
        assert!(
            packaging.contains(&package.verify_command),
            "packaging manual missing verify command for {}",
            package.output
        );
    }

    let graphics_matrix =
        GraphicsCompatibilityMatrix::parse(&read_workspace_file("compatibility/graphics.toml"))
            .expect("graphics matrix parses");
    for backend in &graphics_matrix.backends {
        assert!(
            manual("manual/rendering-reference.md").contains(&backend.backend),
            "rendering manual missing graphics backend {}",
            backend.backend
        );
    }

    let host_matrix =
        HostCompatibilityMatrix::parse(&read_workspace_file("compatibility/hosts.toml"))
            .expect("host matrix parses");
    for host in &host_matrix.hosts {
        assert!(
            manual("manual/plugin-editors.md").contains(&host.format),
            "plugin manual missing host format {}",
            host.format
        );
    }

    assert_security_manual_documents_release_and_evidence_boundaries(&security);

    for security_fixture in [
        "fixtures/security/unsupported-style.css",
        "fixtures/security/unsupported-script.ts",
        "fixtures/security/unsafe-vector.svg",
        "fixtures/security/oversized-asset.manifest",
        "fixtures/security/hash-mismatch.manifest",
        "fixtures/security/missing-asset.manifest",
        "fixtures/security/malformed-manifest.toml",
    ] {
        assert!(
            security.contains(security_fixture),
            "security manual missing fixture {security_fixture}"
        );
    }
    assert_security_fixture_validators_reject_adversarial_inputs();
}

#[test]
fn manual_examples_index_tracks_repository_examples() {
    let examples = manual("manual/examples.md");

    for path in [
        "examples/desktop-basic/hawk.json",
        "examples/desktop-dashboard/hawk.json",
        "examples/plugin-basic/hawk.json",
        "examples/plugin-synth-editor/hawk.json",
        "examples/plugin-meter-analyzer/hawk.json",
        "examples/style-gallery/hawk.json",
        "examples/security-denials/hawk.json",
        "examples/frameworks/svelte-basic/hawk.json",
        "examples/frameworks/react-basic/hawk.json",
        "examples/frameworks/vue-basic/hawk.json",
        "examples/frameworks/solid-basic/hawk.json",
        "examples/frameworks/native-basic/hawk.json",
    ] {
        assert!(
            workspace_path(path).is_file(),
            "example fixture missing: {path}"
        );
        let manifest = HawkManifest::parse(&read_workspace_file(path))
            .unwrap_or_else(|error| panic!("example manifest `{path}` must parse: {error:?}"));
        let entry_path = workspace_path(path)
            .parent()
            .expect("example manifest path should have a parent")
            .join(manifest.source.entry);
        assert!(
            entry_path.is_file(),
            "example manifest `{path}` declares missing source entry `{}`",
            entry_path.display()
        );
        assert!(examples.contains(path), "examples manual missing {path}");
    }
}
