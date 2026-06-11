# Phase 5: Simulation Integration - Pattern Map

**Mapped:** 2026-06-08
**Files analyzed:** 18 new/modified files
**Analogs found:** 17 / 18

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `cli/src/main.rs` | controller | request-response | self (modification) | exact |
| `cli/src/simulate.rs` | service | CRUD + batch | `cli/src/resolver.rs` | role-match |
| `cli/src/evidence.rs` | service | file-I/O + CRUD | `cli/src/sysml_v2.rs` | role-match |
| `cli/src/verify.rs` | service | transform + request-response | `cli/src/sysml_v2.rs` + `cli/src/reqif.rs` | role-match |
| `cli/src/sims_protocol.rs` | utility | transform | `cli/src/schema_registry.rs` | role-match |
| `src/lib.zig` | middleware | request-response | self (modification) | exact |
| `src/sim/deal_sim.zig` | utility | transform + event-driven | `src/sema.zig` (C ABI pattern) | partial-match |
| `src/sim/zig_runner.zig` | service | transform | `src/lib.zig` (arena pattern) | partial-match |
| `spec/sims/v0/input.schema.json` | config | — | `spec/ir/v0/schema.json` | exact |
| `spec/sims/v0/output.schema.json` | config | — | `spec/ir/v0/schema.json` | exact |
| `spec/sims/v0/metadata.schema.json` | config | — | `spec/ir/v0/schema.json` | exact |
| `spec/sims/v0/README.md` | config | — | `spec/ir/v0/README.md` | exact |
| `deal-sim/src/deal_sim/simulation.py` | model | — | `spec/examples/showcase/simulations/thermal/battery_thermal.py` | exact |
| `deal-sim/src/deal_sim/cli.py` | controller | request-response | `spec/examples/showcase/simulations/thermal/battery_thermal.py` | exact |
| `deal-sim/src/deal_sim/validation.py` | utility | transform | — | no analog |
| `deal-sim/src/deal_sim/metadata.py` | utility | transform | — | no analog |
| `deal-sim/pyproject.toml` | config | — | `cli/Cargo.toml` (structure) | partial-match |
| `build.zig` | config | — | self (modification) | exact |
| `scripts/verify-fresh-worktree.sh` | config | — | self (modification) | exact |

---

## Pattern Assignments

### `cli/src/main.rs` (controller, request-response) — modification

**Analog:** `cli/src/main.rs` (self — existing pattern to extend)

**Subcommand enum extension pattern** (`cli/src/main.rs` lines 81-132):
```rust
#[derive(Subcommand)]
enum Command {
    /// Run semantic checks; exit 1 on any blocking diagnostic.
    Check {
        paths: Vec<std::path::PathBuf>,
    },
    /// Scaffold a new DEAL project in ./<name>/ (D-67/D-69).
    Init {
        name: Option<String>,
    },
    /// Resolve dependencies declared in deal.toml and write deal.lock (D-66/D-68).
    Install,
}
```

**Add these new variants following the same shape:**
```rust
    /// Run one or more simulations registered in deal.sims.toml.
    Simulate {
        /// Sim name(s) to run; omit to run all (with --all).
        names: Vec<String>,
        /// Run all registered simulations in dependency order.
        #[arg(long)]
        all: bool,
        /// Re-run only simulations with stale evidence.
        #[arg(long)]
        stale: bool,
    },
    /// Evidence capture and baseline management.
    Evidence {
        #[command(subcommand)]
        subcommand: EvidenceCommand,
    },
```

**Check subcommand flag additions** — extend existing `Check { paths }` with:
```rust
    Check {
        paths: Vec<std::path::PathBuf>,
        /// Evaluate verification criteria against captured evidence (SIM-5).
        #[arg(long)]
        verify: bool,
        /// Validate deal.sims.toml bindings structurally (no sim execution).
        #[arg(long)]
        simulations: bool,
        /// When combined with --verify, re-run stale sims before evaluating (D-84 opt-in).
        #[arg(long)]
        run_sims: bool,
    },
```

**Dispatch pattern** (`cli/src/main.rs` lines 1666-1681):
```rust
fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Command::Check { paths } => run_check(&paths, cli.json, cli.color),
        Command::Init { name } => run_init(name, cli.json, cli.color),
        Command::Install => run_install(cli.json, cli.color),
    }
}
```

