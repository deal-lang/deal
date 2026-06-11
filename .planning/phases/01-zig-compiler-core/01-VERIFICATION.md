---
phase: 01-zig-compiler-core
verified: 2026-05-20T00:00:00Z
status: passed
score: 5/5 must-haves verified
overrides_applied: 0
---

# Phase 1: Zig Compiler Core Verification Report

**Phase Goal:** `deal parse showcase/*.deal showcase/*.dealx` produces AST JSON for all 19 files, with zero panics and meaningful diagnostics on malformed input. Compiler is consumable from Rust via a C ABI.
**Verified:** 2026-05-20
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| #  | Truth                                                                                        | Status     | Evidence                                                                                                               |
|----|----------------------------------------------------------------------------------------------|------------|------------------------------------------------------------------------------------------------------------------------|
| 1  | Zero `.unknown` tokens across the 19-file showcase corpus                                    | ✓ VERIFIED | `lexer.snapshot` test in `tests/unit/lexer_snapshot.zig` asserts `std.mem.indexOf(u8, actual, "\"k\":\"unknown\"") == null` for all 19 files; orchestrator gate run confirms 24/24 tests pass including this test |
| 2  | 19 byte-stable AST snapshots (15 `.deal` + 4 `.dealx`)                                      | ✓ VERIFIED | `tests/snapshots/ast/` contains exactly 19 files (15 `*.deal.json` + 4 `*.dealx.json`); snapshots are substantive rich JSON (verified by spot-reading `traceability.dealx.json`); `parser_deal.snapshot` + `parser_dealx.snapshot` tests enforce byte stability |
| 3  | 50+ malformed-input corpus survives without panic; structured Diagnostic JSON at every emission | ✓ VERIFIED | `tests/malformed/` contains exactly 50 files; `recovery.corpus` test (`recovery_corpus.zig`) asserts `>=50` files, zero panics, and `>=60%` with diagnostics; orchestrator reports 50 files, 166 total diagnostics, 13 files with 0 diagnostics, 0 panics |
| 4  | Zero leaks across showcase + malformed via `std.testing.allocator`                           | ✓ VERIFIED | `c_abi.no_leaks` test in `tests/unit/c_abi_no_leaks.zig` iterates all 69 files (19 showcase + 50 malformed) via `deal_parse_internal(std.testing.allocator, ...)`, forces both JSON caches before `deal_free_internal`; orchestrator reports 69 files parsed and freed with zero leaks |
| 5  | Rust FFI `gate_all_19` passes; `nm` symbol table contains exactly 6 `deal_*` exports with zero `_internal` leaks | ✓ VERIFIED | `tests/ffi/tests/gate.rs::gate_all_19` passes per orchestrator (9/9 Rust tests pass including gate_all_19); `nm -gU zig-out/lib/libdeal.a` returns exactly 6 exports (`_deal_ast_json`, `_deal_diagnostics_count`, `_deal_diagnostics_json`, `_deal_free`, `_deal_has_errors`, `_deal_parse`); `nm | grep -E '_deal_(parse|free)_internal'` returns 0 |

**Score:** 5/5 truths verified

---

### Required Artifacts

