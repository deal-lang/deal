//! Per-document handle table for deal-lsp.
//!
//! Implements D-42 (per-document DealHandle lifecycle) + D-44 (handle lives
//! for the workspace lifetime, not per-buffer-close) + D-45 (per-handle
//! tokio Mutex for Send/Sync safety).
//!
//! Maintains two parallel DashMaps keyed by `tower_lsp::lsp_types::Url`:
//!   - `handles` — `Arc<Mutex<OwnedDealHandle>>` per URI.
//!   - `buffers` — `ropey::Rope` per URI, needed by formatting.rs to compute
//!     full-document `TextEdit` ranges.
//!
//! A `parse_count` atomic counter (test-only hook) lets the debounce
//! integration test verify that rapid did_change events collapse into a
//! single re-parse.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use dashmap::DashMap;
use deal_ffi::OwnedDealHandle;
use ropey::Rope;
use std::sync::Mutex as StdMutex;
use tokio::sync::Mutex;
use tower_lsp::lsp_types::Url;
use tower_lsp::Client;

use crate::diagnostics;
use crate::index::Index;

/// Per-document handle + buffer table.
///
/// `Arc<Documents>` is cheap to clone into tokio tasks; cloning shares the
/// underlying DashMaps.
pub struct Documents {
    handles: DashMap<Url, Arc<Mutex<OwnedDealHandle>>>,
    buffers: DashMap<Url, Rope>,
    parse_count: AtomicUsize,
    /// One-shot hand-off slot: `initialize` (which sees InitializeParams)
    /// stashes the workspace root here so `initialized` (which does not
    /// receive InitializeParams in tower-lsp's trait signature) can pick
    /// it up and spawn the eager parse.
    pending_workspace_root: StdMutex<Option<PathBuf>>,
    /// Semantic-tokens delta cache (Plan 03-04 Task 3): keyed by URI,
    /// stores the most recently emitted (result_id, encoded data) so
    /// `semanticTokens/full/delta` requests can compute diffs (or fall
    /// back to a full response on cache eviction).
    tokens_cache: DashMap<Url, (String, Vec<tower_lsp::lsp_types::SemanticToken>)>,
}

impl Documents {
    pub fn new() -> Self {
        Self {
            handles: DashMap::new(),
            buffers: DashMap::new(),
            parse_count: AtomicUsize::new(0),
            pending_workspace_root: StdMutex::new(None),
            tokens_cache: DashMap::new(),
        }
    }

    /// Store the most recently emitted (result_id, data) pair for
    /// `semanticTokens/full/delta` cache. Overwrites the prior entry —
    /// LSP delta protocol only requires the most-recent id to be valid.
    pub fn cache_tokens(
        &self,
        uri: Url,
        result_id: String,
        data: Vec<tower_lsp::lsp_types::SemanticToken>,
    ) {
        self.tokens_cache.insert(uri, (result_id, data));
    }

    /// Retrieve the cached semantic tokens for `uri` IF the cached
    /// result_id matches `expected_result_id`. Returns `None` on
    /// cache miss or id mismatch (eviction recovery path).
    pub fn get_cached_tokens(
        &self,
        uri: &Url,
        expected_result_id: &str,
    ) -> Option<Vec<tower_lsp::lsp_types::SemanticToken>> {
        let entry = self.tokens_cache.get(uri)?;
        if entry.0 == expected_result_id {
            Some(entry.1.clone())
        } else {
            None
        }
    }

    /// Stash the workspace root for `initialized` to consume.
    pub fn set_pending_workspace_root(&self, root: PathBuf) {
        if let Ok(mut slot) = self.pending_workspace_root.lock() {
            *slot = Some(root);
        }
    }

    /// One-shot consume: returns the stashed root and clears the slot.
    pub fn take_pending_workspace_root(&self) -> Option<PathBuf> {
        self.pending_workspace_root
            .lock()
            .ok()
            .and_then(|mut s| s.take())
    }

    /// Total number of `deal_parse` calls this Documents instance has issued.
    /// Test hook for `debounce_collapses_rapid_changes`.
    pub fn parse_count(&self) -> usize {
        self.parse_count.load(Ordering::SeqCst)
    }

    /// Read the current Rope for a URI (for formatting full-doc range).
    pub fn get_buffer(&self, uri: &Url) -> Option<Rope> {
        self.buffers.get(uri).map(|r| r.clone())
    }

    /// Borrow the Arc'd handle for a URI (for formatting / Plan 04 callers).
    pub fn get_handle(&self, uri: &Url) -> Option<Arc<Mutex<OwnedDealHandle>>> {
        self.handles.get(uri).map(|h| h.clone())
    }

    /// Snapshot every in-memory workspace buffer as `(Url, String)`. After
    /// `eager_parse` this is the whole workspace, so the rename verify-gate
    /// (rename.rs) can re-analyze the post-edit workspace against the same
    /// merged declaration set `eager_parse` used.
    pub fn snapshot_buffers(&self) -> Vec<(Url, String)> {
        self.buffers
            .iter()
            .map(|kv| (kv.key().clone(), kv.value().to_string()))
            .collect()
    }

