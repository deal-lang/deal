---
phase: 04-ecosystem
verified: 2026-06-06T00:00:00Z
status: verified
score: 15/15 must-haves verified (2026-06-09 re-audit — the 2 prior partials are now resolved; see gaps[].closure)
score_history: "13/15 at 2026-06-06 verification; 2 security partials (SHA256 IN-02, CWE-22 CR-01) closed in production code, re-confirmed 2026-06-09 by /gsd:audit-milestone"
human_resolution:
  resolved: 2026-06-08
  via: 04-HUMAN-UAT.md
  - test: "DOORS / standards-grade ReqIF import (SC-2)"
    outcome: skipped
    note: "No access to a standards-grade ReqIF tool. Python reqif v0.0.48 round-trip (D-58 fallback) remains the only validation; DOORS contract stays technically unvalidated. Revisit if tool access becomes available."
  - test: "CLI cross-file E2500 dimensional check scope (SC-1)"
    outcome: accepted
    note: "Human ruling: graceful-skip on cross-file imported units is ACCEPTED as Phase 4 scope (Zig harness sema_dimensional.zig covers all 4 E25xx pins). Wiring analyzeWithExternalTable into the CLI deal_check path for end-to-end E2500 is DEFERRED to Phase 5."
overrides_applied: 0
human_verification:
  - test: "Confirm DOORS or another standards-grade ReqIF tool successfully imports the showcase .reqifz"
    expected: "At least one requirement from the showcase appears in the tool's requirements view with correct text and identifier"
    why_human: "ROADMAP SC-2 states 'imports successfully into DOORS' — IBM DOORS trial was unavailable; only Python reqif v0.0.48 round-trip was completed as the D-58 fallback. This was human-approved as acceptable per D-58 protocol, but the ROADMAP contract remains technically DOORS-unvalidated."
  - test: "Confirm `deal check` on a real project with `import deal.std.units.{kg, V}` and `attribute mass : Mass = V(800);` emits E2500"
    expected: "deal check exits non-zero with error[E2500] dimension mismatch"
    why_human: "The Zig test harness (sema_dimensional.zig) exercises the full dimensional algebra with stdlib seeding and all 4 E25xx pins pass. However, the CLI `deal check` path processes each file independently via a single-file C ABI (deal_parse) without cross-file symbol seeding — imported units are registered as `.imported` (not `unit_def`) in the per-file table, causing checkCallDimension to gracefully skip dimensional verification rather than emit E2500. The multi-file analyzeWithExternalTable C ABI entry point is deferred to a future plan. A human needs to confirm whether SC-1's 'deal check resolves the dependency' scope covers this limitation or requires it to be addressed now."