| Artifact                                      | Expected                                                       | Status     | Details                                                                                       |
|-----------------------------------------------|----------------------------------------------------------------|------------|-----------------------------------------------------------------------------------------------|
| `deal/src/lib.zig`                            | C ABI exports, UTF-8 validation, arena ownership              | ✓ VERIFIED | 335 lines; contains `std.unicode.utf8ValidateSlice`, `Codes.e_invalid_utf8`, `Codes.e_source_too_large`, 5 `handle_ptr orelse return` null guards, `callconv(.c)` ×9 (6 exports + 3 helpers) |
| `deal/src/lexer.zig`                          | Full token set, four-mode dispatch                            | ✓ VERIFIED | 882 lines; `Mode` enum has `deal_def`, `dealx_outer`, `dealx_tag`, `dealx_expr_brace`; 47 grep-hits on token variants including `unknown`, `eof`, `identifier` |
| `deal/src/parser_deal.zig`                    | 87 `deal.ebnf` production recursive-descent + Pratt parser    | ✓ VERIFIED | 1,984 lines (non-stub); supports D-17 sync helpers `syncToStatement`/`syncToDefinition` |
| `deal/src/parser_dealx.zig`                   | 43 `dealx.ebnf` productions, stack-based tag balancing        | ✓ VERIFIED | 1,419 lines; `via=`/`carrying=` inline block parsing confirmed by `ffi_parse_dealx_real` asserting `k:comp_connect` |
| `deal/src/ast.zig`                            | Tagged-union AST, `CompConnect` with typed slots, `Span` as `extern struct` | ✓ VERIFIED | `CompConnect` struct has `from_expr`, `to_expr`, `via_expr`, `carrying_expr` fields; `Span = extern struct { start: u32, end: u32 }` present |
| `deal/src/diagnostics.zig`                    | `Diagnostic` with all fields, `Codes` namespace               | ✓ VERIFIED | Contains `Severity`, `SpanLabel`, `FixIt`, `Diagnostic`, `secondary_spans`, `fix_it`, `notes`, `DiagnosticCollector`, `Codes` (E0001..H0600) |
| `deal/src/json.zig`                           | Real `emitAst` + `emitDiagnostics` (not stubs)               | ✓ VERIFIED | 1,173 lines; "placeholder" labels in comments are stale Wave-0 labels — the actual `switch` arms for all composition node kinds are fully implemented with alphabetical field order (D-18) |
| `deal/include/deal.h`                         | Hand-written C header with doxygen, C++ guard, error-code table | ✓ VERIFIED | 8 `@brief` blocks (≥6 required); 2 `extern "C"` occurrences; error-code reference table present (E0001, E0100, E0300, E0400 all present) |
| `deal/tests/snapshots/ast/*.json`             | 19 AST snapshots (15 `.deal` + 4 `.dealx`)                   | ✓ VERIFIED | 15 `*.deal.json` + 4 `*.dealx.json` = 19 total; substantive rich JSON (not stubs) |
| `deal/tests/malformed/*.deal` + `*.dealx`     | ≥50 malformed-input corpus files                              | ✓ VERIFIED | Exactly 50 files |
| `deal/tests/unit/c_abi_invalid_utf8.zig`      | 5-case UTF-8 validation test                                  | ✓ VERIFIED | `test "c_abi.invalid_utf8"` declared; 5 cases (lone continuation, 0xFF, truncated 2-byte, overlong, valid+invalid mix) |
| `deal/tests/unit/c_abi_no_leaks.zig`          | Zero-leak test via `std.testing.allocator`                    | ✓ VERIFIED | `test "c_abi.no_leaks"` declared; 69-file corpus loop |
| `deal/tests/unit/c_abi_source_too_large.zig`  | Source-length overflow test                                   | ✓ VERIFIED | `test "c_abi.source_too_large"` declared |
| `deal/tests/ffi/tests/gate.rs`                | `gate_all_19` Phase 1 exit gate test                          | ✓ VERIFIED | `fn gate_all_19()` present; iterates all 19 showcase files |
| `deal/tests/ffi/tests/parse.rs`               | Extended FFI tests (5 real-parser tests + 2 legacy smoke)    | ✓ VERIFIED | 8 test functions confirmed: `ffi_smoke`, `ffi_dealx_mode_detection`, `ffi_parse_battery_real`, `ffi_parse_dealx_real`, `ffi_diagnostics_on_malformed`, `ffi_ast_json_cache_stable`, `ffi_invalid_utf8`, plus `gate_all_19` in gate.rs = 9 total Rust tests |
| `deal/build.zig`                              | `phase-1-gate` step, `-Dtest-filter` option, both test targets | ✓ VERIFIED | `b.step("phase-1-gate", ...)` at line 164; `"test-filter"` at line 24; `_all.zig` unit umbrella wired |

---

### Key Link Verification

| From                              | To                                     | Via                                     | Status     | Details                                                                               |
|-----------------------------------|----------------------------------------|-----------------------------------------|------------|---------------------------------------------------------------------------------------|
| `deal/build.zig`                  | `deal/src/lib.zig`                     | `b.createModule(.root_source_file = b.path("src/lib.zig"))` | ✓ WIRED    | Line 52 in build.zig |
| `deal/build.zig`                  | `deal/include/deal.h`                  | `b.installFile("include/deal.h", ...)`  | ✓ WIRED    | Present in build.zig |
| `deal/tests/ffi/build.rs`         | `deal/zig-out/lib/libdeal.a`           | `cargo::rustc-link-lib=static=deal`     | ✓ WIRED    | Rust tests link against built library; gate_all_19 passes |
| `deal/src/lib.zig deal_parse`     | `std.unicode.utf8ValidateSlice`        | UTF-8 guard before parser invocation    | ✓ WIRED    | Line 115 in lib.zig: `if (!std.unicode.utf8ValidateSlice(handle.source))` |
| `deal/tests/unit/c_abi_no_leaks.zig` | `deal/src/lib.zig deal_parse_internal` | `@import("lib").deal_parse_internal`   | ✓ WIRED    | `lib_as_module` wired into `unit_root` in build.zig (line 104-113) |
| Six Zig export fn signatures      | `deal/include/deal.h` prototypes       | Type-compatible C declarations          | ✓ WIRED    | `callconv(.c)` ×6 exports; nm confirms 6 exported symbols; zero `_internal` leak |

