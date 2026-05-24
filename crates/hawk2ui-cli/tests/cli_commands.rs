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
