# Phase 4: Ecosystem - Pattern Map

**Mapped:** 2026-06-05
**Files analyzed:** 22 new/modified files across 4 streams
**Analogs found:** 18 / 22

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `cli/src/reqif.rs` | service | transform (IR JSON → XML) | `cli/src/sysml_v2.rs` | exact |
| `cli/src/reqif_schema.rs` | service | file-I/O + validation | `cli/src/schema_registry.rs` | exact |
| `cli/src/resolver.rs` | service | file-I/O + CRUD | `cli/src/main.rs` (TOML parse section) | role-match |
| `cli/src/main.rs` (extended) | controller | request-response | `cli/src/main.rs` | exact (modify) |
| `src/sema.zig` (extended) | service | transform + validation | `src/sema.zig` | exact (modify) |
| `src/diagnostics.zig` (extended) | config | — | `src/diagnostics.zig` | exact (modify) |
| `build.zig` (extended) | config | — | `build.zig` (phase-3-gate block) | exact (modify) |
| `scripts/verify-fresh-worktree.sh` (extended) | config | — | `scripts/verify-fresh-worktree.sh` | exact (modify) |
| `spec/references/omg-reqif/reqif.xsd` | config | file-I/O | `spec/references/omg-sysml-v2/SysML.json` | role-match |
| `spec/references/omg-reqif/SHA256SUMS` | config | — | `spec/references/omg-sysml-v2/` manifest | role-match |
| `tests/golden/reqif/*.deal` + `*.expected.reqif` | test | batch | `tests/golden/sysml-v2/*.deal` + `*.expected.json` | exact |
| `tests/unit/sema_dimensional.zig` | test | batch | `tests/unit/sema_corpus.zig` | exact |
| `tests/regressions/sema/07-dimensional.deal` | test | — | `tests/regressions/sema/06-import.deal` | exact |
| `deal-stdlib/deal.toml` | config | — | `spec/examples/showcase/deal.toml` | role-match |
| `deal-stdlib/packages/units/dimensions.deal` | config | — | showcase `packages/requirements/system.deal` (annotation syntax) | partial |
| `deal-stdlib/packages/units/si.deal` | config | — | showcase `packages/requirements/system.deal` | partial |
| `deal-stdlib/packages/units/imperial.deal` | config | — | showcase `packages/requirements/system.deal` | partial |
| `deal-stdlib/packages/interfaces/electrical/*.deal` | config | — | showcase `packages/interfaces/electrical.deal` | partial |
| `deal-lang.org/astro.config.mjs` | config | — | (no analog — new Astro/Starlight) | none |
| `deal-lang.org/src/content/docs/**/*.mdx` | config | — | (no analog — new docs content) | none |
| `deal-lang.org/.github/workflows/deploy.yml` | config | — | (no analog — new GH Actions workflow) | none |
| `deal-lang.org/scripts/check-snippets.sh` | utility | batch | `scripts/verify-fresh-worktree.sh` (bash + deal binary invocation) | partial |

---

## Pattern Assignments

### `cli/src/reqif.rs` (service, transform)

**Analog:** `cli/src/sysml_v2.rs`

**Imports pattern** (lines 1–20):
```rust
//! ReqIF XML emitter.
//!
//! Consumes a DEAL IR v0 JSON document (produced by `deal_ir_json` C ABI)
//! and emits a ReqIF 1.2 XML document per D-59 + D-61.
//!
//! Key design decisions:
//!   D-19: Rust CLI crate owns this emitter; walks IR via serde_json::Value.
//!   D-59: Only requirement_def/need_def/trace/verification nodes are emitted.
//!   D-61: Output is .reqifz zip archive.
//!   D-18: Alphabetical key order — BTreeMap for any map structures.

use anyhow::{anyhow, Context as _};
use serde_json::Value;
use uuid::Uuid;
// NEW deps vs sysml_v2.rs:
use quick_xml::{Writer, events::{BytesDecl, BytesStart, BytesText, BytesEnd, Event}};
use zip::write::{FileOptions, ZipWriter};
use std::io::{Cursor, Write};
```

