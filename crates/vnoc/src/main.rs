//! `vnoc`: the core process, and the command line.
//!
//! The editor extension starts it as `vnoc --stdio` (vscode-languageclient adds the flag) and speaks LSP-framed JSON-RPC on its stdin / stdout.
//! `vnoc --stdio`, `vnoc serve` and `vnoc serve --stdio` run that main loop, and so does `vnoc` with no arguments **when its standard input and
//! output are pipes**: a person who types `vnoc` (a terminal), or runs it with `< /dev/null` or a file, gets the help, not a server that waits for
//! LSP frames. The explicit forms with streams that are not pipes are refused with a sentence, never a panic. Anything else is a command
//! (`vnoc status`). "Pipe" is what the server's reader takes: a FIFO or a socket (Node, which runs the editor's extension, hands a child a socket).
//!
//! **In the server, stdout IS the protocol.** Nothing else may write there; logs go to stderr (`VNOC_LOG=debug` for more).
#![deny(clippy::print_stdout)]

use std::ffi::OsString;
use std::io::IsTerminal;

use async_lsp::MainLoop;
use async_lsp::concurrency::ConcurrencyLayer;
use async_lsp::panic::CatchUnwindLayer;
use async_lsp::server::LifecycleLayer;
use async_lsp::tracing::TracingLayer;
use tower::ServiceBuilder;

/// What a standard stream is, as far as the server cares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stream {
    Terminal,
    /// a FIFO or a socket: what the server can read and write (on Windows: anything that is not a terminal)
    Pipe,
    /// a regular file, `/dev/null`, a device
    Other,
}

#[cfg(unix)]
fn stream_of(fd: std::os::fd::BorrowedFd<'_>, terminal: bool) -> Stream {
    use std::os::unix::fs::FileTypeExt;
    if terminal {
        return Stream::Terminal;
    }
    match fd
        .try_clone_to_owned()
        .and_then(|fd| std::fs::File::from(fd).metadata())
    {
        Ok(meta) if meta.file_type().is_fifo() || meta.file_type().is_socket() => Stream::Pipe,
        _ => Stream::Other,
    }
}

#[cfg(unix)]
fn standard_streams() -> (Stream, Stream) {
    use std::os::fd::AsFd;
    let (stdin, stdout) = (std::io::stdin(), std::io::stdout());
    (
        stream_of(stdin.as_fd(), stdin.is_terminal()),
        stream_of(stdout.as_fd(), stdout.is_terminal()),
    )
}

#[cfg(not(unix))]
fn standard_streams() -> (Stream, Stream) {
    let of = |terminal| {
        if terminal {
            Stream::Terminal
        } else {
            Stream::Pipe
        }
    };
    (
        of(std::io::stdin().is_terminal()),
        of(std::io::stdout().is_terminal()),
    )
}

/// What `main` does with these arguments.
#[derive(Debug, PartialEq, Eq)]
enum Start {
    /// the main loop
    Server,
    /// no arguments, and not an editor on the other end: the short help
    Help,
    /// the server was asked for with streams it cannot use: a sentence and exit 2
    Refuse,
    /// the command line
    Command,
}

/// What the arguments (without the program's name) and the standard streams make `vnoc` do. Decided **before** clap: the extension's `--stdio` is not a
/// flag of the command line, and it must never reach the parser as an unknown one. No arguments start the server only when both streams are pipes;
/// otherwise they print the help. `--stdio` and `serve` start it whatever the streams are, except that with `strict_pipes` (Unix: the server's reader
/// panics on anything else) streams that are not pipes are refused.
fn start(args: &[OsString], stdin: Stream, stdout: Stream, strict_pipes: bool) -> Start {
    let pipes = stdin == Stream::Pipe && stdout == Stream::Pipe;
    let args: Vec<Option<&str>> = args.iter().map(|a| a.to_str()).collect();
    match args.as_slice() {
        // on Windows the first form is "not a terminal", as before
        [] if strict_pipes => {
            if pipes {
                Start::Server
            } else {
                Start::Help
            }
        }
        [] => {
            if stdin == Stream::Terminal {
                Start::Help
            } else {
                Start::Server
            }
        }
        [Some("--stdio")] | [Some("serve")] | [Some("serve"), Some("--stdio")] => {
            if pipes || !strict_pipes {
                Start::Server
            } else {
                Start::Refuse
            }
        }
        _ => Start::Command,
    }
}

/// The sentence for `--stdio` where there are no pipes (exit 2), and for a reader or writer the server could not make of them.
const NOT_PIPES: &str =
    "vnoc --stdio is for an editor, which talks to it through pipes; for a command, see vnoc -h";

fn refuse() -> ! {
    eprintln!("error: {NOT_PIPES}");
    std::process::exit(2)
}

