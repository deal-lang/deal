//! deal-lsp — Language Server Protocol implementation for the DEAL language.
//!
//! This crate exposes the LSP backend as a library so the binary `deal-lsp`
//! (src/main.rs) and integration tests under `tests/showcase.rs` can both
//! consume the same `Backend` / `Documents` / `Debouncer` types.

pub mod backend;
pub mod code_action;
pub mod code_lens;
pub mod completion;
pub mod debounce;
pub mod definition;
pub mod diagnostics;
pub mod doc_links;
pub mod documents;
pub mod folding;
pub mod formatting;
pub mod hover;
pub mod index;
pub mod inlay;
pub mod references;
pub mod rename;
pub mod semantic_tokens;
pub mod signature;
pub mod symbols;
pub mod sysml_mapping;
pub mod workspace;

pub use backend::{Backend, DEBOUNCE_MS};
pub use debounce::Debouncer;
pub use diagnostics::parse_diagnostics;
pub use documents::Documents;
pub use index::Index;
pub use workspace::Workspace;
