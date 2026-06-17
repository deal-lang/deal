//! `textDocument/signatureHelp` for calc / constraint invocations (P2 WS-E).
//!
//! On a `call` whose callee resolves to a `calc_def` / `constraint_def`, render
//! the callee's signature (reusing hover's param renderers) and highlight the
//! active parameter, derived from a depth-aware comma count between the opening
//! paren and the cursor. Callee resolution reuses `definition::resolved_path_at`
//! so cross-file invocations work exactly like goto-definition.

use serde_json::Value;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    ParameterInformation, ParameterLabel, Position, SignatureHelp, SignatureHelpParams,
    SignatureInformation, Url,
};

use crate::definition;
use crate::documents::Documents;
use crate::hover;
use crate::index::{byte_to_position, Index};

/// `textDocument/signatureHelp` handler.
pub async fn handle_signature_help(
    documents: &Documents,
    index: &Index,
    params: SignatureHelpParams,
) -> LspResult<Option<SignatureHelp>> {
    let uri = params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;
    Ok(signature_at(documents, index, &uri, position).await)
}

async fn signature_at(
    documents: &Documents,
    index: &Index,
    uri: &Url,
    position: Position,
) -> Option<SignatureHelp> {
    let rope = documents.get_buffer(uri)?;
    let cursor = hover::position_to_byte_pub(&rope, position)?;

    let root = ast_root(documents, uri).await?;
    let entry = ast_entry(&root)?;

    // Innermost call enclosing the cursor; the cursor must be inside its parens.
    let call = deepest_with_kinds(entry, cursor, &["call"])?;
    let callee = call.get("callee")?;
    let callee_span = callee.get("span").and_then(parse_span)?;

    let src = rope.to_string();
    let open = find_open_paren(&src, callee_span[1])?;
    if cursor <= open {
        return None; // cursor is on the callee, not within the argument list.
    }
    let active = active_param(&src, open, cursor);

    // Resolve the callee to its declaration (cross-file aware), then locate the
    // declaring calc_def / constraint_def node to render its signature.
    let callee_pos = byte_to_position(&rope, callee_span[0]);
    let (_fq, loc) = definition::resolved_path_at(documents, index, uri, callee_pos).await?;

    let decl_rope = documents.get_buffer(&loc.uri)?;
    let decl_byte = hover::position_to_byte_pub(&decl_rope, loc.range.start)?;
    let decl_root = ast_root(documents, &loc.uri).await?;
    let decl_entry = ast_entry(&decl_root)?;
    let def = deepest_with_kinds(decl_entry, decl_byte, &["calc_def", "constraint_def"])?;

    let (label, param_labels) = render_callable_signature(def)?;
    let n = param_labels.len();
    let active = if n == 0 { 0 } else { active.min(n - 1) };
    let parameters: Vec<ParameterInformation> = param_labels
        .into_iter()
        .map(|l| ParameterInformation {
            label: ParameterLabel::Simple(l),
            documentation: None,
        })
        .collect();

    Some(SignatureHelp {
        signatures: vec![SignatureInformation {
            label,
            documentation: None,
            parameters: Some(parameters),
            active_parameter: Some(active as u32),
        }],
        active_signature: Some(0),
        active_parameter: Some(active as u32),
    })
}

async fn ast_root(documents: &Documents, uri: &Url) -> Option<Value> {
    let handle = documents.get_handle(uri)?;
    let bytes = {
        let guard = handle.lock().await;
        deal_ffi::safe::ast_json(&guard)?
    };
    if bytes.is_empty() {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}

/// The AST envelope wraps the tree in a `root` field; descend if needed.
fn ast_entry(root: &Value) -> Option<&Value> {
    if root.get("span").is_some() {
        Some(root)
    } else {
        root.get("root")
    }
}

/// Build `(full_signature_label, per-parameter labels)` for a `calc_def` /
/// `constraint_def`, e.g. `("Drag(in velocity : Real, in cd : Real) : Force",
/// ["in velocity : Real", "in cd : Real"])`.
fn render_callable_signature(def: &Value) -> Option<(String, Vec<String>)> {
    let name = def.get("name").and_then(|v| v.as_str())?;
    let params_node = def.get("params");
    let param_labels: Vec<String> = params_node
        .and_then(|p| p.get("params"))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().map(hover::render_param_decl).collect())
        .unwrap_or_default();

    let mut label = format!(
        "{name}{}",
        params_node
            .map(hover::render_param_list)
            .unwrap_or_else(|| "()".to_string())
    );
    if let Some(ty) = def.get("type_node") {
        let ts = hover::render_type_annotation(ty);
        if !ts.is_empty() {
            label.push_str(&format!(" : {ts}"));
        }
    }
    if let Some(rc) = def.get("return_contract") {
        let rcs = hover::render_return_contract(rc);
        if !rcs.is_empty() {
            label.push(' ');
            label.push_str(&rcs);
        }
    }
    Some((label, param_labels))
}