/// Logs go to standard error: `VNOC_LOG`, else `default`.
fn init_logging(default: &str) {
    let filter = tracing_subscriber::EnvFilter::try_from_env("VNOC_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();
}

fn main() {
    let mut args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let (stdin, stdout) = standard_streams();
    let server = match start(&args, stdin, stdout, cfg!(unix)) {
        Start::Server => true,
        Start::Command => false,
        Start::Help => {
            // bare `vnoc` with no editor on the other end: the short help, as `vnoc -h` prints it
            args.push(OsString::from("-h"));
            false
        }
        Start::Refuse => refuse(),
    };
    if !server && std::env::var_os("VNOC_LOG").is_none() {
        // A command says nothing on standard error unless it fails (05 §4), and the driver it starts reads the same variable: its own
        // default is `info`. Set before the runtime exists, so no other thread is reading the environment.
        // SAFETY: this is the only thread; the runtime, and with it every other thread, is built below.
        unsafe { std::env::set_var("VNOC_LOG", "warn") };
    }
    init_logging(if server { "warn,vnoc=info" } else { "warn" });
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("a tokio runtime");
    runtime.block_on(async {
        if server {
            serve().await;
        } else {
            std::process::exit(vnoc::cli::run(args).await);
        }
    });
}

async fn serve() {
    let (server, _client) = MainLoop::new_server(|client| {
        ServiceBuilder::new()
            .layer(TracingLayer::default())
            .layer(LifecycleLayer::default())
            .layer(CatchUnwindLayer::default()) // a panicking handler becomes an error response, not a dead core
            .layer(ConcurrencyLayer::default())
            .service(vnoc::server::router(client))
    });

    #[cfg(unix)]
    let (stdin, stdout) = (
        async_lsp::stdio::PipeStdin::lock_tokio().unwrap_or_else(|_| refuse()),
        async_lsp::stdio::PipeStdout::lock_tokio().unwrap_or_else(|_| refuse()),
    );
    #[cfg(not(unix))]
    let (stdin, stdout) = (
        tokio_util::compat::TokioAsyncReadCompatExt::compat(tokio::io::stdin()),
        tokio_util::compat::TokioAsyncWriteCompatExt::compat_write(tokio::io::stdout()),
    );

    if let Err(e) = server.run_buffered(stdin, stdout).await {
        tracing::error!("the main loop ended with an error: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Stream::{Other, Pipe, Terminal};

    fn started(args: &[&str], stdin: Stream, stdout: Stream, strict: bool) -> Start {
        start(
            &args.iter().map(OsString::from).collect::<Vec<_>>(),
            stdin,
            stdout,
            strict,
        )
    }

    /// (arguments, what stdin is, what stdout is) → what `vnoc` does, on Unix.
    #[test]
    fn what_vnoc_does_as_this_table_says_on_unix() {
        use Start::{Command, Help, Refuse, Server};
        let table: [(&[&str], Stream, Stream, Start); 22] = [
            // no arguments: the server only when an editor is on the other end of two pipes
            (&[], Pipe, Pipe, Server),
            (&[], Terminal, Terminal, Help),
            (&[], Terminal, Pipe, Help),
            (&[], Other, Pipe, Help),    // `vnoc < /dev/null`, or a file
            (&[], Pipe, Terminal, Help), // `echo | vnoc`
            (&[], Pipe, Other, Help),
            (&[], Other, Other, Help),
            // the explicit forms: the server on pipes, a sentence otherwise
            (&["--stdio"], Pipe, Pipe, Server),
            (&["--stdio"], Other, Pipe, Refuse),
            (&["--stdio"], Terminal, Terminal, Refuse),
            (&["--stdio"], Pipe, Other, Refuse),
            (&["serve"], Pipe, Pipe, Server),
            (&["serve"], Other, Other, Refuse),
            (&["serve", "--stdio"], Pipe, Pipe, Server),
            (&["serve", "--stdio"], Pipe, Terminal, Refuse),
            // everything else is a command, whatever the streams
            (&["status"], Pipe, Pipe, Command),
            (&["status"], Terminal, Terminal, Command),
            (&["--stdio", "status"], Pipe, Pipe, Command),
            (&["--help"], Other, Other, Command),
            (&["serve", "extra"], Pipe, Pipe, Command),
            (&["--stdio", "--stdio"], Pipe, Pipe, Command),
            (&["status"], Other, Other, Command),
        ];
        for (args, stdin, stdout, expected) in table {
            assert_eq!(
                started(args, stdin, stdout, true),
                expected,
                "{args:?}, stdin {stdin:?}, stdout {stdout:?}"
            );
        }
    }

    /// Windows keeps "not a terminal": nothing is refused, and no arguments start the server unless stdin is a terminal.
    #[test]
    fn what_vnoc_does_on_windows_is_what_it_was() {
        use Start::{Command, Help, Server};
        assert_eq!(started(&[], Pipe, Pipe, false), Server);
        assert_eq!(started(&[], Pipe, Terminal, false), Server);
        assert_eq!(started(&[], Terminal, Terminal, false), Help);
        for args in [&["--stdio"][..], &["serve"], &["serve", "--stdio"]] {
            assert_eq!(started(args, Terminal, Terminal, false), Server, "{args:?}");
        }
        assert_eq!(started(&["status"], Terminal, Terminal, false), Command);
    }

    #[cfg(unix)]
    #[test]
    fn a_fifo_and_a_socket_are_pipes_and_a_null_device_and_a_file_are_not() {
        use std::os::fd::AsFd;
        let null = std::fs::File::open("/dev/null").unwrap();
        assert_eq!(stream_of(null.as_fd(), false), Other);
        let file =
            std::fs::File::open(env!("CARGO_MANIFEST_DIR").to_string() + "/Cargo.toml").unwrap();
        assert_eq!(stream_of(file.as_fd(), false), Other);
        let (socket, _other) = std::os::unix::net::UnixStream::pair().unwrap();
        assert_eq!(stream_of(socket.as_fd(), false), Pipe);
        let (read, _write) = std::io::pipe().unwrap();
        assert_eq!(stream_of(read.as_fd(), false), Pipe);
        assert_eq!(stream_of(null.as_fd(), true), Terminal);
    }
}
