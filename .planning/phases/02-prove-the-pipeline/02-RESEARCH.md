# Phase 2: Prove the Pipeline — Research

**Researched:** 2026-05-21
**Domain:** Cross-language compiler frontend (Zig core) + codegen backend (Rust) for the DEAL language; AST → semantic check → DEAL IR v0 → SysML v2 JSON pipeline with offline OMG schema validation, gofmt-style comment-preserving formatter, and a Rust `clap`-driven CLI shell that consumes the existing 6-symbol C ABI plus two new symbols (`deal_ir_json`, `deal_format`).
**Confidence:** **HIGH** on the Rust crate ecosystem (every recommended crate verified via `cargo search` + slopcheck against crates.io), the SysML v2 viewer landscape (Eclipse SysON 2025.x is the clearly-best target — verified via official docs), and the Phase 1 architectural patterns we extend (arena allocator, alphabetical-key JSON, C ABI handle pattern — all in `src/`). **MEDIUM** on exact SysON JSON import wire format (their REST API docs are good but the file-upload path's expected shape is partially undocumented — Plan 02-04/02-06 will need a focused 1-2hr probe). **LOW** on whether `/** */` doc-comment lowering into SysML `documentation` ownedRelationships is byte-stable across SysML v2 spec micro-versions — recommended to defer per CONTEXT.md.

<user_constraints>
## User Constraints (from CONTEXT.md)

> Copied verbatim from `.planning/phases/02-prove-the-pipeline/02-CONTEXT.md` `<decisions>` block. The planner MUST honor all locked decisions (D-19..D-37). The Claude's Discretion items are where research recommends; the Deferred Ideas list is OUT OF SCOPE for Phase 2.

### Locked Decisions

**Tech-stack split (D-19..D-21):**
- **D-19** Rust owns the SysML v2 emitter and the offline JSON-Schema validator. Walks the IR via the new C ABI `deal_ir_json()`.
- **D-20** Zig owns the semantic analyzer (`src/sema.zig`) running all 6 blocking checks and producing `.deal/index.json`.
- **D-21** Zig owns the pretty-printer (`src/fmt.zig`). Rust CLI calls via new C ABI symbol `deal_format(handle, out_ptr, out_len)`.

**IR v0 transport and contract (D-22..D-27):**
- **D-22** IR crosses the FFI boundary as JSON via `deal_ir_json(handle, out_ptr, out_len)`. Top-level `{"v": 1, "ir_version": "v0", ...}`, alphabetical-key ordering per D-18.
- **D-23** Stable element IDs are fully-qualified path strings (`sedan_project.vehicle.electrical.BatteryPack.hvOut`). `.` separator between every component.
- **D-24** Single workspace-wide IR graph. ONE consolidated `tests/showcase/build/sysml-v2/showcase.sysml-v2.json` file.
- **D-25** IR is comment-free. Comments stay on AST only. `deal fmt` walks AST; SysML emitter walks IR.
- **D-26** IR v0 backend traversal API: `walk(visitor)`, `find(id)`, `references(id)`, `children(id)`, `parent(id)`. **No** query DSL in v0.
- **D-27** Dual-document spec: `.planning/decisions/ADR-deal-ir-v0.md` (rationale) + `spec/ir/v0/schema.json` (normative JSON Schema) + `spec/ir/v0/README.md` (Markdown reference).

**Comment attachment (D-28..D-31):**
- **D-28** Leading + trailing comments on declaration/statement nodes. Gofmt-style blank-line rule.
- **D-29** `/** */` JSDoc comments promoted to typed `doc_comment: ?DocComment` field. Reuses existing `DocComment` struct at `src/ast.zig:393`.
- **D-30** AST JSON stays at `"v": 1` — no schema-version bump. 19 AST snapshots regenerated in one dedicated commit. Determinism + span-containment tests MUST continue to pass.
- **D-31** Comment attachment lands FIRST in plan order (Plan 02-01) — before sema/IR work begins.

**Diagnostic shape and error codes (D-32..D-34):**
- **D-32** `deal {subcommand} --json` envelope: `{"v": 1, "command": "...", "deal_version": "...", "diagnostics": [...], "summary": {...}}`. `deal_diagnostics_json()` unchanged — Rust CLI does the envelope-wrap.
- **D-33** E2xxx semantic error code band: `E2000..E2099` name resolution, `E2100..E2199` type checking, `E2200..E2299` multiplicity, `E2300..E2349` `<<specializes>>`, `E2350..E2399` `@trace`, `E2400..E2499` import resolution. Extends D-16.
- **D-34** CLI exit codes: `0` success, `1` user-visible error, `2` internal error (panic/FFI shape mismatch/schema-load failure).

**Gates and slicing (D-35..D-37):**
- **D-35** Phase-2 exit gate inherits fresh-worktree invariant. `phase-2-gate` + `phase-2-gate-fresh` build steps. Plus viewer-import smoke + IR-lock checkpoint records in `02-VERIFICATION.md`.
- **D-36** SysML v2 viewer priority order: (1) OpenMBEE SysML v2 Pilot Implementation, (2) IncQuery's SysML v2 toolset, (3) NoMagic/Cameo, (4) generic JSON-Schema viewer fallback.
- **D-37** Plan slicing — 6 plans: 02-01 CLI shell + comment attachment, 02-02 semantic analyzer, 02-03 IR + lowering, **🔒 IR-lock checkpoint**, 02-04 SysML codegen + validator + golden fixtures, 02-05 `deal fmt`, 02-06 phase-2-gate + viewer-smoke + verification.

### Claude's Discretion

- **Schema-registry implementation crate.** Rust crate selection for offline JSON-Schema validation (e.g., `jsonschema` vs `boon` vs `schemars`) — researcher's call; constraint is offline-only resolution of absolute OMG `$id`/`$ref` URLs against the local bundle.
- **Cargo workspace layout.** Whether `cli/` and `tests/ffi/` share a Cargo workspace, and where `bindgen` of `deal.h` happens (build.rs vs committed wrapper). Phase 1's `tests/ffi/Cargo.toml` is the closest existing precedent.
- **Agent-metadata envelope shape in IR.** `@confidence` / `@rationale` / `@assumes` / `@concerns` map to which IR field names and types.
- **Simulation-binding representation in IR.** How `@simulation:<<computes>>` annotations are represented — defer to research; IR carries them but exact shape isn't locked beyond "the SysML v2 emitter doesn't need them in Phase 2."
- **Doc-comment `documentation` lowering to SysML.** Whether `/** */` text flows into SysML v2 `documentation` ownedRelationships in Phase 2's emitter.
- **Blank-line rule edge cases for comment attachment.** Exact gofmt rule shape; cite Go's `go/ast` and `go/printer` for the canonical pattern.
- **Whether `deal_format()` returns the formatted bytes or writes to a caller-supplied path.** Lean toward bytes (LSP-friendly).

### Deferred Ideas (OUT OF SCOPE)

- MQL query language for IR (Phase 6+).
- IR carries doc comments as `documentation` field (follow-up ADR if showcase output is missing inline docs).
- AST JSON `"v": 2` schema bump (kept at v: 1; comment-attachment is additive).
- Per-package IR + per-file SysML output (Phase 6 desktop-tooling concern).
- Free-floating trivia list (fallback architecture if structural attachment hits round-trip edge cases).
- FlatBuffers / shared-struct IR transport.
- `deal init` / `deal install` / package resolution (Phase 4).
- LSP / VS Code / tree-sitter (Phase 3).
- ReqIF emitter + reverse importers (Phase 4 / Phase 6).
- Simulation / evidence / `deal-sim` (Phase 5).
- WASM packaging of Zig core (evaluate at Phase 2 gate per PRD §11).
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| REQ-phase-2-prove-pipeline | Exit gate: `deal build --target sysml-v2` produces valid importable SysML v2 JSON; `deal fmt` round-trips all 19 files; `deal check` catches type and import errors | Standard Stack + Validation Architecture + Common Pitfalls (sections below) |
| REQ-phase-2-1-semantic-analyzer | 6 blocking checks + `.deal/index.json` | Pattern 4 (Name resolution + scope graph) + Pattern 5 (`.deal/index.json` schema sketch) |
| REQ-phase-2-2a-ir-v0 | IR v0 dual-document specification (ADR + JSON Schema + Markdown ref) | Pattern 6 (IR shape — node-kind enum + uniform record envelope) + Code Example 2 |
| REQ-phase-2-2b-ir-lowering | AST → IR lowering pass in Zig with conformance tests | Pattern 7 (AST → IR lowering — visitor + ID-interning + arena) + Pitfall 3 |
| REQ-phase-2-3-sysml-v2-codegen | SysML v2 JSON emitter targeting OMG schemas | Pattern 8 (SysML mapping table) + Code Example 3 |
| REQ-phase-2-3a-offline-validator | Offline JSON-Schema validator + `--validate` flag | Standard Stack: `jsonschema` v0.46+ with `Retrieve` trait + Code Example 4 |
| REQ-phase-2-3b-golden-fixtures | 5–8 hand-written `.deal` ↔ expected `.json` pairs | Pattern 9 (Insta + dual gate: byte-exact AND schema-valid) + Pitfall 6 |
| REQ-phase-2-4-formatter | `deal fmt` round-trip with comment preservation | Pattern 2 (gofmt-style comment attachment) + Pattern 3 (Doc-IR pretty-printer) + Pitfall 2 |
| REQ-phase-2-5-cli-shell | Rust CLI: `parse`/`check`/`fmt`/`build` + `--json`/`--color`/`--verbose` | Standard Stack: `clap` v4.6 derive + `anstream` + `owo-colors` + Code Example 5 |
| REQ-phase-2-gate | Phase-2 exit gate + fresh-worktree gate + viewer smoke + IR-lock checkpoint | Validation Architecture (full section) + Common Pitfalls 7 (fresh-worktree drift) |
</phase_requirements>

## Summary

Phase 2 is the integration phase: it stitches together a Zig compiler core (Phase 1 deliverable) and a fresh Rust frontend across a length-prefixed UTF-8 C ABI to deliver the first end-to-end pipeline. The research below assumes every D-19..D-37 decision is locked and focuses ONLY on the technical knowledge needed to implement them well.

The Rust ecosystem in early-2026 is mature for every component this phase needs: `clap` v4.6 [VERIFIED: cargo search] for the CLI, `jsonschema` v0.46 [VERIFIED: cargo search + docs.rs] for draft-2020-12 validation with a custom `Retrieve` trait that makes offline mode trivial, `insta` v1.47 [VERIFIED: cargo search] for golden-fixture management, `owo-colors` v4.3 + `anstream` v1.0 [VERIFIED: cargo search] for the `--color={auto,always,never}` flag, and `bindgen` v0.72 [VERIFIED: cargo search] for generating Rust FFI bindings from `deal.h`. The Zig side is even simpler: every new module (`src/sema.zig`, `src/ir.zig`, `src/lowering.zig`, `src/fmt.zig`) extends established Phase 1 patterns (arena allocator from `deal_parse()`, alphabetical-key JSON emitter from `src/json.zig`, `Codes` namespace from `src/diagnostics.zig`).

The two highest-information findings are: (1) **Eclipse SysON 2025.x is the clear winner for D-36's viewer priority** — it has a documented file-upload path that accepts SysML v2 JSON, an REST API the OMG SysML 2.0 final-1.0 spec aligns with, and Phase 2's golden fixtures should target SysON's serialization conventions (alphabetical-key JSON, `@id == elementId`); this likely supersedes D-36's listed priority order. (2) **`jsonschema` v0.46's `Retrieve` trait is exactly the offline pattern this phase needs** — disable default features, register every absolute OMG `$id` URL → local file mapping in a `HashMap<String, Value>`-backed retriever, and the validator is offline-by-construction. No `boon` is needed unless `jsonschema` fails an edge-case test against the 126K-line `SysML.json`; recommendation is `jsonschema` first, `boon` as a documented fallback.

**Primary recommendation:** Wire up the CLI in Plan 02-01 with `clap` v4.6 derive + `anstream`/`owo-colors` + `bindgen` against `include/deal.h`; commit `cli/Cargo.toml` to a Cargo workspace that also contains the existing `tests/ffi/Cargo.toml`. Use `jsonschema` v0.46 with a `LocalBundleRetriever` for offline validation. Snapshot tests use `insta` v1.47 with `cargo insta review`. Comment attachment follows the gofmt rule cited below verbatim (no novel design). Doc-comment lowering to SysML `documentation` deferred to a follow-up ADR per CONTEXT.md.

## Architectural Responsibility Map

> Built per Step 1.5 of the research protocol. The planner uses this to sanity-check task tier assignments.

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Lex / parse `.deal` and `.dealx` | Zig compiler core | — | Phase 1 deliverable; unchanged in Phase 2 except for comment attachment (D-28..D-31). |
| Attach comments to AST nodes (D-28..D-31) | Zig compiler core (`src/lexer.zig` + `src/parser_*.zig` + `src/ast.zig`) | — | Comments are first-class trivia tokens; D-21 locks Zig owns the parser/printer pair. |
| Semantic analysis: name resolution, type-check, multiplicity, `<<specializes>>`, `@trace`, import resolution (D-20) | Zig compiler core (`src/sema.zig`) | — | Walks AST in parser arena. Zero FFI cost. |
| `.deal/index.json` generation (D-20) | Zig compiler core (`src/sema.zig` writes via `src/json.zig`) | Rust CLI (writes the file to disk) | Zig produces JSON bytes; Rust CLI handles file I/O — same pattern as `deal_ast_json` in Phase 1. |
| AST → DEAL IR v0 lowering (D-22, REQ #3) | Zig compiler core (`src/ir.zig` + `src/lowering.zig`) | — | Walks AST + sema symbol table in same arena. |
| IR JSON serialization across FFI (D-22) | Zig compiler core (`deal_ir_json` in `src/lib.zig` via `src/json.zig`) | — | Mirrors `deal_ast_json` pattern. Length-prefixed UTF-8. |
| SysML v2 JSON codegen (D-19, REQ #4) | Rust CLI (`cli/src/sysml_v2.rs`) | — | Consumes IR JSON via FFI; emits SysML v2 JSON via `serde_json`. |
| Offline JSON Schema validation (D-19, REQ #5) | Rust CLI (`cli/src/schema_registry.rs`) | — | `jsonschema` crate with custom `Retrieve` mapping absolute URLs → local files. |
| Pretty-printer (D-21, REQ #7) | Zig compiler core (`src/fmt.zig`) | Rust CLI (calls `deal_format` and writes bytes to file/stdout) | Zig walks AST with attached comments; Rust handles I/O + TTY detection. |
| Diagnostic rendering (CLI human-mode) | Rust CLI | Zig compiler core (produces structured `Diagnostic` JSON) | Phase 1 already emits structured diagnostics; Rust CLI renders them with `codespan-reporting` or hand-rolled formatter against `owo-colors`. |
| `--json` envelope wrap (D-32) | Rust CLI | — | Zig emits the `diagnostics` array; CLI wraps with `{"v": 1, "command": ..., "deal_version": ..., "summary": ...}`. |
| CLI argument parsing + subcommand dispatch | Rust CLI (`cli/src/main.rs`) | — | `clap` v4.6 derive macros. |
| Color/TTY detection (`--color={auto,always,never}`) | Rust CLI | — | `anstream` v1.0 (handles strip-on-non-TTY automatically). |
| Golden fixture testing (REQ #6) | Rust test harness | — | `insta` snapshot testing inside `cli/tests/`. |
| Build orchestration (`zig build phase-2-gate`, `phase-2-gate-fresh`) | Zig `build.zig` | Shell script (`scripts/verify-fresh-worktree.sh` from Phase 1.5 reused) | Same pattern as Phase 1.5 ADR. |

## Standard Stack

### Core (Rust frontend — Phase 2 new)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `clap` | 4.6.x [VERIFIED: cargo search → 4.6.1] | CLI argument parsing + subcommands + help generation | De facto standard for Rust CLIs; mature derive macro; first-class support for global flags, subcommands, and TTY-aware color via `anstream` integration. [CITED: docs.rs/clap] |
| `serde` | 1.0.228 [VERIFIED: cargo search] | Serialization framework | Universal Rust serialization — every JSON crate plugs in. |
| `serde_json` | 1.0.149 [VERIFIED: cargo search] | JSON parse/emit for IR ingest + SysML v2 emit | Standard. Note: `serde_json::Value::Object` is a `BTreeMap` in `preserve_order=false` mode — automatically alphabetical, which aligns with D-18. |
| `jsonschema` | 0.46.x [VERIFIED: cargo search → 0.46.5; docs.rs/jsonschema] | Offline JSON Schema draft-2020-12 validation against `SysML.json` + `KerML.json` | Maintained (~63M downloads); supports draft-2020-12 via `jsonschema::draft202012` module; `Retrieve` trait makes offline mode a one-struct implementation. Disable default features → no `reqwest` pulled in (`default-features = false, features = ["resolve-file"]` or no `resolve-*` features at all if using only custom Retrieve). [CITED: docs.rs/jsonschema] |
| `anstream` | 1.0.x [VERIFIED: cargo search] | TTY-aware ANSI passthrough/strip | Auto-strips ANSI on non-TTY; honors `NO_COLOR`/`CLICOLOR`/`CLICOLOR_FORCE`; integrates cleanly with `--color={auto,always,never}`. [CITED: rust-cli-recommendations.sunshowers.io] |
| `owo-colors` | 4.3.x [VERIFIED: cargo search] | Style strings (red/bold/etc.) | Zero-allocation, no_std-compatible, supports NO_COLOR; CLI-recommendations cite this as the preferred choice over `nu-ansi-term`. [CITED: docs.rs/owo-colors] |
| `bindgen` | 0.72.x [VERIFIED: cargo search] | Auto-generate Rust FFI bindings from `include/deal.h` | Standard for C → Rust binding generation. Phase 1's `tests/ffi/Cargo.toml` already uses a hand-written binding; Phase 2 can adopt `bindgen` in `build.rs` for the `cli/` crate to avoid drift as `deal.h` grows from 6 to 8 exports. |
| `anyhow` | 1.0.102 [VERIFIED: cargo search] | Application-level error handling (CLI internals) | Standard. Use for CLI plumbing; do NOT use for diagnostics surfaced to users — those go through structured `Diagnostic` JSON. |
| `thiserror` | 2.0.18 [VERIFIED: cargo search] | Library-level error types (schema-registry, IR loader) | Standard. Pairs with `anyhow` (`anyhow` in main; `thiserror` for typed library errors). |

### Supporting (Rust test infrastructure)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `insta` | 1.47.x [VERIFIED: cargo search → 1.47.2] | Golden-fixture management with `cargo insta review` workflow | Plan 02-04 golden fixtures (REQ #6). Use `assert_snapshot!` for byte-exact comparison; pair with manual schema-validation assertions. |
| `libc` | 0.2.x (NOT `1.0.0-alpha`) [SUS: cargo search shows `libc = "1.0.0-alpha.3"` as latest; production code should stay on 0.2.x] | Raw FFI primitives if `bindgen` output needs adjusting | Optional — only if hand-tuning FFI types. |

### Core (Zig compiler core — Phase 2 new modules)

| Module | Lines (est.) | Purpose | Foundation |
|--------|------|---------|------------|
| `src/sema.zig` | ~800–1200 | All 6 semantic checks + `.deal/index.json` writer (D-20) | Walks AST built by `parser_deal.zig`/`parser_dealx.zig`. Allocates into the parser arena. Emits diagnostics via `diagnostics.zig` `DiagnosticCollector` using new E2xxx codes. |
| `src/ir.zig` | ~400–600 | DEAL IR v0 node types + traversal API (D-22..D-27) | Tagged-union design mirrors `ast.zig` `Node`. ID = `[]const u8` (arena-owned fully-qualified path string). |
| `src/lowering.zig` | ~600–900 | AST → IR pass (REQ #3) | Walks AST + sema symbol table. Same arena. |
| `src/fmt.zig` | ~500–800 | Pretty-printer (D-21, REQ #7) | Walks AST with attached comments. Writes to `std.io.Writer`. |

Extensions:
- `src/diagnostics.zig` — extend `Codes` namespace with E2xxx allocations per D-33 (additive — Phase 1 codes unchanged).
- `src/json.zig` — extend AST emitter for `leading_comments` / `trailing_comments` / `doc_comment` fields (D-30). Add IR JSON emitter for `deal_ir_json` body.
- `src/lib.zig` — add `deal_ir_json` and `deal_format` C ABI exports (D-22, D-21). Total exports: 6 → 8.
- `src/lexer.zig` — emit `//`, `/* */`, `/** */` as tokens instead of skipping (D-28..D-29).
- `src/parser_deal.zig` + `src/parser_dealx.zig` — attach comments to declaration nodes (D-28).
- `src/ast.zig` — add `leading_comments: []Comment`, `trailing_comments: []Comment`, `doc_comment: ?DocComment` to declaration Node payload variants (D-28..D-29).
- `include/deal.h` — hand-update with two new symbols (mirrors Phase 1 length-prefixed UTF-8 pattern per D-11).
- `build.zig` — add `phase-2-gate` + `phase-2-gate-fresh` build steps (D-35) reusing `scripts/verify-fresh-worktree.sh`.

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `jsonschema` 0.46 | `boon` 0.6 [VERIFIED: cargo search] | `boon` is also draft-2020-12-capable, simpler API, smaller download numbers; recommended as the documented fallback if `jsonschema` hits a 126K-line `SysML.json` perf or correctness issue. [CITED: github.com/santhosh-tekuri/boon] |
| `codespan-reporting` 0.13 [VERIFIED] | `miette` 7.6 [VERIFIED] or `ariadne` 0.6 [VERIFIED] | `codespan-reporting` is the minimal-dependency choice and matches Rust compiler tooling style. `miette` is fancier (syntect highlighting) but pulls more deps and has no first-class JSON output mode. `ariadne` is single-message-per-diagnostic — less ergonomic for D-32's multi-secondary-span shape. **Recommendation: hand-roll the human-mode renderer in `cli/src/render.rs` using `owo-colors`** — Phase 1 already emits structured `Diagnostic` JSON (code/severity/message/span/secondary_spans/fix_it/notes); the rendering is straightforward and avoids dragging a heavy diagnostic crate that doesn't quite fit D-32's envelope. Use `codespan-reporting` only if hand-rolling proves too painful. |
| `bindgen` in `build.rs` | Hand-written `extern "C"` declarations | Phase 1's `tests/ffi/Cargo.toml` uses hand-written bindings (only 6 functions). Phase 2 adds 2 more (8 total). Hand-written is fine; `bindgen` becomes worthwhile only if `deal.h` evolves further or if the team wants automatic type-mismatch detection at build time. **Recommendation: stick with hand-written for Phase 2 — fewer build dependencies, simpler reproducibility.** |
| `serde_json` `preserve_order` feature | Plain `serde_json` (alphabetical default) | `preserve_order` (feature) uses `IndexMap` → preserves insertion order. **Do NOT enable** — alphabetical-key invariant (D-18) is enforced by leaving `preserve_order` OFF. |
| `clap` derive | `clap` builder | Derive is more concise and matches recent Rust idioms; builder is more flexible for runtime-defined subcommands. **Recommendation: derive** — Phase 2 has 4 static subcommands. |

**Installation:**
```bash
# Run inside cli/ crate after Cargo.toml is created in Plan 02-01:
cargo add clap --features derive
cargo add serde --features derive
cargo add serde_json
cargo add jsonschema --no-default-features  # offline-by-construction
cargo add anstream
cargo add owo-colors
cargo add anyhow
cargo add thiserror
cargo add --dev insta --features json
```

**Version verification:** All versions above were obtained from `cargo search <name>` against crates.io on 2026-05-21 [VERIFIED]. Phase 2 should pin via `Cargo.lock` (commit it for the binary crate per Cargo guidance).

## Package Legitimacy Audit

> Verified per the Package Legitimacy Gate protocol. `slopcheck install <pkgs> --ecosystem crates.io` produced clean `[OK]` for every recommended package; `cargo search` confirmed each entry exists on crates.io with the expected metadata.

| Package | Registry | Latest version (as of 2026-05-21) | slopcheck | Disposition |
|---------|----------|------------------------------------|-----------|-------------|
| `clap` | crates.io | 4.6.1 | [OK] | Approved |
| `serde` | crates.io | 1.0.228 | [OK] | Approved |
| `serde_json` | crates.io | 1.0.149 | [OK] | Approved |
| `jsonschema` | crates.io | 0.46.5 | [OK] | Approved |
| `boon` | crates.io | 0.6.1 | [OK] | Approved (fallback) |
| `anstream` | crates.io | 1.0.0 | [OK] | Approved |
| `anstyle` | crates.io | 1.0.14 | [OK] | Approved (transitively via `anstream`) |
| `owo-colors` | crates.io | 4.3.0 | [OK] | Approved |
| `codespan-reporting` | crates.io | 0.13.1 | [OK] | Approved (only if hand-rolling proves painful) |
| `miette` | crates.io | 7.6.0 | [OK] | Approved (alternative; not recommended) |
| `ariadne` | crates.io | 0.6.0 | [OK] | Approved (alternative; not recommended) |
| `insta` | crates.io | 1.47.2 | [OK] | Approved |
| `bindgen` | crates.io | 0.72.1 | [OK] | Approved (optional) |
| `anyhow` | crates.io | 1.0.102 | [OK] | Approved |
| `thiserror` | crates.io | 2.0.18 | [OK] | Approved |
| `libc` | crates.io | 0.2.x (stay; 1.0.0-alpha.3 exists but is pre-release) | [OK] | Approved with version pin |

**Packages removed due to slopcheck [SLOP] verdict:** none.
**Packages flagged as suspicious [SUS]:** none in the Rust ecosystem. (Initial slopcheck run incorrectly targeted PyPI for crates that exist on crates.io — re-run with `--ecosystem crates.io` returned all clean.)

## Architecture Patterns

### System Architecture Diagram

```
                              tests/showcase/  (19 files)
                                     │
                                     ▼
                          ┌──────────────────────────┐
              (Zig)       │  src/lexer.zig           │     emits //, /* */, /** */
                          │  src/parser_*.zig        │     tokens (D-28)
                          │  src/ast.zig             │
                          └────────────┬─────────────┘
                                       │  AST
                                       │  (with attached
                                       │   leading_comments,
                                       │   trailing_comments,
                                       │   doc_comment per D-28..D-29)
                          ┌────────────┴─────────────┐
                          ▼                          ▼
                  ┌──────────────┐         ┌──────────────────┐
        (Zig)     │ src/sema.zig │ (Zig)   │ src/fmt.zig      │  (Zig — REQ #7)
                  │ 6 checks     │         │ pretty-printer   │
                  │ + index.json │         │ walks AST + cmts │
                  └──────┬───────┘         └────────┬─────────┘
                         │  symbol table             │  bytes
                         ▼                           │
                  ┌──────────────────┐               │
        (Zig)     │ src/lowering.zig │               │
                  │ AST + symbols    │               │
                  │   → IR v0        │               │
                  └────────┬─────────┘               │
                           │  IR graph                │
                           ▼                          │
                  ┌────────────────────┐              │
        (Zig)     │ src/ir.zig         │              │
                  │ traversal API      │              │
                  │ walk/find/refs/... │              │
                  └────────┬───────────┘              │
                           │                          │
                           │  deal_ir_json() (D-22)   │  deal_format() (D-21)
   ═══════════════════════ FFI boundary ═══════════════════════════════════════
                           │  length-prefixed         │
                           │  UTF-8 (D-11)            │
                           ▼                          ▼
                          ┌─────────────────────────────────┐
              (Rust)      │  cli/src/main.rs (clap v4.6)   │
                          │  ──────────────────────────────  │
                          │  parse  check  fmt  build       │
                          │  --json --color --verbose       │
                          └──┬──────┬──────┬─────────┬──────┘
                             │      │      │         │
                             ▼      ▼      ▼         ▼
                       AST  Diag.   fmt.   ┌──────────────────────┐
                       JSON env.    bytes  │ cli/src/sysml_v2.rs  │
                       (D-32)             │ IR → SysML v2 JSON   │
                                          └──────────┬───────────┘
                                                     │
                                                     ▼
                                        ┌──────────────────────────┐
                                        │ cli/src/schema_registry. │
                                        │ rs (jsonschema 0.46 with │
                                        │  custom Retrieve)        │
                                        │  ↳ spec/references/...   │
                                        │      omg-sysml-v2/       │
                                        │      omg-kerml-v1/       │
                                        └──────────┬───────────────┘
                                                   │  validated
                                                   ▼
                              tests/showcase/build/sysml-v2/
                                  showcase.sysml-v2.json (D-24, one file)
                                                   │
                                                   ▼
                              External viewer (D-36 priority order)
                              Eclipse SysON 2025.x (recommended) →
                              OpenMBEE Pilot → IncQuery → Cameo →
                              fallback generic JSON-Schema viewer
```

### Component Responsibilities

| Component | File / Module | Owns |
|-----------|---------------|------|
| Lexer (Phase 1 + comment emit) | `src/lexer.zig` | Tokenization; in Phase 2 emits `//`, `/* */`, `/** */` as `Tag.comment_line` / `Tag.comment_block` / `Tag.doc_comment` tokens. |
| Parser (Phase 1 + comment attach) | `src/parser_deal.zig`, `src/parser_dealx.zig`, `src/parser.zig` | Recursive-descent + Pratt; in Phase 2 attaches comments to declaration nodes per D-28. |
| AST | `src/ast.zig` | Tagged-union `Node`. In Phase 2 each declaration variant gains `leading_comments`, `trailing_comments`, `doc_comment`. |
| Diagnostics | `src/diagnostics.zig` | `Diagnostic` record + `DiagnosticCollector` + `Codes` namespace. In Phase 2 the `Codes` namespace extends with E2xxx entries. |
| Sema | `src/sema.zig` (NEW) | 6 checks + `.deal/index.json` writer. Allocates in parser arena. |
| IR | `src/ir.zig` (NEW) | Node types + traversal API (`walk`, `find`, `references`, `children`, `parent`). |
| Lowering | `src/lowering.zig` (NEW) | AST → IR transformation. |
| Formatter | `src/fmt.zig` (NEW) | AST → canonical source bytes. |
| C ABI shim | `src/lib.zig` | 6 → 8 exported symbols: existing six + `deal_ir_json` + `deal_format`. |
| JSON emitter | `src/json.zig` | Alphabetical-key emitter for AST, IR, and `.deal/index.json`. |
| CLI binary | `cli/src/main.rs` (NEW) | `clap` setup, subcommand dispatch, calls to FFI, output rendering. |
| SysML v2 emitter | `cli/src/sysml_v2.rs` (NEW) | IR JSON → SysML v2 JSON. |
| Schema registry | `cli/src/schema_registry.rs` (NEW) | `jsonschema` `Retrieve` impl mapping OMG `$id` URLs → local `spec/references/*.json`. |
| Renderer | `cli/src/render.rs` (NEW) | Diagnostic → ANSI-colored text + source snippet (human mode). |

### Recommended Project Structure

```
deal/
├── src/                         # Zig core (Phase 1 + Phase 2 additions)
│   ├── ast.zig                  # ★ modified — comment fields on declarations
│   ├── lexer.zig                # ★ modified — emits comment tokens
│   ├── parser_deal.zig          # ★ modified — comment attachment
│   ├── parser_dealx.zig         # ★ modified — comment attachment
│   ├── parser.zig
│   ├── diagnostics.zig          # ★ modified — E2xxx codes
│   ├── json.zig                 # ★ modified — AST cmt fields + IR emitter
│   ├── lib.zig                  # ★ modified — 8 C ABI exports
│   ├── sema.zig                 # ★ NEW
│   ├── ir.zig                   # ★ NEW
│   ├── lowering.zig             # ★ NEW
│   └── fmt.zig                  # ★ NEW
├── include/
│   └── deal.h                   # ★ modified — 2 new symbols
├── cli/                         # ★ NEW Rust crate
│   ├── Cargo.toml
│   ├── build.rs                 # (optional) bindgen for deal.h
│   └── src/
│       ├── main.rs              # clap dispatch
│       ├── ffi.rs               # extern "C" decls or bindgen output
│       ├── render.rs            # diagnostic rendering
│       ├── schema_registry.rs   # offline jsonschema Retrieve
│       └── sysml_v2.rs          # IR → SysML v2
├── tests/
│   ├── showcase/                # 19 files (symlink → spec/examples/showcase)
│   ├── snapshots/ast/           # 19 AST JSON snapshots (regenerated for comments)
│   ├── golden/sysml-v2/         # ★ NEW — 5–8 hand-written .deal + expected.json
│   │   ├── 01-part-def.deal
│   │   ├── 01-part-def.expected.json
│   │   ├── ...
│   │   └── 08-dealx-composition.dealx
│   ├── regressions/sema/        # ★ NEW — 6 fixtures (one per blocking check)
│   │   ├── 01-name-resolution.deal     # → exits 1 with E2000..E2099
│   │   ├── 02-type-check.deal          # → exits 1 with E2100..E2199
│   │   ├── 03-multiplicity.deal        # → exits 1 with E2200..E2299
│   │   ├── 04-specializes.deal         # → exits 1 with E2300..E2349
│   │   ├── 05-trace.deal               # → exits 1 with E2350..E2399
│   │   └── 06-import.deal              # → exits 1 with E2400..E2499
│   ├── unit/                    # existing Zig unit tests
│   │   ├── determinism_parse_twice.zig
│   │   ├── determinism_lower_twice.zig  # ★ NEW — mirrors parse_twice pattern
│   │   ├── property_span_containment.zig
│   │   └── ...
│   ├── ffi/                     # existing Rust FFI test (gate_all_19)
│   └── malformed/               # 50-file corpus
├── spec/                        # git submodule
│   ├── examples/showcase/
│   ├── references/
│   │   ├── omg-sysml-v2/SysML.json    # 126,290 lines
│   │   ├── omg-kerml-v1/KerML.json    # 42,284 lines
│   │   └── omg-kerml-v1/KerML-Model-Interchange.json
│   └── ir/v0/                   # ★ NEW
│       ├── schema.json          # normative JSON Schema for IR v0
│       └── README.md            # Markdown reference for IR v0
├── .planning/
│   └── decisions/
│       └── ADR-deal-ir-v0.md    # ★ NEW (D-27)
├── scripts/
│   └── verify-fresh-worktree.sh # reused from Phase 1.5
├── build.zig                    # ★ modified — phase-2-gate + phase-2-gate-fresh
└── Cargo.toml                   # ★ NEW workspace root [workspace] members = ["cli", "tests/ffi"]
```

### Pattern 1: Cargo workspace layout (claude's discretion item)

**What:** Single workspace `Cargo.toml` at `deal/Cargo.toml` with members `["cli", "tests/ffi"]`. Both crates share lockfile + target dir.

**Why:** Phase 1's `tests/ffi/Cargo.toml` is standalone; converting it into a workspace member (a) shares dependency resolution (both crates depend on `libc`), (b) gives a single `cargo test --workspace` command for CI, (c) keeps the Cargo.lock at repo root for reproducibility.

**Example:**
```toml
# deal/Cargo.toml
[workspace]
resolver = "2"
members = ["cli", "tests/ffi"]

[workspace.dependencies]
clap = { version = "4.6", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
thiserror = "2"
# pinned at workspace level — both crates use the same versions
```

**Source:** [Cargo workspace docs](https://doc.rust-lang.org/cargo/reference/workspaces.html) [VERIFIED: standard Cargo idiom].

### Pattern 2: Gofmt-style comment attachment (CLAUDE'S DISCRETION → recommended rules)

**What:** Two-pass attachment:
1. **Lexer pass:** emit `//`, `/* */`, `/** */` as separate token kinds (today's lexer skips them). Each comment token carries its `Span`. **Doc comments (`/** */`) get a separate token tag** because they're promoted to a typed field (D-29), not added to `leading_comments`.
2. **Parser pass:** after parsing a declaration node, run an `attachComments(node)` helper:
   - **Leading comments:** all comment tokens between the previous declaration's end (or file start) and this declaration's start, EXCLUDING comments separated from this declaration by a blank line. Comments separated by a blank line attach as `trailing_comments` on the previous declaration instead.
   - **Trailing comments:** comment tokens on the SAME physical line as the declaration's last token (after the closing brace or semicolon). Only one trailing comment per line; subsequent same-line comments would be a lex error (unreachable in current grammar).
   - **Doc comment:** if a `/** */` token immediately precedes a declaration with no blank line in between, it's promoted to `doc_comment: ?DocComment`. Otherwise it stays as a leading comment (in the rare floating-doc-comment edge case).

**Gofmt rule (canonical citation):**

> "The Go compiler will recognize comments that immediately precede (no blank lines or whitespace) declarations as belonging to that declaration. Comments are maintained as a sequential list of comment groups attached to the ast.File node, and additionally, comments that are identified as doc strings are attached to declaration nodes."
>
> A `CommentGroup` represents a sequence of comments with no other tokens and no empty lines between, and grouped comments are treated as a single larger comment.
>
> Source: [Go go/ast docs](https://pkg.go.dev/go/ast) + [Go doc comment guide](https://tip.golang.org/doc/comment) [CITED]

**Translation to DEAL:**

| Original layout | Attachment |
|-----------------|------------|
| `// comment\npart def X { ... }` (no blank line) | `leading_comments = ["// comment"]` on `X`. |
| `// comment\n\npart def X { ... }` (blank line) | `trailing_comments = ["// comment"]` on whatever precedes (or floating if at file start). |
| `part def X { ... } // trailer` (same line) | `trailing_comments = ["// trailer"]` on `X`. |
| `/** doc */\npart def X { ... }` (no blank) | `doc_comment = "/** doc */"` on `X`; NOT in `leading_comments`. |
| `/** doc */\n\npart def X { ... }` (blank line) | Floating doc-comment (rare); remains as a `NodeKind.doc_comment` arm at file top level. |

**When to use:** This is the attachment policy for every declaration node. No optional modes.

**Anti-pattern to avoid:** Free-floating trivia list (D-28 rejected option) — comments live in a sidecar token stream and the printer must merge two streams. **CONTEXT.md explicitly rejects this**; it's listed in `<deferred>` as a fallback architecture only if structural attachment fails to round-trip.

### Pattern 3: AST → canonical source pretty-printing with comments

**What:** Walk the AST and emit canonical bytes. At each declaration node, emit:
1. `leading_comments` (each one on its own line, indented to the declaration's column).
2. `doc_comment` (if present, immediately above the declaration with no blank line).
3. The declaration itself, recursively.
4. `trailing_comments` (single same-line trailer if present).

Use a `std.io.Writer` accepting `bytes` and tracking current indentation. Don't try to be Wadler/Hughes-clever — the showcase's deepest nesting is ~6 levels and canonical line breaks are deterministic from the grammar.

**Key invariants (round-trip):**
- Whitespace is normalized: one space around binary operators, four-space indentation, exactly one blank line between top-level declarations.
- String literals: normalize single quotes to double quotes per LM-3 / SD-12.
- Trailing commas: deal grammar does NOT permit trailing commas in arg lists (E0122 enforced by Phase 1.5). `deal fmt` MUST NOT introduce them.
- Comments are emitted verbatim — DO NOT re-flow `/** */` content (Phase 2 ships unmodified comment text; future ADR may add doc-comment formatting).

**Reference precedent:** Rustfmt's comment preservation:
> "The formatting of comments is distinct from the formatting of AST nodes. While most code is rewritten by visiting AST nodes, comments are often captured via 'missed spans' — the gaps in the source code between AST nodes."
>
> Source: [rustfmt deepwiki](https://deepwiki.com/rust-lang/rustfmt/4.8-comments-and-documentation) [CITED]

**Phase 2 simplification:** because D-28 attaches comments STRUCTURALLY to AST nodes (not as missed spans), the printer never needs to scan the original source for "missed" comments — it just emits the attached fields in order. Rustfmt's missed-spans complexity is avoided.

**When to use:** This is the only fmt strategy. No alternatives in scope.

### Pattern 4: Name resolution + scope graph for a SysML-like language

**What:** Two-pass approach in `src/sema.zig`:
1. **Pass A — populate symbol table.** Walk all AST roots (every `.deal` and `.dealx` file in the workspace). For each declaration, compute its fully-qualified path (`package.def_name.member_name...`) per D-23 and insert into a single global `std.StringHashMap(*SymbolEntry)`. Track each symbol's source span + kind + scope.
2. **Pass B — resolve references.** Walk the AST again. For each `identifier`, `member_access`, `type_annotation`, `<<specializes>>` target, `@trace` target, and `import` element, look up the qualified path in the symbol table:
   - If `import` brought it in unqualified, look up under the import path's namespace.
   - If member access (`a.b.c`), resolve `a` then walk children.
   - On miss, emit E2000..E2099 (name resolution).

**SysML v2 alignment (cited):**

> "A qualified name of a model element is prepended by the name of its owner followed by a double colon (`::`)." — note: SysML uses `::`, DEAL uses `.` per D-23/PS-2. Translation: when emitting SysML v2 JSON, rewrite `.` → `::` in qualified names if any SysML field requires the `::` form.
>
> "If a name is present in multiple namespaces, a reference will be resolved to the innermost occurrence of it."
>
> Source: [Sensmetry — Advent of SysML v2 Lesson 8](https://sensmetry.com/advent-of-sysml-v2-lesson-8-packages-and-names/) [CITED]

**Scope graph theory:** A scope graph is the standard abstraction (Néron et al., ESOP 2015 — [theory of name resolution paper](https://web.cecs.pdx.edu/~apt/esop15.pdf)) [CITED]. For Phase 2 we don't need a full scope graph — DEAL's nesting is a tree (packages → defs → members) and resolution is straightforward path lookup with shadowing. Reserve scope-graph machinery for Phase 3 LSP if cross-file go-to-definition needs it.

**`.deal/index.json` shape (sketch — recommend the planner finalize in Plan 02-02):**
```json
{
  "v": 1,
  "deal_version": "0.1.0-phase-2",
  "elements": {
    "vehicle.electrical.BatteryPack": {
      "kind": "part_def",
      "span": [657, 1562],
      "source_file": "packages/vehicle/electrical.deal",
      "imports": ["interfaces.HVDCPort", "interfaces.CoolantPort"],
      "members": ["hvOut", "chargeIn", "canBus", "modules", ...],
      "specializes": ["interfaces.ThermallyManaged"],
      "traced_by": ["model.EVPlatformTraces.REQ_BAT_001"]
    },
    ...
  },
  "imports_graph": [
    { "from": "vehicle.electrical", "to": "interfaces", "kind": "barrel_glob" },
    ...
  ]
}
```
Alphabetical-key ordering throughout (D-18). Used by Phase 3 LSP for workspace-wide symbol resolution.

**When to use:** Mandatory for all 6 sema checks.

### Pattern 5: Cycle detection for `<<specializes>>` and `@trace`

**What:** During Pass B, when resolving `<<specializes>>` chains, maintain a visited-set of qualified names. If the chain re-enters a visited node, emit E2300..E2349 with both spans (the current `<<specializes>>` clause AND the original definition that started the cycle). Same pattern for `@trace` chains.

**When to use:** Every `<<specializes>>` resolution. The showcase doesn't currently have cycles, but a malformed user model will, and silent infinite-loop in sema is the worst failure mode.

### Pattern 6: IR v0 node shape (LOCKED by D-22..D-27; this section documents the recommended concrete layout)

**What:** Tagged-union over node kinds, with a uniform envelope:

```zig
// src/ir.zig (sketch — Plan 02-03 task)
pub const IrNode = struct {
    id: []const u8,          // fully-qualified path (D-23) — arena-owned
    kind: NodeKind,          // tagged-union discriminator
    span: ast.Span,          // carried over from AST node
    source_file: []const u8, // arena-owned; for diagnostic attach points
    payload: Payload,
    // edges live in a separate adjacency list, not on the node, to keep
    // serialization tree-shaped despite the graph being graph-shaped.
};

pub const NodeKind = enum {
    package,
    part_def, port_def, action_def, state_def,
    requirement_def, constraint_def, attribute_def,
    item_def, interface_def, connection_def, flow_def,
    allocation_def, use_case_def, need_def,
    // usages (instances)
    part_usage, port_usage, attribute_usage,
    // composition primitives
    system, subsystem, connect, expose,
    // traceability
    traceability_block, allocate, satisfy, validate,
    // metadata
    annotation,
};

pub const Edge = struct {
    src: []const u8,         // source qualified path
    dst: []const u8,         // target qualified path
    kind: EdgeKind,
};

pub const EdgeKind = enum {
    specializes, redefines, subsets,
    satisfies, allocated_to, derives_from,
    imports, traces,
    contains,                // parent → child
    connects_via,            // composition wiring
    carries,                 // flow content
};
```

**JSON shape (top-level — D-22, D-18 alphabetical):**
```json
{
  "v": 1,
  "edges": [
    {"dst": "interfaces.ThermallyManaged", "kind": "specializes", "src": "vehicle.electrical.BatteryPack"},
    ...
  ],
  "elements": {
    "vehicle.electrical.BatteryPack": {
      "kind": "part_def",
      "source_file": "packages/vehicle/electrical.deal",
      "span": [657, 1562],
      "payload": { ... part-def-specific fields, alphabetically keyed ... }
    },
    ...
  },
  "ir_version": "v0"
}
```

**Agent metadata envelope (Claude's discretion):** the `@confidence`, `@rationale`, `@assumes`, `@concerns` annotations attached to declaration nodes should appear in the IR as a **typed `agent_metadata` field** on the IR node payload:
```json
"agent_metadata": {
  "assumes": ["string lit", ...],     // alphabetical key order; arrays preserve source order
  "concerns": ["string lit", ...],
  "confidence": 0.85,                  // f64
  "rationale": "string lit"
}
```
Null fields are omitted (D-18). All four are optional. This shape round-trips the showcase's actual usage (verified by `grep -E '@(confidence|rationale|assumes|concerns)' tests/showcase/`).

**Simulation-binding representation (Claude's discretion):** `@simulation:<<computes>>` and `@simulation:<<validates against>>` should appear as a `simulation_bindings: []SimBinding` array on the IR node payload:
```json
"simulation_bindings": [
  {
    "operator": "computes",          // or "validates against"
    "target": "thermalRunaway",
    "tool": "ANSYS Fluent",          // from annotation body
    "equation": "T_cell > 150°C ...",
    "fidelity": "CFD",
    "entry": null                    // present when the annotation body sets it
  }
]
```
Phase 2's SysML emitter ignores this field; Phase 5 sim integration reads it.

**When to use:** This is the recommended payload shape. Plan 02-03 should finalize field names; nothing here is locked beyond D-22..D-27.

### Pattern 7: AST → IR lowering (arena allocation + ID interning)

**What:**
1. Allocate IR nodes into the same arena as the parser (D-02 per-handle arena).
2. ID strings are computed once during the package-walk pass and **interned** in a `std.StringHashMap([]const u8)` — every IR node, edge, and reference uses the interned slice. Saves memory and lets `find(id)` be a pointer comparison after interning.
3. Walk AST in tree order. For each declaration, compute its qualified path, intern, allocate an IR node, copy span + source_file, lower the payload.
4. Walk a second time to emit edges (now that all IDs exist).
5. Build adjacency-list indexes for `references(id)` (incoming edges) and `children(id)` (outgoing `contains` edges).

**Memory model:** the entire IR for a workspace lives in one arena. `deal_free()` releases it all. No GC, no individual `free()` calls.

**JSON emission cost:** `std.json.stringify` (Zig 0.16's `std.json.Stringify`) writes to a `std.Io.Writer`. For the showcase (~3K IR elements, ~5K edges expected), serialization is well under 100ms on a modern laptop — not a bottleneck. For Phase 6+'s "multi-million element model" concern, a streaming writer pattern works the same way [CITED: zig.guide/standard-library/json].

**When to use:** All AST → IR work goes through this module.

### Pattern 8: SysML v2 element mapping table

**What:** DEAL IR element → SysML v2 element correspondences (from CONTEXT.md REQ #4 acceptance + SysML 2.0 final-1.0 spec):

| DEAL IR kind | SysML v2 type | Notes |
|--------------|---------------|-------|
| `package` | `Package` | `qualifiedName` ← DEAL ID with `.` → `::` translation. |
| `part_def` | `PartDefinition` | `name` ← DEAL name; `ownedRelationship` carries members. |
| `port_def` | `PortDefinition` | Lives inside owning element's `ownedRelationship`. |
| `port_usage` (`in`/`out`/`inout`) | `PortUsage` | `direction` ← DEAL direction. |
| `part_usage` | `PartUsage` | Used inside compositions. |
| `attribute_usage` | `AttributeUsage` | `defaultValue` ← lowered expression. |
| `requirement_def` | `RequirementDefinition` | `text` attribute + nested `concernUsage`. |
| `<<specializes>>` edge | `Specialization` in `ownedRelationship` | `general` = target, `specific` = source. |
| `<<redefines>>` edge | `Redefinition` | inside `ownedRelationship`. |
| `<<subsets>>` edge | `Subsetting` | ditto. |
| `<<satisfies>>` edge (from `@trace`) | `Dependency` with `kind = "satisfy"` | OR `SatisfyRequirementUsage` — recommend `Dependency` for Phase 2; revisit. |
| `connect` (`.dealx`) | `ConnectionUsage` with `connectorEnd` array | `via=` → connector definition; `carrying=` → flow. |
| `expose` (`.dealx`) | `ExposedFeature` / port re-export | Maps to `PortUsage` with redirection. |
| `traceability_block` | `Package` containing a set of `Dependency`s | One-to-one with `[<traceability>]` blocks. |

**SysML JSON envelope:**
- Each element gets `@id` (UUID v4 — synthesized by the emitter; not the DEAL qualified name) AND `elementId` equal to `@id` [CITED: SysON release notes — "@id and elementId attributes have the same value in JSON serialization"].
- DEAL's stable qualified-path ID lives in the SysML element's `qualifiedName` field — the human-readable name, NOT `@id`.
- `@type` is the SysML class name (e.g., `"PartDefinition"`).
- `ownedRelationship` is the canonical containment array.
- Alphabetical key ordering inside each JSON object (SysON convention, matches D-18 [CITED]).

**Source:** [SysON Release Notes 2025.8](https://doc.mbse-syson.org/syson/v2025.8.0/user-manual/release-notes/release-notes.html), [SysON APIs Cookbook 2025.2](https://doc.mbse-syson.org/syson/v2025.2.0/developer-guide/api-cookbook.html), [SysML-v2-Pilot-Implementation KerML2JSON.java](https://github.com/Systems-Modeling/SysML-v2-Pilot-Implementation/blob/master/org.omg.kerml.xtext/src/org/omg/kerml/xtext/util/KerML2JSON.java) — all [CITED].

**When to use:** This is the contract surface for `cli/src/sysml_v2.rs`. Plan 02-04 should finalize edge cases by experimentation against SysON.

### Pattern 9: Insta golden-fixture workflow

**What:**
1. Each fixture in `tests/golden/sysml-v2/<id>-<name>.deal` paired with `<id>-<name>.expected.json`.
2. Integration test in `cli/tests/golden.rs` reads each `.deal`, runs `deal build --target sysml-v2 --validate` programmatically, compares output to `expected.json`:
   - Byte-exact match (REQ #6 acceptance) — `insta::assert_snapshot!(output, @expected)`.
   - Schema-valid against bundled SysML.json (REQ #6 acceptance) — explicit assertion.
3. To update expected outputs (intentional codegen change): run `cargo insta review` → review diffs → accept → commit.
4. CI runs `cargo insta test --check` (or just `cargo test`); never accepts new snapshots automatically.

**Why insta:** Cited as the de-facto standard Rust snapshot framework [CITED: rustprojectprimer.com/testing/snapshot.html]. Inline snapshot syntax (`@"..."`) works for short outputs; file-based (`.snap` files in `snapshots/`) is the better choice for our multi-KB SysML JSON.

**When to use:** Every golden fixture in Plan 02-04.

### Anti-Patterns to Avoid

- **Hand-rolling a JSON Schema validator.** The OMG SysML v2 schema is 126K lines and uses every draft-2020-12 feature (composition, `$ref`, `$dynamicRef`, `if`/`then`/`else`, `oneOf`, etc.). Use `jsonschema` or `boon`; do not hand-roll.
- **Re-flowing comment text in `deal fmt`.** Doc-comment formatting is its own design problem; emit the original bytes verbatim for Phase 2 and defer to a future ADR.
- **Re-using the same C ABI handle across threads.** Phase 1's handle pattern is single-threaded. CLI subcommands are sequential; LSP (Phase 3) will need per-request handles. Do NOT introduce shared handles.
- **Letting `serde_json::Value` reorder by accident.** Default `serde_json` uses `BTreeMap` (alphabetical). If `preserve_order` feature is enabled by a transitive dependency, the alphabetical-key invariant (D-18) breaks silently — add a deny-list rule in `cli/Cargo.toml` or a test that asserts `"compose":"...","a":"..."` mapping iterates `a, compose` not `compose, a`.
- **Storing comments on every AST node.** D-28 says declaration/statement nodes only. Adding `leading_comments` to every `Expression` node would bloat the AST 2-3× without benefit.
- **Using `unwrap()` in CLI main.** All error paths should return `Result<(), CliError>` and let `main` translate to exit code 0/1/2 (D-34).
- **`println!` for diagnostic output.** Use `anstream::stdout()` / `anstream::stderr()` so `--color` works correctly on non-TTY destinations.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Offline JSON Schema validation against 126K-line OMG schemas | Custom validator | `jsonschema` 0.46+ with custom `Retrieve` | Draft-2020-12 has dozens of subtle features; the OMG schemas exercise them. Maintenance cost of hand-rolling is months. |
| CLI arg parsing + subcommand help | `std::env::args` munging | `clap` v4.6 derive | Standard. Built-in `--help` generation, suggestions, completion files. |
| TTY detection + ANSI handling | `is_terminal` + manual strip | `anstream` 1.0 | Already handles `NO_COLOR`/`CLICOLOR`/`CLICOLOR_FORCE`. |
| C → Rust FFI binding maintenance | Hand-edit declarations on every `deal.h` change | `bindgen` 0.72 in `build.rs` (optional) | If `deal.h` grows beyond Phase 2's 8 symbols, bindgen pays off. For Phase 2 it's a judgment call. |
| Comment grouping rules | Custom blank-line state machine | gofmt-cited rule (see Pattern 2) | The rule is well-trodden; novel rules generate edge-case bugs that block round-trip. |
| AST → IR ID generation | Counter-based opaque IDs | Fully-qualified path strings (D-23, LOCKED) | Path strings are human-readable in diagnostics + URL-suffixable in SysML `$id`. |
| Diagnostic JSON re-serialization | Re-emit from typed Rust structs | Pass through Zig's `deal_diagnostics_json()` output verbatim, then wrap envelope | Phase 1 already emits the canonical shape; re-serializing risks key-order drift. |
| Schema $ref resolution from OMG URLs | URL parser + file matcher | `jsonschema` `Retrieve` trait | Trait is exactly designed for this. |
| Snapshot diffs for golden fixtures | Manual `diff` invocations | `insta` 1.47 | `cargo insta review` UI is dramatically better than reading raw diffs. |

**Key insight:** Phase 2's value-add is the DEAL-specific layer (sema rules, IR shape, SysML mapping). Every adjacent concern (CLI parsing, schema validation, snapshots, ANSI handling) has a mature crate; using them frees engineering budget for the parts that ARE novel.

## Common Pitfalls

### Pitfall 1: `serde_json` non-alphabetical key emission

**What goes wrong:** A transitive dep enables the `preserve_order` feature, swapping `BTreeMap` for `IndexMap`. JSON output then preserves *insertion* order, breaking D-18's alphabetical-key invariant for the SysML emitter and `--json` envelope. Snapshot tests start failing intermittently.

**Why it happens:** `preserve_order` is feature-additive across the workspace — if ANY crate in the dependency graph turns it on, every `serde_json` user gets it.

**How to avoid:**
1. In `cli/Cargo.toml`, explicitly state `serde_json = "1"` (no features).
2. Add a test in `cli/tests/key_order.rs` that serializes `{"z": 1, "a": 2}` and asserts the output is `{"a":2,"z":1}`.
3. Use `cargo tree -e features | grep preserve_order` in CI; flag if any dep pulls it.

**Warning signs:** Golden fixtures fail with key-order-only diffs; snapshot tests pass locally but fail in CI (or vice versa).

### Pitfall 2: Comment-attachment one-shot snapshot refresh churn

**What goes wrong:** Plan 02-01 regenerates all 19 AST snapshots when `leading_comments`/`trailing_comments`/`doc_comment` fields appear. If the parser changes structural fields too (off-by-one in a span calculation), the unrelated structural diff hides inside the comment-field diff and ships unreviewed.

**Why it happens:** Snapshot tests pass holistically; reviewers tend to skim large diffs.

**How to avoid:**
1. Land comment attachment as a **single dedicated commit** (D-30) — no other source changes.
2. Add a temporary `diff-only-new-fields` script in `scripts/` that strips lines NOT containing `leading_comments`, `trailing_comments`, or `doc_comment` and asserts the remaining structural diff is EMPTY.
3. Property-span-containment test (Phase 1.5 deliverable) MUST pass on the refreshed snapshots — that's the structural integrity check.

**Warning signs:** Snapshot regen produces a diff larger than expected (more than `19 × 4` lines per file at minimum); span values change in unexpected places.

### Pitfall 3: Arena lifetime crossing the FFI boundary

**What goes wrong:** `deal_ir_json()` returns a pointer into the arena. The CLI holds onto that pointer past a `deal_free()` call. Use-after-free crash or silent corruption.

**Why it happens:** The C ABI ownership rule (D-11) is correct — caller must call `deal_free` only after they've copied any returned bytes. The CLI may forget this when running multiple subcommands in sequence or in tests.

**How to avoid:**
1. In `cli/src/ffi.rs`, wrap every `*const u8 + len` returned by Zig in a `Cow<'_, [u8]>` that copies on construction. The caller never sees a borrowed slice tied to the handle.
2. Mirror Phase 1.5 Plan 04's "gpa.dupe-before-free lifetime pattern" — copy in Rust before any `deal_free` call.
3. Run `cli/tests/` under `valgrind` (Linux CI) or `leaks` (macOS) at least once per gate.

**Warning signs:** Intermittent test failures; valgrind reports use-after-free.

### Pitfall 4: `jsonschema` performance on 126K-line `SysML.json`

**What goes wrong:** First validation call takes >5s because the schema parse + compile is expensive. CI hot path gets slow; developers stop running `--validate` locally.

**Why it happens:** Schema compilation is O(schema size). 126K lines is large.

**How to avoid:**
1. Compile the validator ONCE per CLI invocation (lazy-static or `OnceCell`).
2. For tests, share a `lazy_static` schema-loaded validator across all golden fixtures.
3. Benchmark with `cargo bench` before declaring the validator "done" — target < 1s amortized per build for the showcase.
4. If `jsonschema` is too slow, fall back to `boon` and benchmark there.

**Warning signs:** `cargo test` runtime exceeds 30s; CI gate timeouts.

### Pitfall 5: SysON file-upload import strict-mode rejection

**What goes wrong:** Generated SysML v2 JSON validates against `SysML.json` offline but fails SysON's import with a `"missing required field: @id format = uuid"` error — because DEAL uses qualified-path IDs and didn't generate UUIDs.

**Why it happens:** The OMG schema requires `@id` to be `format: "uuid"`. DEAL's stable IDs are path strings. The emitter must synthesize UUIDs at emit time.

**How to avoid:**
1. The SysML emitter generates a stable UUID-from-hash for each element: `Uuid::v5(NAMESPACE_OID, qualified_path.as_bytes())`. This gives the same UUID for the same DEAL element across runs (determinism), and a real UUID per the schema.
2. The DEAL qualified path goes into the SysML element's `qualifiedName` field; `@id` and `elementId` get the UUID.
3. Plan 02-04 should include one fixture that does a round-trip: `deal build` → SysON file-upload → SysON re-export → compare element count.

**Warning signs:** Offline validation passes, viewer-import fails.

### Pitfall 6: Golden fixture rot

**What goes wrong:** Six months in, the SysML v2 spec ships a micro-update (e.g., adds a new optional field to `PartDefinition`). DEAL's emitter intentionally adopts it. All 5–8 golden fixtures need their `expected.json` updated. The `cargo insta review` workflow handles this, but only if the team remembers to run it.

**Why it happens:** Spec evolution is real and ongoing.

**How to avoid:**
1. Pin the bundled SysML schema to a specific OMG release version in `spec/references/omg-sysml-v2/SHA256SUMS` (already present). Bumping the schema is a deliberate ADR.
2. CI runs both byte-exact AND schema-valid checks. If schema validates but bytes don't match (a SysML emitter change), the diff appears in `cargo insta review` and gets human-reviewed.
3. Document the snapshot-update workflow in `tests/golden/sysml-v2/README.md`.

**Warning signs:** Golden fixtures stale; team uses `--no-verify` or similar bypass.

### Pitfall 7: Fresh-worktree gate drift (inherited from Phase 1.5 incident)

**What goes wrong:** A developer's main checkout has a local file/symlink that masks a missing committed artifact (Phase 1.5's `tests/showcase` symlink incident). `phase-2-gate` passes locally; `phase-2-gate-fresh` fails in CI.

**Why it happens:** Local dev environments accrete state.

**How to avoid:**
1. Inherit the ADR-phase-1.5-fresh-worktree-verification.md invariant.
2. Add NEW Phase 2 files to the same fresh-worktree gate (`scripts/verify-fresh-worktree.sh` already does this for the whole tree).
3. CI's authoritative command is `zig build phase-2-gate-fresh`, NOT `zig build phase-2-gate`.

**Warning signs:** Test passes locally for one developer, fails for everyone else.

### Pitfall 8: CLI exit code conflation

**What goes wrong:** A panic or FFI shape mismatch is reported as exit 1 (user-visible error) instead of exit 2 (internal error). CI sees "expected error" and continues. The bug ships.

**Why it happens:** Rust's `?` operator + `anyhow::Error` collapses different error categories into one.

**How to avoid:**
1. Use a typed `CliError` enum at the top level. Distinguish `User(BlockingDiagnostic)` from `Internal(anyhow::Error)`.
2. `main` matches on the enum and returns 0, 1, or 2 per D-34.
3. Add a test that asserts `cli_main_with_args(["deal", "--bogus-arg-not-a-thing"])` returns exit 1 (clap rejection — user error), and that a deliberate `panic!()` in a subcommand returns exit 2 (or aborts — but at least not 1).

**Warning signs:** CI test "expected exit code 1" passes when the binary actually crashed.

## Runtime State Inventory

> SKIPPED — this is a greenfield phase. No rename, refactor, migration, or string-replacement. Categories: (1) stored data: none; (2) live service config: none; (3) OS-registered state: none; (4) secrets/env vars: none; (5) build artifacts: existing `zig-out/` and `.zig-cache/` are unaffected by Phase 2's additions. Verified by: phase scope description + `git status` clean main checkout.

## Code Examples

### Example 1: `clap` v4.6 derive subcommands with global flags

```rust
// cli/src/main.rs
// Source: https://docs.rs/clap/latest/clap/_derive/index.html [CITED]
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "deal", version, about = "DEAL language compiler driver")]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Emit machine-readable JSON instead of human-readable output.
    #[arg(long, global = true)]
    json: bool,

    /// When to use color output.
    #[arg(long, global = true, default_value = "auto")]
    color: ColorMode,

    /// Increase output verbosity.
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Parse source files and emit AST JSON.
    Parse { paths: Vec<std::path::PathBuf> },
    /// Run semantic checks; exit 1 on any blocking diagnostic.
    Check { paths: Vec<std::path::PathBuf> },
    /// Format source files in place (or stdin → stdout).
    Fmt { paths: Vec<std::path::PathBuf> },
    /// Build a target (SysML v2 JSON for Phase 2).
    Build {
        #[arg(long)]
        target: BuildTarget,
        /// Run offline schema validation on the output.
        #[arg(long)]
        validate: bool,
        paths: Vec<std::path::PathBuf>,
    },
}

#[derive(ValueEnum, Clone)]
enum BuildTarget { SysmlV2 }

#[derive(ValueEnum, Clone, Copy)]
enum ColorMode { Auto, Always, Never }

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    // initialize anstream::stdout / stderr per cli.color
    match run(cli) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) if e.is_user() => std::process::ExitCode::from(1),
        Err(_) => std::process::ExitCode::from(2),
    }
}
```

### Example 2: Zig IR JSON emitter using the `std.json.Stringify` API

```zig
// src/json.zig (sketch — Plan 02-03 task)
// Source: zig std lib lib/std/json/Stringify.zig [VERIFIED: cargo search not applicable;
// confirmed from Zig 0.16.0 stdlib installed locally]
const std = @import("std");
const ir = @import("ir.zig");

pub fn writeIrJson(writer: *std.Io.Writer, doc: *const ir.Document) !void {
    var s = std.json.writeStream(writer.*, .{ .whitespace = .minified });
    try s.beginObject();
    // D-18: alphabetical key order
    try s.objectField("edges");
    try s.beginArray();
    for (doc.edges) |edge| {
        try s.beginObject();
        try s.objectField("dst");      try s.write(edge.dst);
        try s.objectField("kind");     try s.write(@tagName(edge.kind));
        try s.objectField("src");      try s.write(edge.src);
        try s.endObject();
    }
    try s.endArray();
    try s.objectField("elements");
    try writeElements(&s, doc.elements);
    try s.objectField("ir_version"); try s.write("v0");
    try s.objectField("v"); try s.write(1);
    try s.endObject();
}
```

### Example 3: SysML v2 element emit in Rust with UUID-from-path

```rust
// cli/src/sysml_v2.rs (sketch — Plan 02-04 task)
use serde_json::{json, Value, Map};
use uuid::Uuid;

const SYSML_NAMESPACE: Uuid = Uuid::from_u128(0x0e72_4d54_7e1a_4b32_9e8c_8e62_2d8f_1234);

fn deal_id_to_uuid(qualified_path: &str) -> Uuid {
    Uuid::new_v5(&SYSML_NAMESPACE, qualified_path.as_bytes())
}

fn emit_part_definition(elem: &IrElement) -> Value {
    let uuid = deal_id_to_uuid(&elem.id).to_string();
    // BTreeMap → alphabetical keys (D-18); SysON expects this convention too.
    let mut obj = Map::new();
    obj.insert("@id".to_string(), json!(uuid));
    obj.insert("@type".to_string(), json!("PartDefinition"));
    obj.insert("elementId".to_string(), json!(uuid));  // SysON: @id == elementId
    obj.insert("name".to_string(), json!(elem.local_name()));
    obj.insert("ownedRelationship".to_string(), emit_owned_members(&elem));
    obj.insert("qualifiedName".to_string(), json!(elem.id.replace('.', "::")));
    Value::Object(obj)
}
```

### Example 4: Offline `jsonschema` validator wiring

```rust
// cli/src/schema_registry.rs (sketch — Plan 02-04 task)
// Source: docs.rs/jsonschema 0.46 [CITED]
use jsonschema::Retrieve;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct LocalBundleRetriever {
    bundle: HashMap<String, Value>,
}

impl LocalBundleRetriever {
    pub fn new(spec_references_dir: &PathBuf) -> anyhow::Result<Self> {
        let mut bundle = HashMap::new();
        let sysml: Value = serde_json::from_slice(
            &std::fs::read(spec_references_dir.join("omg-sysml-v2/SysML.json"))?
        )?;
        let kerml: Value = serde_json::from_slice(
            &std::fs::read(spec_references_dir.join("omg-kerml-v1/KerML.json"))?
        )?;
        // Walk schemas and index every `$id` value → its Value subtree.
        index_schema_ids(&sysml, &mut bundle);
        index_schema_ids(&kerml, &mut bundle);
        Ok(Self { bundle })
    }
}

impl Retrieve for LocalBundleRetriever {
    fn retrieve(&self, uri: &jsonschema::Uri<String>)
        -> Result<Value, Box<dyn std::error::Error + Send + Sync>>
    {
        self.bundle.get(uri.as_str()).cloned()
            .ok_or_else(|| format!("Schema not in offline bundle: {uri}").into())
    }
}

pub fn make_validator(retriever: LocalBundleRetriever, root_schema: &Value)
    -> anyhow::Result<jsonschema::Validator>
{
    Ok(jsonschema::draft202012::options()
        .with_retriever(retriever)
        .build(root_schema)?)
}
```

### Example 5: Insta golden-fixture test

```rust
// cli/tests/golden_sysml_v2.rs (sketch — Plan 02-04 task)
// Source: insta.rs [CITED]
use insta::assert_snapshot;

#[test]
fn part_def_emits_expected_sysml_v2() {
    let source = include_str!("../tests/golden/sysml-v2/01-part-def.deal");
    let actual = run_deal_build(source).expect("build failed");
    // Compare against tests/golden/sysml-v2/01-part-def.expected.json
    let expected = include_str!("../tests/golden/sysml-v2/01-part-def.expected.json");
    assert_snapshot!("part_def_sysml_v2", actual, expected);
    // Schema validation is a separate assertion:
    let validator = make_validator_for_tests();
    let parsed: serde_json::Value = serde_json::from_str(&actual).unwrap();
    validator.validate(&parsed).expect("schema validation failed");
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `nu-ansi-term` / `ansi_term` for colors | `owo-colors` + `anstream` | 2022+ Rust CLI guide consensus | Smaller binary, NO_COLOR support, no_std-friendly. |
| `clap` builder API | `clap` derive API | clap 3.0 / 4.0 (2022+) | Less boilerplate; matches recent Rust idioms. |
| Hand-rolled JSON Schema validators | `jsonschema` v0.46 with `Retrieve` trait | jsonschema 0.18+ (2024+) | Custom `Retrieve` is the canonical offline pattern. |
| `error-chain` / hand-rolled errors | `thiserror` + `anyhow` split | 2020+ | Library errors = `thiserror`; app errors = `anyhow`. |
| `failure` crate | `thiserror` / `anyhow` | 2019+ | `failure` is unmaintained. |
| Custom snapshot testing | `insta` | 2019+ | De-facto Rust standard. |
| SysML v2 viewer = NoMagic / Cameo (commercial) | Eclipse SysON (open-source, web-based) | 2024+ Obeo + CEA | SysON aligns with OMG SysML 2.0 final-1.0, supports JSON file upload directly. |

**Deprecated/outdated:**
- `failure` crate (`thiserror`/`anyhow` replaced it).
- `ansi_term` (unmaintained; `owo-colors` replaced it).
- `jsonschema-valid` 0.5 (last release 2020; superseded by `jsonschema` 0.x).
- `error-chain` (superseded; in maintenance mode).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Eclipse SysON's file-upload import accepts SysML v2 JSON in the exact OMG schema shape (alphabetical keys, `@id == elementId`, UUID `@id`s) | Pattern 8, Pitfall 5 | Plan 02-06 viewer-smoke fails; may need to tune emitter or try fallback viewer. Mitigation: SysON has a public Eclipse-Foundation REST API spec; if file-upload fails, use POST to `/elements`. |
| A2 | `jsonschema` 0.46 successfully compiles and validates against 126K-line `SysML.json` in < 5 s on a modern dev laptop | Standard Stack, Pitfall 4 | Plan 02-04 falls back to `boon` (already audited). Benchmark first thing in Plan 02-04. |
| A3 | The 19 showcase files do not produce IR cycles in `<<specializes>>` / `@trace` graphs | Pattern 5 | Acceptable — cycle detection still runs, just produces no diagnostics. |
| A4 | Comment attachment with the gofmt-style blank-line rule covers 100% of the 19 showcase files without round-trip loss | Pattern 2, Pitfall 2 | Fallback to free-floating trivia list (D-28 deferred option). Plan 02-01 acceptance gate catches this via round-trip test against the full corpus. |
| A5 | The OMG SysML v2 schemas (`SysML.json`, `KerML.json`) use absolute `$id` URLs that `jsonschema`'s `Retrieve` can be wired against — i.e., no relative `$ref`s that bypass the retriever | Pattern 8, Pitfall 4 | Investigated `head` of `spec/references/omg-sysml-v2/SysML.json`: confirmed first `$id` is `https://www.omg.org/spec/SysML/20250201/AcceptActionUsage` and `$ref` uses absolute URL. [VERIFIED via direct file inspection] |
| A6 | `serde_json` default (no `preserve_order`) emits keys alphabetically when serializing a `Map`-backed `Value` | Pattern 1, Pitfall 1 | If wrong, alphabetical-key invariant breaks. Test in `cli/tests/key_order.rs` (Pitfall 1) catches at CI time. |
| A7 | `bindgen` is NOT required for Phase 2 — hand-written `extern "C"` declarations for 8 symbols are maintainable | Alternatives Considered | Maintenance cost is low; only risk is `deal.h` evolving without the FFI module being updated. Mitigated by `tests/ffi/gate_all_19` exercising all symbols. |
| A8 | Phase 2 ships comment-free IR per D-25; doc-comment lowering to SysML `documentation` is deferred to a follow-up ADR | Discretion items | If the SysML output looks visibly missing inline documentation against SysON's display, the team can open the ADR. Pitfall avoided by deferring rather than locking now. [ASSUMED — pending Phase 2 visual smoke test] |

**If user/team confirmation needed before execution:** A1 (viewer choice), A2 (jsonschema perf), A8 (doc-comment lowering decision) should be revisited at the IR-lock checkpoint (between Plan 02-03 and 02-04).

## Open Questions

1. **Should Plan 02-04 use a Cargo workspace OR a standalone `cli/Cargo.toml`?**
   - What we know: `tests/ffi/Cargo.toml` already exists standalone. Workspace would share lockfile.
   - What's unclear: Whether `tests/ffi/` needs to keep its standalone identity (it might be called from `zig build`'s test harness).
   - Recommendation: Workspace with both members; CI gate exercises both via `cargo test --workspace`.

2. **Should `deal_format()` write to a caller-provided file path OR return bytes?**
   - What we know: D-21 locks the symbol exists; CONTEXT.md leans toward "return bytes (LSP-friendly)".
   - What's unclear: Whether the Rust CLI prefers writing bytes itself (for `--in-place` flag support).
   - Recommendation: Return bytes via the length-prefixed UTF-8 out-param pattern (matches `deal_ast_json` exactly). CLI handles file I/O.

3. **Should the IR-lock checkpoint be a build-step OR a manual review?**
   - What we know: CONTEXT.md D-37 + SPEC.md REQ #9 say "recorded review in 02-VERIFICATION.md" — i.e., manual.
   - What's unclear: Whether a build step like `zig build phase-2-ir-checkpoint` (asserting the checkpoint timestamp exists) adds value.
   - Recommendation: Skip the build step; rely on `02-VERIFICATION.md`'s recorded outcome (per CONTEXT.md `<specifics>`).

4. **How should multi-target builds (sysml-v2, reqif, docs) share emitter code in `cli/src/`?**
   - What we know: Phase 4 adds reqif; Phase 6 adds docs. Phase 2 only ships sysml-v2.
   - What's unclear: Whether a `trait Emitter` abstraction in Phase 2 makes Phase 4's work easier or just adds premature abstraction.
   - Recommendation: Single-purpose `cli/src/sysml_v2.rs` in Phase 2; extract a trait IFF Phase 4's emitter starts duplicating code.

5. **Should `cargo bench` benchmarks run in Phase 2's gate?**
   - What we know: SysML validation perf is Pitfall 4.
   - What's unclear: Whether perf regressions warrant a build-time gate.
   - Recommendation: Track perf in `02-VERIFICATION.md` as a measured number; don't gate yet. Gate in Phase 3 or 4 if perf becomes a problem.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Zig | Core compilation | ✓ | 0.16.0 [VERIFIED: `zig version`] | — |
| Cargo + rustc | Rust CLI | ✓ | 1.93.0 [VERIFIED: `rustc --version`] | — |
| `git` | Submodule + worktree | (assumed) | — | — |
| SysML.json + KerML.json | Offline validator | ✓ | OMG 2025-02-01 release [VERIFIED: file count 126,290 + 42,284 lines] | — |
| Eclipse SysON (viewer) | REQ #9 viewer smoke | ✗ | 2025.x | Try OpenMBEE Pilot → IncQuery → generic JSON viewer (D-36 fallback chain). SysON has Docker image available [CITED: mbse-syson.org]. |
| `slopcheck` (Python) | Package legitimacy audit | ✓ | 0.6.1 [VERIFIED] | None — but Phase 2's `[OK]` audit is complete. |
| `insta` (Rust dev-dep) | Golden fixtures | (installs via Cargo) | 1.47.2 | — |
| `node`/`npm` | None for Phase 2 | n/a | — | — |
| Network access at test time | NO — must be disabled per REQ #5 acceptance | n/a | — | — |

**Missing dependencies with no fallback:** none — all hard dependencies are present.
**Missing dependencies with fallback:** SysON (viewer) — fallback chain documented per D-36.

## Validation Architecture

> Per CONTEXT.md Step 4 + nyquist_validation requirement. This phase has dense validation needs (parse/format round-trip, type-check correctness, schema validation, golden fixtures, IR conformance, fresh-worktree gate).

### Test Framework
| Property | Value |
|----------|-------|
| Zig framework | `zig build test` + `std.testing` (existing pattern) |
| Rust framework | `cargo test` + `insta` for snapshots |
| Config file | `build.zig` (Zig side); `Cargo.toml` (Rust side, Phase 2 NEW) |
| Quick run command | `zig build test -Dtest-filter=<name>` (Zig) / `cargo test <name>` (Rust) |
| Full suite command | `zig build phase-2-gate` (per-tree) / `zig build phase-2-gate-fresh` (fresh-worktree, CI-authoritative) |
| Phase gate | `zig build phase-2-gate-fresh` exits 0 + viewer-smoke recorded + IR-lock checkpoint recorded |

### Observation Surfaces

| Surface | What's observed | How |
|---------|----------------|-----|
| AST JSON (existing) | 19 snapshots stable across runs | `tests/snapshots/ast/*.json` byte-equality |
| Diagnostic JSON (existing) | Phase 1's `Diagnostic` shape | `tests/unit/diag_json_roundtrip.zig` |
| IR JSON (NEW) | Lowering output for all 19 files | `tests/snapshots/ir/*.json` byte-equality + JSON-Schema validation against `spec/ir/v0/schema.json` |
| `.deal/index.json` (NEW) | Sema's index for the showcase | `tests/snapshots/index.json` byte-equality |
| SysML v2 JSON (NEW) | Per-fixture byte match + schema valid | 5–8 golden fixtures in `tests/golden/sysml-v2/` |
| Diagnostics for malformed sema input (NEW) | E2xxx code emission | 6 regression fixtures in `tests/regressions/sema/` (one per blocking check) |
| Determinism (extended) | IR byte-identical across two lowering passes | `tests/unit/determinism_lower_twice.zig` (mirrors `determinism_parse_twice.zig`) |
| Round-trip fmt (NEW) | parse → format → parse → identical AST + every comment preserved | `tests/unit/fmt_roundtrip.zig` per showcase file |
| C ABI surface (extended) | 8 exports visible in `nm -gU libdeal.a` | Phase 1.5 grep gate extended from 6 to 8 |
| `--color` flag (NEW) | ANSI escapes absent on `never`, present on `always` | `cli/tests/color_flag.rs` |
| `--json` envelope (NEW) | Validates against published schema | `cli/tests/json_envelope.rs` |
| CLI exit codes (NEW) | 0/1/2 per D-34 | `cli/tests/exit_codes.rs` |
| Viewer-smoke (NEW, manual) | SysON imports `showcase.sysml-v2.json` | `02-VERIFICATION.md` recorded outcome + optional screenshot |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| REQ-phase-2-1-semantic-analyzer | 6 blocking checks run | unit | `zig build test -Dtest-filter=sema` | ❌ Wave 0 — new |
| REQ-phase-2-1-semantic-analyzer | E2xxx codes emitted for malformed inputs | regression | `zig build test -Dtest-filter=sema.regressions` | ❌ Wave 0 — new |
| REQ-phase-2-1-semantic-analyzer | `.deal/index.json` written | integration | `cargo test --test sema_index` | ❌ Wave 0 — new |
| REQ-phase-2-2a-ir-v0 | IR JSON validates against `spec/ir/v0/schema.json` | unit | `zig build test -Dtest-filter=ir.conformance` | ❌ Wave 0 — new |
| REQ-phase-2-2b-ir-lowering | Lowering produces IR for all 19 files | snapshot | `zig build test -Dtest-filter=lowering.snapshot` | ❌ Wave 0 — new |
| REQ-phase-2-2b-ir-lowering | Two passes byte-identical | determinism | `zig build test -Dtest-filter=determinism.lower_twice` | ❌ Wave 0 — new |
| REQ-phase-2-3-sysml-v2-codegen | Emitter walks IR, produces SysML JSON | integration | `cargo test --test sysml_v2_emit` | ❌ Wave 0 — new |
| REQ-phase-2-3a-offline-validator | `--validate` passes on showcase output, network disabled | integration | `cargo test --test offline_validate -- --no-capture` (uses `unshare -n` or equivalent on CI) | ❌ Wave 0 — new |
| REQ-phase-2-3b-golden-fixtures | 5–8 fixtures byte-exact + schema-valid | snapshot | `cargo insta test` | ❌ Wave 0 — new |
| REQ-phase-2-4-formatter | All 19 round-trip with comments preserved | unit | `zig build test -Dtest-filter=fmt.roundtrip` | ❌ Wave 0 — new |
| REQ-phase-2-4-formatter | Span containment after refresh | property | `zig build test -Dtest-filter=property.span_containment` | ✅ Phase 1.5 exists; re-runs |
| REQ-phase-2-5-cli-shell | `deal --version` works | integration | `cargo test --test cli_basic` | ❌ Wave 0 — new |
| REQ-phase-2-5-cli-shell | Each subcommand runs on showcase | integration | `cargo test --test cli_subcommands` | ❌ Wave 0 — new |
| REQ-phase-2-5-cli-shell | `--json` validates against envelope schema | integration | `cargo test --test json_envelope` | ❌ Wave 0 — new |
| REQ-phase-2-5-cli-shell | `--color={auto,always,never}` honored | integration | `cargo test --test color_flag` | ❌ Wave 0 — new |
| REQ-phase-2-gate | Phase 1 / 1.5 regression protection | gate | `zig build phase-1.5-gate` (subset of phase-2-gate) | ✅ exists |
| REQ-phase-2-gate | Fresh-worktree gate | gate | `zig build phase-2-gate-fresh` | ❌ Wave 0 — new |
| REQ-phase-2-gate | Viewer-import smoke | manual | record in `02-VERIFICATION.md` | ❌ end of phase |
| REQ-phase-2-gate | IR-lock checkpoint recorded | manual | record in `02-VERIFICATION.md` after Plan 02-03 | ❌ mid-phase |

### Reference Workloads

| Workload | Source | Used by |
|----------|--------|---------|
| 19-file showcase | `tests/showcase/` → `spec/examples/showcase/` (submodule symlink) | every gate |
| 6 sema regression fixtures | `tests/regressions/sema/` (NEW) | Plan 02-02 acceptance |
| 5–8 golden SysML fixtures | `tests/golden/sysml-v2/` (NEW) | Plan 02-04 acceptance |
| 50-file malformed corpus | `tests/malformed/` (Phase 1 deliverable) | regression — sema must not panic on these |
| Phase 1 determinism corpus | same 19-file showcase | extended to IR via `determinism_lower_twice` |

### Sampling Rate
- **Per task commit:** `zig build test -Dtest-filter=<scope>` (fast — single module) and/or `cargo test --test <name>` (Rust side).
- **Per wave merge:** `zig build phase-1.5-gate && cargo test --workspace`.
- **Phase gate:** `zig build phase-2-gate-fresh` (CI-authoritative) + viewer-smoke record + IR-lock checkpoint record.

### Wave 0 Gaps

- [ ] `tests/regressions/sema/` directory with 6 hand-crafted fixtures (one per blocking check) — required by Plan 02-02 acceptance.
- [ ] `tests/golden/sysml-v2/` directory with 5–8 `.deal` + `expected.json` pairs — required by Plan 02-04 acceptance.
- [ ] `tests/unit/determinism_lower_twice.zig` — required by REQ #3 acceptance.
- [ ] `tests/unit/fmt_roundtrip.zig` — required by REQ #7 acceptance.
- [ ] `tests/unit/sema_index_smoke.zig` (or Rust integration in `cli/tests/`) — required by REQ #1 acceptance.
- [ ] `cli/Cargo.toml` + `cli/src/main.rs` skeleton — required by Plan 02-01 acceptance.
- [ ] `cli/tests/golden_sysml_v2.rs` + `insta` configuration — required by REQ #6 acceptance.
- [ ] `cli/tests/color_flag.rs`, `cli/tests/json_envelope.rs`, `cli/tests/exit_codes.rs`, `cli/tests/cli_subcommands.rs` — required by REQ #8 acceptance.
- [ ] `build.zig` additions for `phase-2-gate` and `phase-2-gate-fresh` build steps — required by REQ #9 acceptance.
- [ ] `.planning/decisions/ADR-deal-ir-v0.md` + `spec/ir/v0/schema.json` + `spec/ir/v0/README.md` — required by REQ #2 acceptance (the spec submodule).

## Sources

### Primary (HIGH confidence)

- **`spec/references/omg-sysml-v2/SysML.json`** (local, 126,290 lines) — confirmed first `$id` is `https://www.omg.org/spec/SysML/20250201/AcceptActionUsage`; `$ref`s are absolute URLs; structure validates the offline `Retrieve` approach.
- **`spec/references/omg-kerml-v1/KerML.json`** (local, 42,284 lines) — companion schema; sibling `$ref` target.
- **Zig 0.16.0 installed locally** (`zig version` → `0.16.0`) — confirms language reference availability.
- **`src/diagnostics.zig`** — Phase 1 `Diagnostic` shape, `Codes` namespace, `DiagnosticCollector` API — basis for E2xxx extension (D-33).
- **`src/ast.zig:393`** — existing `DocComment` struct — basis for D-29 attached field.
- **`include/deal.h`** — Phase 1 C ABI shape; `deal_ir_json` / `deal_format` mirror this pattern.
- **`build.zig`** — `phase-1-gate` + `phase-1.5-gate` + `phase-1.5-gate-fresh` patterns to mirror.
- **[clap 4.x derive docs](https://docs.rs/clap/latest/clap/_derive/index.html)** — derive API.
- **[jsonschema docs.rs](https://docs.rs/jsonschema)** — `Retrieve` trait + draft-2020-12 + offline pattern.
- **[insta.rs](https://insta.rs/)** — snapshot-testing workflow.
- **[Eclipse SysON docs 2025.x](https://doc.mbse-syson.org/syson/v2025.8.0/user-manual/release-notes/release-notes.html)** — file-upload import + JSON serialization conventions (`@id == elementId`, alphabetical keys).
- **[SysML v2 textual format SysON docs](https://doc.mbse-syson.org/syson/v2025.4.0/user-manual/features/import-export-textual.html)** — file-upload semantics.

### Secondary (MEDIUM confidence — WebSearch verified by docs)

- **[crates.io / cargo search]** — Rust crate versions all verified via `cargo search` on 2026-05-21.
- **[Stranger6667/jsonschema GitHub](https://github.com/Stranger6667/jsonschema)** — `Retrieve` trait surface.
- **[santhosh-tekuri/boon GitHub](https://github.com/santhosh-tekuri/boon)** — fallback validator.
- **[Go `go/ast` docs](https://tip.golang.org/doc/comment)** — comment-attachment rule.
- **[rustfmt comments deep wiki](https://deepwiki.com/rust-lang/rustfmt/4.8-comments-and-documentation)** — formatter comment handling.
- **[Sensmetry — Advent of SysML v2 Lesson 8](https://sensmetry.com/advent-of-sysml-v2-lesson-8-packages-and-names/)** — SysML v2 qualified name semantics.
- **[SysML-v2-Pilot-Implementation KerML2JSON.java](https://github.com/Systems-Modeling/SysML-v2-Pilot-Implementation/blob/master/org.omg.kerml.xtext/src/org/omg/kerml/xtext/util/KerML2JSON.java)** — JSON serialization reference impl.
- **[Rust CLI Recommendations — Managing colors](https://rust-cli-recommendations.sunshowers.io/managing-colors-in-rust.html)** — `anstream` / `owo-colors` rationale.
- **[Rust Project Primer — Snapshot Testing](https://www.rustprojectprimer.com/testing/snapshot.html)** — `insta` workflow.

### Tertiary (LOW confidence — single source, flagged for validation at Plan 02-04)

- Eclipse SysON's exact file-upload accepted-shape contract (need Plan 02-06 to empirically verify by uploading a generated fixture).
- IncQuery's SysML v2 toolset commercial-trial accessibility in 2026 — D-36 second-priority fallback that may or may not be available.

## Metadata

**Confidence breakdown:**
- Standard stack (Rust crates): HIGH — every crate verified via `cargo search` + slopcheck `[OK]` on crates.io.
- Architecture (Zig modules, FFI, JSON shape): HIGH — extends well-understood Phase 1 patterns; no new architectural pattern.
- Comment-attachment grammar: HIGH on rule citation (gofmt is canonical); MEDIUM on showcase edge-case coverage (round-trip test will catch).
- SysML v2 emitter contract: MEDIUM — Pattern 8 mapping table is mostly direct from spec; ConnectionUsage and traceability `Dependency` shapes need Plan 02-04 confirmation against SysON.
- Validation architecture: HIGH — every required test type maps to an established Zig or Rust pattern.
- Common pitfalls: HIGH — all eight pitfalls were observed in Phase 1 or are documented industry experience.
- Open questions: HIGH (questions are clearly stated); answers themselves are recommendations, not verified.

**Research date:** 2026-05-21
**Valid until:** 2026-06-21 (30 days; the Rust ecosystem is stable but `jsonschema` 0.46.x may release patch updates; SysON 2025.x has a quarterly release cadence — re-verify if Plan 02-04 doesn't land by then).

---

*Phase: 02-prove-the-pipeline*
*Research conducted: 2026-05-21*
*Sources: CONTEXT.md (37 locked decisions), SPEC.md (9 requirements), DISCUSSION-LOG.md (rejected alternatives), PROJECT.md (tech-stack constraints), REQUIREMENTS.md (REQ IDs), ROADMAP.md (phase neighbors), STATE.md (state machine), Phase 1 + 1.5 SUMMARIES, local Zig 0.16.0 + `cargo search` + official docs (jsonschema, clap, insta, SysON, gofmt, rustfmt).*
