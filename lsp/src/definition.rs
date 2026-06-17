//! `textDocument/definition` handler (Plan 03-04 Task 2; cross-file
//! go-to-definition via the in-memory `Index`).
//!
//! Strategy (PLAN.md):
//!
//! 1. Look up the document handle from `Documents`.
//! 2. Translate the request position to a UTF-8 byte offset via the
//!    document's Rope.
//! 3. Walk the AST recursively to find the deepest `type_annotation`,
//!    `identifier`, or `qualified_name` node containing the offset.
//! 4. Assemble the candidate PathString from the node:
//!    - `type_annotation` carries `name_segments: ["Battery", "Pack"]`
//!      → join with `.`
//!    - `identifier` carries `name: "BatteryPack"`
//!    - qualified names appear as nested identifiers; we collect the
//!      path-segment text from each.
//! 5. Try the candidate verbatim first, then alias-expanded, then with
//!    every PathString-suffix in the index (a workspace symbol whose
//!    FQ-path ends in `.<candidate>` is a valid match — supports the
//!    common case of `: BatteryPack` resolving to `vehicle.battery.BatteryPack`).
//! 6. Return `Location { uri, range }` from the index hit, or `Ok(None)`.

use serde_json::Value;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    GotoDefinitionParams, GotoDefinitionResponse, Location, Position, Range, Url,
};

use crate::documents::Documents;
use crate::hover; // reuse position_to_byte + node walker (re-exported below)
use crate::index::Index;

/// Implementation of `textDocument/definition`.
pub async fn handle_definition(
    documents: &Documents,
    index: &Index,
    params: GotoDefinitionParams,
) -> LspResult<Option<GotoDefinitionResponse>> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;
    Ok(definition_for(documents, index, uri, position)
        .await
        .map(GotoDefinitionResponse::Scalar))
}

async fn definition_for(
    documents: &Documents,
    index: &Index,
    uri: &Url,
    position: Position,
) -> Option<Location> {
    // P2 WS-E2: a parameter reference resolves to its `param_decl` in the same
    // file. Local scope wins over the global symbol table (correct shadowing),
    // so this is tried before the workspace lookup.
    if let Some(loc) = local_param_definition(documents, uri, position).await {
        return Some(loc);
    }
    resolved_path_at(documents, index, uri, position)
        .await
        .map(|(_path, loc)| loc)
}

