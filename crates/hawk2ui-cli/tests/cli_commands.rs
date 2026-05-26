use hawk2ui_cli::{CliCommand, CliExitCode, CommandCatalog, WorkspaceCommandRunner};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn cli_commands_help_lists_required_workflows() {
    let catalog = CommandCatalog::default();
    let help = catalog.render_help();

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
        assert!(help.contains(command), "help missing command: {command}");
    }
}

#[test]
fn cli_commands_parse_known_commands_and_reject_invalid_command() {
    let catalog = CommandCatalog::default();

    assert_eq!(
        catalog.parse(["hawk2ui", "new"]).unwrap(),
        CliCommand::NewProject
    );
    assert_eq!(
        catalog.parse(["hawk2ui", "build-release"]).unwrap(),
        CliCommand::BuildRelease
    );

    let error = catalog
        .parse(["hawk2ui", "nope"])
        .expect_err("invalid command should fail");
    assert_eq!(error.exit_code, CliExitCode::Usage);
    assert!(error.message.contains("unknown command"));
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
    let runner = BuildCommandRunner::default();

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
        ".root { color: white; }",
    );
    write_file(&root.join("assets/logo.svg"), "<svg />");

    let execution = WorkspaceCommandRunner::new(&root).execute(CliCommand::BuildRelease);

    assert_eq!(execution.exit_code, CliExitCode::Success);
    assert!(execution.stdout.contains("built production artifact"));
    assert!(execution.stdout.contains("compiled-scripts: 2"));
    assert!(execution.stdout.contains("compiled-styles: 1"));
    assert!(execution.stdout.contains("compiled-assets: 1"));
    assert!(execution.stdout.contains("content-hash: sha256:"));
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
            .contains("materialized plugin package outputs")
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