**UUID namespace pattern** — copy directly from `cli/src/sysml_v2.rs` lines 29–38:
```rust
const REQIF_NAMESPACE: Uuid = Uuid::from_u128(/* new unique constant — different from SYSML_NAMESPACE */);

pub fn deal_id_to_reqif_id(qualified_path: &str) -> String {
    // ReqIF identifiers must be valid XML IDs (no dots).
    // Strategy: replace '.' with '_' and prefix with 'DEAL_'
    format!("DEAL_{}", qualified_path.replace('.', "_"))
}
```

**IR node structs pattern** — copy from `cli/src/sysml_v2.rs` lines 57–74:
```rust
#[derive(Debug)]
struct IrNode<'a> {
    id: &'a str,
    kind: &'a str,
    payload: &'a Value,
}

#[derive(Debug)]
struct IrEdge<'a> {
    src: &'a str,
    dst: &'a str,
    kind: &'a str,
}
```

**Top-level emitter pattern** — copy from `cli/src/sysml_v2.rs` lines 84–96 (IR envelope validation + node parse loop), then filter to `requirement_def`/`need_def` only (D-59):
```rust
pub fn emit(ir_json: &Value) -> anyhow::Result<Vec<u8>> {
    let elements_map = ir_json
        .get("elements")
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow!("IR JSON missing 'elements' object"))?;

    let edges_arr = ir_json
        .get("edges")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("IR JSON missing 'edges' array"))?;
    // ... parse edges into typed structs (same as sysml_v2.rs lines 97-119)
    // ... filter nodes: only requirement_def, need_def (D-59)
    // ... emit ReqIF XML via quick_xml Writer
    // ... wrap in .reqifz zip (D-61)
}
```

**`emit_from_bytes` wrapper** — copy signature from `cli/src/sysml_v2.rs` lines 576–580:
```rust
pub fn emit_from_bytes(ir_json_bytes: &[u8], output_path: &std::path::Path) -> anyhow::Result<()> {
    let ir_value: Value = serde_json::from_slice(ir_json_bytes)
        .with_context(|| "failed to parse IR JSON from deal_ir_json output")?;
    let xml_bytes = emit(&ir_value)?;
    // zip into .reqifz, write to output_path
    Ok(())
}
```

**Alphabetical / deterministic output** — copy from `cli/src/sysml_v2.rs` lines 121–123:
```rust
// Sort nodes by id for deterministic output (D-18).
nodes.sort_by_key(|n| n.id);
```

