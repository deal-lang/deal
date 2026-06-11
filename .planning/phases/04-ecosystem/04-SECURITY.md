---
phase: 4
slug: ecosystem
status: verified
threats_open: 0
asvs_level: 1
created: 2026-06-06
---

# Phase 4 — Security

> Per-phase security contract: threat register, accepted risks, and audit trail.

---

## Trust Boundaries

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| OMG network → local repo | Downloaded XSD crosses from external host into trusted schema bundle | XSD schema files (public spec) |
| deal.toml → resolver | Untrusted manifest input controls clone destinations and filesystem reads | git URLs, path strings |
| remote git host → .deal/deps | Cloned repository contents enter the project tree | DEAL sources |
| stdlib DEAL source → Zig sema | Exponent metadata declared in stdlib is trusted by dimensional algebra | unit definitions (public BIPM/NIST data) |
| stdlib DEAL source → consumer projects | Interface definitions become a public dependency surface | connector pinouts, bolt patterns (Unclassified) |
| IR JSON → ReqIF emitter | IR data transformed into XML interchange artifact for external tools | requirement text, trace relations |
| XSD bundle on disk → validator | Bundled XSD read and trusted as validation oracle | XSD schema files |
| npm registry → docs build | Third-party Astro/Starlight packages enter docs toolchain | npm packages |
| GitHub Actions → GitHub Pages | Deploy pipeline publishes static site to public domain | docs site, deploy token |
| MDX snippet content → deal parse | Docs code blocks executed through deal binary in CI | DEAL snippet text |
| GitHub Releases binary → CI | deal binary downloaded into CI trusted to run gates | release binary |
| ephemeral worktree → sibling repos | Fresh gate symlinks sibling repos it reads but must not mutate | repo contents (read-only) |

---

## Threat Register

| Threat ID | Category | Component | Disposition | Mitigation | Status |
|-----------|----------|-----------|-------------|------------|--------|
| T-4-SC-01 | Tampering | XSD download (reqif.xsd/driver.xsd) | mitigate | Human checkpoint confirmed authenticity; `spec/references/omg-reqif/SHA256SUMS` pins bytes (`shasum -c` OK); offline bundling — no runtime fetch | closed |
| T-4-01 | Tampering | omg-reqif bundle integrity over time | mitigate | `reqif_schema.rs:90-91` `verify_sha256` for both XSDs in `load_reqif_schemas`; test `sha256_pins_match` | closed |
| T-4-02 | Tampering | path dependency in deal.toml | mitigate | `resolver.rs:155-187` `validate_dep_path_lexical` rejects absolute paths; `resolver.rs:355-360` mandatory `canonicalize()` fails closed (CR-01 fix); `resolver.rs:201-215` boundary-anchored `assert_not_system_path` (WR-09 fix); `test_path_traversal_rejection` | closed |
| T-4-03 | Spoofing/Tampering | malicious git URL scheme | mitigate | `resolver.rs:127-136` `validate_git_url_scheme` allows only https://, ssh://, git@, file://; called before any clone; `test_bad_git_scheme_rejected` | closed |
| T-4-04 | Tampering | non-deterministic deal.lock | mitigate | `resolver.rs:30` BTreeMap + `resolver.rs:378-379` sort-by-name; `test_lockfile_determinism` asserts byte-identical lock | closed |
| T-4-05 | Info disclosure | vendored repo content trusted by check | accept | `run_check` (main.rs) calls only parse FFI — no exec/spawn into vendored content (D-66) | closed |
| T-4-06 | Tampering | malformed stdlib exponent metadata | accept | Stdlib authored in-repo, reviewed, parse-gated | closed |
| T-4-07 | Info disclosure | unit definitions | accept | Public BIPM/NIST reference data — no sensitive content | closed |
| T-4-08 | DoS | pathological unit expression depth | mitigate | `ast.zig:50` `MAX_TAG_DEPTH = 1024` enforced at `parser_deal.zig:338`; bounded Check #7 traversal (`sema.zig:65-67`); WR-04 visited-set cycle detection bounded by symbol-table size | closed |
| T-4-09 | Tampering | vendored stdlib swapped | accept | deal.lock SHA pins exact stdlib commit; check only parses | closed |
| T-4-10 | Tampering | malformed interface declaration | accept | In-repo, parse-gated; interface data carries no secrets | closed |
| T-4-11 | Info disclosure | interface definitions | accept | Public reference data; all markings Unclassified | closed |
| T-4-12 | Tampering | XSD bundle swap before validation | mitigate | `reqif.rs:605-609` `emit_from_bytes` calls `locate_reqif_schemas_dir` + `load_reqif_schemas` before `validate_reqif_xml` — fail-closed SHA256 verification in the production path (fixed during this audit; was IN-02). Tamper smoke: modified bundle → exit 1 with T-4-12 diagnostic | closed |
| T-4-13 | Tampering/Injection | XML metacharacters in requirement text | mitigate | `quick_xml::Writer` escapes attribute/text content unconditionally (`reqif.rs:252`, `push_attribute` at `reqif.rs:752`). Advisory: planned `<>&` fixture absent — closure rests on quick-xml library contract | closed |
| T-4-14 | Info disclosure | over-export of part/port into ReqIF | mitigate | `reqif.rs:164` filters to requirement_def/need_def; `reqif.rs:210` relation endpoint filter (CR-02 fix); tests `emit_skips_part_def`, `emit_skips_relation_with_non_requirement_endpoint` | closed |
| T-4-SC-06 | Tampering | `pip install reqif` soft smoke supply chain | mitigate | Non-blocking soft smoke only; hard gate is offline Rust validator `validate_reqif_xml` on every build | closed |
| T-4-15 | Tampering | npm supply chain (astro/starlight) | mitigate | `deal-lang.org/package-lock.json` committed; `deploy.yml:29` `withastro/action@v6` (npm ci, lockfile-pinned) | closed |
| T-4-16 | Tampering | docs Shiki grammar drift | accept | Grammar copied from vscode-deal at authoring time; Plan 08 snippet parse-gate catches mismatch in CI | closed |
| T-4-17 | Elevation/Tampering | GH Pages deploy token scope | mitigate | `deploy.yml:9-12` least-privilege `permissions: pages: write, id-token: write` only; OIDC; no PAT | closed |
| T-4-18 | Tampering | docs snippet drift | mitigate | `check-snippets.sh:88` runs `deal parse` on every extracted block; invoked in CI (`deploy.yml:51`); non-zero exit on parse failure | closed |
| T-4-19 | Tampering | error-expected marker abuse | mitigate | `extract_deal_blocks.py:70-86` flags `is_error_expected`; marker visible in diff review; skip documented in `check-snippets.sh:12` | closed |
| T-4-20 | Tampering | downloaded deal binary in CI | accept | Own GitHub Releases — same trust domain as repo; SHA256SUMS from Phase 3 pipeline; release-tag pinning is future hardening | closed |
| T-4-SC-08 | Tampering | `deal parse` running arbitrary snippet content | mitigate | Local binary, file reads only, no network; grammar bounded by MAX_TAG_DEPTH=1024 | closed |
| T-4-21 | Tampering | false-GREEN gate (dev-local untracked files) | mitigate | `verify-fresh-worktree.sh` ephemeral worktree under `mktemp -d` with submodule init (ADR-phase-1.5); human confirmed exit 0 | closed |
| T-4-22 | Tampering | fresh gate mutates sibling repo via symlink | mitigate | EXIT trap removes worktree/symlinks (`verify-fresh-worktree.sh:101`); symlinks live in TMPDIR; gate steps read-only against siblings | closed |
| T-4-23 | DoS | smoke `deal install` hangs on network | mitigate | `phase-4-smoke.sh:93` `file://` local-path override for deal-std — offline resolution; file:// is in T-4-03 allowlist | closed |