**Error handling + exit code pattern** (`cli/src/main.rs` lines 1684-1710):
```rust
fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(ref e) if e.is_user() => {
            let msg = format!("{}", e);
            if !msg.is_empty() {
                let _ = writeln!(stderr, "error: {}", msg);
            }
            std::process::ExitCode::from(1)
        }
        Err(e) => {
            let _ = writeln!(stderr, "error: {}", e);
            std::process::ExitCode::from(2)
        }
    }
}
```

**Module declarations to add** (`cli/src/main.rs` lines 23-28):
```rust
pub mod render;
pub mod reqif;
pub mod reqif_schema;
pub mod resolver;
pub mod schema_registry;
pub mod sysml_v2;
// NEW for Phase 5:
pub mod simulate;
pub mod evidence;
pub mod verify;
pub mod sims_protocol;
```

---

### `cli/src/simulate.rs` (service, CRUD + batch)

**Analog:** `cli/src/resolver.rs`

**Imports pattern** (`cli/src/resolver.rs` lines 30-36):
```rust
use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{anyhow, Context as _};
use serde::{Deserialize, Serialize};
```

**For simulate.rs add:**
```rust
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Context as _};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::CliError;
```

**TOML struct deserialization pattern** (`cli/src/resolver.rs` lines 44-53):
```rust
#[derive(Debug, Deserialize)]
pub struct DealToml {
    pub project: ProjectSection,
    #[serde(default)]
    pub workspace: WorkspaceSection,
    /// D-18: BTreeMap ensures alphabetical key order
    #[serde(default)]
    pub dependencies: BTreeMap<String, Dependency>,
}
```

**Copy this shape for `deal.sims.toml` structs:**
```rust
#[derive(Debug, Deserialize)]
pub struct SimsRegistry {
    /// D-18: BTreeMap ensures alphabetical key order; HashMap is forbidden
    #[serde(default)]
    pub simulations: BTreeMap<String, SimEntry>,
}

#[derive(Debug, Deserialize)]
pub struct SimEntry {
    pub tool: String,                           // "python", "matlab", "zig", "generic"
    pub entry: Option<String>,                  // script path for python/zig
    pub class: Option<String>,                  // class name
    pub runner: Option<String>,                 // for matlab/generic
    pub binds_to: Option<String>,               // model path binding
    pub annotation: Option<String>,
    #[serde(default)]
    pub inputs: Vec<SimIoBinding>,
    #[serde(default)]
    pub outputs: Vec<SimIoBinding>,
    #[serde(default)]
    pub auto_run: bool,
    pub reproducibility: Option<String>,        // NEW: @reproducibility tier (D-75)
    pub depends_on: Option<Vec<String>>,        // NEW: explicit dependency override
}
```

**Error pattern** — `CliError::User` / `CliError::Internal` (from `cli/src/main.rs` lines 37-50):
```rust
return Err(CliError::User(format!(
    "circular dependency in deal.sims.toml: {:?}", cycle
)));

return Err(CliError::Internal(anyhow::anyhow!(
    "cannot read {:?}: {}", path, e
)));
```

**Subprocess dispatch pattern** — use `std::process::Command` (already in main.rs for `check_fs3_identity`):
```rust
// From cli/src/main.rs lines 587-595 — std::process::Command pattern:
let result = std::process::Command::new("git")
    .args(["config", "user.name"])
    .output();
match result {
    Ok(output) if output.status.success() => { /* use output.stdout */ }
    Ok(_) => Err(CliError::User("...".into())),
    Err(_) => Err(CliError::Internal(anyhow::anyhow!("..."))),
}
```

**Graceful-skip pattern** (D-72 MATLAB, mirrors D-58 DOORS) — follow this template:
```rust
// When tool binary not found or license error:
// Write .deal/evidence/<sim_name>/skip.json with D-18 sorted keys
let skip = serde_json::json!({
    "reason": format!("{}", e),
    "skipped": true,
    "sim": sim_name,
    "timestamp": chrono::Utc::now().to_rfc3339(),
});
// Write skip.json to evidence cache dir
// Log warning to stderr; continue to next sim (do NOT return Err)
```

**Content-hash staleness key** (D-83):
```rust
use sha2::{Sha256, Digest};
fn compute_staleness_key(
    resolved_inputs: &serde_json::Value,
    sim_source_path: &Path,
    registry_entry_toml: &str,
) -> String {
    let mut h = Sha256::new();
    // D-18 determinism: serialize with BTreeMap-ordered keys
    h.update(serde_json::to_vec(resolved_inputs).unwrap());
    h.update(std::fs::read(sim_source_path).unwrap_or_default());
    h.update(registry_entry_toml.as_bytes());
    format!("{:x}", h.finalize())
}
```

