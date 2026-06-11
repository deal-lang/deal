# deal-lsp

Rust crate implementing the Language Server Protocol for the DEAL language.
Drives diagnostics, formatting, completion, hover, goto-definition, semantic
tokens, and workspace symbols.

> NOTE: this crate supersedes the stale TypeScript design sketch at
> `/Users/dunnock/projects/deal-lang/lsp/README.md`. Phase 03 Plan 07 retires
> that file. The Rust implementation here is the only deal-lsp going forward
> (D-49 architectural split: Rust workspace tooling, Zig core single-file-scoped).

## Architecture

```
                 +----------------------+
   stdin/stdout  |   tower-lsp Server   |   (Plan 03-03)
   <----JSON--->|     (Backend impl)   |
                 +----------+-----------+
                            |
                +-----------+------------+
                |     Documents          |   DashMap<Url, Arc<Mutex<...>>>
                |   (D-42 per-doc        |   buffers: DashMap<Url, Rope>
                |    handle table,       |   parse_count: AtomicUsize (test hook)
                |    D-44 persists for   |
                |    workspace lifetime) |
                +-----------+------------+
                            |
                  deal_ffi::safe::*       (Plan 03-03 Task 1 extracted crate)
                            |
                 +----------+-----------+
                 |   libdeal.a (Zig)    |   9 extern "C" exports
                 +----------------------+
```

## Capabilities

All capabilities below are implemented and wired into `backend.rs`.

| Capability                            | Status   | Notes                                                        |
| ------------------------------------- | -------- | ------------------------------------------------------------ |
| text_document_sync = FULL             | shipped  | D-43 (full-buffer per change)                                |
| publishDiagnostics (push)             | shipped  | RESEARCH §11; Phase 2 E-codes via `diagnostics.rs`           |
| textDocument/formatting               | shipped  | D-21 in-memory format                                        |
| textDocument/completion               | shipped  | `completion.rs` — 11 element keywords + workspace types      |
| textDocument/hover                    | shipped  | `hover.rs` — doc comments + `kind name` signature fallback   |
| textDocument/definition               | shipped  | `definition.rs` — cross-file via the in-memory `Index`       |
| textDocument/semanticTokens (+delta)  | shipped  | `semantic_tokens.rs` — full + delta                          |
| workspace/symbol                      | shipped  | `backend::symbol` → `Index::workspace_symbols`               |

## Design Locks

| Lock  | Description                                                                 |
| ----- | --------------------------------------------------------------------------- |
| D-21  | In-memory formatting — reuse live handle, return single TextEdit.           |
| D-42  | Per-document `DealHandle*` table keyed by URI.                              |
| D-43  | 300 ms debounce window for did_change → re-parse (`DEBOUNCE_MS` const).     |
| D-44  | did_close is a NO-OP; handles persist for workspace lifetime.               |
| D-45  | Per-handle `tokio::sync::Mutex`; `OwnedDealHandle: Send` is safe.           |
| D-49  | Rust workspace tooling, Zig core single-file-scoped.                        |

## FFI consumption (Plan 03-03 Task 1)

The 9-export C ABI is provided by the sibling `deal-ffi` crate (single
`links = "deal"` claimant in the Cargo workspace). deal-lsp imports the
`OwnedDealHandle` newtype and the `deal_ffi::safe::*` helpers, which
encapsulate the clone-before-free invariant (Pitfall 3 / T-02-29) so the
LSP call sites never see raw pointers.

| Safe wrapper                          | Used by                                |
| ------------------------------------- | -------------------------------------- |
| `deal_ffi::safe::parse`               | `documents::open` / `documents::update` |
| `deal_ffi::safe::has_errors`          | `formatting::handle_formatting`         |
| `deal_ffi::safe::diagnostics_json`    | `documents::{open,update}`              |
| `deal_ffi::safe::format`              | `formatting::handle_formatting`         |

Completion / hover / definition / semantic-tokens / workspace-symbol
additionally consume `ast_json` and `index_json` through the same safe
wrappers.

## Build

```bash
cd /Users/dunnock/projects/deal-lang/deal
cargo build -p deal-lsp --release
# Binary lands at target/release/deal-lsp
```

## Test

```bash
# Library + crate unit tests (~2-5s)
cargo test -p deal-lsp --lib

# Integration tests against the showcase (Plan 03-03 Task 3)
cargo test -p deal-lsp --test showcase -- --test-threads=1
```

## Dependencies (pinned)

| Crate              | Version | Source                                              |
| ------------------ | ------- | --------------------------------------------------- |
| tower-lsp          | 0.20.0  | RESEARCH §1 + Issue 1 pin (verified via cargo search) |
| tokio              | 1.x     | full features                                       |
| dashmap            | 6       | per-URI handle table                                |
| ropey              | 1.6     | full-document range computation                     |
| async-trait        | 0.1     | tower-lsp trait sugar                               |
| tracing            | 0.1     | structured logs to stderr (stdout reserved for JSON-RPC) |
| tracing-subscriber | 0.3     | env-filter via DEAL_LSP_LOG                         |

## Logging

Verbosity precedence: `--log=LEVEL` flag > `DEAL_LSP_LOG` env > `info` default.

```bash
DEAL_LSP_LOG=debug ./target/release/deal-lsp
./target/release/deal-lsp --log=debug   # e.g. the VS Code debug launch config
```
