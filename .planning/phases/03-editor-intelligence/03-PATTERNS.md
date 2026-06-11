# Phase 3: Editor Intelligence — Pattern Map

**Mapped:** 2026-05-22
**Files analyzed:** 7 plans × ~5–12 files each (≈ 50 new files + 4 modified)
**Analogs found:** 7 / 7 plans have at least one strong in-repo analog; binary-distribution and TextMate grammars rely partly on external (rust-analyzer / textmate.org) analogs cited in RESEARCH.md.

---

## File Classification

Files are grouped by plan (per RESEARCH.md §16). All paths are absolute. Repository roots involved:
- `/Users/dunnock/projects/deal-lang/deal/` — Zig core + Rust workspace + `.planning/`
- `/Users/dunnock/projects/deal-lang/vscode-deal/` — VS Code extension scaffold
- `/Users/dunnock/projects/deal-lang/tree-sitter-deal/` — tree-sitter package scaffold
- `/Users/dunnock/projects/deal-lang/lsp/` — STALE TypeScript scaffold (Phase 3 closeout retires or rewrites this — see Plan 07)

### Plan 01 — TextMate + VS Code scaffold (Wave 1)

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `/Users/dunnock/projects/deal-lang/vscode-deal/package.json` | config (extension manifest) | request-response (activation events) | RESEARCH.md §5 canonical shape (external) — no in-repo analog (vscode-deal is scaffold-only) | external |
| `/Users/dunnock/projects/deal-lang/vscode-deal/language-configuration.json` | config | static declaration | RESEARCH.md §6 + VS Code language-configuration docs (external) | external |
| `/Users/dunnock/projects/deal-lang/vscode-deal/syntaxes/deal.tmLanguage.json` | config (grammar) | transform (regex → scopes) | `/Users/dunnock/projects/deal-lang/deal/include/deal.h` D-16 error-code categories (only in-repo signal for scope sets); RESEARCH.md §6 mapping table (authoritative source) | role-match (external mapping is authoritative) |
| `/Users/dunnock/projects/deal-lang/vscode-deal/syntaxes/dealx.tmLanguage.json` | config (grammar) | transform | sibling of `deal.tmLanguage.json` — shares 80% rules + adds `[< >]` composition tag scopes (RESEARCH.md §6 row "Composition tag") | sibling |
| `/Users/dunnock/projects/deal-lang/vscode-deal/snippets/deal.json` | config (data) | static lookup | RESEARCH.md §14 table (19 snippets) | external |
| `/Users/dunnock/projects/deal-lang/vscode-deal/snippets/dealx.json` | config (data) | static lookup | RESEARCH.md §14 table — composition-tag snippets only | external |
| `/Users/dunnock/projects/deal-lang/vscode-deal/icons/deal-light.svg` (+ dark) | asset | static | none in-repo — planner originates per CONTEXT.md "Claude's Discretion" file-icon section | none |
| `/Users/dunnock/projects/deal-lang/vscode-deal/icons/dealx-light.svg` (+ dark) | asset | static | sibling of `deal-light.svg` | sibling |
| `/Users/dunnock/projects/deal-lang/vscode-deal/COLOR-CATEGORIES.md` | docs | static | RESEARCH.md §8 table (canonical source) | external |
| `/Users/dunnock/projects/deal-lang/vscode-deal/.gitignore`, `tsconfig.json`, `.vscodeignore` | config | static | RESEARCH.md §5 (typical VS Code extension layout) | external |

### Plan 02 — tree-sitter-deal grammar (Wave 1)

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `/Users/dunnock/projects/deal-lang/tree-sitter-deal/grammar.js` | grammar | transform (source → AST) | RESEARCH.md §3 outline; in-repo: `/Users/dunnock/projects/deal-lang/spec/grammar/lexical.ebnf` + `deal.ebnf` + `dealx.ebnf` are the productive truth | external + role-match (in-repo EBNF is the source-of-truth that tree-sitter mirrors) |
| `/Users/dunnock/projects/deal-lang/tree-sitter-deal/queries/highlights.scm` | grammar query | transform | RESEARCH.md §3 + §8 capture column | external |
| `/Users/dunnock/projects/deal-lang/tree-sitter-deal/queries/injections.scm` | grammar query | transform | RESEARCH.md §3 (empty / minimal) | external |
| `/Users/dunnock/projects/deal-lang/tree-sitter-deal/queries/indents.scm` | grammar query | transform | RESEARCH.md §3 indents.scm sample | external |
| `/Users/dunnock/projects/deal-lang/tree-sitter-deal/test/corpus/*.txt` | test fixtures | input → expected-tree | in-repo: `/Users/dunnock/projects/deal-lang/spec/examples/showcase/packages/vehicle/battery.deal` (and 18 siblings) are corpus seeds | role-match |
| `/Users/dunnock/projects/deal-lang/tree-sitter-deal/package.json` | config | static | RESEARCH.md §3 + `tree-sitter-cli` docs | external |
| `/Users/dunnock/projects/deal-lang/tree-sitter-deal/.gitignore` | config | static | sibling of vscode-deal/.gitignore | sibling |

### Plan 03 — deal-lsp scaffold + diagnostics + formatting (Wave 2)

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `/Users/dunnock/projects/deal-lang/deal/Cargo.toml` (MODIFIED) | config (workspace root) | static | self — current state already declares `[workspace] members = ["cli", "tests/ffi"]` (lines 1–25) | exact |
| `/Users/dunnock/projects/deal-lang/deal/deal-ffi/Cargo.toml` (NEW) | config | static | `/Users/dunnock/projects/deal-lang/deal/cli/Cargo.toml` (sibling crate, `links = "deal"` ownership) | exact (workspace sibling) |
| `/Users/dunnock/projects/deal-lang/deal/deal-ffi/build.rs` (NEW) | build script | side-effect (zig build + link search emission) | `/Users/dunnock/projects/deal-lang/deal/cli/build.rs` (52 lines) — VERBATIM with `.join("..")` count adjustment per crate depth | exact |
| `/Users/dunnock/projects/deal-lang/deal/deal-ffi/src/lib.rs` (NEW) | library / FFI wrapper | request-response (Rust ↔ C ABI) | `/Users/dunnock/projects/deal-lang/deal/cli/src/ffi.rs` (109 lines, all 9 extern blocks) — MOVE wholesale, then wrap each `extern` in safe Rust wrappers | exact |
| `/Users/dunnock/projects/deal-lang/deal/cli/Cargo.toml` (MODIFIED) | config | static | self — drop `links = "deal"` (lines 7–10); add `deal-ffi = { path = "../deal-ffi" }` to `[dependencies]` | self |
| `/Users/dunnock/projects/deal-lang/deal/cli/src/ffi.rs` (DELETED) | — | — | move contents to `deal-ffi/src/lib.rs` | self |
| `/Users/dunnock/projects/deal-lang/deal/cli/src/main.rs` (MODIFIED) | controller | request-response | self — replace `mod ffi;` (line 17) with `use deal_ffi as ffi;` (or analogous re-export); call-site signatures unchanged | self |
| `/Users/dunnock/projects/deal-lang/deal/lsp/Cargo.toml` (NEW) | config | static | `/Users/dunnock/projects/deal-lang/deal/cli/Cargo.toml` + RESEARCH.md §10 manifest | role-match |
| `/Users/dunnock/projects/deal-lang/deal/lsp/src/main.rs` (NEW) | controller (server entrypoint) | event-driven (LSP stdio loop) | RESEARCH.md §1 `tokio::main` snippet; in-repo loosest analog: `/Users/dunnock/projects/deal-lang/deal/cli/src/main.rs` (clap entry → handler functions) | role-match (transport differs: clap vs LSP) |
| `/Users/dunnock/projects/deal-lang/deal/lsp/src/backend.rs` (NEW) | service (`LanguageServer` impl) | event-driven | RESEARCH.md §1 `Backend` trait impl | external |
| `/Users/dunnock/projects/deal-lang/deal/lsp/src/documents.rs` (NEW) | model (per-document `DealHandle*` table) | CRUD (didOpen / didChange / didClose) | in-repo: parse + free pattern in `/Users/dunnock/projects/deal-lang/deal/cli/src/main.rs` lines 173–227 (parse → diagnostics_json → index_json → free) | role-match (re-used per-document) |
| `/Users/dunnock/projects/deal-lang/deal/lsp/src/diagnostics.rs` (NEW) | service | transform (Zig diagnostic JSON → LSP Diagnostic) | in-repo: `cli/src/main.rs` lines 230–244 (diagnostic JSON parse + array-of-objects iteration); D-32 envelope reuse (RESEARCH.md §11) | role-match |
| `/Users/dunnock/projects/deal-lang/deal/lsp/src/formatting.rs` (NEW) | service | transform (in-memory bytes → TextEdit) | in-repo: `cli/src/main.rs` lines 480–565 (`fn invoke_formatter` — parse → has_errors → deal_format → clone bytes BEFORE deal_free) | exact (formatter call sequence is verbatim) |
| `/Users/dunnock/projects/deal-lang/deal/lsp/tests/showcase.rs` (NEW) | test | input → assertion | in-repo: `/Users/dunnock/projects/deal-lang/deal/tests/ffi/tests/parse.rs` lines 1–90 (showcase-driven `tests/showcase/...` harness) | exact |
| `/Users/dunnock/projects/deal-lang/deal/lsp/README.md` (NEW) | docs | static | RESEARCH.md §10 + the (STALE) `/Users/dunnock/projects/deal-lang/lsp/README.md` (DO NOT use the stale TS design; reuse only the capability list) | mixed |

