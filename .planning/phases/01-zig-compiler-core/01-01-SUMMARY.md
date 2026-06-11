---
phase: 01-zig-compiler-core
plan: 01-01-foundation
status: complete
completed: 2026-05-19
requirements:
  - REQ-phase-1-foundation
commits:
  - 7d6aa2d  chore(01-01): scaffold build.zig, build.zig.zon, and tests/ skeleton
  - 3607340  feat(01-01): scaffold src/*.zig stubs and hand-written include/deal.h
  - 2c1c9d5  feat(01-01): add Rust FFI test harness scaffold
verification:
  zig_build: pass (zig-out/lib/libdeal.a + zig-out/include/deal.h present)
  zig_build_test: pass (2/2 stub-compile tests)
  zig_build_help_test_filter: pass (-Dtest-filter=[string] visible)
  cargo_test: pass (3/3 in tests/parse.rs: ffi_smoke, ffi_dealx_mode_detection, ffi_free_null_is_safe)
  no_uppercase_callconv: pass (grep -c 'callconv(\.C)' src/*.zig → 0)
---

# Plan 01-01-foundation — Summary

Wave 0 scaffolding for the Zig compiler core. Establishes the build system,
empty-but-valid stubs for every Phase 1 source file, the C ABI surface as a
no-op stub, the hand-written C header, and the Rust FFI test harness — all
wired together so subsequent plans (02-06) only fill bodies rather than
create files.

## What Shipped

**Task 1.1 — Build scaffolding** (commit chore(01-01):
scaffold build.zig...)

- `deal/build.zig` — Zig 0.16.0 build entry. Uses `b.addLibrary(.{ .linkage
  = .static, ..., .root_module = b.createModule(...) })`, `b.installFile`,
  `b.addTest(.{ .filters = ... })`, `b.addRunArtifact`, and `b.step`.
  Declares `-Dtest-filter` so plans 02-06 can run targeted suites; reserves
  a `gen-malformed` step placeholder for Plan 05.
- `deal/build.zig.zon` — enum-literal `.name = .deal`, version
  `0.1.0-draft`, `minimum_zig_version 0.16.0`, empty `.dependencies`. The
  fingerprint required by `zig build` is `0xe3fec116726111a5` (assigned by
  the toolchain on first build — see Deviations below).
- `deal/tests/{snapshots,malformed,unit}/.gitkeep` so the directories
  commit ahead of later waves.
- `deal/.gitignore` covers `zig-out/`, `.zig-cache/`, `.codegraph/`, and
  the Cargo target/ + Cargo.lock under `tests/ffi/`.

**Task 1.2 — Source stubs + hand-written deal.h** (commit feat(01-01):
scaffold src/*.zig stubs...)

11 Zig modules:

| File | Wave 0 contents | Decisions materialized |
|---|---|---|
| `src/ast.zig` | `Span` extern struct, `NodeKind` enum (file kinds), `Mode` enum, `Node` struct | D-01, D-03, D-15 |
| `src/diagnostics.zig` | `Severity`, `SpanLabel`, `FixIt`, `Diagnostic` + code-namespace doc comment | D-14, D-16 |
| `src/source_map.zig` | `SourceMap` stub (returns 1:1) | D-15 |
| `src/lexer.zig` | `Mode` (4-variant), `Tag`, `Token`, `Lexer.{init,next,peek}` stubs | D-06 |
| `src/keywords.zig` | `std.StaticStringMap(Tag).initComptime(.{})` empty | (Plan 02 fills) |
| `src/expr.zig` | `parseExpression(parser, min_bp)` signature | D-08 |
| `src/parser_deal.zig` | `Handle = opaque {}`, `parseFile` stub | (Plan 03 fills) |
| `src/parser_dealx.zig` | `parseFile` stub | (Plan 04 fills) |
| `src/parser.zig` | mode-dispatch driver over parseFile | D-17 (entry stub) |
| `src/json.zig` | `emitAst` → D-04 envelope; `emitDiagnostics` → `[]` | D-04, D-18 |
| `src/lib.zig` | Six `callconv(.c)` exports + DealHandle + 2 inline tests | D-02, D-05, D-10, D-11, D-12, D-13 |

`deal/include/deal.h` is hand-written (per Pitfall 6 — `-femit-h` not used).
Documents ownership, thread model (D-13), error model (D-10), and string
contract (D-11) in Doxygen-style block comments. `#ifdef __cplusplus`
guards added for safety.

**Task 1.3 — Rust FFI harness** (commit feat(01-01): add Rust FFI test
harness scaffold)

- `deal/tests/ffi/Cargo.toml` — package `deal-ffi-tests`, `links = "deal"`.
- `deal/tests/ffi/build.rs` — invokes `zig build` in `deal/`, emits the
  Cargo 1.77+ double-colon directives. Confirmed link-search path:
  `/Users/dunnock/projects/deal-lang/deal/zig-out/lib`.
- `deal/tests/ffi/src/lib.rs` — raw `extern "C"` declarations for all six
  exports. `DealHandle` carries a `PhantomData<(*mut u8, PhantomPinned)>`
  marker so the type is `!Send + !Sync + !Unpin` (per D-13).
- `deal/tests/ffi/tests/parse.rs` — three integration tests
  (`ffi_smoke`, `ffi_dealx_mode_detection`, `ffi_free_null_is_safe`). All
  pass on the stub library, proving the link recipe works on this
  macOS arm64 + Zig 0.16.0 + Rust 1.93 stack.

## Verification Results

```
$ cd deal
$ zig build
  → zig-out/lib/libdeal.a (3.0 MB)
  → zig-out/include/deal.h
$ zig build test
  → 2/2 tests pass (stub: parse empty source / .dealx mode by filename)
$ zig build --help | grep test-filter
  -Dtest-filter=[string]       Filter test names by substring (...)
$ cargo test --manifest-path tests/ffi/Cargo.toml
  → 3/3 pass: ffi_smoke, ffi_dealx_mode_detection, ffi_free_null_is_safe
$ grep -c 'callconv(\.C)' src/*.zig
  → 0 (uppercase `.C` is banned; only lowercase `.c` appears in src/lib.zig — 6 occurrences, one per export)
```

## RESEARCH Assumption Resolutions

**A1 — `std.unicode.utf8ValidateSlice` name in 0.16.0.** Not exercised by
Wave 0 (lexer is a no-op). Plan 02 will resolve this on first invocation.
Carrying forward as open; recommend Plan 02's Task 2.1 verify via
`zig build` against a probe.

**A4 — `installFile` vs `addInstallFile` in 0.16.0.** **RESOLVED:
`b.installFile(src_path, dest_rel_path)` is correct.** Confirmed by
inspecting `/opt/homebrew/Cellar/zig/0.16.0_1/lib/zig/std/Build.zig` line
1675:

```
pub fn installFile(b: *Build, src_path: []const u8, dest_rel_path: []const u8) void
pub fn addInstallFile(b: *Build, source: LazyPath, dest_rel_path: []const u8) *Step.InstallFile
```

Both exist; `installFile` is the simpler convenience wrapper that takes a
path string and immediately registers the step on the default install
graph (no return value to chain). `addInstallFile` takes a `LazyPath` and
returns the step so the caller can compose dependencies. For our header
install we want the simple side-effect form, so `installFile` is the
right choice. `zig build` produced `zig-out/include/deal.h` correctly.

**A9 — `cargo::rustc-link-lib=static=deal` on macOS arm64.** **RESOLVED:
works as documented.** The build.rs script printed:

```
cargo::rustc-link-search=native=/Users/dunnock/projects/deal-lang/deal/zig-out/lib
cargo::rustc-link-lib=static=deal
```

`cargo test` linked the three integration tests against `libdeal.a` and
all three pass. The double-colon (`cargo::`) form is the standard on this
host (Cargo 1.93). The link recipe is now proven end-to-end on
Darwin 25.4.0 arm64 + Zig 0.16.0 + Rust 1.93.

## Test-Filter Mechanism

**Resolved: `addTest(.{ .filters = ... })`** via the `TestOptions.filters:
[]const []const u8` field (verified at `std/Build.zig:862`). Threaded
through as:

```zig
const test_filter: ?[]const u8 = b.option([]const u8, "test-filter", "...");
const tests = b.addTest(.{
    .root_module = ...,
    .filters = if (test_filter) |f| &[_][]const u8{f} else &[_][]const u8{},
});
```

Single-substring filter today; the field accepts an array so Plan 02 can
extend to multi-filter without an API change.

## Deviations from the Plan / RESEARCH §Example A

1. **`build.zig.zon` fingerprint required.** The plan listed only
   `.name`, `.version`, `.minimum_zig_version`, `.dependencies`, and
   `.paths`. Zig 0.16.0 additionally requires a `.fingerprint = 0x...`
   field for packages — the first `zig build` produced
   `error: invalid fingerprint: ...; if this is a new or forked package,
   use this value: 0xe3fec116726111a5`. Adopted the toolchain-assigned
   value verbatim. Confirmed via `zig init` probe output at
   `/tmp/zig-probe/build.zig.zon` (which also carries a `.fingerprint`).

2. **`build.zig.zon` `.name` is an enum literal.** Plan note already
   warned about this; verified against `zig init` (`.name = .zig_probe`)
   and confirmed `.name = .deal` works for us.

3. **`src/lib.zig` does not yet call `parser.parseFile`.** The plan
   suggests the parser dispatcher is called from `deal_parse`; deferred
   until Plan 02/03 because the opaque `parser_deal.Handle` and the
   concrete `DealHandle` are not yet the same type (and they cannot be
   merged at Wave 0 without a circular import). The dispatch will be
   wired in Plan 02's first task with a single-line edit, once the
   parser knows enough lexer state to do anything. Wave 0's behavior is
   identical (parser is a no-op either way).

4. **`comptime { _ = lexer; _ = parser; }` block** added to `src/lib.zig`
   so the unused-import warnings don't fire while the parser is still a
   stub. Plan 02 deletes this block on first real use of the imports.

5. **Added `#ifdef __cplusplus` guards** in `include/deal.h`. Not
   strictly required by the plan but matches the convention for any C
   header that might be consumed by a C++ project (the showcase Rust
   harness doesn't need it, but future Tauri integration in Phase 6
   might).

6. **Added an `ffi_free_null_is_safe` test** in `tests/ffi/tests/parse.rs`
   (the plan called for two tests; this is the third). Useful one-line
   guarantee that `deal_free(NULL)` is a no-op — matches the documented
   contract in `deal.h` and exercises the `if (handle_ptr) |p| { ... }`
   guard in `src/lib.zig`.

## Open Items Carrying Forward

- **A1 resolution** — Plan 02 should confirm
  `std.unicode.utf8ValidateSlice` on its first lexer task.
- **`src/parser_deal.Handle` opaque → concrete unification** — Plan 02
  decides whether `DealHandle` lives in `lib.zig` (current) and the
  parsers take `*DealHandle`, or whether `lib.zig` imports `Handle` from
  `parser_deal.zig`. Current direction (lib.zig owns) is the simpler
  option; flag if Plan 02 finds friction.
- **`gen-malformed` step body** — Plan 05 fills in.
- **Real parser dispatch from `deal_parse`** — Plan 02 (Task 2.1's
  first edit).

## Files Changed

```
deal/.gitignore
deal/build.zig
deal/build.zig.zon
deal/include/deal.h
deal/src/ast.zig
deal/src/diagnostics.zig
deal/src/expr.zig
deal/src/json.zig
deal/src/keywords.zig
deal/src/lexer.zig
deal/src/lib.zig
deal/src/parser.zig
deal/src/parser_deal.zig
deal/src/parser_dealx.zig
deal/src/source_map.zig
deal/tests/ffi/Cargo.toml
deal/tests/ffi/build.rs
deal/tests/ffi/src/lib.rs
deal/tests/ffi/tests/parse.rs
deal/tests/malformed/.gitkeep
deal/tests/snapshots/.gitkeep
deal/tests/unit/.gitkeep
```