**Unit tests pattern** — copy test structure from `cli/src/sysml_v2.rs` lines 583–718:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reqif_id_is_deterministic() { /* mirrors uuid_is_deterministic() */ }

    #[test]
    fn emit_minimal_requirement_ir() { /* mirrors emit_minimal_ir() */ }

    #[test]
    fn emit_reqifz_is_valid_zip() { /* new — verify archive is openable by zip crate */ }
}
```

---

### `cli/src/reqif_schema.rs` (service, file-I/O + validation)

**Analog:** `cli/src/schema_registry.rs`

**Full structure to mirror** — `cli/src/schema_registry.rs` is the exact analog. Key patterns:

**SHA256 pin constants** (lines 26–31):
```rust
const EXPECTED_REQIF_XSD_SHA256: &str = "..."; // filled once XSD is acquired (D-60)
const EXPECTED_DRIVER_XSD_SHA256: &str = "...";
```

**Offline bundle loading + SHA256 verification** (lines 52–102):
```rust
pub fn load_reqif_schemas(bundle_dir: &Path) -> anyhow::Result<ReqifSchemas> {
    let bundle_dir = bundle_dir.canonicalize()
        .with_context(|| format!("cannot canonicalize bundle_dir {}", bundle_dir.display()))?;
    // assert_within guard (lines 67-76) — copy exactly
    let reqif_bytes = std::fs::read(&reqif_xsd_path)...;
    verify_sha256(&reqif_bytes, EXPECTED_REQIF_XSD_SHA256, "reqif.xsd")?;
    // ...
}
```

**`verify_sha256` helper** (lines 121–136) — copy verbatim from `schema_registry.rs`:
```rust
fn verify_sha256(bytes: &[u8], expected_hex: &str, label: &str) -> anyhow::Result<()> {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let actual_hex = hex::encode(digest);
    if actual_hex != expected_hex {
        return Err(anyhow!(
            "Schema bundle tamper-detection failure ({label}): \
             expected SHA256 {expected_hex}, got {actual_hex}. ..."
        ));
    }
    Ok(())
}
```

**Schema directory location helper** (lines 250–296) — copy `locate_schemas_dir()` pattern, adapting env var name to `DEAL_REQIF_SCHEMAS_DIR` and path to `spec/references/omg-reqif/`:
```rust
pub fn locate_reqif_schemas_dir() -> anyhow::Result<std::path::PathBuf> {
    if let Ok(dir) = std::env::var("DEAL_REQIF_SCHEMAS_DIR") { ... }
    // Walk up from CARGO_MANIFEST_DIR looking for spec/references/omg-reqif/
    // Walk up from current_exe()
}
```

**Note on XSD validation:** `schema_registry.rs` uses `jsonschema` for JSON Schema. ReqIF uses XSD; no mature offline Rust XSD 1.0 validator exists (RESEARCH Pitfall 3). Use `quick-xml` for structural validation (well-formedness + required element/attribute presence) as the hard gate. Document scope in comments following the `schema_registry.rs` documentation style.

---

### `cli/src/resolver.rs` (service, file-I/O + CRUD)

**Analog:** `cli/src/main.rs` (TOML parsing sections at lines 142–165, 319–365)

No exact existing analog for a git resolver, but patterns to extract:

**deal.toml parsing pattern** — from `cli/src/main.rs` workspace index section (lines 319–362), plus the `toml` crate is already in workspace:
```rust
// cli/src/resolver.rs
use std::collections::BTreeMap;  // D-18: deterministic output
use anyhow::{anyhow, Context as _};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct DealToml {
    pub project: ProjectSection,
    #[serde(default)]
    pub workspace: WorkspaceSection,
    #[serde(default)]
    pub dependencies: BTreeMap<String, Dependency>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Dependency {
    Git { git: String, tag: Option<String>, rev: Option<String>, branch: Option<String> },
    Path { path: String },
}
```

**`deal.lock` serialization — BTreeMap for D-18 determinism:**
```rust
// Use BTreeMap (not HashMap) for alphabetical key order (D-18, Pitfall 6 mitigation)
#[derive(Debug, Serialize, Deserialize)]
pub struct LockFile {
    pub version: u32,
    pub package: Vec<LockedPackage>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct LockedPackage {
    // Alphabetical field order for D-18:
    pub git: Option<String>,
    pub name: String,
    pub path: Option<String>,
    pub rev: Option<String>,
    pub tag: Option<String>,
}
```

**Error handling pattern** — from `cli/src/main.rs` lines 172–173 / 189–193:
```rust
let source_bytes = std::fs::read(path).map_err(|e| {
    CliError::Internal(anyhow::anyhow!("cannot read {:?}: {}", path, e))
})?;
```

**Path traversal guard** — from `cli/src/schema_registry.rs` lines 67–76:
```rust
fn assert_within(path: &std::path::Path, base: &std::path::Path) -> anyhow::Result<()> {
    if !path.starts_with(base) {
        return Err(anyhow!("path traversal guard: {} is not inside {}", path.display(), base.display()));
    }
    Ok(())
}
```

---

### `cli/src/main.rs` (extended — controller)

**Analog:** `cli/src/main.rs` itself

**New subcommands to add** — following `Command` enum pattern (lines 73–108):
```rust
#[derive(Subcommand)]
enum Command {
    // ... existing Parse, Check, Fmt, Build ...

    /// Scaffold a new DEAL project with a working starter model (D-69).
    Init {
        /// Project name (default: current directory name).
        name: Option<String>,
    },
    /// Resolve and vendor dependencies into .deal/deps/ (D-66).
    Install,
}

// Extend BuildTarget enum (line 110-113):
#[derive(ValueEnum, Clone)]
enum BuildTarget {
    SysmlV2,
    Reqif,  // NEW (D-61)
}
```

**New subcommand dispatch** — following `run()` match pattern (lines 1026–1037):
```rust
fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        // ... existing arms ...
        Command::Init { name } => run_init(name.as_deref(), cli.json, cli.color),
        Command::Install => run_install(cli.json, cli.color),
        Command::Build { target: BuildTarget::Reqif, validate, output, paths } => {
            run_build_reqif(&paths, validate, cli.json, cli.color, output.as_deref())
        }
        // ... existing SysmlV2 arm ...
    }
}
```

**`run_build_reqif` function** — mirrors `run_build` (lines 814–975) with:
- Same FFI call pattern (parse → IR JSON clone before free)
- Call `reqif::emit_from_bytes()` instead of `sysml_v2::emit_from_bytes()`
- Output path inferred to `build/reqif/workspace.reqifz` (mirrors sysml-v2 path inference)

---

### `src/sema.zig` (extended — service, transform + validation)

**Analog:** `src/sema.zig` itself

**Module header / check enumeration comment** — extend the existing block (lines 1–15):
```zig
//!   7. Dimensional algebra — unit expression type consistency
//!      (E2500..E2599 dimensional/unit algebra band per D-33 extension)
```

**DimVector type** — new type, add near top of sema.zig after existing type declarations:
```zig
/// SI dimension exponent vector: [M, L, T, I, Θ, N, J] (D-56)
/// Index mapping: 0=Mass, 1=Length, 2=Time, 3=Current, 4=Temp, 5=Amount, 6=Luminosity
pub const DimVector = [7]i8;

