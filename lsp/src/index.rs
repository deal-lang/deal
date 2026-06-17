//! In-memory workspace symbol index (D-46 / D-47 / D-49).
//!
//! Maintains `DashMap<PathString, (FileUri, Range)>` populated by walking the
//! parsed `deal_index_json` envelope of every workspace document. The
//! in-memory map is authoritative — Plan 04 invariant D-48 forbids the LSP
//! from writing back to `.deal/index.json` (the disk artifact remains the
//! CLI's responsibility). This module DOES NOT call `fs::write` to the
//! `.deal/index.json` path or any equivalent — verify with the grep gate
//! cited in PLAN.md `<verification>`.
//!
//! ## Wire format (verified live via probe binary, 2026-05-25)
//!
//! ```json
//! {
//!   "v": 1,
//!   "deal_version": "0.1.0-phase2",
//!   "elements": {
//!     "vehicle.battery.BatteryPack": {
//!       "id": "vehicle.battery.BatteryPack",
//!       "kind": "part_def",
//!       "source_file": "/abs/path/.../battery.deal",
//!       "span": [2624, 4854]
//!     },
//!     ...
//!   },
//!   "imports_graph": [...]
//! }
//! ```
//!
//! Spans are UTF-8 byte offsets into the source file; LSP needs
//! `Range { start: Position, end: Position }` in (line, UTF-16-character)
//! coordinates. The `update_*` paths translate via `ropey::Rope`.
//!
//! ## PS-5 workspace alias resolution
//!
//! `[workspace.aliases]` in `deal.toml` maps short namespaces to long ones
//! (e.g. `vehicle = "packages/vehicle"`). Per D-49 the resolution layers on
//! top of the merged HashMap in Rust without modifying the Zig core: callers
//! invoke `resolve_with_alias` to expand a possibly-aliased path before
//! `lookup`.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use dashmap::DashMap;
use ropey::Rope;
use serde::Deserialize;
use tower_lsp::lsp_types::{Location, Position, Range, SymbolInformation, SymbolKind, Url};

/// Fully-qualified path-string ID (D-23). e.g. `"vehicle.battery.BatteryPack"`.
///
/// The `symbols` field below is therefore concretely `DashMap<String, (Url, Range)>`
/// (PLAN.md verify-gate substring `DashMap<String`).
pub type PathString = String;

/// P3 WS-C: a declaration's locations + kind within one file, as returned by
/// [`Index::elements_in_file`]. Consumed by the inlay-hint and code-lens
/// providers.
#[derive(Debug, Clone)]
pub struct ElementInfo {
    /// Canonical fully-qualified path (e.g. `vehicle.battery.BatteryPack`).
    pub path: PathString,
    /// Range of the whole definition (keyword through closing brace).
    pub decl_range: Range,
    /// Range of the declared name token (falls back to `decl_range`).
    pub name_range: Range,
    /// Element kind string (e.g. `part_def`).
    pub kind: String,
}

/// In-memory workspace symbol index.
///
/// Concurrent reads from completion / hover / definition providers share the
/// underlying `DashMap` with concurrent writes from `update_from_handle` /
/// `update_from_file` invoked off the did_change debounce callback.
pub struct Index {
    symbols: DashMap<PathString, (Url, Range)>,
    /// Parallel map carrying the element `kind` string (e.g. "part_def") for
    /// each PathString, alongside its owning Url so `remove_file` can evict by
    /// file. Kept separate from `symbols` so the existing `(Url, Range)` value
    /// shape — and its consumers in completion.rs / definition.rs — stay
    /// untouched. Consumed by `workspace_symbols` to assign SymbolKind icons.
    kinds: DashMap<PathString, (Url, String)>,
    /// P2 WS-A: reverse usage index. Maps a resolved fully-qualified path to the
    /// reference sites that bind to it, sourced from the compiler's own
    /// resolution (the envelope's `references[]`). Powers find-references /
    /// documentHighlight / rename with compiler-exact, not heuristic, results.
    usages: DashMap<PathString, Vec<(Url, Range)>>,
    /// P2 WS-C0: precise declared-NAME range per symbol (from the envelope's
    /// `name_span`), keyed by FQ path. Used by rename / documentHighlight to
    /// edit/mark the declaration's identifier token rather than its whole span.
    decl_names: DashMap<PathString, (Url, Range)>,
    /// P3 WS-C: per-file `import`-item reference sites paired with the canonical
    /// path they resolve to (`ref_kind == "import"` from the envelope). Keyed by
    /// the importing file's Url. Powers `documentLink` (import → declaring file)
    /// — kept separate from `usages` because that map drops the ref-kind.
    import_links: DashMap<Url, Vec<(Range, PathString)>>,
    /// PS-5: short → long PathString prefix expansions from
    /// `[workspace.aliases]` in deal.toml. Wrapped in RwLock so the
    /// `backend::initialized` handler can swap the table once the
    /// workspace is discovered, without needing exclusive `&mut Index`
    /// access (Index lives behind `Arc<Index>` shared across tasks).
    aliases: RwLock<Arc<HashMap<String, String>>>,
}

