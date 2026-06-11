# Phase 4: Ecosystem - Research

**Researched:** 2026-06-05
**Domain:** Multi-domain: stdlib authoring (DEAL/Zig), ReqIF codegen (Rust), docs site (Astro/Starlight/Shiki), package resolution (Rust)
**Confidence:** HIGH (core stack), MEDIUM (ReqIF XSD validation), HIGH (Astro/Starlight)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Stdlib scope & unit semantics (D-54..D-57):**
- D-54: Stdlib ships REQ scope only — `units/` (SI + imperial), `interfaces/electrical/` (RJ45, USB-C, CAN, RS-422), `interfaces/mechanical/` (bolt patterns). RF, protocols, standards → Phase 6.
- D-55: Full dimensional algebra in `deal check` — unit expressions only. `km(200) / hr(1)` evaluates to Speed (L¹T⁻¹). General arithmetic expression semantics beyond units stays deferred.
- D-56: Dimensional knowledge is data-driven from stdlib, not hardcoded. Zig sema implements generic 7-exponent SI base-vector algebra (M, L, T, I, Θ, N, J) reading exponent metadata from DEAL-source declarations. User-defined units work automatically.
- D-57: Explicit conversions only at check time. Mixed-unit same-dimension expressions without explicit conversion = check error. `deal-stdlib` ships conversion call forms. New E-code band allocated.

**ReqIF codegen (D-58..D-61):**
- D-58: DOORS import validation follows D-36 viewer-smoke pattern. Priority: IBM DOORS → Eclipse RMF/ProR → `reqif` Python library → Jama/Polarion. XSD validation is the hard gate; tool import is the smoke.
- D-59: Mapping scope: `requirement def`/`need def` → SpecObjects; `@trace:<<satisfies>>`/`<<derives from>>` → SpecRelations; verification blocks → attributes; package hierarchy → Specification document tree. `part def`/`port def` NOT in ReqIF. Emitter is Rust walking IR JSON, mirroring D-19 pattern. IDs derive from D-23 path IDs.
- D-60: ReqIF 1.2 XSD acquisition is Plan 04-01. Download XSD + DOORS-compatible sample → `spec/references/omg-reqif/` + SHA256SUMS. No emitter work starts before validation target exists.
- D-61: Output is `.reqifz` archive. XSD validation runs against XML inside the archive. Golden-fixture pattern mirrors Phase 2.

**Docs site (D-62..D-65):**
- D-62: Language reference is topic pages, showcase-driven. ~10–14 pages lifted from the 19-file showcase.
- D-63: Deploy on GitHub Pages from `deal-lang.org` sibling repo via GitHub Actions. Astro static output.
- D-64: Domain is `deal-lang.org` (NOT `.dev`). NC-1 amendment ADR required. DNS CNAME setup is user-side manual step. Sibling repo directory name `deal-lang.org/` may stay or be renamed — planner's call.
- D-65: CI parses every docs snippet with real CLI (`deal parse`). Intentional-error examples carry an explicit fence marker. Build fails on placeholders/missing Shiki scopes.

**Package resolution (D-66..D-69):**
- D-66: Dependencies vendor per-project into `.deal/deps/<name>/`. Git deps cloned there; local-path deps referenced in-place.
- D-67: `deal-stdlib` is an explicit git-resolved dependency. `deal init` writes `[dependencies] deal-std = { git = "https://github.com/deal-lang/deal-stdlib", tag = "v0.x" }` into scaffolded `deal.toml`.
- D-68: Exact refs only; SHA-pinned lockfile. No semver ranges. `deal.lock` format details are researcher/planner discretion.
- D-69: `deal init` scaffolds a working starter model. Full PS-8 layout + minimal real example. `deal check` passes immediately after `deal init` + `deal install`. No interactive prompts.

### Claude's Discretion

- Dimension/exponent declaration syntax in DEAL source (D-56) — how `deal-stdlib` declares "kg is M¹". Must use existing locked grammar constructs (likely SD-15/SD-16 annotations on `attribute def`-style unit/dimension definitions).
- Explicit conversion call syntax (D-57) — function-form per PS-6/SD-13 spirit; exact naming (e.g., `to_kg(...)`, `kg.from(lb(3300))`).
- New E-code band for dimensional errors — extend D-16/D-33 (e.g., `E25xx` for unit/dimension checks).
- ReqIF identifier derivation from D-23 path IDs and exact SpecObject attribute schema (D-59).
- Whether plain `.reqif` XML is emittable alongside `.reqifz` (D-61).
- `deal.lock` file format (D-68) — TOML vs JSON, field shape.
- `deal install` invocation model — explicit-only vs auto-resolve-on-build.
- Starlight configuration details — theme, search (Pagefind default), sidebar structure, dark/light.
- Plan slicing — suggested waves: (1) XSD acquisition + package resolution core, (2) stdlib authoring + dimensional algebra sema, (3) ReqIF emitter + golden fixtures, (4) docs site + CI gates, (5) phase-4-gate + gate-fresh + closeout.

### Deferred Ideas (OUT OF SCOPE)

- Stdlib expansion packages (RF, protocols, standards) → Phase 6
- Implicit SI normalization → rejected D-57 alternative; future ergonomics ADR
- Semver range resolution + centralized registry → out of scope v1
- Per-package ReqIF slices for supplier exchange
- Semantic snippet validation (`deal check` with stdlib-resolving docs workspace) — parse-only chosen
- Docs versioning UI → single current version
- Interactive `deal init` prompts → Phase 6 polish
- `deal index --watch` / `deal init --watch` → still deferred
- General arithmetic expression semantics beyond unit expressions
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| REQ-phase-4-ecosystem | Standard library + second backend (ReqIF) + documentation site | All four research domains below |
| REQ-phase-4-1-stdlib | Author first standard library packages in DEAL itself — SI + imperial units, electrical + mechanical interfaces | §Standard Stack (Zig sema), §Dimensional Algebra Pattern, §BIPM SI reference |
| REQ-phase-4-2-reqif-codegen | IR → ReqIF XML backend for DOORS/Jama, XSD validation, `.reqifz` container | §ReqIF Domain, §XSD Validation Crates, §Package Legitimacy Audit |
| REQ-phase-4-3-docs-site | `deal-lang.org` on Astro + Starlight with Shiki grammar, CI snippet gate, GH Pages deploy | §Docs Site Domain, §Astro/Starlight patterns |
| REQ-phase-4-4-package-resolution | `deal.lock`, `deal install` (local-path + git), `deal init` scaffolding | §Package Resolution Domain, §Lockfile Design |
| REQ-phase-4-gate | Phase 4 exit gate — minimum viable language, `phase-4-gate` + `phase-4-gate-fresh` | §Gate Pattern, §Environment Availability |
</phase_requirements>

---

## Summary

Phase 4 delivers four parallel work streams that form the first external release candidate for the DEAL toolchain. Each stream has a clear technical owner (established by D-19/D-49): Zig owns the new sema (dimensional algebra), Rust owns ReqIF codegen and package resolution CLI, the `deal-lang.org` sibling repo owns the docs site.

**Stream 1 (stdlib):** Author `units/`, `interfaces/electrical/`, and `interfaces/mechanical/` DEAL source files in the `deal-stdlib` sibling repo. Add exponent metadata to unit/dimension definitions via SD-15/SD-16 annotation syntax. The Zig sema gains a generic 7-exponent SI vector algebra that reads these annotations — all without any hardcoded stdlib knowledge (D-56, FA-2). The critical design question is the annotation syntax for exponent metadata: based on the locked grammar, `@si_exponent[M:1, L:0, T:0, I:0, θ:0, N:0, J:0]` or equivalent on `attribute def`-style declarations is the natural fit.

**Stream 2 (ReqIF):** Acquire the OMG ReqIF 1.2 XSD (`https://www.omg.org/spec/ReqIF/20110401/reqif.xsd` — confirmed live, 892 lines). Build a Rust emitter mirroring `sysml_v2.rs` that walks IR JSON and emits ReqIF XML inside a `.reqifz` zip archive. Validate against the XSD using `quick-xml` for parsing and a lightweight XSD validator. The XSD imports `driver.xsd` which handles XHTML content types.

