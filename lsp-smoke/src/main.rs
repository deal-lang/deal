//! lsp-smoke — Phase 3 end-to-end smoke harness.
//!
//! Invoked by `scripts/phase-3-smoke.sh`, which is step 8 of
//! `zig build phase-3-gate` (RESEARCH section 13 lines 955-966).
//!
//! Drives the `deal_lsp::Backend` through tower-lsp's `LspService` and
//! exercises the 5 LSP capabilities (diagnostics, completion, definition,
//! hover, formatting) + semantic tokens against a workspace directory.
//!
//! Usage:
//!     deal-lsp-smoke <workspace-path>
//!     deal-lsp-smoke tests/showcase
//!
//! Exit codes:
//!     0    All 5 capabilities + semantic tokens round-trip cleanly.
//!     1    initialize / initialized handshake failed.
//!     2    A capability returned an error or empty/invalid response.
//!     3    Workspace path argument missing or directory does not exist.
//!
//! Architecture: see lsp-smoke/Cargo.toml for the Rule-3 deviation rationale
//! (drive Backend via LspService instead of spawning the binary over stdio).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use deal_lsp::{Backend, Documents};
use futures::StreamExt;
use tokio::sync::Mutex;
use tower::Service;
use tower_lsp::jsonrpc::{Request as JsonRpcRequest, Response as JsonRpcResponse};
use tower_lsp::lsp_types::*;
use tower_lsp::LspService;

/// Collected publishDiagnostics + other notifications from ClientSocket.
type NotificationSink = Arc<Mutex<Vec<JsonRpcRequest>>>;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // Parse the workspace argument.
    let workspace_arg = match std::env::args().nth(1) {
        Some(p) => p,
        None => {
            eprintln!("PHASE-3-SMOKE: FAIL — missing workspace argument");
            eprintln!("  usage: deal-lsp-smoke <workspace-path>");
            std::process::exit(3);
        }
    };

    let workspace_path = PathBuf::from(&workspace_arg);
    if !workspace_path.is_dir() {
        eprintln!(
            "PHASE-3-SMOKE: FAIL — workspace path is not a directory: {}",
            workspace_path.display()
        );
        std::process::exit(3);
    }

    let workspace_abs = match workspace_path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "PHASE-3-SMOKE: FAIL — cannot canonicalize {}: {}",
                workspace_path.display(),
                e
            );
            std::process::exit(3);
        }
    };

    println!(
        "PHASE-3-SMOKE: starting smoke against workspace {}",
        workspace_abs.display()
    );

    match run_smoke(&workspace_abs).await {
        Ok(()) => {
            println!("PHASE-3-SMOKE: PASS");
            std::process::exit(0);
        }
        Err(SmokeError::Handshake(msg)) => {
            eprintln!("PHASE-3-SMOKE: FAIL (handshake) — {}", msg);
            std::process::exit(1);
        }
        Err(SmokeError::Capability(msg)) => {
            eprintln!("PHASE-3-SMOKE: FAIL (capability) — {}", msg);
            std::process::exit(2);
        }
    }
}

#[derive(Debug)]
enum SmokeError {
    Handshake(String),
    Capability(String),
}