/// Resolve a parameter reference under the cursor to the `Location` of its
/// declaring `param_decl` in the same file, or `None` when the cursor is not on
/// a parameter of the enclosing `calc` / `constraint`.
async fn local_param_definition(
    documents: &Documents,
    uri: &Url,
    position: Position,
) -> Option<Location> {
    let handle_arc = documents.get_handle(uri)?;
    let rope = documents.get_buffer(uri)?;
    let byte = hover::position_to_byte_pub(&rope, position)?;
    let ast_bytes = {
        let guard = handle_arc.lock().await;
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
    let span = param_decl_span_at(entry, byte)?;
    Some(Location {
        uri: uri.clone(),
        range: Range {
            start: crate::index::byte_to_position(&rope, span[0]),
            end: crate::index::byte_to_position(&rope, span[1]),
        },
    })
}

/// Pure core of WS-E2: the span of the `param_decl` a parameter reference at
/// `byte` resolves to. Finds the innermost `identifier` under the cursor and
/// the innermost enclosing `calc_def` / `constraint_def`, then matches the
/// identifier name against that callable's parameter list. `None` when the
/// identifier is not one of the enclosing callable's parameters.
fn param_decl_span_at(entry: &Value, byte: usize) -> Option<[usize; 2]> {
    let mut stack: Vec<&Value> = Vec::new();
    collect_containing(entry, byte, &mut stack);

    let ident = stack.iter().rev().find_map(|n| {
        match n.get("k").and_then(|v| v.as_str()) {
            Some("identifier") => n.get("name").and_then(|v| v.as_str()),
            _ => None,
        }
    })?;
    let callable = stack.iter().rev().find(|n| {
        matches!(
            n.get("k").and_then(|v| v.as_str()),
            Some("calc_def") | Some("constraint_def")
        )
    })?;

    let list = callable.get("params")?.get("params")?.as_array()?;
    for p in list {
        if p.get("name").and_then(|v| v.as_str()) == Some(ident) {
            return p.get("span").and_then(parse_span);
        }
    }
    None
}

/// Resolve the symbol under the cursor to its canonical fully-qualified path
/// AND its declaration `Location`. Shared by goto-definition and (P2 WS-A)
/// find-references, which needs the canonical path to key the usage index.
///
/// The returned path is the matched index key (alias-expanded for a verbatim
/// hit, or the winning suffix-match key) — i.e. the same FQ id `sema` records
/// as a binding's `resolved_path`, so the two line up in the reverse index.
pub(crate) async fn resolved_path_at(
    documents: &Documents,
    index: &Index,
    uri: &Url,
    position: Position,
) -> Option<(String, Location)> {
    let handle_arc = documents.get_handle(uri)?;
    let rope = documents.get_buffer(uri)?;
    let byte_offset = hover::position_to_byte_pub(&rope, position)?;

    let ast_bytes = {
        let guard = handle_arc.lock().await;
        deal_ffi::safe::ast_json(&guard)?
    };
    if ast_bytes.is_empty() {
        return None;
    }
    let root: Value = serde_json::from_slice(&ast_bytes).ok()?;
    let candidate = candidate_path_at(&root, byte_offset)?;

    // 1. Verbatim lookup (also exercises alias expansion in Index::lookup).
    //    The canonical key is the alias-expanded candidate.
    if let Some((u, r)) = index.lookup(&candidate) {
        let canonical = index.resolve_with_alias(&candidate);
        return Some((canonical, Location { uri: u, range: r }));
    }
    // 2. Suffix match — the candidate is the unqualified type name; find
    //    any indexed PathString whose final dotted segment(s) equal the
    //    candidate. Deterministic by sorting the candidate hits.
    let suffix = format!(".{candidate}");
    let mut hits: Vec<(String, Location)> = index
        .snapshot()
        .into_iter()
        .filter_map(|(p, (u, r))| {
            if p == candidate || p.ends_with(&suffix) {
                Some((p, Location { uri: u, range: r }))
            } else {
                None
            }
        })
        .collect();
    hits.sort_by(|a, b| a.0.cmp(&b.0));
    hits.into_iter().next()
}

/// Find the deepest type-reference-like node at `byte_offset` and assemble
/// its PathString candidate. The AST root envelope wraps the actual tree
/// in a `root: {...}` field, so we descend one level if the outer node
/// has no `span` (mirrors hover.rs's `find_deepest_node_at` logic).
fn candidate_path_at(root: &Value, byte_offset: usize) -> Option<String> {
    let entry = if root.get("span").is_some() {
        root
    } else {
        root.get("root").unwrap_or(root)
    };
    // Walk a generic stack tracking nodes whose span contains the offset.
    let mut stack: Vec<&Value> = Vec::new();
    collect_containing(entry, byte_offset, &mut stack);

    // Preference order (innermost first):
    //   1. type_annotation  → join name_segments
    //   2. structural_relationship → join target_segments (covers
    //      <<specializes>>/<<redefines>>/<<conjugates>>/<<derives>>)
    //   3. identifier       → use .name
    for node in stack.iter().rev() {
        if let Some(s) = pathstring_from_type_annotation(node) {
            return Some(s);
        }
        if let Some(s) = pathstring_from_structural_relationship(node) {
            return Some(s);
        }
    }
    for node in stack.iter().rev() {
        if let Some(s) = pathstring_from_identifier(node) {
            return Some(s);
        }
    }
    None
}

fn collect_containing<'a>(node: &'a Value, byte_offset: usize, stack: &mut Vec<&'a Value>) {
    if let Some(span) = node.get("span").and_then(parse_span) {
        if byte_offset >= span[0] && byte_offset < span[1] {
            stack.push(node);
        } else {
            return;
        }
    }
    match node {
        Value::Object(map) => {
            for (_k, v) in map.iter() {
                collect_containing(v, byte_offset, stack);
            }
        }
        Value::Array(items) => {
            for item in items.iter() {
                collect_containing(item, byte_offset, stack);
            }
        }
        _ => {}
    }
}

