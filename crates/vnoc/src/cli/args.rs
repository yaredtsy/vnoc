//! The command line's types (clap, derive). The help text is part of the interface: each command's `about` is written for an agent that
//! reads it cold, so it says what the command tells you and when to run it.
//!
//! The server is not here: `main.rs` decides `vnoc`, `vnoc --stdio`, `vnoc serve` and `vnoc serve --stdio` before clap sees anything, and
//! `--stdio` is not a flag of this parser (05 §1).

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// V-NOC reads a project by structure: the functions and classes of its code, and the notes attached to them.
///
/// EXPERIMENTAL until 1.0: command names and the text output can still change (the release notes say when). `--json` only ever gains fields.
/// Every path printed is relative to the root, except the root itself. Exit codes: 0 done (also "found nothing"), 1 `check` found problems,
/// 2 the request cannot be met (bad arguments, no such node), 3 the environment (no driver, a driver that failed).
#[derive(Debug, Parser)]
#[command(
    name = "vnoc",
    version,
    about,
    long_about,
    disable_help_subcommand = true
)]
pub struct Cli {
    #[command(flatten)]
    pub global: Global,
    #[command(subcommand)]
    pub command: Command,
}

/// The flags every command takes.
#[derive(Debug, Args)]
pub struct Global {
    /// The project folder. Default: the nearest folder at or above the current one that has `.git`, else one that has `.vnoc/`, else the current folder.
    #[arg(long, global = true, value_name = "DIR")]
    pub root: Option<PathBuf>,
    /// The language driver to run. Default: `$VNOC_DRIVER`, else `vnoc-driver-python` next to this binary.
    #[arg(long, global = true, value_name = "PATH")]
    pub driver: Option<PathBuf>,
    /// Print one JSON value, `{ "schema": 1, "command": …, … }`, instead of text.
    #[arg(long, global = true, conflicts_with = "format")]
    pub json: bool,
    /// For commands that print a list: one ID per line (`id`) or one file path per line (`path`), and nothing else.
    #[arg(long, global = true, value_enum, value_name = "FORMAT")]
    pub format: Option<Format>,
    /// Print only the rows: no hints about what to run next, no totals.
    #[arg(long, global = true)]
    pub quiet: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Id,
    Path,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Tells you whether V-NOC works in this project and what it knows: its version, the root, the driver, how many folders, files,
    /// classes, functions and docs there are, and what is Missing or broken. Run it first when anything seems off, or to see whether a
    /// project has docs at all.
    Status,
}