---

### Data-Flow Trace (Level 4)

| Artifact               | Data Variable    | Source                                      | Produces Real Data | Status      |
|------------------------|------------------|---------------------------------------------|--------------------|-------------|
| `deal_ast_json`        | `cached_ast_json` | `json.emitAst(handle.arena.allocator(), handle.ast_root, ...)` | Yes — AST tree from real parser | ✓ FLOWING  |
| `deal_diagnostics_json`| `cached_diag_json`| `json.emitDiagnostics(allocator, handle.diagnostics.items)` | Yes — actual Diagnostic structs | ✓ FLOWING  |
| AST snapshots          | `parser_deal.snapshot` / `parser_dealx.snapshot` | parser produces real AST nodes (1984 + 1419 line parsers) | Yes — rich JSON confirmed by spot-read | ✓ FLOWING  |

---

### Behavioral Spot-Checks

| Behavior                                   | Command                                                                                   | Result                                      | Status  |
|--------------------------------------------|-------------------------------------------------------------------------------------------|---------------------------------------------|---------|
| `libdeal.a` produced                       | `ls zig-out/lib/libdeal.a`                                                               | File exists                                 | ✓ PASS  |
| Exactly 6 exports, 0 internals leaked      | `nm -gU zig-out/lib/libdeal.a \| grep -o '_deal_[a-z_]*' \| sort -u`                    | 6 lines: `_deal_ast_json`, `_deal_diagnostics_count`, `_deal_diagnostics_json`, `_deal_free`, `_deal_has_errors`, `_deal_parse` | ✓ PASS  |
| Zero `_internal` leak in symbol table      | `nm -gU zig-out/lib/libdeal.a \| grep -E '_deal_(parse\|free)_internal' \| wc -l`       | 0                                           | ✓ PASS  |
| Zero `callconv(.C)` (uppercase) violations | `grep -c 'callconv(\.C)' src/*.zig`                                                      | All 0 (11 files)                            | ✓ PASS  |
| 19 AST snapshots present                   | `ls tests/snapshots/ast/*.json \| wc -l`                                                 | 19                                          | ✓ PASS  |
| 15 `.deal` + 4 `.dealx` split              | `ls *.deal.json \| wc -l` / `ls *.dealx.json \| wc -l`                                  | 15 / 4                                      | ✓ PASS  |
| Exactly 50 malformed corpus files          | `ls tests/malformed/*.deal tests/malformed/*.dealx \| wc -l`                             | 50                                          | ✓ PASS  |
| 22 Zig unit tests registered               | Count of `test "..."` declarations in `tests/unit/`                                      | 22                                          | ✓ PASS  |
| 9 Rust FFI tests registered                | Count of `#[test]` in `tests/ffi/tests/`                                                 | 9                                           | ✓ PASS  |

---

### Probe Execution

All probes were executed by the orchestrator at HEAD=b481608. This verifier independently cross-checked artifacts, symbols, file counts, and code substance.

| Probe                                                          | Command                                             | Result                         | Status |
|----------------------------------------------------------------|-----------------------------------------------------|--------------------------------|--------|
| Full Zig test suite                                            | `zig build test`                                    | 24/24 tests passed             | PASS   |
| Rust FFI test suite                                            | `cargo test --manifest-path tests/ffi/Cargo.toml`  | 9/9 passed                     | PASS   |
| Phase 1 gate (Rust)                                            | `cargo test ... gate_all_19`                        | 1/1 ok                         | PASS   |
| Symbol table check                                             | `nm -gU zig-out/lib/libdeal.a \| grep _deal_`       | Exactly 6 exports, 0 _internal | PASS   |
| No-leaks across 69 files                                       | `zig build test -Dtest-filter=c_abi.no_leaks`       | 69 files, 0 leaks              | PASS   |
| Recovery corpus                                                | `zig build test -Dtest-filter=recovery.corpus`      | 50 files, 166 diags, 0 panics  | PASS   |

---

### Requirements Coverage

