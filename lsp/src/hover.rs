//! `textDocument/hover` handler (Plan 03-04 Task 2; LM-2 doc-comment rendering).
//!
//! On hover request the provider:
//!
//! 1. Looks up the per-document handle from `Documents`.
//! 2. Calls `deal_ffi::safe::ast_json` to obtain the parsed AST as UTF-8
//!    JSON (Phase 1 D-04 alphabetical-key invariant).
//! 3. Translates the request `(line, UTF-16 character)` position into a
//!    UTF-8 byte offset via the document's `ropey::Rope`.
//! 4. Walks the AST recursively to find the DEEPEST node whose `span`
//!    `[start_byte, end_byte]` contains that offset.
//! 5. If the node has a `doc` field containing a `doc_comment` record
//!    (Phase 1 LM-2 comment-attachment), strips the `/** … */` chrome,
//!    de-indents the leading `* ` line markers, and returns the result
//!    as Markdown HoverContents.
//! 6. Returns `Ok(None)` if there is no handle, no AST, no node hit, or
//!    no attached doc comment — the LSP MUST NOT panic on any of these.

use serde_json::Value;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    Hover, HoverContents, HoverParams, MarkupContent, MarkupKind, Position, Range, Url,
};

use crate::documents::Documents;
use crate::index::{byte_to_position, Index};

/// Implementation of `textDocument/hover`.
pub async fn handle_hover(
    documents: &Documents,
    index: &Index,
    params: HoverParams,
) -> LspResult<Option<Hover>> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;
    Ok(hover_for(documents, index, uri, position).await)
}

/// Inner helper — returns `Option<Hover>` so the LSP-error mapping stays
/// at the trait boundary.
///
/// Two-tier result (innermost containing node wins within each tier):
///   1. If a containing node carries a `/** … */` doc comment, render it
///      (prefixed with a `kind name` signature line when available).
///   2. Otherwise, render a signature line for the innermost node we can
///      label — `part def BatteryPack`, `type vehicle.battery.BatteryPack`,
///      etc. — so hover fires on any definition or identifier, not only on
///      doc-commented declarations.
async fn hover_for(
    documents: &Documents,
    index: &Index,
    uri: &Url,
    position: Position,
) -> Option<Hover> {
    let handle_arc = documents.get_handle(uri)?;
    let rope = documents.get_buffer(uri)?;

    let byte_offset = position_to_byte(&rope, position)?;

    let ast_bytes = {
        let guard = handle_arc.lock().await;
        deal_ffi::safe::ast_json(&guard)?
    };
    if ast_bytes.is_empty() {
        return None;
    }
    let root: Value = serde_json::from_slice(&ast_bytes).ok()?;

    // Descend through the outer envelope to the span-bearing tree root.
    let entry = if root.get("span").is_some() {
        &root
    } else {
        root.get("root")?
    };

    // Collect the chain of nodes (outer → inner) whose span brackets the offset.
    let mut stack: Vec<&Value> = Vec::new();
    collect_containing(entry, byte_offset, &mut stack);
    if stack.is_empty() {
        return None;
    }

    // Tier 0 (P3 WS-C5): if the cursor is on a resolvable reference — a type
    // reference or a name that binds to a declaration — show the *declaration's*
    // hover (signature + doc + SysML mapping), the way goto-definition resolves
    // it. The hover range stays on the reference token under the cursor. Hovering
    // a declaration's own name yields no candidate here, so it falls through to
    // the local rendering below (which renders the declaration itself).
    if let Some(value) = declaration_hover_value(documents, index, uri, position).await {
        // `stack` is non-empty; the innermost node is the reference token.
        return Some(make_hover(&rope, stack[stack.len() - 1], value));
    }

    // Tier 1/2: render the innermost local node (declarations, and anything not
    // resolvable to another declaration).
    if let Some((value, node)) = hover_value_for_stack(&stack) {
        return Some(make_hover(&rope, node, value));
    }
    None
}

/// Render the hover value for the innermost meaningful node in a containing
/// chain: prefer the innermost node carrying a doc comment (signature + doc),
/// else the innermost node we can label by kind. Returns the value and the node
/// it came from (so the caller can range the hover to that node).
fn hover_value_for_stack<'a>(stack: &[&'a Value]) -> Option<(String, &'a Value)> {
    // Tier 1: innermost node carrying a doc comment.
    for node in stack.iter().rev() {
        if let Some(doc) = extract_doc_text(node) {
            let mut value = match node_label(node) {
                Some(sig) => format!("{sig}\n\n{doc}"),
                None => doc,
            };
            value.push_str(&sysml_suffix(node));
            return Some((value, node));
        }
    }
    // Tier 2: innermost node we can label by kind (+ name).
    for node in stack.iter().rev() {
        if let Some(sig) = node_label(node) {
            let mut value = sig.clone();
            value.push_str(&sysml_suffix(node));
            return Some((value, node));
        }
    }
    None
}

