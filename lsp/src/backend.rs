//! deal-lsp `LanguageServer` implementation (Plan 03-03 surface).
//!
//! Capabilities this plan ships:
//!   - text_document_sync = FULL (D-43 — client sends full buffer per change)
//!   - document_formatting (D-21 — in-memory format via live handle)
//!   - push-mode diagnostics via publishDiagnostics (RESEARCH §11)
//!
//! Capabilities Plan 04 layers on:
//!   - completion, hover, goto-definition, semantic-tokens, workspace-symbol
//!
//! Plan 04 ALSO adds eager workspace parse from `initialized`. For now we
//! just log readiness.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use crate::code_action;
use crate::code_lens;
use crate::completion;
use crate::debounce::Debouncer;
use crate::definition;
use crate::doc_links;
use crate::documents::Documents;
use crate::folding;
use crate::formatting;
use crate::hover;
use crate::index::Index;
use crate::inlay;
use crate::references;
use crate::rename;
use crate::semantic_tokens;
use crate::signature;
use crate::symbols;
use crate::workspace::{self, Workspace};

/// D-43 debounce window for did_change → re-parse, in milliseconds.
pub const DEBOUNCE_MS: u64 = 300;

/// LSP backend state. `Backend` is the concrete struct that implements
/// `LanguageServer`; tower-lsp owns a single `Arc<Backend>` for the
/// lifetime of the server.
pub struct Backend {
    pub client: Client,
    pub documents: Arc<Documents>,
    pub debouncer: Arc<Debouncer>,
    pub index: Arc<Index>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(Documents::new()),
            debouncer: Arc::new(Debouncer::new()),
            index: Arc::new(Index::new()),
        }
    }
}

