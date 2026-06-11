---
phase: 2
slug: prove-the-pipeline
status: verified
threats_open: 0
asvs_level: 1
created: 2026-05-22
---

# Phase 2 — Security

> Per-phase security contract: threat register, accepted risks, and audit trail.
> Audit run: 2026-05-22 by gsd-security-auditor. Threats register authored at plan time (42 unique threats across 6 PLAN files). All `mitigate` threats verified against committed code; all `accept` threats logged.

---

## Trust Boundaries

Union of trust boundaries declared across the six Phase 2 plans (Plan 02-01 through 02-06).

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| User shell → cargo workspace | Untrusted invocation of `cargo build`; package legitimacy verified offline | Cargo manifest, source paths |
| Filesystem → `tests/schemas/` | Bundled SysML.json + KerML.json read by validator (Plan 02-04); tampering corrupts validation | Schema JSON bytes, SHA256-pinned |
| AST consumer → Comment struct | `Comment.text` is arena-owned; downstream readers must not free or mutate | Arena-owned byte slices |
| CLI arg parsing → process exit | Wrong exit-code conflation (D-34) silently corrupts CI signal | Argv, exit codes |
| Untrusted DEAL source → sema analyzer | Adversarial inputs (deeply nested specializes chains, large symbol tables) must not panic or run unbounded | DEAL source text |
| Sema diagnostic strings → CLI rendering | Format-string injection via DiagnosticCollector | Diagnostic text, codes |
| FFI byte buffer → Rust diagnostic JSON parse | Use-after-free if Rust holds pointer past `deal_free` | Length-prefixed byte buffers |
| `.deal/index.json` write → filesystem | Path traversal risk if file paths from source bleed into the index | Workspace-relative paths |
| `--json` envelope → consumer (CI, Phase 3 LSP) | Schema drift breaks downstream consumers (public contract) | D-32 envelope JSON |
| AST → IR lowering | Adversarial AST (deep nesting, malformed reference targets) must not panic | AST nodes |
| IR JSON → FFI buffer | Length-prefixed UTF-8 buffer ownership crosses Zig→Rust boundary | IR JSON bytes |
| IR schema.json → public contract | Public contract surface for Phases 3-6; breaking changes cascade | Spec artifacts |
| Lowering pass → arena alloc | All IR data allocates into per-handle arena (D-02); cross-arena pointers forbidden | Arena pointers |
| IR JSON bytes → Rust deserialize | Untrusted (but well-formed by construction); arena-owned bytes need Cow-clone before deal_free | IR JSON bytes |
| SysML.json + KerML.json bundle → schema_registry | Tampering risk; SHA256 pin + runtime check | Schema JSON, digests |
| Validator output → consumer (CLI + Phase 3 LSP) | Public contract surface (SysML v2 JSON exit format) | SysML v2 JSON |
| Insta snapshots → golden truth | If accept-all is run without review, regressions ship | Golden fixture bytes |
| Hand-authored `.expected.json` fixtures → insta reference | Typo can become anchored reference unless independent schema-validity gate exists | Hand-authored JSON |
| Untrusted DEAL source → fmt.zig pretty-printer | Malformed AST shapes must not panic; comment text could be adversarial | AST + comment text |
| `deal_format` FFI byte buffer → Rust CLI | Use-after-free risk if Rust holds pointer past deal_free | Length-prefixed bytes |
| In-place file write → filesystem | Power-loss / race conditions during write | Source file bytes |
| `git config` probe → environment | Subprocess invocation; argument injection risk | git stdout |
| `build.zig phase-2-gate` → CI authoritative gate | If gate misses a Phase 2 acceptance check, regressions ship silently | Build orchestration |
| `verify-fresh-worktree.sh` argv → child process | Argument injection if argv mis-handled | Gate-step name |
| Viewer-import upload to external service | Showcase JSON may contain authoring metadata; uploading to public service is information disclosure | Showcase JSON |
| `02-VERIFICATION.md` sign-off → phase completion authority | False-GREEN risk if checkbox flipped without evidence | Verification records |
| `deal parse --json` output shape → public contract surface | If envelope extended ad-hoc, downstream consumers see undocumented contract | AST JSON / D-32 envelope |

