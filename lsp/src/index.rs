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
}

#[derive(Debug, Deserialize)]
struct ElementEntry {
    /// `[start_byte, end_byte]` half-open UTF-8 byte span.
    span: [usize; 2],
    /// Element kind from the envelope (e.g. "part_def", "requirement_def").
    /// Defaults to empty when absent so older envelopes still deserialize.
    #[serde(default)]
    kind: String,
}

impl Index {
    pub fn new() -> Self {
        Self {
            symbols: DashMap::new(),
            kinds: DashMap::new(),
            aliases: RwLock::new(Arc::new(HashMap::new())),
        }
    }

    pub fn with_aliases(aliases: HashMap<String, String>) -> Self {
        Self {
            symbols: DashMap::new(),
            kinds: DashMap::new(),
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
            self.symbols
                .insert(path, (uri.clone(), Range { start, end }));
        }
    }

    /// Remove every entry whose value points at `uri` (called before
    /// re-inserting from a fresh re-parse).
    pub fn remove_file(&self, uri: &Url) {
        self.symbols.retain(|_, v| &v.0 != uri);
        self.kinds.retain(|_, v| &v.0 != uri);
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
fn map_symbol_kind(kind: &str) -> SymbolKind {
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