**Stream 3 (docs site):** Initialize Astro 6.4.4 + Starlight 0.39.3 in `deal-lang.org`. Starlight uses Expressive Code for code blocks; custom TextMate grammars from `vscode-deal/syntaxes/` are loaded via `expressiveCode.shiki.langs`. GitHub Pages deployment is workflow-only (no adapter package needed — `withastro/action@v6` handles it). Domain is `deal-lang.org` per D-64.

**Stream 4 (package resolution):** Extend the Rust CLI with `deal init`, `deal install`, and `[dependencies]` parsing. The `toml` crate (already at 0.8 in workspace) parses `deal.toml`. Git cloning uses `git2` (libgit2 bindings). `deal.lock` is TOML format with SHA-pinned git revisions.

**Primary recommendation:** Execute streams 1+4 in parallel first (stdlib foundations + package resolution), then stream 2 (ReqIF emitter, gated on XSD acquisition), then stream 3 (docs site, gated on streams 1+2 having working CLI output to showcase). Gate step should wire all four.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Dimensional algebra / unit checks | Zig sema (`src/sema.zig`) | — | D-49 architectural split; sema owns all type-level analysis |
| `deal-stdlib` DEAL sources | `deal-stdlib` sibling repo | Zig sema (consumes) | Stdlib is a normal package (D-56, FA-2) |
| ReqIF XML generation + `.reqifz` archive | Rust CLI (`cli/src/reqif.rs`) | — | D-19: Rust owns all codegen backends |
| XSD validation (ReqIF) | Rust CLI | `spec/references/omg-reqif/` (schema bundle) | Mirrors `schema_registry.rs` offline pattern |
| `deal init` / `deal install` | Rust CLI (`cli/src/`) | Zig core (parse to validate init model) | CLI owns filesystem operations |
| `deal.toml` [dependencies] parsing | Rust CLI | — | `toml` crate already in workspace |
| Git dependency resolution | Rust CLI | `git2` crate | Native git clone, no subprocess |
| `deal.lock` generation | Rust CLI | — | SHA pinning lives in Rust resolver |
| Astro + Starlight docs site | `deal-lang.org` sibling repo | CI (GH Actions) | Separate repo; deploys independently |
| Shiki custom grammar | `deal-lang.org` (`astro.config.mjs`) | `vscode-deal/syntaxes/` (source) | Expressive Code loads TextMate JSON at build time |
| Docs snippet CI gate (`deal parse`) | `deal-lang.org` GitHub Actions | Rust CLI binary | CI invokes `deal parse` against extracted snippets |
| `phase-4-gate` + `phase-4-gate-fresh` | `build.zig` (Zig build system) | `scripts/verify-fresh-worktree.sh` | Inherits phase-3 gate pattern + sibling-repo symlinking |

---

## Standard Stack

### Core (Rust CLI additions)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `toml` | 0.8 [VERIFIED: Cargo.toml workspace] | `deal.toml` [dependencies] parsing | Already in workspace; parses the existing format |
| `git2` | 0.21.0 [VERIFIED: cargo search] | Git clone/fetch for `deal install` | libgit2 Rust bindings; used by Cargo itself for git deps |
| `quick-xml` | 0.40.1 [VERIFIED: cargo search] | ReqIF XML emission and XSD validation | High-performance, widely used; passes slopcheck [OK] |
| `zip` | 9.0.0-pre2 [VERIFIED: cargo search] | `.reqifz` archive creation | Standard zip crate; passes slopcheck [OK] |
| `sha2` + `hex` | 0.10 / 0.4 [CITED: cli/Cargo.toml] | SHA256 pinning in `deal.lock` + XSD bundle verify | Already used in `schema_registry.rs` |
| `uuid` | 1.x (v5 feature) [VERIFIED: Cargo.toml workspace] | ReqIF identifier derivation from D-23 path IDs | Same pattern as SysML v2 emitter |
| `serde` + `serde_json` | 1.x [VERIFIED: Cargo.toml workspace] | IR JSON deserialization for ReqIF emitter | Established workspace pattern |

### Core (Docs site)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `astro` | 6.4.4 [VERIFIED: npm registry] | Static site framework | Current stable; Starlight is an Astro integration |
| `@astrojs/starlight` | 0.39.3 [VERIFIED: npm registry] | Documentation theme with sidebar, search, i18n | Official Astro documentation framework |
| `shiki` | 4.2.0 [VERIFIED: npm registry] | Syntax highlighting engine (via Expressive Code) | Used by Starlight/Expressive Code internally |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `toml_edit` | 0.25.12 [VERIFIED: cargo search] | Format-preserving TOML write for `deal.lock` generation | If lock file needs to be human-readable and round-trip safe; alternative: plain `toml` serialize |
| `pagefind` | 1.5.2 [VERIFIED: npm registry] | Client-side search for Starlight docs | Starlight default; no config needed |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `git2` | `std::process::Command` calling `git` | Subprocess is simpler but requires `git` on PATH (acceptable for dev, but fragile in CI); `git2` is stdlib-independent |
| `quick-xml` | `xmltree` / `xml-rs` | `quick-xml` is 10x faster and streaming; `xmltree` requires loading full DOM (fine for ReqIF which is small) |
| `zip` crate | `flate2` + manual zip format | `zip` handles the container format correctly; ReqIF spec requires exact `.reqifz` format |
| Starlight Expressive Code | Plain Shiki integration | Expressive Code provides filename labels, diff highlighting, and CI-friendly build behavior — Starlight ships it as default |
| `toml` 0.8 for lock | JSON (`serde_json`) for lock | TOML is more readable and matches `deal.toml` convention; JSON is simpler to deserialize |

**Installation (Rust):**
```bash
# In cli/Cargo.toml (new deps alongside existing)
git2 = "0.21"
quick-xml = { version = "0.40", features = ["serialize"] }
zip = "2"   # 2.x is the stable series (not the 9.x pre-release)
```

**Installation (docs site — in deal-lang.org/):**
```bash
npm create astro@latest -- --template starlight
npm install  # installs astro + @astrojs/starlight
```

> **Note on `zip` version:** `cargo search` returned `9.0.0-pre2` which is a pre-release. The stable series is `2.x`. The planner should use `zip = "2"` for the Cargo.toml entry.

---

## Package Legitimacy Audit

> slopcheck 0.6.1 run on 2026-06-05 against crates.io registry.

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| `toml` | crates.io | 10+ yrs | Extremely high | github.com/toml-rs/toml | [OK] | Approved |
| `git2` | crates.io | 10+ yrs | Very high | github.com/rust-lang/git2-rs | [OK] | Approved |
| `quick-xml` | crates.io | 7+ yrs | Very high | github.com/tafia/quick-xml | [OK] (flagged "quick-" prefix as LLM bait, but established) | Approved |
| `zip` | crates.io | 8+ yrs | High | github.com/zip-rs/zip | [OK] | Approved |
| `uuid` | crates.io | 10+ yrs | Very high | github.com/uuid-rs/uuid | [OK] | Approved |
| `serde` | crates.io | 10+ yrs | Dominant | github.com/serde-rs/serde | [OK] | Approved |
| `serde_json` | crates.io | 10+ yrs | Dominant | github.com/serde-rs/json | [OK] | Approved |
| `astro` (npm) | npm | 4+ yrs | Very high | github.com/withastro/astro | Not checked by slopcheck (npm) | Approved — major framework |
| `@astrojs/starlight` (npm) | npm | 2+ yrs | High | github.com/withastro/starlight | Not checked by slopcheck (npm) | Approved — official Astro integration |
| `shiki` (npm) | npm | 4+ yrs | Very high | github.com/shikijs/shiki | [SUS] on crates.io (wrong registry — `shiki` npm is clean) | Approved — npm package is legitimate; slopcheck error was wrong-registry check |

**Packages removed due to slopcheck [SLOP] verdict:** none

**Packages flagged as suspicious [SUS]:** The `shiki` crates.io flag is a registry confusion — this phase uses the `shiki` npm package (legitimate, 4.2.0, official Shiki project), not a crates.io package named `shiki`.

---

## Architecture Patterns

### System Architecture Diagram

