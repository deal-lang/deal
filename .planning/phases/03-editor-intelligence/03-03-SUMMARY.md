---
phase: 03-editor-intelligence
plan: 03
plan_id: 03-03
subsystem: editor-intelligence
tags: [lsp, ffi, rust-workspace, tower-lsp, debounce, formatting, diagnostics]
dependency_graph:
  requires:
    - REQ-phase-2-1-c-abi  # 9-export ABI from Phase 2 (deal_parse..deal_index_json)
    - phase-2-pipeline       # libdeal.a builds via zig build
  provides:
    - deal-ffi              # shared Rust crate (single workspace links="deal" claimant)
    - deal-lsp              # binary serving textDocument/publishDiagnostics + formatting
    - OwnedDealHandle       # D-45 Send + Drop-frees-arena newtype consumed by Plan 04
    - Documents             # DashMap<Url, Arc<Mutex<OwnedDealHandle>>> primitive consumed by Plan 04
    - Debouncer             # per-URI 300ms cancellable debouncer (D-43)
  affects:
    - cli/                   # extracted FFI; cli now consumes deal-ffi via `use deal_ffi as ffi;`
tech_stack:
  added:
    - tower-lsp:0.20.0 (pinned exact; Issue 1 resolution)
    - tokio:1.x (full features)
    - tower:0.5
    - dashmap:6
    - ropey:1.6
    - async-trait:0.1
    - tracing:0.1
    - tracing-subscriber:0.3 (env-filter)
    - futures:0.3 (dev-only — ClientSocket stream draining in integration tests)
  patterns:
    - "Single-claimant `links = native_lib` per Cargo workspace (deal-ffi owns `links = \"deal\"`)"
    - "Safe-wrapper layer encapsulating clone-before-free invariant (Pitfall 3 / T-02-29)"
    - "Per-URI Debouncer with cancel-and-replace JoinHandles (D-43)"
    - "ClientSocket-as-Stream notification draining for in-process LSP tests"
key_files:
  created:
    - deal-ffi/Cargo.toml
    - deal-ffi/build.rs (moved from cli/build.rs)
    - deal-ffi/src/lib.rs
    - lsp/Cargo.toml
    - lsp/README.md
    - lsp/src/lib.rs
    - lsp/src/main.rs
    - lsp/src/backend.rs
    - lsp/src/documents.rs
    - lsp/src/diagnostics.rs
    - lsp/src/debounce.rs
    - lsp/src/formatting.rs
    - lsp/tests/showcase.rs
    - lsp/tests/golden/formatted/battery.deal.expected
    - .planning/phases/03-editor-intelligence/deferred-items.md
  modified:
    - Cargo.toml (workspace members + 8 new workspace.dependencies)
    - cli/Cargo.toml (dropped `links`; added deal-ffi dep)
    - cli/src/main.rs (replaced `pub mod ffi;` with `use deal_ffi as ffi;`)
  deleted:
    - cli/src/ffi.rs (relocated into deal-ffi/src/lib.rs)
decisions:
  - "Honoured D-49 architectural split: deal-ffi + deal-lsp live in the `deal/` Rust workspace; Zig core remains single-file-scoped."
  - "tower-lsp pinned to exact 0.20.0 (Issue 1) — verified available via `cargo search tower-lsp` before commit."
  - "Phase 2 D-32 envelope is a top-level JSON array with `span: [start_byte, end_byte]` byte offsets — NOT the line/character `range` shape cited in PLAN.md `<interfaces>`. Discovered live during Task 3 via a built deal-ffi probe; diagnostics.rs rewritten to consume the actual wire format (Rule 1 bug fix)."
  - "Golden file named `battery.deal.expected` (not `BatteryPack.deal.expected`) because the canonical showcase battery model lives at `spec/examples/showcase/packages/vehicle/battery.deal` — the PLAN.md path was a typo (Rule 3 deviation)."
  - "Workspace `members` adds `deal-ffi` in Task 1 and `lsp` in Task 2 (NOT both at once) — adding `lsp` before lsp/Cargo.toml exists breaks `cargo build --workspace` (Rule 3 deviation)."
metrics:
  duration_min: 19
  completed_date: "2026-05-23"
  tasks_completed: 3
  files_created: 14
  files_modified: 3
  files_deleted: 1
  unit_tests_added: 12 (deal-ffi 2 + lsp 12 — diagnostics 6, debounce 2, formatting 3, ffi 2 (counted once))
  integration_tests_added: 3
  workspace_tests_total: 73 (was 70; +3 new lsp integration tests)