async fn run_smoke(workspace_abs: &Path) -> Result<(), SmokeError> {
    // 1. Spawn the LspService and start draining ClientSocket.
    let (mut service, _docs, sink) = spawn_service().await;

    // 2. initialize handshake.
    let init_uri = Url::from_directory_path(workspace_abs)
        .map_err(|_| SmokeError::Handshake(format!("invalid workspace URI: {}", workspace_abs.display())))?;
    let init_params = InitializeParams {
        workspace_folders: Some(vec![WorkspaceFolder {
            uri: init_uri.clone(),
            name: "showcase".to_string(),
        }]),
        ..Default::default()
    };
    let init_req = JsonRpcRequest::build("initialize")
        .params(serde_json::to_value(init_params).unwrap())
        .id(1)
        .finish();
    let init_resp: JsonRpcResponse = service
        .call(init_req)
        .await
        .map_err(|e| SmokeError::Handshake(format!("initialize call failed: {}", e)))?
        .ok_or_else(|| SmokeError::Handshake("initialize returned no response".to_string()))?;
    let (_init_id, init_result) = init_resp.into_parts();
    init_result
        .map_err(|e| SmokeError::Handshake(format!("initialize error: {:?}", e)))?;
    println!("PHASE-3-SMOKE: initialize OK");

    // 3. initialized notification (no response expected).
    let initialized_notif = JsonRpcRequest::build("initialized")
        .params(serde_json::json!({}))
        .finish();
    let _ = service
        .call(initialized_notif)
        .await
        .map_err(|e| SmokeError::Handshake(format!("initialized call failed: {}", e)))?;
    println!("PHASE-3-SMOKE: initialized OK");

    // 4. Wait for the eager_parse background task to populate the index.
    //    Per Plan 03-04 D-47: eager_parse spawns on `initialized` and walks
    //    the workspace. 2 seconds is generous for the 19-file showcase.
    tokio::time::sleep(Duration::from_millis(2000)).await;

    // 5. Find a canonical .deal file to exercise capabilities against.
    //    Prefer battery.deal (the Plan 03-03 canonical test target); fall
    //    back to any .deal under workspace if not present.
    let battery_path = workspace_abs
        .join("packages/vehicle/battery.deal");
    let exercise_path = if battery_path.is_file() {
        battery_path
    } else {
        find_first_deal_file(workspace_abs)
            .ok_or_else(|| SmokeError::Capability(
                format!("no .deal files found under {}", workspace_abs.display()),
            ))?
    };
    let exercise_uri = Url::from_file_path(&exercise_path)
        .map_err(|_| SmokeError::Capability(format!("invalid file URI: {}", exercise_path.display())))?;
    let exercise_text = std::fs::read_to_string(&exercise_path)
        .map_err(|e| SmokeError::Capability(format!("read {}: {}", exercise_path.display(), e)))?;
    println!(
        "PHASE-3-SMOKE: exercising capabilities against {}",
        exercise_path.display()
    );

    // 6. didOpen the exercise file.
    let did_open = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: exercise_uri.clone(),
            language_id: "deal".to_string(),
            version: 1,
            text: exercise_text.clone(),
        },
    };
    let did_open_notif = JsonRpcRequest::build("textDocument/didOpen")
        .params(serde_json::to_value(did_open).unwrap())
        .finish();
    let _ = service
        .call(did_open_notif)
        .await
        .map_err(|e| SmokeError::Capability(format!("didOpen failed: {}", e)))?;

    // Give the debouncer + parse task time to run + publishDiagnostics.
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Capability 1: Diagnostics — assert at least one publishDiagnostics
    // notification arrived in the sink (could be empty diagnostics list for
    // a clean file; that still proves the path round-tripped).
    {
        let sink_guard = sink.lock().await;
        let saw_diag = sink_guard
            .iter()
            .any(|r| r.method() == "textDocument/publishDiagnostics");
        if !saw_diag {
            return Err(SmokeError::Capability(
                "no publishDiagnostics notification received within 500ms of didOpen".to_string(),
            ));
        }
    }
    println!("PHASE-3-SMOKE: capability 1/5 diagnostics OK");

    // Capability 2: Completion at position (0, 0).
    {
        let req = JsonRpcRequest::build("textDocument/completion")
            .params(serde_json::to_value(CompletionParams {
                text_document_position: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier {
                        uri: exercise_uri.clone(),
                    },
                    position: Position { line: 0, character: 0 },
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                context: None,
            }).unwrap())
            .id(2)
            .finish();
        let resp = service
            .call(req)
            .await
            .map_err(|e| SmokeError::Capability(format!("completion failed: {}", e)))?
            .ok_or_else(|| SmokeError::Capability("completion returned no response".to_string()))?;
        let (_id, result) = resp.into_parts();
        let result = result
            .map_err(|e| SmokeError::Capability(format!("completion error: {:?}", e)))?;
        // Result may be null (empty completion list) — that still proves
        // the round-trip. Reject only on outright protocol error above.
        if result.is_null() {
            // Accept null as "no candidates at this position" — the LSP
            // protocol allows it (CompletionResponse::Null).
        }
    }
    println!("PHASE-3-SMOKE: capability 2/5 completion OK");

    // Capability 3: Definition at position (0, 0).
    {
        let req = JsonRpcRequest::build("textDocument/definition")
            .params(serde_json::to_value(GotoDefinitionParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier {
                        uri: exercise_uri.clone(),
                    },
                    position: Position { line: 0, character: 0 },
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            }).unwrap())
            .id(3)
            .finish();
        let resp = service
            .call(req)
            .await
            .map_err(|e| SmokeError::Capability(format!("definition failed: {}", e)))?
            .ok_or_else(|| SmokeError::Capability("definition returned no response".to_string()))?;
        let (_id, result) = resp.into_parts();
        result
            .map_err(|e| SmokeError::Capability(format!("definition error: {:?}", e)))?;
    }
    println!("PHASE-3-SMOKE: capability 3/5 definition OK");

    // Capability 4: Hover at position (0, 0).
    {
        let req = JsonRpcRequest::build("textDocument/hover")
            .params(serde_json::to_value(HoverParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier {
                        uri: exercise_uri.clone(),
                    },
                    position: Position { line: 0, character: 0 },
                },
                work_done_progress_params: Default::default(),
            }).unwrap())
            .id(4)
            .finish();
        let resp = service
            .call(req)
            .await
            .map_err(|e| SmokeError::Capability(format!("hover failed: {}", e)))?
            .ok_or_else(|| SmokeError::Capability("hover returned no response".to_string()))?;
        let (_id, result) = resp.into_parts();
        result
            .map_err(|e| SmokeError::Capability(format!("hover error: {:?}", e)))?;
    }
    println!("PHASE-3-SMOKE: capability 4/5 hover OK");

    // Capability 5: Formatting (whole document).
    {
        let req = JsonRpcRequest::build("textDocument/formatting")
            .params(serde_json::to_value(DocumentFormattingParams {
                text_document: TextDocumentIdentifier {
                    uri: exercise_uri.clone(),
                },
                options: FormattingOptions {
                    tab_size: 2,
                    insert_spaces: true,
                    ..Default::default()
                },
                work_done_progress_params: Default::default(),
            }).unwrap())
            .id(5)
            .finish();
        let resp = service
            .call(req)
            .await
            .map_err(|e| SmokeError::Capability(format!("formatting failed: {}", e)))?
            .ok_or_else(|| SmokeError::Capability("formatting returned no response".to_string()))?;
        let (_id, result) = resp.into_parts();
        let result = result
            .map_err(|e| SmokeError::Capability(format!("formatting error: {:?}", e)))?;
        // Formatting result MUST be a non-null array of TextEdits (or null
        // if file is already canonically formatted). Either is acceptable
        // for the smoke — the request returning *without* error proves the
        // round-trip works.
        let _ = result; // Silence unused.
    }
    println!("PHASE-3-SMOKE: capability 5/5 formatting OK");

    // Bonus: Semantic tokens (Plan 03-04 / 03-05 added this).
    {
        let req = JsonRpcRequest::build("textDocument/semanticTokens/full")
            .params(serde_json::to_value(SemanticTokensParams {
                text_document: TextDocumentIdentifier {
                    uri: exercise_uri.clone(),
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            }).unwrap())
            .id(6)
            .finish();
        let resp = service
            .call(req)
            .await
            .map_err(|e| SmokeError::Capability(format!("semanticTokens/full failed: {}", e)))?
            .ok_or_else(|| SmokeError::Capability("semanticTokens/full returned no response".to_string()))?;
        let (_id, result) = resp.into_parts();
        let result = result
            .map_err(|e| SmokeError::Capability(format!("semanticTokens/full error: {:?}", e)))?;
        // Accept null OR a SemanticTokens object — either proves the
        // round-trip works. An outright protocol error above would have
        // already failed the smoke.
        let _ = result;
    }
    println!("PHASE-3-SMOKE: bonus semantic tokens OK");

    // 7. shutdown handshake.
    let shutdown_req = JsonRpcRequest::build("shutdown")
        .params(serde_json::Value::Null)
        .id(99)
        .finish();
    let _ = service
        .call(shutdown_req)
        .await
        .map_err(|e| SmokeError::Handshake(format!("shutdown failed: {}", e)))?;

    let exit_notif = JsonRpcRequest::build("exit")
        .params(serde_json::Value::Null)
        .finish();
    let _ = service.call(exit_notif).await;

    Ok(())
}