/// P3 WS-C5: resolve the reference under the cursor to its declaration (reusing
/// goto-definition's resolver) and render *that* declaration's hover value, so
/// hovering a use shows the same rich content as hovering the definition.
/// Returns `None` when the cursor is not on a resolvable reference (e.g. it is
/// on a declaration name, a keyword, or an unresolved symbol).
async fn declaration_hover_value(
    documents: &Documents,
    index: &Index,
    uri: &Url,
    position: Position,
) -> Option<String> {
    let (_path, decl) = crate::definition::resolved_path_at(documents, index, uri, position).await?;

    let handle_arc = documents.get_handle(&decl.uri)?;
    let rope = documents.get_buffer(&decl.uri)?;
    let byte = position_to_byte(&rope, decl.range.start)?;
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
    let mut stack: Vec<&Value> = Vec::new();
    collect_containing(entry, byte, &mut stack);
    hover_value_for_stack(&stack).map(|(value, _)| value)
}

/// Build the trailing SysML-v2-mapping block for a node (P1 WS-C), or an empty
/// string when the node kind has no recorded mapping — the hover then degrades
/// to exactly its pre-P1 output (signature + doc), never an error.
///
/// Markdown hard line-breaks use a trailing two-space sequence so each fact
/// renders on its own line in the VS Code hover popup.
fn sysml_suffix(node: &Value) -> String {
    let kind = match node.get("k").and_then(|v| v.as_str()) {
        Some(k) => k,
        None => return String::new(),
    };
    let mut out = String::new();
    if let Some(m) = crate::sysml_mapping::resolve(kind) {
        out.push_str(&format!(
            "\n\n---\n**SysML v2** · `{}`  ({})",
            m.metaclass, m.clause
        ));
        if !m.kerml.is_empty() {
            out.push_str(&format!("  \n**KerML basis** · {}", m.kerml));
        }
        if let Some(inj) = &m.injects {
            out.push_str(&format!("  \n_+ injects {inj}_"));
        }
        if let Some(marker) = &m.marker {
            out.push_str(&format!("  \n_marker · {marker}_"));
        }
    }
    // qualifiedName for reference-like nodes that carry a dotted path. Def nodes
    // expose only a bare `name` (no package context on the AST), so a true
    // package-qualified name for definitions is a follow-up that reads the
    // enclosing package from the workspace index.
    if let Some(qn) = qualified_name_colon(node) {
        out.push_str(&format!("  \n**qualifiedName** · `{qn}`"));
    }
    out
}

/// Join a node's `name_segments` / `target_segments` with `::` (SysML
/// qualified-name separator) when there are at least two segments — i.e. a real
/// dotted path, not a single bare identifier. Returns `None` otherwise.
fn qualified_name_colon(node: &Value) -> Option<String> {
    let arr = node
        .get("name_segments")
        .or_else(|| node.get("target_segments"))?
        .as_array()?;
    let parts: Vec<&str> = arr.iter().filter_map(|s| s.as_str()).collect();
    if parts.len() < 2 {
        return None;
    }
    Some(parts.join("::"))
}

/// Build a Markdown hover from a node's span + a precomputed value string.
fn make_hover(rope: &ropey::Rope, node: &Value, value: String) -> Hover {
    let range = node.get("span").and_then(parse_span).map(|s| Range {
        start: byte_to_position(rope, s[0]),
        end: byte_to_position(rope, s[1]),
    });
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value,
        }),
        range,
    }
}

/// Push every node on the path from `node` down to the deepest descendant
/// whose `span` brackets `byte_offset` (outermost first). Span-less nodes are
/// transparent — we descend through them but do not record them. Mirrors
/// `definition::collect_containing`.
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