---

### `cli/src/evidence.rs` (service, file-I/O + CRUD)

**Analog:** `cli/src/sysml_v2.rs` (IR-walk + output-write pattern)

**Imports pattern** (`cli/src/sysml_v2.rs` lines 16-19):
```rust
use std::collections::HashMap;
use anyhow::{anyhow, Context as _};
use serde_json::{json, Map, Value};
use uuid::Uuid;
```

**For evidence.rs use:**
```rust
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context as _};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::CliError;
```

**BTreeMap-keyed JSON output for D-18 invariant** (`cli/src/main.rs` lines 441-468):
```rust
// This is the authoritative pattern for D-18 alphabetical-key output:
let mut merged_elements = std::collections::BTreeMap::<String, serde_json::Value>::new();
// ... populate ...
let workspace_index = serde_json::json!({
    "deal_version": deal_version,
    "elements": merged_elements,     // BTreeMap auto-sorts keys
    "imports_graph": merged_imports,
    "v": 1,
});
let serialized = serde_json::to_vec(&workspace_index)
    .map_err(|e| CliError::Internal(anyhow::anyhow!("serialize: {}", e)))?;
std::fs::write(&index_path, &serialized)
    .map_err(|e| CliError::Internal(anyhow::anyhow!("write {:?}: {}", index_path, e)))?;
```

**Evidence cache write pattern** — copy the atomic-write pattern from `cli/src/main.rs` `write_file_atomic` (lines 713-749):
```rust
fn write_file_atomic(path: &std::path::Path, data: &[u8]) -> Result<(), CliError> {
    let parent = path.parent().unwrap_or(std::path::Path::new("."));
    let tmp_path = parent.join(format!(
        ".deal_fmt_tmp_{}_{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
        std::process::id()
    ));
    // write to tmp, then rename (atomic on POSIX)
    std::fs::rename(&tmp_path, path).map_err(|e| { ... })?;
    Ok(())
}
```

**Directory creation pattern** (`cli/src/main.rs` lines 469-484):
```rust
let index_dir = workspace_root.join(".deal");
std::fs::create_dir_all(&index_dir).map_err(|e| {
    CliError::Internal(anyhow::anyhow!(
        "cannot create {:?}: {}",
        index_dir,
        e
    ))
})?;
```

---

### `cli/src/verify.rs` (service, transform + request-response)

**Analogs:** `cli/src/sysml_v2.rs` + `cli/src/reqif.rs` (IR-walk + JSON-output emitters)

**IR-walk entry point pattern** (`cli/src/sysml_v2.rs` lines 84-100):
```rust
pub fn emit(ir_json: &Value) -> anyhow::Result<Value> {
    let elements_map = ir_json
        .get("elements")
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow!("IR JSON missing 'elements' object"))?;

    let edges_arr = ir_json
        .get("edges")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("IR JSON missing 'edges' array"))?;
    // ... walk elements_map by kind
}
```

**`emit_from_bytes` wrapper** (`cli/src/sysml_v2.rs` — the `emit_from_bytes` function):
```rust
pub fn emit_from_bytes(ir_bytes: &[u8]) -> anyhow::Result<Value> {
    let ir_json: Value = serde_json::from_slice(ir_bytes)
        .context("IR JSON parse error")?;
    emit(&ir_json)
}
```

**Copy this shape for verify.rs:**
```rust
pub fn evaluate_from_bytes(
    ir_bytes: &[u8],
    evidence_dir: &Path,
) -> anyhow::Result<VerifyReport> {
    let ir_json: Value = serde_json::from_slice(ir_bytes)
        .context("IR JSON parse error")?;
    evaluate(&ir_json, evidence_dir)
}

pub fn evaluate(ir_json: &Value, evidence_dir: &Path) -> anyhow::Result<VerifyReport> {
    // Walk elements looking for satisfy_block / criteria / compute / evidence nodes
    let elements_map = ir_json
        .get("elements")
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow!("IR JSON missing 'elements' object"))?;
    // ...
}
```

**D-32 JSON envelope pattern** (adapt from `cli/src/main.rs` lines 393-430):
```rust
// For the --json verify report (D-87):
let deal_version = env!("CARGO_PKG_VERSION");
write!(
    stdout,
    r#"{{"command":"verify","deal_version":"{}","requirements":{},"summary":{},"v":1}}"#,
    deal_version, requirements_json, summary_json
)?;
writeln!(stdout)?;
```

