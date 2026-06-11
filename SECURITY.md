# SECURITY.md — Phase 05.1 Security Audit

**Phase:** 05.1 — Resolve Pending TODOs and Integrate deal-stdlib Functionality
**Audited:** 2026-06-09
**ASVS Level:** 1
**Auditor:** gsd-security-auditor (claude-sonnet-4-6)
**Result:** SECURED — 11/11 threats closed

---

## Threat Verification

| Threat ID | Category | Disposition | Status | Evidence |
|-----------|----------|-------------|--------|----------|
| T-05.1-A01 | Tampering | mitigate | CLOSED | spec HEAD 74dfea5 is a fast-forward descendant of dd3bb78; commit 814b63e body explicitly states "74dfea5 is a fast-forward descendant of dd3bb78" and "7ce59bc is NOT included (D-07 honored)"; no force-push artifact in git log |
| T-05.1-A02 | DoS | mitigate | CLOSED | `scripts/verify-fresh-worktree.sh` exists and implements: dirty-tree rejection (line 87), ephemeral worktree creation (line 97), `git submodule update --init --recursive` (line 126), showcase materialization sanity check (line 133), gate step execution (line 221); 01-SUMMARY.md records gate ran 65/66 (1 pre-existing E2503 carryover) |
| T-05.1-A03 | Repudiation | accept | CLOSED | See accepted risks log below |
| T-05.1-B01 | DoS | mitigate | CLOSED | Guard at `cli/src/main.rs:433`: `if !use_stdlib_check { ... deal_index_json ... }` — `deal_index_json` is skipped for stdlib-check handles; `grep -c 'deal_index_json' cli/tests/e2500_cli.rs` = 0 (verified) |
| T-05.1-B02 | DoS | accept | CLOSED | See accepted risks log below |
| T-05.1-C01 | DoS | mitigate | CLOSED | `parser_deal.zig:1938`: `_ = p.advance(); continue;` in comma arm; `parser_deal.zig:1985`: `_ = p.advance(); continue;` in unknown-verification-key else arm; `parser_dealx.zig:834`: `_ = p.advance(); continue;` in object-field unknown-token arm — every diagnostic arm advances cursor before continue |
| T-05.1-C02 | Tampering | mitigate | CLOSED | `src/diagnostics.zig:89`: `pub const e_annotation_comma_separator = "E0123"` (hard .err); `src/diagnostics.zig:91`: `pub const e_unknown_verification_key = "E0124"` (hard .err); `parser_deal.zig:1924-1928` emits E0123 on comma; `parser_deal.zig:1979-1983` emits E0124 on unknown key; corpus pins m21→E0123, m22→E0124 at `tests/unit/recovery_corpus.zig:159-162` |
| T-05.1-C03 | Info Disclosure | mitigate | CLOSED | `parser_deal.zig:1928`: `.span = key_tok.span`; `parser_deal.zig:1935`: `.span = key_tok.span`; `parser_deal.zig:1983`: `.span = key_tok.span` — all three diagnostic sites use lexer-produced `key_tok.span`; no new span arithmetic |
| T-05.1-D01 | DoS | mitigate | CLOSED | `src/expr.zig:283-286`: `// T-05.1-D01 DoS guard: loop terminates on .r_bracket OR .eof.` + `while (parser.peek().tag != .r_bracket and parser.peek().tag != .eof)` dual-sentinel; `expr.zig:308-314`: EOF emits E0402 (e_unclosed_bracket); `expr.zig:316`: `try parser.expect(.r_bracket)` terminates the loop |
| T-05.1-D02 | Tampering | mitigate | CLOSED | `tests/snapshots/ast/showcase__model__vehicle.dealx.json`: 0 `__array__` hits, 1 `array_literal` hit; `tests/snapshots/ast/showcase__packages__requirements__system.deal.json`: 0 `__array__` hits, 1 `array_literal` hit; SysML/reqif goldens contain no `__array__` (grep returns 0); 04-SUMMARY.md records regenerate-then-re-run-clean gate executed |
| T-05.1-D03 | Info Disclosure | mitigate | CLOSED | `src/expr.zig:317`: `const span = ast.Span{ .start = tok.span.start, .end = close.span.end }` — span from opening `[` token start to closing `]` token end, both lexer-produced; `close` is produced by `parser.expect(.r_bracket)` at line 316, within buffer |
| T-05.1-SC | Tampering | mitigate | CLOSED | `build.zig.zon:11`: `.dependencies = .{}` (no Zig deps); `cli/Cargo.toml:52`: `tempfile = "3"` is a pre-existing dev-dependency (present since Phase 4, not added in Phase 05.1 per git log); RESEARCH.md explicitly states "No new packages are installed in this phase. N/A." |

