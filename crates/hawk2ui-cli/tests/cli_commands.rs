use hawk2ui_cli::{CliCommand, CliExitCode, CommandCatalog, WorkspaceCommandRunner};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn cli_commands_help_lists_required_workflows() {
    let catalog = CommandCatalog;
    let help = catalog.render_help();

    for command in [
        "new",
        "run",
        "dev",
        "validate",
        "build-dev",
        "build-release",
        "verify-artifact",
        "run-desktop",
        "package-plugin",
        "export-schemas",
        "export-params",
        "diagnostics",
        "explain",
    ] {
        assert!(help.contains(command), "help missing command: {command}");
    }
}

#[test]
fn cli_commands_parse_known_commands_and_reject_invalid_command() {
    let catalog = CommandCatalog;

    assert_eq!(
        catalog.parse(["hawk2ui", "new"]).unwrap(),
        CliCommand::NewProject
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

    let error = catalog
        .parse(["hawk2ui", "nope"])
        .expect_err("invalid command should fail");
    assert_eq!(error.exit_code, CliExitCode::Usage);
    assert!(error.message.contains("unknown command"));
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

    assert_eq!(execution.exit_code, CliExitCode::Success);
    assert!(execution.stdout.contains("src/main.ts"));
    assert!(execution.stdout.contains("src/bootstrap.ts"));
    assert!(execution.stdout.contains("styles/main.hawk.css"));
    assert!(execution.stdout.contains("assets/logo.svg"));
}

#[test]
fn workspace_new_project_creates_buildable_desktop_and_plugin_scaffold() {
    let root = temp_cli_workspace("new-project");

    let created = WorkspaceCommandRunner::new(&root).execute(CliCommand::NewProject);

    assert_eq!(created.exit_code, CliExitCode::Success);
    for path in [
        "manifest.hawk.toml",
        "src/main.ts",
        "src/bootstrap.ts",
        "styles/main.hawk.css",
        "assets/logo.svg",
        "README.md",
    ] {
        assert!(root.join(path).is_file(), "scaffold missing {path}");
    }

    let manifest = fs::read_to_string(root.join("manifest.hawk.toml"))
        .expect("generated manifest should be readable");
    assert!(manifest.contains("kind = \"desktop\""));
    assert!(manifest.contains("kind = \"plugin\""));
    assert!(manifest.contains("[[parameters]]"));
    assert!(manifest.contains("[[assets]]"));

    let validate = WorkspaceCommandRunner::new(&root).execute(CliCommand::Validate);
    assert_eq!(validate.exit_code, CliExitCode::Success);

    let build = WorkspaceCommandRunner::new(&root).execute(CliCommand::BuildRelease);
    assert_eq!(build.exit_code, CliExitCode::Success);
    assert!(build.stdout.contains("compiled-scripts: 2"));
    assert!(build.stdout.contains("compiled-styles: 1"));
    assert!(build.stdout.contains("compiled-assets: 1"));

    let package = WorkspaceCommandRunner::new(&root).execute(CliCommand::PackagePlugin);
    assert_eq!(package.exit_code, CliExitCode::Success);
    assert!(
        package
            .stdout
            .contains("layout-verification-status: passed")
    );
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
    assert!(execution.stdout.contains("#[derive(Params)]"));
    assert!(execution.stdout.contains("pub struct PluginParams"));
    assert!(
        execution.stdout.contains("range = \"linear(20, 20000)\""),
        "manifest range should flow into the emitted source: {}",
        execution.stdout
    );
    assert!(execution.stdout.contains("unit = \"Hz\""));
    // A manifest meter flows through the same model into a truce `#[meter]`
    // field — the read-only level output the editor consumes.
    assert!(
        execution.stdout.contains("#[meter]")
            && execution.stdout.contains("pub output_level: MeterSlot,"),
        "manifest meter should flow into the emitted source: {}",
        execution.stdout
    );
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
    // The choice parameter emits a generated `#[derive(ParamEnum)]` type
    // (named after the struct) and an `EnumParam<...>` field referencing it.
    assert!(
        execution.stdout.contains("#[derive(ParamEnum)]")
            && execution.stdout.contains("pub enum PluginParamsOscShape {"),
        "enum type should be generated: {}",
        execution.stdout
    );
    assert!(
        execution.stdout.contains("#[name = \"Square / Pulse\"]")
            && execution.stdout.contains("SquarePulse,"),
        "variant display name should be emitted: {}",
        execution.stdout
    );
    assert!(
        execution
            .stdout
            .contains("pub osc_shape: EnumParam<PluginParamsOscShape>,"),
        "enum field should reference the generated type: {}",
        execution.stdout
    );
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

use hawk2ui_cli::{BuildCommandRunner, BuildCommandScenario};

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

    let execution = WorkspaceCommandRunner::new(&root).execute(CliCommand::BuildRelease);

    assert_eq!(execution.exit_code, CliExitCode::Success);
    assert!(execution.stdout.contains("built production artifact"));
    assert!(execution.stdout.contains("compiled-scripts: 2"));
    assert!(execution.stdout.contains("compiled-styles: 1"));
    assert!(execution.stdout.contains("compiled-assets: 1"));
    assert!(execution.stdout.contains("content-hash: sha256:"));
    assert!(execution.stdout.contains("artifact-path: "));
    assert!(
        root.join("target/hawk2ui/release/hawk2ui-artifact.hawk")
            .is_file()
    );
}

#[test]
fn workspace_verify_artifact_reads_written_container_and_rejects_tampering() {
    let root = temp_cli_workspace("verify-artifact");
    write_desktop_project(&root, "com.hawk2ui.cli-verify", "CLI Verify");

    let build = WorkspaceCommandRunner::new(&root).execute(CliCommand::BuildRelease);
    assert_eq!(build.exit_code, CliExitCode::Success);
    let artifact_path = root.join("target/hawk2ui/release/hawk2ui-artifact.hawk");

    let verify = WorkspaceCommandRunner::new(&root).execute(CliCommand::VerifyArtifact {
        path: Some(artifact_path.display().to_string()),
    });
    assert_eq!(verify.exit_code, CliExitCode::Success);
    assert!(verify.stdout.contains("verified artifact container"));
    assert!(
        verify
            .stdout
            .contains("signature-status: unsigned-development")
    );

    let mut bytes = fs::read(&artifact_path).expect("artifact container should be readable");
    let last = bytes
        .last_mut()
        .expect("artifact container should not be empty");
    *last ^= 0x01;
    fs::write(&artifact_path, bytes).expect("tampered artifact should be written");

    let tampered = WorkspaceCommandRunner::new(&root).execute(CliCommand::VerifyArtifact {
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
    write_file(
        &root.join("manifest.hawk.toml"),
        r#"
[identity]
id = "com.hawk2ui.cli-plugin"
name = "CLI Plugin"
version = "1.0.0"

[source]
entry = "src/main.ts"

[capabilities]
keys = ["sealed-artifacts"]

[[targets]]
kind = "plugin"
name = "audio-plugin"

[plugin]
id = "com.hawk2ui.cli-plugin"
name = "CLI Plugin"

[editor]
width = 960
height = 540

[[parameters]]
id = "gain"
name = "Gain"
default = 0.5
"#,
    );
    write_file(&root.join("src/main.ts"), "export const app = 'plugin';");

    let execution = WorkspaceCommandRunner::new(&root).execute(CliCommand::PackagePlugin);

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
            .contains("host-loadable-binaries: not-produced-by-this-command")
    );
    for extension in ["clap", "vst3", "component", "app"] {
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
    }
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

    let execution = WorkspaceCommandRunner::new(&root).execute(CliCommand::RunDesktop);

    assert_eq!(execution.exit_code, CliExitCode::Validation);
    assert_eq!(execution.diagnostics[0].rule, "build.file-missing");
    assert!(execution.stderr.contains("assets/logo.svg"));
}

use hawk2ui_cli::{DevLoop, DevLoopEvent, RecordingReloadTarget, RecordingWatcher};

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

fn write_desktop_project(root: &Path, id: &str, name: &str) {
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
"#
        ),
    );
    write_file(&root.join("src/main.ts"), "export const app = 'desktop';");
    write_file(
        &root.join("styles/main.hawk.css"),
        ".root { display: flex; font-size: 18px; background-color: token(color.surface); }",
    );
}