**Verdict rubric** (D-86) — implement as an enum:
```rust
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Verdict { Pass, Fail, Partial }

#[derive(Debug, serde::Serialize)]
pub struct CriterionResult {
    pub verdict: Verdict,
    pub stale: bool,        // D-86: orthogonal to verdict
    // ...
}
```

**FFI call pattern for dimension check (D-85)** — mirrors the existing `deal_parse` call in `cli/src/main.rs` lines 300-352:
```rust
// Call new C ABI: deal_check_with_stdlib(source, filename, stdlib_ir, out_diag)
let handle = unsafe {
    ffi::deal_check_with_stdlib(
        source_bytes.as_ptr(), source_bytes.len(),
        filename.as_bytes().as_ptr(), filename.len(),
        stdlib_ir_bytes.as_ptr(), stdlib_ir_bytes.len(),
        &mut out_ptr, &mut out_len,
    )
};
// Clone bytes before free (Pitfall 3 / T-02-13 pattern):
let diag_json_owned = unsafe {
    let bytes = std::slice::from_raw_parts(out_ptr, out_len).to_vec();
    ffi::deal_free(handle);
    bytes
};
```

---

### `cli/src/sims_protocol.rs` (utility, transform)

**Analog:** `cli/src/schema_registry.rs`

**Schema validation boilerplate** (`cli/src/schema_registry.rs` lines 1-43):
```rust
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use anyhow::{anyhow, Context as _};
use jsonschema::{Retrieve, Uri, Validator};
use serde_json::Value;
use sha2::{Digest, Sha256};
```

**SHA-256 pin + offline validation pattern** (`cli/src/schema_registry.rs` lines 22-31):
```rust
/// Expected SHA256 of tests/schemas/SysML.json (from tests/schemas/README.md).
const EXPECTED_SYSML_SHA256: &str =
    "bb0d8af159cf2cbe4a0df4ed6b903505a57e33d047068e5a50a7008a18d546c5";
```

**For sims_protocol.rs, pin the spec/sims/v0/ schemas at the same pattern:**
```rust
/// Expected SHA256 of spec/sims/v0/input.schema.json
const EXPECTED_INPUT_SCHEMA_SHA256: &str = "..."; // fill after authoring

pub fn validate_output(json: &Value) -> Result<(), Vec<String>> {
    // Same OnceLock + Validator pattern as schema_registry::validate()
    static VALIDATOR: OnceLock<Validator> = OnceLock::new();
    let validator = VALIDATOR.get_or_init(|| build_output_validator().unwrap());
    // ...
}
```

---

### `src/lib.zig` (middleware, request-response) — modification

**Analog:** `src/lib.zig` (self — extend with new C ABI export)

**Existing export function signature pattern** (`src/lib.zig` lines 90-95):
```zig
pub export fn deal_parse(
    source_ptr: [*]const u8, source_len: usize,
    filename_ptr: [*]const u8, filename_len: usize,
) callconv(.c) ?*anyopaque {
```

**Internal function with stdlib seeding** (`src/lib.zig` lines 336-448) — `deal_parse_internal_with_stdlib` is already the pattern for the new export. The new C ABI function `deal_check_with_stdlib` follows this same internal structure but adds a third input (stdlib_ir JSON bytes):
```zig
// NEW export for D-85 / D-88:
pub export fn deal_check_with_stdlib(
    source_ptr: [*]const u8, source_len: usize,
    filename_ptr: [*]const u8, filename_len: usize,
    stdlib_ir_ptr: [*]const u8, stdlib_ir_len: usize,
    out_diag_ptr: *[*]const u8, out_diag_len: *usize,
) callconv(.c) bool {
    // 1. Deserialize stdlib_ir JSON to build external SymbolTable
    // 2. Call sema.analyzeWithExternalTable(arena, ast_root, diags, filename, &merged)
    // 3. Write diagnostics JSON to out_diag_ptr/len
    // 4. Return has_errors
}
```

**Arena + handle lifecycle** (`src/lib.zig` lines 109-187) — copy this pattern exactly:
```zig
var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
const init_alloc = arena.allocator();
const handle = init_alloc.create(DealHandle) catch {
    arena.deinit();
    return null;
};
handle.* = .{ .arena = arena, .source = "", .filename = "" };
const alloc = handle.arena.allocator();
// ... use alloc for all subsequent allocations
// Caller calls deal_free(handle) to release all memory
```

---

### `src/sim/deal_sim.zig` (utility, transform + event-driven)

**Analog:** `src/sema.zig` (Zig module with pub functions called by lib.zig)

