//! `textDocument/inlayHint` — inline SysML metaclass badges (P3 WS-C).
//!
//! For every declaration in the requested range, render the SysML v2 / KerML
//! metaclass it normalizes to as a guillemet badge (`«PartDefinition»`)
//! immediately after the name. This surfaces the P1 mapping answer — "what does
//! this become in SysML v2" — inline, not just on hover. Hints are
//! client-toggleable (`editor.inlayHints.enabled`), so they add no forced noise.

use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    InlayHint, InlayHintKind, InlayHintLabel, InlayHintParams, InlayHintTooltip, Position, Range,
};

use crate::index::Index;
use crate::sysml_mapping;

/// `textDocument/inlayHint` handler. Range-scoped: only declarations whose name
/// falls in the requested window get a hint (large-file performance).
pub async fn handle_inlay_hint(
    index: &Index,
    params: InlayHintParams,
) -> LspResult<Option<Vec<InlayHint>>> {
    let uri = &params.text_document.uri;
    let range = params.range;
    let mut hints = Vec::new();
    for el in index.elements_in_file(uri) {
        if !position_in_range(&range, &el.name_range.start) {
            continue;
        }
        if let Some(hint) = metaclass_hint(el.name_range.end, &el.kind) {
            hints.push(hint);
        }
    }
    Ok(Some(hints))
}

/// Build the `«Metaclass»` badge hint for a declaration kind, or `None` when the
/// kind has no SysML mapping (the declaration then gets no badge).
fn metaclass_hint(name_end: Position, kind: &str) -> Option<InlayHint> {
    let m = sysml_mapping::resolve(kind)?;
    Some(InlayHint {
        position: name_end,
        label: InlayHintLabel::String(format!("«{}»", m.metaclass)),
        kind: Some(InlayHintKind::TYPE),
        text_edits: None,
        tooltip: Some(InlayHintTooltip::String(format!(
            "SysML v2 · {} ({})",
            m.metaclass, m.clause
        ))),
        padding_left: Some(true),
        padding_right: Some(false),
        data: None,
    })
}

/// Inclusive containment of `pos` within `range` by (line, character) order.
fn position_in_range(range: &Range, pos: &Position) -> bool {
    let p = (pos.line, pos.character);
    p >= (range.start.line, range.start.character) && p <= (range.end.line, range.end.character)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metaclass_hint_renders_guillemet_badge() {
        let h = metaclass_hint(Position::new(2, 18), "part_def").expect("part_def maps");
        match h.label {
            InlayHintLabel::String(s) => assert_eq!(s, "«PartDefinition»"),
            _ => panic!("expected a string label"),
        }
        assert_eq!(h.kind, Some(InlayHintKind::TYPE));
        assert_eq!(h.position, Position::new(2, 18));
        assert_eq!(h.padding_left, Some(true));
    }

    #[test]
    fn unmapped_kind_yields_no_hint() {
        assert!(metaclass_hint(Position::new(0, 0), "definitely_not_a_kind").is_none());
    }

    #[test]
    fn position_in_range_is_inclusive() {
        let r = Range {
            start: Position::new(1, 0),
            end: Position::new(5, 0),
        };
        assert!(position_in_range(&r, &Position::new(1, 0)));
        assert!(position_in_range(&r, &Position::new(3, 4)));
        assert!(position_in_range(&r, &Position::new(5, 0)));
        assert!(!position_in_range(&r, &Position::new(0, 9)));
        assert!(!position_in_range(&r, &Position::new(6, 0)));
    }
}
