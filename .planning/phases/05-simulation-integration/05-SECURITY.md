---
phase: 5
slug: simulation-integration
status: verified
threats_open: 0
asvs_level: 2
created: 2026-06-09
---

# Phase 5 — Security

> Per-phase security contract: threat register, accepted risks, and audit trail.
> Plan-time STRIDE register (7 of 8 plans authored `<threat_model>` blocks);
> mitigations verified against the implementation by `gsd-security-auditor`.

---

## Trust Boundaries

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| Rust CLI → Zig C ABI | Untrusted source bytes + stdlib IR JSON cross into `deal_check_with_stdlib` | source/filename/stdlib_ir pointers + lengths |
| Orchestrator → Python entry | `input.json` bytes (untrusted file) deserialized by `cli()` | simulation input values |
| `cli()` → `run()` | Validated inputs passed to author-supplied physics code | typed input dict |
| `deal.sims.toml` → orchestrator | Registry `runner`/`entry`/`model_path` strings are untrusted config | command + path strings |
| Orchestrator → subprocess | Spawns external tool processes (python/matlab/generic) | argv (never a shell line) |
| Working cache → baseline | Evidence promoted from gitignored cache into tracked VCS | output.json bytes + content hashes |
| Evidence files → verify engine | Captured `output.json` values (untrusted on disk) drive criteria | numeric evidence + metadata |
| Gate/fresh worktree → sibling repos | `verify-fresh-worktree.sh` symlinks + `pip install -e` deal-sim | local sibling source |

---

## Threat Register

| Threat ID | Category | Component | Disposition | Mitigation | Status |
|-----------|----------|-----------|-------------|------------|--------|
| T-05-01 | Tampering | `deal_check_with_stdlib` NULL/len ptr args | mitigate | NULL + non-zero len pairs rejected at FFI entry before any deref — `src/lib.zig:502-505` | closed |
| T-05-02 | Denial of Service | malformed stdlib_ir JSON deserialization | mitigate | Parse failure → `catch null`, empty external table, no panic across C ABI — `src/lib.zig:569-626` | closed |
| T-05-03 | Tampering | malformed/wrong-type values in input.json | mitigate | `_validate_inputs` type-checks every field before `run()`; `ValueError` on mismatch — `deal-sim/.../cli.py:156-157`, `validation.py:39-80` | closed |
| T-05-04 | Information Disclosure | non-deterministic JSON key order in hashed evidence | mitigate | `json.dump(..., sort_keys=True)` on output.json + metadata.json — `deal-sim/.../cli.py:189,201` | closed |
| T-05-05 | Tampering | third-party dependency supply chain in Python SDK core | mitigate | `deal-sim` is stdlib-only (`pyproject.toml` `dependencies = []`); **automated grep gate** forbids numpy/scipy/jsonschema imports — `scripts/phase-5-smoke.sh` §0b (T-05-05) | closed |
| T-05-06 | Tampering | path traversal via `model_path` values | mitigate | `validate_model_path` enforces `^[a-zA-Z0-9._]+$` before any FS op — `cli/src/simulate.rs:32-40` (called :350, :914-919) | closed |
| T-05-07 | Tampering | subprocess injection via `runner` field | mitigate | `split_whitespace()` → `Command::new(cmd).args(args)`; no shell interpolation (CWE-78) — `cli/src/simulate.rs:566-575` | closed |
| T-05-08 | Denial of Service | MATLAB absent fails the whole run | mitigate | `ErrorKind::NotFound` → `write_skip_record` → `Ok(Skipped)`; does not error (D-72) — `cli/src/simulate.rs:592-597` | closed |
| T-05-09 | Repudiation | non-deterministic staleness key | mitigate | SHA-256 over BTreeMap-ordered inputs + source bytes + TOML section; no mtime — `cli/src/simulate.rs:219-231` | closed |
| T-05-10 | Tampering | use-after-free of arena diagnostics across FFI | mitigate | Diagnostic bytes cloned to `Vec<u8>` before `deal_free` (Clone-Before-Free) — `cli/src/main.rs:356,419-420` | closed |
| T-05-11 | Information Disclosure | non-reproducible in-process float results | mitigate | `@setFloatMode(.strict)` default → bit-reproducible Zig fast path; tier in metadata (D-73) — `src/sim/deal_sim.zig:88-96` | closed |
| T-05-12 | Repudiation | evidence cache poisoning / undetectable tamper | mitigate | Per-sim SHA-256 `content_hash` in git-tracked manifest — `cli/src/evidence.rs:251-255` | closed |
| T-05-13 | Tampering | working `.deal` artifacts accidentally committed | mitigate | `.gitignore` scopes `.deal/` (line 21); **automated `git check-ignore` assertion** enforces the D-81 split — `cli/tests/evidence_baseline.rs::test_gitignore_enforces_d81_evidence_split` (T-05-13) | closed |
| T-05-14 | Information Disclosure | non-deterministic manifest leaks ordering into hashes | mitigate | BTreeMap alphabetical keys; byte-identical-twice test — `cli/src/evidence.rs:270-296`, `cli/tests/evidence_baseline.rs:136` | closed |
| T-05-15 | Elevation of Privilege | silent sim execution during verify | mitigate | verify never auto-runs; stale → STALE non-zero exit; `--run-sims` explicit opt-in (D-84) — `cli/src/verify.rs:1253-1317` | closed |
| T-05-16 | Tampering | reusing stale evidence as if fresh | mitigate | L3 freshness recomputes SHA-256 staleness key, flags STALE on mismatch — `cli/src/verify.rs:492-550` | closed |
| T-05-17 | Tampering | use-after-free across the dimension-check FFI | mitigate | Diagnostic bytes cloned before `deal_free` in `get_ast_json` — `cli/src/verify.rs:1388-1391` | closed |
| T-05-18 | Denial of Service | gate fails hard when MATLAB absent | mitigate | Same `ErrorKind::NotFound` graceful-skip; gate asserts skip.json — `cli/src/simulate.rs:592-597`, `scripts/phase-5-smoke.sh:144-157` | closed |
| T-05-19 | Tampering | deal_sim not importable in fresh worktree | mitigate | `verify-fresh-worktree.sh` installs sibling editable + hard-asserts `import deal_sim` (exit 1 on failure) — `scripts/verify-fresh-worktree.sh:179-213` | closed |
| T-05-SC | Tampering | supply-chain installs (sha2, walkdir, deal-sim) | accept | sha2/walkdir pre-approved (Plan 01); deal-sim is a local sibling package, not a registry install — see Accepted Risks | closed |