**Module structure pattern** (`src/sema.zig` lines 240-315):
```zig
/// Public function called from lib.zig — takes arena allocator, returns result.
pub fn analyzeWithExternalTable(
    arena: std.mem.Allocator,
    root: ?*ast.Node,
    diag_list: *std.ArrayList(Diagnostic),
    source_file: []const u8,
    external_table: ?*const SymbolTable,
) !*SymbolTable {
    const table = try arena.create(SymbolTable);
    // ... implementation
    return table;
}
```

**@setFloatMode usage** (from ADR-deal-stdlib-numeric-model.md) — must be placed at comptime function scope before arithmetic:
```zig
// In deal_sim.zig — the DealSimulation comptime wrapper:
pub fn DealSimulation(comptime T: type) type {
    return struct {
        pub export fn sim_run(
            input_json: [*]const u8, input_len: usize,
            out_json: *[*]const u8, out_len: *usize,
        ) callconv(.C) bool {
            // D-73/D-75: set float mode from declared @reproducibility tier
            const mode = if (@hasDecl(T, "reproducibility"))
                T.reproducibility
            else
                .strict;
            @setFloatMode(mode);
            // ... deserialize input, call T.run(), serialize output
        }
    };
}
```

---

### `src/sim/zig_runner.zig` (service, transform)

**Analog:** `src/lib.zig` (arena pattern, JSON serialization via `src/json.zig`)

**JSON output serialization** — copy the IR JSON emission pattern from `src/lib.zig` lines 533-577 (`deal_ir_json`):
```zig
pub export fn deal_ir_json(
    handle_ptr: ?*anyopaque,
    out_ptr: *[*]const u8,
    out_len: *usize,
) callconv(.c) bool {
    const handle: *DealHandle = @ptrCast(@alignCast(handle_ptr orelse return false));
    const alloc = handle.arena.allocator();
    const ir_root = handle.index_root orelse return false;
    const bytes = json.writeIrJson(alloc, ir_root) catch return false;
    out_ptr.* = bytes.ptr;
    out_len.* = bytes.len;
    return true;
}
```

---

### `spec/sims/v0/{input,output,metadata}.schema.json` (config)

**Analog:** `spec/ir/v0/schema.json`

Key conventions from `spec/ir/v0/README.md`:
- JSON Schema draft-2020-12
- All keys alphabetical (D-18)
- Normative `deal_sim_protocol` version field mirrors `ir_version`
- `v: 1` integer version field in every artifact

**Envelope shape** (mirrors `spec/ir/v0/` conventions):
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://deal-lang.org/spec/sims/v0/output.schema.json",
  "title": "DEAL Simulation Protocol v0 — output.json",
  "type": "object",
  "required": ["deal_sim_protocol", "exit_code", "outputs"],
  "additionalProperties": false,
  "properties": {
    "deal_sim_protocol": {"const": "v0"},
    "exit_code": {"type": "integer"},
    "outputs": { ... },
    "v": {"const": 1}
  }
}
```

---

### `deal-sim/src/deal_sim/simulation.py` (model)

**Analog:** `spec/examples/showcase/simulations/thermal/battery_thermal.py` (the oracle)

**Complete base class contract** (`battery_thermal.py` lines 1-63 — this is the oracle the SDK must make work without modification):
```python
from deal_sim import DealSimulation

class BatteryThermal(DealSimulation):
    inputs = {
        "packResistance": {"type": "Real", "unit": "ohm"},
        "totalCurrent": {"type": "Real", "unit": "A"},
        "coolantFlowRate": {"type": "Real", "unit": "L/min"},
    }
    outputs = {
        "heatGenerated": {"type": "Real", "unit": "W"},
        "coolantOutTemp": {"type": "Real", "unit": "degC"},
    }
    def run(self, inputs: dict) -> dict:
        heat_generated = inputs["totalCurrent"] ** 2 * inputs["packResistance"]
        # ...
        return {"heatGenerated": round(heat_generated, 1), "coolantOutTemp": ...}

if __name__ == "__main__":
    BatteryThermal.cli()