---

## Accepted Risks Log

### T-05.1-A03 — Repudiation: gitlink bumped without recorded rationale

**Accepted.** Rationale is reasonable:

- Commit 814b63e (`chore(spec): bump gitlink for Wave-A grammar reconciliation`) records the full rationale in its body: spec advances from dd3bb78 to 74dfea5, enumerates each change (D-05/I5, D-03/I2, D-01/I1, I6, SD-15a/SD-15b, NC-1), and explicitly notes D-07 exclusion of 7ce59bc.
- Single-maintainer repository; full audit trail in git log.
- No multi-party authorization is required or expected at ASVS Level 1.

### T-05.1-B02 — DoS: malformed .deal fixture in CLI integration tests

**Accepted.** Rationale is reasonable:

- E25xx CLI test fixtures (`mixed_mass_units.deal`, `unknown_unit.deal`, `mass_for_voltage.deal`) are dimension-wrong but syntactically well-formed; they exercise the sema/dimensional layer, not the parser recovery path.
- Parser robustness against adversarial/malformed input is covered separately by the 50-file malformed corpus (Plan 03 recovery_corpus.zig regression gate, green at 55 files after Phase 05.1).
- Risk is bounded: fixtures are created inline in test functions within controlled tempdir environments; no external fixture file ingestion.

---

## Unregistered Flags

None. All four SUMMARY.md `## Threat Flags` sections explicitly state "None — no new network endpoints, auth paths, or schema changes introduced."

---

## Audit Notes

1. **T-05.1-C01 scope clarification:** The threat register cites `parser_dealx.zig` as the primary file, but the new diagnostic arms in this phase live in `parser_deal.zig` (annotation body loop at :1916-1939 and verification key loop at :1976-1986). Both files satisfy the invariant. The parser_dealx.zig loop at line 834 also advances before continue, confirming the pattern holds project-wide.

2. **T-05.1-SC tempfile note:** `tempfile = "3"` appears in `cli/Cargo.toml` but was introduced in a prior phase (git log for Cargo.toml shows it predates Phase 05.1). Phase 05.1 added no new Cargo, Zig, or Python package dependencies.

3. **`__array__` residual comments:** Two occurrences of `__array__` remain in source as comments only (`src/fmt.zig:766` doc comment, `src/json.zig:448` inline comment). These are not executable references; the synthetic encoding is fully removed from all code paths.

---
---

# SECURITY.md — Phase 05.2 Security Audit

**Phase:** 05.2 — Implement calc/constraint grammar (SD-21/22/23) in Zig compiler + Rust codegen + editor tooling
**Audited:** 2026-06-11
**ASVS Level:** 1
**Auditor:** gsd-security-auditor (claude-sonnet-4-6)
**Result:** SECURED — 13/13 threats closed

---

## Threat Verification