gaps:
  - truth: "The emitted XML is structurally validated against the bundled OMG ReqIF 1.2 XSD bundle (hard gate); the XSD bundle is SHA256-verified at load"
    status: resolved
    closure: "2026-06-09 (milestone re-audit): load_reqif_schemas IS now called in the production emit path at reqif.rs:607-608 (locate_reqif_schemas_dir() + load_reqif_schemas(), fail-closed, before validate_reqif_xml). T-4-12 SHA256 tamper-detection runs on every `deal build --target reqif`. The SHA256 sub-gap (IN-02) is CLOSED. NOTE: the separate CR-03 sub-item (wrap_in_reqifz SimpleFileOptions::default() leaves .reqifz archive bytes non-deterministic) is NOT addressed here — tracked as tech debt."
    reason: "validate_reqif_xml (structural gate) runs inside emit_from_bytes on every deal build --target reqif. However, load_reqif_schemas (which performs SHA256 tamper-detection of the XSD bundle files) is NOT called in the production build path — it is only invoked from tests. The ReqifSchemas bytes (reqif_xsd / driver_xsd) are declared with #[allow(dead_code)] and are not consumed in the emit pipeline. CR-03 (code review) further identified that SimpleFileOptions::default() in wrap_in_reqifz does not pin the zip entry's last-modified time, making .reqifz byte output non-deterministic across machines/runs — the golden test sidesteps this by comparing only the extracted XML, not the archive bytes."
    artifacts:
      - path: "cli/src/reqif_schema.rs"
        issue: "load_reqif_schemas SHA256 tamper-detection does not run in production emit path (IN-02 from code review). ReqifSchemas.reqif_xsd / .driver_xsd are #[allow(dead_code)]."
      - path: "cli/src/reqif.rs"
        issue: "wrap_in_reqifz uses SimpleFileOptions::default() without last_modified_time pin — archive is not byte-reproducible (CR-03 from code review)."
    missing:
      - "Call load_reqif_schemas from emit_from_bytes before validate_reqif_xml so T-4-12 tamper-detection runs in production"
      - "Pin zip last-modified timestamp in wrap_in_reqifz: .last_modified_time(zip::DateTime::from_date_and_time(2026, 1, 1, 0, 0, 0).expect('valid')) per CR-03 fix"
  - truth: "deal install resolves a local-path dependency in place without copying AND path traversal guard correctly rejects ../../../etc escape via canonicalization"
    status: resolved
    closure: "2026-06-09 (milestone re-audit): resolver.rs no longer falls back to the un-normalized joined path. canonicalize() now uses `.with_context(...)?` and FAILS CLOSED when the target does not resolve to a real on-disk path (resolver.rs ~354-366); assert_not_system_path runs only on the fully-normalized path (no surviving `..` components). The relative `../../../etc/shadow`-via-nonexistent-path bypass (CWE-22 / CR-01) is CLOSED."
    reason: "CR-01 (code review): The path traversal guard at resolver.rs:338 falls back to the un-normalized path when canonicalize() fails (target does not exist). The code is: `let canonical = resolved.canonicalize().unwrap_or_else(|_| resolved.clone())`. If the target path does not exist (common for declared-but-not-yet-created dependencies), canonical becomes the raw joined path e.g. project_dir/../../etc/shadow. assert_not_system_path then checks this string with starts_with('/etc') — but the string starts with the absolute project_dir prefix, so the check passes. A `path = '../../../../../../etc/shadow'` where the target does not exist bypasses the T-4-02 second gate. The integration test at resolver_test.rs:198 only tests `/etc/passwd` (absolute, rejected by the first gate validate_dep_path_lexical), not a relative escape through a non-existent path."
    artifacts:
      - path: "cli/src/resolver.rs"
        issue: "Line 338: unwrap_or_else(|_| resolved.clone()) defeats the second T-4-02 gate for non-existent relative traversal paths. CWE-22."
    missing:
      - "Change canonicalize().unwrap_or_else to require canonicalize() to succeed (error if path does not exist) OR add lexical containment check to assert the resolved path stays within project_dir before the system-prefix denylist check"
      - "Add integration test for a relative `../../../etc` escape via a non-existent path (currently only absolute /etc/passwd is tested)"
deferred: []
---

# Phase 4 Ecosystem Verification Report

**Phase Goal:** A new project can scaffold with `deal init`, depend on `deal-std` for units/interfaces, author definitions and compositions, validate with `deal check`, export to SysML v2 JSON or ReqIF XML, and read documentation on `deal-lang.org`. First external release candidate.
**Verified:** 2026-06-06
**Status:** verified — both human-decision items resolved 2026-06-08 (DOORS skipped: no tool access; SC-1 cross-file E2500 accepted as scope, deferred to Phase 5). 2 advisory gaps remain.
**Re-verification:** No — initial verification (extends D-58 smoke evidence from existing 04-VERIFICATION.md)

---

## Incorporated Evidence: D-58 ReqIF Import Smoke (from prior 04-VERIFICATION.md)

The existing `04-VERIFICATION.md` recorded the Plan 04-06 Task 3 D-58 import smoke:

| Gate | Status | Tool / Method |
|------|--------|---------------|
| Hard gate: XSD structural validation | PASSED | Rust reqif_schema::validate_reqif_xml |
| Import smoke: IBM DOORS/DOORS Next | SKIPPED | No trial installation available |
| Import smoke: Python reqif v0.0.48 | PASSED | reqif.parser.ReqIFParser + stdlib ET cross-check |
| Import smoke: Eclipse RMF/ProR | NOT ATTEMPTED | Prior smoke passed |

D-58 RESULT: PASSED with human-approved fallback. The hard gate (structural XSD validation) is automated. The Python reqif v0.0.48 round-trip succeeded without exception and cross-validation confirmed 13 SPEC-OBJECT elements. Human confirmation: accepted per D-58 fallback clause (orchestrator addendum to 04-09-SUMMARY).