---

# Phase 3 Plan 3: deal-lsp Scaffold + FFI Extraction Summary

deal-lsp now serves push-mode diagnostics and in-memory formatting against
the showcase via a tower-lsp 0.20.0 backend that consumes the Phase 2
9-export C ABI through the new shared `deal-ffi` crate.

## One-Liner

Extracted the 9-export FFI surface from `cli/src/ffi.rs` into a new
`deal-ffi` rlib crate (single `links = "deal"` workspace claimant), scaffolded
the `deal-lsp` binary on tower-lsp 0.20.0 with per-document handle lifecycle
(D-42/D-44/D-45) + 300 ms debounce (D-43) + diagnostic push + in-memory
formatting (D-21), and shipped three passing tokio integration tests that
exercise the Backend through the real LspService pipeline (diagnostics →
E-codes, format → golden byte-equality, debounce → coalescing).

## Tasks Completed

| Task | Name                                                         | Commit  |
| ---- | ------------------------------------------------------------ | ------- |
| 1    | Extract deal-ffi crate; cli migrates to `use deal_ffi as ffi;` | c4b5328 |
| 2    | Scaffold deal-lsp; backend.rs + documents.rs + debounce.rs + formatting.rs + diagnostics.rs | 97c89ab |
| 3    | Integration tests + golden file; fix diagnostic envelope schema | 9e85c44 |

## Architectural Highlights

### deal-ffi shared crate (Task 1)

- Single-claim invariant: `links = "deal"` moved from `cli/Cargo.toml` to
  `deal-ffi/Cargo.toml`. Cargo enforces exactly one claim per native library
  per workspace; both `cli` and `lsp` now consume libdeal.a transitively.
- 9 extern "C" declarations match `include/deal.h` verbatim
  (`deal_parse`, `deal_free`, `deal_has_errors`, `deal_diagnostics_count`,
  `deal_diagnostics_json`, `deal_ast_json`, `deal_ir_json`, `deal_format`,
  `deal_index_json`).
- New `OwnedDealHandle` newtype (D-45) with `unsafe impl Send` justification
  + `Drop`-fires-`deal_free` so panic-safe even on tokio task paths.
- `pub mod safe` provides 8 helpers (`parse`, `has_errors`, `diagnostics_count`,
  `diagnostics_json`, `ast_json`, `ir_json`, `index_json`, `format`) that
  encapsulate the Pitfall 3 / T-02-29 clone-before-free invariant — LSP
  call sites never see raw pointers.
- `cli/src/main.rs` does `use deal_ffi as ffi;` so all 1500+ existing
  `ffi::deal_*` call sites compile unchanged. Cli unit + integration tests
  pass with zero regressions (70 → 70 pre-extraction parity).

### deal-lsp crate (Task 2)

- `lsp/src/lib.rs` exposes `Backend`, `Documents`, `Debouncer`, and
  `parse_diagnostics` for both the binary and integration tests.
- `Backend` implements 7 LanguageServer methods. Capabilities advertised:
  `text_document_sync = FULL`, `document_formatting_provider = true`.
  Plan 04 will extend with completion / hover / definition / semantic-tokens /
  workspace-symbol.
- `Documents` holds `DashMap<Url, Arc<tokio::sync::Mutex<OwnedDealHandle>>>`
  (D-42 per-URI handle table; D-45 per-handle Mutex) plus a parallel
  `DashMap<Url, Rope>` for formatting range computation and byte-span →
  position translation. `parse_count: AtomicUsize` is a test hook.
- `Debouncer` keyed by URI with cancel-and-replace JoinHandles — `schedule()`
  aborts the prior task before spawning the new one, so a burst of
  did_change events within `DEBOUNCE_MS = 300` collapses to one re-parse.
- `did_close` is a NO-OP per D-44; handles persist for workspace lifetime
  so Plan 04 workspace-symbol / cross-file goto-definition keep working
  against closed buffers.
- `formatting::handle_formatting` reuses the LIVE handle (no re-parse per
  D-21), acquires the Mutex (D-45), declines if `has_errors`, computes
  full-document Range from the stored Rope, returns a single `TextEdit`.

### Integration tests + golden (Task 3)

- Golden file `lsp/tests/golden/formatted/battery.deal.expected` (4 767 bytes)
  generated ONCE by `./target/release/deal fmt --stdout
  spec/examples/showcase/packages/vehicle/battery.deal` and committed.
