//! `vnoc`: the core process. The editor extension starts it and speaks LSP-framed JSON-RPC on its stdin / stdout.
//!
//! **stdout IS the protocol.** Nothing else may write there; logs go to stderr (`VNOC_LOG=debug` for more).
#![deny(clippy::print_stdout)]

use async_lsp::MainLoop;
use async_lsp::concurrency::ConcurrencyLayer;
use async_lsp::panic::CatchUnwindLayer;
use async_lsp::server::LifecycleLayer;
use async_lsp::tracing::TracingLayer;
use tower::ServiceBuilder;

#[tokio::main]
async fn main() {
    let filter = tracing_subscriber::EnvFilter::try_from_env("VNOC_LOG").unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn,vnoc=info"));
    tracing_subscriber::fmt().with_env_filter(filter).with_writer(std::io::stderr).with_ansi(false).init();

    let (server, _client) = MainLoop::new_server(|_client| {
        ServiceBuilder::new()
            .layer(TracingLayer::default())
            .layer(LifecycleLayer::default())
            .layer(CatchUnwindLayer::default()) // a panicking handler becomes an error response, not a dead core
            .layer(ConcurrencyLayer::default())
            .service(vnoc::server::router())
    });

    #[cfg(unix)]
    let (stdin, stdout) = (async_lsp::stdio::PipeStdin::lock_tokio().expect("stdin is a pipe"), async_lsp::stdio::PipeStdout::lock_tokio().expect("stdout is a pipe"));
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
