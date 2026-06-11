# Phase 4: Ecosystem - Context

**Gathered:** 2026-06-05
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 4 delivers the **first external release candidate** — the "minimum viable language" gate. A new project can scaffold with `deal init`, depend on `deal-std` for units/interfaces, author definitions and compositions, validate with `deal check`, export to SysML v2 JSON **or ReqIF XML**, and read documentation on **deal-lang.org**. Four deliverables across three repos:

1. **`deal-stdlib`** (sibling repo) — first standard library packages authored in DEAL itself: `units/` (SI base + derived + imperial), `interfaces/electrical/` (RJ45, USB-C, CAN, RS-422), `interfaces/mechanical/` (bolt patterns). Includes the dimensional-algebra metadata the new sema checks consume (D-55..D-57).
2. **ReqIF codegen** (this repo) — second codegen backend: IR → ReqIF XML packaged as `.reqifz`, validating against the OMG ReqIF 1.2 XSD (to be acquired into `spec/references/omg-reqif/`), with a DOORS-or-fallback import smoke (D-58).
3. **`deal-lang.org` docs site** (`deal-lang.org` sibling repo) — Astro + Starlight with landing, getting-started, ~10–14 topic-page language reference, CLI reference, VS Code setup. Shiki grammar derived from Phase 3 TextMate grammars; CI fails on unhighlighted or unparseable `.deal`/`.dealx` snippets (D-62, D-65).
4. **Package resolution** (this repo) — `deal init` (working starter model), `deal install` (local-path + git resolution vendored into `.deal/deps/`), `deal.lock` (SHA-pinned, exact refs only).

The phase also requires `phase-4-gate` + `phase-4-gate-fresh` build steps per the binding fresh-worktree ADR, and inherits `phase-2-gate`/`phase-3-gate` as regression checks.

The phase does NOT ship: centralized package registry (out of scope v1); semver range resolution (D-68 rejects until a registry exists); RF/protocols/standards stdlib packages (Phase 6 stdlib expansion); `deal import` reverse direction (Phase 6); simulation integration (Phase 5); WASM packaging.

</domain>

<decisions>
## Implementation Decisions

> Decisions are numbered D-54 onward to continue the project-wide D-01..D-53 sequence from Phases 1, 1.5, 2, and 3.

### Stdlib scope & unit semantics (REQ-phase-4-1)

- **D-54:** **Stdlib ships REQ scope only.** `units/` (SI + imperial), `interfaces/electrical/` (RJ45, USB-C, CAN, RS-422), `interfaces/mechanical/` (bolt patterns). The broader tree sketched in `deal-stdlib/README.md` (RF connectors, protocols like MIL-STD-1553/ARINC 429, standards like DO-178C/DO-254) is explicitly deferred to Phase 6's "stdlib expansion". The README should be updated to mark non-Phase-4 packages as planned.
- **D-55:** **Full dimensional algebra in `deal check`.** Not just dimension-compatibility checking — derived-unit arithmetic too: `km(200) / hr(1)` evaluates to a Speed (L¹T⁻¹) dimension; `mass : Mass = V(800)` is a dimension error. This intersects the deferred-ADR item "expression syntax details (arithmetic)": the grammar already locks unit-expression syntax (SD-13 shows `km(200) / hr(1)`), so Phase 4 implements evaluation semantics for **unit expressions only** — general arithmetic expression semantics beyond what units require stays deferred.
- **D-56:** **Dimensional knowledge is data-driven from the stdlib, not hardcoded.** `deal-stdlib` declares dimensions as DEAL definitions carrying exponent metadata (exact declaration syntax is researcher/planner discretion — likely annotations on `attribute def`-style unit/dimension definitions). Zig sema implements a **generic 7-exponent SI base-vector algebra** (M, L, T, I, Θ, N, J) that reads those declarations. User-defined units and dimensions work automatically with zero compiler changes — honors FA-2 (libraries are first-class). The Zig core gains no stdlib-specific knowledge.
- **D-57:** **Explicit conversions only at check time.** Mixed-unit same-dimension expressions (e.g., a threshold in `lb` compared against an attribute in `kg`) are a **check error** unless the author writes an explicit conversion call. Nothing implicit — maximally auditable for defense work. Consequences: (a) `deal-stdlib` must ship conversion call forms (exact syntax is researcher/planner discretion, e.g. `to_kg(lb(3300))` or equivalent); (b) a new E-code is allocated for the mixed-unit-comparison error; (c) the rejected "normalize to SI" approach is recorded as a deferred idea for a future ergonomics pass.