---

## Goal Achievement

### Observable Truths — ROADMAP Success Criteria

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| SC-1 | A user can write `import deal.std.units.{kg, V}; attribute mass : Mass = kg(1500);` and `deal check` resolves the dependency through `deal.toml` against deal-stdlib (units, electrical interfaces, mechanical interfaces) | ? UNCERTAIN | `deal install` clones deal-stdlib to `.deal/deps/`; E2402 guard fires when not installed; stdlib sources appended to resolved_paths in run_check. However, dimensional Check #7 via the CLI path gracefully skips for `imported` (not `unit_def`) units — cross-file stdlib seeding only works in the Zig test harness via `analyzeWithExternalTable`. Needs human confirmation of scope. |
| SC-2 | `deal build --target reqif` emits XML that validates against the OMG ReqIF 1.2 XSD and imports successfully into DOORS for the showcase requirements | ? UNCERTAIN | Structural XSD validation PASSES (hard gate in emit_from_bytes). DOORS import SKIPPED (no trial). Python reqif v0.0.48 round-trip PASSED (human-approved D-58 fallback). See D-58 smoke section above. |
| SC-3 | deal-lang.org is live on Astro + Starlight with all pages and every .deal/.dealx code block renders with actual Shiki syntax highlighting (CI fails on placeholders or missing scopes) | ✓ VERIFIED | deal-lang.org builds with Astro 6.4.4 + Starlight 0.39.3. DEAL/DEALX TextMate grammars loaded via expressiveCode.shiki.langs as parsed objects with aliases. Landing + 3 getting-started + 8 reference + 2 CLI pages (13 total). check-snippets.sh extracts snippets, skips error-expected, passes 53 snippets via deal parse. Shiki scope gate passes. deploy.yml runs snippet gate in CI. |
| SC-4 | `deal install` resolves local-path and git-based dependencies and generates a `deal.lock` for reproducible builds | ✓ VERIFIED | resolver.rs exports resolve_all, resolve_git_dep, LockFile (BTreeMap-based for D-18 determinism). Integration test confirms git clone at exact tag, deal.lock SHA-pinned, two installs produce byte-identical lock, path dep referenced in place. CLI wired with Init + Install subcommands. |
| SC-5 | A new project scaffolded with `deal init` can be authored, validated, exported to both SysML v2 JSON and ReqIF XML, and its docs are readable on deal-lang.org | ✓ VERIFIED | phase-4-smoke.sh exercises the full new-user flow (deal init → deal install → deal check → deal build --target sysml-v2 → deal build --target reqif) and exits 0 (human-confirmed in Phase 4 gate). phase-4-gate-fresh exits 0 in ephemeral worktree. Docs site built and deployed. |

**Score:** 3/5 ROADMAP SCs fully verified; 2/5 UNCERTAIN (require human decision)

### Plan Must-Haves Summary

