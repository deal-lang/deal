//! `textDocument/formatting` handler.
//!
//! Per CONTEXT.md (Claude's Discretion note) + D-21 (in-memory formatting):
//! the formatter reuses the live per-document handle held by `Documents`
//! and returns a single `TextEdit` spanning the full document. No save
//! round-trip is required; the editor's buffer becomes the authoritative
//! source as soon as the client applies the edit.
//!
//! If the handle has any error-severity diagnostics, we decline to format
//! (returning `Ok(None)`). Diagnostics were already pushed via
//! `publishDiagnostics`; the user can see them.

use std::sync::Arc;

use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{Position, Range, TextEdit, Url};

use crate::documents::Documents;

/// Compute the full-document Range from a Rope (`0:0` to last-line/last-col).
fn full_document_range(rope: &ropey::Rope) -> Range {
    let len_lines = rope.len_lines();
    if len_lines == 0 {
        return Range {
            start: Position::new(0, 0),
            end: Position::new(0, 0),
        };
    }
    // Rope::len_lines counts a final newline as starting a phantom blank
    // line; clamp so we land on the last line containing characters.
    let last_line_idx = len_lines.saturating_sub(1);
    let last_line = rope.line(last_line_idx);
    // last_line may end with '\n' — `len_chars` includes it; LSP wants the
    // column AFTER the last char, so subtract any trailing newline.
    let mut last_col = last_line.len_chars();
    if last_col > 0 {
        // If the last visible line ends with '\n', drop one column so the
        // range ends at the start of the phantom line (UTF-16-safe for
        // ASCII; DEAL source is UTF-8 + the formatter emits ASCII whitespace).
        let last_char = last_line.char(last_col - 1);
        if last_char == '\n' || last_char == '\r' {
            last_col = last_col.saturating_sub(1);
        }
    }
    Range {
        start: Position::new(0, 0),
        end: Position::new(last_line_idx as u32, last_col as u32),
    }
}

/// Implementation of `textDocument/formatting`.
pub async fn handle_formatting(
    documents: &Documents,
    uri: &Url,
) -> LspResult<Option<Vec<TextEdit>>> {
    let handle_arc: Arc<Mutex<deal_ffi::OwnedDealHandle>> = match documents.get_handle(uri) {
        Some(h) => h,
        None => return Ok(None),
    };
    let rope = match documents.get_buffer(uri) {
        Some(r) => r,
        None => return Ok(None),
    };

    // Acquire per-handle Mutex (D-45) — only one task may touch this handle.
    let guard = handle_arc.lock().await;

    if deal_ffi::safe::has_errors(&guard) {
        // D-21: do not format invalid syntax. Diagnostics already pushed.
        return Ok(None);
    }

    let formatted_bytes = match deal_ffi::safe::format(&guard) {
        Some(b) => b,
        None => return Ok(None),
    };
    // Release the handle lock as soon as bytes are cloned.
    drop(guard);

    let new_text = match String::from_utf8(formatted_bytes) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("formatter emitted non-UTF8 for {uri}: {e}");
            return Ok(None);
        }
    };

    let range = full_document_range(&rope);
    Ok(Some(vec![TextEdit { range, new_text }]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::Rope;

    #[test]
    fn full_document_range_empty_rope() {
        let rope = Rope::new();
        let r = full_document_range(&rope);
        assert_eq!(r.start, Position::new(0, 0));
        assert_eq!(r.end, Position::new(0, 0));
    }

    #[test]
    fn full_document_range_single_line_no_newline() {
        let rope = Rope::from_str("hello");
        let r = full_document_range(&rope);
        assert_eq!(r.start, Position::new(0, 0));
        assert_eq!(r.end, Position::new(0, 5));
    }

    #[test]
    fn full_document_range_with_trailing_newline() {
        let rope = Rope::from_str("a\nb\n");
        let r = full_document_range(&rope);
        // Rope splits "a\nb\n" into lines ["a\n", "b\n", ""] → last_line_idx=2,
        // last line len=0, end is (2, 0).
        assert_eq!(r.start, Position::new(0, 0));
        assert_eq!(r.end, Position::new(2, 0));
    }
}
