//! `textDocument/references` handler (P2 WS-A / WS-B).
//!
//! Find-all-references built on the compiler-authoritative reverse usage index
//! (`Index::usages`, populated from the envelope's `references[]`). The symbol
//! under the cursor is resolved to its canonical fully-qualified path via the
//! shared `definition::resolved_path_at`, then every site the compiler bound to
//! that path is returned. Because the bindings come from `sema`'s own name
//! resolution, the result is exact — two same-named symbols in different
//! packages never bleed into each other.
//!
//! Returns `Ok(None)` when the cursor is not on a resolvable symbol or no sites
//! exist — the LSP MUST NOT panic.

use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{Location, ReferenceParams};

use crate::definition;
use crate::documents::Documents;
use crate::index::Index;

/// Implementation of `textDocument/references`.
pub async fn handle_references(
    documents: &Documents,
    index: &Index,
    params: ReferenceParams,
) -> LspResult<Option<Vec<Location>>> {
    let uri = &params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;
    let include_decl = params.context.include_declaration;

    let Some((path, _decl)) =
        definition::resolved_path_at(documents, index, uri, position).await
    else {
        return Ok(None);
    };

    let locations = index.references_of(&path, include_decl);
    if locations.is_empty() {
        Ok(None)
    } else {
        Ok(Some(locations))
    }
}
