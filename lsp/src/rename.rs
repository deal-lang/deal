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
//! The verify-before-return re-analysis gate (P2 WS-C C3) closes the loop: the
//! candidate `WorkspaceEdit` is applied to in-memory copies of the affected
//! buffers, the whole workspace is re-analyzed against the same merged
//! declaration set `eager_parse` uses, and the rename is refused if the edit
//! would introduce ANY new error-level diagnostic in ANY file. This catches
//! regressions the structural collision check cannot — e.g. a rename that makes
//! a reference in an unedited file newly ambiguous (`E2001`).

use std::collections::{HashMap, HashSet};

use tower_lsp::jsonrpc::{Error as LspError, Result as LspResult};
use tower_lsp::lsp_types::{
    Diagnostic, DiagnosticSeverity, Location, Position, PrepareRenameResponse, Range, RenameParams,
    TextDocumentPositionParams, TextEdit, Url, WorkspaceEdit,
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

    // C3: verify-before-return. Re-analyze the post-edit workspace; refuse if
    // the rename would introduce any new error-level diagnostic anywhere.
    if let Err(msg) = verify_no_new_diagnostics(documents, &changes) {
        return Err(LspError::invalid_params(msg));
    }

    Ok(Some(WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    }))
}

/// Apply a file's `TextEdit`s to its source, returning the post-edit text.
/// Edits are resolved to UTF-8 byte ranges via the shared position→byte
/// converter, then spliced right-to-left so earlier offsets stay valid.
fn apply_edits(text: &str, edits: &[TextEdit]) -> String {
    let rope = ropey::Rope::from_str(text);
    let mut resolved: Vec<(usize, usize, &str)> = Vec::with_capacity(edits.len());
    for e in edits {
        if let (Some(start), Some(end)) = (
            crate::hover::position_to_byte_pub(&rope, e.range.start),
            crate::hover::position_to_byte_pub(&rope, e.range.end),
        ) {
            if start <= end {
                resolved.push((start, end, e.new_text.as_str()));
            }
        }
    }
    resolved.sort_by(|a, b| b.0.cmp(&a.0));
    let mut out = text.to_string();
    for (start, end, new) in resolved {
        if end <= out.len() {
            out.replace_range(start..end, new);
        }
    }
    out
}

/// Error-severity diagnostic fingerprints `(code, message)` — stable across the
/// column shifts a rename causes (name lengths differ), so a set difference
/// surfaces only genuinely new errors.
fn error_fingerprints(diags: &[Diagnostic]) -> HashSet<String> {
    diags
        .iter()
        .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
        .map(|d| format!("{:?}|{}", d.code, d.message))
        .collect()
}

/// Full cross-file check of one file against the merged workspace set,
/// returning its diagnostics (empty on FFI failure — a null handle cannot
/// manufacture a regression, and the structural checks already ran).
fn analyze(text: &str, filename: &str, externals: &[&[u8]]) -> Vec<Diagnostic> {
    let Some(handle) = deal_ffi::safe::check_with_external(text.as_bytes(), filename, externals)
    else {
        return Vec::new();
    };
    let bytes = deal_ffi::safe::diagnostics_json(&handle).unwrap_or_default();
    let rope = ropey::Rope::from_str(text);
    crate::diagnostics::parse_diagnostics(&bytes, &rope)
}

/// Re-analyze the post-edit workspace and reject the rename if it introduces a
/// new error in any file. `Ok(())` ⇒ safe to apply. Mirrors `eager_parse`'s
/// merged-source convention (every file's bytes, self-inclusion harmless).
fn verify_no_new_diagnostics(
    documents: &Documents,
    changes: &HashMap<Url, Vec<TextEdit>>,
) -> Result<(), String> {
    let snapshot = documents.snapshot_buffers();
    if snapshot.is_empty() {
        return Ok(());
    }

    // Post-edit text per file (edited where the rename touches it, else original).
    let post: Vec<(Url, String)> = snapshot
        .iter()
        .map(|(uri, text)| {
            let edited = changes
                .get(uri)
                .map(|edits| apply_edits(text, edits))
                .unwrap_or_else(|| text.clone());
            (uri.clone(), edited)
        })
        .collect();

    let pre_bytes: Vec<&[u8]> = snapshot.iter().map(|(_, t)| t.as_bytes()).collect();
    let post_bytes: Vec<&[u8]> = post.iter().map(|(_, t)| t.as_bytes()).collect();

    for (i, (uri, orig)) in snapshot.iter().enumerate() {
        let filename = uri.path();
        let base = error_fingerprints(&analyze(orig, filename, &pre_bytes));
        let after = error_fingerprints(&analyze(&post[i].1, filename, &post_bytes));
        if let Some(new) = after.difference(&base).next() {
            let path = uri
                .to_file_path()
                .ok()
                .and_then(|p| p.file_name().map(|f| f.to_string_lossy().into_owned()))
                .unwrap_or_else(|| uri.path().to_string());
            let detail = new.split_once('|').map(|(_, m)| m).unwrap_or(new.as_str());
            return Err(format!(
                "rename refused: would introduce a new error in {path}: {detail}"
            ));
        }
    }
    Ok(())
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
    fn apply_edits_splices_multiple_sites_right_to_left() {
        // Two occurrences of `Foo` on one line; both renamed to `Bar` (shorter
        // replacements would shift later offsets if applied left-to-right).
        let src = "part def Foo <<specializes>> Foo {}\n";
        let edits = vec![
            TextEdit {
                range: Range {
                    start: Position::new(0, 9),
                    end: Position::new(0, 12),
                },
                new_text: "Bar".to_string(),
            },
            TextEdit {
                range: Range {
                    start: Position::new(0, 29),
                    end: Position::new(0, 32),
                },
                new_text: "Bar".to_string(),
            },
        ];
        assert_eq!(
            apply_edits(src, &edits),
            "part def Bar <<specializes>> Bar {}\n"
        );
    }

    #[test]
    fn error_fingerprints_keep_only_errors() {
        use tower_lsp::lsp_types::NumberOrString;
        let err = Diagnostic {
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String("E2001".into())),
            message: "ambiguous reference `X`".into(),
            ..Default::default()
        };
        let warn = Diagnostic {
            severity: Some(DiagnosticSeverity::WARNING),
            message: "unused".into(),
            ..Default::default()
        };
        let fps = error_fingerprints(&[err, warn]);
        assert_eq!(fps.len(), 1);
        assert!(fps.iter().next().unwrap().contains("ambiguous reference"));
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