```
User invokes deal CLI
      │
      ├─ deal init ──────────────────────────────────────────────────────────────►
      │        │                                                                 │
      │   Rust: scaffold PS-8 layout                                    deal.toml + .deal/
      │   + write [dependencies] deal-std = {git=...}                   model/ packages/ ...
      │
      ├─ deal install ───────────────────────────────────────────────────────────►
      │        │                                                                 │
      │   Rust: parse deal.toml [dependencies]                          .deal/deps/<name>/
      │   → git2 clone/fetch at exact tag/rev                           deal.lock (SHA-pinned)
      │
      ├─ deal check <dir>
      │        │
      │   Rust: expand paths → call FFI per file
      │        │
      │   Zig (sema): pass A symbol table, pass B resolve imports
      │        │         │
      │        │    external import (deal.std.units.{kg}):
      │        │    resolve pkg → .deal/deps/deal-std/ → parse stdlib .deal files
      │        │    read @si_exponent annotations → 7-vector algebra check
      │        │    emit E25xx on dimension mismatch / mixed-unit comparison
      │        │
      │   diagnostics JSON → Rust render
      │
      ├─ deal build --target reqif
      │        │
      │   Rust: expand paths → FFI parse+lower → IR JSON bytes (clone pre-free)
      │        │
      │   Rust reqif.rs: walk IR JSON → select requirement/need/trace/verification nodes
      │        │         → emit ReqIF XML → validate against XSD bundle (quick-xml)
      │        │         → zip into .reqifz archive
      │        │
      │   tests/golden/reqif/ ← golden fixture comparison
      │
      └─ deal parse <file>  ← used by docs CI
               │
          stdout: raw AST JSON (D-32 WARNING-05 Option A)

docs site (deal-lang.org repo — separate)
      │
      ├─ astro build
      │        │
      │   Starlight: MDX pages + Expressive Code
      │        │         │
      │        │    expressiveCode.shiki.langs = [
      │        │      JSON.parse(deal.tmLanguage.json),   ← from vscode-deal/syntaxes/
      │        │      JSON.parse(dealx.tmLanguage.json)
      │        │    ]
      │        │
      │   CI: extract .deal/.dealx fenced blocks → deal parse → assert exit 0
      │   CI: build fails if Shiki unknown-scope tokens detected
      │
      └─ GitHub Actions: withastro/action@v6 → GitHub Pages
```

### Recommended Project Structure

**deal-stdlib sibling repo:**
```
deal-stdlib/
├── deal.toml                    # [project] name="deal-std" version="0.4.0"
├── packages/
│   ├── units/
│   │   ├── dimensions.deal      # SI base dimension definitions with @si_exponent
│   │   ├── si.deal              # SI base + derived units
│   │   ├── imperial.deal        # Imperial unit definitions + conversion forms
│   │   └── mod.deal             # exports
│   └── interfaces/
│       ├── electrical/
│       │   ├── rj45.deal
│       │   ├── usb_c.deal
│       │   ├── can.deal
│       │   ├── rs422.deal
│       │   └── mod.deal
│       └── mechanical/
│           ├── bolt_patterns.deal
│           └── mod.deal
```

**deal-lang.org sibling repo (after Astro init):**
```
deal-lang.org/
├── astro.config.mjs
├── package.json
├── public/
│   └── CNAME                    # contains: deal-lang.org
├── src/
│   ├── content/
│   │   └── docs/
│   │       ├── index.mdx        # landing page
│   │       ├── getting-started.mdx
│   │       ├── reference/
│   │       │   ├── definitions.mdx
│   │       │   ├── compositions.mdx
│   │       │   ├── requirements.mdx
│   │       │   ├── traceability.mdx
│   │       │   ├── imports.mdx
│   │       │   ├── annotations.mdx
│   │       │   ├── units.mdx
│   │       │   └── deal-toml.mdx
│   │       └── cli/
│   │           ├── index.mdx
│   │           └── vscode-setup.mdx
│   └── grammars/                # symlink or copy of vscode-deal/syntaxes/
│       ├── deal.tmLanguage.json
│       └── dealx.tmLanguage.json
├── scripts/
│   └── check-snippets.sh        # CI: extract + deal parse every .deal block
└── .github/
    └── workflows/
        └── deploy.yml           # withastro/action@v6 → GH Pages
```

**deal/ main repo additions:**
```
cli/src/
├── reqif.rs                     # New: ReqIF emitter (mirrors sysml_v2.rs)
├── reqif_schema.rs              # New: XSD validation (mirrors schema_registry.rs)
├── resolver.rs                  # New: deal.toml [dependencies] + git2 clone
└── main.rs                      # Extended: Init, Install subcommands; BuildTarget::Reqif

spec/references/omg-reqif/
├── reqif.xsd                    # Downloaded from OMG (Plan 04-01)
├── driver.xsd                   # XHTML schema imported by reqif.xsd
└── SHA256SUMS                   # Matches existing omg-sysml-v2 convention

tests/golden/reqif/
├── 01-requirement-def.deal
├── 01-requirement-def.expected.reqif  # Hand-written
└── ...
```

### Pattern 1: Expressive Code Custom Grammar in Starlight

```javascript
// astro.config.mjs
// Source: https://expressive-code.com/key-features/syntax-highlighting/
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import { readFileSync } from 'fs';
import { fileURLToPath } from 'url';
import path from 'path';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// Load DEAL TextMate grammars from vscode-deal sibling repo (or local copy)
const dealGrammar = JSON.parse(
  readFileSync(path.join(__dirname, 'src/grammars/deal.tmLanguage.json'), 'utf-8')
);
const dealxGrammar = JSON.parse(
  readFileSync(path.join(__dirname, 'src/grammars/dealx.tmLanguage.json'), 'utf-8')
);

export default defineConfig({
  site: 'https://deal-lang.org',  // D-64: org, not dev
  integrations: [
    starlight({
      title: 'DEAL',
      expressiveCode: {
        shiki: {
          langs: [dealGrammar, dealxGrammar],
        },
      },
    }),
  ],
});
```

### Pattern 2: ReqIF XML Emitter Structure (Rust)

```rust
// cli/src/reqif.rs — mirrors sysml_v2.rs structure
// Source: [ASSUMED] — follows D-19 + D-59 + D-61 + existing sysml_v2.rs pattern

use quick_xml::{Writer, events::{BytesDecl, BytesStart, BytesText, Event}};
use zip::write::{FileOptions, ZipWriter};
use std::io::Cursor;

/// Emit a .reqifz archive from IR v0 JSON bytes.
/// Returns the archive bytes.
pub fn emit_from_bytes(ir_json: &[u8], output_path: &Path) -> anyhow::Result<()> {
    let ir: serde_json::Value = serde_json::from_slice(ir_json)?;

    // Filter IR nodes to requirements/needs/traces only (D-59)
    let req_nodes: Vec<_> = ir["elements"].as_object()
        .map(|e| e.values()
            .filter(|n| matches!(n["kind"].as_str(), Some("requirement_def" | "need_def")))
            .collect())
        .unwrap_or_default();

    // Emit ReqIF XML
    let mut xml_buf = Vec::new();
    emit_reqif_xml(&req_nodes, &ir, &mut xml_buf)?;

    // Wrap in .reqifz zip archive (D-61)
    let zip_buf = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(zip_buf);
    zip.start_file("model.reqif", FileOptions::default())?;
    zip.write_all(&xml_buf)?;
    let finished = zip.finish()?;
    std::fs::write(output_path, finished.into_inner())?;
    Ok(())
}
```

### Pattern 3: Dimensional Algebra in Zig Sema

```zig
// src/sema.zig extension for D-55/D-56
// Source: [ASSUMED] — follows D-56 data-driven design

/// SI dimension exponent vector: [M, L, T, I, Θ, N, J]
pub const DimVector = [7]i8;

pub const DIM_DIMENSIONLESS: DimVector = .{ 0, 0, 0, 0, 0, 0, 0 };
pub const DIM_MASS:          DimVector = .{ 1, 0, 0, 0, 0, 0, 0 };
pub const DIM_LENGTH:        DimVector = .{ 0, 1, 0, 0, 0, 0, 0 };
pub const DIM_TIME:          DimVector = .{ 0, 0, 1, 0, 0, 0, 0 };
// ... etc.

/// Check #7: unit expression dimensional consistency
/// Reads exponent metadata from deal-stdlib DEAL declarations (D-56)
fn checkDimensionalExpr(
    sema: *Sema,
    expr: *ast.Node,
    expected_dim: DimVector,
) !bool {
    // evaluate unit expression → derive DimVector
    // compare against declared type's DimVector
    // emit E25xx on mismatch
}
```

