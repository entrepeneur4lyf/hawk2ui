#![forbid(unsafe_code)]
//! Command-line interface for `Hawk2UI` validation, builds, runs, packaging, and diagnostics.

use hawk2ui_cli::{CliExitCode, CommandCatalog, CommandExecution, WorkspaceCommandRunner};

fn main() {
    let catalog = CommandCatalog;
    match catalog.parse(std::env::args()) {
        Ok(command) => {
            let root = match std::env::current_dir() {
                Ok(root) => root,
                Err(error) => {
                    eprintln!("failed to resolve current directory: {error}");
                    std::process::exit(12);
                }
            };
            let runner = match WorkspaceCommandRunner::new(root)
                .with_desktop_launcher_binary_from_environment()
                .with_release_security_from_environment()
            {
                Ok(runner) => runner,
                Err(diagnostic) => {
                    let execution =
                        CommandExecution::failure(CliExitCode::Verification, vec![*diagnostic]);
                    if !execution.stderr.is_empty() {
                        eprint!("{}", execution.stderr);
                    }
                    std::process::exit(execution.exit_code as i32);
                }
            };
            let execution = runner.with_unbounded_dev_loop().execute(command);
            if !execution.stdout.is_empty() {
                print!("{}", execution.stdout);
            }
            if !execution.stderr.is_empty() {
                eprint!("{}", execution.stderr);
            }
            std::process::exit(execution.exit_code as i32);
        }
        Err(error) => {
            eprintln!("{}", error.message);
            std::process::exit(error.exit_code as i32);
        }
    }
}
