//! The command line: `vnoc <command>`, over the same library the server uses (docs/21-agent-cli/05-how-it-runs.md).
//!
//! A command is three things: parse, call, print. `run` parses, one function per command calls the library and prints through `out`, and
//! one function, [`exit_code`], says what each failure exits with (C6).

mod args;
mod calls;
mod check;
mod code;
mod doc;
mod find;
mod grep;
mod nodes;
mod out;
pub mod project;
mod show;
mod status;
pub mod style;
mod tree;

use std::ffi::OsString;

use clap::Parser;

pub use args::{Cli, Command, Format, Global};

/// Why a command did not do what it was asked. Each case has one exit code ([`exit_code`]), so a new error cannot pick one by accident.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum Failure {
    /// the request cannot be met because of how it was made: a bad flag, a folder that is not one
    #[error("{0}")]
    Usage(String),
    /// the request names something that is not there
    #[error("{0}")]
    NoSuchNode(String),
    /// `check` found problems (not an error of the command: its answer)
    #[error("check found problems")]
    CheckFailed,
    /// no driver, a driver that failed, a `vnoc.toml` that is too new, a lock that could not be taken
    #[error("{0}")]
    Environment(String),
    /// the request could not be met for some of what it named, and each was **already** printed in its place (`show a.py b.py`: `b.py` is not
    /// there): nothing more to say, exit `2`
    #[error("some nodes were not found")]
    Reported,
    /// the reader of standard output went away (`vnoc tree | head`): the command ends quietly, successfully
    #[error("standard output was closed")]
    Closed,
}

/// C6 (05 §5): `0` done · `1` `check` found problems · `2` the request cannot be met · `3` the environment.
pub fn exit_code(failure: &Failure) -> i32 {
    match failure {
        Failure::Closed => 0,
        Failure::CheckFailed => 1,
        Failure::Usage(_) | Failure::NoSuchNode(_) | Failure::Reported => 2,
        Failure::Environment(_) => 3,
    }
}

/// Runs the command line `args` (without the program's name) and returns the exit code. clap's own usage errors are `2` and `--help` /
/// `--version` are `0`, as clap has them.
pub async fn run(args: Vec<OsString>) -> i32 {
    // `--color` is read before clap parses, because clap's own output (`--help`, a usage error) is painted by it too
    style::init(style::when_in(&args));
    let cli = match Cli::try_parse_from(std::iter::once(OsString::from("vnoc")).chain(args)) {
        Ok(cli) => cli,
        Err(e) => return out::clap_error(&e),
    };
    match execute(&cli).await {
        Ok(()) => 0,
        Err(failure) => {
            // `check` printed its findings and its summary: they are its answer, and the exit code says the rest
            if !matches!(
                failure,
                Failure::Closed | Failure::Reported | Failure::CheckFailed
            ) {
                out::error(&failure.to_string());
            }
            exit_code(&failure)
        }
    }
}

async fn execute(cli: &Cli) -> Result<(), Failure> {
    match &cli.command {
        Command::Status => status::run(&cli.global).await,
        Command::Show(args) => show::run(&cli.global, args).await,
        Command::Code(args) => code::run(&cli.global, args).await,
        Command::Doc(command) => doc::run(&cli.global, command).await,
        Command::Tree(args) => tree::run(&cli.global, args).await,
        Command::Find(args) => find::run(&cli.global, args).await,
        Command::Grep(args) => grep::run(&cli.global, args).await,
        Command::Calls(args) => calls::run(&cli.global, args).await,
        Command::Missing => check::missing(&cli.global).await,
        Command::Check(args) => check::run(&cli.global, args).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_failure_has_the_exit_code_of_c6() {
        let table = [
            (Failure::Closed, 0),
            (Failure::CheckFailed, 1),
            (Failure::Usage("x".into()), 2),
            (Failure::NoSuchNode("x".into()), 2),
            (Failure::Reported, 2),
            (Failure::Environment("x".into()), 3),
        ];
        for (failure, code) in table {
            assert_eq!(exit_code(&failure), code, "{failure:?}");
        }
    }

    #[tokio::test]
    async fn clap_exits_the_way_clap_does() {
        let run = |args: &[&str]| run(args.iter().map(OsString::from).collect());
        assert_eq!(run(&["--help"]).await, 0);
        assert_eq!(run(&["--version"]).await, 0);
        assert_eq!(run(&["status", "--help"]).await, 0);
        assert_eq!(run(&["--no-such-flag"]).await, 2);
        assert_eq!(run(&["no-such-command"]).await, 2);
        assert_eq!(run(&["status", "--format", "nonsense"]).await, 2);
    }
}
