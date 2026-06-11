# Phase 1: Zig Compiler Core - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-19
**Phase:** 1-zig-compiler-core
**Areas discussed:** AST representation & arena lifetime, .dealx mode switching at {...} boundaries, C ABI memory ownership & error path, Diagnostic richness

---

## AST Representation & Arena Lifetime

### Q1: How should AST nodes be represented in memory?

| Option | Description | Selected |
|--------|-------------|----------|
| Tagged union | Single Node type with kind + union(NodeKind) payload. Idiomatic Zig, simple traversal, easy debug. | ✓ |
| Data-oriented (NodeIndex + side tables) | Zig's own compiler style. Best cache behavior, harder to write. | |
| Typed struct-per-kind with common header | Separate concrete types sharing a header. More verbose. | |

**User's choice:** Tagged union
**Notes:** User added explicit instruction to follow Zig 0.16.0 langref at `spec/references/zig-lang/0.16.0/*.html` — added as a MUST-READ canonical ref and as an Implementation Decision (D-05).

---

### Q2: What is the lifetime of the AST arena?

| Option | Description | Selected |
|--------|-------------|----------|
| Per-handle (lives until deal_free) | Arena owned by parse handle. Single dealloc walks whole arena. Matches opaque-handle ownership. | ✓ |
| Per-parse (free after JSON serialization) | Arena freed after snapshot generation. Bad fit for FFI handle pattern. | |
| Single global arena per process | Leaks accumulate. Unacceptable for LSP-shaped use case. | |

**User's choice:** Per-handle (lives until deal_free)

---

### Q3: Should `.deal` and `.dealx` share a single AST node enum, or have separate hierarchies?

| Option | Description | Selected |
|--------|-------------|----------|
| Unified NodeKind enum, file-mode tag on root | One Node type with kinds for both. Common subtree shared. Uniform JSON. | ✓ |
| Two AST hierarchies, shared expression subtree | Type-level separation but doubles serialization paths. | |
| Fully separate ASTs, no shared subtree | Maximum isolation. Worst code reuse. | |

**User's choice:** Unified NodeKind enum, file-mode tag on root

---

### Q4: AST JSON output shape?

| Option | Description | Selected |
|--------|-------------|----------|
| Compact, schema-versioned, kind-tagged | `{ "v": 1, "mode": "deal", "nodes": [...] }`, short keys, span always present. | ✓ |
| Verbose, human-readable | Long key names, larger snapshots, no versioning. | |
| Index-array form (data-oriented serialization) | Top-level arrays + indices. Mismatch with tagged-union memory choice. | |

**User's choice:** Compact, schema-versioned, kind-tagged

---

## `.dealx` Mode Switching at `{...}` Boundaries

### Q1: How should the lexer/parser handle mode switching between composition tags and embedded `{expr}` blocks?

| Option | Description | Selected |
|--------|-------------|----------|
| Parser-driven mode stack on a single lexer | Lexer holds mode_stack; parser explicitly pushes/pops. | |
| Pure lexer state machine | Lexer maintains DFA over modes from recent tokens. | |
| Lexer-on-demand: parser passes expected mode each call | Stateless lexer; parser passes mode each `nextToken()` call. | ✓ |

**User's choice:** Lexer-on-demand: parser passes expected mode each call
**Notes:** All mode context lives in the parser. More plumbing but cleaner separation.

---

### Q2: Tag balancing — where does the open-tag stack live and how strict is mismatch handling?

| Option | Description | Selected |
|--------|-------------|----------|
| Parser-owned stack, structural error on mismatch | Diagnostic with both spans; pop anyway to continue. Best matches 50+ malformed gate. | ✓ |
| Parser-owned stack, strict halt on mismatch | Mismatch terminates composition parse. Kills rest of AST. | |
| Lexer pre-validates balance, structured open/close tokens | Mixes parsing concerns into lexing. | |

**User's choice:** Parser-owned stack, structural error on mismatch

---

### Q3: Inside attribute `{...}` blocks on tags — what level of DEAL syntax do we accept in Phase 1?

| Option | Description | Selected |
|--------|-------------|----------|
| Full `deal.ebnf` expression production | Unit-literal calls, member access, identifiers, arithmetic, literals. Closing `}` terminator. | ✓ |
| Limited subset (literals + unit-literal calls + identifiers) | Phase 1 narrowest forms only. Phase 2 must extend. | |
| Raw-string capture (delay parsing to Phase 2) | Capture span only. Defeats snapshot-stability goal. | |

**User's choice:** Full `deal.ebnf` expression production

---

### Q4: `[<connect>]` inline `via={...} carrying={...}` blocks — how modeled in AST?

| Option | Description | Selected |
|--------|-------------|----------|
| First-class CompConnect node with typed via/carrying slots | Dedicated node kind. Encodes CS-4 at AST level. Phase 2 lowering trivial. | ✓ |
| Generic CompTag node with via/carrying as named attributes | Less code now; Phase 2 has to re-discover the structure. | |

**User's choice:** First-class CompConnect node with typed via/carrying expression slots

---

## C ABI Memory Ownership & Error Path

### Q1: When `deal_parse()` encounters a fatal parse failure, what does it return?

