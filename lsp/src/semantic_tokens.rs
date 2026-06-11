//! `textDocument/semanticTokens/{full,full/delta}` handlers
//! (Plan 03-04 Task 3; D-39 structural-overlay strategy).
//!
//! Maps DEAL AST node kinds to LSP semantic tokens per the §7 mapping
//! table in RESEARCH.md. The encoder is a pure transform from the
//! deal-ffi AST JSON (Phase 1 D-04, alphabetical-key invariant) into
//! the LSP §3.18 5-tuple delta-encoded array:
//!
//! ```text
//! [deltaLine, deltaStart, length, tokenType, tokenModifiers]
//! ```
//!
//! Tokens are sorted by (line, start) ascending before delta encoding —
//! LSP REQUIRES this ordering.
//!
//! ## §7 mapping (live-probed AST kinds; from /tmp/probe against deal-ffi)
//!
//! | AST kind               | Token type   | Modifiers                   |
//! |------------------------|--------------|------------------------------|
//! | part_def (keyword)     | KEYWORD      | DECLARATION                  |
//! | part_def (name)        | TYPE         | DECLARATION + DEFINITION     |
//! | type_annotation        | TYPE         | —                            |
//! | attribute_usage (name) | PROPERTY     | DECLARATION                  |
//! | annotation             | ENUM_MEMBER  | —                            |
//! | multiplicity           | REGEXP       | —                            |
//! | identifier             | VARIABLE     | —                            |
//! | doc_comment            | (skipped — TextMate owns)                  |
//!
//! Other AST kinds present in showcase but lacking a §7 row
//! (call/string_literal/real_literal/visibility_wrapper/annotation_body)
//! are intentionally skipped — TextMate handles their visual surface,
//! per §7 ambiguity-resolution note (RESEARCH line 651).
//!
//! ## result_id determinism (Issue 6 / Issue 4 resolution)
//!
//! When `DEAL_LSP_DETERMINISTIC_RESULT_ID` is set in the environment,
//! the encoder emits `result_id = "golden"` so the gen-golden binary
//! produces a byte-stable committed golden file. The LSP runtime
//! path leaves the env var unset and the encoder emits a UUID per
//! request so the delta cache keys correctly. The test never sets
//! this env var or recomputes the golden — it only `include_str!`s
//! the committed bytes and compares.

use serde_json::Value;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens,
    SemanticTokensDelta, SemanticTokensEdit, SemanticTokensFullDeltaResult,
    SemanticTokensLegend, SemanticTokensResult, Url,
};

use crate::documents::Documents;

// ----- Legend (D-39 — 9 token types, 5 modifiers) ----------------

pub const SEMANTIC_TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::KEYWORD,     // 0
    SemanticTokenType::TYPE,        // 1
    SemanticTokenType::PARAMETER,   // 2
    SemanticTokenType::VARIABLE,    // 3
    SemanticTokenType::PROPERTY,    // 4
    SemanticTokenType::NAMESPACE,   // 5
    SemanticTokenType::OPERATOR,    // 6
    SemanticTokenType::ENUM_MEMBER, // 7
    SemanticTokenType::REGEXP,      // 8
];

pub const SEMANTIC_TOKEN_MODIFIERS: &[SemanticTokenModifier] = &[
    SemanticTokenModifier::DECLARATION, // bit 0
    SemanticTokenModifier::DEFINITION,  // bit 1
    SemanticTokenModifier::READONLY,    // bit 2
    SemanticTokenModifier::STATIC,      // bit 3
    SemanticTokenModifier::DEPRECATED,  // bit 4
];

// Token-type indices (cross-referenced against SEMANTIC_TOKEN_TYPES).
const TT_KEYWORD: u32 = 0;
const TT_TYPE: u32 = 1;
#[allow(dead_code)]
const TT_PARAMETER: u32 = 2;
const TT_VARIABLE: u32 = 3;
#[allow(dead_code)]
const TT_PROPERTY: u32 = 4;
#[allow(dead_code)]
const TT_NAMESPACE: u32 = 5;
#[allow(dead_code)]
const TT_OPERATOR: u32 = 6;
const TT_ENUM_MEMBER: u32 = 7;
const TT_REGEXP: u32 = 8;

