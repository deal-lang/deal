//! deal-lsp binary entrypoint.
//!
//! Boots the tower-lsp Server on stdin/stdout per RESEARCH §1 lines 161–167.
//! The VS Code extension (deal-lsp client, Plan 03-05) spawns this binary
//! and speaks JSON-RPC over the inherited pipes.

use deal_lsp::Backend;
use tower_lsp::{LspService, Server};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    // Logs go to stderr (stdout is reserved for JSON-RPC).
    //
    // Verbosity precedence (highest first):
    //   1. `--log=LEVEL` CLI flag — the VS Code client passes `--log=debug` on
    //      its debug launch config (client.ts). Previously this flag was
    //      silently ignored because main() parsed no args, so debug-mode
    //      launches got no extra logging.
    //   2. `DEAL_LSP_LOG` env var (e.g. DEAL_LSP_LOG=trace ./deal-lsp).
    //   3. Default: "info".
    let cli_level: Option<String> = std::env::args().skip(1).find_map(|arg| {
        arg.strip_prefix("--log=").map(|l| l.to_string())
    });
    let env_filter = match cli_level {
        Some(level) => EnvFilter::new(level),
        None => EnvFilter::try_from_env("DEAL_LSP_LOG")
            .unwrap_or_else(|_| EnvFilter::new("info")),
    };
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(std::io::stderr)
        .init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