#[async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> LspResult<InitializeResult> {
        // Stash the first workspace folder for `initialized` to consume.
        // tower-lsp does not pass InitializeParams to `initialized`, so we
        // capture the relevant data here and hand it off via an internal
        // slot on Documents.
        if let Some(folders) = params.workspace_folders.as_ref() {
            if let Some(first) = folders.first() {
                if let Ok(path) = first.uri.to_file_path() {
                    self.documents.set_pending_workspace_root(path);
                }
            }
        } else if let Some(root) = params.root_uri.as_ref().and_then(|u| u.to_file_path().ok()) {
            self.documents.set_pending_workspace_root(root);
        }

        // ADR-0004 P5 WS-B: a configured stdlib path (initializationOptions
        // `stdlibPath`) lets the editor point the server at the stdlib so
        // hover/goto into `deal.std.units` resolves without `deal install`.
        if let Some(p) = params
            .initialization_options
            .as_ref()
            .and_then(|o| o.get("stdlibPath"))
            .and_then(|v| v.as_str())
        {
            self.documents
                .set_pending_stdlib_path(std::path::PathBuf::from(p));
        }

        let capabilities = ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
            document_formatting_provider: Some(OneOf::Left(true)),
            workspace_symbol_provider: Some(OneOf::Left(true)),
            document_symbol_provider: Some(OneOf::Left(true)),
            folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
            inlay_hint_provider: Some(OneOf::Left(true)),
            code_lens_provider: Some(CodeLensOptions {
                resolve_provider: Some(false),
            }),
            document_link_provider: Some(DocumentLinkOptions {
                resolve_provider: Some(false),
                work_done_progress_options: Default::default(),
            }),
            code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
            references_provider: Some(OneOf::Left(true)),
            document_highlight_provider: Some(OneOf::Left(true)),
            rename_provider: Some(OneOf::Right(RenameOptions {
                prepare_provider: Some(true),
                work_done_progress_options: Default::default(),
            })),
            completion_provider: Some(CompletionOptions {
                trigger_characters: Some(vec![".".to_string(), ":".to_string(), "<".to_string()]),
                ..Default::default()
            }),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            signature_help_provider: Some(SignatureHelpOptions {
                trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
                retrigger_characters: Some(vec![",".to_string()]),
                work_done_progress_options: Default::default(),
            }),
            definition_provider: Some(OneOf::Left(true)),
            semantic_tokens_provider: Some(
                SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
                    legend: semantic_tokens::semantic_tokens_legend(),
                    full: Some(SemanticTokensFullOptions::Delta { delta: Some(true) }),
                    range: Some(false),
                    work_done_progress_options: Default::default(),
                }),
            ),
            ..Default::default()
        };

        Ok(InitializeResult {
            capabilities,
            server_info: Some(ServerInfo {
                name: "deal-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        tracing::info!("deal-lsp ready (Plan 03-04 — eager workspace parse starting)");
        // D-47: eager parse runs in a background task so `initialized` returns
        // immediately. Per-file failures are logged inside `eager_parse`.
        let root = match self.documents.take_pending_workspace_root() {
            Some(r) => r,
            None => {
                tracing::info!("initialized: no workspace folder declared — skipping eager parse");
                return;
            }
        };
        let documents = self.documents.clone();
        let index = self.index.clone();
        let stdlib_path = self.documents.take_pending_stdlib_path();
        tokio::spawn(async move {
            let ws = match Workspace::discover(&root) {
                Ok(w) => w,
                Err(e) => {
                    tracing::warn!("workspace discover({}) failed: {e}", root.display());
                    return;
                }
            };
            // Replace the empty alias table the index booted with.
            // Index::with_aliases would mean a new instance — use a setter
            // path to keep the shared Arc<Index> stable.
            index.replace_aliases(ws.aliases.clone());
            workspace::eager_parse(Arc::new(ws), documents, index, stdlib_path).await;
        });
    }

    async fn shutdown(&self) -> LspResult<()> {
        tracing::info!("deal-lsp shutdown");
        Ok(())
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> LspResult<Option<Vec<SymbolInformation>>> {
        // The capability is advertised in `initialize` (workspace_symbol_provider);
        // this handler answers it from the in-memory Index. Without it, a client
        // sending workspace/symbol got method-not-found despite the advertised
        // capability.
        Ok(Some(self.index.workspace_symbols(&params.query)))
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        let version = Some(params.text_document.version);

        if let Err(e) = self
            .documents
            .open(uri.clone(), text, version, &self.client)
            .await
        {
            tracing::error!("did_open({uri}) failed: {e}");
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // text_document_sync = FULL: there is exactly one content change
        // and it contains the entire buffer.
        let uri = params.text_document.uri;
        let version = Some(params.text_document.version);
        let Some(change) = params.content_changes.into_iter().next() else {
            return;
        };
        let text = change.text;

        // D-43: schedule via the debouncer; the previous pending parse for
        // this URI (if any) is cancelled.
        let docs = self.documents.clone();
        let client = self.client.clone();
        let index = self.index.clone();
        let uri_for_action = uri.clone();
        self.debouncer
            .schedule(uri, Duration::from_millis(DEBOUNCE_MS), async move {
                if let Err(e) = docs
                    .update(uri_for_action.clone(), text, version, &client, Some(&index))
                    .await
                {
                    tracing::error!("did_change update({uri_for_action}) failed: {e}");
                }
            });
    }

    async fn did_close(&self, _params: DidCloseTextDocumentParams) {
        // D-44 / RESEARCH Open Q2: NO-OP. Handles persist for workspace
        // lifetime, not editor lifetime, so cross-file features (Plan 04
        // workspace_symbol, goto-definition into a closed buffer) keep
        // working.
    }

    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> LspResult<Option<Vec<TextEdit>>> {
        formatting::handle_formatting(&self.documents, &params.text_document.uri).await
    }

    async fn completion(&self, params: CompletionParams) -> LspResult<Option<CompletionResponse>> {
        completion::handle_completion(&self.documents, &self.index, params).await
    }

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        hover::handle_hover(&self.documents, &self.index, params).await
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        definition::handle_definition(&self.documents, &self.index, params).await
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> LspResult<Option<DocumentSymbolResponse>> {
        symbols::handle_document_symbol(&self.documents, params).await
    }

    async fn folding_range(
        &self,
        params: FoldingRangeParams,
    ) -> LspResult<Option<Vec<FoldingRange>>> {
        folding::handle_folding_range(&self.documents, params).await
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> LspResult<Option<Vec<InlayHint>>> {
        inlay::handle_inlay_hint(&self.index, params).await
    }

    async fn code_lens(&self, params: CodeLensParams) -> LspResult<Option<Vec<CodeLens>>> {
        code_lens::handle_code_lens(&self.index, params).await
    }

    async fn document_link(
        &self,
        params: DocumentLinkParams,
    ) -> LspResult<Option<Vec<DocumentLink>>> {
        doc_links::handle_document_link(&self.index, params).await
    }

    async fn signature_help(
        &self,
        params: SignatureHelpParams,
    ) -> LspResult<Option<SignatureHelp>> {
        signature::handle_signature_help(&self.documents, &self.index, params).await
    }

    async fn code_action(&self, params: CodeActionParams) -> LspResult<Option<CodeActionResponse>> {
        code_action::handle_code_action(&self.documents, &self.index, params).await
    }

    async fn references(&self, params: ReferenceParams) -> LspResult<Option<Vec<Location>>> {
        references::handle_references(&self.documents, &self.index, params).await
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> LspResult<Option<Vec<DocumentHighlight>>> {
        references::handle_document_highlight(&self.documents, &self.index, params).await
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> LspResult<Option<PrepareRenameResponse>> {
        rename::handle_prepare_rename(&self.documents, &self.index, params).await
    }

    async fn rename(&self, params: RenameParams) -> LspResult<Option<WorkspaceEdit>> {
        rename::handle_rename(&self.documents, &self.index, params).await
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> LspResult<Option<SemanticTokensResult>> {
        semantic_tokens::handle_full(&self.documents, &params.text_document.uri).await
    }

    async fn semantic_tokens_full_delta(
        &self,
        params: SemanticTokensDeltaParams,
    ) -> LspResult<Option<SemanticTokensFullDeltaResult>> {
        semantic_tokens::handle_full_delta(
            &self.documents,
            &params.text_document.uri,
            params.previous_result_id,
        )
        .await
    }
}
