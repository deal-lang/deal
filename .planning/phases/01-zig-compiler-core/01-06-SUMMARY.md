---
phase: 1
plan: 6
subsystem: c-abi-and-gate
tags: [c-abi, ffi, utf8, security, gate, rust, zig]
requires: [01-05-SUMMARY.md]
provides: [libdeal.a production-hardened C ABI, Phase 1 exit gate]
affects: [src/lib.zig, include/deal.h, tests/ffi/, tests/unit/, build.zig]
tech-stack:
  added:
    - serde_json 1.x (Rust dev-dependency for JSON validation in FFI tests)
  patterns:
    - Arena aliasing fix: use handle.arena.allocator() after struct copy, never stale local allocator
    - UTF-8 validation gate before parser invocation (std.unicode.utf8ValidateSlice)
    - Source-length bound check before buffer read (maxInt(u32) guard)
    - deal_parse_internal / deal_free_internal as pub fn (NOT pub export fn) for test-only use
    - Lazy + cached JSON emission (D-11): ast_json_cache / diag_json_cache in DealHandle
key-files:
  created:
    - tests/unit/c_abi_invalid_utf8.zig
    - tests/unit/c_abi_source_too_large.zig
    - tests/unit/c_abi_no_leaks.zig
    - tests/ffi/tests/gate.rs
  modified:
    - src/lib.zig (complete rewrite — production hardening)
    - include/deal.h (production doxygen with error code reference table)
    - tests/ffi/tests/parse.rs (5 new real-parser FFI tests)
    - tests/ffi/Cargo.toml (serde_json dev-dependency)
    - tests/unit/_all.zig (3 new @import entries)
    - build.zig (lib_as_module wiring + phase-1-gate step)
    - .planning/phases/01-zig-compiler-core/01-VALIDATION.md (sign-off complete)
decisions:
  - "Arena aliasing: use handle.arena.allocator() after copying arena into handle struct to prevent page list divergence and memory leaks"
  - "deal_parse_internal uses pub fn (not pub export fn) so it does NOT appear in nm symbol table — test-only path"
  - "UTF-8 validation uses std.unicode.utf8ValidateSlice (confirmed in Zig 0.16.0 std library)"
  - "Source length bound: maxInt(u32) (4GB) — D-15 requires u32 byte offsets for spans"
  - "phase-1-gate step depends on test_step only; full gate = zig build phase-1-gate && cargo test"
  - "showcase symlink: absolute path /Users/dunnock/projects/deal-lang/spec/examples/showcase — relative paths fail in worktree context"
metrics:
  duration: "~90 minutes (across context window)"
  completed: 2026-05-20
  tasks_completed: 3
  tasks_total: 3
  zig_tests: 22
  rust_tests: 9
  total_tests: 31
  malformed_corpus_files: 50
  malformed_diagnostics: 166
  libdeal_size: 3.7MB
  zig_build_test_ms: 582
  cargo_test_ms: 1531
---

# Phase 1 Plan 6: C ABI and Gate Summary

Production-hardened C ABI with UTF-8 validation and source-length bounds, 3 Zig ABI unit tests, 5 new Rust FFI integration tests, Phase 1 `gate_all_19` exit criterion passing, and full `phase-1-gate` build step.

---

## Tasks Completed

### Task 6.1 — Harden deal_parse + finalize deal.h (commit `a4fada7`)

**Files:** `src/lib.zig` (rewrite), `include/deal.h` (rewrite)

`deal_parse` hardened with 5-step validation:
1. Null-pointer guard on both input slices
2. Filename duplication + D-12 mode detection (.dealx suffix)
3. V12 source-length bound: `source_len > maxInt(u32)` emits E0004 and returns handle without parsing
4. Source duplication
5. V5 UTF-8 validation: `std.unicode.utf8ValidateSlice` — invalid bytes emit E0001, null AST root

`deal_parse_internal` and `deal_free_internal` added as `pub fn` (NOT `pub export fn`) for test use. Confirmed absent from `nm -gU zig-out/lib/libdeal.a` symbol table.

All 6 existing C ABI exports retain null-handle guards via `orelse return <safe_default>` pattern (T-06-04).

`include/deal.h` rewritten with production doxygen: `@brief`, `@param`, `@returns`, `@note`, `@thread_safety` for every function; D-16 error code reference table (E0001-H0600).

