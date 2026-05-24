use hawk2ui_cli::{CliCommand, CliExitCode, CommandCatalog};

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