### Pattern 4: GitHub Pages Deployment Workflow

```yaml
# .github/workflows/deploy.yml (in deal-lang.org repo)
# Source: https://docs.astro.build/en/guides/deploy/github/
name: Deploy to GitHub Pages
on:
  push:
    branches: [main]
  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: "pages"
  cancel-in-progress: false

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Build with Astro
        uses: withastro/action@v6
      - name: Check snippets
        run: |
          # Download deal binary from GitHub Releases
          curl -sSL https://github.com/deal-lang/deal/releases/latest/download/deal-linux-x86_64.tar.gz | tar xz
          ./scripts/check-snippets.sh
  deploy:
    needs: build
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    steps:
      - name: Deploy to GitHub Pages
        id: deployment
        uses: actions/deploy-pages@v4
```

### Pattern 5: deal.lock Format Design

```toml
# deal.lock — TOML format, D-18 alphabetical-key discipline
# Source: [ASSUMED] — modeled on Cargo.lock git-dep format

version = 1

[[package]]
name = "deal-std"
source = "git+https://github.com/deal-lang/deal-stdlib?tag=v0.4.0#abc123def456..."
# resolved = exact commit SHA at time of `deal install`

[[package]]
name = "my-interfaces"
source = "path+/Users/engineer/projects/my-interfaces"
# local-path deps: no SHA (referenced in place, D-66)
```

### Pattern 6: `deal init` Scaffolded deal.toml

```toml
# Generated by `deal init` per D-67/D-69
[project]
name = "my-project"
version = "0.1.0"
schema = "deal/0.1"
description = "A DEAL model project"

[workspace]
packages = ["packages/*"]

[dependencies]
# deal-stdlib: git-resolved per D-67. Tag will be updated to match the
# deal toolchain release at install time (D-68 exact-refs only).
deal-std = { git = "https://github.com/deal-lang/deal-stdlib", tag = "v0.4.0" }

[build.targets]
sysml-v2 = { format = "json", output = "build/sysml-v2/" }
reqif = { format = "xml", output = "build/reqif/" }
```

### Anti-Patterns to Avoid

- **Hardcoding SI unit knowledge in the Zig compiler:** Violates D-56/FA-2. All dimensional metadata must come from `deal-stdlib` DEAL source annotations. The Zig sema must remain stdlib-agnostic.
- **Implicit unit coercion at check time:** Violates D-57. Never silently normalize `lb` to `kg` — emit E25xx and require explicit conversion.
- **Using the `path` property in Shiki v1.0+:** The `path` property was removed in Shiki v1.0. Pass the parsed JSON object, not a filesystem path.
- **Fetching schemas at runtime:** Violates the offline-only constraint. All schemas (`reqif.xsd`, `driver.xsd`) must be bundled in `spec/references/omg-reqif/` and SHA256-verified at startup (mirrors T-02-21 pattern).
- **Not running `deal parse` on intentional-error docs examples:** Intentional-error snippets must carry a fence marker (e.g., `deal error-expected`) so the CI script skips them — otherwise CI will spuriously fail on examples that are supposed to show errors.
- **Using semver ranges in `deal.lock`:** Violates D-68. Only exact commit SHAs for git deps.
- **Writing `.deal/deps/` to the package-level gitignore without committing:** PS-10 mandates `.deal/` is gitignored; vendored deps inside it are the user's policy choice (D-66 explicitly supports committing the vendored tree for airgap programs).

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| XML emission | Custom string formatting | `quick-xml` Writer | Namespace handling, XML declaration, attribute escaping are subtle |
| Zip archive creation | Manual byte layout | `zip` crate | `.reqifz` requires exact PKZIP format; byte alignment and CRC are non-trivial |
| Git repository cloning | `std::process::Command("git")` | `git2` crate | Object resolution, ref peeling, auth, and transport protocols are complex; `git2` is what Cargo uses |
| SHA256 hashing | Custom implementation | `sha2` crate (already in workspace) | Already used in `schema_registry.rs` for T-02-21; one pattern for all schema verification |
| TOML serialization | Custom text formatting | `toml` crate (already in workspace) | Quoting rules, multi-line strings, datetime types |
| Syntax highlighting | Custom tokenizer | Shiki via Expressive Code | TextMate grammar engine is ~20k lines; scope resolution is non-trivial |
| Client-side search | Custom index | Pagefind (Starlight default) | Static search index, WASM-powered, zero config in Starlight |
| CNAME custom domain wiring | Manual GitHub Pages config | `public/CNAME` file | GitHub Pages reads this file automatically; no API calls needed |