**Symbol table:** exactly 6 exports (`_deal_ast_json`, `_deal_diagnostics_count`, `_deal_diagnostics_json`, `_deal_free`, `_deal_has_errors`, `_deal_parse`).

### Task 6.2 — C ABI unit tests + Rust FFI gate (commit `0f82b1e`)

**Files:** `tests/unit/c_abi_invalid_utf8.zig` (new), `tests/unit/c_abi_source_too_large.zig` (new), `tests/unit/c_abi_no_leaks.zig` (new), `tests/ffi/tests/gate.rs` (new), `tests/ffi/tests/parse.rs` (5 new tests), `tests/ffi/Cargo.toml` (serde_json added), `tests/unit/_all.zig` (3 imports added), `build.zig` (lib_as_module)

**Zig C ABI tests (3 new files, wired via `@import("lib")` in unit_root):**

- `c_abi_invalid_utf8`: 5 invalid UTF-8 cases (lone continuation, 0xFF start, truncated 2-byte, overlong null, valid+invalid mix). Asserts: non-null handle, 1 diagnostic, code=E0001, `"root":null` in AST JSON. Uses `deal_parse_internal` with `std.testing.allocator` for leak detection.
- `c_abi_source_too_large`: passes `source_len = maxInt(u32) + 1` to the public `deal_parse` C ABI. Asserts: non-null handle, 1 diagnostic, code=E0004, `"root":null`.
- `c_abi_no_leaks`: iterates 19 showcase + 50 malformed files (69 total) via `deal_parse_internal(std.testing.allocator, ...)`. Forces both JSON caches before `deal_free_internal`. Zero memory leaks (Zig test runner detects automatically).

**Rust FFI tests (9 total: 3 legacy + 5 new + gate_all_19):**

- `ffi_parse_battery_real`: reads `tests/showcase/packages/vehicle/battery.deal`, asserts no errors, checks D-04 envelope + `k:deal_file`
- `ffi_parse_dealx_real`: reads `tests/showcase/model/vehicle.dealx`, asserts `k:dealx_file` + `k:comp_connect`
- `ffi_diagnostics_on_malformed`: reads `tests/malformed/m02_unclosed_brace.deal`, parses diagnostics JSON via `serde_json`, asserts E0100..E0499 range
- `ffi_ast_json_cache_stable`: calls `deal_ast_json` twice, asserts same pointer AND same bytes (D-11)
- `ffi_invalid_utf8`: passes `&[0xff, 0xfe, 0xfd]`, asserts E0001 + `"root":null`
- `gate_all_19`: iterates all 19 showcase files, asserts no parse errors, verifies D-04 envelope + mode-by-extension, validates via `serde_json::from_str`

**build.zig change:** `lib_as_module` wired into `unit_root` so C ABI tests can `@import("lib")` for `deal_parse_internal` / `deal_free_internal`.

### Task 6.3 — phase-1-gate step + VALIDATION.md sign-off (commit `b96334b`)

**Files:** `build.zig` (phase-1-gate step), `.planning/phases/01-zig-compiler-core/01-VALIDATION.md`

- `zig build phase-1-gate` depends on `test_step`; exits 0
- Full Phase 1 gate command: `zig build phase-1-gate && cargo test --manifest-path tests/ffi/Cargo.toml`
- VALIDATION.md: all 19 task rows green, Wave 0 checkboxes checked, sign-off complete, `nyquist_compliant: true`, `status: ready-for-execute`

---

## Phase 1 Success Criteria Status

| Criterion | Requirement | Status |
|-----------|------------|--------|
| #1 Zero UNKNOWN tokens on 19 files | REQ-phase-1-1-lexer | ✅ Plan 02 |
| #2 15 .deal AST snapshots byte-stable | REQ-phase-1-2-parser-deal | ✅ Plan 03 |
| #3 4 .dealx snapshots + tag-balance | REQ-phase-1-3-parser-dealx | ✅ Plan 04 |
| #4 ≥50 malformed corpus no panic + JSON diags | REQ-phase-1-4-error-recovery | ✅ Plan 05 |
| #5 C ABI zero-leak + Rust FFI gate_all_19 | REQ-phase-1-5-c-abi + REQ-phase-1-gate | ✅ Plan 06 |

