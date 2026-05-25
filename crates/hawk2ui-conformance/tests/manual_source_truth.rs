use std::{fs, path::PathBuf};

use hawk2ui_api::{ApiInventory, ApiModule, ApiTypeStatus};
use hawk2ui_build::{HawkManifest, PackageTarget};
use hawk2ui_cli::CommandCatalog;
use hawk2ui_compat::{
    CompatibilityMatrix, GraphicsCompatibilityMatrix, HostCompatibilityMatrix,
    PackageCompatibilityMatrix,
};
use hawk2ui_style::{PropertyId, PropertyRegistry};

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
        "examples/plugin-basic/manifest.hawk.toml",
        "examples/plugin-synth-editor/manifest.hawk.toml",
        "examples/plugin-meter-analyzer/manifest.hawk.toml",
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
    ] {
        assert!(
            style.contains(selector_rule),
            "style manual missing selector diagnostic {selector_rule}"
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
        .map(|entry| entry.name())
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
}

#[test]
fn manual_examples_index_tracks_repository_examples() {
    let examples = manual("manual/examples.md");

    for path in [
        "examples/desktop-basic/manifest.hawk.toml",
        "examples/desktop-dashboard/manifest.hawk.toml",
        "examples/plugin-basic/manifest.hawk.toml",
        "examples/plugin-synth-editor/manifest.hawk.toml",
        "examples/plugin-meter-analyzer/manifest.hawk.toml",
        "examples/style-gallery/manifest.hawk.toml",
        "examples/security-denials/manifest.hawk.toml",
        "examples/frameworks/svelte-basic/manifest.hawk.toml",
        "examples/frameworks/react-basic/manifest.hawk.toml",
        "examples/frameworks/vue-basic/manifest.hawk.toml",
        "examples/frameworks/solid-basic/manifest.hawk.toml",
        "examples/frameworks/native-basic/manifest.hawk.toml",
    ] {
        assert!(
            workspace_path(path).is_file(),
            "example fixture missing: {path}"
        );
        assert!(examples.contains(path), "examples manual missing {path}");
    }
}