| Threat ID | Category | Disposition | Status | Evidence |
|-----------|----------|-------------|--------|----------|
| T-05.2-W0-01 | Tampering | mitigate | CLOSED | `build.zig:699-700`: `phase-05.2-wave0-gate-fresh` step delegates to `scripts/verify-fresh-worktree.sh phase-05.2-gate`; script at line 125-126 runs `git submodule update --init --recursive` and fails fast if it fails; wave0 gate PASSED per 01-SUMMARY.md |
| T-05.2-W0-02 | Repudiation | mitigate | CLOSED | `spec/grammar/DESIGN-DECISIONS.md:455,551`: both "SD-15a" (line 455) and "SD-21" (line 551) present; `spec/grammar/deal.ebnf:545,793,1647`: both "CalcDefinition" (line 545) and "AnnotationKey" (line 1647) present — merge preserved both sides |
| T-05.2-W1-01 | Tampering | mitigate | CLOSED | `src/lexer.zig:875-883`: 0xC2/0xB1 dispatch inserted before the generic `c0 >= 0x80` path (line 892); `src/lexer.zig:916-920`: `peekByte` returns 0 when `idx >= self.source.len` (bounds-safe sentinel); truncated 0xC2 at EOF falls through to generic path per comment at line 879 |
| T-05.2-W1-02 | Tampering | mitigate | CLOSED | `src/keywords.zig`: "sig" absent from `global_keywords` map (grep for `kw_sig` = 0 in keywords.zig); `src/lexer.zig:96`: explicit note "kw_sig is NOT added — D-07"; `tests/unit/lexer_keywords.zig:106-109`: unit assertion `global_keywords.get("sig") == null` |
| T-05.2-W2-01 | DoS | mitigate | CLOSED | `src/parser_deal.zig:1411-1441` (parseCalcBody else arm): `syncToStatement` + three-tier forward progress guard (`semicolon/comma → advance`, `r_brace/eof → break`, `same position → force advance`); `src/parser_deal.zig:1518-1537` (parseConstraintBody else arm): identical three-tier guard; arena allocator backs all node allocation (parser.zig:29-30) |
| T-05.2-W2-02 | Tampering | mitigate | CLOSED | `src/parser_deal.zig:1280-1312` (`parseReturnContract`): only accepts `.ident`/`"sig"`, `.plus_minus`, `.plus`/`.slash` as ContractItem FIRST tokens; `{` (r_brace) falls to the `else => break` path and terminates the contract — no `{` can be misread as a ContractItem |
| T-05.2-W2-03 | Tampering | mitigate | CLOSED | `src/ast.zig:316`: `ConstraintDef = ElementDef` alias removed (comment at line 316; grep for live alias = 1 match which is a comment only); `src/ast.zig:322`: real `ConstraintDefinition` struct; `src/ast.zig:691`: `constraint_def: ConstraintDefinition` in Payload union; Zig exhaustive-switch enforced at compile time across all 5 dispatch sites (build compiles green per gate) |
| T-05.2-W3-01 | Tampering | mitigate | CLOSED | `src/sema.zig:1262-1290` (`checkConstraintRefCycle`): `max_hops = a.table.entries.count() + 1` at line 1268; `seen = std.StringHashMap(void)` at line 1270 (visited-set); loop bounded by `hops < max_hops` at line 1275; E2611 emitted on cycle at line 1277 |
| T-05.2-W3-02 | DoS | mitigate | CLOSED | `tests/malformed/m25_calc_out_param_expr.deal`, `tests/malformed/m26_calc_no_return.deal`, `tests/malformed/m27_require_not_boolean.deal` all exist; pinned in `tests/unit/recovery_corpus.zig:173-190` (three corpus entries with empirical-truth pin policy documented inline); sema operates on already-recovered AST |
| T-05.2-W3-03 | Tampering | mitigate | CLOSED | `tests/unit/sema_calc.zig:305-337` (`sema_calc.dimensional_no_regression`): asserts zero E25xx diagnostics for a dimensionally-consistent calc; test passes per gate (10/10 sema_calc tests pass) |
| T-05.2-W4-01 | Tampering | mitigate | CLOSED | `spec/ir/v0/schema.json`: `grep -c '"calc_def"'` = 1 (in NodeKind enum); `cargo test --workspace` schema-validation tests pass per gate (golden_fixture_schema_validity_09_calc_def + golden_fixture_schema_validity_10_constraint_def) |
| T-05.2-W4-02 | Tampering | mitigate | CLOSED | `cli/src/sysml_v2.rs:192-194`: `"constraint_def"` routes to dedicated `emit_constraint_def` (line 193), NOT to `emit_generic_element`; `cli/src/sysml_v2.rs:615`: `emit_constraint_def` emits Predicate type; golden fixture `tests/golden/sysml-v2/10-constraint-def.expected.json` asserts Predicate byte-for-byte; cargo golden test passes per gate |
| T-05.2-W4-03 | Tampering | mitigate | CLOSED | `build.zig:844-853`: `phase-05.2-gate-fresh` step; `scripts/verify-fresh-worktree.sh:125-126`: `git submodule update --init --recursive`; 05.2-05-SUMMARY.md records "phase-05.2-gate-fresh: PASSED"; spec submodule at 22bfa85 confirmed pushed to origin/main |
| T-05.2-W4-04 | Tampering | mitigate | CLOSED | `spec/ir/v0/schema.json`: `grep -c '#/$defs/Precision'` = 3 (calc-return, attribute-value, require-threshold payloads all $ref the same shared Precision $def); schema validation via cargo golden tests rejects malformed/duplicated shapes |
| T-05.2-W5-01 | DoS | mitigate | CLOSED | `lsp/src/semantic_tokens.rs:283`: `"calc_def"` arm present; tree-sitter grammar produces ERROR nodes on malformed input (error-tolerant, no panic); `lsp/src/hover.rs:162-165`: `calc_def` arm falls through to `None` if `calc_def_signature` returns None — unknown node kinds produce no crash |
| T-05.2-W5-02 | Tampering | mitigate | CLOSED | `tree-sitter-deal/queries/highlights.scm:121`: `(contract_item (sig_keyword) @keyword.control)` — contextual by grammar structure (sig_keyword only reachable from contract_item); `/Users/dunnock/projects/deal-lang/vscode-deal/syntaxes/deal.tmLanguage.json:175-180`: `sig-precision-keyword` entry with `"match": "(?<![\\w-])sig(?=\\s+\\d)"` — captures `sig 4` but not bare `sig` identifier |
| T-05.2-SC | Tampering | accept | CLOSED | See accepted risks log below |