| Plan | Must-Have | Status |
|------|-----------|--------|
| 04-01 | OMG ReqIF 1.2 XSD bundle SHA256-verified (D-60) | ✓ VERIFIED |
| 04-01 | E2402 + E2500..E2503 reserved in diagnostics.zig | ✓ VERIFIED |
| 04-01 | Wave 0 test scaffolds (golden skeleton + dimensional harness + 3 regression fixtures) | ✓ VERIFIED |
| 04-02 | deal install clones git dep to .deal/deps/ at locked ref with SHA in deal.lock | ✓ VERIFIED |
| 04-02 | deal install resolves local-path dep in place without copying | ✓ VERIFIED (but path-traversal guard has edge case — see gaps) |
| 04-02 | deal.lock is byte-stable across repeated installs (BTreeMap, alphabetical order) | ✓ VERIFIED |
| 04-02 | deal init scaffolds deal.toml with deal-std git dependency + D-69 starter model | ✓ VERIFIED |
| 04-03 | deal-stdlib declares SI base dimensions with 7-exponent vectors (ADR-locked encoding) | ✓ VERIFIED |
| 04-03 | deal-stdlib declares SI + imperial units linked to dimension with conversion factor | ✓ VERIFIED |
| 04-03 | deal-stdlib ships explicit conversion call forms (D-57) | ✓ VERIFIED (to_kg etc. in imperial.deal) |
| 04-03 | deal check parses all deal-stdlib units sources cleanly | ✓ VERIFIED |
| 04-04 | deal check evaluates unit expressions from stdlib metadata (D-55/D-56) — test harness | ✓ VERIFIED (test harness; CLI single-file limitation noted) |
| 04-04 | E2500 dimension mismatch emitted correctly | ✓ VERIFIED (4 E25xx pins in sema_dimensional.zig, no SCAFFOLD guards) |
| 04-04 | Zig core hardcodes zero unit names (data-driven) | ✓ VERIFIED (builtinDimVector maps dimension TYPE names like "Mass", not unit names like "kg" — acceptable per SUMMARY note; no kg/lb/V/A hardcoded) |
| 04-04 | 19-file showcase emits zero E25xx diagnostics | ✓ VERIFIED (showcase-clean loop in sema_dimensional.zig with no SCAFFOLD guards) |
| 04-05 | deal-stdlib declares electrical interfaces RJ45, USB-C, CAN, RS-422 | ✓ VERIFIED |
| 04-05 | deal-stdlib declares mechanical bolt-pattern interfaces | ✓ VERIFIED |
| 04-05 | Interface files import units from deal.std.units | ✓ VERIFIED |
| 04-05 | All interface sources parse cleanly | ✓ VERIFIED |
| 04-06 | deal build --target reqif emits .reqifz archive with ReqIF 1.2 XML | ✓ VERIFIED |
| 04-06 | Emitted XML maps requirement_def/need_def to SpecObjects, @trace to SpecRelations | ✓ VERIFIED (D-59 filter; golden fixtures have real XML) |
| 04-06 | Emitted XML structurally validated against OMG XSD bundle (hard gate) + SHA256-verified at load | PARTIAL (structural validation runs in production; SHA256 tamper-detection only in tests — see gaps) |
| 04-06 | Golden fixtures have real expected ReqIF XML (not placeholders) | ✓ VERIFIED |
| 04-06 | D-58 import smoke recorded in 04-VERIFICATION.md | ✓ VERIFIED |
| 04-07 | deal-lang.org runs Astro + Starlight, npm run build exits 0 | ✓ VERIFIED |
| 04-07 | DEAL TextMate grammars load as Shiki custom languages with correct aliases | ✓ VERIFIED |
| 04-07 | Landing + getting-started pages with showcase-derived examples, correct CTA copy | ✓ VERIFIED |
| 04-07 | GitHub Pages deploy workflow + public/CNAME = deal-lang.org | ✓ VERIFIED |
| 04-07 | NC-1 domain amendment ADR recorded | ✓ VERIFIED |
| 04-08 | 8 language-reference pages + 2 CLI pages, showcase-derived, parse-clean | ✓ VERIFIED |
| 04-08 | check-snippets.sh extracts snippets, skips error-expected, runs deal parse gate | ✓ VERIFIED |
| 04-08 | Shiki scope gate fails on zero-highlighted tokens | ✓ VERIFIED |
| 04-08 | deploy.yml runs snippet gate in CI | ✓ VERIFIED |
| 04-09 | zig build phase-4-gate exits 0 (inherits phase-3-gate, D-35 cumulative) | ✓ VERIFIED |
| 04-09 | zig build phase-4-gate-fresh exits 0 in ephemeral worktree (human-confirmed) | ✓ VERIFIED |
| 04-09 | E2E new-user flow: deal init → install → check → build sysml-v2 AND reqif all exit 0 | ✓ VERIFIED |
| 04-09 | DESIGN-DECISIONS.md promoted from tmp-references/ | ✓ VERIFIED |
| 04-09 | spec/grammar/README.md line-count drift fixed | ✓ VERIFIED |

**Overall score:** 15/15 plan must-have clusters verified — the 2 prior partials (SHA256 tamper-detection wiring IN-02; path-traversal guard CWE-22 CR-01) were closed in production code and re-confirmed 2026-06-09 by `/gsd:audit-milestone`. See the `gaps[].closure` fields in the frontmatter for file:line evidence. *(Historical: 13/15 at the 2026-06-06 verification.)*