### Plan 04 — deal-lsp completion + hover + definition + semantic tokens (Wave 3)

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `/Users/dunnock/projects/deal-lang/deal/lsp/src/workspace.rs` (NEW) | service / store | CRUD (in-memory `HashMap<PathString, (FileUri, Range)>`) | in-repo: `cli/src/main.rs` lines 215–223 (`deal_index_json` per-file → push to `per_file_indexes` Vec) — merge-pattern is the same | role-match |
| `/Users/dunnock/projects/deal-lang/deal/lsp/src/index.rs` (NEW) | service (path-string resolver) | transform (per-file IR JSON → global map) | in-repo: `cli/src/main.rs` index merge logic around line 220+ | role-match |
| `/Users/dunnock/projects/deal-lang/deal/lsp/src/completion.rs` (NEW) | service | request-response | RESEARCH.md §7 token-type mapping + workspace HashMap lookup; in-repo: none (new capability) | external |
| `/Users/dunnock/projects/deal-lang/deal/lsp/src/hover.rs` (NEW) | service | request-response | RESEARCH.md §1 (`hover` trait method); in-repo signal: LM-2 doc-comment attachment field in `include/deal.h` §"comment attachment fields on ElementDef / ElementUsage" (line 380) — hover renders these | role-match (in-repo data model defined; rendering new) |
| `/Users/dunnock/projects/deal-lang/deal/lsp/src/definition.rs` (NEW) | service | request-response | RESEARCH.md §1; D-23 fully-qualified path-string IDs (CONTEXT.md line 43) — HashMap lookup is O(1) | external |
| `/Users/dunnock/projects/deal-lang/deal/lsp/src/semantic_tokens.rs` (NEW) | service | transform (AST → token deltas) | RESEARCH.md §7 mapping table; in-repo: `cli/src/main.rs` lines 644–700 (`deal_ast_json` walk pattern) | role-match |
| `/Users/dunnock/projects/deal-lang/deal/lsp/tests/showcase.rs` (MODIFIED — extends Plan 03 file) | test | input → assertion | self — add `completion_returns_element_keywords`, `definition_lookup_specializes`, `hover_renders_jsdoc`, `semantic_tokens_match_golden`, `workspace_index_populated_after_initialize` cases following Plan 03's harness shape | self |

### Plan 05 — vscode-deal LSP wiring (Wave 3)

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `/Users/dunnock/projects/deal-lang/vscode-deal/src/extension.ts` (NEW) | controller (activation entrypoint) | event-driven | RESEARCH.md §5 `activate()` + `deactivate()` snippet | external |
| `/Users/dunnock/projects/deal-lang/vscode-deal/src/client.ts` (NEW) | service (`LanguageClient` wrapper) | request-response (stdio) | RESEARCH.md §5 `LanguageClient` setup block | external |
| `/Users/dunnock/projects/deal-lang/vscode-deal/src/status_bar.ts` (NEW) | component (status-bar item) | event-driven (`onDidChangeState`) | RESEARCH.md §5 `statusBar(client)` snippet — D-40 state machine | external |
| `/Users/dunnock/projects/deal-lang/vscode-deal/src/commands.ts` (NEW) | controller (command palette) | event-driven | RESEARCH.md §5 `commands` contribution (lines 503–506); VS Code Command API | external |
| `/Users/dunnock/projects/deal-lang/vscode-deal/src/test/suite/activation.test.ts` (NEW) | test | event-driven | RESEARCH.md §5 `@vscode/test-electron` block | external |
| `/Users/dunnock/projects/deal-lang/vscode-deal/src/test/runTest.ts` (NEW) | test harness | side-effect | `@vscode/test-electron` README boilerplate (cited RESEARCH.md §5) | external |

### Plan 06 — Binary distribution + release pipeline (Wave 4)

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `/Users/dunnock/projects/deal-lang/deal/.github/workflows/release.yml` (NEW) | config (CI workflow) | event-driven (tag push) | RESEARCH.md §12 5-job pipeline (verbatim) — no in-repo workflow analog (no existing `.github/workflows/` content in deal/) | external |
| `/Users/dunnock/projects/deal-lang/vscode-deal/src/bootstrap.ts` (NEW) | service (auto-download) | request-response (HTTP + filesystem) | RESEARCH.md §4 rust-analyzer `bootstrap.ts` paraphrase (50 lines verbatim adaptable) | external (rust-analyzer is the pattern source) |
| `/Users/dunnock/projects/deal-lang/vscode-deal/src/sha256.ts` (NEW) | utility | transform (bytes → hex) | RESEARCH.md §4 `sha256()` helper call site (line 418) — Node `crypto.createHash` | external |
| `/Users/dunnock/projects/deal-lang/vscode-deal/src/test/auto_download.test.ts` (NEW) | test | side-effect + assertion | sibling of `activation.test.ts` (Plan 05) | sibling |
| `/Users/dunnock/projects/deal-lang/deal/SHA256SUMS.template` (NEW, optional) | data | static | RESEARCH.md §12 Job 4 (`sha256sum deal-lsp*`) | external |

### Plan 07 — Phase 3 gate (Wave 5)

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `/Users/dunnock/projects/deal-lang/deal/build.zig` (MODIFIED) | build script | side-effect (step graph) | self — `phase_2_gate` + `phase_2_gate_fresh` blocks at lines 264–287 (verbatim mirror) | exact |
| `/Users/dunnock/projects/deal-lang/deal/scripts/phase-3-smoke.sh` (NEW) | utility (integration smoke) | side-effect | RESEARCH.md §13; in-repo signal: `/Users/dunnock/projects/deal-lang/deal/scripts/verify-fresh-worktree.sh` for argv conventions + `set -euo pipefail` style | role-match |
| `/Users/dunnock/projects/deal-lang/deal/lsp-smoke/` crate (NEW, optional dev-binary) | test harness binary | event-driven | `/Users/dunnock/projects/deal-lang/deal/tests/ffi/tests/parse.rs` (showcase walker) | role-match |
| `/Users/dunnock/projects/deal-lang/deal/scripts/verify-fresh-worktree.sh` (NO CHANGE — already generic per Plan 02-06) | — | — | self (no changes; gate-step argument plumbing is already generalized) | n/a |
| `/Users/dunnock/projects/deal-lang/deal/.planning/REQUIREMENTS.md` (MODIFIED) | docs | static | self — update DEFER-textmate-vs-treesitter-vscode entry to resolved citing D-38 (CONTEXT.md §specifics line 176) | self |
| `/Users/dunnock/projects/deal-lang/deal/.planning/STATE.md` (MODIFIED) | docs | static | self — mark Phase 3 complete | self |
| `/Users/dunnock/projects/deal-lang/deal/.planning/ROADMAP.md` (MODIFIED) | docs | static | self | self |
| `/Users/dunnock/projects/deal-lang/lsp/README.md` (DELETED or REWRITTEN) | docs | static | RESEARCH.md §16 Plan 07; CONTEXT.md `<deferred>` last bullet (line 200) — STALE TypeScript design retired | self |

---