---

## Accepted Risks Log

### T-05.2-SC — Tampering: npm/pip/cargo package installs

**Accepted.** No new package installs were introduced in Phase 05.2:

- `git log -- Cargo.toml Cargo.lock` shows no Cargo manifest changes during the phase 05.2 commit range (11852e0..c6e990e).
- `tree-sitter-cli` version pinned at 0.25.10 — unchanged from prior phases (confirmed: no package.json changes in tree-sitter-deal during this phase).
- Phase 05.2 RESEARCH.md Package Legitimacy Audit explicitly states "none" for all wave plans.

---

## Unregistered Flags

None. Phase 05.2 SUMMARY.md files for Plans 01-06 all report no new network endpoints, auth paths, file access patterns, or schema changes beyond the declared scope of each wave. Plan 04 SUMMARY explicitly states "Threat Flags: None."

---

## Audit Notes

1. **T-05.2-W2-01 sync-set discrepancy:** The threat register states `.kw_calc in STATEMENT_SYNC` as one of three mitigation elements. The actual `STATEMENT_SYNC` set (`src/parser_deal.zig:409-417`) contains only punctuation tokens (`semicolon`, `comma`, `r_brace`, `r_paren`, `r_bracket`). `kw_calc` is not there. It is in `canStartDefinition` and `isKeyword` helpers (lines 2643, 2660). The effective DoS protection is provided by the other two declared elements — the three-tier forward progress guard in the `else` arms of `parseCalcBody` (lines 1431-1440) and `parseConstraintBody` (lines 1526-1536) — which prevent infinite loops regardless of what token is encountered. The arena allocator (line 29-30) bounds memory. This is a documentation inaccuracy in the threat description, not a missing mitigation. Threat is CLOSED on the strength of the two elements that ARE present.

2. **T-05.2-W2-01 DEFINITION_SYNC gap:** `kw_calc` is also absent from `DEFINITION_SYNC` (lines 421-442). This means definition-level error recovery (`syncToDefinition`) will skip over a `calc def` keyword rather than stopping at it. The top-level parse loop uses `canStartDefinition` (which includes `kw_calc`) before deciding whether to sync, so the gap is benign for the top-level loop. No DoS vector identified.

3. **T-05.2-W3-01 implementation detail:** `checkConstraintRefCycle` uses the `constraint_chain_map` (not the specialization graph) for cycle detection. The map is seeded at sema pass 1 (line 510-521 of sema.zig); cycles in the map are detected at pass 2. This mirrors `checkSpecializationCycle` exactly as documented in the threat mitigation.

4. **Human verification item (T-05.2-W5-02 partial):** The TextMate grammar contextual-sig pattern was verified via `node -e "JSON.parse(...)"` and `npm run compile/lint/test:unit` (9/9 tests pass). Visual rendering in VS Code was approved by the user during Plan 06 Task 3 checkpoint (commit `6c359aa` in vscode-deal). This is recorded in `05.2-VERIFICATION.md` under `human_verification_status: approved_in_session`.
