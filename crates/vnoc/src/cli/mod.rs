//! The command line: `vnoc <command>`, over the same library the server uses (docs/21-agent-cli/05-how-it-runs.md).
//!
//! A command is three things: parse, call, print. `run` parses, one function per command calls the library and prints through `out`, and
//! one function, [`exit_code`], says what each failure exits with (C6).

mod agent;
mod args;
mod calls;
mod check;
mod code;
mod doc;
mod find;
mod grep;
pub mod guide;
mod nodes;
mod out;
pub mod project;
mod relink;
mod show;
mod status;
pub mod style;
mod tags;
mod tour;
mod tree;
mod write;

use std::ffi::OsString;

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
    /// no driver found, no driver started, a `vnoc.toml` that is too new, a lock that could not be taken
    #[error("{0}")]
    Environment(String),
    /// the request could not be met for some of what it named, and each was **already** printed in its place (`show a.py b.py`: `b.py` is not
    /// there): nothing more to say, exit `2`
    #[error("some nodes were not found")]
    Reported,
    /// the reader stopped the tour (02 §5): the refusal was **already** printed in its place, as the answer, and the exit is `1`
    #[error("the reader stopped this tour")]
    Stopped,
    /// the reader of standard output went away (`vnoc tree | head`): the command ends quietly, successfully
    #[error("standard output was closed")]
    Closed,
}

/// C6 (05 §5): `0` done · `1` `check` found problems, or the reader stopped the tour · `2` the request cannot be met · `3` the environment.
pub fn exit_code(failure: &Failure) -> i32 {
    match failure {
        Failure::Closed => 0,
        Failure::CheckFailed | Failure::Stopped => 1,
        Failure::Usage(_) | Failure::NoSuchNode(_) | Failure::Reported => 2,
        Failure::Environment(_) => 3,
    }
}

/// Runs the command line `args` (without the program's name) and returns the exit code. clap's own usage errors are `2` and `--help` /
/// `--version` are `0`, as clap has them.
pub async fn run(args: Vec<OsString>) -> i32 {
    // `--color` is read before clap parses, because clap's own output (`--help`, a usage error) is painted by it too
    style::init(style::when_in(&args));
    let words: Vec<String> = args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let cli = match Cli::parse_args(std::iter::once(OsString::from("vnoc")).chain(args)) {
        Ok(cli) => cli,
        Err(mut e) => {
            add_filter_hint(&mut e, &words);
            return out::clap_error(&e);
        }
    };
    match execute(&cli).await {
        Ok(()) => 0,
        Err(failure) => {
            // `check` printed its findings and its summary: they are its answer, and the exit code says the rest
            if !matches!(
                failure,
                Failure::Closed | Failure::Reported | Failure::CheckFailed | Failure::Stopped
            ) {
                out::error(&failure.to_string());
            }
            exit_code(&failure)
        }
    }
}

/// The filters of a `find` query, which people (and agents) write as flags: `vnoc find retry --kind fn`.
const FILTER_NAMES: [&str; 6] = ["kind", "in", "tag", "author", "has", "no"];

/// A usage error for a flag that is a filter's name says where filters go (A14: Codex wrote `find parse_order --kind fn` in one of the 28 runs). `find` takes them in
/// the query, `tree` in `--where`; `tree --kind` is a real flag and is left alone. Exit code and clap's own text are as they were: one `tip:` more.
fn add_filter_hint(e: &mut clap::Error, args: &[String]) {
    use clap::error::{ContextKind, ContextValue, ErrorKind};
    if e.kind() != ErrorKind::UnknownArgument {
        return;
    }
    let Some(refused) = e.get(ContextKind::InvalidArg).map(ToString::to_string) else {
        return;
    };
    let Some(hint) = filter_hint(args, refused.split(' ').next().unwrap_or("")) else {
        return;
    };
    let mut tips = match e.get(ContextKind::Suggested) {
        Some(ContextValue::StyledStrs(tips)) => tips.clone(),
        _ => Vec::new(),
    };
    tips.push(hint.into());
    e.insert(ContextKind::Suggested, ContextValue::StyledStrs(tips));
}

