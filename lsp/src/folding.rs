//! `textDocument/foldingRange` — collapsible regions (P3 WS-C).
//!
//! Pure AST walk (the same `ast_json` the other providers read): every node
//! whose span covers more than one line and whose kind is a foldable construct
//! — a package, any `*_def` body, a behavioral block, or a doc comment — yields
//! a `FoldingRange`. Single-line nodes are pruned by the `end > start` guard, so
//! the foldable-kind set can be generous without emitting empty folds.

use ropey::Rope;
use serde_json::Value;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{FoldingRange, FoldingRangeKind, FoldingRangeParams};

use crate::documents::Documents;
use crate::index::byte_to_position;

/// `textDocument/foldingRange` handler.
pub async fn handle_folding_range(
    documents: &Documents,
    params: FoldingRangeParams,
) -> LspResult<Option<Vec<FoldingRange>>> {
    let uri = params.text_document.uri;
    Ok(folding_ranges(documents, &uri).await)
}

async fn folding_ranges(documents: &Documents, uri: &tower_lsp::lsp_types::Url) -> Option<Vec<FoldingRange>> {
    let handle = documents.get_handle(uri)?;
    let rope = documents.get_buffer(uri)?;
    let ast_bytes = {
        let guard = handle.lock().await;
        deal_ffi::safe::ast_json(&guard)?
    };
    if ast_bytes.is_empty() {
        return None;
    }
    let root: Value = serde_json::from_slice(&ast_bytes).ok()?;
    let entry = if root.get("span").is_some() {
        &root
    } else {
        root.get("root")?
    };
    let mut out = Vec::new();
    collect_folds(entry, &rope, &mut out);
    Some(out)
}

fn collect_folds(node: &Value, rope: &Rope, out: &mut Vec<FoldingRange>) {
    if let Value::Object(map) = node {
        if let Some(kind) = map.get("k").and_then(|v| v.as_str()) {
            if let Some(fr) = fold_for(kind, map, rope) {
                out.push(fr);
            }
        }
        for v in map.values() {
            collect_folds(v, rope, out);
        }
    } else if let Value::Array(items) = node {
        for v in items {
            collect_folds(v, rope, out);
        }
    }
}

/// Build a `FoldingRange` for a foldable, multi-line node; `None` otherwise.
fn fold_for(kind: &str, map: &serde_json::Map<String, Value>, rope: &Rope) -> Option<FoldingRange> {
    if !is_foldable(kind) {
        return None;
    }
    let span = map.get("span").and_then(parse_span)?;
    let start_line = byte_to_position(rope, span[0]).line;
    // span[1] is half-open (one past the closing token); step back one byte so
    // the closing-brace line is the fold's end.
    let end_line = byte_to_position(rope, span[1].saturating_sub(1)).line;
    if end_line <= start_line {
        return None; // single-line construct — nothing to fold.
    }
    Some(FoldingRange {
        start_line,
        end_line,
        start_character: None,
        end_character: None,
        kind: fold_kind(kind),
        collapsed_text: None,
    })
}

fn is_foldable(kind: &str) -> bool {
    kind.ends_with("_def")
        || matches!(
            kind,
            "package"
                | "doc_comment"
                | "decide_block"
                | "par_block"
                | "loop_statement"
                | "transition_statement"
        )
}

fn fold_kind(kind: &str) -> Option<FoldingRangeKind> {
    if kind == "doc_comment" {
        Some(FoldingRangeKind::Comment)
    } else {
        None // region fold
    }
}

fn parse_span(v: &Value) -> Option<[usize; 2]> {
    let arr = v.as_array()?;
    Some([
        arr.first()?.as_u64()? as usize,
        arr.get(1)?.as_u64()? as usize,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn folds(ast: &Value, src: &str) -> Vec<FoldingRange> {
        let rope = Rope::from_str(src);
        let mut out = Vec::new();
        collect_folds(ast, &rope, &mut out);
        out
    }

    #[test]
    fn folds_multiline_definition_not_single_line() {
        let src = "package p;\npart def Pack {\n  attribute m;\n}\npart def Tiny {}\n";
        // Derive spans from the source so byte offsets match the line layout.
        let pack_start = src.find("part def Pack").unwrap();
        let pack_end = src.find("}\n").unwrap() + 1; // half-open, just past the `}`
        let attr_start = src.find("attribute m").unwrap();
        let attr_end = attr_start + "attribute m;".len();
        let tiny_start = src.find("part def Tiny").unwrap();
        let tiny_end = tiny_start + "part def Tiny {}".len();
        let ast = json!({
            "k": "package", "name": "p", "span": [0, src.len()],
            "body": [
                { "k": "part_def", "name": "Pack", "span": [pack_start, pack_end],
                  "body": [ { "k": "attribute_usage", "name": "m", "span": [attr_start, attr_end] } ] },
                { "k": "part_def", "name": "Tiny", "span": [tiny_start, tiny_end] }
            ]
        });
        let fr = folds(&ast, src);
        // Pack spans line 1 → its `}` on line 3; Tiny is single-line; package folds.
        let pack = fr.iter().find(|f| f.start_line == 1);
        assert!(pack.is_some(), "Pack fold missing: {fr:?}");
        assert_eq!(pack.unwrap().end_line, 3, "Pack should fold to its closing-brace line");
        assert!(!fr.iter().any(|f| f.start_line == 4), "single-line Tiny should not fold");
        assert!(fr.iter().any(|f| f.start_line == 0), "package fold missing");
    }

    #[test]
    fn doc_comment_folds_as_comment_kind() {
        let src = "/**\n * docs\n */\npart def X {}\n";
        let ast = json!({
            "k": "part_def", "name": "X", "span": [16, 29],
            "doc_comment": { "k": "doc_comment", "span": [0, 15] }
        });
        let fr = folds(&ast, src);
        let dc = fr.iter().find(|f| f.start_line == 0).expect("doc comment fold");
        assert_eq!(dc.kind, Some(FoldingRangeKind::Comment));
        assert_eq!(dc.end_line, 2);
    }

    #[test]
    fn is_foldable_covers_defs_blocks_package_comments() {
        assert!(is_foldable("part_def"));
        assert!(is_foldable("constraint_def"));
        assert!(is_foldable("package"));
        assert!(is_foldable("decide_block"));
        assert!(is_foldable("doc_comment"));
        assert!(!is_foldable("identifier"));
        assert!(!is_foldable("type_annotation"));
    }
}
