use hawk2ui_build::{
    ArtifactSchemaVersion, ArtifactSignaturePolicy, ArtifactSignatureStatus, ArtifactSigningKey,
    HawkManifest, SealedArtifact,
};
use hawk2ui_cli::{
    CliCommand, CliExitCode, CliPackageManager, CliPresentationBackend, CliProjectTemplate,
    CommandCatalog, WorkspaceCommandRunner,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn cli_commands_help_lists_required_workflows() {
    let catalog = CommandCatalog;
    let help = catalog.render_help();

    for command in [
        "init",
        "new",
        "run",
        "dev",
        "validate",
        "build-dev",
        "build-release",
        "verify-artifact",
        "run-desktop",
        "package-desktop",
        "package-plugin",
        "export-schemas",
        "export-params",
        "pin-ids",
        "migrate-manifest",
        "diagnostics",
        "explain",
    ] {
        assert!(help.contains(command), "help missing command: {command}");
    }
    assert!(
        help.contains("CLAP, VST3, and AU"),
        "package-plugin help must advertise all release-backed plugin binary formats"
    );
    assert!(
        help.contains("react-app|react-plugin|vue-app|vue-plugin|native"),
        "init help must list every parsed project template"
    );
    for unsupported in ["standalone", "desktop bundle"] {
        assert!(
            !help.contains(unsupported),
            "package-plugin help must not advertise unsupported target: {unsupported}"
        );
    }
}

#[test]
fn cli_commands_parse_known_commands_and_reject_invalid_command() {
    let catalog = CommandCatalog;

    assert_eq!(
        catalog.parse(["hawk2ui", "new"]).unwrap(),
        CliCommand::NewProject {
            template: CliProjectTemplate::ReactApp,
            package_manager: CliPackageManager::Bun,
        }
    );
    assert_eq!(
        catalog.parse(["hawk2ui", "init"]).unwrap(),
        CliCommand::NewProject {
            template: CliProjectTemplate::ReactApp,
            package_manager: CliPackageManager::Bun,
        }
    );
    assert_eq!(catalog.parse(["hawk2ui", "run"]).unwrap(), CliCommand::Run);
    assert_eq!(catalog.parse(["hawk2ui", "dev"]).unwrap(), CliCommand::Dev);
    assert_eq!(
        catalog.parse(["hawk2ui", "build-release"]).unwrap(),
        CliCommand::BuildRelease
    );
    assert_eq!(
        catalog.parse(["hawk2ui", "export-schemas"]).unwrap(),
        CliCommand::ExportSchemas
    );
    assert_eq!(
        catalog.parse(["hawk2ui", "export-params"]).unwrap(),
        CliCommand::ExportParams
    );
    assert_eq!(
        catalog.parse(["hawk2ui", "pin-ids"]).unwrap(),
        CliCommand::PinIds
    );
    assert_eq!(
        catalog.parse(["hawk2ui", "migrate-manifest"]).unwrap(),
        CliCommand::MigrateManifest { force: false }
    );
    assert_eq!(
        catalog
            .parse(["hawk2ui", "migrate-manifest", "--force"])
            .unwrap(),
        CliCommand::MigrateManifest { force: true }
    );
    assert_eq!(
        catalog
            .parse([
                "hawk2ui",
                "verify-artifact",
                "target/hawk2ui/release/hawk2ui-artifact.hawk"
            ])
            .unwrap(),
        CliCommand::VerifyArtifact {
            path: Some("target/hawk2ui/release/hawk2ui-artifact.hawk".into())
        }
    );
    assert_eq!(
        catalog.parse(["hawk2ui", "explain"]).unwrap(),
        CliCommand::Explain
    );
    assert_eq!(
        catalog.parse(["hawk2ui", "run-desktop"]).unwrap(),
        CliCommand::RunDesktop {
            presentation_backend: CliPresentationBackend::Software,
        }
    );
    assert_eq!(
        catalog
            .parse([
                "hawk2ui",
                "run-desktop",
                "--presentation-backend",
                "gpu-required",
            ])
            .unwrap(),
        CliCommand::RunDesktop {
            presentation_backend: CliPresentationBackend::GpuRequired,
        }
    );
    assert_eq!(
        catalog
            .parse(["hawk2ui", "run-desktop", "--gpu-preferred"])
            .unwrap(),
        CliCommand::RunDesktop {
            presentation_backend: CliPresentationBackend::GpuPreferred,
        }
    );
    assert_eq!(
        catalog.parse(["hawk2ui", "package-desktop"]).unwrap(),
        CliCommand::PackageDesktop
    );

    let error = catalog
        .parse(["hawk2ui", "nope"])
        .expect_err("invalid command should fail");
    assert_eq!(error.exit_code, CliExitCode::Usage);
    assert!(error.message.contains("unknown command"));
}

#[test]
fn cli_commands_parse_init_template_and_package_manager_options() {
    let catalog = CommandCatalog;

    assert_eq!(
        catalog
            .parse([
                "hawk2ui",
                "init",
                "--template",
                "react-plugin",
                "--package-manager",
                "npm",
            ])
            .unwrap(),
        CliCommand::NewProject {
            template: CliProjectTemplate::ReactPlugin,
            package_manager: CliPackageManager::Npm,
        }
    );
    assert_eq!(
        catalog
            .parse([
                "hawk2ui",
                "init",
                "--template",
                "vue-app",
                "--package-manager",
                "pnpm",
            ])
            .unwrap(),
        CliCommand::NewProject {
            template: CliProjectTemplate::VueApp,
            package_manager: CliPackageManager::Pnpm,
        }
    );
    assert_eq!(
        catalog
            .parse([
                "hawk2ui",
                "new",
                "--template",
                "vue-plugin",
                "--package-manager",
                "yarn",
            ])
            .unwrap(),
        CliCommand::NewProject {
            template: CliProjectTemplate::VuePlugin,
            package_manager: CliPackageManager::Yarn,
        }
    );
}

#[test]
fn workspace_dev_runs_validated_hot_reload_loop_without_rust_commands() {
    let root = temp_cli_workspace("dev");
    write_desktop_project(&root, "com.hawk2ui.cli-dev", "CLI Dev");

    let execution = WorkspaceCommandRunner::new(&root).execute(CliCommand::Dev);

    assert_eq!(execution.exit_code, CliExitCode::Success);
    assert!(execution.stdout.contains("development loop ready"));
    assert!(execution.stdout.contains("IncrementalRebuildTriggered"));
    assert!(execution.stdout.contains("NativeSurfaceReloaded"));
}

#[test]
fn workspace_dev_watches_manifest_declared_sources_and_assets() {
    let root = temp_cli_workspace("dev-filesystem");
    write_file(
        &root.join("manifest.hawk.toml"),
        r#"
[identity]
id = "com.hawk2ui.cli-dev-filesystem"
name = "CLI Dev Filesystem"
version = "1.0.0"

[source]
entry = "src/main.ts"
style = "styles/main.hawk.css"
script = "src/bootstrap.ts"

[capabilities]
keys = ["native-windowing", "sealed-artifacts"]

[[targets]]
kind = "desktop"
name = "linux-wayland"

[[assets]]
id = "logo"
kind = "vector"
path = "assets/logo.svg"
"#,
    );
    write_file(&root.join("src/main.ts"), "export const app = 'desktop';");
    write_file(&root.join("src/bootstrap.ts"), "export const boot = true;");
    write_file(
        &root.join("styles/main.hawk.css"),
        ".root { display: flex; font-size: 18px; background-color: token(color.surface); }",
    );
    write_file(&root.join("assets/logo.svg"), "<svg />");

    let execution = WorkspaceCommandRunner::new(&root).execute(CliCommand::Dev);

    assert_eq!(
        execution.exit_code,
        CliExitCode::Success,
        "package-desktop should succeed\nstdout:\n{}\nstderr:\n{}\ndiagnostics:\n{:#?}",
        execution.stdout,
        execution.stderr,
        execution.diagnostics
    );
    assert!(execution.stdout.contains("src/main.ts"));
    assert!(execution.stdout.contains("src/bootstrap.ts"));
    assert!(execution.stdout.contains("styles/main.hawk.css"));
    assert!(execution.stdout.contains("assets/logo.svg"));
}

#[test]
fn workspace_new_project_creates_buildable_desktop_and_plugin_scaffold() {
    let root = temp_cli_workspace("new-project");

    let created = WorkspaceCommandRunner::new(&root).execute(CliCommand::NewProject {
        template: CliProjectTemplate::Native,
        package_manager: CliPackageManager::Bun,
    });

    assert_eq!(created.exit_code, CliExitCode::Success);
    for path in [
        "hawk.json",
        "src/main.ts",
        "src/bootstrap.ts",
        "styles/main.hawk.css",
        "assets/logo.svg",
        "README.md",
    ] {
        assert!(root.join(path).is_file(), "scaffold missing {path}");
    }

    let manifest =
        fs::read_to_string(root.join("hawk.json")).expect("generated manifest should be readable");
    assert!(manifest.contains("\"schemaVersion\": 1"));
    assert!(manifest.contains("\"desktop\""));
    assert!(manifest.contains("\"plugin\""));
    assert!(manifest.contains("\"parameters\""));
    assert!(manifest.contains("\"entries\""));

    let validate = WorkspaceCommandRunner::new(&root).execute(CliCommand::Validate);
    assert_eq!(validate.exit_code, CliExitCode::Success);

    let build = signed_runner(&root).execute(CliCommand::BuildRelease);
    assert_eq!(build.exit_code, CliExitCode::Success);
    assert!(build.stdout.contains("compiled-scripts: 2"));
    assert!(build.stdout.contains("compiled-styles: 1"));
    assert!(build.stdout.contains("compiled-assets: 1"));

    let package = signed_runner(&root).execute(CliCommand::PackagePlugin);
    assert_eq!(package.exit_code, CliExitCode::Success);
    assert!(
        package
            .stdout
            .contains("layout-verification-status: passed")
    );
}

#[test]
fn workspace_init_react_templates_generate_framework_manifests_and_package_metadata() {
    let app_root = temp_cli_workspace("init-react-app");
    let app_created = WorkspaceCommandRunner::new(&app_root).execute(CliCommand::NewProject {
        template: CliProjectTemplate::ReactApp,
        package_manager: CliPackageManager::Bun,
    });

    assert_eq!(app_created.exit_code, CliExitCode::Success);
    assert!(app_root.join("hawk.json").is_file());
    assert!(app_root.join("src/App.tsx").is_file());
    assert!(app_root.join("package.json").is_file());
    let app_manifest_source =
        fs::read_to_string(app_root.join("hawk.json")).expect("app manifest should read");
    let app_manifest =
        HawkManifest::parse(&app_manifest_source).expect("react app manifest should validate");
    assert_eq!(app_manifest.source.entry, "src/App.tsx");
    assert!(app_manifest_source.contains("\"framework\": \"react\""));
    assert!(app_manifest_source.contains("\"desktop\""));
    assert!(!app_manifest_source.contains("\"plugin\""));
    assert!(
        fs::read_to_string(app_root.join("package.json"))
            .expect("app package should read")
            .contains("\"packageManager\": \"bun@1.0.0\"")
    );

    let plugin_root = temp_cli_workspace("init-react-plugin");
    let plugin_created =
        WorkspaceCommandRunner::new(&plugin_root).execute(CliCommand::NewProject {
            template: CliProjectTemplate::ReactPlugin,
            package_manager: CliPackageManager::Npm,
        });

    assert_eq!(plugin_created.exit_code, CliExitCode::Success);
    let plugin_manifest_source =
        fs::read_to_string(plugin_root.join("hawk.json")).expect("plugin manifest should read");
    let plugin_manifest = HawkManifest::parse(&plugin_manifest_source)
        .expect("react plugin manifest should validate");
    assert_eq!(plugin_manifest.source.entry, "src/App.tsx");
    assert!(
        plugin_manifest_source.contains("\"framework\": \"react\""),
        "{plugin_manifest_source}"
    );
    assert!(plugin_manifest_source.contains("\"plugin\""));
    assert!(plugin_manifest_source.contains("\"parameters\""));
    assert!(
        fs::read_to_string(plugin_root.join("package.json"))
            .expect("plugin package should read")
            .contains("\"packageManager\": \"npm@10.0.0\"")
    );
    assert!(
        fs::read_to_string(plugin_root.join("src/App.tsx"))
            .expect("plugin source should read")
            .contains("hawk:plugin")
    );
}

#[test]
fn workspace_init_vue_templates_generate_framework_manifests_and_package_metadata() {
    let app_root = temp_cli_workspace("init-vue-app");
    let app_created = WorkspaceCommandRunner::new(&app_root).execute(CliCommand::NewProject {
        template: CliProjectTemplate::VueApp,
        package_manager: CliPackageManager::Bun,
    });

    assert_eq!(app_created.exit_code, CliExitCode::Success);
    assert!(app_root.join("hawk.json").is_file());
    assert!(app_root.join("src/main.ts").is_file());
    assert!(app_root.join("src/App.vue").is_file());
    assert!(app_root.join("vite.hawk.config.ts").is_file());
    assert!(app_root.join("package.json").is_file());
    let app_manifest_source =
        fs::read_to_string(app_root.join("hawk.json")).expect("app manifest should read");
    let app_manifest =
        HawkManifest::parse(&app_manifest_source).expect("vue app manifest should validate");
    assert_eq!(app_manifest.source.entry, "src/main.ts");
    assert!(app_manifest_source.contains("\"framework\": \"vue\""));
    assert!(app_manifest_source.contains("\"packageManager\": \"bun\""));
    assert!(app_manifest_source.contains("\"output\": \"dist/main.js\""));
    assert!(app_manifest_source.contains("\"desktop\""));
    assert!(!app_manifest_source.contains("\"plugin\""));
    assert!(
        fs::read_to_string(app_root.join("package.json"))
            .expect("app package should read")
            .contains("\"packageManager\": \"bun@1.0.0\"")
    );
    assert!(
        fs::read_to_string(app_root.join("src/main.ts"))
            .expect("app entry should read")
            .contains("createApp")
    );
    assert!(
        fs::read_to_string(app_root.join("src/App.vue"))
            .expect("app component should read")
            .contains("<template>")
    );
    assert!(
        fs::read_to_string(app_root.join("vite.hawk.config.ts"))
            .expect("vite config should read")
            .contains("fileName: () => \"main.js\"")
    );

    let plugin_root = temp_cli_workspace("init-vue-plugin");
    let plugin_created =
        WorkspaceCommandRunner::new(&plugin_root).execute(CliCommand::NewProject {
            template: CliProjectTemplate::VuePlugin,
            package_manager: CliPackageManager::Npm,
        });

    assert_eq!(plugin_created.exit_code, CliExitCode::Success);
    let plugin_manifest_source =
        fs::read_to_string(plugin_root.join("hawk.json")).expect("plugin manifest should read");
    let plugin_manifest =
        HawkManifest::parse(&plugin_manifest_source).expect("vue plugin manifest should validate");
    assert_eq!(plugin_manifest.source.entry, "src/main.ts");
    assert!(
        plugin_manifest_source.contains("\"framework\": \"vue\""),
        "{plugin_manifest_source}"
    );
    assert!(plugin_manifest_source.contains("\"packageManager\": \"npm\""));
    assert!(plugin_manifest_source.contains("\"output\": \"dist/main.js\""));
    assert!(plugin_manifest_source.contains("\"plugin\""));
    assert!(plugin_manifest_source.contains("\"parameters\""));
    assert!(
        fs::read_to_string(plugin_root.join("package.json"))
            .expect("plugin package should read")
            .contains("\"packageManager\": \"npm@10.0.0\"")
    );
    assert!(
        fs::read_to_string(plugin_root.join("src/App.vue"))
            .expect("plugin source should read")
            .contains("hawk:plugin")
    );
}

#[test]
fn workspace_migrate_manifest_writes_canonical_hawk_json() {
    let root = temp_cli_workspace("migrate-manifest");
    write_file(
        &root.join("manifest.hawk.toml"),
        r#"
[identity]
id = "com.hawk2ui.cli-migrate"
name = "CLI Migrate"
version = "1.0.0"

[source]
entry = "src/main.ts"

[capabilities]
keys = ["native-windowing"]

[[targets]]
kind = "desktop"
name = "standalone"

[plugin]
id = "com.hawk2ui.cli-migrate"
name = "CLI Migrate"

[editor]
width = 800
height = 480

[[parameters]]
id = "gain"
param_id = 11
name = "Gain"
default = 0.5
"#,
    );
    write_file(&root.join("src/main.ts"), "export const app = 'migrated';");

    let execution =
        WorkspaceCommandRunner::new(&root).execute(CliCommand::MigrateManifest { force: false });

    assert_eq!(execution.exit_code, CliExitCode::Success);
    assert!(
        execution
            .stdout
            .contains("migrated manifest.hawk.toml to hawk.json")
    );
    assert!(root.join("manifest.hawk.toml").is_file());
    let migrated = fs::read_to_string(root.join("hawk.json")).expect("hawk.json should be written");
    let value: serde_json::Value = serde_json::from_str(&migrated).expect("hawk.json is JSON");
    assert_eq!(value["schemaVersion"], 1);
    assert_eq!(value["package"]["id"], "com.hawk2ui.cli-migrate");
    assert_eq!(value["targets"]["desktop"][0]["name"], "standalone");
    assert_eq!(value["plugin"]["parameters"][0]["paramId"], 11);

    let validate = WorkspaceCommandRunner::new(&root).execute(CliCommand::Validate);
    assert_eq!(validate.exit_code, CliExitCode::Success);
    assert!(validate.stdout.contains("com.hawk2ui.cli-migrate"));
}

#[test]
fn workspace_explain_reports_targets_capabilities_and_next_commands() {
    let root = temp_cli_workspace("explain");
    write_desktop_project(&root, "com.hawk2ui.cli-explain", "CLI Explain");

    let execution = WorkspaceCommandRunner::new(&root).execute(CliCommand::Explain);

    assert_eq!(execution.exit_code, CliExitCode::Success);
    assert!(
        execution
            .stdout
            .contains("project: com.hawk2ui.cli-explain")
    );
    assert!(execution.stdout.contains("targets:"));
    assert!(execution.stdout.contains("linux-wayland (desktop)"));
    assert!(execution.stdout.contains("native-windowing"));
    assert!(execution.stdout.contains("hawk2ui run-desktop"));
}

#[test]
fn workspace_export_schemas_writes_central_schema_catalog() {
    let root = temp_cli_workspace("export-schemas");
    let execution = WorkspaceCommandRunner::new(&root).execute(CliCommand::ExportSchemas);
    let catalog: serde_json::Value =
        serde_json::from_str(&execution.stdout).expect("schema catalog stdout is JSON");
    let ids = catalog["schemas"]
        .as_array()
        .expect("schemas is an array")
        .iter()
        .filter_map(|entry| entry["id"].as_str())
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(execution.exit_code, CliExitCode::Success);
    assert_eq!(catalog["schema_version"], "1.0.0");
    assert!(ids.contains("hawk2ui.raw-manifest"));
    assert!(ids.contains("hawk2ui.capability-table"));
    assert!(ids.contains("hawk2ui.package-verification-report"));
}

#[test]
fn workspace_export_params_emits_truce_param_source() {
    let root = temp_cli_workspace("export-params");
    write_file(
        &root.join("manifest.hawk.toml"),
        r#"
[identity]
id = "com.hawk2ui.export-params"
name = "Export Params"
version = "0.1.0"

[source]
entry = "src/main.ts"

[plugin]
id = "com.hawk2ui.export-params"
name = "Export Params"

[[parameters]]
id = "filter.cutoff"
name = "Cutoff"
min = 20.0
max = 20000.0
default = 1000.0
unit = "Hz"

[[meters]]
id = "output.level"
name = "Output Level"
"#,
    );

    let execution = WorkspaceCommandRunner::new(&root).execute(CliCommand::ExportParams);

    assert_eq!(execution.exit_code, CliExitCode::Success);
    // Exact-match the whole emitted source: a substring check would pass on
    // malformed output that merely *contains* the right fragment. The manifest
    // declares one float parameter (Hz range/unit) and one meter, so the full
    // truce `#[derive(Params)]` struct — field, id, range, unit, and the
    // read-only `#[meter]` the editor consumes — must round-trip byte-for-byte.
    let expected = r#"// Generated by hawk2ui from the plugin manifest. Do not edit by hand.

use truce::prelude::*;

#[derive(Params)]
pub struct PluginParams {
    #[param(id = 0, name = "Cutoff", range = "linear(20, 20000)", unit = "Hz", default = 1000)]
    pub filter_cutoff: FloatParam,
    #[meter]
    pub output_level: MeterSlot,
}
"#;
    assert_eq!(execution.stdout, expected);
}

#[test]
fn workspace_export_params_emits_enum_param_source() {
    let root = temp_cli_workspace("export-params-enum");
    write_file(
        &root.join("manifest.hawk.toml"),
        r#"
[identity]
id = "com.hawk2ui.export-enum"
name = "Export Enum"
version = "0.1.0"

[source]
entry = "src/main.ts"

[plugin]
id = "com.hawk2ui.export-enum"
name = "Export Enum"

[[parameters]]
id = "osc.shape"
name = "Osc Shape"
kind = "enum"
default = 1.0

[[parameters.variants]]
id = "sine"
name = "Sine"

[[parameters.variants]]
id = "square-pulse"
name = "Square / Pulse"
"#,
    );

    let execution = WorkspaceCommandRunner::new(&root).execute(CliCommand::ExportParams);

    assert_eq!(execution.exit_code, CliExitCode::Success);
    // Exact-match the whole emitted source. The enum kind generates a
    // `#[derive(ParamEnum)]` type whose variants are PascalCased from the
    // variant ids, with a `#[name = ...]` override only where the display name
    // differs from the ident ("Square / Pulse" vs `SquarePulse`; "Sine" needs
    // none). The parameter field then references that generated type by name.
    let expected = r#"// Generated by hawk2ui from the plugin manifest. Do not edit by hand.

use truce::prelude::*;

#[derive(ParamEnum)]
pub enum PluginParamsOscShape {
    Sine,
    #[name = "Square / Pulse"]
    SquarePulse,
}

#[derive(Params)]
pub struct PluginParams {
    #[param(id = 0, name = "Osc Shape", default = 1)]
    pub osc_shape: EnumParam<PluginParamsOscShape>,
}
"#;
    assert_eq!(execution.stdout, expected);
}

#[test]
fn workspace_pin_ids_writes_unpinned_ids_and_is_idempotent() {
    let root = temp_cli_workspace("pin-ids");
    let manifest_path = root.join("manifest.hawk.toml");
    write_file(
        &manifest_path,
        r#"
[identity]
id = "com.hawk2ui.pin"
name = "Pin"
version = "0.1.0"

[source]
entry = "src/main.ts"

[plugin]
id = "com.hawk2ui.pin"
name = "Pin"

[[parameters]]
id = "gain"
name = "Gain"
default = 0.5
"#,
    );

    let execution = WorkspaceCommandRunner::new(&root).execute(CliCommand::PinIds);
    assert_eq!(execution.exit_code, CliExitCode::Success);
    assert!(
        execution.stdout.contains("gain = 0"),
        "{}",
        execution.stdout
    );

    let rewritten = fs::read_to_string(&manifest_path).expect("manifest is readable");
    assert!(rewritten.contains("param_id = 0"), "{rewritten}");

    // Running again is a no-op: every parameter id is already pinned.
    let again = WorkspaceCommandRunner::new(&root).execute(CliCommand::PinIds);
    assert_eq!(again.exit_code, CliExitCode::Success);
    assert!(again.stdout.contains("already"), "{}", again.stdout);
}

use hawk2ui_cli::{CliDiagnostic, DiagnosticSeverity, SourceSpan};

#[test]
fn diagnostics_render_warning_error_capability_denial_and_target_incompatibility() {
    let warning = CliDiagnostic::warning("style.unsupported", "unsupported style property")
        .file("src/app.hawk")
        .span(SourceSpan::new(12, 4, 12, 18))
        .suggested_fix("remove the unsupported property");
    let error = CliDiagnostic::error("manifest.invalid", "manifest is invalid")
        .related_target("desktop:linux-wayland");
    let capability = CliDiagnostic::capability_denial("filesystem.read", "filesystem read denied");
    let target = CliDiagnostic::target_incompatibility("plugin:vst3", "Wayland-only surface");

    assert_eq!(warning.severity, DiagnosticSeverity::Warning);
    assert!(warning.render().contains("src/app.hawk:12:4"));
    assert!(warning.render().contains("remove the unsupported property"));
    assert!(error.render().contains("desktop:linux-wayland"));
    assert!(capability.render().contains("capability=filesystem.read"));
    assert!(target.render().contains("target=plugin:vst3"));
}

#[test]
fn diagnostics_render_capability_denial_manifest_path() {
    let capability = CliDiagnostic::capability_denial(
        "hawk:files.readText",
        "file read denied by manifest permissions",
    )
    .manifest_path("permissions.capabilities[0]")
    .suggested_fix("declare hawk:files.readText in hawk.json permissions.capabilities");

    let rendered = capability.render();

    assert!(rendered.contains("capability=hawk:files.readText"));
    assert!(rendered.contains("manifest-path=permissions.capabilities[0]"));
    assert!(rendered.contains("declare hawk:files.readText"));
}

use hawk2ui_cli::testkit::{BuildCommandRunner, BuildCommandScenario};

#[test]
fn build_commands_return_success_validation_failure_and_verification_failure_codes() {
    let runner = BuildCommandRunner;

    assert_eq!(
        runner.validate(BuildCommandScenario::Success).exit_code,
        CliExitCode::Success
    );
    assert_eq!(
        runner.build_dev(BuildCommandScenario::Success).exit_code,
        CliExitCode::Success
    );
    assert_eq!(
        runner
            .build_release(BuildCommandScenario::Success)
            .exit_code,
        CliExitCode::Success
    );
    assert_eq!(
        runner
            .verify_artifact(BuildCommandScenario::Success)
            .exit_code,
        CliExitCode::Success
    );

    let validation = runner.validate(BuildCommandScenario::ValidationFailure);
    assert_eq!(validation.exit_code, CliExitCode::Validation);
    assert!(
        validation.diagnostics[0]
            .render()
            .contains("manifest.invalid")
    );

    let verification = runner.verify_artifact(BuildCommandScenario::VerificationFailure);
    assert_eq!(verification.exit_code, CliExitCode::Verification);
    assert!(
        verification.diagnostics[0]
            .render()
            .contains("artifact.verification-failed")
    );
}

#[test]
fn workspace_build_release_materializes_real_project_artifact() {
    let root = temp_cli_workspace("build-release");
    write_file(
        &root.join("manifest.hawk.toml"),
        r#"
[identity]
id = "com.hawk2ui.cli-build"
name = "CLI Build"
version = "1.0.0"

[source]
entry = "src/main.ts"
style = "styles/main.hawk.css"
script = "src/bootstrap.ts"

[capabilities]
keys = ["native-windowing", "sealed-artifacts"]

[[targets]]
kind = "desktop"
name = "linux-wayland"

[[assets]]
id = "logo"
kind = "vector"
path = "assets/logo.svg"
"#,
    );
    write_file(&root.join("src/main.ts"), "export const app = 'cli';");
    write_file(&root.join("src/bootstrap.ts"), "export const boot = true;");
    write_file(
        &root.join("styles/main.hawk.css"),
        ".root { display: flex; font-size: 18px; background-color: token(color.surface); }",
    );
    write_file(&root.join("assets/logo.svg"), "<svg />");

    let execution = signed_runner(&root).execute(CliCommand::BuildRelease);

    assert_eq!(execution.exit_code, CliExitCode::Success);
    assert!(execution.stdout.contains("built production artifact"));
    assert!(execution.stdout.contains("compiled-scripts: 2"));
    assert!(execution.stdout.contains("compiled-styles: 1"));
    assert!(execution.stdout.contains("compiled-assets: 1"));
    assert!(execution.stdout.contains("content-hash: sha256:"));
    assert!(execution.stdout.contains("artifact-path: "));
    assert!(
        execution
            .stdout
            .contains("signature-policy: verified-release")
    );
    assert!(
        root.join("target/hawk2ui/release/hawk2ui-artifact.hawk")
            .is_file()
    );
}

#[test]
fn workspace_build_release_requires_release_signing_key() {
    let root = temp_cli_workspace("build-release-unsigned");
    write_desktop_project(&root, "com.hawk2ui.cli-build-unsigned", "Unsigned Build");

    let execution = WorkspaceCommandRunner::new(&root).execute(CliCommand::BuildRelease);

    assert_eq!(execution.exit_code, CliExitCode::Verification);
    assert_eq!(
        execution.diagnostics[0].rule,
        "artifact.signature.signing-key-missing"
    );
}

#[test]
fn workspace_build_release_and_verify_accept_hex_release_key_configuration() {
    let root = temp_cli_workspace("build-release-hex-signing");
    write_desktop_project_with_asset(&root, "com.hawk2ui.cli-hex-signing", "Hex Signing");
    let signing_key_hex = "07".repeat(32);
    let verification_key =
        ArtifactSigningKey::ed25519_sha256_v1("hex-release-key", [7; 32]).verification_key();

    let build = WorkspaceCommandRunner::new(&root)
        .with_release_signing_key_hex("hex-release-key", signing_key_hex)
        .expect("hex signing key should parse")
        .execute(CliCommand::BuildRelease);
    assert_eq!(build.exit_code, CliExitCode::Success);

    let artifact_path = root.join("target/hawk2ui/release/hawk2ui-artifact.hawk");
    let verify = WorkspaceCommandRunner::new(&root)
        .with_trusted_release_key_hex("hex-release-key", verification_key.public_key)
        .execute(CliCommand::VerifyArtifact {
            path: Some(artifact_path.display().to_string()),
        });

    assert_eq!(verify.exit_code, CliExitCode::Success);
    assert!(verify.stdout.contains("trust-status: release-ready"));
}

#[test]
fn workspace_verify_artifact_reads_written_container_and_rejects_tampering() {
    let root = temp_cli_workspace("verify-artifact");
    write_desktop_project_with_asset(&root, "com.hawk2ui.cli-verify", "CLI Verify");

    let build = signed_runner(&root).execute(CliCommand::BuildRelease);
    assert_eq!(build.exit_code, CliExitCode::Success);
    let artifact_path = root.join("target/hawk2ui/release/hawk2ui-artifact.hawk");

    let verify = signed_runner(&root).execute(CliCommand::VerifyArtifact {
        path: Some(artifact_path.display().to_string()),
    });
    assert_eq!(verify.exit_code, CliExitCode::Success);
    assert!(verify.stdout.contains("verified artifact container"));
    assert!(verify.stdout.contains("signature-status: verified"));
    assert!(verify.stdout.contains("trust-status: release-ready"));
    assert!(verify.stdout.contains("compiled-scripts: "));
    assert!(verify.stdout.contains("compiled-assets: "));
    assert!(verify.stdout.contains("runtime-scene: "));

    let mut bytes = fs::read(&artifact_path).expect("artifact container should be readable");
    let last = bytes
        .last_mut()
        .expect("artifact container should not be empty");
    *last ^= 0x01;
    fs::write(&artifact_path, bytes).expect("tampered artifact should be written");

    let tampered = signed_runner(&root).execute(CliCommand::VerifyArtifact {
        path: Some(artifact_path.display().to_string()),
    });
    assert_eq!(tampered.exit_code, CliExitCode::Verification);
    assert!(
        tampered.diagnostics[0]
            .rule
            .starts_with("artifact.container")
            || tampered.diagnostics[0].rule.starts_with("artifact.schema")
    );
}

#[test]
fn workspace_verify_artifact_accepts_react_js_module_graph_payload() {
    let root = temp_cli_workspace("verify-react-js-graph");
    write_react_desktop_project(&root, "com.hawk2ui.cli-react-verify", "CLI React Verify");

    let build = signed_runner(&root).execute(CliCommand::BuildRelease);
    assert_eq!(build.exit_code, CliExitCode::Success);
    let artifact_path = root.join("target/hawk2ui/release/hawk2ui-artifact.hawk");

    let verify = signed_runner(&root).execute(CliCommand::VerifyArtifact {
        path: Some(artifact_path.display().to_string()),
    });

    assert_eq!(verify.exit_code, CliExitCode::Success);
    assert!(verify.stdout.contains("verified artifact container"));
    assert!(verify.stdout.contains("js-module-graphs: 1"));
    assert!(verify.stdout.contains("trust-status: release-ready"));
}

#[test]
fn workspace_build_release_accepts_vue_sealed_js_graph_payload() {
    let root = temp_cli_workspace("build-vue-js-graph");
    write_vue_desktop_project(&root, "com.hawk2ui.cli-vue-build", "CLI Vue Build");

    let build = signed_runner(&root).execute(CliCommand::BuildRelease);

    assert_eq!(build.exit_code, CliExitCode::Success);
    assert!(build.stdout.contains("built production artifact"));
    assert!(build.stdout.contains("compiled-frameworks: 0"));
    assert!(build.stdout.contains("js-module-graphs: 1"));
}

#[test]
fn workspace_verify_artifact_accepts_vue_sealed_js_graph_payload() {
    let root = temp_cli_workspace("verify-vue-js-graph");
    write_vue_desktop_project(&root, "com.hawk2ui.cli-vue-verify", "CLI Vue Verify");

    let build = signed_runner(&root).execute(CliCommand::BuildRelease);
    assert_eq!(build.exit_code, CliExitCode::Success);
    let artifact_path = root.join("target/hawk2ui/release/hawk2ui-artifact.hawk");

    let verify = signed_runner(&root).execute(CliCommand::VerifyArtifact {
        path: Some(artifact_path.display().to_string()),
    });

    assert_eq!(verify.exit_code, CliExitCode::Success);
    assert!(verify.stdout.contains("verified artifact container"));
    assert!(verify.stdout.contains("js-module-graphs: 1"));
    assert!(verify.stdout.contains("trust-status: release-ready"));
}

#[test]
fn workspace_run_desktop_accepts_vue_sealed_js_graph_payload() {
    let root = temp_cli_workspace("run-vue-js-graph");
    write_vue_desktop_project(&root, "com.hawk2ui.cli-vue-run", "CLI Vue Run");

    let execution = Command::new(env!("CARGO_BIN_EXE_hawk2ui-cli"))
        .current_dir(&root)
        .env("HAWK2UI_EXIT_AFTER_FIRST_FRAME", "1")
        .arg("run-desktop")
        .output()
        .expect("hawk2ui-cli run-desktop child process should execute");
    let stdout = String::from_utf8_lossy(&execution.stdout);
    let stderr = String::from_utf8_lossy(&execution.stderr);

    assert!(
        execution.status.success(),
        "run-desktop should accept Vue sealed JS graphs\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(stdout.contains("desktop runtime exited cleanly"));
}

#[test]
fn workspace_verify_artifact_rejects_unsigned_development_container() {
    let root = temp_cli_workspace("verify-unsigned-artifact");
    write_desktop_project_with_asset(
        &root,
        "com.hawk2ui.cli-verify-unsigned",
        "CLI Verify Unsigned",
    );

    let build = WorkspaceCommandRunner::new(&root).execute(CliCommand::BuildDev);
    assert_eq!(build.exit_code, CliExitCode::Success);
    let artifact_path = root.join("target/hawk2ui/dev/hawk2ui-artifact.hawk");

    let verify = signed_runner(&root).execute(CliCommand::VerifyArtifact {
        path: Some(artifact_path.display().to_string()),
    });

    assert_eq!(verify.exit_code, CliExitCode::Verification);
    assert_eq!(
        verify.diagnostics[0].rule,
        "security.package.signature-missing"
    );
}

#[test]
fn workspace_verify_artifact_rejects_signed_container_without_runtime_payloads() {
    let root = temp_cli_workspace("verify-empty-runtime-payload");
    let manifest = HawkManifest::parse(
        r#"
[identity]
id = "com.hawk2ui.cli-empty-runtime"
name = "CLI Empty Runtime"
version = "1.0.0"

[source]
entry = "src/main.ts"

[capabilities]
keys = ["native-windowing", "sealed-artifacts"]

[[targets]]
kind = "desktop"
name = "linux-wayland"
"#,
    )
    .expect("manifest parses");
    let signing_key = ArtifactSigningKey::ed25519_sha256_v1("test-release-key", [7; 32]);
    let artifact = signing_key.sign(&SealedArtifact::from_manifest(
        ArtifactSchemaVersion::new(1, 0),
        &manifest,
    ));
    let artifact_path = root.join("empty-runtime-payload.hawk");
    fs::write(
        &artifact_path,
        artifact
            .to_container_bytes(ArtifactSignaturePolicy::RequireVerifiedSignature)
            .expect("signed artifact container serializes"),
    )
    .expect("signed artifact container writes");

    let verify = WorkspaceCommandRunner::new(&root)
        .with_trusted_release_key(signing_key.verification_key())
        .execute(CliCommand::VerifyArtifact {
            path: Some(artifact_path.display().to_string()),
        });

    assert_eq!(verify.exit_code, CliExitCode::Verification);
    assert_eq!(
        verify.diagnostics[0].rule,
        "security.package.script-hashes-missing"
    );
}

#[test]
fn workspace_build_release_rejects_missing_declared_asset() {
    let root = temp_cli_workspace("missing-asset");
    write_file(
        &root.join("manifest.hawk.toml"),
        r#"
[identity]
id = "com.hawk2ui.cli-missing-asset"
name = "CLI Missing Asset"
version = "1.0.0"

[source]
entry = "src/main.ts"

[[assets]]
id = "logo"
kind = "vector"
path = "assets/logo.svg"
"#,
    );
    write_file(&root.join("src/main.ts"), "export const app = 'cli';");

    let execution = WorkspaceCommandRunner::new(&root).execute(CliCommand::BuildRelease);

    assert_eq!(execution.exit_code, CliExitCode::Validation);
    assert_eq!(execution.diagnostics[0].rule, "build.file-missing");
    assert!(execution.stderr.contains("assets/logo.svg"));
}

#[test]
fn workspace_validate_rejects_missing_declared_asset() {
    let root = temp_cli_workspace("validate-missing-asset");
    write_file(
        &root.join("manifest.hawk.toml"),
        r#"
[identity]
id = "com.hawk2ui.cli-validate-missing-asset"
name = "CLI Validate Missing Asset"
version = "1.0.0"

[source]
entry = "src/main.ts"

[[assets]]
id = "logo"
kind = "vector"
path = "assets/logo.svg"
"#,
    );
    write_file(&root.join("src/main.ts"), "export const app = 'cli';");

    let execution = WorkspaceCommandRunner::new(&root).execute(CliCommand::Validate);

    assert_eq!(execution.exit_code, CliExitCode::Validation);
    assert_eq!(execution.diagnostics[0].rule, "build.file-missing");
    assert!(execution.stderr.contains("assets/logo.svg"));
}

#[test]
fn workspace_diagnostics_reports_missing_declared_asset() {
    let root = temp_cli_workspace("diagnostics-missing-asset");
    write_file(
        &root.join("manifest.hawk.toml"),
        r#"
[identity]
id = "com.hawk2ui.cli-diagnostics-missing-asset"
name = "CLI Diagnostics Missing Asset"
version = "1.0.0"

[source]
entry = "src/main.ts"

[[assets]]
id = "logo"
kind = "vector"
path = "assets/logo.svg"
"#,
    );
    write_file(&root.join("src/main.ts"), "export const app = 'cli';");

    let execution = WorkspaceCommandRunner::new(&root).execute(CliCommand::Diagnostics);

    assert_eq!(execution.exit_code, CliExitCode::Validation);
    assert_eq!(execution.diagnostics[0].rule, "build.file-missing");
    assert!(execution.stderr.contains("assets/logo.svg"));
}

#[test]
fn workspace_package_plugin_materializes_plugin_outputs() {
    let root = temp_cli_workspace("package-plugin");
    write_plugin_project(&root, "com.hawk2ui.cli-plugin", "CLI Plugin");

    let execution = signed_runner(&root).execute(CliCommand::PackagePlugin);

    assert_eq!(execution.exit_code, CliExitCode::Success);
    assert!(
        execution
            .stdout
            .contains("materialized plugin package layouts")
    );
    assert!(
        execution
            .stdout
            .contains("layout-verification-status: passed")
    );
    assert!(
        execution
            .stdout
            .contains("host-loadable-binaries: produced=3")
    );
    for extension in ["clap", "vst3", "component"] {
        let package_root = root
            .join("target/hawk2ui")
            .join(format!("com-hawk2ui-cli-plugin.{extension}"));
        assert!(
            package_root.is_dir(),
            "{} should exist",
            package_root.display()
        );
        assert!(
            package_root.join("hawk2ui-package.toml").is_file(),
            "{} package metadata should exist",
            package_root.display()
        );
        assert!(
            package_root
                .join("Contents/Resources/hawk2ui-artifact.toml")
                .is_file(),
            "{} artifact descriptor should exist",
            package_root.display()
        );
        let runtime_artifact_path =
            package_root.join("Contents/Resources/hawk2ui-runtime-artifact.json");
        assert!(
            runtime_artifact_path.is_file(),
            "{} runtime artifact should exist",
            package_root.display()
        );
        assert!(
            package_root
                .join("Contents/Resources/hawk2ui-editor.toml")
                .is_file(),
            "{} editor descriptor should exist",
            package_root.display()
        );
        let runtime_artifact: SealedArtifact = serde_json::from_str(
            &fs::read_to_string(runtime_artifact_path).expect("runtime artifact reads"),
        )
        .expect("runtime artifact decodes as a sealed artifact");
        assert_eq!(
            runtime_artifact.signature.status,
            ArtifactSignatureStatus::Verified
        );
    }
    let standalone_package_root = root.join("target/hawk2ui/com-hawk2ui-cli-plugin.app");
    assert!(
        !standalone_package_root.exists(),
        "{} should not be emitted until package-plugin can compile a real host binary for it",
        standalone_package_root.display()
    );

    #[cfg(target_os = "linux")]
    {
        let clap_binary = root
            .join("target/hawk2ui/com-hawk2ui-cli-plugin.clap")
            .join("CLI Plugin.clap");
        let vst3_binary = root
            .join("target/hawk2ui/com-hawk2ui-cli-plugin.vst3")
            .join("Contents/x86_64-linux/CLI Plugin.vst3");
        let au_binary = root
            .join("target/hawk2ui/com-hawk2ui-cli-plugin.component")
            .join("Contents/MacOS/CLI Plugin");
        for binary in [clap_binary, vst3_binary, au_binary] {
            let bytes = fs::read(&binary).unwrap_or_else(|error| {
                panic!(
                    "host-loadable binary `{}` should read: {error}",
                    binary.display()
                )
            });
            assert!(
                bytes.starts_with(b"\x7fELF"),
                "{} must be an ELF shared library, not a text placeholder",
                binary.display()
            );
        }
    }
}

#[test]
fn workspace_package_desktop_materializes_signed_native_launcher_bundle() {
    let root = temp_cli_workspace("package-desktop");
    write_desktop_project(
        &root,
        "com.hawk2ui.cli-desktop-package",
        "CLI Desktop Package",
    );

    let execution =
        signed_runner_with_packaged_desktop_launcher(&root).execute(CliCommand::PackageDesktop);

    assert_eq!(execution.exit_code, CliExitCode::Success);
    assert!(execution.stdout.contains("materialized desktop package"));
    assert!(
        execution
            .stdout
            .contains("launcher-verification-status: passed")
    );
    let package_root = root
        .join("target/hawk2ui")
        .join("com-hawk2ui-cli-desktop-package.AppDir");
    let launcher = package_root.join("usr/bin/com-hawk2ui-cli-desktop-package");
    let artifact_path = package_root.join("usr/share/hawk2ui/hawk2ui-artifact.hawk");
    let manifest_path = package_root.join("hawk2ui-desktop-package.json");
    let hash_manifest_path = package_root.join("usr/share/hawk2ui/hawk2ui-hashes.json");
    let generated_launcher_dir = package_root.join("usr/share/hawk2ui/generated-launcher");

    assert!(launcher.is_file(), "desktop launcher should exist");
    assert!(artifact_path.is_file(), "signed artifact should be bundled");
    assert!(
        manifest_path.is_file(),
        "desktop package manifest should exist"
    );
    assert!(
        hash_manifest_path.is_file(),
        "desktop package hash manifest should exist"
    );
    assert!(
        !generated_launcher_dir.exists(),
        "desktop package should not generate a Cargo launcher workspace when a prebuilt launcher is supplied"
    );

    let artifact: SealedArtifact =
        serde_json::from_str(&fs::read_to_string(&artifact_path).expect("desktop artifact reads"))
            .expect("desktop artifact decodes as sealed artifact");
    assert_eq!(artifact.signature.status, ArtifactSignatureStatus::Verified);

    #[cfg(target_os = "linux")]
    {
        let bytes = fs::read(&launcher).expect("launcher binary reads");
        assert!(
            bytes.starts_with(b"\x7fELF"),
            "{} must be an ELF native executable, not a script placeholder",
            launcher.display()
        );
    }

    let launch = Command::new(&launcher)
        .env("HAWK2UI_EXIT_AFTER_FIRST_FRAME", "1")
        .output()
        .expect("packaged launcher should execute");
    assert!(
        launch.status.success(),
        "packaged launcher should run the native desktop runtime: stdout={} stderr={}",
        String::from_utf8_lossy(&launch.stdout),
        String::from_utf8_lossy(&launch.stderr)
    );
}

#[test]
fn workspace_package_desktop_writes_json_metadata_values() {
    let root = temp_cli_workspace("package-desktop-escaping");
    let display_name = "CLI \"Quoted\" Desktop";
    write_desktop_project(&root, "com.hawk2ui.cli-desktop-escaping", display_name);

    let execution =
        signed_runner_with_packaged_desktop_launcher(&root).execute(CliCommand::PackageDesktop);

    assert_eq!(
        execution.exit_code,
        CliExitCode::Success,
        "package-desktop should succeed for quoted names\nstdout:\n{}\nstderr:\n{}\ndiagnostics:\n{:#?}",
        execution.stdout,
        execution.stderr,
        execution.diagnostics
    );
    let package_root = root
        .join("target/hawk2ui")
        .join("com-hawk2ui-cli-desktop-escaping.AppDir");
    let manifest_path = package_root.join("hawk2ui-desktop-package.json");
    let hash_manifest_path = package_root.join("usr/share/hawk2ui/hawk2ui-hashes.json");
    let manifest_source =
        fs::read_to_string(&manifest_path).expect("desktop package manifest should read");
    let manifest: serde_json::Value = serde_json::from_str(&manifest_source)
        .expect("desktop package manifest should be valid JSON");
    let hash_manifest_source =
        fs::read_to_string(&hash_manifest_path).expect("desktop hash manifest should read");
    let hash_manifest: serde_json::Value = serde_json::from_str(&hash_manifest_source)
        .expect("desktop hash manifest should be valid JSON");

    assert_eq!(manifest["displayName"].as_str(), Some(display_name));
    assert_eq!(
        manifest["entry"].as_str(),
        Some("usr/bin/com-hawk2ui-cli-desktop-escaping")
    );
    assert!(hash_manifest["files"].as_array().is_some_and(|files| {
        files.iter().any(|file| {
            file["path"]
                .as_str()
                .is_some_and(|path| path == "usr/bin/com-hawk2ui-cli-desktop-escaping")
        })
    }));
}

#[test]
fn workspace_package_desktop_uses_path_safe_launcher_name_from_identity_id() {
    let root = temp_cli_workspace("package-desktop-safe-launcher");
    let outside_launcher = root.join("escaped-launcher");
    let display_name = outside_launcher.to_string_lossy();
    write_desktop_project(
        &root,
        "com.hawk2ui.cli-desktop-safe-launcher",
        &display_name,
    );

    let execution =
        signed_runner_with_packaged_desktop_launcher(&root).execute(CliCommand::PackageDesktop);

    assert_eq!(
        execution.exit_code,
        CliExitCode::Success,
        "package-desktop should not treat display names as paths\nstdout:\n{}\nstderr:\n{}\ndiagnostics:\n{:#?}",
        execution.stdout,
        execution.stderr,
        execution.diagnostics
    );
    let package_root = root
        .join("target/hawk2ui")
        .join("com-hawk2ui-cli-desktop-safe-launcher.AppDir");
    let launcher = package_root.join("usr/bin/com-hawk2ui-cli-desktop-safe-launcher");
    let manifest_path = package_root.join("hawk2ui-desktop-package.json");
    let manifest_source =
        fs::read_to_string(&manifest_path).expect("desktop package manifest should read");
    let manifest: serde_json::Value = serde_json::from_str(&manifest_source)
        .expect("desktop package manifest should be valid JSON");

    assert!(launcher.is_file(), "launcher should use the package id");
    assert!(
        !outside_launcher.exists(),
        "package-desktop must not write a launcher outside the package root"
    );
    assert_eq!(
        manifest["displayName"].as_str(),
        Some(display_name.as_ref())
    );
    assert_eq!(
        manifest["entry"].as_str(),
        Some("usr/bin/com-hawk2ui-cli-desktop-safe-launcher")
    );
}

#[test]
fn workspace_package_plugin_requires_release_signing_key() {
    let root = temp_cli_workspace("package-plugin-unsigned");
    write_plugin_project(&root, "com.hawk2ui.cli-plugin-unsigned", "Unsigned Plugin");

    let execution = WorkspaceCommandRunner::new(&root).execute(CliCommand::PackagePlugin);

    assert_eq!(execution.exit_code, CliExitCode::Verification);
    assert_eq!(
        execution.diagnostics[0].rule,
        "artifact.signature.signing-key-missing"
    );
}

#[test]
fn workspace_package_plugin_rejects_missing_declared_asset() {
    let root = temp_cli_workspace("package-plugin-missing-asset");
    write_file(
        &root.join("manifest.hawk.toml"),
        r#"
[identity]
id = "com.hawk2ui.cli-plugin-missing-asset"
name = "CLI Plugin Missing Asset"
version = "1.0.0"

[source]
entry = "src/main.ts"

[[targets]]
kind = "plugin"
name = "audio-plugin"

[plugin]
id = "com.hawk2ui.cli-plugin-missing-asset"
name = "CLI Plugin Missing Asset"

[editor]
width = 960
height = 540

[[assets]]
id = "logo"
kind = "vector"
path = "assets/logo.svg"
"#,
    );
    write_file(&root.join("src/main.ts"), "export const app = 'plugin';");

    let execution = WorkspaceCommandRunner::new(&root).execute(CliCommand::PackagePlugin);

    assert_eq!(execution.exit_code, CliExitCode::Validation);
    assert_eq!(execution.diagnostics[0].rule, "build.file-missing");
    assert!(execution.stderr.contains("assets/logo.svg"));
}

#[test]
fn workspace_run_desktop_rejects_missing_declared_asset_before_runtime() {
    let root = temp_cli_workspace("run-desktop-missing-asset");
    write_file(
        &root.join("manifest.hawk.toml"),
        r#"
[identity]
id = "com.hawk2ui.cli-desktop-missing-asset"
name = "CLI Desktop Missing Asset"
version = "1.0.0"

[source]
entry = "src/main.ts"

[[targets]]
kind = "desktop"
name = "linux-wayland"

[[assets]]
id = "logo"
kind = "vector"
path = "assets/logo.svg"
"#,
    );
    write_file(&root.join("src/main.ts"), "export const app = 'desktop';");

    let execution = WorkspaceCommandRunner::new(&root).execute(CliCommand::RunDesktop {
        presentation_backend: CliPresentationBackend::Software,
    });

    assert_eq!(execution.exit_code, CliExitCode::Validation);
    assert_eq!(execution.diagnostics[0].rule, "build.file-missing");
    assert!(execution.stderr.contains("assets/logo.svg"));
}

use hawk2ui_cli::{
    DevLoop, DevLoopEvent,
    testkit::{RecordingReloadTarget, RecordingWatcher},
};

#[test]
fn dev_loop_watches_rebuilds_validates_reloads_and_preserves_state() {
    let watcher = RecordingWatcher::new(["src/main.ts", "styles/main.hawk.css"]);
    let reload_target = RecordingReloadTarget::default();
    let mut dev_loop = DevLoop::new(watcher, reload_target).preserve_state(true);

    let report = dev_loop.run_once().expect("dev loop should run");

    assert_eq!(
        report.events,
        vec![
            DevLoopEvent::FileChanged("src/main.ts".into()),
            DevLoopEvent::FileChanged("styles/main.hawk.css".into()),
            DevLoopEvent::IncrementalRebuildTriggered,
            DevLoopEvent::ValidationPassed,
            DevLoopEvent::NativeSurfaceReloaded {
                preserve_state: true
            },
        ]
    );
    assert!(report.visible_errors.is_empty());
    assert!(report.error_overlay.is_none());
}

#[test]
fn dev_loop_reports_visible_errors_before_runtime_reload() {
    let watcher = RecordingWatcher::new(["manifest.hawk.toml"]);
    let reload_target = RecordingReloadTarget::default();
    let mut dev_loop = DevLoop::new(watcher, reload_target).validation_fails("manifest.invalid");

    let report = dev_loop
        .run_once()
        .expect("dev loop should report validation errors");

    assert_eq!(
        report.events,
        vec![
            DevLoopEvent::FileChanged("manifest.hawk.toml".into()),
            DevLoopEvent::IncrementalRebuildTriggered,
            DevLoopEvent::ValidationFailed,
        ]
    );
    assert_eq!(report.visible_errors[0].rule, "manifest.invalid");
    let overlay = report
        .error_overlay
        .expect("validation failure should produce an in-window overlay record");
    assert_eq!(overlay.rule(), "manifest.invalid");
    assert_eq!(overlay.message(), "validation failed");
    assert_eq!(overlay.to_desktop_overlay().rule(), "manifest.invalid");
}

#[test]
fn dev_loop_clears_visible_errors_after_recovered_validation() {
    let watcher = RecordingWatcher::new(["src/App.tsx"]);
    let reload_target = RecordingReloadTarget::default();
    let mut dev_loop = DevLoop::new(watcher, reload_target).validation_fails("react.syntax");

    let failed = dev_loop
        .run_once()
        .expect("first dev loop should expose validation errors");
    assert_eq!(failed.events.last(), Some(&DevLoopEvent::ValidationFailed));
    assert!(failed.error_overlay.is_some());

    dev_loop.validation_passes();
    let recovered = dev_loop
        .run_once()
        .expect("second dev loop should clear stale diagnostics and reload");

    assert_eq!(
        recovered.events,
        vec![
            DevLoopEvent::FileChanged("src/App.tsx".into()),
            DevLoopEvent::IncrementalRebuildTriggered,
            DevLoopEvent::ValidationPassed,
            DevLoopEvent::DiagnosticsCleared,
            DevLoopEvent::NativeSurfaceReloaded {
                preserve_state: false
            },
        ]
    );
    assert!(recovered.visible_errors.is_empty());
    assert!(recovered.error_overlay.is_none());
}

#[test]
fn manual_presence_pages_exist_and_contain_required_headings() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let pages = [
        ("manual/user-manual.md", "# Hawk2UI User Manual"),
        ("manual/developer-guide.md", "# Hawk2UI Developer Guide"),
        ("manual/style-reference.md", "# Hawk2UI Style Reference"),
        (
            "manual/plugin-author-guide.md",
            "# Hawk2UI Plugin Author Guide",
        ),
        ("manual/desktop-app-guide.md", "# Hawk2UI Desktop App Guide"),
        ("manual/troubleshooting.md", "# Hawk2UI Troubleshooting"),
        ("manual/api-reference.md", "# Hawk2UI API Reference"),
        ("manual/examples-index.md", "# Hawk2UI Examples Index"),
    ];

    for (path, heading) in pages {
        let content = std::fs::read_to_string(root.join(path)).expect("manual page should exist");
        assert!(content.contains(heading), "{path} missing {heading}");
    }
}

fn temp_cli_workspace(label: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("hawk2ui-cli-{label}-{now}"));
    fs::create_dir_all(&root).expect("temp cli workspace should be created");
    root
}

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("test parent directory should be created");
    }
    fs::write(path, contents).expect("test file should be written");
}

