//! `textDocument/documentSymbol` — per-file hierarchical outline (P2 WS-D).
//!
//! Built by walking the file's AST (the same `ast_json` goto-definition reads),
//! so nesting follows true syntactic containment: a `part def` owns the
//! `action`/`state`/`attribute` members declared inside it. Icons come from the
//! P1 element-kind → `SymbolKind` map (shared with `workspace/symbol`).
//!
//! The selection range is the element's full span: precise name-token selection
//! would require threading `name_span` into the AST JSON (it currently lives only
//! in the index envelope), which would churn the AST golden snapshots — deferred
//! as polish. A full-span selection range is valid LSP and standard practice.

use ropey::Rope;
use serde_json::Value;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse, Range, Url,
};

use crate::documents::Documents;
use crate::index::{byte_to_position, map_symbol_kind};

/// `textDocument/documentSymbol` handler.
pub async fn handle_document_symbol(
    documents: &Documents,
    params: DocumentSymbolParams,
) -> LspResult<Option<DocumentSymbolResponse>> {
    let uri = params.text_document.uri;
    Ok(document_symbols(documents, &uri)
        .await
        .map(DocumentSymbolResponse::Nested))
}

async fn document_symbols(documents: &Documents, uri: &Url) -> Option<Vec<DocumentSymbol>> {
    let handle_arc = documents.get_handle(uri)?;
    let rope = documents.get_buffer(uri)?;
    let ast_bytes = {
        let guard = handle_arc.lock().await;
        deal_ffi::safe::ast_json(&guard)?
    };
    if ast_bytes.is_empty() {
        return None;
    }
    let root: Value = serde_json::from_slice(&ast_bytes).ok()?;
    // The AST envelope wraps the tree in a `root` field; descend if needed.
    let entry = if root.get("span").is_some() {
        &root
    } else {
        root.get("root")?
    };
    Some(gather(entry, &rope))
}

/// Outline symbols contained in `node`'s subtree. If `node` is itself an
/// outline-worthy declaration, the subtree's symbols become its children;
/// otherwise they bubble up to the nearest such ancestor.
fn gather(node: &Value, rope: &Rope) -> Vec<DocumentSymbol> {
    let mut children: Vec<DocumentSymbol> = match node {
        Value::Object(map) => map.values().flat_map(|v| gather(v, rope)).collect(),
        Value::Array(items) => items.iter().flat_map(|v| gather(v, rope)).collect(),
        _ => return Vec::new(),
    };
    children.sort_by_key(|s| (s.range.start.line, s.range.start.character));
    match outline_symbol(node, rope, children) {
        Ok(sym) => vec![sym],
        Err(children) => children,
    }
}

/// Build a `DocumentSymbol` for `node` owning `children`, or return `children`
/// unchanged when `node` is not an outline-worthy named declaration.
fn outline_symbol(
    node: &Value,
    rope: &Rope,
    children: Vec<DocumentSymbol>,
) -> Result<DocumentSymbol, Vec<DocumentSymbol>> {
    let Some(kind) = node.get("k").and_then(|v| v.as_str()) else {
        return Err(children);
    };
    if !is_outline_kind(kind) {
        return Err(children);
    }
    let Some(name) = node
        .get("name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    else {
        return Err(children);
    };
    let Some(span) = node.get("span").and_then(parse_span) else {
        return Err(children);
    };
    let range = Range {
        start: byte_to_position(rope, span[0]),
        end: byte_to_position(rope, span[1]),
    };
    #[allow(deprecated)] // `deprecated` field is required by the struct literal.
    Ok(DocumentSymbol {
        name: name.to_string(),
        detail: Some(kind.replace('_', " ")),
        kind: map_symbol_kind(kind),
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: if children.is_empty() {
            None
        } else {
            Some(children)
        },
    })
}

/// Element definitions and named usages are outline entries; structural nodes
/// (bodies, annotations, expressions) are not.
fn is_outline_kind(kind: &str) -> bool {
    kind.ends_with("_def") || kind.ends_with("_usage")
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

    #[test]
    fn is_outline_kind_accepts_defs_and_usages() {
        assert!(is_outline_kind("part_def"));
        assert!(is_outline_kind("attribute_usage"));
        assert!(!is_outline_kind("type_annotation"));
        assert!(!is_outline_kind("param_list"));
    }

    #[test]
    fn gather_nests_members_under_their_definition() {
        let rope = Rope::from_str(
            "part def Pack {\n  attribute mass;\n  action charge;\n}\n",
        );
        // Minimal AST: a part_def whose body holds two member usages.
        let ast = json!({
            "k": "part_def", "name": "Pack", "span": [0, 50],
            "body": [
                { "k": "attribute_usage", "name": "mass", "span": [18, 32] },
                { "k": "action_def", "name": "charge", "span": [35, 48] }
            ]
        });
        let syms = gather(&ast, &rope);
        assert_eq!(syms.len(), 1, "one top-level symbol");
        let pack = &syms[0];
        assert_eq!(pack.name, "Pack");
        let kids = pack.children.as_ref().expect("Pack has members");
        assert_eq!(kids.len(), 2);
        // Sorted by source position: mass (18) before charge (35).
        assert_eq!(kids[0].name, "mass");
        assert_eq!(kids[1].name, "charge");
    }
}
