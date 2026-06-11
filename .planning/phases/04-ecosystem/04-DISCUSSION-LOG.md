# Phase 4: Ecosystem - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-05
**Phase:** 4-ecosystem
**Areas discussed:** Stdlib scope & unit semantics, ReqIF mapping & DOORS validation, Docs site depth & deployment, Package resolution & deal init UX

---

## Stdlib scope & unit semantics

| Option | Description | Selected |
|--------|-------------|----------|
| REQ scope only (Recommended) | units (SI + imperial), interfaces/electrical (RJ45, USB-C, CAN, RS-422), interfaces/mechanical (bolt patterns); RF/protocols/standards defer to Phase 6 | ✓ |
| REQ scope + RF connectors | Add interfaces/rf (SMA, N-type) — structurally identical to electrical | |
| Full README tree | Ship protocols and standards packages too | |

**User's choice:** REQ scope only

| Option | Description | Selected |
|--------|-------------|----------|
| Dimension-aware checking (Recommended) | Dimension compatibility only; no unit arithmetic | |
| Typed literals only | No unit-specific sema; cheapest | |
| Full dimensional algebra | Dimension checking + derived-unit arithmetic (km/hr → Speed) + conversion awareness | ✓ |

**User's choice:** Full dimensional algebra
**Notes:** Goes beyond the recommended option — user wants the full engineering experience at first release.

| Option | Description | Selected |
|--------|-------------|----------|
| Data-driven from stdlib (Recommended) | Stdlib declares dimension exponent metadata in DEAL source; Zig sema implements generic 7-exponent SI vector algebra | ✓ |
| Hardcoded SI core in sema | Built-in SI knowledge; user-defined units need compiler changes | |
| Rust-side checking | Dimensional analysis in Rust walking IR; LSP wouldn't surface live | |

**User's choice:** Data-driven from stdlib

| Option | Description | Selected |
|--------|-------------|----------|
| Normalize to SI at check (Recommended) | Implicit normalization; lb-vs-kg thresholds just work | |
| Same-dimension = compatible, no value math | Defer numeric conversion to Phase 5 | |
| Explicit conversions only | Mixed-unit expressions error unless explicit conversion call written | ✓ |

**User's choice:** Explicit conversions only
**Notes:** Deliberately rejected the recommended implicit option — auditability over ergonomics for the defense audience.

---

## ReqIF mapping & DOORS validation

| Option | Description | Selected |
|--------|-------------|----------|
| Priority list, D-36 style (Recommended) | DOORS trial → Eclipse RMF/ProR / reqif Python → Jama/Polarion; first success recorded; XSD is the hard gate | ✓ |
| I have DOORS access | Real DOORS instance gate | |
| XSD-only gate | Schema validation only — weakest evidence | |

**User's choice:** Priority list, D-36 style

| Option | Description | Selected |
|--------|-------------|----------|
| Requirements + traces + verification attrs (Recommended) | SpecObjects with typed attributes incl. verification block fields; SpecRelations; Specification tree | ✓ |
| Requirements + traces only | Verification stays DEAL-side | |
| Everything including parts | Export part/port structure too — renders poorly in req tools | |

**User's choice:** Requirements + traces + verification attrs

| Option | Description | Selected |
|--------|-------------|----------|
| First plan task, researcher fetches (Recommended) | Download ReqIF 1.2 XSD + DOORS sample into spec/references/omg-reqif/ with SHA256SUMS before emitter work | ✓ |
| I'll acquire it manually first | User downloads before planning | |
| Defer validation, emit first | Start from spec PDF, bolt validation on later | |

**User's choice:** First plan task, researcher fetches

| Option | Description | Selected |
|--------|-------------|----------|
| One .reqif per workspace, mirror D-24 (Recommended) | Single consolidated XML file matching SysML backend shape | |
| .reqifz archive bundle | Zipped container (XML + tool metadata) — what DOORS users exchange | ✓ |
| Per-package .reqif files | Supplier slicing — breaks D-24 symmetry | |

**User's choice:** .reqifz archive bundle
**Notes:** Chose over the recommended plain-XML option because .reqifz is the real-world interchange format.

---

## Docs site depth & deployment