pub const DIM_DIMENSIONLESS: DimVector = .{ 0, 0, 0, 0, 0, 0, 0 };
pub const DIM_MASS:          DimVector = .{ 1, 0, 0, 0, 0, 0, 0 };
pub const DIM_LENGTH:        DimVector = .{ 0, 1, 0, 0, 0, 0, 0 };
pub const DIM_TIME:          DimVector = .{ 0, 0, 1, 0, 0, 0, 0 };
// ... etc.
```

**New sema check function** — modeled on existing check functions in `src/sema.zig`. Existing checks use the `DiagnosticCollector` pattern (`dc.emitFmt(...)`) from `src/diagnostics.zig` lines 174+:
```zig
/// Check #7: dimensional algebra check for unit expressions (D-55/D-56)
/// Reads @si_base_dimension / @si_unit annotations from stdlib declarations.
/// Emits E2500..E2503 on mismatch.
fn checkDimensionalExpr(
    sema: *Sema,
    dc: *DiagnosticCollector,
    expr: *const ast.Node,
    expected_dim: DimVector,
) !void {
    // resolve unit literal → look up DimVector from symbol table annotation
    // compare with expected_dim
    // dc.emitFmt(Codes.e_dimension_mismatch, span, "...", .{});
}
```

**SymbolKind extension** — add to `SymbolKind` enum (lines 56–74):
```zig
pub const SymbolKind = enum {
    // ... existing kinds ...
    dimension_def,    // @si_base_dimension annotated attribute def
    unit_def,         // @si_unit annotated attribute def
};
```

---

### `src/diagnostics.zig` (extended — config)

**Analog:** `src/diagnostics.zig` itself

**New E-code band documentation** — extend the namespace comment at lines 8–19:
```zig
//!   E2500..E2599  semantic — dimensional algebra / unit checks (Phase 4 D-33 extension)
```

**New code constants** — add to `Codes` struct after line 148:
```zig
    // E2500..E2599 — Check #7 dimensional algebra / unit checks (Phase 4 D-33)
    // tests/regressions/sema/07-dimensional.deal
    /// Check #7 — dimension mismatch (e.g., mass : Mass = V(800))
    pub const e_dimension_mismatch = "E2500";
    /// Check #7 — mixed-unit same-dimension without explicit conversion (D-57)
    pub const e_mixed_unit_comparison = "E2501";
    /// Check #7 — unit literal not resolvable to a declared dimension
    pub const e_unknown_unit = "E2502";
    /// Check #7 — conversion call type mismatch (wrong source or target dimension)
    pub const e_conversion_type_mismatch = "E2503";
    /// Check #7 — missing dependency: [dependencies] entry not resolved
    pub const e_dependency_not_resolved = "E2402";