fn pathstring_from_type_annotation(node: &Value) -> Option<String> {
    if node.get("k").and_then(|v| v.as_str())? != "type_annotation" {
        return None;
    }
    let segments = node.get("name_segments")?.as_array()?;
    let parts: Vec<String> = segments
        .iter()
        .filter_map(|s| s.as_str().map(|x| x.to_string()))
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("."))
    }
}

fn pathstring_from_identifier(node: &Value) -> Option<String> {
    if node.get("k").and_then(|v| v.as_str())? != "identifier" {
        return None;
    }
    let name = node.get("name")?.as_str()?;
    Some(name.to_string())
}

// Phase 05.2 (D-04) go-to-def for calc_def / constraint_def:
// NO new code is required here. The `definition_for` path already works:
//   1. `candidate_path_at` extracts the identifier/type_annotation PathString
//      at the click position (a calc invocation's callee is an `identifier` node).
//   2. `Index::lookup` + suffix-match finds the `calc_def` or `constraint_def`
//      entry populated during `eager_parse` (the Index definition scan indexes
//      ALL definition kinds including calc_def — see lsp/src/index.rs).
//   3. The Location returned points to the calc_def span in the source file.
//
// Phase 6 seam: This is the anchor for "jump to calc parameter" go-to-def.
// When Phase 6 adds param_decl indexing, add `pathstring_from_param_decl`
// here that extracts from the `param_decl` node kind. Do NOT refactor
// `candidate_path_at` — just add a new preference tier after `identifier`.

/// Extract the referent PathString from a `structural_relationship`
/// node (the AST representation of `<<specializes>> ThermallyManaged`
/// and friends). The relationship records `target_segments: ["..."]`.
fn pathstring_from_structural_relationship(node: &Value) -> Option<String> {
    if node.get("k").and_then(|v| v.as_str())? != "structural_relationship" {
        return None;
    }
    let segments = node.get("target_segments")?.as_array()?;
    let parts: Vec<String> = segments
        .iter()
        .filter_map(|s| s.as_str().map(|x| x.to_string()))
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("."))
    }
}