| Option | Description | Selected |
|--------|-------------|----------|
| Topic pages, showcase-driven (Recommended) | ~10-14 topic pages; all examples lifted from the 19-file showcase | ✓ |
| Per-keyword exhaustive | TypeScript-handbook style, 2-3x volume | |
| Minimal launch set | Landing + getting-started + one long tour page | |

**User's choice:** Topic pages, showcase-driven

| Option | Description | Selected |
|--------|-------------|----------|
| GitHub Pages (Recommended) | Free, org-consistent, Actions deploy | ✓ |
| Cloudflare Pages | Faster CDN, per-PR previews | |
| Vercel | Best preview DX, another vendor | |

**User's choice:** GitHub Pages

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, registered | Owns deal-lang.org, can set DNS | |
| Not yet — I'll register it | Register during phase | |
| Use github.io for now | Default Pages URL, domain fast-follow | |
| **Other (free text)** | **"Sorry, I actually have deal-lang.org not deal-lang.org"** | ✓ |

**User's choice:** Free-text correction — the owned domain is **deal-lang.org**, not deal-lang.org.
**Notes:** This amends NC-1 [LOCKED] (website: deal-lang.org). Captured as D-64 with a required one-line ADR amendment. All REQ/ROADMAP references to deal-lang.org read as deal-lang.org.

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — parse all snippets (Recommended) | CI runs `deal parse` on every .deal/.dealx fence; marker for intentional errors | ✓ |
| Highlight gate only | Just the locked Shiki/placeholder gate | |
| Parse + check imports | Semantic validation against docs-local workspace — friction-heavy | |

**User's choice:** Yes — parse all snippets

---

## Package resolution & deal init UX

| Option | Description | Selected |
|--------|-------------|----------|
| Per-project vendor in .deal/ (Recommended) | Git deps clone into .deal/deps/; local paths referenced in place; airgap-friendly | ✓ |
| Shared user cache | ~/.deal/cache/ cargo-style — premature before a registry | |
| node_modules-style deal_modules/ | Visible second generated dir | |

**User's choice:** Per-project vendor in .deal/

| Option | Description | Selected |
|--------|-------------|----------|
| Explicit dependency, git-resolved (Recommended) | deal init writes deal-std git dep into deal.toml; stdlib is a normal package | ✓ |
| Bundled with the toolchain | Implicit deal.std resolution; couples releases | |
| Hybrid: bundled fallback + overridable | Two resolution paths to test | |

**User's choice:** Explicit dependency, git-resolved

| Option | Description | Selected |
|--------|-------------|----------|
| Exact refs only (Recommended) | Tag/rev/branch pins; deal.lock records commit SHA; no semver ranges without a registry | ✓ |
| Semver ranges over git tags | Range resolution via tag listing — complexity for marginal value | |
| You decide | Planner/researcher picks from cargo/npm precedents | |

**User's choice:** Exact refs only

| Option | Description | Selected |
|--------|-------------|----------|
| Working starter model (Recommended) | PS-8 layout + real example; `deal check` passes immediately after init+install | ✓ |
| Empty skeleton | Directories + deal.toml only | |
| Interactive prompts | cargo/npm-init-style Q&A | |

**User's choice:** Working starter model

---

## Claude's Discretion

- Dimension/exponent declaration syntax in DEAL source (vehicle: SD-15/SD-16 annotations)
- Explicit conversion call syntax/naming
- New E-code band for dimensional errors (e.g., E25xx)
- ReqIF identifier derivation from D-23 path IDs; SpecObject attribute schema
- Whether plain .reqif XML is emittable alongside .reqifz
- deal.lock file format (TOML vs JSON)
- deal install invocation model (explicit vs auto-on-build)
- Starlight configuration (theme, search, sidebar)
- Plan slicing (suggested 5 waves; stdlib-sema and ReqIF emitter parallelizable)

## Deferred Ideas

- Stdlib expansion: RF, protocols (MIL-STD-1553, ARINC 429, SpaceWire, HTTP, MQTT), standards (DO-178C, DO-254, MIL-STD-810H) → Phase 6
- Implicit SI normalization at check time (rejected D-57 alternative; future ergonomics ADR)
- Semver ranges + centralized registry (out of scope v1)
- Per-package ReqIF slices for supplier exchange
- Semantic snippet validation (deal check on docs workspace)
- Docs versioning UI
- Interactive deal init prompts