fn signed_runner(root: &Path) -> WorkspaceCommandRunner {
    let signing_key = ArtifactSigningKey::ed25519_sha256_v1("test-release-key", [7; 32]);
    WorkspaceCommandRunner::new(root)
        .with_release_signing_key(signing_key.clone())
        .with_trusted_release_key(signing_key.verification_key())
}

fn signed_runner_with_packaged_desktop_launcher(root: &Path) -> WorkspaceCommandRunner {
    signed_runner(root).with_desktop_launcher_binary(env!("CARGO_BIN_EXE_hawk2ui-packaged-desktop"))
}

fn write_desktop_project(root: &Path, id: &str, name: &str) {
    let id = test_toml_string(id);
    let name = test_toml_string(name);
    write_file(
        &root.join("manifest.hawk.toml"),
        &format!(
            r#"
[identity]
id = {id}
name = {name}
version = "1.0.0"

[source]
entry = "src/main.ts"
style = "styles/main.hawk.css"

[capabilities]
keys = ["native-windowing", "sealed-artifacts"]

[[targets]]
kind = "desktop"
name = "linux-wayland"
"#
        ),
    );
    write_file(&root.join("src/main.ts"), "export const app = 'desktop';");
    write_file(
        &root.join("styles/main.hawk.css"),
        ".root { display: flex; font-size: 18px; background-color: token(color.surface); }",
    );
}