/// Render a one-line signature for a node as a fenced `deal` code block, e.g.
/// ````text
/// ```deal
/// part def BatteryPack
/// ```
/// ````
/// Returns `None` for nodes with no `k` field (not a meaningful hover target).
///
/// For `calc_def` nodes this renders the full signature including params,
/// return type, and return contract (Phase 05.2 D-04 rich hover):
/// ````text
/// ```deal
/// calc def Drag(in velocity : Real, in cd : Real) : Force => ± percent(1), PositiveForce
/// ```
/// ````
fn node_label(node: &Value) -> Option<String> {
    let kind = node.get("k").and_then(|v| v.as_str())?;

    // Phase 05.2 (D-04): rich calc_def signature hover.
    // Phase 6 seam: Phase 05.2 adds ONLY the calc_def arm here. Phase 6 extends
    // this function with constraint_def parameter display and cross-file type
    // resolution — it does NOT refactor the node_label infrastructure.
    if kind == "calc_def" {
        if let Some(sig) = callable_signature(node, "calc def") {
            return Some(format!("```deal\n{sig}\n```"));
        }
    }
    // G1: parameterized constraint definitions get the same rich signature.
    if kind == "constraint_def" {
        if let Some(sig) = callable_signature(node, "constraint def") {
            return Some(format!("```deal\n{sig}\n```"));
        }
    }

    let name = node
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| join_segments(node.get("name_segments")))
        .or_else(|| join_segments(node.get("target_segments")));
    let keyword = kind_to_keyword(kind);
    let body = match name {
        Some(n) if !n.is_empty() => format!("{keyword} {n}"),
        _ => keyword.to_string(),
    };
    Some(format!("```deal\n{body}\n```"))
}

/// Render a rich signature for a `calc_def` AST node.
///
/// AST shape (from Phase 05.2 Plan 03, live-probed):
/// ```json
/// { "k": "calc_def", "name": "Drag",
///   "params": { "k": "param_list", "params": [
///     { "k": "param_decl", "name": "velocity", "direction": "in",
///       "type": { "k": "type_annotation", "name_segments": ["Real"] } }
///   ]},
///   "type_node": { "k": "type_annotation", "name_segments": ["Force"] },
///   "return_contract": { ... }
/// }
/// ```
///
/// Returns `None` when the node lacks a name (graceful degradation — caller
/// falls back to the generic `kind_to_keyword` label).
///
/// `keyword` is the rendered prefix (`calc def` / `constraint def`). A `calc`
/// always carries a `param_list`; a `constraint` may have none, in which case
/// no parens are rendered (matching how it is written). Return type and
/// contract are emitted only when present, so constraints — which have neither
/// — degrade to `constraint def Name(params)`.
fn callable_signature(node: &Value, keyword: &str) -> Option<String> {
    let name = node.get("name")?.as_str()?;

    // Parameter list — only when an actual `param_list` node is present.
    let params_str = match node.get("params") {
        Some(p) if p.is_object() => render_param_list(p),
        _ => String::new(),
    };

    let return_type = node
        .get("type_node")
        .map(render_type_annotation)
        .unwrap_or_default();
    let contract_str = node
        .get("return_contract")
        .map(render_return_contract)
        .unwrap_or_default();

    let mut sig = format!("{keyword} {name}{params_str}");
    if !return_type.is_empty() {
        sig.push_str(" : ");
        sig.push_str(&return_type);
    }
    if !contract_str.is_empty() {
        sig.push(' ');
        sig.push_str(&contract_str);
    }
    Some(sig)
}

/// Render a `param_list` node as `(dir name : Type, ...)`.
pub(crate) fn render_param_list(param_list: &Value) -> String {
    let params = match param_list.get("params").and_then(|v| v.as_array()) {
        Some(p) => p,
        None => return "()".to_string(),
    };
    let rendered: Vec<String> = params.iter().map(render_param_decl).collect();
    format!("({})", rendered.join(", "))
}

/// Render a single `param_decl` node as `direction name : Type`.
pub(crate) fn render_param_decl(param: &Value) -> String {
    let dir = param
        .get("direction")
        .and_then(|v| v.as_str())
        .unwrap_or("in");
    let name = param.get("name").and_then(|v| v.as_str()).unwrap_or("_");
    let type_str = param
        .get("type_node")
        .map(render_type_annotation)
        .unwrap_or_default();
    if type_str.is_empty() {
        format!("{dir} {name}")
    } else {
        format!("{dir} {name} : {type_str}")
    }
}

/// Render a `type_annotation` node to its dotted name.
pub(crate) fn render_type_annotation(type_node: &Value) -> String {
    if let Some(segs) = join_segments(type_node.get("name_segments")) {
        return segs;
    }
    if let Some(name) = type_node.get("name").and_then(|v| v.as_str()) {
        return name.to_string();
    }
    String::new()
}