```

---

### `build.zig` (extended — phase-4-gate + phase-4-gate-fresh)

**Analog:** `build.zig` lines 322–407 (phase-3-gate + phase-3-gate-fresh)

**Gate step declaration pattern** (lines 322–323):
```zig
const gate_4_step = b.step("phase-4-gate", "Run the full Phase 4 exit-gate suite (Zig + cargo + stdlib + reqif + docs + init/install E2E)");
gate_4_step.dependOn(gate_3_step); // inherit all prior gates (D-35 cumulative)
```

**addSystemCommand pattern for each gate sub-step** (lines 324–386):
```zig
// (N) cargo test --workspace (includes reqif + resolver integration tests)
const gate_4_cargo_test = b.addSystemCommand(&[_][]const u8{
    "cargo", "test", "--workspace", "--manifest-path", "Cargo.toml",
});
gate_4_step.dependOn(&gate_4_cargo_test.step);

// (N+1) deal-stdlib check (cd ../deal-stdlib && deal check packages/)
const gate_4_stdlib_check = b.addSystemCommand(&[_][]const u8{
    "bash", "-c", "cd ../deal-stdlib && cargo run --manifest-path ../deal/Cargo.toml -p deal -- check packages/",
});
gate_4_step.dependOn(&gate_4_stdlib_check.step);

// (N+2) deal init + install + check + build smoke (D-69 acceptance criterion)
const gate_4_init_smoke = b.addSystemCommand(&[_][]const u8{
    "bash", "scripts/phase-4-smoke.sh",
});
gate_4_step.dependOn(&gate_4_init_smoke.step);

// (N+3) docs build (cd ../deal-lang.org && npm ci && npm run build)
const gate_4_docs_build = b.addSystemCommand(&[_][]const u8{
    "bash", "-c", "cd ../deal-lang.org && npm ci && npm run build",
});
gate_4_step.dependOn(&gate_4_docs_build.step);

// (N+4) docs snippet gate (cd ../deal-lang.org && bash scripts/check-snippets.sh)
const gate_4_docs_snippets = b.addSystemCommand(&[_][]const u8{
    "bash", "-c", "cd ../deal-lang.org && bash scripts/check-snippets.sh",
});
gate_4_step.dependOn(&gate_4_docs_snippets.step);
```

**Fresh-worktree step pattern** (lines 398–407) — copy exactly, change `"phase-3-gate"` to `"phase-4-gate"`:
```zig
const gate_4_fresh_step = b.step(
    "phase-4-gate-fresh",
    "Run phase-4-gate inside a freshly-created ephemeral worktree with submodule init (per ADR-phase-1.5-fresh-worktree-verification)",
);
const run_fresh_gate_4 = b.addSystemCommand(&[_][]const u8{
    "bash", "scripts/verify-fresh-worktree.sh",
    "phase-4-gate",
});
gate_4_fresh_step.dependOn(&run_fresh_gate_4.step);
```

---

### `scripts/verify-fresh-worktree.sh` (extended)

**Analog:** `scripts/verify-fresh-worktree.sh` lines 163–177

**Sibling symlink extension** — extend the sibling loop at lines 165–172:
```bash
for sibling in tree-sitter-deal vscode-deal deal-stdlib deal-lang.org; do
  if [ -d "$DEAL_PARENT/$sibling" ]; then
    ln -sfn "$DEAL_PARENT/$sibling" "$WORKTREE_PARENT/$sibling"
    echo "verify-fresh-worktree: linked $sibling into $WORKTREE_PARENT"
  else
    echo "verify-fresh-worktree: WARNING — sibling repo $sibling not found at $DEAL_PARENT/$sibling" >&2
    echo "  Cross-repo gate steps that reference ../$sibling will fail." >&2
  fi