#[derive(Debug, Deserialize)]
struct IndexEnvelope {
    #[serde(default)]
    elements: HashMap<String, ElementEntry>,
    /// P2 WS-A: resolved reference bindings. `#[serde(default)]` so pre-P2
    /// envelopes (no `references` key) still deserialize cleanly.
    #[serde(default)]
    references: Vec<RefEntry>,
}

#[derive(Debug, Deserialize)]
struct ElementEntry {
    /// `[start_byte, end_byte]` half-open UTF-8 byte span.
    span: [usize; 2],
    /// Element kind from the envelope (e.g. "part_def", "requirement_def").
    /// Defaults to empty when absent so older envelopes still deserialize.
    #[serde(default)]
    kind: String,
    /// P2 WS-C0: `[start, end]` byte span of the declared NAME token (subset of
    /// `span`). `#[serde(default)]` so pre-WS-C0 envelopes still deserialize.
    #[serde(default)]
    name_span: Option<[usize; 2]>,
}

/// P2 WS-A: one resolved reference binding from the envelope's `references[]`.
#[derive(Debug, Deserialize)]
struct RefEntry {
    /// `[start_byte, end_byte]` half-open UTF-8 byte span of the reference site.
    from_span: [usize; 2],
    /// Fully-qualified path the reference resolved to.
    #[serde(default)]
    resolved_path: String,
    /// Reference kind ("specializes", "type_ref", …). Retained for future
    /// filtering; not yet used by the query API.
    #[serde(default)]
    ref_kind: String,
}

impl Index {
    pub fn new() -> Self {
        Self {
            symbols: DashMap::new(),
            kinds: DashMap::new(),
            usages: DashMap::new(),
            decl_names: DashMap::new(),
            import_links: DashMap::new(),
            aliases: RwLock::new(Arc::new(HashMap::new())),
        }
    }

    pub fn with_aliases(aliases: HashMap<String, String>) -> Self {
        Self {
            symbols: DashMap::new(),
            kinds: DashMap::new(),
            usages: DashMap::new(),
            decl_names: DashMap::new(),
            import_links: DashMap::new(),
            aliases: RwLock::new(Arc::new(aliases)),
        }
    }

    /// Replace the alias table (called once after `Workspace::discover`).
    /// Interior-mutable: works through `&Index` so `Arc<Index>` callers can
    /// invoke it without exclusive ownership.
    pub fn replace_aliases(&self, aliases: HashMap<String, String>) {
        if let Ok(mut slot) = self.aliases.write() {
            *slot = Arc::new(aliases);
        }
    }

    pub fn aliases(&self) -> Arc<HashMap<String, String>> {
        self.aliases
            .read()
            .map(|a| a.clone())
            .unwrap_or_else(|_| Arc::new(HashMap::new()))
    }

    /// Test hook for the workspace_index_populated_after_initialize test
    /// (PLAN.md Task 4 row "showcase symbol count threshold").
    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    /// O(1) HashMap lookup. Performs alias expansion on the input first.
    pub fn lookup(&self, path: &str) -> Option<(Url, Range)> {
        let expanded = self.resolve_with_alias(path);
        self.symbols.get(expanded.as_str()).map(|v| v.clone())
    }