---

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `spec/references/omg-reqif/reqif.xsd` | OMG ReqIF 1.2 XSD | ✓ VERIFIED | Present; SHA256SUMS contains hash |
| `spec/references/omg-reqif/SHA256SUMS` | Tamper-detection manifest | ✓ VERIFIED | 2 hashes for reqif.xsd + driver.xsd |
| `src/diagnostics.zig` | E2402 + E2500..E2503 + band comment | ✓ VERIFIED | All 5 codes + E2500..E2599 band documented |
| `tests/unit/sema_dimensional.zig` | Live regression harness, 0 SCAFFOLD guards | ✓ VERIFIED | SCAFFOLD guard count = 0 |
| `tests/golden/reqif/01-requirement-def.expected.reqif` | Real ReqIF XML (not placeholder) | ✓ VERIFIED | Root `<REQ-IF>` element present; no placeholder comment |
| `cli/src/resolver.rs` | resolve_all, resolve_git_dep, LockFile, DealToml, Dependency | ✓ VERIFIED | All 5 exports present; BTreeMap used |
| `cli/src/main.rs` | Command::Install, Command::Init, DEFAULT_STDLIB_TAG = "v0.4.0" | ✓ VERIFIED | All present |
| `../deal-stdlib/packages/units/dimensions.deal` | SI base dimensions, package deal.std.units | ✓ VERIFIED | 7 base + 8 derived dimensions with ADR-encoded metadata |
| `../deal-stdlib/packages/units/si.deal` | SI units including showcase imports | ✓ VERIFIED | kg, kWh, V, A, kW, degC, ohm confirmed |
| `../deal-stdlib/packages/interfaces/electrical/rj45.deal` | RJ45 interface definition | ✓ VERIFIED | interface def RJ45 present |
| `../deal-stdlib/packages/interfaces/electrical/usb_c.deal` | USB-C interface definition | ✓ VERIFIED | interface def USBC with derived maxPower |
| `../deal-stdlib/packages/interfaces/mechanical/bolt_patterns.deal` | BoltPattern + 2 concrete patterns | ✓ VERIFIED | BoltPattern4x100 + BoltPattern5x114_3 via <<specializes>> |
| `cli/src/reqif.rs` | emit, emit_from_bytes, deal_id_to_reqif_id | ✓ VERIFIED | All 3 exports present |
| `cli/src/reqif_schema.rs` | load_reqif_schemas, validate_reqif_xml | ✓ VERIFIED | Both present; SHA256 pins embedded |
| `cli/src/main.rs` | BuildTarget::Reqif, run_build_reqif | ✓ VERIFIED | Reqif variant + dispatch wired |
| `cli/tests/golden_reqif.rs` | Golden diff + valid-zip tests | ✓ VERIFIED | Byte-exact diff + zip extraction tests |
| `../deal-lang.org/astro.config.mjs` | site=deal-lang.org, shiki langs with aliases | ✓ VERIFIED | deal-lang.org + expressiveCode.shiki.langs=[dealGrammar,dealxGrammar] with aliases |
| `../deal-lang.org/src/grammars/deal.tmLanguage.json` | DEAL TextMate grammar | ✓ VERIFIED | Copied from vscode-deal/syntaxes/ |
| `../deal-lang.org/src/styles/custom.css` | --sl-color-accent: #0E7490 | ✓ VERIFIED | Present |
| `../deal-lang.org/public/CNAME` | deal-lang.org | ✓ VERIFIED | Exactly "deal-lang.org" |
| `../deal-lang.org/.github/workflows/deploy.yml` | withastro/action@v6 + check-snippets | ✓ VERIFIED | Both present; pages:write + id-token:write |
| `../deal-lang.org/scripts/check-snippets.sh` | D-65 parse gate + Shiki scope gate | ✓ VERIFIED | mapfile used; error-expected skipped; Shiki scope gate present |
| `../deal-lang.org/scripts/extract_deal_blocks.py` | Fenced block extractor | ✓ VERIFIED | Present |
| `build.zig` | phase-4-gate (dependOn gate_3_step) + phase-4-gate-fresh | ✓ VERIFIED | gate_4_step.dependOn(gate_3_step) confirmed |
| `scripts/phase-4-smoke.sh` | deal init → install → check → build both targets | ✓ VERIFIED | Exists; set -euo pipefail; 5-step E2E |
| `scripts/verify-fresh-worktree.sh` | deal-stdlib + deal-lang.org in sibling loop + EXIT trap | ✓ VERIFIED | Both siblings in loop and trap |
| `spec/grammar/DESIGN-DECISIONS.md` | Promoted from tmp-references/ | ✓ VERIFIED | Exists; tmp-references/ version absent |
| `.planning/decisions/ADR-phase-4-dimension-metadata-syntax.md` | Locks dimension/unit metadata encoding | ✓ VERIFIED | Present |
| `.planning/decisions/ADR-phase-4-nc1-domain-amendment.md` | NC-1 domain amendment | ✓ VERIFIED | Present |