done
```

**EXIT trap extension** — extend the trap at line 177:
```bash
trap '...cleanup...; for s in tree-sitter-deal vscode-deal deal-stdlib deal-lang.org; do [ -L "$WORKTREE_PARENT/$s" ] && rm -f "$WORKTREE_PARENT/$s"; done' EXIT
```

---

### `tests/golden/reqif/` (test, batch)

**Analog:** `tests/golden/sysml-v2/` directory

**File naming pattern** — copy from `tests/golden/sysml-v2/`:
```
tests/golden/reqif/
├── 01-requirement-def.deal           # single requirement_def input
├── 01-requirement-def.expected.reqif  # hand-written ReqIF XML (NOT .reqifz — unwrapped for readability)
├── 02-trace-relation.deal
├── 02-trace-relation.expected.reqif
├── 03-verification-attrs.deal
├── 03-verification-attrs.expected.reqif
└── README.md
```

The `.expected.reqif` files contain raw ReqIF XML (unwrapped from zip) for diff readability; the test runner compares the extracted XML from the emitted `.reqifz` against the fixture.

---

### `tests/unit/sema_dimensional.zig` (test, batch)

**Analog:** `tests/unit/sema_corpus.zig`

**File structure to copy** — from `tests/unit/sema_corpus.zig` lines 1–49:
```zig
//! sema.dimensional — Phase 4 dimensional algebra regression tests.
//!
//! Two test loops:
//!   1. sema.dimensional.regression — For each fixture in tests/regressions/sema/07-*.deal,
//!      parse + analyze and assert at least one E25xx diagnostic.
//!   2. sema.dimensional.showcase_clean — For all 19 showcase files, assert ZERO
//!      dimensional diagnostics (showcase uses units correctly).

const std = @import("std");
const lib = @import("lib");
const diagnostics_mod = @import("diagnostics");

const Codes = diagnostics_mod.Codes;

const Pin = struct {
    name: []const u8,
    expected_code: []const u8,
};

const regression_pins = [_]Pin{
    .{ .name = "07-dimensional-mismatch.deal", .expected_code = Codes.e_dimension_mismatch },
    .{ .name = "07-mixed-unit.deal", .expected_code = Codes.e_mixed_unit_comparison },
    .{ .name = "07-unknown-unit.deal", .expected_code = Codes.e_unknown_unit },
};
```

---

### `deal-stdlib/deal.toml` (config)

**Analog:** `spec/examples/showcase/deal.toml` (lines 1–28)

**Pattern to follow:**
```toml
[project]
name = "deal-std"
version = "0.4.0"     # planner chooses exact version per open question OQ-3
schema = "deal/0.1"
description = "DEAL standard library — units and interfaces"

[workspace]
packages = ["packages/*"]
```

No `[dependencies]` section — deal-std has no dependencies. No `[build.targets]` — stdlib is checked but not built to SysML v2 / ReqIF.

---

### `deal-stdlib/packages/units/dimensions.deal` and `si.deal` and `imperial.deal` (config, DEAL source)

**Analog:** `spec/examples/showcase/packages/requirements/system.deal` — the annotation pattern and `attribute def` style.

**File header pattern** — from showcase system.deal lines 1–11:
```deal
@header {
    path:       packages/units/dimensions.deal
    schema:     deal/0.1
    created:    ... by "..."
    status:     release
    marking:    Unclassified
}
```

**Annotation syntax for dimension metadata** — uses SD-15/SD-16 annotation syntax (same grammar as `@header`, `@confidence`, etc. in showcase). The `@si_base_dimension` and `@si_unit` annotation names are Claude's Discretion per D-56:
```deal
package deal.std.units;

// Base dimension declarations — these carry the 7-exponent SI vector
// that the Zig sema reads via the @si_base_dimension annotation.
@si_base_dimension[M:1, L:0, T:0, I:0, θ:0, N:0, J:0]
attribute def Mass;

@si_base_dimension[M:0, L:1, T:0, I:0, θ:0, N:0, J:0]
attribute def Length;

// Unit declarations — @si_unit links a unit to its base dimension
// and records the conversion factor to the SI base unit.
@si_unit[dim: "Mass", factor: 1.0]
attribute def kg : Mass;

@si_unit[dim: "Mass", factor: 0.001]
attribute def g : Mass;
```

**Explicit conversion call form** (D-57) — function-call syntax per PS-6/SD-13:
```deal
// In imperial.deal: conversion call forms for D-57
@si_unit[dim: "Mass", factor: 0.453592]
attribute def lb : Mass;

