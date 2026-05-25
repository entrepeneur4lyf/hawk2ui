#![forbid(unsafe_code)]
//! Command-line interface for `Hawk2UI` validation, builds, runs, packaging, and diagnostics.

use hawk2ui_cli::{CommandCatalog, WorkspaceCommandRunner};

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
            let execution = WorkspaceCommandRunner::new(root).execute(command);
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