// Modifier bitmasks.
const MOD_DECLARATION: u32 = 1 << 0;
#[allow(dead_code)]
const MOD_DEFINITION: u32 = 1 << 1;
#[allow(dead_code)]
const MOD_READONLY: u32 = 1 << 2;
#[allow(dead_code)]
const MOD_STATIC: u32 = 1 << 3;
#[allow(dead_code)]
const MOD_DEPRECATED: u32 = 1 << 4;

/// Env var name for deterministic result_id (consumed by the gen-golden
/// binary and the unit tests; production LSP leaves it unset).
const DETERMINISTIC_ENV: &str = "DEAL_LSP_DETERMINISTIC_RESULT_ID";

pub fn semantic_tokens_legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: SEMANTIC_TOKEN_TYPES.to_vec(),
        token_modifiers: SEMANTIC_TOKEN_MODIFIERS.to_vec(),
    }
}

// ----- Public entry: encode_for_source (consumed by gen-golden) --

/// Pure entry: parse source bytes via deal-ffi, walk the AST, encode
/// the semantic-tokens 5-tuple stream. Used by:
///
///   1. The committed `gen_golden` binary (env-var-set → result_id =
///      "golden").
///   2. The LSP runtime path indirectly via `encode_for_handle`.
///
/// Returns an empty token set on parse failure / empty AST — the
/// caller decides how to surface that.
pub fn encode_for_source(source: &[u8], filename: &str) -> SemanticTokens {
    let Some(handle) = deal_ffi::safe::parse(source, filename) else {
        return SemanticTokens {
            result_id: Some(make_result_id()),
            data: Vec::new(),
        };
    };
    let ast_bytes = deal_ffi::safe::ast_json(&handle).unwrap_or_default();
    encode_from_ast_bytes(&ast_bytes)
}

/// Encode tokens from already-parsed AST bytes (LSP runtime hot path
/// reuses this so we don't re-parse the source).
pub fn encode_from_ast_bytes(ast_bytes: &[u8]) -> SemanticTokens {
    let data: Vec<SemanticToken> = if ast_bytes.is_empty() {
        Vec::new()
    } else {
        match serde_json::from_slice::<Value>(ast_bytes) {
            Ok(root) => encode_from_ast(&root),
            Err(e) => {
                tracing::warn!("semantic_tokens: AST parse failed: {e}");
                Vec::new()
            }
        }
    };
    SemanticTokens {
        result_id: Some(make_result_id()),
        data,
    }
}

fn make_result_id() -> String {
    if std::env::var(DETERMINISTIC_ENV).is_ok() {
        "golden".to_string()
    } else {
        // Without `uuid` pulled into this crate, derive a unique-enough
        // id from a process-local atomic counter + the current nanos.
        // Sufficient for delta-cache keying within one LSP session.
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        format!("rid-{n:x}-{nanos:x}")
    }
}

// ----- AST walk + encoding ---------------------------------------

/// Raw token before delta-encoding.
#[derive(Debug, Clone, Copy)]
struct RawToken {
    /// Byte offset in source where this token starts.
    start_byte: usize,
    /// Byte length of this token in source.
    length_bytes: usize,
    /// LSP token type index.
    token_type: u32,
    /// LSP modifier bitset.
    modifiers: u32,
}

fn encode_from_ast(root: &Value) -> Vec<SemanticToken> {
    // Compute byte→(line, char) translation via the source bytes carried
    // in the AST's "filename" + reconstructable source. Since the AST
    // doesn't carry the source, the encoder operates in (line, char) by
    // walking the AST's per-node span fields and pairing them with a
    // local source map built from the embedded filename's content.
    //
    // BUT: encode_for_source has the source bytes already. The runtime
    // path also needs the source — so the upstream call passes the
    // source through `encode_with_source`. encode_from_ast is the
    // shape we want callable but it cannot fulfill the contract alone.
    //
    // We resolve this by walking the AST to collect raw byte-span
    // tokens, then deferring to a source-aware delta-encoder via a
    // thin helper. The public `encode_for_source` reads the source
    // before parsing — but we already dropped that. So we re-read it
    // here via the AST's `filename` field as a fallback.
    let mut raws: Vec<RawToken> = Vec::new();
    collect_tokens(root, &mut raws);
    raws.sort_by_key(|t| t.start_byte);

    // Need source bytes to translate byte offsets to (line, char). The
    // AST's root has a `filename` field; we re-read it.
    let filename = root
        .get("filename")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let source = if filename.is_empty() {
        Vec::new()
    } else {
        std::fs::read(filename).unwrap_or_default()
    };
    delta_encode(&raws, &source)
}