fn write_react_desktop_project(root: &Path, id: &str, name: &str) {
    let id_json = serde_json::to_string(id).expect("id serializes");
    let name_json = serde_json::to_string(name).expect("name serializes");
    write_file(
        &root.join("hawk.json"),
        &format!(
            r#"{{
  "$schema": "https://hawk2ui.dev/schemas/hawk.schema.json",
  "schemaVersion": 1,
  "package": {{
    "id": {id_json},
    "name": {name_json},
    "version": "1.0.0",
    "bundleId": {id_json}
  }},
  "app": {{
    "entry": "src/App.tsx",
    "framework": "react"
  }},
  "build": {{
    "output": "dist/main.js"
  }},
  "permissions": {{
    "capabilities": ["native-windowing", "sealed-artifacts"]
  }},
  "targets": {{
    "desktop": [
      {{
        "name": "linux-wayland",
        "platforms": ["linux-wayland"]
      }}
    ]
  }}
}}"#,
        ),
    );
    write_file(&root.join("bun.lock"), "lockfileVersion = 1\n");
    write_file(
        &root.join("src/App.tsx"),
        r#"import { createRoot } from "@hawk2ui/react";
function App() {
  return <view id="root"><text>Ready</text></view>;
}
createRoot("root").render(<App />);"#,
    );
    write_file(
        &root.join("dist/main.js"),
        "globalThis.__hawk2uiCliReactBundle = true;",
    );
}