## Pattern Assignments

### Plan 03 / `/Users/dunnock/projects/deal-lang/deal/deal-ffi/` crate (NEW)

**Analog:** `/Users/dunnock/projects/deal-lang/deal/cli/` crate. Three files transfer almost verbatim with one mechanical adjustment (depth in `.join("..")` chain).

**`deal-ffi/Cargo.toml` shape** — Pattern from `cli/Cargo.toml` lines 1–14:

```toml
# /Users/dunnock/projects/deal-lang/deal/cli/Cargo.toml lines 1–14 (PATTERN)
[package]
name = "deal"
version = "0.1.0"
edition = "2021"
publish = false
# `links` declares that this crate links against the native library `deal`
# (i.e. libdeal.a). Per PATTERNS Lane 8 option (c): cli/ claims the native-lib
# link; tests/ffi/ drops it (the `links` field was removed from
# tests/ffi/Cargo.toml so that exactly ONE crate in the workspace claims "deal").
links = "deal"

[[bin]]
name = "deal"
path = "src/main.rs"
```

**Adaptation for `deal-ffi/Cargo.toml`:**
- Rename `name = "deal"` → `name = "deal-ffi"`, `version = "0.3.0"`.
- **MOVE the `links = "deal"` claim from `cli/Cargo.toml` TO `deal-ffi/Cargo.toml`** so deal-ffi is the single native-lib claimant (Cargo enforces single-claim rule across the workspace). Then drop `links` from `cli/Cargo.toml` AND continue to keep it absent from `tests/ffi/Cargo.toml` (already absent per existing PATTERNS Lane 8 option (c) comment).
- Replace `[[bin]]` block with `[lib] name = "deal_ffi" crate-type = ["rlib"]`.
- Add `[build-dependencies] bindgen = { workspace = true }` and a build.rs that runs bindgen (or copy the entire hand-written extern block as-is — `bindgen` is optional; the existing 109-line file is already hand-written and works).
- **Recommended**: skip bindgen (RESEARCH §10 A7 risk note); move hand-written `cli/src/ffi.rs` (109 lines) to `deal-ffi/src/lib.rs` verbatim and add the safe `OwnedDealHandle` newtype wrapper from D-45.

**`deal-ffi/build.rs` shape** — Pattern from `cli/build.rs` lines 20–52:

```rust
// /Users/dunnock/projects/deal-lang/deal/cli/build.rs lines 20–52 (PATTERN)
fn main() {
    // cli/ is one level below deal/.
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR unset");
    let deal_dir = PathBuf::from(&manifest_dir)
        .join("..")
        .canonicalize()
        .expect("failed to canonicalize deal/ directory");

    // 1. Build libdeal.a via the Zig build system.
    let status = Command::new("zig")
        .arg("build")
        .current_dir(&deal_dir)
        .status()
        .expect("failed to invoke `zig build` — is zig 0.16.0 on PATH?");
    assert!(status.success(), "`zig build` failed in {}", deal_dir.display());

    // 2. Tell rustc / lld where to find the static library.
    let lib_dir = deal_dir.join("zig-out").join("lib");
    println!("cargo::rustc-link-search=native={}", lib_dir.display());

    // 3. Link libdeal.a statically.
    println!("cargo::rustc-link-lib=static=deal");

    // 4. Re-run if any Zig-side source changes.
    let src_dir = deal_dir.join("src");
    let include_dir = deal_dir.join("include");
    let build_zig = deal_dir.join("build.zig");
    let build_zon = deal_dir.join("build.zig.zon");
    println!("cargo::rerun-if-changed={}", src_dir.display());
    println!("cargo::rerun-if-changed={}", include_dir.display());
    println!("cargo::rerun-if-changed={}", build_zig.display());
    println!("cargo::rerun-if-changed={}", build_zon.display());
}
```

**Adaptation:** verbatim copy. `deal-ffi/` lives at the same depth as `cli/` (one level below `deal/`), so the single `.join("..")` is correct unchanged. Compare to `tests/ffi/build.rs` lines 18–23, which uses `.join("..").join("..")` — that's because `tests/ffi/` is two levels deep. PATTERNS Lane 4 comment in `cli/build.rs` line 14 documents this distinction.

**`deal-ffi/src/lib.rs` shape** — Pattern from `cli/src/ffi.rs` lines 14–108 (VERBATIM):

```rust
// /Users/dunnock/projects/deal-lang/deal/cli/src/ffi.rs lines 14–46 (PATTERN excerpt)
#![allow(non_camel_case_types)]

use std::marker::{PhantomData, PhantomPinned};

/// Opaque handle returned by `deal_parse`. The handle owns the arena that
/// holds the parsed AST. Per D-13: single-threaded; marked !Send and !Sync.
#[repr(C)]
pub struct DealHandle {
    _opaque: [u8; 0],
    // Mark !Send / !Sync — per-handle thread affinity per D-13.
    _marker: PhantomData<(*mut u8, PhantomPinned)>,
}

extern "C" {
    /// Parse `source` bytes (UTF-8, length-prefixed) with the given filename.
    /// Returns an opaque handle owning the resulting AST + diagnostics.
    /// Caller must eventually call `deal_free` on the returned handle.
    pub fn deal_parse(
        source_ptr: *const u8,
        source_len: usize,
        filename_ptr: *const u8,
        filename_len: usize,
    ) -> *mut DealHandle;
    // ... (deal_free, deal_has_errors, deal_diagnostics_count, deal_ast_json,
    //      deal_diagnostics_json, deal_ir_json, deal_format, deal_index_json)
}
```

**Adaptations required:**
1. Move file verbatim from `cli/src/ffi.rs` → `deal-ffi/src/lib.rs`. Note the `lib.rs` rename (was a `mod ffi` in cli; now top-level lib root).
2. **Add D-45 `OwnedDealHandle` newtype** (CONTEXT.md line 39):
   ```rust
   // NEW (Phase 3 — D-45 concurrency wrapper):
   pub struct OwnedDealHandle(*mut DealHandle);
   unsafe impl Send for OwnedDealHandle {}  // each handle owns an independent arena (D-02, D-13)
   impl Drop for OwnedDealHandle {
       fn drop(&mut self) { unsafe { deal_free(self.0); } }
   }
   ```
   Note the `PhantomData<*mut u8, PhantomPinned>` already marks the inner struct `!Send`; the newtype overrides this for arena-isolated single-thread-per-handle access per D-13/D-45.
3. **Add safe Rust wrappers** around each `extern "C"` (parse_bytes → returns `Option<OwnedDealHandle>`; diagnostics_json → returns `Vec<u8>` cloned from arena before drop; etc.). The current `cli/src/main.rs` lines 173–227, 480–565 demonstrate the clone-before-free pattern (Pitfall 3 / T-02-29) — encapsulate that pattern inside `deal-ffi`'s safe wrappers so call-sites in `lsp/` and `cli/` don't have to re-implement it.
4. Update `cli/Cargo.toml`: remove `links = "deal"`, add `deal-ffi = { path = "../deal-ffi" }` to `[dependencies]`.
5. Update `cli/src/main.rs` line 17: change `pub mod ffi;` to `use deal_ffi as ffi;` (re-export keeps all 1500+ `ffi::deal_*` call-sites working unchanged).
6. **DELETE `cli/src/ffi.rs`** after move.

---

### Plan 03 / `/Users/dunnock/projects/deal-lang/deal/lsp/Cargo.toml` (NEW)

**Analog:** `/Users/dunnock/projects/deal-lang/deal/cli/Cargo.toml` for crate shape; RESEARCH.md §10 for dependency list.

**Pattern shape** — sibling crate manifest, no `links` claim (deal-ffi owns it):

```toml
# /Users/dunnock/projects/deal-lang/deal/cli/Cargo.toml lines 16–31 (PATTERN — [dependencies])
[dependencies]
clap = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
anstream = { workspace = true }
owo-colors = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
jsonschema = { workspace = true }
uuid = { workspace = true }
sha2 = "0.10"
hex = "0.4"

[dev-dependencies]
insta = { workspace = true }
```