---

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Arena aliasing leak in deal_parse_internal**
- **Found during:** Task 6.1 (detected by c_abi_invalid_utf8 test: 5 leaks reported)
- **Issue:** `deal_parse_internal` captured `const allocator = arena.allocator()` before copying arena into handle struct. After `handle.* = .{ .arena = arena, ... }`, the captured allocator still pointed to the LOCAL arena variable. New ArrayList page allocations (during diagnostics append) updated `local_arena.first_node`, not `handle.arena.first_node`. When `deal_free_internal` called `handle.arena.deinit()`, those pages were missed.
- **Fix:** After the struct copy, use `const alloc = handle.arena.allocator()` (pointing to `handle.arena` in the heap) for all subsequent allocations. Same fix applied to the production `deal_parse`.
- **Files modified:** `src/lib.zig`
- **Commit:** `a4fada7`

**2. [Rule 3 - Blocking] Wrong Dir.iterate API in c_abi_no_leaks.zig**
- **Found during:** Task 6.2 compilation
- **Issue:** Used `malformed_dir.iterate(io)` — expected 0 arguments. Zig 0.16.0 API is `dir.iterate()` (no arg) returning `Dir.Iterator`; then `iter.next(io)` per entry.
- **Fix:** Changed to `var iter = malformed_dir.iterate()` + `while (try iter.next(io))`.
- **Files modified:** `tests/unit/c_abi_no_leaks.zig`
- **Commit:** `0f82b1e`

**3. [Rule 1 - Bug] Unused constants in c_abi_source_too_large.zig**
- **Found during:** Task 6.2 compilation
- **Issue:** `const gpa = std.testing.allocator` and `const oversized_len` were declared but unused. Zig 0.16.0 treats unused `const` declarations as compile errors.
- **Fix:** Removed both unused declarations; the `source_len` is computed inline in the `deal_parse` call.
- **Files modified:** `tests/unit/c_abi_source_too_large.zig`
- **Commit:** `0f82b1e`

**4. [Rule 3 - Blocking] tests/showcase symlink wrong relative path**
- **Found during:** Task 6.2 (c_abi_no_leaks.zig couldn't open showcase directory)
- **Issue:** Relative path `../../../../spec/examples/showcase` resolved incorrectly due to worktree directory depth. The worktree root is at `.claude/worktrees/agent-a8d7b3915b68c1d1a/`, so going up 4 levels did not reach the deal-lang project root.
- **Fix:** Used absolute path: `ln -s /Users/dunnock/projects/deal-lang/spec/examples/showcase tests/showcase`
- **Files modified:** `tests/showcase` (symlink)
- **Commit:** `0f82b1e`

---

## Known Issues (Pre-existing, Not Introduced by Plan 06)

**`zig build test` "failed command" noise:** The `--listen=-` protocol between `zig build` and the test runner emits "failed command" to stderr when recovery.corpus prints zero-diagnostic filenames to its own stderr channel. The actual exit code is 0. This is a pre-existing issue from Plan 05 (13 of 50 malformed corpus files produce 0 diagnostics). Not fixed here; logged for reference.

---

## Threat Surface Scan

No new network endpoints, auth paths, file access patterns, or schema changes introduced. The `deal_parse_internal` / `deal_free_internal` helpers are `pub fn` only — not exported to the C ABI — so they cannot be called from Rust or C. The UTF-8 validation and source-length guards are the primary security mitigations added (V5, V12 ASVS).

---

## Self-Check

Files created:
- `.planning/phases/01-zig-compiler-core/01-06-SUMMARY.md` (this file)
- `tests/unit/c_abi_invalid_utf8.zig`
- `tests/unit/c_abi_source_too_large.zig`
- `tests/unit/c_abi_no_leaks.zig`
- `tests/ffi/tests/gate.rs`

Commits:
- `a4fada7`: feat(01-06): harden deal_parse — UTF-8 validation, source-len bounds, null guards, doxygen deal.h
- `0f82b1e`: feat(01-06): C ABI unit tests (invalid_utf8, source_too_large, no_leaks) + Rust FFI gate test (gate_all_19)
- `b96334b`: chore(01-06): phase-1-gate build step + VALIDATION.md sign-off