fn write_vue_desktop_project(root: &Path, id: &str, name: &str) {
    let id_json = serde_json::to_string(id).expect("id serializes");
    let name_json = serde_json::to_string(name).expect("name serializes");
    write_file(
        &root.join("hawk.json"),
        &format!(
            r#"{{
  "$schema": "https://hawk2ui.dev/schemas/hawk.schema.json",
  "schemaVersion": 1,
  "package": {{
    "id": {id_json},
    "name": {name_json},
    "version": "1.0.0",
    "bundleId": {id_json}
  }},
  "app": {{
    "entry": "src/main.ts",
    "framework": "vue"
  }},
  "build": {{
    "output": "dist/main.js"
  }},
  "permissions": {{
    "capabilities": ["native-windowing", "sealed-artifacts"]
  }},
  "targets": {{
    "desktop": [
      {{
        "name": "linux-wayland",
        "platforms": ["linux-wayland"]
      }}
    ]
  }}
}}"#,
        ),
    );
    write_file(&root.join("bun.lock"), "lockfileVersion = 1\n");
    write_file(
        &root.join("src/main.ts"),
        r#"import { createApp } from "@hawk2ui/vue";
import App from "./App.vue";

createApp(App).mount();"#,
    );
    write_file(
        &root.join("dist/main.js"),
        "globalThis.__hawk2uiCliVueBundle = true;",
    );
}

