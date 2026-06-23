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

use std::collections::BTreeMap;
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

/// The cached workspace import closure (ADR-0004 P5 WS-B/WS-C). `eager_parse`
/// builds it; the did-change live-strict path re-analyzes the edited file
/// against `blob` (the salsa "durable global"), and rebuilds the whole cache
/// when an edit changes a file's reachability (its import set).
pub struct ClosureCache {
    /// Every file in the closure (workspace + reachable deps, incl. stdlib), in
    /// deterministic closure order — parallel to `blob`. `eager_parse` analyzes
    /// and INDEXES each of these (not just workspace files) so goto/hover INTO
    /// dependency/stdlib declarations resolves.
    pub files: Vec<PathBuf>,
    /// Source bytes of every file in `files` (same order) — the external table
    /// fed to `check_with_external`.
    pub blob: Vec<Vec<u8>>,
    /// path → that file's sorted import paths, for the WS-C.2 reachability gate.
    pub imports: BTreeMap<PathBuf, Vec<String>>,
    /// Rebuild context: the workspace root and the configured stdlib path.
    pub root: PathBuf,
    pub stdlib_path: Option<PathBuf>,
}

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
    /// One-shot hand-off slot for a configured stdlib path (ADR-0004 P5 WS-B):
    /// `initialize` reads it from `initializationOptions` and stashes it here for
    /// `initialized` → `eager_parse` to seed the stdlib into the closure.
    pending_stdlib_path: StdMutex<Option<PathBuf>>,
    /// Cached workspace import closure (ADR-0004 P5 WS-B/WS-7). Built once by
    /// `eager_parse`; `did_change` re-analyzes the edited file against
    /// `cache.blob` (the salsa "global derived data" boundary) and rebuilds the
    /// cache when an edit changes reachability. `Arc` so readers clone cheaply.
    closure: StdMutex<Option<Arc<ClosureCache>>>,
}

impl Documents {
    pub fn new() -> Self {
        Self {
            handles: DashMap::new(),
            buffers: DashMap::new(),
            parse_count: AtomicUsize::new(0),
            pending_workspace_root: StdMutex::new(None),
            tokens_cache: DashMap::new(),
            pending_stdlib_path: StdMutex::new(None),
            closure: StdMutex::new(None),
        }
    }

    /// Stash a configured stdlib path for `initialized` to consume (WS-B).
    pub fn set_pending_stdlib_path(&self, path: PathBuf) {
        if let Ok(mut slot) = self.pending_stdlib_path.lock() {
            *slot = Some(path);
        }
    }

    /// One-shot consume of the configured stdlib path.
    pub fn take_pending_stdlib_path(&self) -> Option<PathBuf> {
        self.pending_stdlib_path
            .lock()
            .ok()
            .and_then(|mut s| s.take())
    }

    /// Install the cached workspace closure (WS-B; replaced on WS-C.2 rebuild).
    pub fn set_closure_cache(&self, cache: ClosureCache) {
        if let Ok(mut slot) = self.closure.lock() {
            *slot = Some(Arc::new(cache));
        }
    }

    /// Read the cached workspace closure, if built (WS-7 did-change path).
    pub fn closure_cache(&self) -> Option<Arc<ClosureCache>> {
        self.closure.lock().ok().and_then(|s| s.clone())
    }

    /// Snapshot all open editor buffers as `(path, bytes)` for a buffer-aware
    /// closure rebuild (WS-C.2) — so unsaved edits are reflected. Files not open
    /// in the editor fall back to disk during the rebuild.
    pub fn buffer_sources(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        let mut out = BTreeMap::new();
        for kv in self.buffers.iter() {
            if let Ok(path) = kv.key().to_file_path() {
                out.insert(path, kv.value().to_string().into_bytes());
            }
        }
        out
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

        // ADR-0004 P5 WS-7 (live-strict): if the workspace import closure has
        // been built, analyze the edited file against its cached external table
        // (cross-file + stdlib references resolve, import-scoping is enforced) —
        // the salsa "global derived data" boundary: the closure is the durable
        // global, this keystroke is the local recompute. With no closure (a
        // scratch file, or before eager_parse finishes) fall back to lenient
        // single-file parse so a file outside any workspace shows no false
        // strict errors (D6).
        //
        // SAFETY/D-88: the safe wrappers clone all JSON out before the handle is
        // freed, and analyzeWithExternalTable copies resolved entries into the
        // handle's own arena, so the stored handle is self-contained (same
        // pattern as `open_silent_with_external`).
        let cache = self.closure_cache();
        let handle = if let Some(cache) = &cache {
            let refs: Vec<&[u8]> = cache.blob.iter().map(|v| v.as_slice()).collect();
            deal_ffi::safe::check_with_external(text.as_bytes(), filename, &refs)
                .ok_or_else(|| anyhow::anyhow!("deal_check_with_external returned null for {uri}"))?
        } else {
            deal_ffi::safe::parse(text.as_bytes(), filename)
                .ok_or_else(|| anyhow::anyhow!("deal_parse returned null for {uri}"))?
        };
        self.parse_count.fetch_add(1, Ordering::SeqCst);

        let rope = Rope::from_str(&text);
        let diag_bytes = deal_ffi::safe::diagnostics_json(&handle).unwrap_or_default();
        let diagnostics_vec = diagnostics::parse_diagnostics(&diag_bytes, &rope);

        // Envelope for the index refresh + the WS-C.2 reachability gate.
        let envelope_bytes = deal_ffi::safe::index_json(&handle).unwrap_or_default();
        if let Some(idx) = index {
            idx.refresh_file(&uri, &envelope_bytes, &rope);
        }

        self.buffers.insert(uri.clone(), rope);
        // Inserting replaces the prior Arc; the displaced Arc drops when no
        // other task holds it, releasing the Mutex which releases the
        // OwnedDealHandle which calls deal_free.
        self.handles
            .insert(uri.clone(), Arc::new(Mutex::new(handle)));

        // WS-C.2 invalidation: if this edit changed the file's import set, the
        // cached closure's reachability is stale — rebuild it from the live
        // editor buffers (+ disk) so OTHER files re-analyze against the new
        // structure. Body edits (imports unchanged) reuse the cache. The buffer
        // was inserted above, so the rebuild snapshot already includes this edit.
        if let Some(cache) = &cache {
            if let Ok(path) = uri.to_file_path() {
                let new_imports = deal_closure::imports_from_index_json(&envelope_bytes);
                if cache.imports.get(&path) != Some(&new_imports) {
                    let overrides = self.buffer_sources();
                    if let Some(rebuilt) = crate::workspace::build_closure_cache(
                        &cache.root,
                        cache.stdlib_path.as_deref(),
                        &overrides,
                    ) {
                        self.set_closure_cache(rebuilt);
                    }
                }
            }
        }

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