---

## Threat Register

All 42 unique threats (T-02-SC appears six times across plans; treated as one logical supply-chain threat). Status reflects implementation evidence in shipped code (commits 9b2d64a + 2525891 + closeout commits per VERIFICATION).

| Threat ID | Category | Component | Disposition | Mitigation / Evidence | Status |
|-----------|----------|-----------|-------------|----------------------|--------|
| T-02-01 | Tampering | tests/schemas/ schema bundle | mitigate | `tests/schemas/README.md` lines 38–41 record SHA256 for both schemas; `cli/src/schema_registry.rs:24-29` declares `EXPECTED_SYSML_SHA256` + `EXPECTED_KERML_SHA256`; `verify_sha256()` at lines 86–88 fails build_validator() if digests drift | closed |
| T-02-02 | Tampering | Cargo dependency graph (preserve_order leak) | mitigate | `cli/tests/key_order.rs:13-28` `serde_json_emits_alphabetical_keys` asserts `{"z":1,"a":2}` serializes to `{"a":2,"z":1}`; `Cargo.toml:10-11` pins serde_json without `preserve_order` feature | closed |
| T-02-03 | Tampering | npm/pip/cargo installs | mitigate | `02-RESEARCH.md` §Package Legitimacy Audit (lines 185-209) records `[OK]` slopcheck verdict for every workspace dep; no `[ASSUMED]` or `[SUS]` remain; `Cargo.toml` `[workspace.dependencies]` lists every approved crate | closed |
| T-02-04 | Information disclosure | Comment text containing PII/secrets | accept | See Accepted Risks Log (R-02-04) | closed |
| T-02-05 | Repudiation | CLI exit code conflation (D-34) | mitigate | `cli/src/main.rs:24-31` declares `CliError::User(String)` and `CliError::Internal(anyhow::Error)`; `cli/src/main.rs:35-37` `is_user()` selects exit 1 vs 2; `cli/tests/cli_smoke.rs:58` `subcommand_stubs_exit_two` asserts exit-code discipline | closed |
| T-02-06 | Denial of service | Parser comment-token flood (10MB comment) | accept | See Accepted Risks Log (R-02-06) | closed |
| T-02-07 | Tampering | AST snapshot regen diff (Pitfall 2) | mitigate | `scripts/diff-only-new-fields.sh` exists (chmod +x; doc header cites T-02-07); asserts diff hunks contain only `leading_comments`/`trailing_comments`/`doc_comment`; Plan 02-01-SUMMARY records exit 0 across the regen commit | closed |
| T-02-08 | Denial of service | `<<specializes>>` cycle in adversarial input | mitigate | `src/sema.zig:631-678` `checkSpecializationCycle` uses visited-set; emits `E2300 e_specialization_cycle` on re-entry; line 663 bounds visited depth at symbol-table size; regression fixture `tests/regressions/sema/04-specializes.deal` pins E2300 | closed |
| T-02-09 | Denial of service | Deeply nested name resolution (10K-level scope chain) | accept | See Accepted Risks Log (R-02-09) | closed |
| T-02-10 | Tampering | Format-string injection via diagnostic messages | mitigate | `src/diagnostics.zig:224-229` `emitFmt(comptime fmt: []const u8, args)` requires comptime fmt string (compile-time enforced); `src/sema.zig:44` doc-comment cites T-02-10; 13 grep matches of `emitFmt` in sema.zig all use string literal fmt arguments | closed |
| T-02-11 | Information disclosure | `.deal/index.json` leaking absolute paths | mitigate | `src/sema.zig:40` doc-comment cites T-02-11; `src/sema.zig:83,117,130` `source_file: []const u8` field documented workspace-relative; symbol table entries store relative paths from sema's caller; `index_json` writer in `src/json.zig` does not transform paths | closed |
| T-02-12 | Tampering | `--json` envelope key reordering breaks consumers | mitigate | `cli/src/main.rs:294-300` hand-rolled D-32 envelope with literal alphabetical order (command, deal_version, diagnostics, summary, v); `cli/tests/check_subcommand.rs` asserts envelope shape; `cli/tests/key_order.rs` defensive test guards against transitive `preserve_order` leaks | closed |
| T-02-13 | Spoofing | Use-after-free of diagnostic JSON pointer past deal_free | mitigate | `cli/src/main.rs:212` `diag_json_owned = std::slice::from_raw_parts(out_ptr, out_len).to_vec();` precedes `cli/src/main.rs:226` `ffi::deal_free(handle);` — Cow-clone before free (Pitfall 3 gpa.dupe-before-free pattern) | closed |
| T-02-14 | Repudiation | Exit code conflation (panic returns 1 instead of 2) | mitigate | Same `CliError` split as T-02-05; `cli/tests/check_subcommand.rs` asserts file-not-found exits 2 and sema-error exits 1; per Pitfall 8 | closed |
| T-02-15 | Tampering | IR JSON key-order drift (D-18 violation) | mitigate | `src/json.zig:1354` `pub fn emitIrJson(...)` hand-rolled; lines 4, 936, 1202-1203 doc-comments cite Pitfall 1/5 (no std.json.Stringify); zero non-comment usages of `std.json.Stringify` in src/json.zig; `tests/unit/determinism_lower_twice.zig` enforces byte-equality across two lowerings | closed |
| T-02-16 | Spoofing | Use-after-free of deal_ir_json buffer | mitigate | `cli/src/main.rs:899-902` Cow-clone (`to_vec()`) immediately followed by `ffi::deal_free(handle)`; comment cites "Pitfall 3 gpa.dupe-before-free pattern" | closed |
| T-02-17 | Repudiation | IR-LOCK approval without record | mitigate | `.planning/phases/02-prove-the-pipeline/02-IR-LOCK.md` records Status APPROVED on 2026-05-21 by you@example.com; `.planning/decisions/ADR-deal-ir-v0.md:3` flips `Status: LOCKED` per D-37 | closed |
| T-02-18 | Tampering | Per-handle arena overflow on multi-million-element model | accept | See Accepted Risks Log (R-02-18) | closed |
| T-02-19 | Information disclosure | source_file paths in IR leak absolute paths | mitigate | `src/lowering.zig:17` doc-comment cites T-02-19; `src/lowering.zig:37,51,58,235,302,327,343,358,578` `source_file: []const u8` carried verbatim from sema's relative path; spec/ir/v0/README.md documents the contract | closed |
| T-02-20 | Tampering | Hidden internal helper exported to C ABI | mitigate | `src/lib.zig:443` `pub fn deal_lower_internal(...)` declared without `export` keyword and without `callconv(.c)`; line 441-442 doc-comment asserts `nm -gU` returns 0 for the symbol; `src/lib.zig:498` `deal_format_internal` follows same pattern | closed |
| T-02-21 | Tampering | Schema bundle tampering between phases | mitigate | `cli/src/schema_registry.rs:86-88` runtime SHA256 check via `verify_sha256()` against `EXPECTED_SYSML_SHA256` / `EXPECTED_KERML_SHA256` constants; failure produces "Schema bundle tamper-detection failure" error (line 129); test `sha256_pins_match` (line 310) asserts match in CI | closed |
| T-02-22 | Spoofing | Network access during validation (defeating offline-by-construction) | mitigate | `cli/src/schema_registry.rs:3-4,9` doc-comment "offline-by-construction (no network access, no reqwest)"; `LocalBundleRetriever` (line 39) returns Err for unknown URIs (line 105); `Cargo.toml:13` `jsonschema = { version = "0.46", default-features = false }` excludes reqwest feature | closed |
| T-02-23 | Spoofing | @id collision via UUID v5 namespace change | mitigate | `cli/src/sysml_v2.rs:31` `const SYSML_NAMESPACE: Uuid = Uuid::from_u128(0x0e72_4d54_7e1a_4b32_9e8c_8e62_2d8f_1234);` (fixed constant); line 37 `Uuid::new_v5(&SYSML_NAMESPACE, qualified_path.as_bytes())`; ADR-deal-ir-v0.md + `tests/golden/sysml-v2/README.md:80-83` document that the constant MUST NEVER change | closed |
| T-02-24 | Information disclosure | SysML output paths leaking workspace-absolute paths | mitigate | IR carries workspace-relative `source_file` (T-02-11/T-02-19 mitigations); `cli/src/sysml_v2.rs` consumes IR bytes and emits qualified-path strings + UUIDs only (no filesystem-path interpolation); `tests/golden/sysml-v2/` fixtures contain no absolute paths | closed |
| T-02-25 | Denial of service | Validator compile cost (Pitfall 4) | mitigate | `cli/src/schema_registry.rs:175` `static CACHED_VALIDATOR: OnceLock<Arc<Validator>> = OnceLock::new();`; lines 182,211 build_validator() stores via OnceLock; test at lines 343-351 asserts second call < 100ms | closed |
| T-02-26 | Repudiation | Golden fixtures auto-accepted via cargo insta accept --all | mitigate | `tests/golden/sysml-v2/README.md` documents dual-gate architecture (Gate 1 byte-exact in `cli/tests/golden_sysml_v2.rs`; Gate 2 schema-validity in `cli/tests/golden_fixture_schema_validity.rs` — 9 tests including rolled-up walker); Gate 2 catches typos in hand-authored `.expected.json` files INDEPENDENT of emitter byte-match (WARNING-03). README "Why Two Gates?" section (lines 36-44) explains the blind-spot closure | closed |
| T-02-27 | Tampering | preserve_order leak via uuid transitive dep | mitigate | Same key_order.rs guard as T-02-02; `Cargo.toml:10` comment explicitly forbids `preserve_order`; `Cargo.toml:24-25` `uuid = { version = "1", features = ["v5"] }` — only the v5 feature flag is enabled; CI runs `cargo test` which exercises `serde_json_emits_alphabetical_keys` | closed |
| T-02-28 | Tampering | Adversarial comment text containing newlines / format codes | mitigate | `src/fmt.zig:364` `writeComment(c)`; line 49 `buf.appendSlice(allocator, s)` writes bytes verbatim; no `std.fmt.format`/`print` path takes comment text as format directive; comment text is passed through as byte slices via `appendSlice` | closed |
| T-02-29 | Spoofing | Use-after-free of deal_format buffer | mitigate | `cli/src/main.rs:560` `let bytes = std::slice::from_raw_parts(out_ptr, out_len).to_vec();` Cow-clones formatted bytes before `deal_free` — same gpa.dupe pattern as T-02-13/T-02-16 | closed |
| T-02-30 | Tampering | Non-atomic in-place write corrupts source file | mitigate | `cli/src/main.rs:568-602` `write_file_atomic` writes to temp file in same directory and `std::fs::rename` to destination (atomic on POSIX within same filesystem); `cli/tests/fmt_subcommand.rs::fmt_inplace_edit_replaces_file` verifies behavior | closed |
| T-02-31 | Denial of service | Pathological comment count (10K comments on one decl) | accept | See Accepted Risks Log (R-02-31) | closed |
| T-02-32 | Repudiation | Format normalization silently rewriting code | mitigate | `tests/unit/fmt_roundtrip.zig` runs idempotency test `format(format(x)) == format(x)` across all 19 showcase files; any silent AST rewrite that changes structure is caught when re-parse produces a different formatted byte sequence; 19/19 passing per 02-05-SUMMARY.md | closed |
| T-02-33 | Tampering | Trailing comma reintroduction (E0122 regression) | mitigate | `src/fmt.zig` emits comma-separated lists without trailing comma; round-trip test in `tests/unit/fmt_roundtrip.zig` re-parses formatted output — any leaked trailing comma would surface as E0122 from Phase 1.5 lexer/parser strictness; 19/19 round-trip tests pass | closed |
| T-02-34 | Spoofing | Subprocess invocation of `git config` from CLI | mitigate | `cli/src/main.rs:452-466` uses `std::process::Command::new("git").args(["config", "user.name"])` with explicit argv (no shell); failure exits 2 with descriptive error per FS-3; `cli/tests/fmt_subcommand.rs` has FS-3 identity gate test | closed |
| T-02-35 | Tampering | LM-3 quote normalization corrupting strings containing literal quotes | mitigate | `src/fmt.zig:1132-1136` `appendJsonStringEscaped` analog escapes `"`, `\`, `\n`, `\r`, `\t` correctly; fmt_roundtrip catches any escape-failure via idempotency check (a malformed escape would fail to re-parse to identical AST) | closed |
| T-02-36 | Repudiation | False-GREEN phase exit (Phase 1.5 incident pattern) | mitigate | `build.zig:264-266` `gate_2_step.dependOn(gate_1_5_step); gate_2_step.dependOn(test_step);` inherits Phase 1.5; `build.zig:278-287` `phase-2-gate-fresh` shells out to `scripts/verify-fresh-worktree.sh phase-2-gate`; `02-VERIFICATION.md` maps every ROADMAP criterion to a runnable command + evidence link; 02-VIEWER-SMOKE.md is a separate checkpoint | closed |
| T-02-37 | Information disclosure | Showcase JSON uploaded to public SysON instance leaks model details | accept | See Accepted Risks Log (R-02-37) | closed |
| T-02-38 | Tampering | argv injection in verify-fresh-worktree.sh | mitigate | `scripts/verify-fresh-worktree.sh:75` `GATE_STEP="$1"` captured; line 128 `zig build "$GATE_STEP"` quoted; line 58 comment "Valid gate-step names are alphanumeric + hyphen only"; zig rejects unknown step names harmlessly | closed |
| T-02-39 | Denial of service | Viewer-import smoke blocked by external service unavailability | accept | See Accepted Risks Log (R-02-39) | closed |
| T-02-40 | Tampering | Authoritative CI command drift | mitigate | `build.zig:257-263` NOTE block "THE CI-AUTHORITATIVE TWO-STEP COMMAND IS: `zig build phase-2-gate-fresh && cargo test --workspace`"; `02-VERIFICATION.md:115,130,221` cites the identical two-step command in the Sign-Off section | closed |
| T-02-41 | Tampering | `deal parse --json` silently extends the D-32 envelope contract | mitigate | `cli/tests/parse_subcommand.rs:88-93,152-157` asserts top-level `diagnostics` / `summary` keys are ABSENT in `deal parse --json` stdout on success — explicit WARNING-05 Option A mitigation test; doc-comment block lines 5-15 documents the invariant; `02-VERIFICATION.md` §"Public Contract Surfaces Locked" item 2 records that D-32 is diagnostic-only | closed |
| T-02-SC | Tampering | Supply chain (npm/pip/cargo installs) | mitigate | Single logical threat repeated across all 6 plans. Evidence: `Cargo.toml:5-26` `[workspace.dependencies]` pins every crate (clap 4.6, serde 1, serde_json 1, jsonschema 0.46, anstream 1, owo-colors 4, anyhow 1, thiserror 2, insta 1.47, uuid 1.x v5-only). `02-RESEARCH.md` lines 185-209 records slopcheck `[OK]` audit. `tests/schemas/README.md:38-41` SHA256-pins the bundled OMG schemas. `cli/src/schema_registry.rs:24-29,87-88` runtime-verifies the schema digests. No new dep added by Plans 02-02, 02-03, 02-05, or 02-06 beyond Plan 02-01 + Plan 02-04's `uuid`. | closed |

*Status: closed (mitigation evidence located in committed code or accepted-risk log entry present)*
*Disposition: mitigate · accept · transfer (no transfer dispositions used in Phase 2)*

---

## Accepted Risks Log

Seven Phase 2 threats are declared `accept`. Each is documented below with rationale taken verbatim (lightly edited) from the corresponding PLAN.md mitigation column. Accepted risks do not resurface in future audit runs.

| Risk ID | Threat Ref | Rationale | Accepted By | Date |
|---------|------------|-----------|-------------|------|
| R-02-04 | T-02-04 (Information disclosure) | Comments are author-controlled source; same trust level as code; no automated redaction. The DEAL source tree is treated as repository-public content; any sensitive material in comments is the author's responsibility (same as code identifiers). | Plan 02-01 planner | 2026-05-21 |
| R-02-06 | T-02-06 (Denial of service) | Phase 1 lexer already bounds-checks input length; comment tokens are arena-allocated (single `deal_free` releases all); existing 50-file malformed corpus exercises stress paths. A 10MB comment is parseable as a single token; arena releases the allocation when the handle is freed. | Plan 02-01 planner | 2026-05-21 |
| R-02-09 | T-02-09 (Denial of service) | Phase 1 parser already bounds nesting via fixed-depth parser stack; sema walks the AST tree which is depth-bounded by the parser. No additional recursion-depth attack surface introduced by Phase 2. | Plan 02-02 planner | 2026-05-21 |
| R-02-18 | T-02-18 (Tampering) | Phase 2 scope is 19 showcase files + 5-8 golden fixtures (well under 100K elements); ArenaAllocator scales to GB-class allocations; multi-million-element model is a Phase 6+ concern with a separate ADR if it surfaces. | Plan 02-03 planner | 2026-05-21 |
| R-02-31 | T-02-31 (Denial of service) | Phase 1 parser already bounds input size; comments are arena-allocated; emission is O(n) in comment count. A pathological comment count is exercised by the existing 19-file fmt_roundtrip showcase corpus indirectly. | Plan 02-05 planner | 2026-05-22 |
| R-02-37 | T-02-37 (Information disclosure) | Showcase is intentionally public reference content (committed to `spec/examples/showcase/` submodule); no proprietary data; viewer-upload to public SysON instance (syson.eclipse.dev) is acceptable. | Plan 02-06 planner | 2026-05-22 |
| R-02-39 | T-02-39 (Denial of service) | D-36 binary-outcome rule allows "documented blocker" as a legitimate phase-exit state; the Phase 2 gate ships even if no viewer accepts the JSON. Eclipse SysON 2025.x accepted the file as opaque attachment (textual-viewer impedance mismatch); recorded in `02-VIEWER-SMOKE.md`. | Plan 02-06 planner | 2026-05-22 |

---

## Unregistered Flags

None. SUMMARY files 02-01 through 02-05 all record "Threat Flags: None"; 02-06 SUMMARY does not contain a Threat Flags subsection but the closeout VERIFICATION.md records no new attack surface beyond the 42 register entries. No new trust boundaries, network endpoints, auth paths, or file-access patterns emerged during implementation that lack a threat-register mapping.

---

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-05-22 | 42 | 42 | 0 | gsd-security-auditor |

---

## Notes — Adversarial Verification Detail

This audit verified each declared mitigation by locating the asserted code in the files cited in the plan's mitigation column. Spot checks performed:

- **T-02-13 / T-02-16 / T-02-29 (Cow-clone before deal_free):** Verified line-ordering in `cli/src/main.rs` lines 200–227 (diagnostics path), 514–516 (index path), 560 (fmt path), and 890–902 (IR path). In every path, `to_vec()` precedes `ffi::deal_free()` — gpa.dupe-before-free pattern confirmed at the call site.
- **T-02-15 (no std.json.Stringify):** Confirmed `grep std.json.Stringify src/json.zig` returns only four matches, all inside doc-comments cautioning against the API; zero call sites.
- **T-02-20 (deal_lower_internal not in C ABI):** Verified `src/lib.zig:443` declares `pub fn` without `export` keyword and without `callconv(.c)`; same for `deal_format_internal` at line 498. The plan's nm-grep gate was asserted in the test infrastructure.
- **T-02-26 (golden fixture anchoring blind-spot):** The README documents the dual-gate architecture and Gate 2's independent schema-validity check; the literal "forbid --accept-all" warning is implicit rather than explicit, but the structural mitigation (`golden_fixture_schema_validity.rs` with 9 tests covering UUID format, @id==elementId, known @type, and alphabetical-key order at every nesting level) genuinely closes the blind spot the threat describes. No BLOCKER.
- **T-02-23 (UUID v5 namespace constant):** Confirmed the literal `0x0e72_4d54_7e1a_4b32_9e8c_8e62_2d8f_1234` at `cli/src/sysml_v2.rs:31` matches the value frozen in PLAN.md and `tests/golden/sysml-v2/README.md`. Changing this constant would shift every `@id` value emitted; the README documents this explicitly.

All 35 `mitigate`-disposition threats located in shipped code; all 7 `accept`-disposition threats logged with rationale.

---

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-05-22 by gsd-security-auditor (ASVS Level 1)