```

**Required SDK surface** (from oracle analysis):
- `DealSimulation` base class with class-level `inputs: dict` and `outputs: dict`
- `run(self, inputs: dict) -> dict` — abstract method authors implement
- `cli(cls)` — classmethod that handles JSON I/O; invoked by `if __name__ == "__main__":`
- The oracle file uses `from deal_sim import DealSimulation` — the `__init__.py` must re-export this

---

### `deal-sim/src/deal_sim/cli.py` (controller, request-response)

**Analog:** `spec/examples/showcase/simulations/thermal/battery_thermal.py` (oracle — the `cli()` method contract)

**CLI runner pattern** (from RESEARCH Pattern 3 + oracle analysis):
```python
@classmethod
def cli(cls):
    import argparse, json, sys, time, pathlib
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", default="input.json")
    parser.add_argument("--output", default="output.json")
    parser.add_argument("--metadata", default="metadata.json")
    args = parser.parse_args()

    with open(args.input) as f:
        raw_inputs = json.load(f)

    instance = cls()
    validated = instance._validate_inputs(raw_inputs)   # stdlib-only (D-78)

    t0 = time.time()
    result = instance.run(validated)
    elapsed = time.time() - t0

    instance._validate_outputs(result)

    # D-18: sort_keys=True for byte-stable evidence artifacts
    with open(args.output, "w") as f:
        json.dump(result, f, indent=2, sort_keys=True)

    with open(args.metadata, "w") as f:
        json.dump({
            "deal_sim_protocol": "v0",
            "deal_sim_version": "0.1.0",
            "duration_s": round(elapsed, 6),
            "reproducibility_tier": "tolerant",
            "timestamp": ...,
            "tool": "python",
            "tool_version": sys.version.split()[0],
        }, f, indent=2, sort_keys=True)
```

---

### `deal-sim/src/deal_sim/validation.py` + `deal-sim/src/deal_sim/metadata.py`

**No direct analog.** These are pure Python stdlib modules. No existing Python code in the repo.

The `validation.py` module must implement stdlib-only type checking (D-78): validate that each key in `inputs`/`outputs` dicts matches the declared `type` and `unit` without any third-party deps.

The `metadata.py` module produces the `metadata.json` artifact using `datetime.datetime.utcnow().isoformat()` and `sys.version`.

See **No Analog Found** section below.

---

### `build.zig` (config) — modification

**Analog:** `build.zig` itself (lines 409-485 — phase-4-gate + phase-4-gate-fresh pattern)

**Phase gate umbrella step pattern** (`build.zig` lines 419-468):
```zig
const gate_4_step = b.step(
    "phase-4-gate",
    "Run the full Phase 4 exit-gate suite (..., inheriting phase-3-gate)",
);
gate_4_step.dependOn(gate_3_step);  // cumulative inherit — D-35

const gate_4_cargo_test = b.addSystemCommand(&[_][]const u8{
    "cargo", "test", "--workspace", "--manifest-path", "Cargo.toml",
});
gate_4_step.dependOn(&gate_4_cargo_test.step);

const gate_4_stdlib_check = b.addSystemCommand(&[_][]const u8{
    "bash", "-c", "cd ../deal-stdlib && ../deal/target/release/deal check packages/",
});
gate_4_step.dependOn(&gate_4_stdlib_check.step);
```

**Fresh-worktree gate sibling pattern** (`build.zig` lines 476-485):
```zig
const gate_4_fresh_step = b.step(
    "phase-4-gate-fresh",
    "Run phase-4-gate inside a freshly-created ephemeral worktree ...",
);
const run_fresh_gate_4 = b.addSystemCommand(&[_][]const u8{
    "bash",
    "scripts/verify-fresh-worktree.sh",
    "phase-4-gate",
});
gate_4_fresh_step.dependOn(&run_fresh_gate_4.step);
```

**Phase 5 gate additions** — add these new steps following the exact same shape:
```zig
// Phase 5 gate: inherits phase-4-gate; adds deal-sim install + Python/Zig sim execution
const gate_5_step = b.step("phase-5-gate", "Run the full Phase 5 exit-gate suite ...");
gate_5_step.dependOn(gate_4_step);  // D-35 cumulative

// (a) cargo test --workspace (simulate/evidence/verify + e2500_cli tests)
const gate_5_cargo_test = b.addSystemCommand(&[_][]const u8{
    "cargo", "test", "--workspace", "--manifest-path", "Cargo.toml",
});
gate_5_step.dependOn(&gate_5_cargo_test.step);

// (b) deal-sim local install + Python SDK tests
const gate_5_dealsim_install = b.addSystemCommand(&[_][]const u8{
    "bash", "-c", "cd ../deal-sim && pip install -e . && python -m pytest tests/ -x",
});
gate_5_step.dependOn(&gate_5_dealsim_install.step);