fn parse_span(v: &Value) -> Option<[usize; 2]> {
    let arr = v.as_array()?;
    if arr.len() != 2 {
        return None;
    }
    Some([arr[0].as_u64()? as usize, arr[1].as_u64()? as usize])
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::Rope;
    use serde_json::json;
    use tower_lsp::lsp_types::Range;

    #[test]
    fn pathstring_from_type_annotation_joins_segments() {
        let n = json!({
            "k": "type_annotation",
            "span": [0, 10],
            "name_segments": ["vehicle", "battery", "BatteryPack"]
        });
        assert_eq!(
            pathstring_from_type_annotation(&n).as_deref(),
            Some("vehicle.battery.BatteryPack")
        );
    }

    #[test]
    fn pathstring_from_identifier_uses_name() {
        let n = json!({ "k": "identifier", "span": [0, 5], "name": "Foo" });
        assert_eq!(pathstring_from_identifier(&n).as_deref(), Some("Foo"));
    }

    #[test]
    fn candidate_prefers_type_annotation_over_inner_identifier() {
        // A type_annotation node wraps an identifier; we want the joined
        // segments, not the bare identifier name.
        let tree = json!({
            "k": "type_annotation",
            "span": [0, 20],
            "name_segments": ["vehicle", "BatteryPack"],
            "callee": {
                "k": "identifier",
                "span": [10, 20],
                "name": "BatteryPack"
            }
        });
        let cand = candidate_path_at(&tree, 15).unwrap();
        assert_eq!(cand, "vehicle.BatteryPack");
    }

    #[tokio::test]
    async fn lookup_falls_back_to_suffix_match() {
        // Plant a known type in the index, query with the bare name.
        let idx = Index::new();
        let env = br#"{"v":1,"elements":{
            "vehicle.battery.BatteryPack":{"id":"x","kind":"part_def","source_file":"a","span":[0,10]},
            "vehicle.battery.Other":{"id":"x","kind":"part_def","source_file":"a","span":[20,30]}
        }}"#;
        let url = Url::parse("file:///fake.deal").unwrap();
        idx.update_from_envelope(&url, env, &Rope::from_str("part def X {}"));

        // Verbatim hit:
        assert!(idx.lookup("vehicle.battery.BatteryPack").is_some());
        // Suffix hit via snapshot+filter:
        let suffix = ".BatteryPack";
        let hits: Vec<String> = idx
            .snapshot()
            .into_iter()
            .filter_map(|(p, _)| if p.ends_with(suffix) { Some(p) } else { None })
            .collect();
        assert_eq!(hits, vec!["vehicle.battery.BatteryPack".to_string()]);
    }

    #[test]
    fn collect_containing_skips_disjoint_subtrees() {
        let tree = json!({
            "k": "root",
            "span": [0, 100],
            "left": { "k": "x", "span": [0, 10] },
            "right": { "k": "y", "span": [50, 60] }
        });
        let mut stack: Vec<&Value> = Vec::new();
        collect_containing(&tree, 55, &mut stack);
        // root + right; left is disjoint and must not appear.
        let kinds: Vec<&str> = stack
            .iter()
            .filter_map(|n| n.get("k").and_then(|v| v.as_str()))
            .collect();
        assert_eq!(kinds, vec!["root", "y"]);
    }

    #[test]
    fn param_decl_span_at_resolves_body_reference() {
        // calc def Drag(velocity : Real) : Real { return velocity * 2; }
        let ast = json!({
            "k": "calc_def", "name": "Drag", "span": [0, 60],
            "params": { "k": "param_list", "span": [13, 30], "params": [
                { "k": "param_decl", "name": "velocity", "span": [14, 29],
                  "type_node": { "k": "type_annotation", "name_segments": ["Real"], "span": [25, 29] } }
            ]},
            "body": [ { "k": "return_statement", "span": [40, 58],
                "value": { "k": "identifier", "name": "velocity", "span": [47, 55] } } ]
        });
        // Cursor inside the body reference → the param_decl span.
        assert_eq!(param_decl_span_at(&ast, 50), Some([14, 29]));
        // Cursor on the calc name (no identifier node there) → None.
        assert_eq!(param_decl_span_at(&ast, 2), None);
    }

    #[test]
    fn param_decl_span_at_ignores_non_parameter_identifiers() {
        // An identifier that is not one of the callable's params resolves to
        // None (it falls through to the global lookup).
        let ast = json!({
            "k": "calc_def", "name": "F", "span": [0, 40],
            "params": { "k": "param_list", "span": [5, 7], "params": [] },
            "body": [ { "k": "identifier", "name": "Other", "span": [20, 25] } ]
        });
        assert_eq!(param_decl_span_at(&ast, 22), None);
    }

    #[test]
    fn location_round_trip_through_index() {
        let idx = Index::new();
        let url = Url::parse("file:///a.deal").unwrap();
        let env = br#"{"v":1,"elements":{
            "pkg.Foo":{"id":"x","kind":"part_def","source_file":"a","span":[0,12]}
        }}"#;
        idx.update_from_envelope(&url, env, &Rope::from_str("part def Foo "));
        let (u, r) = idx.lookup("pkg.Foo").unwrap();
        let _loc = Location { uri: u, range: r };
        let _ = Range::default(); // suppress unused-import warning if hit
    }
}