*Status: open · closed*
*Disposition: mitigate (implementation required) · accept (documented risk) · transfer (third-party)*

---

## Accepted Risks Log

| Risk ID | Threat Ref | Rationale | Accepted By | Date |
|---------|------------|-----------|-------------|------|
| AR-05-1 | T-05-SC | `sha2` (RustCrypto/hashes, ~8yr crates.io history) and `walkdir` (BurntSushi, long-stable v2) are vetted third-party crates pre-approved in Phase 5 Plan 01's RESEARCH package-legitimacy audit — `Cargo.toml:54-57` | DavidAD | 2026-06-09 |
| AR-05-2 | T-05-SC | `deal-sim` is installed via `pip install -e ../deal-sim` from a local sibling path, never a PyPI URL. Created-from-scratch; `pyproject.toml` `dependencies = []` forbids transitive registry deps | DavidAD | 2026-06-09 |

*Accepted risks do not resurface in future audit runs.*

---

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-06-09 | 20 | 18 | 2 | gsd-security-auditor (initial verify) |
| 2026-06-09 | 20 | 20 | 0 | orchestrator (after T-05-05 grep gate + T-05-13 check-ignore test landed) |

**Remediation (2026-06-09):** The initial audit found T-05-05 and T-05-13 OPEN —
the security properties held (deal-sim had no third-party deps; `.gitignore`
correctly scoped `.deal/`) but the *claimed automated regression guards* were
absent. Both were implemented and verified:
- **T-05-05** — added the stdlib-only import gate to `scripts/phase-5-smoke.sh` §0b
  (rejects `numpy`/`scipy`/`jsonschema` imports in the deal-sim SDK core).
- **T-05-13** — added `test_gitignore_enforces_d81_evidence_split` to
  `cli/tests/evidence_baseline.rs` (asserts `.deal/evidence/` ignored,
  `evidence/baselines/` tracked via `git check-ignore`). Test passes.

---

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-06-09