**Adaptations required:**
- Replace `name = "deal"`, `[[bin]] name = "deal"`, `path = "src/main.rs"` with `name = "deal-lsp"`, `[[bin]] name = "deal-lsp"`.
- Drop `clap`, `anstream`, `owo-colors`, `jsonschema`, `uuid`, `sha2`, `hex` (CLI-only deps).
- Add per RESEARCH.md §10:
  ```toml
  deal-ffi    = { path = "../deal-ffi" }
  tower-lsp   = { workspace = true }
  tokio       = { workspace = true }
  tower       = { workspace = true }
  dashmap     = { workspace = true }
  ropey       = { workspace = true }
  serde       = { workspace = true }
  serde_json  = { workspace = true }
  anyhow      = { workspace = true }
  thiserror   = { workspace = true }
  tracing             = "0.1"
  tracing-subscriber  = { version = "0.3", features = ["env-filter"] }
  ```
- **Do NOT declare `links = "deal"`** (deal-ffi is the single workspace claimant per Cargo's single-claim rule).
- Add to workspace `[workspace.dependencies]` block in `/Users/dunnock/projects/deal-lang/deal/Cargo.toml`: tower-lsp, tokio, tower, dashmap, ropey, bindgen (only if bindgen path chosen — RESEARCH §10 A7).

---

### Plan 03 / `/Users/dunnock/projects/deal-lang/deal/Cargo.toml` (MODIFIED)

**Analog:** self. Current workspace declares 2 members; Phase 3 adds 2 more.

**Pattern from current file** (verbatim — current state):

```toml
# /Users/dunnock/projects/deal-lang/deal/Cargo.toml lines 1–25 (CURRENT STATE)
[workspace]
resolver = "2"
members = ["cli", "tests/ffi"]

[workspace.dependencies]
# CLI argument parsing + subcommands + help generation
clap = { version = "4.6", features = ["derive"] }
# Serialization framework
serde = { version = "1", features = ["derive"] }
# JSON parse/emit — NO preserve_order feature (D-18 alphabetical-key invariant)
serde_json = "1"
# Offline JSON Schema draft-2020-12 validation (default-features = false for offline-only)
jsonschema = { version = "0.46", default-features = false }
# TTY-aware ANSI passthrough/strip
anstream = "1"
owo-colors = "4"
anyhow = "1"
thiserror = "2"
insta = { version = "1.47", features = ["json"] }
uuid = { version = "1", features = ["v5"] }
```

**Adaptations required:**
- Extend `members` to `["cli", "lsp", "deal-ffi", "tests/ffi"]`.
- Append to `[workspace.dependencies]` per RESEARCH.md §10:
  ```toml
  # NEW (Phase 3)
  tower-lsp   = { version = "0.20", features = ["runtime-tokio"] }   # [ASSUMED — verify via `cargo search tower-lsp`]
  tokio       = { version = "1", features = ["full"] }
  tower       = "0.5"
  dashmap     = "6"
  ropey       = "1.6"
  bindgen     = "0.70"   # only if bindgen route chosen (see RESEARCH §10 A7)
  ```
- Preserve all existing entries verbatim (alphabetical / by-feature ordering is comment-driven, not enforced).

---

### Plan 03 / `/Users/dunnock/projects/deal-lang/deal/lsp/src/backend.rs` (NEW)

**Analog:** None in-repo (LSP is new). The pattern source is RESEARCH.md §1 `Backend` trait impl (verbatim citation). In-repo loosest analog for the *call-site pattern* (FFI sequence: parse → diagnostic_json → free): `/Users/dunnock/projects/deal-lang/deal/cli/src/main.rs` lines 173–227.

**FFI call-site pattern** (in-repo, applies inside `did_open` / `did_change` handlers):

```rust
// /Users/dunnock/projects/deal-lang/deal/cli/src/main.rs lines 173–227 (PATTERN — parse + diagnostics + free sequence)
let handle = unsafe {
    ffi::deal_parse(
        source_bytes.as_ptr(),
        source_bytes.len(),
        filename.as_bytes().as_ptr(),
        filename.len(),
    )
};

if handle.is_null() {
    return Err(CliError::Internal(anyhow::anyhow!(
        "deal_parse returned null for {:?} (OOM)",
        path
    )));
}

// T-02-13 mitigation (Pitfall 3): clone diagnostic JSON bytes BEFORE
// calling deal_free. The arena-owned pointer is invalidated by deal_free.
let diag_json_owned: Vec<u8>;
let has_errors: bool;

unsafe {
    has_errors = ffi::deal_has_errors(handle);

    let mut out_ptr: *const u8 = std::ptr::null();
    let mut out_len: usize = 0;
    let ok = ffi::deal_diagnostics_json(handle, &mut out_ptr, &mut out_len);

    if !ok { ffi::deal_free(handle); return Err(/* ... */); }
    diag_json_owned = std::slice::from_raw_parts(out_ptr, out_len).to_vec();

    let mut idx_ptr: *const u8 = std::ptr::null();
    let mut idx_len: usize = 0;
    if ffi::deal_index_json(handle, &mut idx_ptr, &mut idx_len) {
        let bytes = std::slice::from_raw_parts(idx_ptr, idx_len).to_vec();
        per_file_indexes.push(bytes);
    }

    ffi::deal_free(handle);
}
```

**LSP `Backend` trait skeleton** (from RESEARCH.md §1 lines 107–153, verbatim adaptation):

```rust
// RESEARCH.md §1 lines 107–153 (EXTERNAL PATTERN)
use tower_lsp::{LanguageServer, LspService, Server, jsonrpc::Result};
use tower_lsp::lsp_types::*;

#[async_trait::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "deal-lsp".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".into(), ":".into(), "<".into()]),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                semantic_tokens_provider: Some(/* ... */),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
        })
    }
    async fn did_open(&self, params: DidOpenTextDocumentParams) { /* D-42 — parse + store handle */ }
    async fn did_change(&self, params: DidChangeTextDocumentParams) { /* D-43 — debounce 300ms then re-parse */ }
    async fn did_close(&self, params: DidCloseTextDocumentParams) { /* D-44 — no-op (hold for workspace lifetime) */ }
    // ... formatting / completion / hover / goto_definition / semantic_tokens
}
```

**Adaptations required:**
1. Replace `clap`-driven CLI parsing (cli/src/main.rs main entry) with `tower_lsp::LspService::new(|client| Backend::new(client))` boot per RESEARCH.md §1 lines 161–167.
2. Wrap the FFI call sequence (the 55-line `cli/src/main.rs` excerpt above) inside `did_open` / debounced `did_change` handlers; the safe wrappers in `deal-ffi` (D-45 OwnedDealHandle) will encapsulate the clone-before-free dance.
3. Store handles in a `dashmap::DashMap<Url, Arc<tokio::sync::Mutex<OwnedDealHandle>>>` per D-42, D-44, D-45.
4. Use `Vec<u8>` for the cloned JSON output, parse via `serde_json::from_slice` (same as cli/src/main.rs line 235), then convert each diagnostic record into `tower_lsp::lsp_types::Diagnostic` per D-32 envelope + D-33 E2xxx codes (CONTEXT.md lines 148–149).
5. `did_close` is a no-op per D-44 + RESEARCH.md Open Question #2 recommendation: hold for workspace lifetime; only drop on `shutdown` or FS-watch removal.

---

### Plan 03 / `/Users/dunnock/projects/deal-lang/deal/lsp/src/formatting.rs` (NEW)

**Analog:** `/Users/dunnock/projects/deal-lang/deal/cli/src/main.rs` lines 480–565 — `invoke_formatter` function (exact `deal_format` adapter pattern).

**Pattern excerpt** (in-repo verbatim):

```rust
// /Users/dunnock/projects/deal-lang/deal/cli/src/main.rs lines 482–565 (PATTERN — deal_format adapter)
let handle = unsafe {
    ffi::deal_parse(
        source_bytes.as_ptr(),
        source_bytes.len(),
        filename.as_bytes().as_ptr(),
        filename.len(),
    )
};

if handle.is_null() {
    return Err(CliError::Internal(anyhow::anyhow!(
        "deal_parse returned null for {:?} (OOM)",
        filename
    )));
}

let has_errors = unsafe { ffi::deal_has_errors(handle) };
if has_errors {
    // Clone diagnostics BEFORE deal_free (Pitfall 3).
    let diag_json_owned: Vec<u8> = unsafe { /* clone */ };
    unsafe { ffi::deal_free(handle); }
    return /* propagate diagnostics, skip formatting */;
}

// (... format the AST ...)
let formatted_owned: Vec<u8> = unsafe {
    let mut out_ptr: *const u8 = std::ptr::null();
    let mut out_len: usize = 0;
    let ok = ffi::deal_format(handle, &mut out_ptr, &mut out_len);
    if !ok {
        ffi::deal_free(handle);
        return Err(anyhow::anyhow!("deal_format failed (OOM) for {:?}", filename));
    }
    // Clone bytes BEFORE deal_free (Pitfall 3 / T-02-29).
    let bytes = std::slice::from_raw_parts(out_ptr, out_len).to_vec();
    ffi::deal_free(handle);
    bytes
};
```

**Adaptations required:**
1. **LSP holds a live handle per document already (D-42)** — DO NOT re-parse on every `formatting` request. Look up the existing `OwnedDealHandle` from the per-document map; call `deal_format` on it; clone the bytes; return as `Vec<TextEdit>` (likely one big `TextEdit` replacing the whole document range).
2. The clone-before-free invariant becomes "clone before releasing the per-document mutex" — same idea, different lifetime owner.
3. Per CONTEXT.md `<decisions>` "Format-on-save semantics with unsaved buffers" (line 66): the design leans toward in-memory (no save needed). Since `deal-lsp` already holds the in-memory buffer via D-42, this is the natural path.
4. The format output is a single `TextEdit { range: <full document range>, new_text: <formatted utf8 string> }`. Compute the full-document range from the document's stored rope (RESEARCH.md §10 lists `ropey` as a dep for this).

---

### Plan 03 / `/Users/dunnock/projects/deal-lang/deal/lsp/tests/showcase.rs` (NEW)

**Analog:** `/Users/dunnock/projects/deal-lang/deal/tests/ffi/tests/parse.rs` (lines 1–90) — showcase-driven Rust integration tests.

**Pattern excerpt** (in-repo verbatim):

```rust
// /Users/dunnock/projects/deal-lang/deal/tests/ffi/tests/parse.rs lines 60–90 (PATTERN — showcase test harness)
/// Helper: resolve path relative to the deal/ package root (the directory
/// containing Cargo.toml and tests/ffi/).
fn deal_root() -> std::path::PathBuf {
    // The manifest file is tests/ffi/Cargo.toml, so the package root is two
    // levels up from the CARGO_MANIFEST_DIR env var.
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR not set by Cargo");
    let mut p = std::path::PathBuf::from(manifest_dir);
    p.pop(); // tests/ffi -> tests
    p.pop(); // tests -> deal/
    p
}

#[test]
fn ffi_smoke() {
    let source: &[u8] = b"";
    let filename: &[u8] = b"test.deal";
    let handle = parse_into_handle(source, filename);
    assert!(!handle.is_null(), "deal_parse returned null on empty source");
    // ... (assert diagnostic count, has_errors, ast_json)
}
```

**Adaptations required:**
1. `deal_root()` helper becomes `lsp_crate_root()` — adjust `pop()` count: `lsp/tests/showcase.rs` → manifest dir `lsp/`, package root is `lsp/`, deal/ is one pop. So one `p.pop()` not two.
2. Replace bare `extern "C"` calls with the new `deal-ffi` safe wrappers (`OwnedDealHandle::parse(bytes, filename)` → `Result<OwnedDealHandle>`).
3. Spawn the LSP server in-process via `tower_lsp::LspService::new(...)` and exercise it via the in-process JSON-RPC client — OR use a separate `lsp-smoke` binary harness (RESEARCH.md §13 leaves this to the planner). Recommendation: in-process for fast unit-style coverage, separate binary only for the gate-level smoke test (Plan 07).
4. Test cases required (RESEARCH.md §16 Plan 03 acceptance):
   - `diagnostics_match_phase_2_codes` (Plan 03)
   - `format_round_trip` (Plan 03)
   - `debounce_collapses_rapid_changes` (Plan 03)
   - Add per Plan 04: `completion_returns_element_keywords`, `definition_lookup_specializes`, `hover_renders_jsdoc`, `semantic_tokens_match_golden`, `workspace_index_populated_after_initialize`.
5. Showcase symlink (`/Users/dunnock/projects/deal-lang/deal/tests/showcase` → `../spec/examples/showcase`) is already committed (Phase 1.5 Plan 05); the LSP tests open files from this path via `deal_root().join("tests/showcase/...")`.

---

### Plan 05 / `/Users/dunnock/projects/deal-lang/vscode-deal/src/extension.ts` (NEW)

**Analog:** RESEARCH.md §5 `activate()` snippet (external — code.visualstudio.com docs). No in-repo analog (vscode-deal is scaffold-only README).

**Pattern excerpt** (from RESEARCH.md §5 lines 513–547, verbatim):

```typescript
// RESEARCH.md §5 lines 513–547 (EXTERNAL PATTERN — vscode-languageclient/node setup)
import { LanguageClient, LanguageClientOptions, ServerOptions, TransportKind } from 'vscode-languageclient/node';

let client: LanguageClient;

export async function activate(context: vscode.ExtensionContext) {
  const lspPath = vscode.workspace.getConfiguration('deal').get<string>('lsp.path')
                  || await ensureDealLspBinary(context);

  const serverOptions: ServerOptions = {
    run:   { command: lspPath, args: [], transport: TransportKind.stdio },
    debug: { command: lspPath, args: ['--log=debug'], transport: TransportKind.stdio },
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: 'file', language: 'deal'  },
      { scheme: 'file', language: 'dealx' },
    ],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher('**/deal.toml'),
    },
    outputChannelName: 'DEAL Language Server',
  };

  client = new LanguageClient('deal', 'DEAL Language Server', serverOptions, clientOptions);
  await client.start();
  context.subscriptions.push(statusBar(client));
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}
```

**Status-bar pattern** (RESEARCH.md §5 lines 553–567 — D-40 silent-fallback status indicator):

```typescript
// RESEARCH.md §5 lines 553–567 (EXTERNAL PATTERN — status-bar state machine for D-40)
function statusBar(client: LanguageClient): vscode.Disposable {
  const item = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
  item.command = 'deal.showOutput';

  client.onDidChangeState(e => {
    switch (e.newState) {
      case 1 /*Stopped*/:  item.text = '$(error) DEAL LSP'; item.tooltip = 'DEAL LSP error — click for output'; break;
      case 2 /*Starting*/: item.text = '$(sync~spin) DEAL LSP'; item.tooltip = 'DEAL LSP starting…'; break;
      case 3 /*Running*/:  item.text = '$(check) DEAL LSP'; item.tooltip = 'DEAL LSP ready'; break;
    }
    item.show();
  });

  return item;
}
```

**Adaptations required:**
1. `ensureDealLspBinary(context)` is implemented in Plan 06's `bootstrap.ts` (auto-download). For Plan 05 (parallel-safe with Plan 04), STUB this function — return either the `deal.lsp.path` config value, or the bundled `server/deal-lsp` path under `context.extensionUri` (offline `.vsix` path per D-51), or throw "deal-lsp not installed; run Plan 06 to enable auto-download." This matches RESEARCH.md §16 Plan 05 note "Stub auto-download for offline `.vsix` path".
2. Wire D-40's silent-TextMate-fallback behavior: catch `client.start()` failures and let the status bar show `$(error) DEAL LSP` rather than throwing a modal — the TextMate grammar from Plan 01 keeps rendering colors.
3. Register the two commands declared in `package.json`: `deal.restartServer` (calls `client.stop()` then `client.start()`) and `deal.showOutput` (calls `client.outputChannel.show()`).
4. Semantic-tokens registration is automatic per RESEARCH.md §5 line 549 — no extra TS code; vscode-languageclient handles it when the server declares the capability.

---

### Plan 06 / `/Users/dunnock/projects/deal-lang/vscode-deal/src/bootstrap.ts` (NEW)

**Analog:** RESEARCH.md §4 — paraphrased from `rust-analyzer/editors/code/src/bootstrap.ts`. No in-repo analog.

**Pattern excerpt** (RESEARCH.md §4 lines 380–448, verbatim):

```typescript
// RESEARCH.md §4 lines 387–448 (EXTERNAL PATTERN — rust-analyzer bootstrap)
const DEAL_LSP_VERSION = '0.3.0';  // D-52: hardcoded pin matching vscode-deal version
const SHA256_MANIFEST: Record<string, string> = {
  'darwin-arm64':   '<sha256>',
  'darwin-x64':     '<sha256>',
  'linux-x64-gnu':  '<sha256>',
  'linux-x64-musl': '<sha256>',
  'win-x64':        '<sha256>',
};

export async function ensureDealLspBinary(ctx: vscode.ExtensionContext): Promise<string> {
  const triple   = platformTriple();
  const filename = process.platform === 'win32' ? 'deal-lsp.exe' : 'deal-lsp';
  const dest     = vscode.Uri.joinPath(ctx.globalStorageUri, DEAL_LSP_VERSION, filename);

  // 1. Bundled binary present? (offline .vsix per D-51)
  const bundled = vscode.Uri.joinPath(ctx.extensionUri, 'server', filename);
  if (await pathExists(bundled)) return bundled.fsPath;

  // 2. Cached download present + checksum matches?
  if (await pathExists(dest) && await sha256(dest.fsPath) === SHA256_MANIFEST[triple]) {
    return dest.fsPath;
  }

  // 3. First-run dialog
  const proceed = await vscode.window.showInformationMessage(
    `DEAL LSP server (${DEAL_LSP_VERSION}) is not installed. Download from GitHub Releases?`,
    'Download', 'Cancel',
  );
  if (proceed !== 'Download') throw new Error('User cancelled deal-lsp download');

  // 4. Download with progress
  const url = `https://github.com/deal-lang/deal/releases/download/v${DEAL_LSP_VERSION}/deal-lsp-${triple}.tar.gz`;
  // ... withProgress + downloadAndExtract

  // 5. Verify checksum, 6. chmod +x
  if (process.platform !== 'win32') fs.chmodSync(dest.fsPath, 0o755);
  return dest.fsPath;
}
```

**Adaptations required:**
1. Generate `SHA256_MANIFEST` constants at CI time (RESEARCH.md §12 Job 4 `sha256sum`); the planner can either hand-edit per release or auto-inject via a build step.
2. **Pinned version constant `DEAL_LSP_VERSION`** must match the `version` in `vscode-deal/package.json` (D-52). Recommend a build-time check (e.g., `npm test` step verifies these strings match) OR a single constant imported from `package.json`.
3. The URL template `https://github.com/deal-lang/deal/releases/download/v{X}/deal-lsp-{triple}.tar.gz` (D-50) — confirm release tag scheme with planner (CONTEXT.md `<specifics>` line 180: keep `deal-lsp` versions in lockstep with `vscode-deal` versions).
4. **`linux-x64-musl` vs `linux-x64-gnu`**: detect at runtime via `detectLinuxLibc()` per RESEARCH.md §4 line 401 (suggestion: check for `/lib/ld-musl*` glob or `process.report.getReport().header.glibcVersionRuntime` presence). Per RESEARCH Open Question #4: ship both as separate downloads.

