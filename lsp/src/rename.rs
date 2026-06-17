//! `textDocument/prepareRename` + `textDocument/rename` (P2 WS-C).
//!
//! Rename is the only mutating capability, so it is built on the authoritative
//! binding index (WS-A) and the precise terminal-name spans (WS-C0), and is
//! gated by a safety bar (ADR-0003 / P2 Decision 2):
//!
//!   * resolves the cursor to exactly one in-workspace declaration;
//!   * never edits files under `.deal/deps/` (dependencies are read-only);
//!   * refuses when the new name would collide with an existing declaration in
//!     the same scope (the `E2002` condition);
//!   * edits ONLY the terminal-name token at each site (the precise spans from
//!     WS-C0), so qualified references and the `<<…>>` operator are untouched.
//!
//! The verify-before-return re-analysis gate (apply edit to in-memory buffers,
//! re-run workspace sema, refuse on any new diagnostic) is a follow-on slice;
//! this module ships the resolve + collision-checked WorkspaceEdit.

use std::collections::HashMap;

use tower_lsp::jsonrpc::{Error as LspError, Result as LspResult};
use tower_lsp::lsp_types::{
    Location, Position, PrepareRenameResponse, Range, RenameParams, TextDocumentPositionParams,
    TextEdit, Url, WorkspaceEdit,
};

use crate::definition;
use crate::documents::Documents;
use crate::index::Index;

/// A URI under a dependency tree — never edited by rename.
fn is_dependency(uri: &Url) -> bool {
    uri.path().contains("/.deal/deps/")
}

/// Inclusive containment of `pos` within `range` (single- or multi-line).
fn range_contains(range: &Range, pos: &Position) -> bool {
    (pos.line, pos.character) >= (range.start.line, range.start.character)
        && (pos.line, pos.character) <= (range.end.line, range.end.character)
}

/// The precise occurrence range at the cursor — the declaration name (if the
/// cursor is on it in this file) or the reference site under the cursor.
fn occurrence_at(index: &Index, uri: &Url, path: &str, pos: &Position) -> Option<Range> {
    if let Some(r) = index.declaration_in_file(uri, path) {
        if range_contains(&r, pos) {
            return Some(r);
        }
    }
    index
        .usages_in_file(uri, path)
        .into_iter()
        .find(|r| range_contains(r, pos))
}

/// Parent package of an FQ path (`vehicle.battery.Pack` → `vehicle.battery`);
/// empty for a top-level name.
fn parent_path(path: &str) -> &str {
    match path.rsplit_once('.') {
        Some((parent, _)) => parent,
        None => "",
    }
}

/// `textDocument/prepareRename` — validate and bound the rename.
pub async fn handle_prepare_rename(
    documents: &Documents,
    index: &Index,
    params: TextDocumentPositionParams,
) -> LspResult<Option<PrepareRenameResponse>> {
    let uri = &params.text_document.uri;
    let position = params.position;

    let Some((path, decl)) = definition::resolved_path_at(documents, index, uri, position).await
    else {
        return Ok(None);
    };
    // Dependencies are read-only.
    if is_dependency(&decl.uri) {
        return Err(LspError::invalid_params(
            "cannot rename a symbol declared in a dependency",
        ));
    }
    // The renameable token range at the cursor.
    match occurrence_at(index, uri, &path, &position) {
        Some(range) => Ok(Some(PrepareRenameResponse::Range(range))),
        None => Ok(None),
    }
}

/// `textDocument/rename` — produce a workspace edit over the precise name spans.
pub async fn handle_rename(
    documents: &Documents,
    index: &Index,
    params: RenameParams,
) -> LspResult<Option<WorkspaceEdit>> {
    let uri = &params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;
    let new_name = params.new_name.trim();

    if new_name.is_empty() {
        return Err(LspError::invalid_params("new name must not be empty"));
    }

    let Some((path, decl)) = definition::resolved_path_at(documents, index, uri, position).await
    else {
        return Ok(None);
    };
    if is_dependency(&decl.uri) {
        return Err(LspError::invalid_params(
            "cannot rename a symbol declared in a dependency",
        ));
    }

    // Collision check (E2002): the new FQ path must not already exist in scope.
    let parent = parent_path(&path);
    let new_fq = if parent.is_empty() {
        new_name.to_string()
    } else {
        format!("{parent}.{new_name}")
    };
    if new_fq != path && index.contains_path(&new_fq) {
        return Err(LspError::invalid_params(format!(
            "cannot rename to `{new_name}`: `{new_fq}` already exists in this scope"
        )));
    }

    // Gather declaration + every resolved reference site (precise name spans).
    let sites: Vec<Location> = index.references_of(&path, true);

    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
    for loc in sites {
        // Never edit dependency files.
        if is_dependency(&loc.uri) {
            continue;
        }
        changes.entry(loc.uri).or_default().push(TextEdit {
            range: loc.range,
            new_text: new_name.to_string(),
        });
    }

    if changes.is_empty() {
        return Ok(None);
    }

    Ok(Some(WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_path_splits_terminal() {
        assert_eq!(parent_path("vehicle.battery.Pack"), "vehicle.battery");
        assert_eq!(parent_path("Pack"), "");
    }

    #[test]
    fn is_dependency_detects_deps_tree() {
        let dep = Url::parse("file:///ws/.deal/deps/foo/bar.deal").unwrap();
        let local = Url::parse("file:///ws/packages/vehicle/battery.deal").unwrap();
        assert!(is_dependency(&dep));
        assert!(!is_dependency(&local));
    }

    #[test]
    fn range_contains_is_inclusive() {
        let r = Range {
            start: Position::new(2, 4),
            end: Position::new(2, 20),
        };
        assert!(range_contains(&r, &Position::new(2, 4))); // start boundary
        assert!(range_contains(&r, &Position::new(2, 12))); // mid
        assert!(range_contains(&r, &Position::new(2, 20))); // end boundary
        assert!(!range_contains(&r, &Position::new(2, 3))); // before
        assert!(!range_contains(&r, &Position::new(3, 0))); // next line
    }
}