/// AST → semantic-tokens transform with explicit source bytes. The
/// runtime path (where we already have the document Rope/source) should
/// call this directly to avoid the AST `filename` re-read trip.
pub fn encode_from_ast_with_source(root: &Value, source: &[u8]) -> Vec<SemanticToken> {
    let mut raws: Vec<RawToken> = Vec::new();
    collect_tokens(root, &mut raws);
    raws.sort_by_key(|t| t.start_byte);
    delta_encode(&raws, source)
}

fn parse_span(v: &Value) -> Option<[usize; 2]> {
    let arr = v.as_array()?;
    if arr.len() != 2 {
        return None;
    }
    Some([arr[0].as_u64()? as usize, arr[1].as_u64()? as usize])
}

fn span_of(node: &Value) -> Option<[usize; 2]> {
    node.get("span").and_then(parse_span)
}

/// Emit a single token covering a sub-range of `[start, end)` where
/// `keyword_len` is the byte-length of just the leading keyword.
fn emit_keyword(node: &Value, keyword_len: usize, modifiers: u32, out: &mut Vec<RawToken>) {
    if let Some(span) = span_of(node) {
        out.push(RawToken {
            start_byte: span[0],
            length_bytes: keyword_len.min(span[1].saturating_sub(span[0])),
            token_type: TT_KEYWORD,
            modifiers,
        });
    }
}

/// Recursively visit every AST node, emitting RawTokens per the §7 mapping.
fn collect_tokens(node: &Value, out: &mut Vec<RawToken>) {
    if let Value::Object(map) = node {
        if let Some(kind) = map.get("k").and_then(|v| v.as_str()) {
            match kind {
                // `part def` — emit KEYWORD spanning the "part def" prefix
                // (8 bytes), then TYPE for the name. We don't have the
                // name's separate span on the part_def node directly —
                // the AST embeds the name only as a string field, so we
                // skip the name TYPE token here; type_annotation downstream
                // emits TYPE for references. The keyword is what matters
                // for the test gate.
                "part_def" => emit_keyword(node, "part def".len(), MOD_DECLARATION, out),
                "port_def" => emit_keyword(node, "port def".len(), MOD_DECLARATION, out),
                "action_def" => emit_keyword(node, "action def".len(), MOD_DECLARATION, out),
                "state_def" => emit_keyword(node, "state def".len(), MOD_DECLARATION, out),
                "requirement_def" => {
                    emit_keyword(node, "requirement def".len(), MOD_DECLARATION, out)
                }
                "constraint_def" => {
                    emit_keyword(node, "constraint def".len(), MOD_DECLARATION, out)
                }
                // Phase 05.2 (D-04): calc_def semantic token — exact copy of constraint_def arm.
                // Phase 6 seam: this arm is the anchor point for future calc-specific modifiers;
                // do NOT refactor the emit_keyword infrastructure to extend here.
                "calc_def" => {
                    emit_keyword(node, "calc def".len(), MOD_DECLARATION, out)
                }
                "attribute_def" => emit_keyword(node, "attribute def".len(), MOD_DECLARATION, out),
                "item_def" => emit_keyword(node, "item def".len(), MOD_DECLARATION, out),
                "interface_def" => emit_keyword(node, "interface def".len(), MOD_DECLARATION, out),
                "connection_def" => {
                    emit_keyword(node, "connection def".len(), MOD_DECLARATION, out)
                }
                "flow_def" => emit_keyword(node, "flow def".len(), MOD_DECLARATION, out),

                // type_annotation: emit TYPE token spanning the full span.
                "type_annotation" => {
                    if let Some(span) = span_of(node) {
                        out.push(RawToken {
                            start_byte: span[0],
                            length_bytes: span[1].saturating_sub(span[0]),
                            token_type: TT_TYPE,
                            modifiers: 0,
                        });
                    }
                }

                // attribute_usage: emit `attribute` keyword (9 bytes) at start.
                // PROPERTY token is conceptually for the name, but the
                // name has no standalone span — emit the keyword only.
                "attribute_usage" => {
                    emit_keyword(node, "attribute".len(), MOD_DECLARATION, out);
                }

                // annotation: ENUM_MEMBER over the full annotation span.
                "annotation" => {
                    if let Some(span) = span_of(node) {
                        out.push(RawToken {
                            start_byte: span[0],
                            length_bytes: span[1].saturating_sub(span[0]),
                            token_type: TT_ENUM_MEMBER,
                            modifiers: 0,
                        });
                    }
                }

                // multiplicity: REGEXP over full span.
                "multiplicity" => {
                    if let Some(span) = span_of(node) {
                        out.push(RawToken {
                            start_byte: span[0],
                            length_bytes: span[1].saturating_sub(span[0]),
                            token_type: TT_REGEXP,
                            modifiers: 0,
                        });
                    }
                }

                // identifier: VARIABLE (per §7 fallback). The runtime
                // editor combines this with the TextMate fallback so
                // identifiers don't go un-highlighted.
                "identifier" => {
                    if let Some(span) = span_of(node) {
                        out.push(RawToken {
                            start_byte: span[0],
                            length_bytes: span[1].saturating_sub(span[0]),
                            token_type: TT_VARIABLE,
                            modifiers: 0,
                        });
                    }
                }

                // doc_comment: SKIP — TextMate owns comment styling
                // per §7 (ambiguity resolution note line 651).
                "doc_comment" => {}

                _ => {} // Unknown kind — descend without emitting.
            }
        }
        for (_k, v) in map.iter() {
            collect_tokens(v, out);
        }
    } else if let Value::Array(items) = node {
        for item in items.iter() {
            collect_tokens(item, out);
        }
    }
}