    /// PS-5: if `possibly_short` starts with an alias key followed by `.` or
    /// equals the alias key exactly, replace the prefix with the canonical
    /// PathString. Otherwise return the input unchanged.
    pub fn resolve_with_alias(&self, possibly_short: &str) -> String {
        let aliases = self.aliases();
        if let Some(canonical) = aliases.get(possibly_short) {
            return canonical.clone();
        }
        for (short, long) in aliases.iter() {
            let prefix = format!("{short}.");
            if let Some(rest) = possibly_short.strip_prefix(&prefix) {
                return format!("{long}.{rest}");
            }
        }
        possibly_short.to_string()
    }

    /// Stream a snapshot of all (PathString, (Url, Range)) entries.
    /// Used by completion to enumerate workspace types.
    pub fn snapshot(&self) -> Vec<(PathString, (Url, Range))> {
        self.symbols
            .iter()
            .map(|e| (e.key().clone(), e.value().clone()))
            .collect()
    }

    /// Populate / refresh the symbols belonging to `uri` from a freshly-parsed
    /// handle's `deal_index_json` envelope. `rope` carries the document text
    /// used to translate byte spans into LSP positions.
    ///
    /// Caller is responsible for first invoking `remove_file` if this is a
    /// re-parse (see `update_from_file` for the combined helper).
    pub fn update_from_envelope(&self, uri: &Url, envelope_bytes: &[u8], rope: &Rope) {
        if envelope_bytes.is_empty() {
            return;
        }
        let env: IndexEnvelope = match serde_json::from_slice(envelope_bytes) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("index envelope parse failed for {uri}: {e}");
                return;
            }
        };
        for (path, entry) in env.elements {
            let start = byte_to_position(rope, entry.span[0]);
            let end = byte_to_position(rope, entry.span[1]);
            self.kinds.insert(path.clone(), (uri.clone(), entry.kind));
            // P2 WS-C0: record the precise declared-name range when present
            // (non-empty span); rename/highlight prefer it over the whole-def span.
            if let Some([ns, ne]) = entry.name_span {
                if ne > ns {
                    let nstart = byte_to_position(rope, ns);
                    let nend = byte_to_position(rope, ne);
                    self.decl_names.insert(
                        path.clone(),
                        (uri.clone(), Range { start: nstart, end: nend }),
                    );
                }
            }
            self.symbols
                .insert(path, (uri.clone(), Range { start, end }));
        }
        // P2 WS-A: ingest resolved reference sites into the reverse usage index.
        for r in env.references {
            if r.resolved_path.is_empty() {
                continue;
            }
            let start = byte_to_position(rope, r.from_span[0]);
            let end = byte_to_position(rope, r.from_span[1]);
            let range = Range { start, end };
            // P3 WS-C: an `import P.{N}` item — record it for documentLink so the
            // import statement can navigate to N's declaring file (goto-definition
            // does not reach import items: they are payload strings, not AST nodes).
            if r.ref_kind == "import" {
                self.import_links
                    .entry(uri.clone())
                    .or_default()
                    .push((range, r.resolved_path.clone()));
            }
            self.usages
                .entry(r.resolved_path)
                .or_default()
                .push((uri.clone(), range));
        }
    }

    /// Remove every entry whose value points at `uri` (called before
    /// re-inserting from a fresh re-parse).
    pub fn remove_file(&self, uri: &Url) {
        self.symbols.retain(|_, v| &v.0 != uri);
        self.kinds.retain(|_, v| &v.0 != uri);
        self.decl_names.retain(|_, v| &v.0 != uri);
        // P2 WS-A: drop this file's reference sites; remove now-empty buckets.
        self.usages.retain(|_, sites| {
            sites.retain(|(u, _)| u != uri);
            !sites.is_empty()
        });
        // P3 WS-C: import links are keyed by the importing file.
        self.import_links.remove(uri);
    }

    /// P2 WS-A: find-references. Returns the declaration (when `include_decl`)
    /// plus every reference site that the compiler resolved to `path`. Results
    /// are de-duplicated and ordered deterministically (uri, then position).
    pub fn references_of(&self, path: &str, include_decl: bool) -> Vec<Location> {
        // `path` is already a canonical fully-qualified id (the symbols/usages
        // key) — it comes from `definition::resolved_path_at` or from the
        // binding's `resolved_path`. We must NOT run `resolve_with_alias` here:
        // workspace aliases map package names to *directory* paths (e.g.
        // `interfaces = "packages/interfaces"`), so alias-expanding a canonical
        // id like `interfaces.thermal.ThermallyManaged` produces a bogus key and
        // misses. (Definition tolerates this via a suffix-match fallback; the
        // reverse index keys are exact.)
        let mut out: Vec<Location> = Vec::new();
        if include_decl {
            // Precise declared-name location (WS-C0) when available.
            if let Some(loc) = self.declaration_location(path) {
                out.push(loc);
            }
        }
        if let Some(sites) = self.usages.get(path) {
            for (u, r) in sites.value().iter() {
                out.push(Location {
                    uri: u.clone(),
                    range: *r,
                });
            }
        }
        out.sort_by(|a, b| {
            a.uri
                .as_str()
                .cmp(b.uri.as_str())
                .then(a.range.start.line.cmp(&b.range.start.line))
                .then(a.range.start.character.cmp(&b.range.start.character))
        });
        out.dedup_by(|a, b| a.uri == b.uri && a.range == b.range);
        out
    }

    /// P2 WS-C: true if a declaration with this exact FQ path exists (for
    /// rename collision detection).
    pub fn contains_path(&self, path: &str) -> bool {
        self.symbols.contains_key(path)
    }

    /// P2 WS-B: reference-site ranges for `path` that live in `uri` (for
    /// documentHighlight, which is single-file). `path` is a canonical id — no
    /// alias resolution (see `references_of`).
    pub fn usages_in_file(&self, uri: &Url, path: &str) -> Vec<Range> {
        self.usages
            .get(path)
            .map(|sites| {
                sites
                    .value()
                    .iter()
                    .filter(|(u, _)| u == uri)
                    .map(|(_, r)| *r)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// P2 WS-B: the declaration range for `path` IFF it is declared in `uri`
    /// (so documentHighlight can mark it `Write`). Canonical id — no alias
    /// resolution.
    pub fn declaration_in_file(&self, uri: &Url, path: &str) -> Option<Range> {
        // Prefer the precise declared-name range (WS-C0); fall back to the
        // whole-definition span for kinds that don't yet carry a name_span
        // (e.g. calc/constraint defs).
        if let Some(v) = self.decl_names.get(path) {
            if &v.0 == uri {
                return Some(v.1);
            }
        }
        self.symbols
            .get(path)
            .and_then(|v| if &v.0 == uri { Some(v.1) } else { None })
    }

    /// P2 WS-C0: the precise declared-name `Location` for a symbol (the name
    /// token, not the whole definition), used by rename to edit the declaration.
    /// Falls back to the whole-definition span when no name_span was recorded.
    pub fn declaration_location(&self, path: &str) -> Option<Location> {
        if let Some(v) = self.decl_names.get(path) {
            let (u, r) = v.clone();
            return Some(Location { uri: u, range: r });
        }
        self.symbols.get(path).map(|v| {
            let (u, r) = v.clone();
            Location { uri: u, range: r }
        })
    }

    /// P3 WS-C: `import`-item links in `uri` — each `(import_item_range,
    /// declaration_location)` pair, for `documentLink`. Items whose target is
    /// not (yet) in the index are skipped, so no dead links are produced.
    pub fn import_links_in_file(&self, uri: &Url) -> Vec<(Range, Location)> {
        self.import_links
            .get(uri)
            .map(|sites| {
                sites
                    .value()
                    .iter()
                    .filter_map(|(range, path)| {
                        self.declaration_location(path).map(|loc| (*range, loc))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// P3 WS-C: every declaration in `uri` with its full-definition range, its
    /// precise declared-name range (falling back to the full span), and its
    /// element kind. Shared by inlay hints (metaclass badge at the name) and
    /// code lens (reference count + emit, anchored at the definition).
    pub fn elements_in_file(&self, uri: &Url) -> Vec<ElementInfo> {
        self.kinds
            .iter()
            .filter(|e| &e.value().0 == uri)
            .filter_map(|e| {
                let path = e.key().clone();
                let kind = e.value().1.clone();
                let decl_range = self
                    .symbols
                    .get(&path)
                    .and_then(|v| if &v.0 == uri { Some(v.1) } else { None })?;
                let name_range = self
                    .decl_names
                    .get(&path)
                    .and_then(|v| if &v.0 == uri { Some(v.1) } else { None })
                    .unwrap_or(decl_range);
                Some(ElementInfo {
                    path,
                    decl_range,
                    name_range,
                    kind,
                })
            })
            .collect()
    }

    /// Build LSP `SymbolInformation` entries for the `workspace/symbol`
    /// request. `query` is matched case-insensitively as a substring against
    /// each fully-qualified PathString; an empty query returns everything.
    /// Results are ordered deterministically (container, then name).
    #[allow(deprecated)] // SymbolInformation + its `deprecated` field are deprecated upstream.
    pub fn workspace_symbols(&self, query: &str) -> Vec<SymbolInformation> {
        let q = query.to_ascii_lowercase();
        let mut out: Vec<SymbolInformation> = self
            .symbols
            .iter()
            .filter_map(|entry| {
                let fqpn = entry.key();
                if !q.is_empty() && !fqpn.to_ascii_lowercase().contains(&q) {
                    return None;
                }
                let val = entry.value();
                let uri = val.0.clone();
                let range = val.1;
                let (name, container) = match fqpn.rsplit_once('.') {
                    Some((parent, last)) => (last.to_string(), Some(parent.to_string())),
                    None => (fqpn.clone(), None),
                };
                let kind = self
                    .kinds
                    .get(fqpn)
                    .map(|k| map_symbol_kind(&k.value().1))
                    .unwrap_or(SymbolKind::OBJECT);
                Some(SymbolInformation {
                    name,
                    kind,
                    tags: None,
                    deprecated: None,
                    location: Location { uri, range },
                    container_name: container,
                })
            })
            .collect();
        out.sort_by(|a, b| {
            a.container_name
                .cmp(&b.container_name)
                .then_with(|| a.name.cmp(&b.name))
        });
        out
    }

    /// Combined refresh: drop the prior entries for this URI, then ingest
    /// the new envelope. Use this on did_change's debounced re-parse path.
    pub fn refresh_file(&self, uri: &Url, envelope_bytes: &[u8], rope: &Rope) {
        self.remove_file(uri);
        self.update_from_envelope(uri, envelope_bytes, rope);
    }
}

impl Default for Index {
    fn default() -> Self {
        Self::new()
    }
}

/// Map a DEAL element-kind string to the closest LSP `SymbolKind` so the
/// editor's symbol picker (Ctrl/Cmd+T) shows a sensible icon. Unknown kinds
/// fall back to `OBJECT`.
pub(crate) fn map_symbol_kind(kind: &str) -> SymbolKind {
    match kind {
        "part_def" => SymbolKind::CLASS,
        "port_def" | "interface_def" => SymbolKind::INTERFACE,
        "action_def" => SymbolKind::METHOD,
        "state_def" => SymbolKind::ENUM,
        "requirement_def" | "need_def" => SymbolKind::PROPERTY,
        "constraint_def" => SymbolKind::OPERATOR,
        "calc_def" => SymbolKind::FUNCTION,
        "attribute_def" | "attribute_usage" => SymbolKind::FIELD,
        "item_def" => SymbolKind::STRUCT,
        "connection_def" | "flow_def" | "allocation_def" => SymbolKind::EVENT,
        "use_case_def" => SymbolKind::CLASS,
        "actor_def" => SymbolKind::CONSTANT,
        _ => SymbolKind::OBJECT,
    }
}

/// Translate a UTF-8 byte offset into a `(line, UTF-16-character)` Position.
/// Out-of-range bytes clamp to the end of the document — the LSP must never
/// panic on a malformed envelope.
pub(crate) fn byte_to_position(rope: &Rope, byte: usize) -> Position {
    let max_bytes = rope.len_bytes();
    let byte = byte.min(max_bytes);
    let char_idx = rope.byte_to_char(byte);
    let line = rope.char_to_line(char_idx);
    let line_start_char = rope.line_to_char(line);
    let prefix = rope.slice(line_start_char..char_idx);
    let character: u32 = prefix.chars().map(|c| c.len_utf16() as u32).sum();
    Position {
        line: line as u32,
        character,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url() -> Url {
        Url::parse("file:///workspace/pkg/file.deal").unwrap()
    }

    #[test]
    fn empty_envelope_is_noop() {
        let idx = Index::new();
        idx.update_from_envelope(&url(), b"", &Rope::new());
        assert_eq!(idx.len(), 0);
    }

    #[test]
    fn malformed_envelope_is_silent() {
        let idx = Index::new();
        idx.update_from_envelope(&url(), b"not json", &Rope::new());
        assert_eq!(idx.len(), 0);
    }

    #[test]
    fn ingest_real_shape_round_trip() {
        // Two definitions on lines 0 and 2.
        let source = "part def A { }\n\npart def B { }\n";
        let rope = Rope::from_str(source);
        let env = br#"{
            "v": 1,
            "elements": {
                "pkg.A": {"id":"pkg.A","kind":"part_def","source_file":"x","span":[0,14]},
                "pkg.B": {"id":"pkg.B","kind":"part_def","source_file":"x","span":[16,30]}
            },
            "imports_graph": []
        }"#;
        let idx = Index::new();
        idx.update_from_envelope(&url(), env, &rope);
        assert_eq!(idx.len(), 2);
        let a = idx.lookup("pkg.A").unwrap();
        assert_eq!(a.1.start.line, 0);
        assert_eq!(a.1.start.character, 0);
        assert_eq!(a.1.end.line, 0);
        assert_eq!(a.1.end.character, 14);
        let b = idx.lookup("pkg.B").unwrap();
        assert_eq!(b.1.start.line, 2);
    }

    #[test]
    fn references_index_ingests_and_queries_resolved_sites() {
        // P2 WS-A: an envelope carrying references[] populates the reverse usage
        // index; references_of returns the sites (and decl when requested).
        let idx = Index::new();
        let url_a = Url::parse("file:///a.deal").unwrap();
        let rope = Rope::from_str("part def Battery {}\npart def Pack {}\n");
        // Battery is declared in a.deal and referenced (specializes) twice.
        let env = br#"{"v":1,
            "elements":{"vehicle.Battery":{"id":"vehicle.Battery","kind":"part_def","source_file":"a","span":[9,16]}},
            "references":[
                {"from_span":[28,35],"ref_kind":"specializes","resolved_path":"vehicle.Battery"},
                {"from_span":[40,47],"ref_kind":"specializes","resolved_path":"vehicle.Battery"}
            ]}"#;
        idx.update_from_envelope(&url_a, env, &rope);

        // Two reference sites, declaration excluded.
        let refs = idx.references_of("vehicle.Battery", false);
        assert_eq!(refs.len(), 2, "expected two reference sites");
        // include_declaration adds the decl site.
        let with_decl = idx.references_of("vehicle.Battery", true);
        assert_eq!(with_decl.len(), 3, "expected two refs + declaration");
        // Unknown path yields nothing.
        assert!(idx.references_of("vehicle.Nope", true).is_empty());
    }

    #[test]
    fn usages_in_file_and_declaration_in_file_scope_by_uri() {
        // P2 WS-B: documentHighlight queries. Battery references Battery's decl
        // from two files; the decl lives in a.deal.
        let idx = Index::new();
        let url_a = Url::parse("file:///a.deal").unwrap();
        let url_b = Url::parse("file:///b.deal").unwrap();
        let rope = Rope::from_str("part def Battery {}\npart def Pack {}\n");
        let env_a = br#"{"v":1,
            "elements":{"vehicle.Battery":{"id":"vehicle.Battery","kind":"part_def","source_file":"a","span":[9,16]}},
            "references":[{"from_span":[28,35],"ref_kind":"specializes","resolved_path":"vehicle.Battery"}]}"#;
        let env_b = br#"{"v":1,"elements":{},
            "references":[{"from_span":[5,12],"ref_kind":"specializes","resolved_path":"vehicle.Battery"}]}"#;
        idx.update_from_envelope(&url_a, env_a, &rope);
        idx.update_from_envelope(&url_b, env_b, &rope);

        // a.deal: one usage + the declaration.
        assert_eq!(idx.usages_in_file(&url_a, "vehicle.Battery").len(), 1);
        assert!(idx.declaration_in_file(&url_a, "vehicle.Battery").is_some());
        // b.deal: one usage, no declaration.
        assert_eq!(idx.usages_in_file(&url_b, "vehicle.Battery").len(), 1);
        assert!(idx.declaration_in_file(&url_b, "vehicle.Battery").is_none());
        // Unknown path: nothing.
        assert!(idx.usages_in_file(&url_a, "vehicle.Nope").is_empty());
    }

    #[test]
    fn declaration_in_file_prefers_precise_name_span() {
        // P2 WS-C0: element span covers the whole def [0,19]; name_span is the
        // "Battery" token [9,16]. declaration_in_file must return the name range.
        let idx = Index::new();
        let url = Url::parse("file:///a.deal").unwrap();
        let rope = Rope::from_str("part def Battery {}\n");
        let env = br#"{"v":1,"elements":{
            "vehicle.Battery":{"id":"vehicle.Battery","kind":"part_def","name_span":[9,16],"source_file":"a","span":[0,19]}
        }}"#;
        idx.update_from_envelope(&url, env, &rope);
        let r = idx
            .declaration_in_file(&url, "vehicle.Battery")
            .expect("declaration present");
        // Name token starts at byte 9 ('B' of Battery) on line 0.
        assert_eq!(r.start.line, 0);
        assert_eq!(r.start.character, 9, "should use the name_span, not the whole-def span");
        assert_eq!(r.end.character, 16);
    }

    #[test]
    fn references_index_evicts_on_refresh() {
        // Re-parsing a file drops its old reference sites (no orphans).
        let idx = Index::new();
        let url = Url::parse("file:///a.deal").unwrap();
        let rope = Rope::from_str("part def Battery {}\npart def Pack {}\n");
        let env = br#"{"v":1,"elements":{},
            "references":[{"from_span":[28,35],"ref_kind":"specializes","resolved_path":"vehicle.Battery"}]}"#;
        idx.update_from_envelope(&url, env, &rope);
        assert_eq!(idx.references_of("vehicle.Battery", false).len(), 1);
        // Refresh with no references → the site is evicted, bucket removed.
        let env2 = br#"{"v":1,"elements":{},"references":[]}"#;
        idx.refresh_file(&url, env2, &rope);
        assert!(idx.references_of("vehicle.Battery", false).is_empty());
    }

    #[test]
    fn refresh_replaces_only_this_file() {
        let url_a = Url::parse("file:///a.deal").unwrap();
        let url_b = Url::parse("file:///b.deal").unwrap();
        let rope = Rope::from_str("part def X { }");
        let env_a = br#"{"v":1,"elements":{"pkg.X":{"id":"pkg.X","kind":"part_def","source_file":"a","span":[0,14]}}}"#;
        let env_b = br#"{"v":1,"elements":{"pkg.Y":{"id":"pkg.Y","kind":"part_def","source_file":"b","span":[0,14]}}}"#;
        let idx = Index::new();
        idx.update_from_envelope(&url_a, env_a, &rope);
        idx.update_from_envelope(&url_b, env_b, &rope);
        assert_eq!(idx.len(), 2);
        // Re-parse a.deal with a different symbol → pkg.X drops, pkg.Z appears,
        // pkg.Y from b.deal is untouched.
        let env_a2 = br#"{"v":1,"elements":{"pkg.Z":{"id":"pkg.Z","kind":"part_def","source_file":"a","span":[0,14]}}}"#;
        idx.refresh_file(&url_a, env_a2, &rope);
        assert_eq!(idx.len(), 2);
        assert!(
            idx.lookup("pkg.X").is_none(),
            "stale pkg.X should be evicted"
        );
        assert!(
            idx.lookup("pkg.Y").is_some(),
            "untouched pkg.Y must survive"
        );
        assert!(idx.lookup("pkg.Z").is_some(), "fresh pkg.Z must be present");
    }

    #[test]
    fn alias_prefix_expansion() {
        let mut aliases = HashMap::new();
        aliases.insert("veh".to_string(), "packages.vehicle".to_string());
        let idx = Index::with_aliases(aliases);
        assert_eq!(
            idx.resolve_with_alias("veh.BatteryPack"),
            "packages.vehicle.BatteryPack"
        );
        // Exact alias hit (no `.` suffix).
        assert_eq!(idx.resolve_with_alias("veh"), "packages.vehicle");
        // Non-aliased prefix passes through.
        assert_eq!(idx.resolve_with_alias("foo.Bar"), "foo.Bar");
    }

    #[test]
    fn lookup_uses_alias_resolution() {
        let mut aliases = HashMap::new();
        aliases.insert("veh".to_string(), "vehicle.battery".to_string());
        let idx = Index::with_aliases(aliases);
        let rope = Rope::from_str("part def BatteryPack { }");
        let env = br#"{"v":1,"elements":{"vehicle.battery.BatteryPack":{"id":"x","kind":"part_def","source_file":"x","span":[0,24]}}}"#;
        idx.update_from_envelope(&url(), env, &rope);
        // Aliased lookup expands then resolves.
        assert!(idx.lookup("veh.BatteryPack").is_some());
        assert!(idx.lookup("vehicle.battery.BatteryPack").is_some());
    }

    #[test]
    fn workspace_symbols_filters_and_maps_kind() {
        use tower_lsp::lsp_types::SymbolKind;
        let idx = Index::new();
        let rope = Rope::from_str("part def X {}");
        let env = br#"{"v":1,"elements":{
            "vehicle.battery.BatteryPack":{"id":"x","kind":"part_def","source_file":"a","span":[0,12]},
            "vehicle.battery.Soc":{"id":"x","kind":"attribute_def","source_file":"a","span":[0,12]},
            "vehicle.motor.Drive":{"id":"x","kind":"action_def","source_file":"a","span":[0,12]}
        }}"#;
        idx.update_from_envelope(&url(), env, &rope);

        // Empty query → all three, ordered by container then name.
        let all = idx.workspace_symbols("");
        assert_eq!(all.len(), 3);

        // Substring query is case-insensitive against the full path.
        let battery = idx.workspace_symbols("batterypack");
        assert_eq!(battery.len(), 1);
        assert_eq!(battery[0].name, "BatteryPack");
        assert_eq!(battery[0].kind, SymbolKind::CLASS);
        assert_eq!(
            battery[0].container_name.as_deref(),
            Some("vehicle.battery")
        );

        // Kind mapping for non-part definitions.
        let soc = idx.workspace_symbols("Soc");
        assert_eq!(soc[0].kind, SymbolKind::FIELD);
        let drive = idx.workspace_symbols("Drive");
        assert_eq!(drive[0].kind, SymbolKind::METHOD);
    }

    #[test]
    fn workspace_symbols_evicted_with_file() {
        let idx = Index::new();
        let rope = Rope::from_str("part def X {}");
        let env = br#"{"v":1,"elements":{"pkg.X":{"id":"x","kind":"part_def","source_file":"a","span":[0,12]}}}"#;
        idx.update_from_envelope(&url(), env, &rope);
        assert_eq!(idx.workspace_symbols("").len(), 1);
        idx.remove_file(&url());
        assert_eq!(
            idx.workspace_symbols("").len(),
            0,
            "kinds map must evict too"
        );
    }

    #[test]
    fn byte_to_position_clamps() {
        let rope = Rope::from_str("hi");
        let p = byte_to_position(&rope, 999);
        assert_eq!(p.line, 0);
        assert_eq!(p.character, 2);
    }

    #[test]
    fn byte_to_position_utf16_multibyte() {
        // "α" = 2 bytes UTF-8 / 1 UTF-16 code unit.
        // "𝄞" = 4 bytes UTF-8 / 2 UTF-16 code units (surrogate pair).
        let rope = Rope::from_str("α𝄞x");
        let p = byte_to_position(&rope, 6);
        assert_eq!(p.character, 3, "α(1) + 𝄞(2) = 3 UTF-16 units before x");
    }

    #[test]
    fn map_symbol_kind_covers_p1_added_kinds() {
        use tower_lsp::lsp_types::SymbolKind;
        // P1: every element-def kind must resolve to a non-OBJECT icon.
        assert_eq!(map_symbol_kind("calc_def"), SymbolKind::FUNCTION);
        assert_eq!(map_symbol_kind("need_def"), SymbolKind::PROPERTY);
        assert_eq!(map_symbol_kind("allocation_def"), SymbolKind::EVENT);
        assert_eq!(map_symbol_kind("use_case_def"), SymbolKind::CLASS);
        assert_eq!(map_symbol_kind("actor_def"), SymbolKind::CONSTANT);
        // Sanity: genuinely unknown kinds still fall back to OBJECT.
        assert_eq!(map_symbol_kind("nonsense_kind"), SymbolKind::OBJECT);
    }
}