fn test_toml_string(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    )
}

fn write_desktop_project_with_asset(root: &Path, id: &str, name: &str) {
    write_file(
        &root.join("manifest.hawk.toml"),
        &format!(
            r#"
[identity]
id = "{id}"
name = "{name}"
version = "1.0.0"

[source]
entry = "src/main.ts"
style = "styles/main.hawk.css"

[capabilities]
keys = ["native-windowing", "sealed-artifacts"]

[[targets]]
kind = "desktop"
name = "linux-wayland"

[[assets]]
id = "logo"
kind = "vector"
path = "assets/logo.svg"
"#
        ),
    );
    write_file(&root.join("src/main.ts"), "export const app = 'desktop';");
    write_file(
        &root.join("styles/main.hawk.css"),
        ".root { display: flex; font-size: 18px; background-color: token(color.surface); }",
    );
    write_file(&root.join("assets/logo.svg"), "<svg />");
}

fn write_plugin_project(root: &Path, id: &str, name: &str) {
    write_file(
        &root.join("manifest.hawk.toml"),
        &format!(
            r#"
[identity]
id = "{id}"
name = "{name}"
version = "1.0.0"

[source]
entry = "src/main.ts"

[capabilities]
keys = ["sealed-artifacts"]

[[targets]]
kind = "plugin"
name = "audio-plugin"

[plugin]
id = "{id}"
name = "{name}"

[editor]
width = 960
height = 540

[[parameters]]
id = "gain"
name = "Gain"
default = 0.5
"#
        ),
    );
    write_file(&root.join("src/main.ts"), "export const app = 'plugin';");
}
