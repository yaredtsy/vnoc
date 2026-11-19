//! `vnoc`: the core process, and the command line.
//!
//! The editor extension starts it as `vnoc --stdio` (vscode-languageclient adds the flag) and speaks LSP-framed JSON-RPC on its stdin / stdout.
//! `vnoc`, `vnoc --stdio`, `vnoc serve` and `vnoc serve --stdio` all run that main loop. Anything else is a command (`vnoc status`).
//!
//! **In the server, stdout IS the protocol.** Nothing else may write there; logs go to stderr (`VNOC_LOG=debug` for more).
#![deny(clippy::print_stdout)]

use std::ffi::OsString;

use async_lsp::MainLoop;
use async_lsp::concurrency::ConcurrencyLayer;
use async_lsp::panic::CatchUnwindLayer;
use async_lsp::server::LifecycleLayer;
use async_lsp::tracing::TracingLayer;
use tower::ServiceBuilder;

/// Whether these arguments (without the program's name) start the server. Decided **before** clap: the extension's `--stdio` is not a flag
/// of the command line, and it must never reach the parser as an unknown one.
fn starts_server(args: &[OsString]) -> bool {
    let args: Vec<Option<&str>> = args.iter().map(|a| a.to_str()).collect();
    matches!(
        args.as_slice(),
        [] | [Some("--stdio")] | [Some("serve")] | [Some("serve"), Some("--stdio")]
    )
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
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let server = starts_server(&args);
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
        async_lsp::stdio::PipeStdin::lock_tokio().expect("stdin is a pipe"),
        async_lsp::stdio::PipeStdout::lock_tokio().expect("stdout is a pipe"),
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

    fn starts(args: &[&str]) -> bool {
        starts_server(&args.iter().map(OsString::from).collect::<Vec<_>>())
    }

    #[test]
    fn the_four_ways_to_start_the_server() {
        assert!(starts(&[]));
        assert!(starts(&["--stdio"]));
        assert!(starts(&["serve"]));
        assert!(starts(&["serve", "--stdio"]));
    }

    #[test]
    fn anything_else_is_a_command() {
        assert!(!starts(&["status"]));
        assert!(!starts(&["--stdio", "status"]));
        assert!(!starts(&["--help"]));
        assert!(!starts(&["serve", "extra"]));
        assert!(!starts(&["--stdio", "--stdio"]));
    }
}