// (c) Phase 5 E2E smoke: deal simulate --all + deal check --verify
const gate_5_smoke = b.addSystemCommand(&[_][]const u8{
    "bash", "scripts/phase-5-smoke.sh",
});
gate_5_step.dependOn(&gate_5_smoke.step);

// Phase 5 fresh-worktree gate
const gate_5_fresh_step = b.step("phase-5-gate-fresh", "Run phase-5-gate inside ephemeral worktree ...");
const run_fresh_gate_5 = b.addSystemCommand(&[_][]const u8{
    "bash", "scripts/verify-fresh-worktree.sh", "phase-5-gate",
});
gate_5_fresh_step.dependOn(&run_fresh_gate_5.step);
```

---

### `scripts/verify-fresh-worktree.sh` (config) — modification

**Analog:** `scripts/verify-fresh-worktree.sh` (self — lines 163-177 are the sibling-symlink loop)

**Existing sibling loop** (`scripts/verify-fresh-worktree.sh` lines 163-177):
```bash
WORKTREE_PARENT="$(dirname "$WORKTREE_DIR")"
DEAL_PARENT="$(cd "$REPO_ROOT/.." && pwd)"
for sibling in tree-sitter-deal vscode-deal deal-stdlib deal-lang.org; do
  if [ -d "$DEAL_PARENT/$sibling" ]; then
    ln -sfn "$DEAL_PARENT/$sibling" "$WORKTREE_PARENT/$sibling"
    echo "verify-fresh-worktree: linked $sibling into $WORKTREE_PARENT"
  else
    echo "verify-fresh-worktree: WARNING — sibling repo $sibling not found ..." >&2
  fi
done
trap '...; for s in tree-sitter-deal vscode-deal deal-stdlib deal-lang.org; do [ -L "$WORKTREE_PARENT/$s" ] && rm -f "$WORKTREE_PARENT/$s"; done' EXIT
```

**Add `deal-sim` to both the loop and the trap cleanup list:**
```bash
# Line 165 — extend sibling list:
for sibling in tree-sitter-deal vscode-deal deal-stdlib deal-lang.org deal-sim; do
```
```bash
# Line 177 — extend trap cleanup:
trap '...; for s in tree-sitter-deal vscode-deal deal-stdlib deal-lang.org deal-sim; do ...' EXIT
```

**Add pip install step after sibling symlink loop** (new block between step 6.5 and step 7):
```bash
# 6.6. Install deal-sim into the ephemeral environment so Python sims can import deal_sim.
if [ -d "$WORKTREE_PARENT/deal-sim" ]; then
  echo "verify-fresh-worktree: installing deal-sim (editable)"
  pip install -e "$WORKTREE_PARENT/deal-sim" --quiet
fi
```

---

## Shared Patterns

### C ABI Call + Clone-Before-Free (Pitfall 3 / T-02-13)

**Source:** `cli/src/main.rs` lines 318-352 (the inner unsafe block in `run_check`)
**Apply to:** `cli/src/verify.rs` (FFI calls for dimension check), any new FFI calls in `cli/src/simulate.rs`

```rust
// ALWAYS clone bytes before deal_free — arena is invalidated on free
let diag_json_owned: Vec<u8>;
let has_errors: bool;

unsafe {
    has_errors = ffi::deal_has_errors(handle);

    let mut out_ptr: *const u8 = std::ptr::null();
    let mut out_len: usize = 0;
    let ok = ffi::deal_diagnostics_json(handle, &mut out_ptr, &mut out_len);

    if !ok {
        ffi::deal_free(handle);
        return Err(CliError::Internal(anyhow::anyhow!("OOM for {:?}", path)));
    }

    // Clone bytes while arena is alive — BEFORE deal_free
    diag_json_owned = std::slice::from_raw_parts(out_ptr, out_len).to_vec();
    ffi::deal_free(handle);  // pointer now invalid — diag_json_owned is the live copy
}
```

### D-18 Alphabetical Key Order

**Source:** `cli/src/main.rs` lines 441-468 (workspace index merge using BTreeMap)
**Apply to:** All JSON artifacts in `cli/src/evidence.rs`, `cli/src/verify.rs`, Python `cli.py` `json.dump(..., sort_keys=True)`

```rust
// Rust: use BTreeMap (not HashMap) for any serialized map
let mut obj = std::collections::BTreeMap::<String, serde_json::Value>::new();
obj.insert("a_key".to_string(), val1);
obj.insert("z_key".to_string(), val2);
// BTreeMap guarantees alphabetical key order on serialization — D-18 invariant
```

### D-32 JSON Envelope

**Source:** `cli/src/main.rs` lines 392-430 (`run_check` JSON envelope assembly)
**Apply to:** `cli/src/verify.rs` `--json` output, `cli/src/simulate.rs` `--json` output

```rust
// Envelope: alphabetical top-level keys: command, deal_version, <payload>, summary, v
write!(
    stdout,
    r#"{{"command":"verify","deal_version":"{}","requirements":{},"summary":{},"v":1}}"#,
    env!("CARGO_PKG_VERSION"), requirements_json, summary_json
)?;
writeln!(stdout)?;
```

### CliError Propagation Pattern

**Source:** `cli/src/main.rs` lines 37-60 (CliError enum + Display impl)
**Apply to:** All new `run_*` functions in Phase 5

```rust
// I/O failures → CliError::Internal  (exit 2)
let bytes = std::fs::read(path).map_err(|e| {
    CliError::Internal(anyhow::anyhow!("cannot read {:?}: {}", path, e))
})?;

