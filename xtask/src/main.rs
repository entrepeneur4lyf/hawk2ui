#![forbid(unsafe_code)]
//! Workspace maintenance and release automation commands for `Hawk2UI`.

use std::process::Command as ProcessCommand;

mod npm_packages;
mod release;

const CRATE_NAME: &str = "xtask";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Command {
    CheckFast,
    Check,
    ReleaseCheck(release::ReleaseCheckMode),
    NpmPackagesVerify,
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

    let rest = args.map(|arg| arg.as_ref().to_owned()).collect::<Vec<_>>();

    match command.as_ref() {
        "check-fast" if rest.is_empty() => Ok(Command::CheckFast),
        "check" if rest.is_empty() => Ok(Command::Check),
        "release-check" => parse_release_check(&rest),
        "npm-packages" if rest == ["--verify"] => Ok(Command::NpmPackagesVerify),
        "npm-packages" => Err(format!("npm-packages requires --verify\n{}", usage())),
        "check-fast" | "check" => Err(format!("too many arguments\n{}", usage())),
        unknown => Err(format!("unknown command '{unknown}'\n{}", usage())),
    }
}

fn parse_release_check(args: &[String]) -> Result<Command, String> {
    match args {
        [] => Ok(Command::ReleaseCheck(release::ReleaseCheckMode::Full)),
        [flag] if flag == "--version-only" => Ok(Command::ReleaseCheck(
            release::ReleaseCheckMode::VersionOnly,
        )),
        [flag] if flag == "--packages-only" => Ok(Command::ReleaseCheck(
            release::ReleaseCheckMode::PackagesOnly,
        )),
        [flag] if flag == "--changelog-only" => Ok(Command::ReleaseCheck(
            release::ReleaseCheckMode::ChangelogOnly,
        )),
        [_] => Err(format!("unknown release-check flag\n{}", usage())),
        _ => Err(format!("too many arguments\n{}", usage())),
    }
}

fn usage() -> String {
    format!(
        "Usage: {CRATE_NAME} <check-fast|check|release-check [--version-only|--packages-only|--changelog-only]|npm-packages --verify>"
    )
}

fn run_command(command: Command) -> Result<(), String> {
    let Some(script) = (match command {
        Command::CheckFast => Some("scripts/check-fast.sh"),
        Command::Check => Some("scripts/check.sh"),
        Command::ReleaseCheck(mode) => return release::run_release_check(mode),
        Command::NpmPackagesVerify => return npm_packages::verify_generated_packages(),
    }) else {
        return Ok(());
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

    const EXPECTED_USAGE: &str = "Usage: xtask <check-fast|check|release-check [--version-only|--packages-only|--changelog-only]|npm-packages --verify>";

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
    fn parses_version_only_release_check_command() {
        let command = parse_command(["xtask", "release-check", "--version-only"]);
        assert_eq!(
            command,
            Ok(Command::ReleaseCheck(
                release::ReleaseCheckMode::VersionOnly
            ))
        );
    }

    #[test]
    fn parses_packages_only_release_check_command() {
        let command = parse_command(["xtask", "release-check", "--packages-only"]);
        assert_eq!(
            command,
            Ok(Command::ReleaseCheck(
                release::ReleaseCheckMode::PackagesOnly
            ))
        );
    }

    #[test]
    fn parses_changelog_only_release_check_command() {
        let command = parse_command(["xtask", "release-check", "--changelog-only"]);
        assert_eq!(
            command,
            Ok(Command::ReleaseCheck(
                release::ReleaseCheckMode::ChangelogOnly
            ))
        );
    }

    #[test]
    fn parses_npm_packages_verify_command() {
        let command = parse_command(["xtask", "npm-packages", "--verify"]);
        assert_eq!(command, Ok(Command::NpmPackagesVerify));
    }

    #[test]
    fn rejects_npm_packages_without_verify_flag() {
        let error = parse_command(["xtask", "npm-packages"]).expect_err("flag is required");
        assert!(error.contains("npm-packages requires --verify"));
    }

    #[test]
    fn rejects_unknown_command_with_usage() {
        let error = parse_command(["xtask", "wat"]).expect_err("unknown command must fail");
        assert!(error.contains("unknown command 'wat'"));
        assert!(error.contains(EXPECTED_USAGE));
    }

    #[test]
    fn rejects_missing_command_with_usage() {
        let error = parse_command(["xtask"]).expect_err("missing command must fail");
        assert!(error.contains("missing command"));
        assert!(error.contains(EXPECTED_USAGE));
    }

    #[test]
    fn rejects_extra_arguments_with_usage() {
        let error = parse_command(["xtask", "check", "again"]).expect_err("extra args must fail");
        assert!(error.contains("too many arguments"));
        assert!(error.contains(EXPECTED_USAGE));
    }

    #[test]
    fn exposes_binary_identity() {
        assert_eq!(CRATE_NAME, "xtask");
    }
}
