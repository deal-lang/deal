//! `textDocument/codeLens` — reference counts above declarations (P3 WS-C).
//!
//! Each `*_def` declaration in the file gets a "N references" lens whose count
//! comes from the P2 reverse index (`references_of`, excluding the declaration
//! itself) — the clearest demonstration that the binding index is exact.
//!
//! The lens command is `deal.showReferences`, a bridge the extension registers:
//! the built-in `editor.action.showReferences` needs real VS Code `Uri` /
//! `Position` / `Location` objects, which cannot be expressed as JSON-RPC
//! command arguments, so the extension reconstructs them and forwards the call
//! (the rust-analyzer pattern). Counts are computed eagerly (file-sized work),
//! so no `codeLens/resolve` round-trip is needed.

use serde_json::Value;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{CodeLens, CodeLensParams, Command};

use crate::index::Index;

/// `textDocument/codeLens` handler.
pub async fn handle_code_lens(
    index: &Index,
    params: CodeLensParams,
) -> LspResult<Option<Vec<CodeLens>>> {
    let uri = &params.text_document.uri;
    let mut lenses = Vec::new();
    for el in index.elements_in_file(uri) {
        // Reference counts belong on definitions, not on every feature usage.
        if !el.kind.ends_with("_def") {
            continue;
        }
        let locations = index.references_of(&el.path, false);
        let title = match locations.len() {
            1 => "1 reference".to_string(),
            n => format!("{n} references"),
        };
        let command = Command {
            title,
            command: "deal.showReferences".to_string(),
            arguments: Some(vec![
                Value::String(uri.to_string()),
                serde_json::to_value(el.name_range.start).unwrap_or(Value::Null),
                serde_json::to_value(&locations).unwrap_or(Value::Null),
            ]),
        };
        lenses.push(CodeLens {
            range: el.decl_range,
            command: Some(command),
            data: None,
        });
    }
    Ok(Some(lenses))
}
