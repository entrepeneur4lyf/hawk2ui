#![forbid(unsafe_code)]
//! Command-line interface for `Hawk2UI` validation, builds, runs, packaging, and diagnostics.

use hawk2ui_cli::{CliExitCode, CommandCatalog};

fn main() {
    let catalog = CommandCatalog;
    match catalog.parse(std::env::args()) {
        Ok(command) => {
            println!("{command:?}");
        }
        Err(error) => {
            eprintln!("{}", error.message);
            std::process::exit(error.exit_code as i32);
        }
    }
}