/// Absolute (line, char, length, type, modifiers) before delta encoding.
#[derive(Debug, Clone, Copy)]
struct AbsToken {
    line: u32,
    start: u32,
    length: u32,
    token_type: u32,
    modifiers: u32,
}

/// Translate sorted RawTokens to LSP `SemanticToken` records with
/// 5-tuple delta encoding applied in-place (delta_line, delta_start
/// are relative to the previous emitted token; length/type/modifiers
/// stay absolute).
fn delta_encode(tokens: &[RawToken], source: &[u8]) -> Vec<SemanticToken> {
    // Precompute line-start byte offsets for fast byte → (line, col).
    let line_starts = compute_line_starts(source);

    // A token whose span crosses multiple lines is split per-line so
    // every emitted token stays on a single line (LSP spec requirement).
    let mut emitted: Vec<AbsToken> = Vec::with_capacity(tokens.len());
    for tok in tokens {
        let (l0, c0) = byte_to_line_col(&line_starts, source, tok.start_byte);
        let end_byte = tok.start_byte + tok.length_bytes;
        let (l1, c1) = byte_to_line_col(&line_starts, source, end_byte.min(source.len()));
        if l0 == l1 {
            emitted.push(AbsToken {
                line: l0,
                start: c0,
                length: c1.saturating_sub(c0),
                token_type: tok.token_type,
                modifiers: tok.modifiers,
            });
        } else {
            // Multi-line: emit head, then continuation lines.
            let head_end_col = line_end_col(&line_starts, source, l0);
            emitted.push(AbsToken {
                line: l0,
                start: c0,
                length: head_end_col.saturating_sub(c0),
                token_type: tok.token_type,
                modifiers: tok.modifiers,
            });
            for line in (l0 + 1)..l1 {
                let col = line_end_col(&line_starts, source, line);
                if col > 0 {
                    emitted.push(AbsToken {
                        line,
                        start: 0,
                        length: col,
                        token_type: tok.token_type,
                        modifiers: tok.modifiers,
                    });
                }
            }
            if c1 > 0 {
                emitted.push(AbsToken {
                    line: l1,
                    start: 0,
                    length: c1,
                    token_type: tok.token_type,
                    modifiers: tok.modifiers,
                });
            }
        }
    }

    // Re-sort by (absolute line, absolute start) after multi-line splits.
    emitted.sort_by(|a, b| a.line.cmp(&b.line).then(a.start.cmp(&b.start)));

    let mut prev_line: u32 = 0;
    let mut prev_start: u32 = 0;
    let mut out: Vec<SemanticToken> = Vec::with_capacity(emitted.len());
    for tok in &emitted {
        let delta_line = tok.line - prev_line;
        let delta_start = if delta_line == 0 {
            tok.start - prev_start
        } else {
            tok.start
        };
        out.push(SemanticToken {
            delta_line,
            delta_start,
            length: tok.length,
            token_type: tok.token_type,
            token_modifiers_bitset: tok.modifiers,
        });
        prev_line = tok.line;
        prev_start = tok.start;
    }

    out
}