/// Render a `return_contract` node as `=> item1, item2`.
pub(crate) fn render_return_contract(rc: &Value) -> String {
    let items = match rc.get("items").and_then(|v| v.as_array()) {
        Some(i) => i,
        None => return String::new(),
    };
    let rendered: Vec<String> = items.iter().map(render_contract_item).collect();
    if rendered.is_empty() {
        String::new()
    } else {
        format!("=> {}", rendered.join(", "))
    }
}

/// Render a single `precision_spec` or `constraint_ref` contract item.
fn render_contract_item(item: &Value) -> String {
    let kind = item.get("k").and_then(|v| v.as_str()).unwrap_or("");
    match kind {
        "precision_spec" => {
            let prec_kind = item.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            let value = item
                .get("value")
                .and_then(|v| v.get("text").or_else(|| v.get("value")))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            match prec_kind {
                "sig_figures" => format!("sig {value}"),
                _ => format!("± {value}"),
            }
        }
        "constraint_ref" => {
            join_segments(item.get("name_segments")).unwrap_or_else(|| "?".to_string())
        }
        // Fallback: try to render name or name_segments.
        _ => join_segments(item.get("name_segments"))
            .or_else(|| {
                item.get("name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_default(),
    }
}

/// Join a JSON array of string segments with `.` (e.g. type/target paths).
fn join_segments(v: Option<&Value>) -> Option<String> {
    let arr = v?.as_array()?;
    let parts: Vec<&str> = arr.iter().filter_map(|s| s.as_str()).collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("."))
    }
}

/// Map an AST node-kind to a human-facing keyword for the hover signature.
/// Unknown kinds pass through verbatim.
fn kind_to_keyword(kind: &str) -> &str {
    match kind {
        "part_def" => "part def",
        "port_def" => "port def",
        "action_def" => "action def",
        "state_def" => "state def",
        "requirement_def" => "requirement def",
        "constraint_def" => "constraint def",
        // Phase 05.2 (D-04): calc_def keyword mapping — exact copy of constraint_def arm.
        // Phase 6 seam: extend here to add param/return-type display on `calc def` hover labels.
        "calc_def" => "calc def",
        "attribute_def" => "attribute def",
        "item_def" => "item def",
        "interface_def" => "interface def",
        "connection_def" => "connection def",
        "flow_def" => "flow def",
        "allocation_def" => "allocation def",
        "need_def" => "need def",
        "use_case_def" => "use case def",
        "actor_def" => "actor def",
        "type_annotation" => "type",
        "structural_relationship" => "relationship",
        other => other,
    }
}

/// Public re-export of `position_to_byte` for sibling provider modules
/// (definition.rs reuses the same translation logic; centralising
/// avoids a second drift-prone copy).
pub fn position_to_byte_pub(rope: &ropey::Rope, position: Position) -> Option<usize> {
    position_to_byte(rope, position)
}

/// Convert an LSP `(line, UTF-16-character)` Position into a UTF-8 byte
/// offset. Returns `None` if `position.line` is past the document.
fn position_to_byte(rope: &ropey::Rope, position: Position) -> Option<usize> {
    let line = position.line as usize;
    if line >= rope.len_lines() {
        return Some(rope.len_bytes());
    }
    let line_start_char = rope.line_to_char(line);
    let line_slice = rope.line(line);
    // Walk `character` UTF-16 code units forward.
    let mut remaining = position.character as usize;
    let mut byte_within_line: usize = 0;
    for c in line_slice.chars() {
        if remaining == 0 {
            break;
        }
        let units = c.len_utf16();
        if units > remaining {
            // Position lands mid-surrogate; clamp to char start.
            break;
        }
        remaining -= units;
        byte_within_line += c.len_utf8();
    }
    let line_start_byte = rope.char_to_byte(line_start_char);
    Some(line_start_byte + byte_within_line)
}

/// Parse a `span: [u, v]` JSON value into `[usize; 2]`.
fn parse_span(v: &Value) -> Option<[usize; 2]> {
    let arr = v.as_array()?;
    if arr.len() != 2 {
        return None;
    }
    let a = arr[0].as_u64()? as usize;
    let b = arr[1].as_u64()? as usize;
    Some([a, b])
}

/// Recursively walk `node` looking for the deepest descendant whose
/// `span` brackets `byte_offset`. "Deepest" = most specific = innermost
/// matching node, which is the meaningful hover target.
///
/// The AST root envelope is `{ "v":1, "mode":"deal", "filename":"...",
/// "root": { "k":"deal_file", "span":[0,N], ... } }` — the outer
/// envelope has NO `span` field, so we look one level into `root` if
/// the top-level node lacks a span.
///
/// Retained for its unit tests; `hover_for` now uses the `collect_containing`
/// stack walk instead (which can pick an outer doc-bearing node over a deeper
/// span-only node).
#[allow(dead_code)]
fn find_deepest_node_at(node: &Value, byte_offset: usize) -> Option<&Value> {
    // Auto-descend through the outer envelope to the first span-bearing
    // node (the AST's actual `deal_file` root).
    let entry = if node.get("span").is_some() {
        node
    } else if let Some(inner) = node.get("root") {
        inner
    } else {
        // No span and no `root` key — give up.
        return None;
    };
    let span = entry.get("span").and_then(parse_span)?;
    if !(byte_offset >= span[0] && byte_offset < span[1]) {
        return None;
    }
    // This node contains the position — recurse into children to find a
    // tighter match. Children appear as nested objects or arrays of
    // objects in arbitrary keys depending on node kind, so we walk the
    // value tree generically.
    let mut best = entry;
    walk_children(entry, byte_offset, &mut best);
    Some(best)
}

#[allow(dead_code)]
fn walk_children<'a>(node: &'a Value, byte_offset: usize, best: &mut &'a Value) {
    match node {
        Value::Object(map) => {
            for (_k, v) in map.iter() {
                consider(v, byte_offset, best);
            }
        }
        Value::Array(items) => {
            for item in items.iter() {
                consider(item, byte_offset, best);
            }
        }
        _ => {}
    }
}

