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
fn readme_states_evidence_based_production_release_gate() {
    let readme = manual("README.md");

    for required in [
        "The release baseline is evidence-based: no MVP scope, candidate backend, partial framework compiler, placeholder runtime path, deferred compatibility target, hidden stub, TODO, or untested integration satisfies release readiness.",
        "A production release is blocked until every release-gated desktop, plugin, framework, rendering, platform, packaging, security, performance, smoke, conformance, manual, and release-evidence path is implemented in code and passing its verification command.",
        "Subsystems may be usable before the full release gate passes, but README status must not describe the whole framework as production-ready or feature-complete until the release evidence proves that claim.",
        "Windows, macOS, Linux Wayland, and Linux X11 are mandatory production release targets, not optional follow-up work.",
        "A public release announcement is blocked until native Windows, macOS, Linux Wayland, and Linux X11 desktop/plugin-host paths have passing runtime, packaging, manual, and release-evidence coverage.",
    ] {
        assert!(
            readme.contains(required),
            "README must state evidence-based production release gate sentence: {required}"
        );
    }

    assert!(
        !readme.contains("The baseline is stable, production-ready, and feature-complete."),
        "README must not overclaim whole-framework production readiness before release evidence passes"
    );

    for stale_reference in [
        "docs/specs/",
        "docs/decisions/",
        "docs/technical/",
        "current limitations",
        "`boa_engine` + `oxc` for JavaScript/TypeScript",
        "Windows and macOS are mandatory production release targets, not optional follow-up work.",
    ] {
        assert!(
            !readme.contains(stale_reference),
            "README must not point users at removed planning docs or soften baseline with `{stale_reference}`"
        );
    }

    assert!(
        readme.contains("`hawk2ui-js-runtime` Deno/V8 for React-first JavaScript"),
        "README native stack must describe the production React-first Deno/V8 runtime"
    );
    assert!(
        readme.contains("packaged as CLAP/VST3/AU."),
        "README plugin target claim must list the release-backed plugin package formats"
    );
    assert!(
        !readme.contains("CLAP/VST3/AU/standalone"),
        "README must not advertise standalone plugin packaging as release-backed"
    );
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
fn manual_desktop_release_platforms_match_matrix_scope() {
    let required =
        "Windows, macOS, Linux Wayland, and Linux X11 are required production release platforms.";

    for manual_path in ["manual/desktop-apps.md", "manual/packaging.md"] {
        let content = manual(manual_path);
        assert!(
            content.contains(required),
            "{manual_path} must name every desktop release platform from the compatibility matrix"
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
fn react_plugin_example_declares_release_formats_and_audio_dsp_capability_usage() {
    let manifest = manual("examples/react-plugin-basic/hawk.json");
    let source = manual("examples/react-plugin-basic/src/App.tsx");

    for required in [
        "\"clap\"",
        "\"vst3\"",
        "\"au\"",
        "\"build\"",
        "\"output\": \"dist/main.js\"",
    ] {
        assert!(
            manifest.contains(required),
            "React plugin example manifest missing release fixture evidence: {required}"
        );
    }

    for required in [
        "from \"hawk:plugin\"",
        "from \"hawk:audio\"",
        "from \"hawk:dsp\"",
        "beginAutomationGesture",
        "subscribeMeters",
        "sendControl",
    ] {
        assert!(
            source.contains(required),
            "React plugin example source missing plugin/audio/DSP operation evidence: {required}"
        );
    }
}

#[test]
fn react_desktop_example_declares_all_release_desktop_platforms_and_capabilities() {
    let manifest = manual("examples/react-desktop-basic/hawk.json");
    let source = manual("examples/react-desktop-basic/src/App.tsx");

    for required in [
        "\"windows\"",
        "\"macos\"",
        "\"linux-wayland\"",
        "\"linux-x11\"",
        "\"build\"",
        "\"output\": \"dist/main.js\"",
    ] {
        assert!(
            manifest.contains(required),
            "React desktop example manifest missing release fixture evidence: {required}"
        );
    }

    for required in [
        "from \"hawk:network\"",
        "from \"hawk:storage\"",
        "from \"hawk:files\"",
        "request(",
        "setItem",
        "pickFile",
    ] {
        assert!(
            source.contains(required),
            "React desktop example source missing capability operation evidence: {required}"
        );
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
fn manual_packaging_documents_sealed_js_module_graph_release_metadata() {
    let packaging = manual("manual/packaging.md");

    for required in [
        "Sealed JS module graph metadata records every module specifier, content hash, source-map hash, dependency origin, static import, dynamic import, chunk membership, package manager, lockfile hash, and graph entrypoint.",
        "`SealedJsDependencyOrigin` records whether a module came from workspace build output, an installed package dependency, or generated build tooling.",
    ] {
        assert!(
            packaging.contains(required),
            "packaging manual missing sealed JS release metadata claim: {required}"
        );
    }
}

#[test]
fn plugin_package_command_docs_include_au_release_target() {
    let getting_started = manual("manual/getting-started.md");
    let plugin_editors = manual("manual/plugin-editors.md");
    let packaging = manual("manual/packaging.md");
    let release_checklist = read_workspace_file("release/checklist.md");

    assert!(
        getting_started.contains(
            "`hawk2ui package-plugin` is the plugin package command name for release-backed CLAP, VST3, and AU targets."
        ),
        "getting-started manual must document AU as a release-backed package-plugin target"
    );
    assert!(
        plugin_editors.contains(
            "`hawk2ui package-plugin` implements release-backed CLAP, VST3, and AU targets through the truce.audio-backed plugin layer."
        ),
        "plugin editors manual must document truce-backed CLAP/VST3/AU package-plugin support"
    );
    assert!(
        packaging.contains(
            "`hawk2ui package-plugin` materializes release-backed CLAP, VST3, and AU bundle layouts."
        ),
        "packaging manual must document AU as a release-backed package-plugin layout"
    );
    assert!(
        packaging.contains(
            "CLAP, VST3, and AU plugin packaging is backed by the truce.audio plugin layer and verified through `hawk2ui package-plugin` evidence."
        ),
        "packaging manual must document truce-backed plugin package support"
    );
    assert!(
        release_checklist.contains("release-backed CLAP/VST3/AU plugin bundles"),
        "release checklist must include AU in plugin bundle evidence"
    );
    assert!(
        release_checklist.contains("Windows, macOS, Linux Wayland, Linux X11"),
        "release checklist must preserve all required desktop platform evidence"
    );
    assert!(
        !plugin_editors.contains("- `standalone`"),
        "plugin editors manual must not advertise standalone as a release-backed host format"
    );
}

#[test]
fn plugin_support_source_truth_does_not_gate_truce_backed_formats() {
    let host_baseview = read_workspace_file("crates/hawk2ui-host-baseview/src/lib.rs");
    let plugin_editors = manual("manual/plugin-editors.md");
    let packaging = manual("manual/packaging.md");

    for source in [&host_baseview, &plugin_editors, &packaging] {
        assert!(
            !source.contains("Windows and macOS remain release-gated targets"),
            "truce-backed plugin support must not be described as gated behind missing Windows/macOS support"
        );
        assert!(
            !source.contains("On supported desktop build hosts"),
            "truce-backed plugin packaging must not be softened with host-support gating language"
        );
    }

    for required in [
        "truce.audio-backed plugin layer",
        "truce.audio plugin layer",
    ] {
        assert!(
            plugin_editors.contains(required) || packaging.contains(required),
            "public plugin docs must preserve truce-backed support claim: {required}"
        );
    }
}

#[test]
fn manual_getting_started_documents_react_developer_experience_commands() {
    let getting_started = manual("manual/getting-started.md");

    for command in [
        "hawk2ui init",
        "hawk2ui new",
        "hawk2ui dev",
        "hawk2ui build-dev",
        "hawk2ui build-release",
        "hawk2ui run-desktop",
        "hawk2ui package-desktop",
        "hawk2ui package-plugin",
        "hawk2ui verify-artifact",
        "hawk2ui diagnostics",
    ] {
        assert!(
            getting_started.contains(command),
            "getting started manual missing developer-experience command: {command}"
        );
    }
}

#[test]
fn manual_runtime_framework_claims_match_react_first_deno_release_scope() {
    let runtime = manual("manual/runtime-apis.md");

    for required in [
        "React 19+ production support uses `@hawk2ui/react` `createRoot` with the sealed Deno runtime.",
        "React emits Hawk2UI scene operations through `hawk2ui-js-runtime`, not `FrameworkNativeProgram` or the legacy source-string compiler path.",
        "Vue, Solid, and Svelte are incubating framework adapters.",
    ] {
        assert!(
            runtime.contains(required),
            "runtime manual missing React-first Deno source-truth claim: {required}"
        );
    }

    assert!(
        !runtime
            .contains("Svelte 5, React 19+, Vue 3.5+, and Solid adapters all accept this boundary"),
        "runtime manual must not present React as part of the legacy framework compiler boundary"
    );
}

#[test]
fn manual_project_manifest_describes_react_sealed_js_graph_release_path() {
    let manifest = manual("manual/project-manifest.md");

    for required in [
        "`app` declares the authoring entrypoint and framework used to produce package-manager build output.",
        "React release builds consume a sealed JavaScript module graph produced from the selected package-manager build output.",
        "Vue, Solid, and Svelte manifest values remain incubating until their runtime renderer adapters have equivalent release evidence.",
        "\"entry\": \"src/App.tsx\"",
        "\"framework\": \"react\"",
        "\"formats\": [\"clap\", \"vst3\", \"au\"]",
        "Supported `formats`: `clap`, `vst3`, and `au`.",
    ] {
        assert!(
            manifest.contains(required),
            "project manifest manual missing React-first manifest claim: {required}"
        );
    }

    for forbidden in [
        "Framework compilers are authoring frontends selected by the manifest; the runtime consumes compiled Hawk artifacts, not framework source directly.",
        "Authoring entrypoint, framework compiler, and optional style/script entries.",
        "A framework entry produces a compiled framework artifact; a `native` entry produces a compiled script artifact.",
        "\"entry\": \"src/App.svelte\"",
        "\"formats\": [\"clap\", \"vst3\", \"au\", \"standalone\"]",
        "- `standalone`",
    ] {
        assert!(
            !manifest.contains(forbidden),
            "project manifest manual must not preserve legacy framework compiler release wording: {forbidden}"
        );
    }
}

#[test]
fn manual_project_manifest_documents_react_build_output_and_package_manager_fields() {
    let manifest = manual("manual/project-manifest.md");

    for required in [
        "## `build`",
        "Package-manager-produced JavaScript output path, package-manager selection, and lockfile detection.",
        "`output` is the package-manager-produced JavaScript bundle path sealed into the release artifact for React builds.",
        "`packageManager` accepts `bun`, `npm`, `pnpm`, or `yarn` and selects the lockfile when more than one supported lockfile is present.",
        "If `packageManager` is omitted, release builds detect `bun.lock`, `package-lock.json`, `pnpm-lock.yaml`, or `yarn.lock`.",
    ] {
        assert!(
            manifest.contains(required),
            "project manifest manual missing React build field documentation: {required}"
        );
    }
}

#[test]
fn manual_project_manifest_examples_include_all_release_desktop_platforms() {
    let manifest = manual("manual/project-manifest.md");

    for required in [
        "\"platforms\": [\"windows\", \"macos\", \"linux-wayland\", \"linux-x11\"]",
        "Desktop target `platforms` must include Windows, macOS, Linux Wayland, and Linux X11 before a production release claim.",
    ] {
        assert!(
            manifest.contains(required),
            "project manifest manual missing release desktop platform documentation: {required}"
        );
    }
}

#[test]
fn manual_runtime_documents_hawk_js_capability_imports() {
    let runtime = manual("manual/runtime-apis.md");

    for module in [
        "hawk:runtime",
        "hawk:network",
        "hawk:api",
        "hawk:storage",
        "hawk:secrets",
        "hawk:files",
        "hawk:desktop",
        "hawk:plugin",
        "hawk:audio",
        "hawk:dsp",
        "hawk:ai",
    ] {
        assert!(
            runtime.contains(module),
            "runtime manual missing Hawk JS capability module {module}"
        );
    }

    for required in [
        "The Hawk JS API is default-deny",
        "Capability denials include the manifest path",
        "No raw filesystem, network, shell, environment, or secret access is exposed to JavaScript.",
        "`js-runtime.module.unsupported-hawk-import`",
    ] {
        assert!(
            runtime.contains(required),
            "runtime manual missing Hawk JS capability boundary: {required}"
        );
    }
}

#[test]
fn manual_runtime_documents_remaining_hawk_js_capability_operations() {
    let runtime = manual("manual/runtime-apis.md");

    for required in [
        "`hawk:network` exposes these network operations: `request`.",
        "`hawk:api` exposes these declared endpoint operations: `call`.",
        "`hawk:storage` exposes these persistent storage and scoped JSON document/database operations: `getItem`, `setItem`, `getDocument`, `putDocument`, `transaction`, and `migrate`.",
        "`hawk:secrets` exposes these secret handle operations: `read`, `isSecretHandle`, and `serializeSecretOptions`.",
        "`hawk:files` exposes these picker-granted file operations: `readText`, `writeText`, `readBytes`, `writeBytes`, `pickFile`, `pickFolder`, `watch`, `importFile`, and `exportFile`.",
        "`hawk:ai` exposes these declared provider operations: `callProvider` and `streamProvider`.",
        "`hawk:runtime` re-exports `network`, `api`, `storage`, `secrets`, `files`, `desktop`, `plugin`, `audio`, `dsp`, and `ai`.",
    ] {
        assert!(
            runtime.contains(required),
            "runtime manual missing Hawk JS operation sentence: {required}"
        );
    }
}

#[test]
fn manual_runtime_documents_desktop_capability_operations() {
    let runtime = manual("manual/runtime-apis.md");

    for operation in [
        "setWindowTitle",
        "showOpenDialog",
        "readClipboard",
        "writeClipboard",
        "notify",
        "registerShortcut",
        "openExternal",
        "onDeepLink",
        "setWindowMode",
        "closeWindow",
    ] {
        assert!(
            runtime.contains(operation),
            "runtime manual missing hawk:desktop operation {operation}"
        );
    }
}

#[test]
fn manual_runtime_documents_plugin_audio_dsp_capability_operations() {
    let runtime = manual("manual/runtime-apis.md");

    for operation in [
        "readParameter",
        "writeParameter",
        "beginAutomationGesture",
        "endAutomationGesture",
        "loadState",
        "saveState",
        "loadPreset",
        "savePreset",
        "getTransport",
        "resizeEditor",
        "focusEditor",
    ] {
        assert!(
            runtime.contains(operation),
            "runtime manual missing hawk:plugin operation {operation}"
        );
    }

    for operation in ["subscribeMeters", "transport", "nextControl"] {
        assert!(
            runtime.contains(operation),
            "runtime manual missing hawk:audio operation {operation}"
        );
    }

    for operation in [
        "sendControl",
        "updateParameterGraph",
        "startAnalysisJob",
        "cancelAnalysisJob",
        "startOfflineRender",
        "exportOfflineRender",
    ] {
        assert!(
            runtime.contains(operation),
            "runtime manual missing hawk:dsp operation {operation}"
        );
    }
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
        "examples/react-desktop-basic/hawk.json",
        "examples/react-plugin-basic/hawk.json",
        "examples/frameworks/svelte-basic/hawk.json",
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

#[test]
fn manual_examples_do_not_document_legacy_react_framework_compiler_fixture() {
    let examples = manual("manual/examples.md");

    assert!(
        examples.contains("## React First Runtime"),
        "examples manual must document the React-first runtime examples"
    );
    assert!(
        examples.contains("examples/react-desktop-basic/hawk.json"),
        "examples manual must include the React desktop runtime example"
    );
    assert!(
        examples.contains("examples/react-plugin-basic/hawk.json"),
        "examples manual must include the React plugin runtime example"
    );
    assert!(
        !examples.contains("examples/frameworks/react-basic/hawk.json"),
        "examples manual must not document the legacy React framework compiler fixture"
    );
}

#[test]
fn react_examples_demonstrate_production_runtime_patterns() {
    let desktop = read_workspace_file("examples/react-desktop-basic/src/App.tsx");
    for required in [
        "useState",
        "useEffect",
        "hawk:network",
        "hawk:storage",
        "hawk:files",
        ".map(",
        "<input",
        "role=",
        "ariaLabel=",
        "onInput",
    ] {
        assert!(
            desktop.contains(required),
            "React desktop example must demonstrate production app pattern `{required}`"
        );
    }

    let plugin = read_workspace_file("examples/react-plugin-basic/src/App.tsx");
    for required in [
        "hawk:plugin",
        "readParameter",
        "writeParameter",
        "beginAutomationGesture",
        "endAutomationGesture",
        "loadState",
        "saveState",
        "loadPreset",
        "savePreset",
        "getTransport",
        "hawk:audio",
        "subscribeMeters",
        "hawk:dsp",
        "sendControl",
    ] {
        assert!(
            plugin.contains(required),
            "React plugin example must demonstrate production plugin pattern `{required}`"
        );
    }
}