---

### Plan 06 / `/Users/dunnock/projects/deal-lang/deal/.github/workflows/release.yml` (NEW)

**Analog:** RESEARCH.md §12 5-job pipeline (verbatim). No in-repo analog (no existing `.github/workflows/`).

**Pattern excerpt** (RESEARCH.md §12 lines 826–931, verbatim):

```yaml
# RESEARCH.md §12 lines 826–886 (EXTERNAL PATTERN — Job 1 + Job 2 sketch)
build-libdeal:
  runs-on: ubuntu-22.04
  strategy:
    matrix:
      target:
        - aarch64-macos
        - x86_64-macos
        - x86_64-linux-gnu
        - x86_64-linux-musl
        - x86_64-windows
  steps:
    - uses: actions/checkout@v4
      with: { submodules: recursive }
    - uses: mlugg/setup-zig@v1
      with: { version: 0.16.0 }
    - run: cd deal && zig build -Doptimize=ReleaseSafe -Dtarget=${{ matrix.target }}
    - uses: actions/upload-artifact@v4
      with:
        name: libdeal-${{ matrix.target }}
        path: deal/zig-out/lib/libdeal.a

build-deal-lsp:
  needs: build-libdeal
  strategy:
    matrix:
      include:
        - { os: macos-14, triple: darwin-arm64, libdeal-target: aarch64-macos }
        - { os: macos-13, triple: darwin-x64,   libdeal-target: x86_64-macos }
        - { os: ubuntu-22.04, triple: linux-x64-gnu, libdeal-target: x86_64-linux-gnu }
        - { os: windows-2022, triple: win-x64,  libdeal-target: x86_64-windows }
  runs-on: ${{ matrix.os }}
  steps:
    - uses: actions/checkout@v4
      with: { submodules: recursive }
    - uses: actions/download-artifact@v4
      with:
        name: libdeal-${{ matrix.libdeal-target }}
        path: deal/zig-out/lib/
    - uses: dtolnay/rust-toolchain@stable
    - run: cd deal && cargo build --release -p deal-lsp
    - run: cd deal && cargo test --workspace
    - if: matrix.triple == 'linux-x64-gnu'
      run: cd deal && zig build phase-3-gate-fresh
    - uses: actions/upload-artifact@v4
      with:
        name: deal-lsp-${{ matrix.triple }}
        path: deal/target/release/deal-lsp*
```