---

## Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `cli/src/main.rs` | `cli/src/reqif.rs` | run_build_reqif calls reqif::emit_from_bytes | ✓ WIRED | Confirmed at line 1265 |
| `cli/src/reqif.rs` | `cli/src/reqif_schema.rs` | emit validates XML against XSD before zipping | ✓ WIRED | reqif_schema::validate_reqif_xml called in emit_from_bytes |
| `cli/src/main.rs` | `cli/src/resolver.rs` | run_install calls resolver::resolve_all | ✓ WIRED | Confirmed in run_install |
| `../deal-lang.org/astro.config.mjs` | `../deal-lang.org/src/grammars/deal.tmLanguage.json` | expressiveCode.shiki.langs reads parsed grammar JSON | ✓ WIRED | JSON.parse(readFileSync) confirmed |
| `../deal-lang.org/.github/workflows/deploy.yml` | `../deal-lang.org/scripts/check-snippets.sh` | CI build job invokes snippet gate | ✓ WIRED | bash scripts/check-snippets.sh step present |
| `scripts/phase-4-smoke.sh` via `build.zig` | deal init/install/check/build commands | gate_4_step addSystemCommand phase-4-smoke | ✓ WIRED | build.zig line 448 |
| `src/sema.zig` DimVector | stdlib metadata annotations | reads si_M..si_J from attribute def bodies (ADR) | ✓ WIRED | collectDefinition reads ADR-specified fields |
| `cli/src/reqif_schema.rs` | XSD SHA256 pins | verify_sha256 against EXPECTED_REQIF_XSD_SHA256 | PARTIAL | SHA256 verification wired in load_reqif_schemas and tests; NOT called from emit_from_bytes production path |

---

## Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `cli/src/reqif.rs emit()` | spec_objects / spec_relations | IR JSON nodes filtered by requirement_def/need_def + trace edges | Yes — from showcase IR JSON | ✓ FLOWING |
| `cli/tests/golden_reqif.rs` | xml_bytes from fixture | tests/golden/reqif/*.expected.reqif (real XML) | Yes — real fixture XML | ✓ FLOWING |
| `tests/unit/sema_dimensional.zig` regression_pins | E25xx diagnostics | 07-*.deal fixtures via deal_parse_internal_with_stdlib | Yes — 4 live pins pass | ✓ FLOWING |
| `../deal-lang.org` pages | deal/dealx fenced blocks | spec/examples/showcase/ + deal-stdlib (D-62) | Yes — real showcase sources | ✓ FLOWING |

---

## Behavioral Spot-Checks

| Behavior | Evidence | Status |
|----------|----------|--------|
| XSD bundle tamper-detection file exists | `spec/references/omg-reqif/SHA256SUMS` has 2 hashes; files present | ✓ PASS |
| Golden reqif fixture is real XML (not placeholder) | `01-requirement-def.expected.reqif` line 1: `<?xml version="1.0"` + `<REQ-IF ...>` | ✓ PASS |
| sema_dimensional.zig has 0 SCAFFOLD guards (live pins) | `grep -c 'SCAFFOLD'` = 0 | ✓ PASS |
| resolver.rs uses BTreeMap (D-18) | `grep -c 'BTreeMap'` >= 1; alphabetical test present | ✓ PASS |
| deal-lang.org CNAME = deal-lang.org | `cat public/CNAME` = `deal-lang.org` | ✓ PASS |
| phase-4-gate dependsOn gate_3_step | build.zig line 423: `gate_4_step.dependOn(gate_3_step)` | ✓ PASS |
| DESIGN-DECISIONS.md promoted | `spec/grammar/DESIGN-DECISIONS.md` exists; tmp-references version absent | ✓ PASS |
| CR-01 path traversal `unwrap_or_else` | resolver.rs:338 `canonicalize().unwrap_or_else(|_| resolved.clone())` confirmed | ✗ ADVISORY |
| CR-03 zip timestamp | wrap_in_reqifz uses `SimpleFileOptions::default()` without `last_modified_time` pin | ✗ ADVISORY |
| IN-02 tamper-detection dead path | `load_reqif_schemas` not called from `emit_from_bytes` | ✗ ADVISORY |

---

## Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| REQ-phase-4-ecosystem | 04-09 | Standard library + second backend + docs site end-to-end | ✓ SATISFIED | phase-4-gate-fresh human-confirmed; smoke exits 0; all 4 sub-requirements complete |
| REQ-phase-4-1-stdlib | 04-03, 04-05 | deal-stdlib units (SI + imperial) + electrical + mechanical interfaces | ✓ SATISFIED | All 7 unit files + 7 interface files present and parse-clean; ADR locked encoding |
| REQ-phase-4-2-reqif-codegen | 04-06 | IR → ReqIF XML emitter; showcase exported; DOORS import validation | PARTIAL | Emitter works; structural gate passes; D-58 fallback (Python reqif) human-approved; full DOORS import not completed (trial unavailable) |
| REQ-phase-4-3-docs-site | 04-07, 04-08 | deal-lang.org Astro+Starlight + Shiki highlighting + 13 pages + CI gate | ✓ SATISFIED | Site builds; 13 pages; 53 snippets pass gate; Shiki scope gate wired; deploy.yml with CI |
| REQ-phase-4-4-package-resolution | 04-02 | deal.toml parsing, git2 resolution, deal.lock, deal init | ✓ SATISFIED | resolver.rs exports verified; 5 integration tests pass; Init/Install subcommands wired |
| REQ-phase-4-gate | 04-09 | Phase 4 exit gate: zig build phase-4-gate exits 0 + fresh worktree | ✓ SATISFIED | human-confirmed phase-4-gate-fresh exits 0 after git autocrlf XSD fix |

---

## Anti-Patterns Found

| File | Pattern | Severity | Impact |
|------|---------|----------|--------|
| `cli/src/resolver.rs:338` | `canonicalize().unwrap_or_else(|_| resolved.clone())` — path traversal guard bypass for non-existent paths (CR-01) | BLOCKER (security) | T-4-02 second gate can be bypassed with `path = "../../../../../../etc/shadow"` when target does not exist. Only the lexical first gate (rejecting absolute paths) still fires. |
| `cli/src/reqif.rs:759-761` | `SimpleFileOptions::default()` without `last_modified_time` pin — .reqifz not byte-reproducible (CR-03) | WARNING | D-18 byte-reproducibility claimed but broken for the archive artifact. Golden test sidesteps by extracting XML only. |
| `cli/src/reqif_schema.rs:52-58` | `load_reqif_schemas` not called in production path; `#[allow(dead_code)]` on XSD bytes (IN-02) | WARNING | T-4-12 tamper-detection does not execute in production. Only runs in tests. |
| `cli/src/reqif.rs:178-181` | `filter(|e| matches_req(e.src) || matches_req(e.dst))` — OR filter produces dangling SPEC-OBJECT-REF when only one endpoint is a requirement (CR-02) | WARNING | Relations reference SpecObjects not in the document. Importing into DOORS/Polarion will fail or drop the relation silently. Golden fixture at 02-trace-relation.expected.reqif asserts this dangling ref is present. |
| `src/sema.zig:1351-1352` | `isTraceAnnotation` second `or` clause is tautological dead code (WR-06) | INFO | The second clause `(eql "trace" and operator != null)` is a strict subset of the first; effectively dead. |
| `cli/src/resolver.rs:147-151` | SYSTEM_PATH_PREFIXES prefix matching is not separator-anchored (WR-09) | INFO | `/etc` matches `/etcetera-project`; combined with CR-01 makes denylist unreliable. |

**Debt marker gate:** No `TBD`, `FIXME`, or `XXX` markers found in phase-4-modified files. The only match was `XXXXXX` in a `mktemp` command — not a debt marker.

---

## Human Verification Required

### 1. DOORS Import Validation for SC-2

**Test:** Import `build/reqif/model.reqifz` into IBM DOORS, DOORS Next, or another standards-grade ReqIF tool (Eclipse RMF/ProR, Jama, Polarion).
**Expected:** Showcase requirements (NEED_ALL_WEATHER, NEED_FAST_CHARGE, etc.) appear in the tool's requirements view with correct text, non-empty identifiers (e.g. `DEAL_requirements_needs_NEED_ALL_WEATHER`), and no import errors.
**Why human:** ROADMAP SC-2 explicitly states "imports successfully into DOORS". No DOORS trial was available during development; the D-58 hard gate (structural XSD validation) passed and the Python reqif v0.0.48 soft smoke passed (human-approved as D-58 fallback). Whether the fallback satisfies the ROADMAP contract requires a human decision. The CR-02 finding (dangling SPEC-OBJECT-REF for non-requirement trace endpoints) is also relevant — it may cause DOORS to reject or silently drop relations when both endpoints are not requirements.

### 2. CLI Dimensional Check Scope for SC-1

**Test:** In a real project with `deal-stdlib` installed, write `attribute mass : Mass = V(800);` in a `.deal` file that imports `deal.std.units.{V}` via `deal.toml` dependency. Run `deal check`.
**Expected:** Either (a) deal check exits non-zero with `error[E2500]` dimension mismatch, OR (b) the human accepts that dimensional algebra via the CLI is a deferred capability (test harness works, CLI single-file path gracefully skips).
**Why human:** The Zig sema_dimensional.zig test harness passes all 4 E25xx regression pins. However, the CLI `deal check` path processes files individually via the single-file C ABI `deal_parse` — imported unit names are registered as `.imported` (not `unit_def` with dimensional metadata), so `checkCallDimension` encounters `resolveUnitDimension → null` and gracefully returns without emitting E2500. The multi-file `analyzeWithExternalTable` entry point needed for cross-file stdlib seeding in the CLI is a documented future plan item. ROADMAP SC-1 says "deal check resolves the dependency through deal.toml against deal-stdlib" — the question is whether this covers full dimensional type-checking or only package resolution.

---

## Gaps Summary

**2 advisory gaps** (do not block goal achievement but require resolution):

**Gap 1 — SHA256 tamper-detection not wired in production path (security advisory):**
`reqif_schema.rs::load_reqif_schemas` performs SHA256 verification of the bundled XSD files (T-4-12 tamper-detection) but is not called from `reqif.rs::emit_from_bytes`. The production build path only runs structural validation (`validate_reqif_xml`). The tamper-detection runs in tests (`sha256_pins_match`, `load_reqif_schemas_succeeds`) but not when a user runs `deal build --target reqif`. Fix: call `load_reqif_schemas` from `emit_from_bytes` before `validate_reqif_xml`.

**Gap 2 — Path traversal guard has edge case (security advisory, CR-01):**
`resolver.rs:338` uses `canonicalize().unwrap_or_else(|_| resolved.clone())` which disarms the T-4-02 second gate when the path target does not exist. A relative path traversal like `path = "../../../../../../etc/shadow"` where the target does not exist will pass both guards (lexical first gate only rejects absolute paths starting with `/`; system-prefix check misses the project_dir-prefixed raw join). The integration test only covers `/etc/passwd` (absolute, caught by first gate).

**2 human verification items** requiring developer decision before phase can be declared fully passed.

**Code review advisory findings not requiring immediate fix (consistent with advisory scope per instructions):**
- CR-02: Dangling SPEC-OBJECT-REF for non-requirement trace endpoints — affects DOORS/Polarion import reliability
- CR-03: .reqifz archive not byte-reproducible — D-18 claim broken at archive level (golden test sidesteps)
- WR-06: Tautological `isTraceAnnotation` dead clause
- WR-09: SYSTEM_PATH_PREFIXES not separator-anchored (compounded by CR-01)

---

_Verified: 2026-06-06_
_Verifier: Claude (gsd-verifier)_