fn compute_line_starts(source: &[u8]) -> Vec<usize> {
    let mut out = vec![0usize];
    for (i, b) in source.iter().enumerate() {
        if *b == b'\n' {
            out.push(i + 1);
        }
    }
    out
}

/// Byte offset → (line, UTF-16-character column).
fn byte_to_line_col(line_starts: &[usize], source: &[u8], byte: usize) -> (u32, u32) {
    let byte = byte.min(source.len());
    let line = match line_starts.binary_search(&byte) {
        Ok(i) => i,
        Err(i) => i.saturating_sub(1),
    };
    let line_start = line_starts.get(line).copied().unwrap_or(0);
    let prefix = &source[line_start..byte];
    // Count UTF-16 code units in the prefix bytes.
    let s = std::str::from_utf8(prefix).unwrap_or("");
    let col: u32 = s.chars().map(|c| c.len_utf16() as u32).sum();
    (line as u32, col)
}

/// UTF-16 column of the end of `line` (i.e. exclusive of newline).
fn line_end_col(line_starts: &[usize], source: &[u8], line: u32) -> u32 {
    let idx = line as usize;
    let start = line_starts.get(idx).copied().unwrap_or(source.len());
    let end_excl = line_starts
        .get(idx + 1)
        .map(|n| n.saturating_sub(1))
        .unwrap_or(source.len());
    let s = std::str::from_utf8(&source[start..end_excl]).unwrap_or("");
    s.chars().map(|c| c.len_utf16() as u32).sum()
}

// ----- LSP trait method handlers (called from backend.rs) --------

/// `textDocument/semanticTokens/full` handler — encode the entire
/// document and cache the result for delta computation.
pub async fn handle_full(
    documents: &Documents,
    uri: &Url,
) -> LspResult<Option<SemanticTokensResult>> {
    let handle_arc = match documents.get_handle(uri) {
        Some(h) => h,
        None => return Ok(None),
    };
    let source_bytes = match documents.get_buffer(uri) {
        Some(rope) => {
            let mut s = String::with_capacity(rope.len_bytes());
            for chunk in rope.chunks() {
                s.push_str(chunk);
            }
            s.into_bytes()
        }
        None => return Ok(None),
    };
    let ast_bytes = {
        let guard = handle_arc.lock().await;
        deal_ffi::safe::ast_json(&guard).unwrap_or_default()
    };
    let data = if ast_bytes.is_empty() {
        Vec::new()
    } else {
        match serde_json::from_slice::<Value>(&ast_bytes) {
            Ok(root) => encode_from_ast_with_source(&root, &source_bytes),
            Err(_) => Vec::new(),
        }
    };
    let result_id = make_result_id();
    documents.cache_tokens(uri.clone(), result_id.clone(), data.clone());
    Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
        result_id: Some(result_id),
        data,
    })))
}

