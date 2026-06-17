//! `textDocument/documentLink` — `import` items navigate to their declaring
//! file (P3 WS-C).
//!
//! goto-definition cannot reach `import P.{N}` items: the items are payload
//! strings on the `import_decl` node, not AST nodes a cursor lands on. The P2
//! binding capture, however, records each import item as a resolved reference
//! (`ref_kind == "import"`), which the index keeps in `import_links`. This
//! provider turns each into a `DocumentLink` over the item's range, targeting
//! the declaration's location — the only navigation affordance for imports.

use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{DocumentLink, DocumentLinkParams};

use crate::index::Index;

/// `textDocument/documentLink` handler.
pub async fn handle_document_link(
    index: &Index,
    params: DocumentLinkParams,
) -> LspResult<Option<Vec<DocumentLink>>> {
    let uri = &params.text_document.uri;
    let links = index
        .import_links_in_file(uri)
        .into_iter()
        .map(|(range, target)| DocumentLink {
            range,
            // Deep-link to the declaration's exact line via a fragment, so the
            // editor opens the file scrolled to the symbol rather than the top.
            target: Some(with_line_fragment(&target)),
            tooltip: Some("Go to declaration".to_string()),
            data: None,
        })
        .collect();
    Ok(Some(links))
}

/// Append an `Ln,Col` fragment to the target URI so the client opens the file
/// at the declaration (VS Code honours `#L<line>,<col>` on file URIs).
fn with_line_fragment(loc: &tower_lsp::lsp_types::Location) -> tower_lsp::lsp_types::Url {
    let mut url = loc.uri.clone();
    // LSP positions are 0-based; the editor fragment is 1-based.
    let line = loc.range.start.line + 1;
    let col = loc.range.start.character + 1;
    url.set_fragment(Some(&format!("L{line},{col}")));
    url
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp::lsp_types::{Location, Position, Range, Url};

    #[test]
    fn line_fragment_is_one_based() {
        let loc = Location {
            uri: Url::parse("file:///ws/interfaces/thermal.deal").unwrap(),
            range: Range {
                start: Position::new(4, 10),
                end: Position::new(4, 26),
            },
        };
        let linked = with_line_fragment(&loc);
        assert_eq!(linked.fragment(), Some("L5,11"));
        assert_eq!(linked.path(), "/ws/interfaces/thermal.deal");
    }
}