| Requirement              | Source Plan(s) | Description                                                              | Status           | Evidence                                                                                     |
|--------------------------|----------------|--------------------------------------------------------------------------|------------------|----------------------------------------------------------------------------------------------|
| REQ-phase-1-foundation   | 01-01 (all 6 plans deliver collectively) | Build the Zig compiler core; exit: AST JSON for all 19 files, zero panics, meaningful diagnostics | ✓ SATISFIED   | gate_all_19 proves exit criterion: 19 showcase files → AST JSON, via Rust → C → Zig boundary. REQUIREMENTS.md checkbox `[ ]` and traceability row "Pending" are stale documentation not updated when Plan 06 completed — codebase evidence is conclusive |
| REQ-phase-1-1-lexer      | 01-02          | Full `lexical.ebnf` tokenizer, zero UNKNOWN on showcase                  | ✓ SATISFIED      | `lexer.snapshot` test asserts zero `"k":"unknown"` across all 19 files; `[x]` in REQUIREMENTS.md |
| REQ-phase-1-2-parser-deal | 01-03         | 87 `deal.ebnf` productions, stable AST snapshots, Pratt expressions      | ✓ SATISFIED      | 15 `.deal.json` snapshots present and substantive; `parser_deal_snapshot.zig` byte-stability test; `[x]` in REQUIREMENTS.md |
| REQ-phase-1-3-parser-dealx | 01-04        | 43 `dealx.ebnf` productions, tag balancing, `via=`/`carrying=`          | ✓ SATISFIED      | 4 `.dealx.json` snapshots present; `ffi_parse_dealx_real` asserts `k:comp_connect`; `[x]` in REQUIREMENTS.md |
| REQ-phase-1-4-error-recovery | 01-05      | ≥50 malformed files, zero panics, structured diagnostic JSON             | ✓ SATISFIED      | 50 malformed files; `recovery.corpus` test passes; `diag.json_roundtrip` passes; `[x]` in REQUIREMENTS.md |
| REQ-phase-1-5-c-abi      | 01-06          | `libdeal.a`, `deal.h`, Rust FFI harness parses showcase file             | ✓ SATISFIED      | `libdeal.a` at `zig-out/lib/libdeal.a`; 8 `@brief` doxygen blocks in `deal.h`; 9 Rust FFI tests pass; `[x]` in REQUIREMENTS.md |
| REQ-phase-1-gate         | 01-06          | All 19 files parse through Zig core; snapshots stable; 50+ malformed; C ABI proven | ✓ SATISFIED | `gate_all_19` passes (1/1); all 5 ROADMAP success criteria confirmed; `[x]` in REQUIREMENTS.md |

**Note on REQ-phase-1-foundation documentation drift:** The REQUIREMENTS.md list checkbox remains `[ ]` (unchecked) and the traceability table shows "Pending" for `REQ-phase-1-foundation`, while all 6 sub-requirements (`REQ-phase-1-1-lexer` through `REQ-phase-1-gate`) are marked `[x]` Complete. ROADMAP.md's Phase 1 row is `[x]` complete dated 2026-05-20 and the progress table shows "Complete". The codebase fully satisfies the requirement's exit criterion (`gate_all_19` is the definitive proof). This is a documentation maintenance miss — the umbrella requirement checkbox was not ticked when Plan 06 was completed, unlike the sub-requirements. This should be corrected in REQUIREMENTS.md.

---

### Anti-Patterns Found

| File                       | Line | Pattern                                                     | Severity     | Impact                                                    |
|----------------------------|------|-------------------------------------------------------------|--------------|-----------------------------------------------------------|
| `src/ast.zig`              | 4, 57, 464, 623 | `// placeholder` comments referencing Wave-0 plan phases | ℹ Info       | Stale plan-era comments left from Wave 0 scaffolding; the code they reference is fully implemented; no functional impact |
| `src/json.zig`             | 457  | `// ── Composition placeholders (Plan 04)` comment         | ℹ Info       | Same stale comment pattern; the `switch` arms for all composition node kinds are fully implemented below the comment |

No `TBD`, `FIXME`, or `XXX` markers found in any phase-modified file. No `return null` / `return {}` stubs in rendering paths. No hardcoded empty data feeding user-visible output.

---

### Human Verification Required

None. All 5 ROADMAP success criteria are verifiable programmatically and have been verified via orchestrator gate run + codebase artifact checks. Phase 1 delivers a C ABI library (not a user-facing UI), so all behaviors are testable without visual inspection.

---

## Gaps Summary

No gaps. All 5 phase success criteria are demonstrably met in the codebase.

One documentation maintenance item identified (not a gap — does not block phase completion or Phase 2 start):

**REQ-phase-1-foundation checkbox drift:** `REQUIREMENTS.md` lines 16 and 125 show the umbrella requirement `REQ-phase-1-foundation` as `[ ]` / "Pending" despite all six constituent sub-requirements being marked complete and ROADMAP.md confirming Phase 1 complete. Recommended fix: update `REQUIREMENTS.md` to `[x]` / "Complete (Plan 01-06)" on those two lines. This is a single-commit docs fix; no code change needed.

---

_Verified: 2026-05-20_
_Verifier: Claude (gsd-verifier)_