*Status: open · closed*
*Disposition: mitigate (implementation required) · accept (documented risk) · transfer (third-party)*

---

## Accepted Risks Log

| Risk ID | Threat Ref | Rationale | Accepted By | Date |
|---------|------------|-----------|-------------|------|
| AR-4-01 | T-4-05 | `deal check` only parses vendored content — no code execution; vendoring is the user's policy choice (D-66) | plan author (04-02-PLAN) | 2026-06-06 |
| AR-4-02 | T-4-06, T-4-10 | Stdlib authored in-repo, reviewed, parse-gated; self-consistency check deferred beyond Phase 4 | plan author (04-03/04-05-PLAN) | 2026-06-06 |
| AR-4-03 | T-4-07, T-4-11 | Units/interfaces are public reference data (BIPM/NIST, connector pinouts) — no sensitive content, marked Unclassified | plan author (04-03/04-05-PLAN) | 2026-06-06 |
| AR-4-04 | T-4-09 | deal.lock SHA pins the exact stdlib commit; reproducibility is the control; check is parse-only | plan author (04-04-PLAN) | 2026-06-06 |
| AR-4-05 | T-4-16 | Grammar copied at authoring time; snippet parse-gate catches drift; sync check deferred | plan author (04-07-PLAN) | 2026-06-06 |
| AR-4-06 | T-4-20 | CI binary from own GitHub Releases (same trust domain); SHA256SUMS from Phase 3; tag pinning is future hardening | plan author (04-08-PLAN) | 2026-06-06 |

---

## Advisories (non-blocking)

- **T-4-13**: the plan's stated "fixture with `<>&` in requirement text" does not exist in `tests/golden/reqif/`; escaping closure rests on the quick-xml 0.40 library contract (attribute values escaped unconditionally). An explicit metacharacter fixture would convert library-contract trust into a regression pin.

---

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-06-06 | 26 | 25 | 1 (T-4-12) | gsd-security-auditor |
| 2026-06-06 | 26 | 26 | 0 | gsd-security-auditor + orchestrator (T-4-12 fixed: `load_reqif_schemas` wired into `emit_from_bytes`; tamper smoke verified exit 1) |

---

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-06-06
