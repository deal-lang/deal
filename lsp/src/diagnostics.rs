//! Diagnostic envelope ↔ LSP Diagnostic translation.
//!
//! Consumes the Phase 2 D-32 envelope produced by `deal_diagnostics_json`
//! and emits `tower_lsp::lsp_types::Diagnostic` values for client push via
//! `textDocument/publishDiagnostics` (RESEARCH §11 push-mode).
//!
//! ## Envelope shape (D-32 — Phase 2 wire format, as verified live)
//!
//! The Zig core emits a TOP-LEVEL JSON ARRAY of records:
//!
//! ```json
//! [
//!   { "code": "E2100",
//!     "fix_it": null,
//!     "message": "type `T` is not defined; ...",
//!     "notes": "",
//!     "secondary_spans": [],
//!     "severity": "err",
//!     "span": [38, 51] }
//! ]
//! ```
//!
//! `span` is a `[start_byte, end_byte]` pair — half-open, UTF-8 byte
//! offsets into the source. LSP needs `Range { start: Position, end:
//! Position }` where Position is `(line, character)`. We translate using
//! the document's `ropey::Rope` so the conversion handles multi-byte UTF-8
//! correctly.
//!
//! ## Phase 2 vs LSP severity vocabulary
//!
//! | Phase 2 (Zig)  | LSP                              |
//! | -------------- | -------------------------------- |
//! | `err`          | `DiagnosticSeverity::ERROR`      |
//! | `warn`         | `DiagnosticSeverity::WARNING`    |
//! | `info`         | `DiagnosticSeverity::INFORMATION`|
//! | `hint`         | `DiagnosticSeverity::HINT`       |
//!
//! Unknown severity strings default to ERROR (fail-loud).

use serde::Deserialize;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range};

#[derive(Debug, Deserialize)]
struct Record {
    code: String,
    severity: String,
    message: String,
    /// `[start_byte, end_byte]` half-open UTF-8 byte span into the source.
    span: [usize; 2],
}

/// Parse the diagnostic envelope and convert each record into an LSP
/// `Diagnostic`. The `rope` is the document's current Rope (needed to
/// translate byte spans into `(line, character)` positions).
///
/// Returns an empty vec on empty bytes, empty array, or deserialization
/// failure — the LSP must keep running even if a single document's
/// envelope is mangled.
pub fn parse_diagnostics(json_bytes: &[u8], rope: &ropey::Rope) -> Vec<Diagnostic> {
    if json_bytes.is_empty() {
        return Vec::new();
    }
    let records: Vec<Record> = match serde_json::from_slice::<Vec<Record>>(json_bytes) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("diagnostic envelope parse failed: {e}");
            return Vec::new();
        }
    };
    records
        .into_iter()
        .map(|r| record_to_diagnostic(r, rope))
        .collect()
}

fn record_to_diagnostic(r: Record, rope: &ropey::Rope) -> Diagnostic {
    let start = byte_span_to_position(rope, r.span[0]);
    let end = byte_span_to_position(rope, r.span[1]);
    Diagnostic {
        range: Range { start, end },
        severity: Some(severity_from_str(&r.severity)),
        code: Some(NumberOrString::String(r.code)),
        code_description: None,
        source: Some("deal-lsp".to_string()),
        message: r.message,
        related_information: None,
        tags: None,
        data: None,
    }
}

/// Translate a UTF-8 byte offset into a `(line, character)` Position via
/// the document's Rope. `character` is in UTF-16 code units per LSP spec
/// (DEAL source uses mostly ASCII so this is usually equal to UTF-8
/// chars, but ropey handles the multi-byte case correctly).
///
/// Out-of-range byte offsets clamp to the end of the document — the LSP
/// must never panic on a malformed envelope.
fn byte_span_to_position(rope: &ropey::Rope, byte: usize) -> Position {
    let max_bytes = rope.len_bytes();
    let byte = byte.min(max_bytes);
    let char_idx = rope.byte_to_char(byte);
    let line = rope.char_to_line(char_idx);
    let line_start_char = rope.line_to_char(line);
    // LSP positions are UTF-16 code units. For ASCII content this matches
    // char count; for multi-byte UTF-8 we need to count UTF-16 units of
    // the prefix on this line.
    let line_prefix = rope.slice(line_start_char..char_idx);
    let character = line_prefix.chars().map(|c| c.len_utf16() as u32).sum();
    Position {
        line: line as u32,
        character,
    }
}