**Adaptations required:**
1. Trigger block (omitted from §12 excerpt — planner adds):
   ```yaml
   on:
     push: { tags: ['v*.*.*'] }
     workflow_dispatch:
   ```
2. **`x86_64-linux-musl` target Zig cross-build** (D-53): verify `zig build -Dtarget=x86_64-linux-musl` works — confirmed by D-53 ("Zig 0.16.0's cross-compile is solid").
3. The musl libdeal needs to be linked against by SOME job — but RESEARCH.md §12 Job 2 matrix only lists 4 Rust targets, omitting musl. Choices:
   - (a) Add a 5th Rust target `x86_64-linux-musl` using `dtolnay/rust-toolchain` + `cargo build --target x86_64-unknown-linux-musl`. Use `ubuntu-22.04` runner.
   - (b) Skip musl-specific Rust build; ship only musl libdeal for downstream consumers who'd want it but no `deal-lsp-linux-x64-musl` binary in Phase 3.
   - Planner picks per RESEARCH Open Question #4 recommendation (ship both as separate downloads).
4. Marketplace publish (Job 5) requires `secrets.VSCE_PAT` — gate with `environment: marketplace` for manual approval per RESEARCH.md §16 Plan 06 autonomy hints.
5. The `--target` flag for per-platform `vsce package` (RESEARCH.md §12 line 908) maps VS Code's platform triples (`darwin-arm64`, `darwin-x64`, `linux-x64`, `win32-x64`) — these differ slightly from the `deal-lsp` triples; planner ensures correct mapping.

---

### Plan 07 / `/Users/dunnock/projects/deal-lang/deal/build.zig` (MODIFIED — add phase-3-gate + phase-3-gate-fresh)

**Analog:** self — `phase_2_gate` + `phase_2_gate_fresh` blocks at lines 264–287. Verbatim mirror.

**Pattern excerpt** (in-repo, lines 264–287):

```zig
// /Users/dunnock/projects/deal-lang/deal/build.zig lines 264–287 (PATTERN — Phase 2 gate pair)
const gate_2_step = b.step("phase-2-gate", "Run the full Phase 2 exit-gate Zig test suite (inherits Phase 1.5)");
gate_2_step.dependOn(gate_1_5_step); // inherits Phase 1.5 (D-35: extends, not replaces)
gate_2_step.dependOn(test_step); // runs Phase 2 Zig unit tests unfiltered

// Phase 2 fresh-worktree gate.
//
// Per ADR-phase-1.5-fresh-worktree-verification.md, every phase gate MUST
// have a -fresh sibling that runs the gate inside an EPHEMERAL worktree
// after `git submodule update --init --recursive`. This prevents the
// false-GREEN class of bug where dev-local untracked files mask failures.
//
// This step does NOT dependOn(gate_2_step) or any build step in the main
// tree — the script is self-contained and creates its own zig build
// invocation inside the ephemeral worktree.
const gate_2_fresh_step = b.step(
    "phase-2-gate-fresh",
    "Run phase-2-gate inside a freshly-created ephemeral worktree with submodule init (per ADR-phase-1.5-fresh-worktree-verification)",
);
const run_fresh_gate_2 = b.addSystemCommand(&[_][]const u8{
    "bash",
    "scripts/verify-fresh-worktree.sh",
    "phase-2-gate", // positional gate-step argument
});
gate_2_fresh_step.dependOn(&run_fresh_gate_2.step);
```