/// Deepest node with `k` in `kinds` whose span contains `offset`. Span-bearing
/// nodes that don't contain the offset prune the search; span-less wrappers
/// (the root envelope, arrays) recurse freely.
fn deepest_with_kinds<'a>(node: &'a Value, offset: usize, kinds: &[&str]) -> Option<&'a Value> {
    if let Some(span) = node.get("span").and_then(parse_span) {
        if offset < span[0] || offset >= span[1] {
            return None;
        }
    }
    let mut deeper = None;
    match node {
        Value::Object(map) => {
            for v in map.values() {
                if let Some(d) = deepest_with_kinds(v, offset, kinds) {
                    deeper = Some(d);
                }
            }
        }
        Value::Array(items) => {
            for v in items {
                if let Some(d) = deepest_with_kinds(v, offset, kinds) {
                    deeper = Some(d);
                }
            }
        }
        _ => {}
    }
    if deeper.is_some() {
        return deeper;
    }
    match node.get("k").and_then(|v| v.as_str()) {
        Some(k) if kinds.contains(&k) => Some(node),
        _ => None,
    }
}

/// First `(` at or after `from` (a byte offset on a char boundary).
fn find_open_paren(src: &str, from: usize) -> Option<usize> {
    if from > src.len() {
        return None;
    }
    src[from..].find('(').map(|i| from + i)
}

/// Top-level comma count in `src(open, cursor)` — commas nested in inner
/// `()`/`[]`/`{}` don't advance the active parameter.
fn active_param(src: &str, open: usize, cursor: usize) -> usize {
    let end = cursor.min(src.len());
    let start = (open + 1).min(end);
    let mut depth = 0i32;
    let mut commas = 0usize;
    for &b in &src.as_bytes()[start..end] {
        match b {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            b',' if depth == 0 => commas += 1,
            _ => {}
        }
    }
    commas
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
    fn active_param_counts_top_level_commas_only() {
        let src = "Drag(a, f(x, y), b)";
        let open = src.find('(').unwrap();
        // Cursor right after the first comma → param index 1.
        assert_eq!(active_param(src, open, src.find(',').unwrap() + 1), 1);
        // Cursor inside the nested call f(x, y) → still param index 1 (the
        // nested comma is at depth 1 and must not advance).
        let nested = src.find("y").unwrap();
        assert_eq!(active_param(src, open, nested), 1);
        // Cursor after the second top-level comma → param index 2.
        let last = src.rfind(',').unwrap() + 1;
        assert_eq!(active_param(src, open, last), 2);
    }

    #[test]
    fn render_callable_signature_includes_params_and_return() {
        let def = json!({
            "k": "calc_def", "name": "Drag", "span": [0, 80],
            "params": { "k": "param_list", "params": [
                { "k": "param_decl", "name": "velocity", "direction": "in",
                  "type_node": { "k": "type_annotation", "name_segments": ["Real"] } },
                { "k": "param_decl", "name": "cd", "direction": "in",
                  "type_node": { "k": "type_annotation", "name_segments": ["Real"] } }
            ]},
            "type_node": { "k": "type_annotation", "name_segments": ["Force"] }
        });
        let (label, params) = render_callable_signature(&def).unwrap();
        assert_eq!(label, "Drag(in velocity : Real, in cd : Real) : Force");
        assert_eq!(params, vec!["in velocity : Real", "in cd : Real"]);
    }

    #[test]
    fn deepest_with_kinds_prefers_innermost() {
        // Outer call contains an inner call; offset inside the inner call must
        // return the inner node.
        let ast = json!({
            "k": "call", "span": [0, 30],
            "callee": { "k": "identifier", "name": "outer", "span": [0, 5] },
            "args": [
                { "k": "call", "span": [6, 20],
                  "callee": { "k": "identifier", "name": "inner", "span": [6, 11] },
                  "args": [] }
            ]
        });
        let n = deepest_with_kinds(&ast, 8, &["call"]).unwrap();
        assert_eq!(n.get("callee").unwrap().get("name").unwrap(), "inner");
        // Offset in the outer-only region returns the outer call.
        let o = deepest_with_kinds(&ast, 25, &["call"]).unwrap();
        assert_eq!(o.get("callee").unwrap().get("name").unwrap(), "outer");
    }
}
