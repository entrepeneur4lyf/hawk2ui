#![forbid(unsafe_code)]
//! Workspace maintenance and release automation commands for `Hawk2UI`.

use std::process::Command as ProcessCommand;

const CRATE_NAME: &str = "xtask";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Command {
    CheckFast,
    Check,
}

fn main() {
    match parse_command(std::env::args()).and_then(run_command) {
        Ok(()) => {}
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    }
}

fn parse_command<I, S>(args: I) -> Result<Command, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut args = args.into_iter();
    let _program = args.next();
    let Some(command) = args.next() else {
        return Err(format!("missing command\n{}", usage()));
    };

    if args.next().is_some() {
        return Err(format!("too many arguments\n{}", usage()));
    }

    match command.as_ref() {
        "check-fast" => Ok(Command::CheckFast),
        "check" => Ok(Command::Check),
        unknown => Err(format!("unknown command '{unknown}'\n{}", usage())),
    }
}

fn usage() -> String {
    format!("Usage: {CRATE_NAME} <check-fast|check>")
}

fn run_command(command: Command) -> Result<(), String> {
    let script = match command {
        Command::CheckFast => "scripts/check-fast.sh",
        Command::Check => "scripts/check.sh",
    };

    let status = ProcessCommand::new("bash")
        .arg(script)
        .status()
        .map_err(|error| format!("failed to run {script}: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("{script} failed with {status}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_check_fast_command() {
        let command = parse_command(["xtask", "check-fast"]);
        assert_eq!(command, Ok(Command::CheckFast));
    }

    #[test]
    fn parses_check_command() {
        let command = parse_command(["xtask", "check"]);
        assert_eq!(command, Ok(Command::Check));
    }

    #[test]
    fn rejects_unknown_command_with_usage() {
        let error = parse_command(["xtask", "wat"]).expect_err("unknown command must fail");
        assert!(error.contains("unknown command 'wat'"));
        assert!(error.contains("Usage: xtask <check-fast|check>"));
    }

    #[test]
    fn rejects_missing_command_with_usage() {
        let error = parse_command(["xtask"]).expect_err("missing command must fail");
        assert!(error.contains("missing command"));
        assert!(error.contains("Usage: xtask <check-fast|check>"));
    }

    #[test]
    fn rejects_extra_arguments_with_usage() {
        let error = parse_command(["xtask", "check", "again"]).expect_err("extra args must fail");
        assert!(error.contains("too many arguments"));
        assert!(error.contains("Usage: xtask <check-fast|check>"));
    }

    #[test]
    fn exposes_binary_identity() {
        assert_eq!(CRATE_NAME, "xtask");
    }
}