**Adaptations required** (per RESEARCH.md §13 lines 953–966):

```zig
// NEW (Phase 3 — append after phase_2_gate_fresh block):
const gate_3_step = b.step("phase-3-gate", "Run the full Phase 3 exit-gate (Zig + Rust + tree-sitter + vscode-deal)");
gate_3_step.dependOn(gate_2_step);  // inherits Phase 2 (D-35: extends, not replaces)
gate_3_step.dependOn(test_step);     // Zig core tests

// Rust workspace build + tests
const run_cargo_build = b.addSystemCommand(&[_][]const u8{
    "cargo", "build", "--workspace", "--release", "--manifest-path", "Cargo.toml",
});
gate_3_step.dependOn(&run_cargo_build.step);

const run_cargo_test = b.addSystemCommand(&[_][]const u8{
    "cargo", "test", "--workspace", "--manifest-path", "Cargo.toml",
});
gate_3_step.dependOn(&run_cargo_test.step);

// tree-sitter-deal corpus
const run_treesitter = b.addSystemCommand(&[_][]const u8{
    "bash", "-c", "cd ../tree-sitter-deal && npm ci && npm test",
});
gate_3_step.dependOn(&run_treesitter.step);

// vscode-deal extension tests (xvfb-run on Linux CI; harmless no-op on macOS/Windows)
const run_vscode_test = b.addSystemCommand(&[_][]const u8{
    "bash", "-c", "cd ../vscode-deal && npm ci && xvfb-run -a npm test",
});
gate_3_step.dependOn(&run_vscode_test.step);

const run_vscode_package = b.addSystemCommand(&[_][]const u8{
    "bash", "-c", "cd ../vscode-deal && npm run package",
});
gate_3_step.dependOn(&run_vscode_package.step);

const run_smoke = b.addSystemCommand(&[_][]const u8{
    "bash", "scripts/phase-3-smoke.sh",
});
gate_3_step.dependOn(&run_smoke.step);

// Phase 3 fresh-worktree gate — VERBATIM MIRROR of phase_2_gate_fresh.
const gate_3_fresh_step = b.step(
    "phase-3-gate-fresh",
    "Run phase-3-gate inside a freshly-created ephemeral worktree with submodule init (per ADR-phase-1.5-fresh-worktree-verification)",
);
const run_fresh_gate_3 = b.addSystemCommand(&[_][]const u8{
    "bash",
    "scripts/verify-fresh-worktree.sh",
    "phase-3-gate",
});
gate_3_fresh_step.dependOn(&run_fresh_gate_3.step);
```

**Specific adaptations:**
1. **Inherit Phase 2** via `gate_3_step.dependOn(gate_2_step)` — same pattern as `gate_2_step.dependOn(gate_1_5_step)` at line 265.
2. **The CI-AUTHORITATIVE TWO-STEP COMMAND** (per the comment block at build.zig lines 253–263) is now obsolete for Phase 3 — because `phase-3-gate` now includes `cargo build --workspace` + `cargo test --workspace`, the Phase 2 split commentary should be updated. Either: (a) Phase 3 gate is single-step `zig build phase-3-gate-fresh` (recommended per RESEARCH §13), OR (b) preserve the two-step convention for muscle memory. Recommendation: (a) — the build.zig step now drives both.
3. **No `gate_3_fresh_step.dependOn(gate_3_step)`** — fresh-gate is self-contained (matches comments at lines 275–277 for Phase 2). The script creates its own ephemeral worktree and invokes its own `zig build phase-3-gate`.
4. The `verify-fresh-worktree.sh` script (lines 1–146 in `scripts/verify-fresh-worktree.sh`) was generalized in Plan 02-06 to accept any `<gate-step>` argument; Phase 3 invocation is identical to Phase 2's at line 285 with the arg changed to `"phase-3-gate"`. **No script changes required** — confirmed by the comment at line 41: "Plan 02-06 generalized the script to accept a gate-step argument so every phase (2..6) can reuse the same script with its own gate step name."

---

### Plan 07 / `/Users/dunnock/projects/deal-lang/deal/scripts/phase-3-smoke.sh` (NEW)

**Analog:** `/Users/dunnock/projects/deal-lang/deal/scripts/verify-fresh-worktree.sh` (146 lines) — argument validation, `set -euo pipefail` style, REPO_ROOT resolution.

**Pattern excerpts** (in-repo verbatim):

```bash
# /Users/dunnock/projects/deal-lang/deal/scripts/verify-fresh-worktree.sh lines 62–84 (PATTERN — bash hygiene)
set -euo pipefail

# 0. Parse and validate the gate-step argument.
if [ -z "${1:-}" ]; then
  echo "verify-fresh-worktree: FATAL — missing required argument <gate-step>." >&2
  exit 2
fi
GATE_STEP="$1"

# 1. Resolve the deal repo root from this script's location.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
```

**Body shape from RESEARCH.md §13:**

```bash
# RESEARCH.md §13 lines 969–975 (EXTERNAL PATTERN — phase-3 smoke skeleton)
#!/usr/bin/env bash
# Spawn deal-lsp, open showcase, exercise the five capabilities.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release -p deal-lsp
cargo run --release -p deal-lsp-smoke -- tests/showcase
```

**Adaptations required:**
1. Use the `SCRIPT_DIR` + `REPO_ROOT` resolution pattern from `verify-fresh-worktree.sh` lines 77–84 rather than the inline `cd "$(dirname "$0")/.."` — more robust under symlinks.
2. The `lsp-smoke` binary is a new dev-binary (Plan 07): either a tiny new workspace member (`/Users/dunnock/projects/deal-lang/deal/lsp-smoke/Cargo.toml`) or a `#[test]`-gated integration test in the existing `lsp/tests/showcase.rs`. RESEARCH.md §13 lines 978–987 enumerates the assertions (initialize → didOpen → diagnostics → completion → definition → hover → formatting → semanticTokens → exit 0).
3. Hook this script as Phase 3 gate step (8) per RESEARCH.md §13 line 965.

---

## Shared Patterns

### FFI clone-before-free (Pitfall 3 / T-02-29)

**Source:** `/Users/dunnock/projects/deal-lang/deal/cli/src/main.rs` lines 192–227 (diagnostics) + 549–562 (formatter).

**Apply to:** All `deal-lsp` files that call any of the 5 buffer-returning FFI symbols (`deal_ast_json`, `deal_diagnostics_json`, `deal_ir_json`, `deal_format`, `deal_index_json`).

```rust
// PATTERN — clone bytes WHILE arena is alive, BEFORE deal_free.
let owned: Vec<u8>;
unsafe {
    let mut out_ptr: *const u8 = std::ptr::null();
    let mut out_len: usize = 0;
    let ok = ffi::deal_diagnostics_json(handle, &mut out_ptr, &mut out_len);
    if !ok { ffi::deal_free(handle); return Err(/* OOM */); }
    owned = std::slice::from_raw_parts(out_ptr, out_len).to_vec();  // <-- CLONE FIRST
    ffi::deal_free(handle);                                          // <-- THEN free
}
```

**LSP wrinkle:** the LSP holds handles live per-document (D-42); `deal_free` only runs at `shutdown` or document removal. So the "clone before free" invariant becomes "clone before releasing the per-document mutex" — encapsulate inside `deal-ffi`'s safe wrappers so call-sites at `lsp/src/{diagnostics,formatting,semantic_tokens,completion,hover,definition,workspace,index}.rs` don't have to think about it.

---

### D-32 envelope reuse (CLI ↔ LSP diagnostic shape)

**Source:** CONTEXT.md line 148, RESEARCH.md §11 (push-mode `publishDiagnostics`).

**Apply to:** `lsp/src/diagnostics.rs`.

**Behavior:** The `deal_diagnostics_json` output is already alphabetical-keyed (D-18) and follows the D-32 envelope. The LSP parses the JSON array (same `serde_json::from_slice` as cli/src/main.rs line 235) and converts each record to `tower_lsp::lsp_types::Diagnostic`. The `code` field maps to `Diagnostic.code = NumberOrString::String("E2###")` per D-33 (CONTEXT.md line 149).

---

### Phase-gate inheritance chain (D-35)