### ReqIF codegen (REQ-phase-4-2)

- **D-58:** **DOORS import validation follows the D-36 viewer-smoke pattern.** Priority-ordered attempts: (1) IBM DOORS / DOORS Next if a trial path exists, (2) open-source ReqIF tooling — Eclipse RMF/ProR, the `reqif` Python library round-trip, (3) Jama / Polarion trial import. First successful import wins and is recorded in `04-VERIFICATION.md` (tool name + timestamp); all attempts logged; documented blocker acceptable only if every listed tool fails with recorded reasons. **XSD validation is the hard gate; tool import is the smoke.**
- **D-59:** **Mapping scope: requirements + traces + verification attributes.** `requirement def` / `need def` → ReqIF SpecObjects with typed attributes (requirement text from JSDoc doc comments; `verification {}` block fields — threshold, operator, accepts/rejects methods — as ReqIF attribute values). `@trace:<<satisfies>>` / `<<derives from>>` links and `[<satisfy>]`/`[<allocate>]` traceability → SpecRelations. Package hierarchy → Specification document tree. `part def`/`port def` structure is NOT exported to ReqIF (requirements tools render it poorly). The emitter is Rust walking IR JSON, mirroring the SysML v2 emitter (D-19 pattern); ReqIF identifiers derive from D-23 fully-qualified path-string IDs (exact derivation is researcher's call).
- **D-60:** **ReqIF 1.2 XSD acquisition is the first plan task.** Researcher (during plan-phase research) or Plan 04-01 downloads the OMG ReqIF 1.2 XSD plus a DOORS-compatible sample export, commits them to `spec/references/omg-reqif/` with a SHA256SUMS manifest, matching the existing `omg-sysml-v2` convention. No emitter work starts before the validation target exists.
- **D-61:** **Output format is `.reqifz` archive.** `deal build --target reqif` emits the zipped `.reqifz` container (ReqIF XML + tool metadata inside) — the interchange format DOORS users actually exchange. XSD validation runs against the XML inside the archive. Whether a plain `.reqif` XML file is also emittable (e.g., via a flag) is planner's discretion. Golden-fixture pattern mirrors Phase 2: hand-written expected XML + XSD validation gates.

### Docs site (REQ-phase-4-3)

- **D-62:** **Language reference is topic pages, showcase-driven.** ~10–14 pages: definitions, compositions, requirements + verification, traceability, imports/packages, annotations, units, `deal.toml`, CLI reference, VS Code setup, getting-started, landing. **Every code example is lifted from (or derived from) the 19-file showcase** so snippets are real, parseable, CI-verifiable. Per-keyword exhaustive pages (TypeScript-handbook style) are deferred.
- **D-63:** **Deploy on GitHub Pages** from the `deal-lang.org` sibling repo via GitHub Actions — consistent with the GitHub-org-centric release pipeline Phase 3 established. Astro static output with the GH Pages adapter/base config.
- **D-64:** **The domain is `deal-lang.org`, not the prior `.dev` candidate.** User owns `deal-lang.org`; the `.dev` domain in NC-1 was aspirational and is not owned. This amends **NC-1 [LOCKED]** — Phase 4 includes a small ADR (`.planning/decisions/ADR-phase-4-nc1-domain-amendment.md`) recording the change. The sibling repo's directory name is now `deal-lang.org/`; DNS (CNAME to GitHub Pages) is a user-side manual step the plan must surface as a checklist item.
- **D-65:** **CI parses every docs snippet with the real CLI.** Beyond the locked Shiki gate (build fails on placeholders/missing scopes), docs CI extracts every `.deal`/`.dealx` fenced block and runs `deal parse` on it; intentional-error examples carry an explicit marker (mechanism is planner's discretion, e.g. a fence meta-attribute). Docs can never drift from the grammar — this is the docs-site equivalent of the showcase oracle.

### Package resolution & `deal init` (REQ-phase-4-4)

- **D-66:** **Dependencies vendor per-project into `.deal/deps/<name>/`.** Git deps are cloned there at the locked revision; local-path deps are referenced in place (no copy). No shared user-level cache, no `deal_modules/`. Self-contained and airgap-friendly (a defense program can choose to commit the vendored tree by policy); consistent with PS-10's `.deal/` gitignore contract.
- **D-67:** **`deal-stdlib` is an explicit, git-resolved dependency.** `deal init` writes `[dependencies] deal-std = { git = "https://github.com/deal-lang/deal-stdlib", tag = "v0.x" }` into the scaffolded `deal.toml`. The stdlib is a normal package: dogfoods the resolver end-to-end, keeps the compiler stdlib-agnostic, pins in `deal.lock` like everything else. No bundled/implicit resolution path.
- **D-68:** **Exact refs only; SHA-pinned lockfile.** Git dependencies pin a tag/rev/branch; `deal install` resolves and `deal.lock` records the resolved commit SHA (and local-path identity for path deps). No semver ranges — they're meaningless without a registry index, and determinism is a feature for the defense audience. `deal.lock` format details (TOML vs JSON, field shape) are researcher/planner discretion; reproducibility is the acceptance bar.
- **D-69:** **`deal init` scaffolds a working starter model.** Full PS-8 layout (`deal.toml`, `.deal/` gitignored, `model/`, `packages/`, `simulations/`, `test/data/`, `docs/`) plus a minimal-but-real example: one `part def` + `port def` in `packages/`, one composition in `model/`, one requirement with a satisfy block, `deal-std` dependency wired. **`deal check` passes immediately after `deal init` + `deal install`** — the new-user moment mirrors `cargo new`. No interactive prompts in Phase 4.

### Claude's Discretion

The following are intentionally NOT locked — researcher and planner have latitude:

- **Dimension/exponent declaration syntax in DEAL source** (D-56) — how `deal-stdlib` declares "kg is M¹" and "Speed is L¹T⁻¹". Must use existing locked grammar constructs (annotations per SD-15/SD-16 are the likely vehicle); researcher drafts against the grammar and showcase.
- **Explicit conversion call syntax** (D-57) — function-form per PS-6/SD-13 spirit; exact naming convention (e.g., `to_kg(...)`, `kg.from(lb(3300))`) is researcher's call.
- **New E-code band for dimensional errors** — extend D-16/D-33 allocation (e.g., `E25xx` for unit/dimension checks); exact band and codes are planner's call, documented in `diagnostics.zig` alongside the existing bands.
- **ReqIF identifier derivation from D-23 path IDs** and the exact SpecObject attribute schema (D-59).
- **Whether plain `.reqif` XML is emittable alongside `.reqifz`** (D-61).
- **`deal.lock` file format** (D-68) — TOML vs JSON, field shape; follow cargo/npm precedents for git-only dependency models.
- **`deal install` invocation model** — explicit-only vs auto-resolve-on-build when `.deal/deps/` is missing; lean explicit with a clear "run deal install" diagnostic.
- **Starlight configuration details** — theme, search (Pagefind is Starlight default), sidebar structure, dark/light. No docs versioning UI in Phase 4 (single current version).
- **Plan slicing** — likely waves: (1) ReqIF XSD acquisition + package resolution core (`deal init`/`deal install`/`deal.lock`), (2) stdlib authoring + dimensional algebra sema, (3) ReqIF emitter + golden fixtures + import smoke, (4) docs site + Shiki + CI gates, (5) phase-4-gate + gate-fresh + closeout. Final slicing is the planner's call; note stdlib sema (Zig) and ReqIF emitter (Rust) are parallelizable.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project authority
- `.planning/PROJECT.md` — Tech stack constraints (Astro + Starlight + Shiki for docs; Rust owns ReqIF codegen; Zig owns sema), 64 LOCKED + 1 CAPTURED decisions in `<decisions>` block. Note NC-1 domain amendment per D-64. Out-of-scope list (no registry, no WASM).
- `.planning/REQUIREMENTS.md` §Phase 4 — REQ-phase-4-ecosystem, REQ-phase-4-1-stdlib, REQ-phase-4-2-reqif-codegen, REQ-phase-4-3-docs-site, REQ-phase-4-4-package-resolution, REQ-phase-4-gate. The REQ-4-2 prerequisite note (ReqIF XSD acquisition) is implemented by D-60.
- `.planning/ROADMAP.md` §"Phase 4: Ecosystem" — Goal statement and 5 success criteria for the `deal-lang.org` docs site.
- `.planning/STATE.md` — Phase 3 complete 2026-05-27; 24/24 plans.

### Inherited locked decisions (Phases 0–3)
- `spec/grammar/DESIGN-DECISIONS.md` — Consolidated ADR. Phase 4 leans on: **PS-1** (`deal.toml` manifest with `[dependencies]`), **PS-4** (five import forms — resolver implements external-form resolution), **PS-5** (workspace aliases), **PS-6/SD-13** (unit literals as function calls), **PS-8** (directory layout `deal init` scaffolds), **PS-10** (`.deal/` gitignored — D-66 vendors deps there), **SD-15/SD-16** (annotation syntax — likely vehicle for D-56 exponent metadata), **SD-20/CS-10** (verification contracts — D-59 maps them to ReqIF attributes), **FA-2** (libraries first-class — D-56 rationale), **NC-1** (amended by D-64: website is deal-lang.org). ⚠ Carried-over closeout task: promote this file out of `tmp-references/` during Phase 4 docs work (flagged since Phase 2).
- `.planning/phases/02-prove-the-pipeline/02-CONTEXT.md` — D-19 (Rust owns codegen + offline validation — ReqIF emitter follows), D-22 (`deal_ir_json` transport), D-23 (path-string IDs → ReqIF identifiers), D-24 (workspace-wide IR graph; D-61 chose `.reqifz` over strict D-24 file-shape symmetry), D-26 (IR traversal API), D-32/D-33/D-34 (diagnostic envelope, E-code bands, exit codes).
- `.planning/phases/03-editor-intelligence/03-CONTEXT.md` — D-41 (TextMate scopes designed for Phase 4's Shiki adapter — the docs site consumes `vscode-deal`'s grammars), D-50..D-53 (GitHub Releases pipeline the docs site links to for install instructions).
- `.planning/decisions/ADR-deal-ir-v0.md` + `spec/ir/v0/schema.json` + `spec/ir/v0/README.md` — Normative IR v0. The ReqIF emitter walks this shape.
- `.planning/decisions/ADR-phase-1.5-fresh-worktree-verification.md` — **Binding ADR.** Phase 4 MUST add `phase-4-gate-fresh`; `zig build phase-4-gate-fresh` must exit 0 in an ephemeral worktree. Note Phase 3's lessons: sibling-repo symlinking and `protocol.file.allow` handling in `scripts/verify-fresh-worktree.sh` (commits 3c60cf6, 03696f1) — Phase 4's gate touches even more siblings (`deal-stdlib`, `deal-lang.org`).

### Grammar specifications (LOCKED at `0.1.0-draft`)
- `spec/grammar/lexical.ebnf`, `spec/grammar/deal.ebnf`, `spec/grammar/dealx.ebnf` — Stdlib `.deal` sources, the `deal init` starter model, and every docs snippet must parse against these. Unit-expression syntax for D-55 is already locked here (SD-13 forms).
- `spec/grammar/README.md` — ⚠ Carried-over closeout task: fix the 370→758 line-count drift during Phase 4 docs work (flagged since Phase 2).

### Codegen target schemas
- `spec/references/omg-reqif/` — **DOES NOT EXIST YET.** Created by D-60 (ReqIF 1.2 XSD + DOORS-compatible sample + SHA256SUMS). The emitter's validation target.
- `spec/references/omg-sysml-v2/SysML.json` + `spec/references/omg-kerml-v1/KerML.json` — Existing precedent for offline schema-registry validation (`cli/src/schema_registry.rs`).

### Implementation precedents (this repo)
- `cli/src/main.rs` — `clap` subcommand surface; Phase 4 adds `init`, `install`, and extends `Build` with `BuildTarget::Reqif`.
- `cli/src/sysml_v2.rs` — The existing Rust emitter; closest precedent for the ReqIF emitter (IR JSON walk → output).
- `cli/src/schema_registry.rs` — Offline schema validation precedent; ReqIF XSD validation needs an XML-schema (XSD) validator, not JSON Schema — researcher picks the Rust crate (e.g., `xmlschema`-class options) with offline-only constraint.
- `Cargo.toml` (workspace) — members: cli, deal-ffi, lsp, lsp-smoke, tests/ffi. New crates (if any) join here.
- `src/sema.zig` + `src/diagnostics.zig` — Dimensional-algebra checks (D-55/D-56) extend sema; new E-code band extends the D-16/D-33 documentation comment.
- `build.zig` — `phase-2-gate`/`phase-3-gate` precedents for `phase-4-gate` + `phase-4-gate-fresh`.
- `scripts/verify-fresh-worktree.sh` — Reuse for `phase-4-gate-fresh` (already handles sibling symlinks + submodule `protocol.file.allow`).

### Sibling repos (Phase 4 implementation targets)
- `../deal-stdlib/README.md` — Package tree sketch (broader than D-54 scope; update README to mark deferred packages). Phase 4 turns `units/`, `interfaces/electrical/`, `interfaces/mechanical/` into real DEAL sources.
- `../deal-lang.org/README.md` — Docs site repo (currently README-only). Phase 4 stands up Astro + Starlight here; deploys to GitHub Pages as `deal-lang.org` (D-63/D-64).
- `../vscode-deal/syntaxes/` — TextMate grammars (`deal.tmLanguage.json`, `dealx.tmLanguage.json`) the Shiki adapter consumes (D-41 parity contract).

### Integration oracle
- `spec/examples/showcase/` (mounted at `tests/showcase/`) — 19-file showcase. ReqIF export gate runs on showcase requirements; docs snippets lift from it; the `deal init` starter model should echo its style. `deal check` on showcase must stay green after dimensional-algebra sema lands (showcase uses units — if any showcase file violates D-57's explicit-conversion rule, that's a showcase fix + spec discussion, not a silent waiver).

### External references — researcher to fetch
- **OMG ReqIF 1.2 specification + XSD** — formal spec (OMG document formal/16-07-01 or current); XSD into `spec/references/omg-reqif/` per D-60.
- **ReqIF tool landscape** — Eclipse RMF/ProR status, `reqif` Python library (strictdoc ecosystem), DOORS Next trial availability — for the D-58 priority list.
- **Astro + Starlight current docs** — config, GH Pages deploy, custom domain setup, Shiki custom-grammar loading (TextMate JSON → Shiki `LanguageRegistration`).
- **Rust XSD validation crates** — offline XML Schema validation options for the `.reqifz` gate.
- **Cargo git-dependency + lockfile model** — precedent for D-66..D-68 (resolution, SHA pinning, vendoring).
- **BIPM SI brochure / NIST SP 811** — authoritative source for SI base/derived unit definitions and conversion factors for `deal-stdlib/units` (REQ-4-1 cites BIPM).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`cli/src/sysml_v2.rs` emitter + `schema_registry.rs` validator** — The ReqIF backend is the second instance of the same shape: walk IR JSON, emit, validate offline. Plumbing (`BuildTarget` enum, `--validate`, output-path inference) already exists.
- **9-export C ABI surface** — Phase 4 likely needs no new C ABI exports if dimensional checks live inside the existing `deal_parse`/sema flow (diagnostics flow through `deal_diagnostics_json`). If sema needs workspace-level dependency info (imports resolved across `.deal/deps/`), the resolver feeds file sets to existing entry points from the Rust side — keep the Zig core single-file/workspace-agnostic as established.
- **`deal.toml` parsing** — Phase 2 CLI already parses `[workspace]`, `[workspace.aliases]`, `[build.targets]`; Phase 4 extends the same parser with `[dependencies]`.
- **Phase 3 release pipeline (`release.yml`)** — Cross-compile + artifact + SHA256SUMS patterns reusable for any Phase 4 release-candidate packaging; docs-site CI is a separate, simpler workflow in the `deal-lang.org` repo.
- **`tests/showcase/` symlink + golden-fixture pattern** — `tests/golden/reqif/` mirrors `tests/golden/sysml-v2/`.

### Established Patterns
- **Zig owns sema; Rust owns codegen/CLI** — dimensional algebra (D-55/D-56) goes in `src/sema.zig`; ReqIF emitter + resolver + lockfile in Rust `cli/`.
- **E-code bands documented in `diagnostics.zig`** (D-16/D-33) — new dimensional/unit band allocated alongside.
- **D-18 alphabetical-key JSON / deterministic output** — `deal.lock` and any new JSON artifacts follow the determinism discipline (byte-stable regeneration).
- **Phase gates: `phase-N-gate` + `phase-N-gate-fresh`** — Phase 4's gate spans sibling repos (stdlib check, docs build, init/install E2E), like Phase 3's gate spanned tree-sitter-deal and vscode-deal.
- **Recorded-review checkpoints** (Phase 2's IR-lock) — a mid-phase checkpoint after the dimensional-algebra design lands (stdlib metadata syntax + sema vector algebra) would de-risk the most novel work; planner's call.

### Integration Points
- **Resolver → sema**: `deal install` materializes `.deal/deps/`; import resolution (sema check #6) must resolve external imports (`import deal.std.units.{kg}`) through `deal.toml [dependencies]` → vendored sources. This is the main new cross-boundary wiring.
- **Stdlib → sema**: dimension/exponent metadata declared in `deal-stdlib` DEAL sources is read by the 7-exponent algebra during `deal check` (D-56).
- **TextMate grammars → Shiki → docs CI**: `vscode-deal/syntaxes/*.tmLanguage.json` load as Shiki custom languages; CI gate fails on missing scopes (REQ) and unparseable snippets (D-65, via `deal parse`).
- **IR → ReqIF emitter**: same `deal_ir_json` transport as SysML v2; requirements/traces/verification nodes are the consumed subset (D-59).
- **LSP (Phase 3) benefits automatically**: once the resolver lands, stdlib symbols flow into LSP completion (anticipated in 03-CONTEXT deferred ideas) — no Phase 4 LSP work required, but worth a smoke check in the gate.

</code_context>

<specifics>
## Specific Ideas

- **Domain correction**: the site is **deal-lang.org** (user owns it; `.dev` is not owned). NC-1 amendment ADR is a Phase 4 closeout task; DNS CNAME setup is a user-side manual checklist item.
- **`.reqifz` because that's what DOORS users exchange** — the user picked the archive container over plain XML; validation still targets the inner XML against the OMG XSD.
- **Explicit-conversions-only is a deliberate defense-audience posture** — auditability over ergonomics. The "normalize to SI" alternative was considered and rejected for Phase 4; record it as a possible future ergonomics ADR.
- **`deal init` → `deal install` → `deal check` green** is the canonical new-user smoke and should be a literal `phase-4-gate` step (scaffold into a temp dir, install, check, build both targets).
- **Showcase remains the oracle**: docs snippets lift from it; ReqIF export runs on its requirements; the starter model echoes its style.
- **Carried-over Phase 4 closeout tasks from Phase 2 deferred list**: (a) promote `DESIGN-DECISIONS.md` out of `spec/grammar/tmp-references/`, (b) fix `spec/grammar/README.md` 370→758 line-count drift. Both were earmarked "during Phase 4 docs work".
- **deal-stdlib versioning**: first tag (e.g., `deal-stdlib v0.4.0` or `v0.1.0`) needed so D-67's scaffolded dependency has a real tag to pin; keep version scheme decision with the planner, but lockstep-with-toolchain (like D-52's vscode-deal↔deal-lsp pairing) is the established precedent.

</specifics>

<deferred>
## Deferred Ideas

- **Stdlib expansion packages** — RF connectors (SMA, N-type), protocols (MIL-STD-1553, ARINC 429, SpaceWire, HTTP, MQTT), standards (DO-178C, DO-254, MIL-STD-810H) → Phase 6 "stdlib expansion" per ROADMAP.
- **Implicit SI normalization at check time** — rejected D-57 alternative; revisit as an ergonomics ADR if explicit conversions prove too noisy in real models.
- **Semver range resolution + centralized registry** — out of scope v1 (PROJECT.md); D-68 exact-refs model is the bridge.
- **Per-package ReqIF slices for supplier exchange** — rejected D-61 alternative; future need if supplier workflows demand partial exports.
- **Semantic snippet validation (`deal check` with stdlib-resolving docs workspace)** — rejected D-65 alternative (parse-only chosen); revisit if docs examples start carrying import-heavy content.
- **Docs versioning UI** — single current version in Phase 4; versioned docs become relevant at/after 1.0.
- **Interactive `deal init` prompts** — rejected D-69 alternative; possible Phase 6 polish.
- **`deal index --watch` / `deal init --watch`** — still deferred (carried from Phase 2).
- **General arithmetic expression semantics beyond unit expressions** — remains deferred per ADR (D-55 implements only what units require).

</deferred>

---

*Phase: 4-ecosystem*
*Context gathered: 2026-06-05*
*Sources: PROJECT.md, REQUIREMENTS.md §Phase 4, ROADMAP.md, STATE.md, 02-CONTEXT.md (D-19..D-37), 03-CONTEXT.md (D-38..D-53), ADR-phase-1.5-fresh-worktree-verification.md, ADR-deal-ir-v0.md, deal-stdlib/README.md, deal-lang.org/README.md, cli/src/{main,sysml_v2,schema_registry}.rs, Cargo.toml workspace*
