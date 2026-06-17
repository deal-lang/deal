//! `textDocument/codeAction` — quick-fixes keyed off diagnostics (P2 WS-F).
//!
//! The compiler does not yet attach `fix_it` suggestions or `H0600`
//! did-you-mean hints to the diagnostic envelope (the fields exist in
//! `diagnostics.zig` but are unpopulated, and `diagnostics.rs` does not parse
//! them), so the suggestion is computed LSP-side: the offending name is read
//! from the diagnostic message, and the nearest workspace symbol by edit
//! distance becomes the replacement.
//!
//! Targeted today: `E2000` (name not found in scope) — the unresolved
//! reference / `<<specializes>>` target case, whose message format
//! (`name `X` not found in scope`) is verified. `E2100`/`E2400` can be added
//! once their message shapes are confirmed.

use std::collections::{HashMap, HashSet};

use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, CodeActionResponse,
    Diagnostic, NumberOrString, Range, TextEdit, Url, WorkspaceEdit,
};

use crate::documents::Documents;
use crate::hover;
use crate::index::{byte_to_position, Index};

/// Diagnostic codes this handler can offer a did-you-mean fix for.
const FIXABLE: &[&str] = &["E2000"];

/// `textDocument/codeAction` handler.
pub async fn handle_code_action(
    documents: &Documents,
    index: &Index,
    params: CodeActionParams,
) -> LspResult<Option<CodeActionResponse>> {
    let uri = params.text_document.uri;
    let Some(rope) = documents.get_buffer(&uri) else {
        return Ok(None);
    };

    let mut actions: Vec<CodeActionOrCommand> = Vec::new();
    for diag in &params.context.diagnostics {
        if !is_fixable(diag) || !ranges_intersect(&diag.range, &params.range) {
            continue;
        }
        if let Some(action) = did_you_mean_action(&uri, &rope, index, diag) {
            actions.push(CodeActionOrCommand::CodeAction(action));
        }
    }

    if actions.is_empty() {
        Ok(None)
    } else {
        Ok(Some(actions))
    }
}

fn is_fixable(d: &Diagnostic) -> bool {
    matches!(&d.code, Some(NumberOrString::String(c)) if FIXABLE.contains(&c.as_str()))
}

/// Build a "Change `X` to `Y`" quick-fix: locate the offending name inside the
/// diagnostic range and replace it with the nearest workspace symbol.
fn did_you_mean_action(
    uri: &Url,
    rope: &ropey::Rope,
    index: &Index,
    diag: &Diagnostic,
) -> Option<CodeAction> {
    let bad = backtick_token(&diag.message)?;

    // The diagnostic span may be broader than the name (e.g. it covers the
    // `<<specializes>>` operator), so find the name precisely within it.
    let start = hover::position_to_byte_pub(rope, diag.range.start)?;
    let end = hover::position_to_byte_pub(rope, diag.range.end)?;
    let slice = rope.byte_slice(start..end).to_string();
    let rel = slice.find(&bad)?;
    let bad_start = start + rel;
    let bad_end = bad_start + bad.len();

    let suggestion = nearest_symbol(index, &bad)?;

    let range = Range {
        start: byte_to_position(rope, bad_start),
        end: byte_to_position(rope, bad_end),
    };
    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
    changes.insert(
        uri.clone(),
        vec![TextEdit {
            range,
            new_text: suggestion.clone(),
        }],
    );

    Some(CodeAction {
        title: format!("Change `{bad}` to `{suggestion}`"),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }),
        is_preferred: Some(true),
        ..Default::default()
    })
}

/// The nearest workspace symbol (by terminal name) to `bad` within an edit-
/// distance threshold, excluding `bad` itself. Deterministic: ties break
/// alphabetically.
fn nearest_symbol(index: &Index, bad: &str) -> Option<String> {
    let mut best: Option<(usize, String)> = None;
    let mut seen: HashSet<String> = HashSet::new();
    for (path, _) in index.snapshot() {
        let term = path.rsplit('.').next().unwrap_or(&path).to_string();
        if term == bad || !seen.insert(term.clone()) {
            continue;
        }
        let dist = levenshtein(bad, &term);
        let better = match &best {
            None => true,
            Some((bd, bn)) => dist < *bd || (dist == *bd && term < *bn),
        };
        if better {
            best = Some((dist, term));
        }
    }
    let (dist, name) = best?;
    // Accept only genuinely close matches: a typo, not an unrelated symbol.
    let threshold = std::cmp::max(2, bad.chars().count() / 3);
    (dist <= threshold).then_some(name)
}

/// Extract the first back-tick-quoted token from a diagnostic message, e.g.
/// ``name `Foo` not found`` → `Foo`.
fn backtick_token(message: &str) -> Option<String> {
    let start = message.find('`')? + 1;
    let rest = &message[start..];
    let end = rest.find('`')?;
    let token = &rest[..end];
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

/// Inclusive overlap of two ranges by (line, character) order.
fn ranges_intersect(a: &Range, b: &Range) -> bool {
    let a_start = (a.start.line, a.start.character);
    let a_end = (a.end.line, a.end.character);
    let b_start = (b.start.line, b.start.character);
    let b_end = (b.end.line, b.end.character);
    a_start <= b_end && b_start <= a_end
}

/// Classic two-row Levenshtein edit distance over Unicode scalar values.
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp::lsp_types::Position;

    #[test]
    fn levenshtein_basic() {
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("ThermallyManagd", "ThermallyManaged"), 1);
        assert_eq!(levenshtein("abc", "abc"), 0);
    }

    #[test]
    fn backtick_token_extracts_first_quoted() {
        assert_eq!(
            backtick_token("name `Foo` not found in scope").as_deref(),
            Some("Foo")
        );
        assert_eq!(backtick_token("no quotes here"), None);
    }

    #[test]
    fn ranges_intersect_detects_overlap_and_gap() {
        let a = Range {
            start: Position::new(2, 4),
            end: Position::new(2, 20),
        };
        let touching = Range {
            start: Position::new(2, 10),
            end: Position::new(2, 30),
        };
        let disjoint = Range {
            start: Position::new(5, 0),
            end: Position::new(5, 4),
        };
        assert!(ranges_intersect(&a, &touching));
        assert!(!ranges_intersect(&a, &disjoint));
    }
}