**Source:** `/Users/dunnock/projects/deal-lang/deal/build.zig` lines 207–208 (1.5 extends 1) + line 265 (2 extends 1.5).

**Apply to:** Plan 07's `phase-3-gate` step.

```zig
// PATTERN — every phase gate dependOn the previous phase's gate.
gate_3_step.dependOn(gate_2_step); // Phase 3 inherits Phase 2 (which inherits 1.5 which inherits 1)
```

This is D-35's "extends, not replaces" decision: running `zig build phase-3-gate` re-validates Phases 1 + 1.5 + 2 transitively. Phase 4..6 will follow the same pattern.

---

### Fresh-worktree gate (ADR-phase-1.5-fresh-worktree-verification)

**Source:** `/Users/dunnock/projects/deal-lang/deal/scripts/verify-fresh-worktree.sh` + `build.zig` lines 233–242 (`gate_1_5_fresh_step`), 278–287 (`gate_2_fresh_step`).

**Apply to:** Plan 07's `phase-3-gate-fresh` step. **NO SCRIPT CHANGES** — the script was generalized in Plan 02-06 to accept `<gate-step>` as argv[1]. The build.zig block is the verbatim mirror of `gate_2_fresh_step` with `"phase-2-gate"` → `"phase-3-gate"` (excerpt above in Plan 07 / build.zig section).

---

### Showcase symlink (tests/showcase → ../spec/examples/showcase)

**Source:** `/Users/dunnock/projects/deal-lang/deal/tests/showcase` — committed symlink to `../spec/examples/showcase` (Phase 1.5 Plan 05 fix).

**Apply to:** All Phase 3 test harnesses that exercise the 19-file showcase corpus:
- `/Users/dunnock/projects/deal-lang/deal/lsp/tests/showcase.rs`
- `/Users/dunnock/projects/deal-lang/tree-sitter-deal/test/corpus/*.txt` (extract / link)
- `/Users/dunnock/projects/deal-lang/vscode-deal/src/test/suite/*` (open showcase workspace)
- `/Users/dunnock/projects/deal-lang/deal/scripts/phase-3-smoke.sh` (line 6 of RESEARCH.md §13 sample)

The 19-file corpus (CONTEXT.md line 120) lives at:
```
/Users/dunnock/projects/deal-lang/spec/examples/showcase/
├── deal.toml
├── packages/
│   ├── vehicle/{battery,motor,components,behaviors,index}.deal
│   ├── requirements/{system,needs,index}.deal
│   └── use-cases/{driving,charging,index}.deal
└── model/
    ├── vehicle.dealx
    ├── traceability.dealx
    └── variants/{sedan,performance}.dealx
```

`phase-3-gate-fresh` requires `git submodule update --init --recursive` to materialize this (verify-fresh-worktree.sh lines 109–116 already does this).

---

### `deal-lsp` workspace-symbol index merge (D-46 + D-49)

**Source:** `/Users/dunnock/projects/deal-lang/deal/cli/src/main.rs` lines 215–223 (per-file `deal_index_json` accumulation into `per_file_indexes: Vec<Vec<u8>>`).

**Apply to:** `lsp/src/workspace.rs` (Plan 04). The LSP's in-memory `HashMap<PathString, (FileUri, Range)>` (D-46) is populated by walking each open document's `deal_index_json` output and merging entries into the global map. Cross-package alias resolution from `deal.toml [workspace.aliases]` (PS-5, CONTEXT.md line 102) layers in Rust on top — does NOT touch the Zig core (D-49).

---

## No Analog Found

| File | Role | Data Flow | Reason | Resolution |
|---|---|---|---|---|
| `vscode-deal/icons/deal-light.svg` (+ dark + dealx) | asset | static | No icons exist anywhere in the workspace; iconography is brand-new (CONTEXT.md `<specifics>` line 183: ".deal = solid blueprint sheet; .dealx = JSX-style angle-brackets") | Designer-originated by planner; no pattern to copy. |
| `vscode-deal/syntaxes/{deal,dealx}.tmLanguage.json` | grammar (regex) | transform | No TextMate grammar exists in the repo. The closest in-repo signal is `spec/grammar/{lexical,deal,dealx}.ebnf` for token names and `deal/include/deal.h` for error-code categories — but neither IS a tmLanguage. | Use RESEARCH.md §6 mapping table (20 categories) verbatim as authoritative; researcher already drafted scope names. External pattern: any well-curated `.tmLanguage.json` (e.g., TypeScript's at `microsoft/TypeScript-TmLanguage`). |
| `tree-sitter-deal/grammar.js` | grammar (DSL) | transform | No tree-sitter grammar exists in-repo. The source-of-truth productions live in `spec/grammar/*.ebnf` — but those are EBNF, not tree-sitter DSL. | Hand-author per RESEARCH.md §3 + §9 (lean grammar). Cite `tree-sitter-rust/grammar.js` (external, in RESEARCH.md §3 references). |
| `vscode-deal/src/extension.ts` + sibling .ts files | controller / service | event-driven | No TypeScript exists in deal/. The stale `/Users/dunnock/projects/deal-lang/lsp/README.md` describes a TS LSP but that design is rejected (CONTEXT.md line 115 — "DO NOT USE AS DESIGN INPUT"). | RESEARCH.md §5 canonical shapes; rust-analyzer's `editors/code/src/*.ts` is the gold-standard cross-reference. |
| `deal/.github/workflows/release.yml` | CI | event-driven | No `.github/workflows/` content exists in `deal/`. | RESEARCH.md §12 verbatim 5-job pipeline. |
| `deal/lsp/src/{completion,hover,definition}.rs` | service | request-response | New LSP capabilities; no in-repo analog (Phase 2 CLI doesn't do completion/hover/definition). | RESEARCH.md §1 + §7; rust-analyzer source is the closest external pattern (CONTEXT.md `<canonical_refs>` line 129). |

---

## Metadata

**Analog search scope:**
- `/Users/dunnock/projects/deal-lang/deal/` (full tree)
- `/Users/dunnock/projects/deal-lang/{vscode-deal,tree-sitter-deal,lsp,editor}/` (scaffold READMEs)
- `/Users/dunnock/projects/deal-lang/spec/examples/showcase/` (corpus inventory)
- `/Users/dunnock/projects/deal-lang/.github/`, `/Users/dunnock/projects/deal-lang/deal/.github/` (CI workflows — none present)

**Files scanned (read-only):**
- `/Users/dunnock/projects/deal-lang/deal/.planning/phases/03-editor-intelligence/03-CONTEXT.md` (full)
- `/Users/dunnock/projects/deal-lang/deal/.planning/phases/03-editor-intelligence/03-RESEARCH.md` (full — sections §1, §3, §4, §5, §6, §7, §8, §10, §11, §12, §13, §14, §16, plus header / assumptions / open-questions tail)
- `/Users/dunnock/projects/deal-lang/deal/Cargo.toml`
- `/Users/dunnock/projects/deal-lang/deal/cli/Cargo.toml`
- `/Users/dunnock/projects/deal-lang/deal/cli/build.rs`
- `/Users/dunnock/projects/deal-lang/deal/cli/src/ffi.rs`
- `/Users/dunnock/projects/deal-lang/deal/cli/src/main.rs` (lines 1–100, 170–245)
- `/Users/dunnock/projects/deal-lang/deal/tests/ffi/Cargo.toml`
- `/Users/dunnock/projects/deal-lang/deal/tests/ffi/build.rs`
- `/Users/dunnock/projects/deal-lang/deal/tests/ffi/src/lib.rs`
- `/Users/dunnock/projects/deal-lang/deal/tests/ffi/tests/parse.rs` (lines 1–90)
- `/Users/dunnock/projects/deal-lang/deal/include/deal.h` (full)
- `/Users/dunnock/projects/deal-lang/deal/scripts/verify-fresh-worktree.sh` (full)
- `/Users/dunnock/projects/deal-lang/deal/build.zig` (lines 1–80, 200–290)
- `/Users/dunnock/projects/deal-lang/vscode-deal/README.md`
- `/Users/dunnock/projects/deal-lang/tree-sitter-deal/README.md`
- `/Users/dunnock/projects/deal-lang/lsp/README.md` (stale — for awareness only)

**Pattern extraction date:** 2026-05-22

## PATTERN MAPPING COMPLETE
