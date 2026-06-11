---
phase: 2
slug: prove-the-pipeline
status: validated
nyquist_compliant: true
wave_0_complete: true
created: 2026-05-21
updated_by_planner: 2026-05-21
revised_by_planner: 2026-05-21
audited_by: gsd-nyquist-auditor
audit_date: 2026-05-22
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution. Populated from `02-RESEARCH.md` `## Validation Architecture`. Per-task rows below were filled in by `gsd-planner` on 2026-05-21 and reflect the 6-plan slicing in CONTEXT D-37, then revised per the BLOCKER-01 task split in Plan 02-01 (Task 1 → Task 1a + Task 1b).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Zig: `zig build test` (Zig stdlib testing). Rust: `cargo test` + (optional) `cargo nextest`. Snapshots: `insta` (Rust), per-file golden `.expected.json` checks (Zig + Rust). |
| **Config file** | `build.zig` (Zig), `Cargo.toml` workspace (Rust at `deal/Cargo.toml`) |
| **Quick run command** | `zig build test && cargo test --workspace --no-fail-fast` |
| **Full suite command** | `zig build phase-2-gate-fresh && cargo test --workspace` (the AUTHORITATIVE Phase 2 CI command) |
| **Estimated runtime** | ~45s quick, ~120–180s full (showcase 19-file round-trip + IR determinism + golden fixtures + schema validation + sema corpus included) |

---

## Sampling Rate

