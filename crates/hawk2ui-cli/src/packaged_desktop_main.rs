#![forbid(unsafe_code)]
//! Native launcher entrypoint for packaged desktop applications.

fn main() {
    if let Err(error) = hawk2ui_cli::run_packaged_desktop_from_default_location() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