// Conversion function (usage: to_kg(lb(3300)))
// Exact attribute def syntax for callable conversions is planner's call
attribute def to_kg : Mass;  // or similar — planner to finalize
```

**NOTE:** Assumption A1 from RESEARCH must be verified — that `@si_base_dimension[M:1, ...]` is valid under the existing SD-15/SD-16 annotation grammar before authoring. Check `spec/grammar/deal.ebnf` annotation production.

---

### `deal-stdlib/packages/interfaces/electrical/*.deal` (config, DEAL source)

**Analog:** `spec/examples/showcase/packages/interfaces/electrical.deal`

**Pattern to follow** — structure an interface as `interface def` with `port def` attributes:
```deal
package deal.std.interfaces.electrical;

/// RJ45 Ethernet connector interface.
interface def RJ45 {
    port def TX : Signal [1];
    port def RX : Signal [1];
    // ... etc.
}
```

---

### `deal-lang.org/astro.config.mjs` (config — no analog)

**Source:** RESEARCH.md Pattern 1 (lines 332–364) is the authoritative pattern. No existing analog in the codebase.

**Pattern from RESEARCH.md:**
```javascript
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import { readFileSync } from 'fs';
import { fileURLToPath } from 'url';
import path from 'path';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const dealGrammar = JSON.parse(
  readFileSync(path.join(__dirname, 'src/grammars/deal.tmLanguage.json'), 'utf-8')
);
const dealxGrammar = JSON.parse(
  readFileSync(path.join(__dirname, 'src/grammars/dealx.tmLanguage.json'), 'utf-8')
);

export default defineConfig({
  site: 'https://deal-lang.org',   // D-64
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

**Anti-pattern:** Do NOT use `{ path: './deal.tmLanguage.json' }` — removed in Shiki v1.0 (RESEARCH Pitfall 2).

---

### `deal-lang.org/scripts/check-snippets.sh` (utility, batch — partial analog)

**Analog:** `scripts/verify-fresh-worktree.sh` — bash pattern for invoking the `deal` binary.

**Binary invocation pattern** from `scripts/verify-fresh-worktree.sh` lines 183–185:
```bash
( cd "$WORKTREE_DIR" && zig build "$GATE_STEP" ) 2>&1 | tee "$LOG_PATH" || GATE_EXIT=${PIPESTATUS[0]}
```

**Adapted for snippet check** — from RESEARCH.md §Code Examples (lines 921–943):
```bash
#!/usr/bin/env bash
# scripts/check-snippets.sh — D-65: run `deal parse` on every .deal/.dealx block
DEAL_BIN="${DEAL_BIN:-deal}"
ERRORS=0

# Skip blocks with error-expected fence marker (intentional-error examples)
# Mechanism: look for ```deal error-expected fence attribute

for mdx_file in src/content/docs/**/*.mdx; do
    python3 scripts/extract_deal_blocks.py "$mdx_file" | while read -r snippet_file; do
        if ! "$DEAL_BIN" parse "$snippet_file" > /dev/null 2>&1; then
            echo "FAIL: $snippet_file (from $mdx_file) did not parse"
            ERRORS=$((ERRORS + 1))
        fi
    done
done

