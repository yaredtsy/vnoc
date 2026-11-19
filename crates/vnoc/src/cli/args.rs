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
    /// Prints what V-NOC knows of each node you name: where it is, the notes (docs) attached to it, and its code. For a file or a folder, the
    /// notes and what it holds. Run it before you change a function: the notes say why it is the way it is. A node whose code is gone prints
    /// MISSING with its notes. One that cannot be found does not stop the others: its error is printed in place and the exit code is 2.
    Show(ShowArgs),
    /// Prints only the code of each function or class you name, with line numbers. Use it to read exactly one def instead of the whole file.
    /// A file, a folder, or a node whose code is gone has no code: that is an error (exit 2), and `vnoc show` prints its notes.
    Code(CodeArgs),
}

/// The nodes a command takes: any number, named in any of the four ways.
#[derive(Debug, Args)]
pub struct Nodes {
    /// A function, class, file or folder, as `src/orders.py::OrderService.parse_order` (`#2` for a second def of the same name),
    /// `src/orders.py:142` (the innermost def holding that line), the node's ID, or a path. A relative path starts at the current folder, else
    /// at the root. `-` reads one name per line from standard input.
    #[arg(required = true, value_name = "NODE")]
    pub nodes: Vec<String>,
}

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[command(flatten)]
    pub nodes: Nodes,
    /// Stop after the notes: no code (and for a file or a folder, no list of what it holds).
    #[arg(long)]
    pub no_code: bool,
    /// Print all the code. By default a def over 120 lines is cut, with the count of what was left out.
    #[arg(long)]
    pub full: bool,
}

#[derive(Debug, Args)]
pub struct CodeArgs {
    #[command(flatten)]
    pub nodes: Nodes,
    /// Lines of the file to add above and below the def (its decorators are already in it).
    #[arg(long, value_name = "N", default_value_t = 0)]
    pub context: u32,
}