- Test harness drives `Backend` through `LspService::new`; the
  `ClientSocket` (a `futures::Stream<Item = Request>`) is drained on a
  background tokio task into an `Arc<Mutex<Vec<JsonRpcRequest>>>` so tests
  can assert on `publishDiagnostics` notifications after triggering
  `did_open` / `did_change`.
- Three required tests all pass on first run after the diagnostic-envelope
  fix (see Deviations below).

## tower-lsp pin status (Issue 1)

| Item                                                          | Value                |
| ------------------------------------------------------------- | -------------------- |
| Pin in `[workspace.dependencies]`                             | `tower-lsp = "0.20.0"` (exact) |
| `cargo search tower-lsp` confirmed at Task 1 start             | yes — 0.20.0 listed |
| Yanked / unpublished?                                          | no                   |
| Plan-revision escalation needed?                               | no                   |

## deal-ffi extraction status

| Item                                                           | Result        |
| -------------------------------------------------------------- | ------------- |
| Lines moved verbatim from `cli/src/ffi.rs`                     | 109 lines (struct + extern block) |
| Lines added in `deal-ffi/src/lib.rs`                           | ~260 lines (OwnedDealHandle + safe::* + 2 smoke tests + module docs) |
| `cli/build.rs` → `deal-ffi/build.rs`                            | git-detected rename (~83% similarity); single `.join("..")` unchanged |
| `links = "deal"` location                                       | deal-ffi/Cargo.toml (was cli/Cargo.toml) |
| Cli test regression                                             | 0 (all 70 prior tests still pass) |
| Cli call-site edits                                             | 1 line (`pub mod ffi;` → `use deal_ffi as ffi;`); 1500+ `ffi::deal_*` call sites unchanged |

## Integration test results

| Test                                       | Wall-clock | Outcome |
| ------------------------------------------ | ---------- | ------- |
| `diagnostics_match_phase_2_codes`          | ~50 ms     | PASS — got E2100 ERROR diagnostic with source="deal-lsp" |
| `format_round_trip`                        | ~50 ms     | PASS — TextEdit.new_text byte-equal to 4 767-byte golden |
| `debounce_collapses_rapid_changes`         | ~430 ms    | PASS — open=1 parse + 5 rapid did_changes coalesced to 1 debounced re-parse |

Committed golden file size: **4 767 bytes** (129 lines, matching the
showcase source line count — the formatter is idempotent on already-canonical
input, a good sanity check).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 – Bug] Diagnostic envelope schema mismatch**
- **Found during:** Task 3 (when wiring `diagnostics_match_phase_2_codes`).
- **Issue:** Task 2 wrote `diagnostics.rs` assuming the envelope shape from
  PLAN.md's `<interfaces>` block:
  `{ "v": 1, "diagnostics": [{ "range": { "start": {"line", "character"} } }] }`.
  A live probe (built `/tmp/dump_probe` against deal-ffi) revealed the
  Phase 2 Zig core actually emits a flat array where each record carries
  `span: [start_byte, end_byte]` UTF-8 byte offsets plus Phase-2-vocab
  severity strings (`"err"`, `"warn"`, `"info"`, `"hint"`).
- **Fix:** Rewrote `parse_diagnostics` to take an `&ropey::Rope` argument,
  deserialize the actual wire shape, and translate byte spans to LSP
  `Position { line, character }` (UTF-16 code units, multi-byte safe via
  ropey). Updated `Documents.{open,update}` to build the Rope first and
  pass it through. Added 3 new unit tests covering the array shape, the
  full severity vocab, out-of-range clamping, and `α + 𝄞` UTF-16 counting.
- **Files modified:** `lsp/src/diagnostics.rs`, `lsp/src/documents.rs`.
- **Commit:** 9e85c44.

**2. [Rule 3 – Blocking] PLAN.md showcase path does not exist**
- **Found during:** Task 3 (when generating the golden file).
- **Issue:** PLAN.md cites
  `tests/showcase/sedan_project/vehicle/electrical/BatteryPack.deal`
  and the golden as `lsp/tests/golden/formatted/BatteryPack.deal.expected`.
  Neither exists — the showcase tree at `tests/showcase/` has just
  `build/`, `model/`, `packages/`, `simulations/`, `test/`, `deal.toml`
  (no `sedan_project/`), and a `find` for `BatteryPack*` returns nothing.
  The canonical battery model is at
  `spec/examples/showcase/packages/vehicle/battery.deal` (129 lines, exists).