#[allow(dead_code)]
fn consider<'a>(v: &'a Value, byte_offset: usize, best: &mut &'a Value) {
    if let Some(span) = v.get("span").and_then(parse_span) {
        if byte_offset >= span[0] && byte_offset < span[1] {
            let cur_span = best
                .get("span")
                .and_then(parse_span)
                .unwrap_or([0, usize::MAX]);
            let cur_width = cur_span[1].saturating_sub(cur_span[0]);
            let new_width = span[1].saturating_sub(span[0]);
            if new_width <= cur_width {
                *best = v;
            }
            walk_children(v, byte_offset, best);
        }
    } else {
        // Not a span-bearing node — descend anyway in case children carry spans.
        walk_children(v, byte_offset, best);
    }
}

/// Extract the doc comment text from a node, if any.
///
/// AST shape (live-probed): a definition node has a `doc` field
/// holding `{ k: "doc_comment", span: [...], text: "/** … */" }`.
/// The `text` field carries the raw `/** … */` source slice.
fn extract_doc_text(node: &Value) -> Option<String> {
    let doc = node.get("doc")?;
    if doc.is_null() {
        return None;
    }
    let text = doc.get("text")?.as_str()?;
    Some(render_doc_comment_as_markdown(text))
}

/// Strip `/**` / `*/` delimiters and leading `* ` line markers from a
/// JSDoc-style block comment, returning Markdown-friendly text.
///
/// Examples:
///
/// ```text
/// /**
///  * Foo bar
///  * baz
///  */
/// ```
///
/// becomes
///
/// ```text
/// Foo bar
/// baz
/// ```
fn render_doc_comment_as_markdown(raw: &str) -> String {
    let trimmed = raw
        .trim_start_matches("/**")
        .trim_end_matches("*/")
        .trim_matches(|c: char| c == '\n' || c == '\r' || c == '*');
    let mut out = String::with_capacity(trimmed.len());
    let mut first = true;
    for line in trimmed.lines() {
        let stripped = line
            .trim_start()
            .trim_start_matches('*')
            .trim_start_matches(' ');
        if !first {
            out.push('\n');
        }
        first = false;
        out.push_str(stripped);
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::Rope;
    use serde_json::json;

    #[test]
    fn position_to_byte_basic_ascii() {
        let rope = Rope::from_str("abc\ndef");
        // (0,0) → byte 0, (0,2) → byte 2, (1,1) → byte 5 (d=4, e=5)
        assert_eq!(position_to_byte(&rope, Position::new(0, 0)), Some(0));
        assert_eq!(position_to_byte(&rope, Position::new(0, 2)), Some(2));
        assert_eq!(position_to_byte(&rope, Position::new(1, 1)), Some(5));
    }

    #[test]
    fn position_to_byte_past_end_clamps() {
        let rope = Rope::from_str("a");
        // Line 99 doesn't exist → end of doc.
        assert_eq!(position_to_byte(&rope, Position::new(99, 0)), Some(1));
    }

    #[test]
    fn position_to_byte_multibyte_utf16() {
        // "α" = 1 UTF-16 code unit / 2 UTF-8 bytes.
        // "𝄞" = 2 UTF-16 code units / 4 UTF-8 bytes (surrogate pair).
        let rope = Rope::from_str("α𝄞x");
        // (0,0)→0, (0,1)→2 (after α), (0,3)→6 (after 𝄞 = 1+2 units)
        assert_eq!(position_to_byte(&rope, Position::new(0, 0)), Some(0));
        assert_eq!(position_to_byte(&rope, Position::new(0, 1)), Some(2));
        assert_eq!(position_to_byte(&rope, Position::new(0, 3)), Some(6));
    }

    #[test]
    fn render_doc_strips_jsdoc_chrome() {
        let raw = "/**\n * First line.\n * Second line.\n */";
        let rendered = render_doc_comment_as_markdown(raw);
        assert_eq!(rendered, "First line.\nSecond line.");
    }

    #[test]
    fn render_doc_handles_single_line_block() {
        let raw = "/** inline note */";
        let rendered = render_doc_comment_as_markdown(raw);
        assert_eq!(rendered, "inline note");
    }

    #[test]
    fn find_deepest_returns_inner_span() {
        // Outer span 0..100; inner span 10..20; inner-inner 12..15.
        let tree = json!({
            "k": "outer",
            "span": [0, 100],
            "members": [
                {
                    "k": "middle",
                    "span": [10, 20],
                    "child": {
                        "k": "leaf",
                        "span": [12, 15]
                    }
                }
            ]
        });
        let hit = find_deepest_node_at(&tree, 13).unwrap();
        assert_eq!(hit.get("k").and_then(|v| v.as_str()), Some("leaf"));
    }

    #[test]
    fn extract_doc_text_from_real_ast_shape() {
        // Mirrors the live-probed shape: { k, span, doc: { k: "doc_comment", span, text } }.
        let node = json!({
            "k": "part_def",
            "span": [657, 1562],
            "doc": {
                "k": "doc_comment",
                "span": [657, 769],
                "text": "/**\n * Individual lithium-ion pouch cell.\n * NMC 811 chemistry — high energy density.\n */"
            }
        });
        let text = extract_doc_text(&node).unwrap();
        assert!(text.contains("Individual lithium-ion pouch cell."));
        assert!(text.contains("NMC 811 chemistry"));
    }

    #[test]
    fn extract_doc_text_returns_none_when_doc_null() {
        let node = json!({ "k": "part_def", "span": [0, 10], "doc": null });
        assert!(extract_doc_text(&node).is_none());
    }

    #[test]
    fn extract_doc_text_returns_none_when_doc_missing() {
        let node = json!({ "k": "part_def", "span": [0, 10] });
        assert!(extract_doc_text(&node).is_none());
    }

    /// Live-AST exercise: parse the showcase battery.deal, walk to the
    /// part_def BatteryCell node, and verify hover extracts the doc.
    /// This catches the path that the integration test hits, without
    /// the LspService channel-plumbing overhead.
    #[test]
    fn live_ast_part_def_yields_doc() {
        let path = "../spec/examples/showcase/packages/vehicle/battery.deal";
        let src = std::fs::read(path).expect("read battery.deal");
        let handle = deal_ffi::safe::parse(&src, path).expect("parse");
        let ast = deal_ffi::safe::ast_json(&handle).expect("ast");
        let root: Value = serde_json::from_slice(&ast).expect("ast parse");
        // Locate "part def BatteryCell" in source, probe inside.
        let needle = "part def BatteryCell";
        let s = std::str::from_utf8(&src).unwrap();
        let byte = s.find(needle).expect("needle in source") + 5;
        let node = find_deepest_node_at(&root, byte).expect("found a node at byte");
        let kind = node.get("k").and_then(|v| v.as_str());
        // The expected hit is the part_def itself (no inner span contains 775).
        assert_eq!(
            kind,
            Some("part_def"),
            "deepest hit at byte {byte}: {kind:?}"
        );
        let doc = extract_doc_text(node).expect("part_def carries a doc comment");
        assert!(doc.contains("Individual lithium-ion pouch cell"));
    }

    #[test]
    fn node_label_part_def_with_name() {
        let n = json!({ "k": "part_def", "span": [0, 10], "name": "BatteryPack" });
        assert_eq!(
            node_label(&n).as_deref(),
            Some("```deal\npart def BatteryPack\n```")
        );
    }

    #[test]
    fn node_label_p1_added_kinds_render_keyword_and_name() {
        for (kind, kw) in [
            ("allocation_def", "allocation def"),
            ("need_def", "need def"),
            ("use_case_def", "use case def"),
            ("actor_def", "actor def"),
        ] {
            let n = json!({ "k": kind, "span": [0, 10], "name": "X" });
            assert_eq!(
                node_label(&n).as_deref(),
                Some(format!("```deal\n{kw} X\n```").as_str()),
                "kind {kind}"
            );
        }
    }

    #[test]
    fn sysml_suffix_structural_behavioral_expression() {
        // Structural.
        let part = json!({ "k": "part_def", "span": [0, 10], "name": "BatteryCell" });
        let s = sysml_suffix(&part);
        assert!(s.contains("**SysML v2**"));
        assert!(s.contains("`PartDefinition`"));
        assert!(s.contains("8.3.11.2"));
        assert!(s.contains("**KerML basis**"));
        // Behavioral with injected pair.
        let dec = json!({ "k": "decide_block", "span": [0, 10] });
        let d = sysml_suffix(&dec);
        assert!(d.contains("`DecisionNode`"));
        assert!(d.contains("injects"));
        assert!(d.contains("MergeNode"));
        // Expression.
        let op = json!({ "k": "operator_expr", "span": [0, 10] });
        assert!(sysml_suffix(&op).contains("`OperatorExpression`"));
    }

    #[test]
    fn sysml_suffix_actor_shows_marker() {
        let actor = json!({ "k": "actor_def", "span": [0, 10], "name": "Operator" });
        let s = sysml_suffix(&actor);
        assert!(s.contains("`PartDefinition`"));
        assert!(s.contains("marker"));
        assert!(s.contains("actor"));
    }

    #[test]
    fn sysml_suffix_unmapped_kind_is_empty() {
        // Graceful degradation: a node kind with no row adds nothing.
        let n = json!({ "k": "doc_comment", "span": [0, 10] });
        assert_eq!(sysml_suffix(&n), "");
        let no_kind = json!({ "span": [0, 10] });
        assert_eq!(sysml_suffix(&no_kind), "");
    }

    #[test]
    fn qualified_name_colon_joins_multi_segment_paths_only() {
        let multi = json!({ "k": "type_annotation", "name_segments": ["vehicle", "battery", "BatteryPack"] });
        assert_eq!(
            qualified_name_colon(&multi).as_deref(),
            Some("vehicle::battery::BatteryPack")
        );
        // A single bare identifier is not a qualified name.
        let single = json!({ "k": "type_annotation", "name_segments": ["BatteryPack"] });
        assert!(qualified_name_colon(&single).is_none());
    }

    #[test]
    fn node_label_type_annotation_joins_segments() {
        let n = json!({
            "k": "type_annotation",
            "span": [0, 10],
            "name_segments": ["vehicle", "battery", "BatteryPack"]
        });
        assert_eq!(
            node_label(&n).as_deref(),
            Some("```deal\ntype vehicle.battery.BatteryPack\n```")
        );
    }

    #[test]
    fn node_label_unknown_kind_passes_through_keyword_only() {
        let n = json!({ "k": "deal_file", "span": [0, 100] });
        assert_eq!(node_label(&n).as_deref(), Some("```deal\ndeal_file\n```"));
    }

    #[test]
    fn node_label_requires_kind() {
        let n = json!({ "span": [0, 10] });
        assert!(node_label(&n).is_none());
    }

    // ── Phase 05.2 (D-04): calc_def hover tests ──────────────────────────

    #[test]
    fn kind_to_keyword_maps_calc_def() {
        assert_eq!(kind_to_keyword("calc_def"), "calc def");
    }

    #[test]
    fn node_label_constraint_def_with_params_renders_signature() {
        // constraint def WithinLimit(in x : Real, in max : Real)
        let n = json!({
            "k": "constraint_def",
            "span": [0, 60],
            "name": "WithinLimit",
            "params": {
                "k": "param_list",
                "params": [
                    { "k": "param_decl", "name": "x", "direction": "in",
                      "type_node": { "k": "type_annotation", "name_segments": ["Real"] } },
                    { "k": "param_decl", "name": "max", "direction": "in",
                      "type_node": { "k": "type_annotation", "name_segments": ["Real"] } }
                ]
            }
        });
        let label = node_label(&n).unwrap();
        assert!(
            label.contains("constraint def WithinLimit(in x : Real, in max : Real)"),
            "got: {label}"
        );
    }

    #[test]
    fn node_label_constraint_def_without_params_omits_parens() {
        let n = json!({ "k": "constraint_def", "span": [0, 20], "name": "AlwaysOn" });
        let label = node_label(&n).unwrap();
        assert!(label.contains("constraint def AlwaysOn"), "got: {label}");
        assert!(!label.contains("AlwaysOn("), "should not render empty parens: {label}");
    }

    #[test]
    fn node_label_calc_def_simple_signature() {
        // calc def Drag(in velocity : Real) : Force
        let n = json!({
            "k": "calc_def",
            "span": [0, 50],
            "name": "Drag",
            "params": {
                "k": "param_list",
                "params": [
                    {
                        "k": "param_decl",
                        "name": "velocity",
                        "direction": "in",
                        "type_node": {
                            "k": "type_annotation",
                            "name_segments": ["Real"]
                        }
                    }
                ]
            },
            "type_node": {
                "k": "type_annotation",
                "name_segments": ["Force"]
            }
        });
        let label = node_label(&n).unwrap();
        assert!(
            label.contains("calc def Drag(in velocity : Real) : Force"),
            "got: {label}"
        );
        assert!(label.starts_with("```deal\n"));
        assert!(label.ends_with("\n```"));
    }

    #[test]
    fn node_label_calc_def_with_tolerance_contract() {
        // calc def Drag(...) : Force => ± percent(1), PositiveForce
        // Contract items stored as constraint_ref nodes.
        let n = json!({
            "k": "calc_def",
            "span": [0, 80],
            "name": "Drag",
            "params": { "k": "param_list", "params": [] },
            "type_node": {
                "k": "type_annotation",
                "name_segments": ["Force"]
            },
            "return_contract": {
                "k": "return_contract",
                "items": [
                    {
                        "k": "precision_spec",
                        "kind": "tolerance_relative",
                        "value": { "k": "identifier", "value": "percent(1)" }
                    },
                    {
                        "k": "constraint_ref",
                        "name_segments": ["PositiveForce"]
                    }
                ]
            }
        });
        let label = node_label(&n).unwrap();
        assert!(
            label.contains("calc def Drag() : Force => "),
            "got: {label}"
        );
        assert!(label.contains("PositiveForce"), "got: {label}");
    }

    #[test]
    fn node_label_calc_def_with_sig_contract() {
        let n = json!({
            "k": "calc_def",
            "span": [0, 60],
            "name": "EstimatedRange",
            "params": { "k": "param_list", "params": [] },
            "type_node": {
                "k": "type_annotation",
                "name_segments": ["Real"]
            },
            "return_contract": {
                "k": "return_contract",
                "items": [
                    {
                        "k": "precision_spec",
                        "kind": "sig_figures",
                        "value": { "k": "int_literal", "text": "4" }
                    }
                ]
            }
        });
        let label = node_label(&n).unwrap();
        assert!(
            label.contains("calc def EstimatedRange() : Real => sig 4"),
            "got: {label}"
        );
    }

    #[test]
    fn node_label_calc_def_no_name_falls_back() {
        // When name is missing, callable_signature returns None and we
        // fall through to the generic "calc def" label.
        let n = json!({
            "k": "calc_def",
            "span": [0, 10]
        });
        let label = node_label(&n).unwrap();
        assert_eq!(label, "```deal\ncalc def\n```");
    }

    #[test]
    fn collect_containing_builds_outer_to_inner_chain() {
        let tree = json!({
            "k": "outer",
            "span": [0, 100],
            "members": [
                { "k": "middle", "span": [10, 40],
                  "child": { "k": "leaf", "span": [12, 15] } }
            ]
        });
        let mut stack: Vec<&Value> = Vec::new();
        collect_containing(&tree, 13, &mut stack);
        let kinds: Vec<&str> = stack
            .iter()
            .filter_map(|n| n.get("k").and_then(|v| v.as_str()))
            .collect();
        assert_eq!(kinds, vec!["outer", "middle", "leaf"]);
    }
}