fn severity_from_str(s: &str) -> DiagnosticSeverity {
    match s {
        "err" | "error" => DiagnosticSeverity::ERROR,
        "warn" | "warning" => DiagnosticSeverity::WARNING,
        "info" | "information" => DiagnosticSeverity::INFORMATION,
        "hint" => DiagnosticSeverity::HINT,
        _ => DiagnosticSeverity::ERROR,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::Rope;

    fn empty_rope() -> Rope {
        Rope::new()
    }

    #[test]
    fn empty_array_yields_no_diagnostics() {
        let out = parse_diagnostics(b"[]", &empty_rope());
        assert!(out.is_empty());
    }

    #[test]
    fn empty_bytes_yields_no_diagnostics() {
        let out = parse_diagnostics(b"", &empty_rope());
        assert!(out.is_empty());
    }

    #[test]
    fn malformed_json_yields_no_diagnostics() {
        let out = parse_diagnostics(b"{ not json", &empty_rope());
        assert!(out.is_empty());
    }

    #[test]
    fn phase2_array_with_one_record() {
        // Synthetic source: 3 chars on line 0, newline, then "abc" on line 1.
        let rope = Rope::from_str("xyz\nabc");
        // span [4, 7] is "abc" on line 1, characters 0..3.
        let json = br#"[{"code":"E2100","severity":"err","message":"oops","span":[4,7]}]"#;
        let out = parse_diagnostics(json, &rope);
        assert_eq!(out.len(), 1);
        let d = &out[0];
        assert_eq!(d.code, Some(NumberOrString::String("E2100".to_string())));
        assert_eq!(d.severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(d.message, "oops");
        assert_eq!(d.range.start, Position::new(1, 0));
        assert_eq!(d.range.end, Position::new(1, 3));
        assert_eq!(d.source.as_deref(), Some("deal-lsp"));
    }

    #[test]
    fn severity_mapping_for_phase2_vocab() {
        let rope = Rope::from_str("abc");
        let json = br#"[
            {"code":"E0001","severity":"err","message":"e","span":[0,1]},
            {"code":"E0002","severity":"warn","message":"w","span":[0,1]},
            {"code":"E0003","severity":"info","message":"i","span":[0,1]},
            {"code":"E0004","severity":"hint","message":"h","span":[0,1]}
        ]"#;
        let out = parse_diagnostics(json, &rope);
        assert_eq!(out.len(), 4);
        assert_eq!(out[0].severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(out[1].severity, Some(DiagnosticSeverity::WARNING));
        assert_eq!(out[2].severity, Some(DiagnosticSeverity::INFORMATION));
        assert_eq!(out[3].severity, Some(DiagnosticSeverity::HINT));
    }

    #[test]
    fn out_of_range_byte_clamps_to_end() {
        // Should not panic on a malformed span larger than the document.
        let rope = Rope::from_str("hi");
        let json = br#"[{"code":"E9999","severity":"err","message":"x","span":[100,200]}]"#;
        let out = parse_diagnostics(json, &rope);
        assert_eq!(out.len(), 1);
        // Both ends clamp to (0, 2) — end of single-line document.
        assert_eq!(out[0].range.start, Position::new(0, 2));
        assert_eq!(out[0].range.end, Position::new(0, 2));
    }

    #[test]
    fn utf16_character_count_for_multibyte() {
        // "α" is U+03B1 — 2 bytes UTF-8, 1 UTF-16 code unit.
        // "𝄞" is U+1D11E — 4 bytes UTF-8, 2 UTF-16 code units (surrogate pair).
        let rope = Rope::from_str("α𝄞x");
        // byte 6 is past α (2 bytes) + 𝄞 (4 bytes) = start of 'x'.
        let json = br#"[{"code":"X","severity":"err","message":"m","span":[6,7]}]"#;
        let out = parse_diagnostics(json, &rope);
        // character: α=1 + 𝄞=2 = 3 UTF-16 code units before 'x'.
        assert_eq!(out[0].range.start, Position::new(0, 3));
        assert_eq!(out[0].range.end, Position::new(0, 4));
    }
}