**Key insight:** The ReqIF XSD imports `driver.xsd` (XHTML namespace). The XML validator must handle cross-schema imports — `quick-xml` alone is a reader, not an XSD validator. Research found no mature, offline XSD 1.0 validator crate for Rust. The recommended approach (per D-58's pragmatic stance) is to use `quick-xml` for XML well-formedness + structural validation (element names, required attributes) as the primary gate, with optional round-trip through the `reqif` Python library for semantic XSD compliance. See pitfall §P-3 below.

---

## ReqIF Domain — Key Technical Facts

### ReqIF 1.2 XSD Acquisition

- **Main XSD:** `https://www.omg.org/spec/ReqIF/20110401/reqif.xsd` — confirmed live, 892 lines, `Content-Type: application/xml` [VERIFIED: direct HTTP probe 2026-06-05]
- **Driver XSD (XHTML import):** `http://www.omg.org/spec/ReqIF/20110402/driver.xsd` — referenced inside `reqif.xsd` via `xsd:import schemaLocation`
- **Formal document number:** formal/2016-07-01 (ReqIF 1.2) [CITED: www.omg.org/spec/ReqIF/1.2/]
- **XSD namespace:** `http://www.omg.org/spec/ReqIF/20110401/reqif.xsd`
- **Machine-readable unchanged since 1.0.1** — XSD schema is stable [CITED: www.omg.org/spec/ReqIF/1.2/]
- Per D-60, acquire both XSDs + SHA256SUMS into `spec/references/omg-reqif/`

### ReqIF Structure for DEAL Mapping (D-59)

```xml
<!-- Minimal ReqIF 1.2 structure for DEAL requirements -->
<REQ-IF xmlns="http://www.omg.org/spec/ReqIF/20110401/reqif.xsd">
  <THE-HEADER>
    <REQ-IF-HEADER IDENTIFIER="..." CREATION-TIME="..." REQ-IF-TOOL-ID="deal" />
  </THE-HEADER>
  <CORE-CONTENT>
    <SPEC-TYPES>
      <!-- One SPEC-OBJECT-TYPE per DEAL element kind (requirement_def, need_def) -->
      <SPEC-OBJECT-TYPE IDENTIFIER="..." LONG-NAME="RequirementType">
        <SPEC-ATTRIBUTES>
          <ATTRIBUTE-DEFINITION-STRING IDENTIFIER="..." LONG-NAME="ReqText" />
          <ATTRIBUTE-DEFINITION-STRING IDENTIFIER="..." LONG-NAME="Threshold" />
          <!-- verification block fields: threshold, operator, method -->
        </SPEC-ATTRIBUTES>
      </SPEC-OBJECT-TYPE>
      <SPEC-RELATION-TYPE IDENTIFIER="..." LONG-NAME="SatisfiesRelation" />
    </SPEC-TYPES>
    <SPEC-OBJECTS>
      <!-- One per requirement_def / need_def in IR -->
      <SPEC-OBJECT IDENTIFIER="$deal_path_id" LONG-NAME="REQ_BAT_001">
        <TYPE><SPEC-OBJECT-TYPE-REF>RequirementType</SPEC-OBJECT-TYPE-REF></TYPE>
        <VALUES>
          <!-- JSDoc doc comment → ReqText attribute -->
          <ATTRIBUTE-VALUE-STRING THE-VALUE="Battery...">
            <DEFINITION><ATTRIBUTE-DEFINITION-STRING-REF>...</ATTRIBUTE-DEFINITION-STRING-REF></DEFINITION>
          </ATTRIBUTE-VALUE-STRING>
        </VALUES>
      </SPEC-OBJECT>
    </SPEC-OBJECTS>
    <SPEC-RELATIONS>
      <!-- @trace:<<satisfies>> → SpecRelation -->
      <SPEC-RELATION IDENTIFIER="..." >
        <TYPE><SPEC-RELATION-TYPE-REF>SatisfiesRelation</SPEC-RELATION-TYPE-REF></TYPE>
        <SOURCE><SPEC-OBJECT-REF>$source_path_id</SPEC-OBJECT-REF></SOURCE>
        <TARGET><SPEC-OBJECT-REF>$target_path_id</SPEC-OBJECT-REF></TARGET>
      </SPEC-RELATION>
    </SPEC-RELATIONS>
    <SPECIFICATIONS>
      <!-- Package hierarchy → Specification document tree -->
      <SPECIFICATION IDENTIFIER="..." LONG-NAME="EV Platform Requirements">
        <CHILDREN>
          <SPEC-HIERARCHY>
            <OBJECT><SPEC-OBJECT-REF>$req_id</SPEC-OBJECT-REF></OBJECT>
          </SPEC-HIERARCHY>
        </CHILDREN>
      </SPECIFICATION>
    </SPECIFICATIONS>
  </CORE-CONTENT>
</REQ-IF>
```

### ReqIF Identifier Derivation (D-59 + D-23)

DEAL uses fully-qualified dot-path IDs (D-23). ReqIF identifiers must be valid XML ID tokens (no dots). Recommended derivation: replace dots with underscores and prefix with `DEAL_`:

- `requirements.system.REQ_BAT_001` → `DEAL_requirements_system_REQ_BAT_001`
- Alternatively: use UUID v5 with a ReqIF namespace (matches SysML v2 pattern)

The planner should lock one approach and document it in the emitter.

---

## Dimensional Algebra Domain

### SI Base Vector (D-56)

| Index | Quantity | Symbol | Unit |
|-------|----------|--------|------|
| 0 | Mass | M | kg |
| 1 | Length | L | m |
| 2 | Time | T | s |
| 3 | Electric current | I | A |
| 4 | Thermodynamic temperature | Θ | K |
| 5 | Amount of substance | N | mol |
| 6 | Luminous intensity | J | cd |

Source: BIPM SI Brochure 9th edition (2019) [CITED: www.bipm.org/en/publications/si-brochure]

### Derived Unit Vectors (examples for stdlib)

| Unit | M | L | T | I | Θ | N | J | From |
|------|---|---|---|---|---|---|---|------|
| kg | 1 | 0 | 0 | 0 | 0 | 0 | 0 | base |
| m | 0 | 1 | 0 | 0 | 0 | 0 | 0 | base |
| s | 0 | 0 | 1 | 0 | 0 | 0 | 0 | base |
| A | 0 | 0 | 0 | 1 | 0 | 0 | 0 | base |
| N (newton) | 1 | 1 | -2 | 0 | 0 | 0 | 0 | kg·m·s⁻² |
| J (joule) | 1 | 2 | -2 | 0 | 0 | 0 | 0 | N·m |
| W (watt) | 1 | 2 | -3 | 0 | 0 | 0 | 0 | J/s |
| V (volt) | 1 | 2 | -3 | -1 | 0 | 0 | 0 | W/A |
| m/s (speed) | 0 | 1 | -1 | 0 | 0 | 0 | 0 | L·T⁻¹ |
| lb (pound-mass) | 1 | 0 | 0 | 0 | 0 | 0 | 0 | same as kg |

### Dimension Metadata Annotation Syntax (Claude's Discretion)

Using SD-15/SD-16 annotations on unit definitions in `deal-stdlib`:

```deal
// dimensions.deal — SI base dimensions
@si_base_dimension[M:1, L:0, T:0, I:0, Θ:0, N:0, J:0]
attribute def Mass;

@si_base_dimension[M:0, L:1, T:0, I:0, Θ:0, N:0, J:0]
attribute def Length;

// si.deal — SI units
@si_unit[dim: "Mass", factor: 1.0]
attribute def kg : Mass;

@si_unit[dim: "Mass", factor: 0.001]  // 1 g = 0.001 kg
attribute def g : Mass;

// imperial.deal — imperial units with conversion factor to SI
@si_unit[dim: "Mass", factor: 0.453592]  // 1 lb = 0.453592 kg
attribute def lb : Mass;

// Conversion call syntax (D-57) — explicit only, no implicit coercion
// Naming convention: to_<target_unit>(<source_unit_expr>)
// Example: to_kg(lb(3300))
```

> **Design note for planner:** The exact annotation keys (`@si_base_dimension`, `@si_unit`) are researcher's recommendation for Claude's Discretion. They must use existing locked grammar annotation syntax (SD-15/SD-16). The Zig sema reads these at check time.

### E-Code Band Recommendation for Dimensional Errors (Claude's Discretion)

Existing bands:
- `E2000..E2499` — Phase 2 semantic checks
- Gap available: `E2500..E2599` for dimensional/unit checks

Recommended allocation:
- `E2500` — dimension mismatch (e.g., `mass : Mass = V(800)` — Voltage ≠ Mass)
- `E2501` — mixed-unit same-dimension without explicit conversion (e.g., `kg(1) + lb(1)`)
- `E2502` — unknown unit in expression (unit literal not resolvable to a declared dimension)
- `E2503` — conversion call type mismatch (wrong source or target dimension in `to_kg(...)`)

---

## Package Resolution Domain

### deal.toml [dependencies] Format

The showcase `deal.toml` already includes a `[dependencies]` section (line 19). The format is already defined by PS-1:

```toml
[dependencies]
deal-std = { git = "https://github.com/deal-lang/deal-stdlib", tag = "v0.4.0" }
my-interfaces = { path = "../my-interfaces" }
```

The `toml` crate (0.8, already in workspace) can deserialize this directly.

### Lockfile Format (Claude's Discretion)

Recommendation: TOML with alphabetical keys (D-18 discipline):

```toml
# deal.lock — auto-generated by `deal install`. Commit to git.
# Format: version 1
version = 1

[[package]]
name = "deal-std"
git = "https://github.com/deal-lang/deal-stdlib"
tag = "v0.4.0"
rev = "abc123def456789abc123def456789abc123def4"

[[package]]
name = "my-interfaces"
path = "../my-interfaces"
# no rev for path deps — they track the filesystem
```

Rationale for TOML over JSON: human-readable, matches `deal.toml` convention, the `toml` crate handles it. TOML is what Cargo uses for `Cargo.lock`.

### Import Resolution through Dependencies (D-66)

The external import resolution path for `import deal.std.units.{kg}`:

1. `deal check` invoked (Rust CLI)
2. CLI reads `deal.toml` → discovers `deal-std` → confirms `.deal/deps/deal-std/` exists (or emits "run deal install")
3. CLI passes `.deal/deps/deal-std/packages/units/si.deal` to Zig FFI alongside the project sources
4. Zig sema resolves `deal.std.units.kg` through the multi-file symbol table

**Key constraint:** The Zig core remains single-file/workspace-agnostic (D-49). The Rust CLI is responsible for assembling the file set and passing it to the FFI.

---

## Docs Site Domain

### Astro + Starlight Facts

- **Astro 6.4.4** — current stable [VERIFIED: npm registry, 2026-06-05]
- **@astrojs/starlight 0.39.3** — current stable [VERIFIED: npm registry, 2026-06-05]
- **No adapter package needed for GitHub Pages** — `withastro/action@v6` handles build + deploy [CITED: docs.astro.build/en/guides/deploy/github/]
- **Static output** — Astro default output mode is `'static'`; no SSR needed for a docs site
- **Custom domain:** `public/CNAME` file containing `deal-lang.org` + remove `base` from `astro.config.mjs` [CITED: docs.astro.build/en/guides/deploy/github/]
- **Pagefind search** — Starlight's default; zero configuration needed [ASSUMED]

### Expressive Code + Shiki Custom Grammar

- Starlight uses Expressive Code for all code blocks [CITED: starlight.astro.build/reference/configuration/]
- Custom language grammars go in `expressiveCode.shiki.langs` as parsed JSON objects [CITED: expressive-code.com/key-features/syntax-highlighting/]
- **Shiki v1.0+ breaking change:** The `path` property no longer works. Must pass parsed object via `JSON.parse(readFileSync(...))` [CITED: shiki.style/guide/load-lang]
- The TextMate grammars from `vscode-deal/syntaxes/deal.tmLanguage.json` and `dealx.tmLanguage.json` (Phase 3, D-41) are the source files for the docs Shiki integration

### Docs CI Snippet Gate (D-65)

Two CI checks:
1. **Shiki scope gate** — Astro build fails if any `.deal`/`.dealx` fenced block contains unresolved scopes (tokens with no TextMate match → unstyled). This is built into the build pipeline: a custom Astro integration or Expressive Code plugin that checks for `token-plain` class on code that should be highlighted.
2. **Parse gate** — a shell script (`scripts/check-snippets.sh`) extracts all `.deal`/`.dealx` fenced blocks from MDX files and runs `deal parse` on each. Intentional-error examples carry a fence metadata marker (e.g., ` ```deal error-expected `) that the script skips.

> The exact mechanism for the "Shiki scope gate" (how to detect unresolved scopes at build time) is not fully specified in Expressive Code docs found during research. The planner should define this as a Wave 4 spike: either a custom Expressive Code plugin, or a post-build script that greps the HTML output for unstyled code tokens.

---

## Runtime State Inventory

This phase is primarily additive (new DEAL sources, new Rust code, new sibling repo content). It is not a rename/refactor phase. The inventory is included to address sibling-repo state:

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | deal-stdlib/README.md lists `rf/`, `protocols/`, `standards/`, `patterns/` packages as planned | Update README to mark non-Phase-4 packages with "Phase 6 — deferred" per D-54 |
| Live service config | deal-lang.org sibling repo contains only `README.md` — no site yet | Astro initialization creates the site structure in Plan 04-0N |
| OS-registered state | None — no cron jobs, task scheduler entries, or OS services | None |
| Secrets/env vars | GitHub Pages deployment uses `GITHUB_TOKEN` from Actions environment | No change; user provides PAT if private repo |
| Build artifacts | `.deal/deps/` per-project vendored deps | These do not exist yet; `deal install` creates them at first run |

**Nothing found in category:** OS-registered state — verified by inspection of Phase 3 commits. Secrets — no new secrets required beyond existing VSCE_PAT and GITHUB_TOKEN already in Phase 3 pipeline.

---

## Common Pitfalls

### Pitfall 1: ReqIF XSD Imports driver.xsd — Offline Bundling Required

**What goes wrong:** `reqif.xsd` imports `http://www.omg.org/spec/ReqIF/20110402/driver.xsd` via `schemaLocation`. Any XSD validator that tries to fetch this URL at validation time will fail in offline environments (air-gapped defense programs).

**Why it happens:** The XSD import uses an HTTP URL by default; Rust's `quick-xml` is a reader/writer, not a schema validator that resolves imports.

**How to avoid:** Bundle both `reqif.xsd` and `driver.xsd` in `spec/references/omg-reqif/`. The XSD validation approach should use structural validation (check required elements are present, IDENTIFIER attributes are set) rather than full XSD 1.0 schema validation. For the hard gate, verify the XML is well-formed and matches the known element/attribute structure; document this as the validation scope.

**Warning signs:** Any validator that makes network calls; test in `--offline` or `--no-network` mode.

### Pitfall 2: Shiki v1.0 Removed path Property

**What goes wrong:** Old Shiki v0.14 tutorials show `{ path: './my-lang.json' }` in the langs array. In Shiki v1.0+ (current is 4.2.0), this throws an error.

**Why it happens:** Shiki v1.0 became environment-agnostic; no file system access.

**How to avoid:** Use `JSON.parse(readFileSync('./deal.tmLanguage.json', 'utf-8'))` in `astro.config.mjs`. The parsed object is passed directly. [CITED: shiki.style/guide/load-lang]

**Warning signs:** `TypeError: Cannot read properties of undefined` or `Unknown language` errors during `astro build`.

### Pitfall 3: No Mature Offline XSD 1.0 Validator in Rust

**What goes wrong:** Research found no battle-tested, fully offline Rust crate for XSD 1.0 schema validation. `xmlschema` on crates.io is at 0.0.1 (extremely new). `xsd-parser` is a code generator (Rust struct generation from XSD), not a runtime validator.

**Why it happens:** XSD 1.0 validation is a complex spec; Python's `lxml` and Java's JAXB are the mature implementations.

**How to avoid:** Use a two-layer validation strategy per D-58:
- **Layer 1 (Rust, hard gate):** `quick-xml` well-formedness check + structural assertion (required elements present, correct namespace, IDENTIFIER attributes non-empty). This is fast and reliable.
- **Layer 2 (Python smoke, soft gate):** The `reqif` Python library (strictdoc ecosystem) for round-trip validation. Run as part of the D-58 import smoke, not blocking CI.

Document this scope clearly in `04-VERIFICATION.md`.

**Warning signs:** CI failures on `spec/references/omg-reqif/reqif.xsd` being unreadable by any Rust crate.

### Pitfall 4: `deal install` Must Detect Missing `.deal/deps/` at check Time

**What goes wrong:** `deal check` invoked without prior `deal install` silently passes if the import resolution treats missing deps as "imported wildcard" (existing sema behavior for external imports).

**Why it happens:** Phase 2 sema's import resolution (E2400 band) treats `import deal.std.units.{kg}` as "whitelisted" because it's an external package name. Without Phase 4's resolver, external imports are accepted.

**How to avoid:** Phase 4 changes import resolution: if `deal.toml` has a `[dependencies]` entry for `deal-std` but `.deal/deps/deal-std/` does not exist, emit a clear diagnostic ("E2402: dependency 'deal-std' not resolved — run `deal install`") and exit 1. The diagnostic should suggest the exact command.

**Warning signs:** `deal check` passing on a project that hasn't run `deal install` yet.

### Pitfall 5: Sibling Repo Symlinking in phase-4-gate-fresh

**What goes wrong:** The `phase-4-gate-fresh` step runs in an ephemeral worktree where `../deal-stdlib` and `../deal-lang.org` do not exist. Gate steps that run `cd ../deal-stdlib && deal check packages/` will fail with "no such file or directory".

**Why it happens:** Phase 3 already encountered this for `tree-sitter-deal` and `vscode-deal` (commits 3c60cf6, 03696f1). The fix is the sibling-repo symlink materialization in `scripts/verify-fresh-worktree.sh`.

**How to avoid:** Phase 4's gate-fresh must extend the existing `verify-fresh-worktree.sh` sibling-symlink list to include `deal-stdlib` and `deal-lang.org`. The EXIT trap already cleans up symlinks. Both repos must be present at `$REPO_ROOT/..` for the worktree to use them (the gate does not mutate them — it only reads/runs tests).

**Warning signs:** Phase-4-gate-fresh failing with "cd: ../deal-stdlib: No such file or directory".

### Pitfall 6: deal.lock Must Not Be Generated Determinism-Unaware

**What goes wrong:** If `deal.lock` keys are emitted in map iteration order (non-deterministic in Rust `HashMap`), byte-comparison CI checks will flake.

**Why it happens:** D-18 alphabetical-key invariant applies to all generated artifacts. `HashMap` in Rust has randomized iteration order per process.

**How to avoid:** Use `BTreeMap` when assembling the lockfile, or sort `[[package]]` entries alphabetically by `name` before serializing. Add a determinism test (mirrors D-18 test pattern from Phase 1.5).

### Pitfall 7: docs site CNAME File Gets Overwritten by Astro Build

**What goes wrong:** If `CNAME` is placed in the repo root instead of `public/`, Astro's build step does not copy it to `dist/`, and GitHub Pages loses the custom domain on every deploy.

**Why it happens:** Astro only copies files from `public/` to `dist/` verbatim. [CITED: docs.astro.build/en/guides/deploy/github/]

**How to avoid:** `public/CNAME` contains `deal-lang.org`. Remove `base` from `astro.config.mjs` when using a custom domain (base is only needed for subdirectory deployments).

---

## Code Examples

### ReqIF Golden Fixture Pattern (mirrors Phase 2 SysML v2)

```
tests/golden/reqif/
├── 01-requirement-def.deal         # Minimal DEAL file with one requirement_def
├── 01-requirement-def.expected.reqif  # Hand-written expected ReqIF XML (unwrapped from .reqifz for readability)
├── 02-trace-relation.deal
├── 02-trace-relation.expected.reqif
└── ...
```

### deal install Git Clone Logic

```rust
// cli/src/resolver.rs
// Source: [ASSUMED] — models Cargo's git dependency resolution

use git2::{Repository, FetchOptions, RemoteCallbacks};
use std::path::Path;

pub struct GitDep {
    pub url: String,
    pub tag: Option<String>,
    pub rev: Option<String>,
    pub branch: Option<String>,
}

pub fn resolve_git_dep(dep: &GitDep, dest: &Path) -> anyhow::Result<String> {
    // Clone or fetch the repo
    let repo = if dest.exists() {
        Repository::open(dest)?
    } else {
        Repository::clone(&dep.url, dest)?
    };

    // Resolve to exact SHA
    let reference = dep.tag.as_deref()
        .map(|t| format!("refs/tags/{}", t))
        .or_else(|| dep.branch.as_deref().map(|b| format!("refs/heads/{}", b)))
        .unwrap_or_else(|| "HEAD".to_string());

    let obj = repo.revparse_single(&reference)?;
    let sha = obj.peel_to_commit()?.id().to_string();

    // Checkout at the resolved SHA
    repo.set_head_detached(obj.peel_to_commit()?.id())?;
    repo.checkout_head(None)?;

    Ok(sha)  // Returns SHA for deal.lock
}
```

### Snippet CI Gate Script

```bash
#!/usr/bin/env bash
# scripts/check-snippets.sh — run `deal parse` on every .deal/.dealx code block in docs
# Source: [ASSUMED] — implements D-65 parse gate

DEAL_BIN="${DEAL_BIN:-deal}"
ERRORS=0

# Extract fenced blocks that are NOT marked error-expected
grep -r '```deal' src/content/docs/ | while read -r line; do
    file="${line%%:*}"
    # Extract block content, skip error-expected blocks
    python3 scripts/extract_deal_blocks.py "$file" | while read -r snippet_file; do
        if ! "$DEAL_BIN" parse "$snippet_file" > /dev/null 2>&1; then
            echo "FAIL: $snippet_file (from $file) did not parse"
            ERRORS=$((ERRORS + 1))
        fi
    done
done

exit $ERRORS
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Shiki `path` property for grammar loading | Pass parsed JSON object | Shiki 1.0 (2024) | Must update any tutorial code that uses `path:` |
| Separate `@astrojs/github-pages` adapter | No adapter; `withastro/action@v6` handles deployment | Astro ~3.x | Simpler deploy — no adapter install |
| reqif.xsd fetched at runtime | Bundle offline in `spec/references/` | Phase 4 design decision | Offline/airgap friendly (matches existing pattern) |
| Semver range resolution for packages | Exact-ref lockfile only | D-68 (Phase 4) | Deterministic; appropriate for pre-registry phase |

**Deprecated/outdated:**
- `@astrojs/github-pages` package: Does not exist on npm — GitHub Pages deployment is adapter-free in current Astro [VERIFIED: npm registry lookup returned no result]
- Shiki v0.14 `path` property: Removed in v1.0 [CITED: shiki.style/guide/load-lang]

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Annotation syntax `@si_base_dimension[M:1, ...]` is valid under SD-15/SD-16 grammar | §Dimensional Algebra, Pattern 3 | Planner must verify against `spec/grammar/deal.ebnf` before authoring stdlib |
| A2 | Conversion call syntax `to_kg(lb(3300))` works under existing expression grammar (PS-6/SD-13) | §Dimensional Algebra Domain | If grammar doesn't support this form, stdlib authors cannot write conversions without a grammar extension |
| A3 | `driver.xsd` (XHTML namespace) is needed for XSD validation and must be bundled offline | §ReqIF Domain, Pitfall 1 | If not required, bundle can be simpler; verify against reqif.xsd content |
| A4 | `git2` crate works on all target platforms without requiring system libgit2 | §Package Resolution Domain | If libgit2 is not available on CI runners, need `--features vendored` flag in Cargo.toml |
| A5 | The "Shiki scope gate" for detecting unresolved tokens can be implemented as a post-build script | §Docs Site, Pitfall 2 | If Expressive Code doesn't expose unstyled token detection, an alternative CI gate mechanism is needed |
| A6 | deal-stdlib version `v0.4.0` (or similar) will be the first release tag | §Package Resolution, deal.toml scaffold | Planner decides the actual version tag; number is illustrative |
| A7 | `zip = "2"` (stable series) is the correct crate version, not `9.0.0-pre2` | §Standard Stack | Pre-release version would be unstable; verify stable series before adding to Cargo.toml |

---

## Open Questions (RESOLVED)

1. **XSD validation depth** — RESOLVED: Plan 06 treats structural validation via `quick-xml` (required elements, correct namespace, non-empty IDENTIFIER attributes) as the hard gate; the Python `reqif` library import is the soft smoke. Scope documented in Plan 06's tasks.
   - What we know: No mature offline XSD 1.0 validator exists in Rust; structural validation via `quick-xml` is feasible
   - What's unclear: Whether the D-58 acceptance criteria ("XSD validation is the hard gate") requires full XSD semantic validation or structural well-formedness is sufficient
   - Recommendation: Treat structural validation (required elements, correct namespace, non-empty IDENTIFIER attributes) as the hard gate; document scope in `04-VERIFICATION.md`; use Python `reqif` library for the soft smoke

2. **Shiki unknown-scope detection mechanism** — RESOLVED: Plan 08 implements a post-build HTML scan as the CI scope gate — any `.deal`/`.dealx` code block rendering with zero colored tokens (grammar not loaded) or placeholder content fails the build.
   - What we know: Expressive Code renders code blocks; TextMate grammars define scopes; Shiki tokenizes against them
   - What's unclear: How to programmatically detect "this token has no matching scope" at build time without reading Shiki's internal token objects
   - Recommendation: Plan a Wave 4 spike; fallback is a visual review CI step that flags any code block where `deal`/`dealx` fence doesn't apply any highlighting at all (zero colored tokens = grammar not loaded)

3. **deal-stdlib tagging/versioning** — RESOLVED: Plan 03 chooses the first stdlib tag (recommendation: `v0.4.0`, lockstep with the Phase-4 toolchain per D-52 precedent) and records it in 04-03-SUMMARY.md; Plan 04 wires that tag into `DEFAULT_STDLIB_TAG` in cli/src/main.rs.
   - What we know: D-52 established lockstep versioning precedent (vscode-deal ↔ deal-lsp); D-67 says `deal init` writes a real tag
   - What's unclear: Whether the first stdlib tag should be `v0.4.0` (matching phase), `v0.1.0` (semantic first release), or something else
   - Recommendation: Planner decision; the RESEARCH.md uses `v0.4.0` as illustrative placeholder only

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `zig` | Sema extension, phase gate | ✓ | 0.16.0 | — |
| `rustc` / `cargo` | CLI extensions (reqif, resolver) | ✓ | 1.93.0 | — |
| `node` / `npm` | Docs site (Astro) | ✓ | Node 26.0.0 / npm 11.12.1 | — |
| `git` | `deal install` (subprocess fallback) / CI | ✓ (assumed) | system | `git2` crate with `--features vendored` |
| `python3` | D-58 ReqIF import smoke (`reqif` library) | ✓ (macOS system) [ASSUMED] | 3.x | Document as "soft gate only" if unavailable |
| OMG ReqIF XSD | ReqIF emitter validation | ✓ (download required in Plan 04-01) | 1.2 (2016) | None — D-60 gates emitter work on acquisition |

**Missing dependencies with no fallback:**
- OMG ReqIF XSD — must be downloaded and committed to `spec/references/omg-reqif/` in Plan 04-01 before any ReqIF emitter work begins (D-60 hard constraint)

**Missing dependencies with fallback:**
- `git2` crate: if libgit2 system library is unavailable, use `features = ["vendored"]` to statically link
- Python `reqif` library: optional smoke only; XSD structural validation (Rust) is the hard gate

---

## Validation Architecture

> `workflow.nyquist_validation` key is absent from `.planning/config.json` — treating as enabled.

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Zig test runner (built-in) + `cargo test --workspace` |
| Config file | `build.zig` (Zig); `Cargo.toml` workspace (Rust) |
| Quick run command | `zig build test -Dtest-filter=sema && cargo test -p deal --lib` |
| Full suite command | `zig build test && cargo test --workspace` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| REQ-phase-4-1-stdlib | stdlib .deal files parse cleanly | unit | `zig build test -Dtest-filter=stdlib` | ❌ Wave 0 |
| REQ-phase-4-1-stdlib | dimensional check catches dim mismatch | unit | `zig build test -Dtest-filter=sema.dimensional` | ❌ Wave 0 |
| REQ-phase-4-1-stdlib | mixed-unit-same-dim without conversion = E25xx | unit | `zig build test -Dtest-filter=sema.dimensional` | ❌ Wave 0 |
| REQ-phase-4-2-reqif-codegen | ReqIF XML well-formed + structural check | unit | `cargo test -p deal --test golden_reqif` | ❌ Wave 0 |
| REQ-phase-4-2-reqif-codegen | .reqifz archive is valid zip | unit | `cargo test -p deal --test reqif_archive` | ❌ Wave 0 |
| REQ-phase-4-3-docs-site | `astro build` exits 0 | smoke | `cd ../deal-lang.org && npm run build` | ❌ Wave 0 |
| REQ-phase-4-3-docs-site | All .deal snippets parse | integration | `../deal-lang.org/scripts/check-snippets.sh` | ❌ Wave 0 |
| REQ-phase-4-4-package-resolution | `deal install` clones git dep + writes lockfile | integration | `cargo test -p deal --test resolver` | ❌ Wave 0 |
| REQ-phase-4-4-package-resolution | `deal install` + `deal check` exits 0 on scaffold | e2e | `zig build phase-4-gate` step 1 | ❌ Wave 0 |
| REQ-phase-4-gate | `deal init` → `deal install` → `deal check` → `deal build` exits 0 | e2e | `zig build phase-4-gate` | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `zig build test -Dtest-filter=<area> && cargo test -p deal --lib`
- **Per wave merge:** `zig build test && cargo test --workspace`
- **Phase gate:** `zig build phase-4-gate && zig build phase-4-gate-fresh`

### Wave 0 Gaps

- [ ] `tests/unit/sema_dimensional.zig` — dimensional algebra unit tests (REQ-4-1)
- [ ] `tests/regressions/sema/07-dimensional.deal` — dimensional check regression fixtures
- [ ] `tests/golden/reqif/` directory + at least 2 fixture pairs (REQ-4-2)
- [ ] `tests/integration/resolver_test.rs` — `deal install` integration test (REQ-4-4)
- [ ] `../deal-lang.org/scripts/check-snippets.sh` — docs snippet CI gate (REQ-4-3)

*(No existing test infrastructure covers Phase 4 requirements — all new)*

---

## Security Domain

> `security_enforcement` absent from config.json — treating as enabled.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | n/a (local CLI, no auth) |
| V3 Session Management | no | n/a (CLI, stateless) |
| V4 Access Control | no | n/a (no multi-user) |
| V5 Input Validation | yes | Validate git URLs before cloning; validate `deal.toml` before executing |
| V6 Cryptography | yes | SHA256 for lockfile integrity + XSD bundle integrity |

### Known Threat Patterns for This Stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Path traversal via `deal.toml` path deps (e.g., `path = "../../../../etc/passwd"`) | Tampering | Canonicalize paths; assert they remain within project boundary (mirrors T-02-11 in sema.zig) |
| Malicious git URL in `deal.toml` | Spoofing/Tampering | Validate URL scheme (only `https://` and `git@`); `git2` handles transport security |
| Zip slip in `.reqifz` extraction (future `deal import --from reqif`) | Tampering | Not in Phase 4 scope; note for Phase 6 import pipeline |
| SHA256 mismatch in XSD bundle | Tampering | Same T-02-21 pattern as `schema_registry.rs`; verify at startup |
| Docs CI running arbitrary `deal parse` on user-supplied snippet content | Tampering | `deal parse` is a local binary reading files; no network; bounded by grammar |
| Supply chain: npm packages in docs site | Tampering | Package.json lock committed; `npm ci` in CI (uses lockfile) |

---

## Sources

### Primary (HIGH confidence)

- Direct HTTP probe of `https://www.omg.org/spec/ReqIF/20110401/reqif.xsd` — existence and content verified 2026-06-05
- `/Users/dunnock/projects/deal-lang/deal/cli/src/main.rs` — existing CLI subcommand structure
- `/Users/dunnock/projects/deal-lang/deal/cli/src/sysml_v2.rs` — existing emitter pattern (ReqIF mirrors this)
- `/Users/dunnock/projects/deal-lang/deal/cli/src/schema_registry.rs` — SHA256 verification pattern
- `/Users/dunnock/projects/deal-lang/deal/src/diagnostics.zig` — E-code bands (E0001..E2499)
- `/Users/dunnock/projects/deal-lang/deal/Cargo.toml` — workspace dependencies (toml 0.8 confirmed)
- `/Users/dunnock/projects/deal-lang/deal/.planning/phases/04-ecosystem/04-CONTEXT.md` — 16 locked decisions D-54..D-69
- `cargo search` results: `git2=0.21.0`, `quick-xml=0.40.1`, `zip=9.0.0-pre2` (stable is 2.x), `toml_edit=0.25.12`
- `npm view` results: `astro=6.4.4`, `@astrojs/starlight=0.39.3`, `shiki=4.2.0`

### Secondary (MEDIUM confidence)

- [shiki.style/guide/load-lang](https://shiki.style/guide/load-lang) — custom language loading, `path` property removal in v1.0
- [docs.astro.build/en/guides/deploy/github/](https://docs.astro.build/en/guides/deploy/github/) — GH Pages deployment, CNAME, no adapter needed
- [expressive-code.com/key-features/syntax-highlighting/](https://expressive-code.com/key-features/syntax-highlighting/) — `shiki.langs` configuration
- [www.omg.org/spec/ReqIF/1.2/](https://www.omg.org/spec/ReqIF/1.2/) — ReqIF 1.2 specification page
- [www.bipm.org/en/publications/si-brochure](https://www.bipm.org/en/publications/si-brochure) — SI Brochure 9th edition 2019 (base unit definitions)

### Tertiary (LOW confidence — marked [ASSUMED] inline)

- ReqIF emitter code structure (Rust patterns for `reqif.rs`) — modeled on existing `sysml_v2.rs`
- Annotation syntax for dimensional metadata in DEAL stdlib
- `zip = "2"` stable series vs `9.0.0-pre2` pre-release distinction
- Python `reqif` library availability for D-58 smoke

---

## Metadata

**Confidence breakdown:**
- Standard stack (npm/cargo): HIGH — verified via registry lookups + slopcheck
- ReqIF XSD availability: HIGH — direct HTTP probe confirmed
- Architecture patterns: HIGH — derived from locked decisions + existing code
- Dimensional algebra design: MEDIUM — locked decisions are clear; exact annotation syntax is Claude's Discretion
- XSD validation approach: MEDIUM — no mature Rust XSD 1.0 validator confirmed; two-layer strategy recommended
- Docs site (Astro/Starlight): HIGH — official docs + npm registry verified
- Package resolution: HIGH — `toml` and `git2` crates are established; lockfile format is Claude's Discretion

**Research date:** 2026-06-05
**Valid until:** 2026-07-05 (30 days — stable ecosystem; Astro/Starlight minor versions move faster, verify before starting Wave 4)