- **Fix:** Substituted the actual path; golden is named
  `battery.deal.expected`. Documented in the test source file header and
  in this Summary.
- **Files affected:** `lsp/tests/showcase.rs`, `lsp/tests/golden/formatted/battery.deal.expected`.
- **Commit:** 9e85c44.

**3. [Rule 3 – Blocking] Cannot list `lsp` in workspace members before lsp/Cargo.toml exists**
- **Found during:** Task 1 (when first running `cargo build --workspace`).
- **Issue:** PLAN.md behavior bullet 1 says
  `members = ["cli", "lsp", "deal-ffi", "tests/ffi"]`. But listing `lsp`
  before `lsp/Cargo.toml` exists makes `cargo build --workspace` fail at
  the workspace-resolve step. Task 2 creates lsp/.
- **Fix:** Task 1 sets `members = ["cli", "deal-ffi", "tests/ffi"]`;
  Task 2 appends `"lsp"`. Commented in `deal/Cargo.toml` so future readers
  understand the staging.
- **Files modified:** `Cargo.toml`.
- **Commit:** c4b5328 (T1) → 97c89ab (T2 amends).

### Deferred Issues

Pre-existing clippy warnings unrelated to Plan 03-03 logged to
`.planning/phases/03-editor-intelligence/deferred-items.md` per the SCOPE
BOUNDARY rule:

- `tests/ffi/tests/parse.rs:87` — `clippy::bool-assert-comparison` (from commit 0f82b1e, Phase 01-06).
- `tests/ffi/tests/parse.rs:227` — `clippy::manual-range-contains` (same commit).
- `cli/src/main.rs:320` — `clippy::unnecessary-unwrap` (from commit 9b2d64a, Phase 2 verifier closeout).

Plan 03-03's clippy gate is therefore scoped to crates this plan creates
(`cargo clippy -p deal-ffi -p deal-lsp --all-targets -- -D warnings`).
A future Phase 3 / Phase 4 lint-cleanup plan can re-enable the workspace-wide
gate.

## Auth Gates

None — this plan executes entirely against the local toolchain (cargo,
zig, libdeal.a). No external services, no auth surface.

## Threat Flags

No new security-relevant surface introduced beyond what `<threat_model>` in
PLAN.md already enumerates (T-3-03 supply chain, T-3-04 DoS, T-3-05 FFI safety).
The 6 added crates from `[workspace.dependencies]` are all on the
`[ASSUMED]` allow-list per RESEARCH §1199 (tokio-rs, xacrimon, helix-editor,
dtolnay maintainers; 5+ year publish history). `futures = "0.3"` is added
as a dev-dep only (not in the runtime binary) — same maintainer cohort
(rust-lang-nursery / tokio-rs).

## Known Stubs

None. No `TODO`, `FIXME`, `unimplemented!()`, or hardcoded-empty-data
patterns in `lsp/src/` or `deal-ffi/src/`. The Backend declares only the
capabilities Plan 03-03 actually serves (`text_document_sync = FULL`,
`document_formatting_provider = true`) so the client never sends requests
for unimplemented Plan-04 capabilities.

## Self-Check: PASSED

Verified files:
- FOUND: deal-ffi/Cargo.toml
- FOUND: deal-ffi/build.rs
- FOUND: deal-ffi/src/lib.rs
- FOUND: lsp/Cargo.toml
- FOUND: lsp/README.md
- FOUND: lsp/src/lib.rs
- FOUND: lsp/src/main.rs
- FOUND: lsp/src/backend.rs
- FOUND: lsp/src/documents.rs
- FOUND: lsp/src/diagnostics.rs
- FOUND: lsp/src/debounce.rs
- FOUND: lsp/src/formatting.rs
- FOUND: lsp/tests/showcase.rs
- FOUND: lsp/tests/golden/formatted/battery.deal.expected

Verified commits (via `git log --oneline`):
- FOUND: c4b5328 — refactor(03-03): extract deal-ffi shared crate
- FOUND: 97c89ab — feat(03-03): scaffold deal-lsp crate
- FOUND: 9e85c44 — test(03-03): integration tests + golden file

Verified test results (most recent `cargo test --workspace`):
- deal-ffi: 2/2 pass
- deal-lsp lib: 12/12 pass
- deal-lsp showcase: 3/3 pass
- all other workspace crates: green (no regressions)
- `cargo clippy -p deal-ffi -p deal-lsp --all-targets -- -D warnings`: clean