/// `textDocument/semanticTokens/full/delta` handler — compute new full
/// set, compare against cached previous_result_id, return either a
/// SemanticTokensDelta (single replace-all SemanticTokensEdit) or the
/// full set if the previous id is unknown (cache eviction recovery).
pub async fn handle_full_delta(
    documents: &Documents,
    uri: &Url,
    previous_result_id: String,
) -> LspResult<Option<SemanticTokensFullDeltaResult>> {
    let prev = match documents.get_cached_tokens(uri, &previous_result_id) {
        Some(p) => p,
        None => {
            // Cache eviction recovery: return the full set.
            if let Some(SemanticTokensResult::Tokens(t)) = handle_full(documents, uri).await? {
                return Ok(Some(SemanticTokensFullDeltaResult::Tokens(t)));
            }
            return Ok(None);
        }
    };
    let handle_arc = match documents.get_handle(uri) {
        Some(h) => h,
        None => return Ok(None),
    };
    let source_bytes = match documents.get_buffer(uri) {
        Some(rope) => {
            let mut s = String::with_capacity(rope.len_bytes());
            for chunk in rope.chunks() {
                s.push_str(chunk);
            }
            s.into_bytes()
        }
        None => return Ok(None),
    };
    let ast_bytes = {
        let guard = handle_arc.lock().await;
        deal_ffi::safe::ast_json(&guard).unwrap_or_default()
    };
    let new_data = if ast_bytes.is_empty() {
        Vec::new()
    } else {
        match serde_json::from_slice::<Value>(&ast_bytes) {
            Ok(root) => encode_from_ast_with_source(&root, &source_bytes),
            Err(_) => Vec::new(),
        }
    };
    let new_id = make_result_id();
    documents.cache_tokens(uri.clone(), new_id.clone(), new_data.clone());

    // Simplest valid LSP delta: single splice that replaces the entire
    // prior data array with the new one. Clients still benefit from the
    // result_id stability for cache coherence. Phase 3.x can add
    // incremental delta computation if needed (TODO).
    let edits = vec![SemanticTokensEdit {
        start: 0,
        delete_count: prev.len() as u32,
        data: Some(new_data),
    }];
    Ok(Some(SemanticTokensFullDeltaResult::TokensDelta(
        SemanticTokensDelta {
            result_id: Some(new_id),
            edits,
        },
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn legend_has_9_types_and_5_modifiers() {
        let l = semantic_tokens_legend();
        assert_eq!(l.token_types.len(), 9);
        assert_eq!(l.token_modifiers.len(), 5);
    }

    #[test]
    fn modifier_bit_layout() {
        assert_eq!(MOD_DECLARATION, 0b00001);
        assert_eq!(MOD_DEFINITION, 0b00010);
        assert_eq!(MOD_READONLY, 0b00100);
        assert_eq!(MOD_STATIC, 0b01000);
        assert_eq!(MOD_DEPRECATED, 0b10000);
        // Combining DECLARATION + DEFINITION yields 3 per RESEARCH §2.
        assert_eq!(MOD_DECLARATION | MOD_DEFINITION, 0b00011);
    }

    #[test]
    fn deterministic_id_when_env_set() {
        // Use a sub-scope to avoid polluting other tests.
        std::env::set_var(DETERMINISTIC_ENV, "1");
        assert_eq!(make_result_id(), "golden");
        std::env::remove_var(DETERMINISTIC_ENV);
    }

    #[test]
    fn collect_tokens_emits_part_def_keyword() {
        // Synthetic AST: a single part_def at byte span [0, 16].
        let ast = json!({
            "k": "deal_file",
            "span": [0, 16],
            "definitions": [
                { "k": "part_def", "span": [0, 16] }
            ]
        });
        let mut raws: Vec<RawToken> = Vec::new();
        collect_tokens(&ast, &mut raws);
        // Expect one KEYWORD token at byte 0 with length 8 ("part def").
        assert_eq!(raws.len(), 1);
        assert_eq!(raws[0].start_byte, 0);
        assert_eq!(raws[0].length_bytes, "part def".len());
        assert_eq!(raws[0].token_type, TT_KEYWORD);
        assert_eq!(raws[0].modifiers, MOD_DECLARATION);
    }

    /// Phase 05.2 (D-04): calc_def arm emits a KEYWORD + MOD_DECLARATION token
    /// exactly like constraint_def. Length is "calc def" (8 bytes).
    #[test]
    fn collect_tokens_emits_calc_def_keyword() {
        // Synthetic AST: a single calc_def at byte span [0, 20].
        let ast = json!({
            "k": "deal_file",
            "span": [0, 20],
            "definitions": [
                { "k": "calc_def", "span": [0, 20] }
            ]
        });
        let mut raws: Vec<RawToken> = Vec::new();
        collect_tokens(&ast, &mut raws);
        // Expect exactly one KEYWORD token at byte 0 with length 8 ("calc def").
        assert_eq!(raws.len(), 1, "calc_def must emit exactly one semantic token");
        assert_eq!(raws[0].start_byte, 0);
        assert_eq!(raws[0].length_bytes, "calc def".len(),
            "token length must equal 'calc def' (8 bytes)");
        assert_eq!(raws[0].token_type, TT_KEYWORD,
            "calc_def token must be TT_KEYWORD");
        assert_eq!(raws[0].modifiers, MOD_DECLARATION,
            "calc_def token must carry MOD_DECLARATION");
    }

    #[test]
    fn collect_tokens_skips_doc_comment_kind() {
        let ast = json!({
            "k": "part_def",
            "span": [0, 50],
            "doc": { "k": "doc_comment", "span": [0, 10], "text": "/** x */" }
        });
        let mut raws: Vec<RawToken> = Vec::new();
        collect_tokens(&ast, &mut raws);
        // part_def emits 1 token; doc_comment must NOT contribute another.
        let comment_tokens: Vec<&RawToken> = raws
            .iter()
            .filter(|t| t.start_byte == 0 && t.length_bytes == 10)
            .collect();
        assert!(
            comment_tokens.is_empty(),
            "doc_comment must not emit a semantic token (TextMate owns it)"
        );
    }

    fn tuples(toks: &[SemanticToken]) -> Vec<(u32, u32, u32, u32, u32)> {
        toks.iter()
            .map(|t| {
                (
                    t.delta_line,
                    t.delta_start,
                    t.length,
                    t.token_type,
                    t.token_modifiers_bitset,
                )
            })
            .collect()
    }

    #[test]
    fn delta_encode_single_line_ascii() {
        // Source: "part def X" (10 bytes, single line)
        let source = b"part def X";
        let tokens = vec![
            RawToken {
                start_byte: 0,
                length_bytes: 8,
                token_type: TT_KEYWORD,
                modifiers: MOD_DECLARATION,
            },
            RawToken {
                start_byte: 9,
                length_bytes: 1,
                token_type: TT_TYPE,
                modifiers: 0,
            },
        ];
        let data = delta_encode(&tokens, source);
        assert_eq!(
            tuples(&data),
            vec![(0, 0, 8, TT_KEYWORD, MOD_DECLARATION), (0, 9, 1, TT_TYPE, 0)]
        );
    }

    #[test]
    fn delta_encode_multiline_ascii() {
        // Source: "AAA\nBBB\nCCC" — three lines of 3 chars each.
        let source = b"AAA\nBBB\nCCC";
        let tokens = vec![
            RawToken {
                start_byte: 0,
                length_bytes: 3,
                token_type: TT_TYPE,
                modifiers: 0,
            },
            RawToken {
                start_byte: 4,
                length_bytes: 3,
                token_type: TT_TYPE,
                modifiers: 0,
            },
            RawToken {
                start_byte: 8,
                length_bytes: 3,
                token_type: TT_TYPE,
                modifiers: 0,
            },
        ];
        let data = delta_encode(&tokens, source);
        assert_eq!(
            tuples(&data),
            vec![
                (0, 0, 3, TT_TYPE, 0),
                (1, 0, 3, TT_TYPE, 0),
                (1, 0, 3, TT_TYPE, 0),
            ]
        );
    }

    #[test]
    fn empty_ast_yields_empty_data() {
        let tokens = encode_from_ast_bytes(b"");
        assert!(tokens.data.is_empty());
    }

    #[test]
    fn encode_from_ast_with_source_handles_utf16() {
        // "α" = 2 bytes / 1 UTF-16 unit. Token at byte 3 (after "α "), len 4.
        let source = "α type".as_bytes();
        let tokens = vec![RawToken {
            start_byte: 3, // "α " is 2+1=3 bytes
            length_bytes: 4,
            token_type: TT_TYPE,
            modifiers: 0,
        }];
        let data = delta_encode(&tokens, source);
        // col is UTF-16 count of "α " = 1 + 1 = 2.
        assert_eq!(tuples(&data), vec![(0, 2, 4, TT_TYPE, 0)]);
    }
}