exit $ERRORS
```

---

### `deal-lang.org/.github/workflows/deploy.yml` (config — no analog in main repo)

**Source:** RESEARCH.md Pattern 4 (lines 434–474) is the authoritative pattern. No analog exists in the main `deal/` repo (the `release.yml` is a different pattern — cross-compilation, not static site deploy).

---

## Shared Patterns

### FFI Call + Arena Clone-Before-Free

**Source:** `cli/src/main.rs` lines 196–232 (run_check), repeated in run_parse, run_build
**Apply to:** `run_build_reqif` in extended `main.rs`

```rust
// T-02-13 mitigation (Pitfall 3): clone IR JSON bytes BEFORE calling deal_free.
let ir_json_owned: Vec<u8>;
unsafe {
    let mut out_ptr: *const u8 = std::ptr::null();
    let mut out_len: usize = 0;
    let ok = ffi::deal_ir_json(handle, &mut out_ptr, &mut out_len);
    if !ok {
        ffi::deal_free(handle);
        return Err(CliError::Internal(anyhow::anyhow!("deal_ir_json failed for {:?}", path)));
    }
    ir_json_owned = std::slice::from_raw_parts(out_ptr, out_len).to_vec();
    ffi::deal_free(handle);   // pointer now invalid — ir_json_owned is the only live copy
}
```

### CliError Propagation

**Source:** `cli/src/main.rs` lines 27–51
**Apply to:** All new `run_*` functions in `main.rs`

```rust
fn run_install(...) -> Result<(), CliError> {
    // I/O failures → CliError::Internal(anyhow::anyhow!(...))
    // User-facing errors → CliError::User("...".to_string())
    // Use map_err(|e| CliError::Internal(anyhow::anyhow!("...: {}", e)))?
}
```

### SHA256 Verification

**Source:** `cli/src/schema_registry.rs` lines 121–136
**Apply to:** `cli/src/reqif_schema.rs` (XSD bundle verification)

```rust
fn verify_sha256(bytes: &[u8], expected_hex: &str, label: &str) -> anyhow::Result<()> {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let actual_hex = hex::encode(digest);
    if actual_hex != expected_hex {
        return Err(anyhow!("Schema bundle tamper-detection failure ({label}): ..."));
    }
    Ok(())
}
```

### D-18 Deterministic Key Order

**Source:** `cli/src/sysml_v2.rs` lines 121–123, `cli/src/main.rs` lines 319–322
**Apply to:** `resolver.rs` (deal.lock generation), `reqif.rs` (SpecObject ordering)

```rust
// Sort before serializing to guarantee D-18 byte-stable output.
nodes.sort_by_key(|n| n.id);          // in reqif.rs
packages.sort_by(|a, b| a.name.cmp(&b.name));  // in resolver.rs lockfile
// Use BTreeMap<K,V> instead of HashMap<K,V> for any map that ends up serialized.
```

### DiagnosticCollector emitFmt Pattern (Zig)

**Source:** `src/diagnostics.zig` lines 174–214 + sema.zig pattern
**Apply to:** `src/sema.zig` dimensional check (new Check #7)

```zig
// All new dimensional diagnostics must use comptime fmt strings (T-02-10).
try dc.emitFmt(Codes.e_dimension_mismatch, expr.span, "dimension mismatch: expected {s}, got {s}", .{ expected_dim_name, actual_dim_name });
try dc.emitFmt(Codes.e_mixed_unit_comparison, expr.span, "E2501: mixed-unit comparison requires explicit conversion (D-57)", .{});
```

### Phase Gate Fan-Out Pattern (Zig build)

**Source:** `build.zig` lines 322–407
**Apply to:** Phase 4 gate block in `build.zig`

```zig
const gate_N_step = b.step("phase-N-gate", "...");
gate_N_step.dependOn(gate_prev_step);  // D-35: cumulative regression
const cmd = b.addSystemCommand(&[_][]const u8{"bash", "-c", "..."});
gate_N_step.dependOn(&cmd.step);
```

---

## No Analog Found

| File | Role | Data Flow | Reason |
|---|---|---|---|
| `deal-lang.org/astro.config.mjs` | config | — | No Astro/Starlight site exists yet; use RESEARCH.md Pattern 1 |
| `deal-lang.org/src/content/docs/**/*.mdx` | config | — | No Starlight MDX content exists; structure from D-62 topic list |
| `deal-lang.org/.github/workflows/deploy.yml` | config | — | No static-site GH Actions workflow exists; use RESEARCH.md Pattern 4 |

---

## Metadata

**Analog search scope:**
- `cli/src/` — all 4 existing Rust source files
- `src/sema.zig`, `src/diagnostics.zig` — sema + diagnostics Zig files
- `build.zig` — full phase-gate pattern (lines 259–407)
- `scripts/verify-fresh-worktree.sh` — sibling symlink pattern
- `tests/unit/sema_corpus.zig` — Zig test structure
- `tests/ffi/tests/gate.rs`, `tests/ffi/tests/parse.rs` — Rust integration test structure
- `tests/golden/sysml-v2/` — golden fixture naming convention
- `spec/examples/showcase/deal.toml` — deal.toml format
- `spec/examples/showcase/packages/requirements/system.deal` — DEAL source annotation syntax
- `../deal-stdlib/README.md` — sibling repo state
- `../deal-lang.org/README.md` — sibling repo state (README-only, no site yet)

**Files scanned:** 14 files read directly; 8 additional via grep/ls
**Pattern extraction date:** 2026-06-05