- **After every task commit:** Run `zig build test && cargo test --workspace --no-fail-fast`
- **After every plan completion (Plans 01–06):** Run `zig build phase-2-gate && cargo test --workspace`
- **Before `/gsd:verify-work`:** Authoritative two-step: `zig build phase-2-gate-fresh && cargo test --workspace` must exit 0 AND `deal build --target sysml-v2 tests/showcase/ --validate` exits 0 AND all 8 golden fixtures byte-match AND all 8 hand-authored `.expected.json` files pass `golden_fixture_schema_validity` (WARNING-03).
- **Max feedback latency:** ~45s quick, ~180s full

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 02-01-T1a | 01 | 1 | REQ-phase-2-5-cli-shell | T-02-03 / T-02-05 / T-02-SC | Cargo workspace + 4 stubs exit 2; links conflict resolved (option (c), cli/ claims links); workspace builds | unit + integration | `cd deal && cargo build --workspace && cargo run --bin deal -- --version && for sc in parse check fmt build; do cargo run --bin deal -- $sc /tmp/nonexistent.deal; test $? -eq 2 || exit 1; done` | yes (created by task) | ✅ green |
| 02-01-T1b | 01 | 1 | REQ-phase-2-5-cli-shell | T-02-01 / T-02-02 / T-02-05 / T-02-SC | Schema bundle SHA256-pinned in README; no preserve_order leak; subcommand stubs exit 2 (cli_smoke); alphabetical-key invariant defended (key_order) | unit + integration | `cd deal && cargo test --workspace --test key_order && cargo test --workspace --test cli_smoke && cmp tests/schemas/SysML.json spec/references/omg-sysml-v2/SysML.json && cmp tests/schemas/KerML.json spec/references/omg-kerml-v1/KerML.json && grep -qE 'SHA256\|sha256' tests/schemas/README.md` | yes (created by task) | ✅ green |
| 02-01-T2 | 01 | 1 | REQ-phase-2-prove-pipeline | T-02-04 / T-02-07 | Lexer emits comment tokens; AST gains 3 fields; snapshot refresh isolated | unit + snapshot | `cd deal && zig build test && zig build phase-1.5-gate && bash scripts/diff-only-new-fields.sh tests/snapshots/ast/showcase__*.json` | yes (created by task) | ✅ green |
| 02-02-T1 | 02 | 2 | REQ-phase-2-1-semantic-analyzer | T-02-08 / T-02-09 / T-02-10 / T-02-11 / T-02-12 | 6 blocking checks; index.json workspace-relative paths; comptime fmt strings; on-disk index.json alphabetical-key shape (D-18 / WARNING-01 mitigation) | unit + corpus + integration | `cd deal && zig build test -Dtest-filter=sema_corpus && zig build phase-1.5-gate && cargo test --workspace --test check_workspace_index` | yes (created by task; plus post-closeout `cli/tests/check_workspace_index.rs` filling the missing on-disk D-18 gate) | ✅ green |
| 02-02-T2 | 02 | 2 | REQ-phase-2-1-semantic-analyzer (CLI portion) | T-02-12 / T-02-13 / T-02-14 | D-32 envelope alphabetical; FFI Cow-clone; D-34 exit codes; no println! | integration | `cd deal && cargo test --workspace --test check_subcommand` | yes (created by task) | ✅ green |
| 02-03-T1 | 03 | 3 | REQ-phase-2-2a-ir-v0 | T-02-17 | ADR + JSON Schema + README; alphabetical-key invariant; 27 NodeKinds | unit + schema-parse | `test -f spec/ir/v0/schema.json && python3 -c 'import json; json.load(open("spec/ir/v0/schema.json"))' && test -f .planning/decisions/ADR-deal-ir-v0.md` | yes (created by task) | ✅ green |
| 02-03-T2 | 03 | 3 | REQ-phase-2-2b-ir-lowering | T-02-15 / T-02-16 / T-02-18 / T-02-19 / T-02-20 | IR comment-free per D-25; nm == 9 (post-closeout); hand-rolled emission; arena-only allocs | unit + determinism | `cd deal && zig build test -Dtest-filter=determinism.lower_twice && zig build test -Dtest-filter=property_ir_id_uniqueness && cd deal && nm -gU zig-out/lib/libdeal.a \| grep -cE 'T _?deal_' \| awk '{ if ($1==9) print "OK"; else exit 1 }'` (count is post-closeout — `deal_index_json` added as export #9; see Wave Waypoints footnote) | yes (created by task) | ✅ green |
| 02-03-T3 | 03 | 3 | REQ-phase-2-2a-ir-v0 (IR-LOCK) | T-02-17 | Recorded human review per SPEC REQ #9 #2 | manual-only (checkpoint) | (human review per 02-IR-LOCK.md; not automated) | yes (02-IR-LOCK.md created by task) | ✅ green (manual-only) |
| 02-04-T1 | 04 | 4 | REQ-phase-2-3-sysml-v2-codegen + REQ-phase-2-3a-offline-validator | T-02-21 / T-02-22 / T-02-23 / T-02-24 / T-02-25 / T-02-27 / T-02-SC | UUID v5 @id; offline validator; SHA256 schema pin; OnceLock cache; no preserve_order; no network | integration + perf | `cd deal && cargo test --workspace --test sysml_validate && cargo tree -e features --workspace \| grep -c preserve_order \| awk '{ if ($1==0) print "OK"; else exit 1 }'` | yes (created by task) | ✅ green |
| 02-04-T2 | 04 | 4 | REQ-phase-2-3b-golden-fixtures | T-02-26 | 8 fixtures byte-exact + schema-valid; README forbids accept-all; independent fixture-schema-validity gate catches typos pre-insta (WARNING-03) | golden + dedicated-schema-validity | `cd deal && cargo test --workspace --test golden_sysml_v2 && cargo test --workspace --test golden_fixture_schema_validity` | yes (created by task) | ✅ green |
| 02-05-T1 | 05 | 4 | REQ-phase-2-4-formatter | T-02-28 / T-02-29 / T-02-32 / T-02-33 / T-02-35 | Round-trip byte-equal AST; no IR walk in fmt; no doc-comment reflow; nm == 9 (post-closeout) | round-trip | `cd deal && zig build test -Dtest-filter=fmt_roundtrip && nm -gU zig-out/lib/libdeal.a \| grep -cE 'T _?deal_' \| awk '{ if ($1==9) print "OK"; else exit 1 }'` (count is post-closeout — `deal_index_json` added as export #9; see Wave Waypoints footnote) | yes (created by task) | ✅ green |
| 02-05-T2 | 05 | 4 | REQ-phase-2-4-formatter (CLI portion) | T-02-30 / T-02-31 / T-02-34 | Atomic rename-from-temp; FS-3 identity gate; stdin/stdout support | integration | `cd deal && cargo test --workspace --test fmt_subcommand` | yes (created by task) | ✅ green |
| 02-06-T1 | 06 | 5 | REQ-phase-2-5-cli-shell (final parse subcommand) | T-02-13 / T-02-41 | Cow-clone before deal_free; raw AST emission on --json (no envelope wrap per WARNING-05 Option A); no stubs remain | integration | `cd deal && cargo test --workspace --test parse_subcommand && grep -c 'not yet implemented' deal/cli/src/main.rs \| awk '{ if ($1==0) print "OK"; else exit 1 }'` | yes (created by task) | ✅ green |
| 02-06-T2 | 06 | 5 | REQ-phase-2-gate | T-02-36 / T-02-38 | phase-2-gate + phase-2-gate-fresh; backward-compat for phase-1.5-gate-fresh | integration | `cd deal && zig build phase-2-gate && zig build phase-1.5-gate-fresh && zig build phase-2-gate-fresh` | yes (created by task) | ✅ green |
| 02-06-T3 | 06 | 5 | REQ-phase-2-gate | T-02-36 / T-02-40 | All 6 ROADMAP criteria + 10 REQ IDs + 14 SPEC criteria mapped to evidence | doc-validation | `test -f .planning/phases/02-prove-the-pipeline/02-VERIFICATION.md && grep -c 'REQ-phase-2-' .planning/phases/02-prove-the-pipeline/02-VERIFICATION.md \| awk '{ if ($1>=10) print "OK"; else exit 1 }'` | yes (created by task) | ✅ green |
| 02-06-T4 | 06 | 5 | REQ-phase-2-3-sysml-v2-codegen (viewer smoke) | T-02-37 / T-02-39 | Recorded outcome per D-36 binary-outcome rule | manual-only (checkpoint) | (human-driven per 02-VIEWER-SMOKE.md; not automated — see Manual-Only Verifications below) | yes (02-VIEWER-SMOKE.md created by task) | ✅ green (manual-only — documented blocker per D-36) |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

### Wave Waypoints — `nm` export-count progression (narrative footnote)

The `nm -gU libdeal.a | grep -cE 'T _?deal_'` gate appears in two rows (02-03-T2 and 02-05-T1). The count was originally designed as a per-wave waypoint:

| Wave | Plan landing | Expected count | Rationale |
|------|--------------|----------------|-----------|
| Wave 3 (Plan 02-03 lands) | `deal_ir_json` added as export #7 | 7 | T1a opened with 6 exports (Phase 1 + Phase 1.5); Plan 02-03 adds the IR JSON FFI |
| Wave 4 (Plan 02-05 lands) | `deal_format` added as export #8 | 8 | Plan 02-05 adds the formatter FFI |
| Post-closeout (Phase 2 SPEC §criterion 1 fix) | `deal_index_json` added as export #9 | 9 | Phase 2 closeout (commit 72907ed) adds the workspace index FFI; backwards-compatible per 02-VERIFICATION.md `## Public C ABI` |

Both 02-03-T2 and 02-05-T1 now gate at `== 9` because the closeout export landed after both waves. The intent of the 7 → 8 → 9 progression as per-wave waypoints is preserved here as a narrative note; the in-row commands assert the post-closeout terminal value. If a future phase changes the export count, BOTH rows must be updated together with this table.

### 02-02-T1 — note on the closed-out index.json on-disk gate

Row 02-02-T1's original command referenced a Zig test `sema_corpus.index_json_alphabetical_keys` that does not exist in `tests/unit/sema_corpus.zig` (the actual tests are `sema.corpus.regression` and `sema.corpus.showcase_clean`), and a build option `-DINDEX_OUT_DIR=...` that is not declared in `build.zig`. The behavior the planner intended to gate — that `deal check <workspace-dir>` writes a `.deal/index.json` with alphabetical top-level keys — is now gated by `cli/tests/check_workspace_index.rs::check_workspace_writes_index_json_with_alphabetical_keys`, which runs the built `deal` binary against `tests/showcase/`, asserts exit 0, parses the resulting `.deal/index.json`, asserts top-level keys are alphabetical (D-18), and asserts the `elements`-inner keys are alphabetical. This is the in-Rust analog of the originally-intended Zig sub-test and exercises the workspace-mode index merge written in `run_check` (cli/src/main.rs) after the Phase 2 closeout. The Zig-side index.json shape continues to be covered by `sema.corpus.regression` and `sema.corpus.showcase_clean` via `zig build test -Dtest-filter=sema_corpus`.

---

## Wave 0 Requirements

Test infrastructure scaffolding (must land before any later task can be verified). Plan 02-01 owns ALL Wave 0 work, split across Task 1a (workspace scaffold) and Task 1b (schema bundle + defensive tests):

**Task 1a (workspace scaffold + 4 subcommand stubs):**
- [ ] `deal/Cargo.toml` — workspace root with pinned `[workspace.dependencies]`
- [ ] `deal/cli/Cargo.toml` — CLI crate manifest with `links = "deal"`
- [ ] `deal/cli/build.rs` — invokes `zig build` + links static libdeal
- [ ] `deal/cli/src/{main.rs, ffi.rs, render.rs}` — clap dispatch + 6 FFI extern decls + render stub
- [ ] `deal/tests/ffi/Cargo.toml` modified — `links = "deal"` removed (resolution: cli/ claims the native lib per Lane 8 option (c))

**Task 1b (schema bundle + defensive tests):**
- [ ] `deal/cli/tests/key_order.rs` — defensive serde_json alphabetical-key test (Pitfall 1)
- [ ] `deal/cli/tests/cli_smoke.rs` — `--version` + 4 subcommand-stub exit-code tests (1 + 4 = 5 tests)
- [ ] `tests/schemas/SysML.json` + `tests/schemas/KerML.json` — bundled OMG schemas (byte-identical to spec/references/)
- [ ] `tests/schemas/README.md` — schema provenance + SHA256 + version pins

**Task 2 (comment attachment):**
- [ ] `deal/scripts/diff-only-new-fields.sh` — snapshot-regen discipline script (Pitfall 2)
- [ ] AST comment fields added — `leading_comments` / `trailing_comments` / `doc_comment` on all declaration variants

After Plan 02-01 completes, Plans 02-02 through 02-06 each add their own per-plan test infrastructure (sema_corpus + index.json alphabetical-key sub-test, determinism_lower_twice, golden fixtures + golden_fixture_schema_validity, fmt_roundtrip, viewer-smoke) as part of their normal task execution.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions | Plan Owner |
|----------|-------------|------------|-------------------|------------|
| IR-LOCK checkpoint review | REQ-phase-2-2a-ir-v0 (SPEC REQ #9 #2) | Human judgment required to lock the public IR contract surface | Per 02-03-PLAN Task 3 "how-to-verify"; record outcome in 02-IR-LOCK.md with timestamp + reviewer | 02-03 Task 3 |
| SysML v2 viewer import smoke test | REQ-phase-2-prove-pipeline (success criterion 6) | Third-party viewer (Eclipse SysON / OpenMBEE Pilot) has no headless CLI; requires GUI interaction | Per 02-06-PLAN Task 4 "how-to-verify"; record outcome in 02-VIEWER-SMOKE.md with screenshot if successful; binary outcome per D-36 (success OR documented blocker) | 02-06 Task 4 |
| Diagnostic visual quality | REQ-phase-2-5-cli-shell (rustc-quality diagnostics) | Color, alignment, label placement need human eyeballs | Run `deal check tests/regressions/sema/*.deal --color always` and visually inspect 6 diagnostic categories (one per check); compare against rustc/ariadne reference. Optional — automated check_subcommand.rs already covers exit codes + ANSI escape presence. | 02-02 Task 2 (sampled during execution) |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies (manual-only items explicitly listed above)
- [x] Sampling continuity: no 3 consecutive tasks without automated verify (every plan ≥1 automated task)
- [x] Wave 0 covers all MISSING references (Plan 02-01 owns Wave 0 in full, split across Task 1a + Task 1b)
- [x] No watch-mode flags (all commands one-shot)
- [x] Feedback latency < 120s for quick suite, < 180s for full
- [x] `nyquist_compliant: true` set in frontmatter (above)

**Approval:** planner-approved 2026-05-21; revised 2026-05-21 per BLOCKER-01 (Task 1 split into 1a + 1b), WARNING-01 (index.json alphabetical-key gate added to 02-02-T1), WARNING-03 (golden_fixture_schema_validity added to 02-04-T2), WARNING-05 (parse raw-AST emission added to 02-06-T1); orchestrator-executed and audited 2026-05-22.

---

## Validation Audit 2026-05-22

Retroactive Nyquist audit run after Phase 2 was verified GREEN (`02-VERIFICATION.md`). The phase's underlying behavior was already covered by passing tests, but the **automated commands as written in this VALIDATION.md** had three classes of defects that would have produced false-GREEN runs going forward.

| Class | Issue | Affected rows | Resolution |
|-------|-------|---------------|------------|
| A | `cargo test --workspace -- <name>` filters by test FUNCTION name, not binary name → 0 tests run, exit 0 (false-GREEN). | 02-01-T1b (×2), 02-02-T2, 02-04-T1, 02-04-T2 (×2), 02-05-T2, 02-06-T1 | Rewrote each as `cargo test --workspace --test <name>` — now runs 1/3/7/4/8+9/6/7 tests respectively (count per row). |
| B | `02-02-T1` referenced a phantom Zig test (`sema_corpus.index_json_alphabetical_keys`) and a non-existent build option (`-DINDEX_OUT_DIR=...`). The on-disk `.deal/index.json` alphabetical-key invariant (added by Phase 2 closeout commit 72907ed) had no automated test. | 02-02-T1 | Wrote `cli/tests/check_workspace_index.rs::check_workspace_writes_index_json_with_alphabetical_keys` — runs `deal check tests/showcase/`, parses the produced `.deal/index.json`, asserts D-18 alphabetical ordering at both top-level and `elements`-inner. Panic-safe RAII cleanup. Rewrote the 02-02-T1 command to invoke it. |
| C | `nm` export-count gates expected the pre-closeout count (7 then 8) but the closeout added `deal_index_json` as export #9, so the commands would now exit 1. | 02-03-T2 (was 7), 02-05-T1 (was 8) | Updated both literals to `9` and added a `### Wave Waypoints` narrative footnote preserving the original 7 → 8 → 9 per-wave waypoint intent. |

| Metric | Count |
|--------|-------|
| Gaps found | 11 (8 Class A + 1 Class B + 2 Class C) |
| Resolved | 11 |
| Escalated | 0 |
| New tests created | 1 (`cli/tests/check_workspace_index.rs`) |
| Implementation files modified | 0 (auditor constraint respected) |

**Post-audit verification:** `zig build test && cargo test --workspace --no-fail-fast` → both exit 0; 14 cargo binaries report green; new `check_workspace_index` binary contributes 1 additional passing test exercising the previously-uncovered behavior.

**Auditor:** `gsd-nyquist-auditor` on 2026-05-22.
**Orchestrator:** validate-phase workflow invoked via `/gsd-validate-phase 2`.