// User-visible domain errors → CliError::User  (exit 1)
return Err(CliError::User(format!(
    "no deal.sims.toml found under {:?}", path
)));
```

### Subprocess-Spawn Exit-Code Capture

**Source:** `cli/src/main.rs` lines 587-613 (`check_fs3_identity`)
**Apply to:** `cli/src/simulate.rs` subprocess adapters (Python, MATLAB, generic)

```rust
let result = std::process::Command::new("python")
    .arg(entry_path)
    .arg("--input").arg(&input_json_path)
    .arg("--output").arg(&output_json_path)
    .arg("--metadata").arg(&metadata_json_path)
    .current_dir(&workdir)
    .output();
match result {
    Ok(output) if output.status.success() => { /* read output.json */ }
    Ok(output) => {
        // Non-zero exit from sim — log stderr, return CliError::User
        Err(CliError::User(format!(
            "simulation '{}' failed (exit {}): {}",
            name, output.status, String::from_utf8_lossy(&output.stderr)
        )))
    }
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
        // Tool not installed — graceful skip (D-72/D-58 pattern)
        write_skip_record(...)?;
        Ok(SimResult::Skipped)
    }
    Err(e) => Err(CliError::Internal(anyhow::anyhow!("spawn failed: {}", e))),
}
```

### Zig Export Function Shape

**Source:** `src/lib.zig` lines 90-187 (`deal_parse` export — the canonical C ABI pattern)
**Apply to:** New `deal_check_with_stdlib` export in `src/lib.zig`, any Zig sim exports in `src/sim/deal_sim.zig`

```zig
pub export fn deal_<name>(
    source_ptr: [*]const u8, source_len: usize,
    // ... additional typed args
    out_ptr: *[*]const u8, out_len: *usize,
) callconv(.c) bool {
    // 1. Guard NULL + non-zero len pairs (ASVS V5)
    if (source_len > 0 and @intFromPtr(source_ptr) == 0) return false;
    // 2. Arena allocation
    var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
    // 3. Core logic in arena
    // 4. Write out_ptr/out_len to arena-allocated slice
    // 5. Return success bool (caller calls deal_free to release)
}
```

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `deal-sim/src/deal_sim/validation.py` | utility | transform | No Python source in repo; stdlib-only dict type checker (D-78) is novel |
| `deal-sim/src/deal_sim/metadata.py` | utility | transform | No Python source in repo; pure stdlib datetime + sys.version collection |

For these files, use RESEARCH.md Pattern 3 (Python SDK CLI Runner Protocol) as the primary reference. The validation logic is: for each key in `raw_inputs`, check it exists in `cls.inputs`, then check its value type matches the declared `type` field ("Real" → `float`/`int`, "Integer" → `int`, "Boolean" → `bool`, "String" → `str`). Raise `ValueError` with the field name and expected type on mismatch.

---

## Metadata

**Analog search scope:** `cli/src/`, `src/`, `scripts/`, `build.zig`, `spec/ir/v0/`, `spec/examples/showcase/simulations/`, `deal-sim/`
**Files scanned:** 14 analog files read in full or by targeted section
**Key oracle files:** `cli/src/main.rs` (1711 lines), `build.zig` (486 lines, gate patterns at lines 409-485), `src/lib.zig` (lines 85-448), `src/sema.zig` (lines 240-315), `scripts/verify-fresh-worktree.sh` (lines 163-198), `spec/examples/showcase/simulations/thermal/battery_thermal.py` (63 lines, complete oracle)
**Pattern extraction date:** 2026-06-08