| Option | Description | Selected |
|--------|-------------|----------|
| Always returns a handle; failure observable via deal_diagnostics() | Non-null on every call. Partial AST useful for LSP. | ✓ |
| Returns NULL on fatal; separate function exposes lex-level diagnostics | Two-path API; needs thread-local for failure-path diagnostics. | |
| Returns DealResult struct (handle + error_count + first_error_message) | Single struct return. Multi-field C struct adds complexity. | |

**User's choice:** Always returns a handle; failure is observable via deal_diagnostics()

---

### Q2: How should AST JSON and diagnostics be delivered across the C ABI?

| Option | Description | Selected |
|--------|-------------|----------|
| Length-prefixed UTF-8 buffers via accessor functions | `deal_ast_json(handle, out_ptr, out_len)`. Lazy generation, cached on handle. | ✓ |
| Null-terminated C strings | Simple but UTF-8 NUL concerns and O(n) length on Rust side. | |
| Structured C struct accessors per node (no JSON) | Efficient traversal but doubles API surface; still need JSON for snapshots. | |

**User's choice:** Length-prefixed UTF-8 buffers via accessor functions

---

### Q3: Should `deal_parse()` take a file path, or source bytes?

| Option | Description | Selected |
|--------|-------------|----------|
| Source bytes + filename hint | Rust reads file. LSP-friendly. Testable from memory. | ✓ |
| File path; Zig reads the bytes | Simpler caller-side. Worse for LSP. | |
| Both APIs (path and bytes) | More surface area. Not justified for Phase 1. | |

**User's choice:** Source bytes + filename hint

---

### Q4: Threading and re-entrancy model for the C ABI?

| Option | Description | Selected |
|--------|-------------|----------|
| Each handle is single-threaded; multiple handles concurrent on different threads | No global state. Fresh arena per call. | ✓ |
| Fully thread-safe handles (internal locking) | Adds Mutex; not needed in Phase 1. | |
| Single global mutex around all calls | Kills LSP concurrency. | |

**User's choice:** Each handle is single-threaded, but multiple handles can exist on different threads concurrently

---

## Diagnostic Richness

### Q1: How rich should diagnostics be in Phase 1?

| Option | Description | Selected |
|--------|-------------|----------|
| Rich data model, simple text renderer | Full struct with secondary spans + fix-its + notes; CLI renders one line. Phase 3 LSP reuses. | ✓ |
| Minimal (code + message + primary span only) | Phase 3 has to re-instrument. Same investment, later. | |
| Full rustc-style with Unicode underlines in Phase 1 | Beautiful but over-invests. CLI is Rust's job per layering. | |

**User's choice:** Rich data model, simple text renderer

---

### Q2: Span representation — what does a Span carry?

| Option | Description | Selected |
|--------|-------------|----------|
| Byte offsets only; line/col computed on demand | `{ start: u32, end: u32 }`. 8 bytes. Matches Zig std + rustc. | ✓ |
| Line/column directly | Larger; UTF-8 column counts get hairy. | |
| Both byte offsets AND line/col cached | Two sources of truth; bloats AST. | |

**User's choice:** Byte offsets only; line/col computed on demand from a SourceMap

---

### Q3: Diagnostic code namespace — how do we organize error codes?

| Option | Description | Selected |
|--------|-------------|----------|
| Letter-prefixed numeric codes by category, stable across versions | `E0001..` lexer, `E0100..` parser-deal, etc. Stable codes. | ✓ |
| Symbolic codes (e.g. `unterminated_string_literal`) | Renames break tooling. | |
| Auto-incrementing codes with no category structure | Loses at-a-glance triage. | |

**User's choice:** Letter-prefixed numeric codes by category, stable across versions

---

### Q4: Error-recovery synchronization granularity — where does the parser jump back to a known state?

| Option | Description | Selected |
|--------|-------------|----------|
| Statement + definition + dealx tag boundary | Three layers. Maximizes salvageable AST. | ✓ |
| Statement boundary only | May lose composition trees on one bad tag. | |
| Definition-boundary only | Coarse; loses too much AST for LSP. | |

**User's choice:** Statement boundary + definition boundary + dealx tag boundary

---

## Claude's Discretion

The following implementation details were not explicitly discussed and remain at the planner's / researcher's discretion (per CONTEXT.md "Claude's Discretion" section):

- Snapshot test infrastructure (snapshot format conventions, update flag, layout under `tests/snapshots/` and `tests/malformed/`)
- `build.zig` structure (library target, test wiring, Rust harness integration)
- String interning strategy (hash map vs raw source slices)
- Token snapshot JSON format (likely mirrors AST schema with `"v": 1` envelope)
- Filename → mode resolution (extension check vs explicit override parameter)
- Lexer-on-demand API exact shape (value return vs out-param; lookahead via peek vs shared position)
- Phase 1 stays at parsing; no symbol table or identifier resolution

## Deferred Ideas

Captured in CONTEXT.md `<deferred>` section. Highlights:

- Formatter, semantic analysis, IR, codegen, CLI, LSP, tree-sitter, VS Code extension, Tauri editor — all in later phases per ROADMAP.md.
- Pretty multi-line CLI diagnostic rendering — Phase 3 LSP / Phase 6 editor.
- Incremental reparse / AST diffing — Phase 3 LSP.
- ADR-deferred items (constraint syntax, path expressions, MQL, `@document:`) remain deferred.