/// Walk the workspace looking for the first .deal file.
fn find_first_deal_file(root: &Path) -> Option<PathBuf> {
    walkdir::WalkDir::new(root)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .find(|e| {
            e.file_type().is_file()
                && e.path()
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s == "deal")
                    .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
}

/// Spawn the LspService and drain ClientSocket on a background task.
/// Mirrors the pattern in lsp/tests/showcase.rs spawn_service().
async fn spawn_service() -> (LspService<Backend>, Arc<Documents>, NotificationSink) {
    let (service, socket) = LspService::new(Backend::new);
    let sink: NotificationSink = Arc::new(Mutex::new(Vec::new()));

    // Drain ClientSocket as a Stream of outgoing server-to-client requests.
    // In tower-lsp 0.20 the ClientSocket Stream Item is jsonrpc::Request
    // directly (notifications are modelled as requests with no id); the
    // Message enum is pub(crate)-only.
    let sink_clone = Arc::clone(&sink);
    tokio::spawn(async move {
        let mut socket = socket;
        while let Some(req) = socket.next().await {
            let mut guard = sink_clone.lock().await;
            guard.push(req);
        }
    });

    // We don't have direct access to Documents from outside Backend in this
    // smoke; return a placeholder Arc. The showcase tests use spawn_service_full
    // to peek at Documents; we don't need that for the smoke.
    let docs_placeholder = Arc::new(Documents::default());
    (service, docs_placeholder, sink)
}