    /// did_open path: parse, install handle + Rope, publish diagnostics.
    ///
    /// On parse failure (OOM) returns Err and publishes no diagnostics —
    /// the LSP keeps running.
    pub async fn open(
        &self,
        uri: Url,
        text: String,
        version: Option<i32>,
        client: &Client,
    ) -> anyhow::Result<()> {
        let filename = uri.path();
        let handle = deal_ffi::safe::parse(text.as_bytes(), filename)
            .ok_or_else(|| anyhow::anyhow!("deal_parse returned null for {uri}"))?;
        self.parse_count.fetch_add(1, Ordering::SeqCst);

        // Publish diagnostics before storing — if the envelope fetch fails
        // we still want to install the handle for later format/hover/etc.
        // The rope is required to translate byte spans into LSP (line, char).
        let rope = Rope::from_str(&text);
        let diag_bytes = deal_ffi::safe::diagnostics_json(&handle).unwrap_or_default();
        let diagnostics_vec = diagnostics::parse_diagnostics(&diag_bytes, &rope);

        self.buffers.insert(uri.clone(), rope);
        self.handles
            .insert(uri.clone(), Arc::new(Mutex::new(handle)));

        client
            .publish_diagnostics(uri, diagnostics_vec, version)
            .await;
        Ok(())
    }

    /// Silent variant of `open` used by the eager workspace parse
    /// (Plan 03-04 / D-47). Parses + installs the handle + Rope +
    /// populates the in-memory `Index` from the handle's `deal_index_json`
    /// envelope — but does NOT publish diagnostics (we do not want to
    /// spam the client with diagnostics for files the user has not
    /// opened in their editor; diagnostics fire on did_open/did_change).
    ///
    /// Returns Err on parse failure (OOM) so the caller can log + skip.
    pub async fn open_silent(&self, uri: Url, text: String, index: &Index) -> anyhow::Result<()> {
        let filename = uri.path();
        let handle = deal_ffi::safe::parse(text.as_bytes(), filename)
            .ok_or_else(|| anyhow::anyhow!("deal_parse returned null for {uri}"))?;
        self.parse_count.fetch_add(1, Ordering::SeqCst);

        let rope = Rope::from_str(&text);
        // Populate the workspace index from this file's symbol table BEFORE
        // dropping the handle (clone-before-free invariant — safe wrappers
        // own this discipline, but we still hold the handle here).
        let envelope_bytes = deal_ffi::safe::index_json(&handle).unwrap_or_default();
        index.update_from_envelope(&uri, &envelope_bytes, &rope);

        self.buffers.insert(uri.clone(), rope);
        self.handles.insert(uri, Arc::new(Mutex::new(handle)));

        Ok(())
    }

    /// Like `open_silent`, but analyzes the file against a workspace-merged
    /// declaration set (`external_sources`, the bytes of every workspace file)
    /// so cross-file references resolve to their declaring fully-qualified id
    /// (ADR-0003). The handle's `deal_index_json` then carries cross-file
    /// `references[]`, which `Index::update_from_envelope` ingests into the
    /// reverse usage index. Used by the two-phase `eager_parse`.
    ///
    /// Including the file itself in `external_sources` is harmless — local
    /// declarations win in `resolveName`'s first tier.
    pub async fn open_silent_with_external(
        &self,
        uri: Url,
        text: String,
        external_sources: &[&[u8]],
        index: &Index,
    ) -> anyhow::Result<()> {
        let filename = uri.path();
        let handle = deal_ffi::safe::check_with_external(text.as_bytes(), filename, external_sources)
            .ok_or_else(|| anyhow::anyhow!("deal_check_with_external returned null for {uri}"))?;
        self.parse_count.fetch_add(1, Ordering::SeqCst);

        let rope = Rope::from_str(&text);
        let envelope_bytes = deal_ffi::safe::index_json(&handle).unwrap_or_default();
        index.update_from_envelope(&uri, &envelope_bytes, &rope);

        self.buffers.insert(uri.clone(), rope);
        self.handles.insert(uri, Arc::new(Mutex::new(handle)));

        Ok(())
    }

    /// did_change path: drop the prior handle (Drop fires deal_free), parse
    /// the new buffer, publish updated diagnostics, and refresh the
    /// workspace index slice for this URI (D-46 invariant — in-memory
    /// index stays current as the user types). `index` is optional so
    /// pre-Plan-04 code paths that never received an Index still compile.
    pub async fn update(
        &self,
        uri: Url,
        text: String,
        version: Option<i32>,
        client: &Client,
        index: Option<&Index>,
    ) -> anyhow::Result<()> {
        let filename = uri.path();
        let handle = deal_ffi::safe::parse(text.as_bytes(), filename)
            .ok_or_else(|| anyhow::anyhow!("deal_parse returned null for {uri}"))?;
        self.parse_count.fetch_add(1, Ordering::SeqCst);

        let rope = Rope::from_str(&text);
        let diag_bytes = deal_ffi::safe::diagnostics_json(&handle).unwrap_or_default();
        let diagnostics_vec = diagnostics::parse_diagnostics(&diag_bytes, &rope);

        // Refresh the workspace index for this URI (drop stale entries,
        // ingest fresh ones) BEFORE dropping the handle.
        if let Some(idx) = index {
            let envelope_bytes = deal_ffi::safe::index_json(&handle).unwrap_or_default();
            idx.refresh_file(&uri, &envelope_bytes, &rope);
        }

        self.buffers.insert(uri.clone(), rope);
        // Inserting replaces the prior Arc; the displaced Arc drops when no
        // other task holds it, releasing the Mutex which releases the
        // OwnedDealHandle which calls deal_free.
        self.handles
            .insert(uri.clone(), Arc::new(Mutex::new(handle)));

        client
            .publish_diagnostics(uri, diagnostics_vec, version)
            .await;
        Ok(())
    }
}

impl Default for Documents {
    fn default() -> Self {
        Self::new()
    }
}