/// `filters go in the query: vnoc find 'retry kind:fn'` for the command line `args` (without `vnoc`) and the flag `refused` clap did not know,
/// or `None` when the command is not `find` or `tree` or the flag is not a filter's name.
fn filter_hint(args: &[String], refused: &str) -> Option<String> {
    let name = refused.strip_prefix("--")?;
    let at = args.iter().position(|a| a == "find" || a == "tree")?;
    let command = args[at].as_str();
    let converts = |n: &str| FILTER_NAMES.contains(&n) && !(command == "tree" && n == "kind");
    if !converts(name) {
        return None;
    }
    let (mut positional, mut filters, mut other_flags) = (Vec::new(), Vec::new(), false);
    let mut rest = args[at + 1..].iter().peekable();
    while let Some(arg) = rest.next() {
        let Some(flag) = arg.strip_prefix("--") else {
            if arg.starts_with('-') {
                other_flags = true;
            } else {
                positional.push(arg.clone());
            }
            continue;
        };
        let (flag_name, inline) = match flag.split_once('=') {
            Some((n, v)) => (n, Some(v.to_string())),
            None => (flag, None),
        };
        if !converts(flag_name) {
            other_flags = true;
            continue;
        }
        let value = inline.or_else(|| rest.next_if(|v| !v.starts_with('-')).cloned());
        filters.push(match value {
            Some(v) => format!("{flag_name}:{v}"),
            None => format!("{flag_name}:"),
        });
    }
    // another flag with a value would make its value look like a word: then only the filters are named
    let words = if other_flags {
        Vec::new()
    } else {
        positional.clone()
    };
    Some(match command {
        "find" => format!(
            "filters go in the query: vnoc find '{}'",
            words
                .into_iter()
                .chain(filters)
                .collect::<Vec<_>>()
                .join(" ")
        ),
        _ => format!(
            "filters go in --where: vnoc tree {}--where '{}'",
            words.iter().map(|w| format!("{w} ")).collect::<String>(),
            filters.join(" ")
        ),
    })
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
        Command::Init(args) => write::init(&cli.global, args).await,
        Command::Relink(args) => relink::relink(&cli.global, args).await,
        Command::Repair(args) => relink::repair(&cli.global, args).await,
        Command::Missing => check::missing(&cli.global).await,
        Command::Check(args) => check::run(&cli.global, args).await,
        Command::Tags(args) => tags::tags(&cli.global, args).await,
        Command::Tag(command) => tags::tag_command(&cli.global, command).await,
        Command::Guide(args) => guide::guide(&cli.global, args),
        Command::Skill => guide::skill(&cli.global),
        Command::Agent(command) => agent::run(&cli.global, command),
        Command::Tour(command) => tour::run(&cli.global, command).await,
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
            (Failure::Stopped, 1),
            (Failure::Usage("x".into()), 2),
            (Failure::NoSuchNode("x".into()), 2),
            (Failure::Reported, 2),
            (Failure::Environment("x".into()), 3),
        ];
        for (failure, code) in table {
            assert_eq!(exit_code(&failure), code, "{failure:?}");
        }
    }

    #[test]
    fn a_filter_written_as_a_flag_is_turned_into_its_filter_form() {
        let hint = |line: &str, refused: &str| {
            let args: Vec<String> = line.split(' ').map(String::from).collect();
            filter_hint(&args, refused)
        };
        assert_eq!(
            hint("find --kind fn", "--kind").unwrap(),
            "filters go in the query: vnoc find 'kind:fn'"
        );
        assert_eq!(
            hint("find retry --tag x --no doc", "--tag").unwrap(),
            "filters go in the query: vnoc find 'retry tag:x no:doc'"
        );
        assert_eq!(
            hint("find --author=me", "--author").unwrap(),
            "filters go in the query: vnoc find 'author:me'"
        );
        // another flag with a value: only the filters are named
        assert_eq!(
            hint("find retry --limit 5 --in billing/", "--in").unwrap(),
            "filters go in the query: vnoc find 'in:billing/'"
        );
        assert_eq!(
            hint("tree billing --tag plan", "--tag").unwrap(),
            "filters go in --where: vnoc tree billing --where 'tag:plan'"
        );
        // `tree --kind` is a real flag; other flags and other commands get no hint
        assert_eq!(hint("tree --kind fn", "--kind"), None);
        assert_eq!(hint("find x --bogus", "--bogus"), None);
        assert_eq!(hint("grep x --kind fn", "--kind"), None);
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
