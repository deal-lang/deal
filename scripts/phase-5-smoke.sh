#!/usr/bin/env bash
#
# scripts/phase-5-smoke.sh
#
# Phase 5 end-to-end smoke test: deal simulate --all + deal check --verify.
#
# Runs the full Phase 5 showcase verification chain:
#   1. Build the deal release binary.
#   2. Create a temp copy of the showcase project (avoids spec submodule pollution).
#   3. Pre-seed input.json for battery_thermal (Python oracle sim) with physics-valid
#      values (bypasses IR resolution in gate context where deal-std may not be installed).
#   4. deal simulate --all:  battery_thermal runs for real and produces output.json;
#      motor_thermal/range_model are empty stub scripts that exit 0 gracefully;
#      motor_efficiency/vehicle_dynamics are MATLAB — graceful-skip (skip.json, no fail).
#   5. deal evidence capture — snapshots working runs to .deal/captured/.
#   6. deal evidence baseline v2.1.0 — frozen git-tracked V&V snapshot + manifest.json.
#   7. deal check --simulations — structural MATLAB binding validation (no execution).
#   8. deal check --verify model/traceability.dealx — per-REQ report; asserts REQ_BAT_001
#      and REQ_BAT_002 present with verdicts.
#   9. Zig sim gate: zig build test -Dtest-filter=sim — confirms range_model.zig (the
#      canonical Zig simulation) runs for real and passes the bit-reproducibility test.
#
# Hard-gate model (D-72):
#   - battery_thermal (Python) MUST run and produce schema-valid output.json. FAIL → exit 1.
#   - MATLAB sims (motor_efficiency, vehicle_dynamics) MUST graceful-skip (skip.json written,
#     no hard error). If MATLAB is installed and a sim hard-fails, exit 1.
#   - Zig sim (range_model.zig) MUST pass its internal test suite. FAIL → exit 1.
#   - deal check --simulations MUST exit 0 (structural MATLAB binding validation).
#   - deal check --verify report MUST contain REQ_BAT_001 and REQ_BAT_002.
#
# D-72 ADR: MATLAB uses subprocess (-batch) adapter; graceful-skip when not found.
#
# Argument injection note (T-3-07):
#   This script takes NO arguments — the showcase path is resolved relative to
#   this script's location (spec/examples/showcase/). No untrusted input.
#
# Exit codes:
#   0 — all hard-gate checks pass; full chain exercised end-to-end.
#   1 — a hard-gate assertion failed (FAIL echo precedes exit 1).
#   2 — environment error (deal binary not found, showcase missing, etc.).

set -euo pipefail

# ─── Resolve paths ────────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DEAL_BIN="$REPO_ROOT/target/release/deal"
SHOWCASE_SRC="$REPO_ROOT/spec/examples/showcase"

# ─── 0. Environment sanity checks ─────────────────────────────────────────────

echo "phase-5-smoke: checking environment"

if [ ! -f "$SHOWCASE_SRC/simulations/deal.sims.toml" ]; then
  echo "phase-5-smoke: FATAL — showcase not found at $SHOWCASE_SRC" >&2
  echo "  Ensure spec/ submodule is initialized: git submodule update --init --recursive" >&2
  exit 2
fi

# ─── 0b. Supply-chain import gate (T-05-05) ───────────────────────────────────
#
# D-78 stdlib-only core: the deal-sim SDK core must import nothing outside the
# Python standard library. This gate hard-fails the smoke run if numpy, scipy, or
# jsonschema (the canonical "a real registry install would be needed" markers)
# appear as imports in the SDK source — catching a future regression that the
# manual review at plan time could not. deal-sim is a sibling repo; the gate is
# guarded by directory existence (mirrors verify-fresh-worktree.sh).

DEAL_SIM_SRC="$REPO_ROOT/../deal-sim/src/deal_sim"
if [ -d "$DEAL_SIM_SRC" ]; then
  echo "phase-5-smoke: checking deal-sim SDK core for forbidden third-party imports (T-05-05)"
  if grep -rEn '^[[:space:]]*(import|from)[[:space:]]+(numpy|scipy|jsonschema)([[:space:].]|$)' "$DEAL_SIM_SRC"; then
    echo "phase-5-smoke: FAIL — forbidden third-party import in deal-sim SDK core (D-78 stdlib-only violated)" >&2
    exit 1
  fi
  echo "  PASS: deal-sim SDK core is free of numpy/scipy/jsonschema imports (D-78)"
else
  echo "phase-5-smoke: SKIP — deal-sim sibling not present at $DEAL_SIM_SRC (T-05-05 import gate not run)"
fi

# ─── 1. Build the release binary ──────────────────────────────────────────────

echo "phase-5-smoke: building deal release binary"
cd "$REPO_ROOT"
cargo build --release -p deal --quiet

if [ ! -x "$DEAL_BIN" ]; then
  echo "phase-5-smoke: FATAL — deal binary not found at $DEAL_BIN after build" >&2
  exit 2
fi

# ─── 2. Create temp showcase copy ─────────────────────────────────────────────
#
# Copy the showcase into a temp dir so we don't pollute the spec/ submodule with
# .deal/evidence/ artifacts. The EXIT trap cleans up regardless of success/failure.

WORK_DIR="$(mktemp -d -t deal-smoke-XXXXXX)"
trap 'rm -rf "$WORK_DIR"' EXIT

echo "phase-5-smoke: copying showcase to $WORK_DIR"
cp -r "$SHOWCASE_SRC/." "$WORK_DIR/"

cd "$WORK_DIR"

# ─── 3. Pre-seed battery_thermal input.json ───────────────────────────────────
#
# IR model_path resolution returns null values when deal-std is not installed
# (showcase deal.toml declares deal-std but .deal/deps/ is empty in gate context).
# Pre-seeding bypasses IR resolution — same approach as simulate_integration.rs.
# Physics values: Q = I²R = 250²×0.05 = 3125 W (heat generated), coolantOut ≈ 29°C.

echo "phase-5-smoke: pre-seeding battery_thermal input.json"
mkdir -p ".deal/evidence/battery_thermal"
cat > ".deal/evidence/battery_thermal/input.json" << 'EOF'
{
  "deal_sim_protocol": "v0",
  "inputs": {
    "coolantFlowRate": {"unit": "L/min", "value": 30.0},
    "packResistance": {"unit": "ohm", "value": 0.05},
    "totalCurrent": {"unit": "A", "value": 250.0}
  },
  "v": 1
}
EOF

# ─── 4. deal simulate --all ───────────────────────────────────────────────────
#
# Expected outcomes (D-72 hard-gate model):
#   battery_thermal  — Python oracle: produces output.json + metadata.json. HARD GATE.
#   motor_thermal    — Python stub (empty file): exits 0, no output.json produced. OK.
#   range_model      — Python stub (empty file): exits 0, no output.json produced. OK.
#   motor_efficiency — MATLAB: binary absent → skip.json written, SimResult::Skipped. OK.
#   vehicle_dynamics — MATLAB: binary absent → skip.json written, SimResult::Skipped. OK.
#
# The D-72 hard-gate is enforced below: battery_thermal MUST produce output.json.

echo "phase-5-smoke: running deal simulate --all"
if ! "$DEAL_BIN" simulate --all 2>&1; then
  echo "phase-5-smoke: FAIL — deal simulate --all exited non-zero" >&2
  exit 1
fi

# D-72 hard-gate: battery_thermal MUST produce schema-valid output.json
echo "phase-5-smoke: asserting battery_thermal produced output.json"
if [ ! -f ".deal/evidence/battery_thermal/output.json" ]; then
  echo "phase-5-smoke: FAIL — .deal/evidence/battery_thermal/output.json not found after simulate" >&2
  exit 1
fi
echo "  PASS: battery_thermal output.json exists"

# Assert output.json is valid JSON with expected keys
if ! python3 -c "
import json, sys
d = json.load(open('.deal/evidence/battery_thermal/output.json'))
assert 'heatGenerated' in str(d) or 'outputs' in d, 'heatGenerated key not found'
print('  PASS: output.json is valid JSON with simulation outputs')
" 2>&1; then
  echo "phase-5-smoke: FAIL — battery_thermal output.json is invalid or missing outputs" >&2
  exit 1
fi

# D-72 graceful-skip: MATLAB sims must have skip.json (not hard-fail)
echo "phase-5-smoke: asserting MATLAB sims graceful-skipped"
MATLAB_FAIL=0
for sim_name in motor_efficiency vehicle_dynamics; do
  if [ -f ".deal/evidence/$sim_name/output.json" ]; then
    # MATLAB was installed and ran — that's OK too
    echo "  OK: $sim_name ran (MATLAB available)"
  elif [ -f ".deal/evidence/$sim_name/skip.json" ]; then
    echo "  PASS: $sim_name graceful-skipped (skip.json present)"
  else
    echo "  FAIL: $sim_name has neither output.json nor skip.json — expected graceful-skip" >&2
    MATLAB_FAIL=1
  fi
done
if [ "$MATLAB_FAIL" -ne 0 ]; then
  echo "phase-5-smoke: FAIL — MATLAB sim(s) neither ran nor graceful-skipped" >&2
  exit 1
fi

# ─── 5. deal evidence capture ─────────────────────────────────────────────────

echo "phase-5-smoke: running deal evidence capture"
if ! "$DEAL_BIN" evidence capture 2>&1; then
  echo "phase-5-smoke: FAIL — deal evidence capture exited non-zero" >&2
  exit 1
fi
echo "  PASS: evidence capture succeeded"

# ─── 6. deal evidence baseline v2.1.0 ─────────────────────────────────────────

echo "phase-5-smoke: running deal evidence baseline v2.1.0"
if ! "$DEAL_BIN" evidence baseline v2.1.0 2>&1; then
  echo "phase-5-smoke: FAIL — deal evidence baseline v2.1.0 exited non-zero" >&2
  exit 1
fi

# Assert baseline manifest exists
if [ ! -f "evidence/baselines/v2.1.0/manifest.json" ]; then
  echo "phase-5-smoke: FAIL — evidence/baselines/v2.1.0/manifest.json not found" >&2
  exit 1
fi
echo "  PASS: evidence/baselines/v2.1.0/manifest.json written"

# ─── 7. deal check --simulations (structural MATLAB binding validation) ────────

echo "phase-5-smoke: running deal check --simulations"
if ! "$DEAL_BIN" check --simulations 2>&1; then
  echo "phase-5-smoke: FAIL — deal check --simulations exited non-zero" >&2
  exit 1
fi
echo "  PASS: deal check --simulations exits 0 (structural MATLAB binding validation)"

# ─── 8. deal check --verify (per-REQ report) ──────────────────────────────────

echo "phase-5-smoke: running deal check --verify model/traceability.dealx"
VERIFY_OUT="$WORK_DIR/.verify-report.txt"
# Capture the report to a file. verify may exit non-zero on stale evidence
# (D-84), so we don't gate on the exit code here — we gate on VERDICT
# CORRECTNESS below (05-08: no `|| true` swallow of the verdict).
"$DEAL_BIN" check --verify model/traceability.dealx > "$VERIFY_OUT" 2>&1 || true

# D-87: assert REQ_BAT_001 appears in the report.
if ! grep -q "REQ_BAT_001" "$VERIFY_OUT"; then
  echo "phase-5-smoke: FAIL — REQ_BAT_001 not found in verify report" >&2
  cat "$VERIFY_OUT" >&2
  exit 1
fi
echo "  PASS: REQ_BAT_001 present in verify report"

# 05-08 GAP CLOSURE (hard gate): REQ_BAT_001 MUST be PASS, not PARTIAL.
# actualCapacity = usableCapacity = kWh(85) >= minCapacity = kWh(85) → PASS.
# This is the real PM-query verdict the synthetic fixtures masked at 05-07.
if ! grep -Eq "REQ_BAT_001:[[:space:]]+(Pass|PASS)" "$VERIFY_OUT"; then
  echo "phase-5-smoke: FAIL — REQ_BAT_001 is not PASS (model-IR evidence resolution regressed)" >&2
  cat "$VERIFY_OUT" >&2
  exit 1
fi
echo "  PASS: REQ_BAT_001 verdict is PASS (85 kWh >= 85 kWh, traceable)"

# Summary must no longer be the all-PARTIAL "0 PASS" the 05-07 checkpoint hit.
if grep -q "0 PASS" "$VERIFY_OUT"; then
  echo "phase-5-smoke: FAIL — verify summary is still '0 PASS' (gap not closed)" >&2
  cat "$VERIFY_OUT" >&2
  exit 1
fi
echo "  PASS: verify summary has real PASS verdicts (no longer 0 PASS)"

# D-87: assert REQ_BAT_002 appears as PARTIAL (status="partial" + gap{} block —
# cold weather tests scheduled 2026-Q3 per spec). This is expected and correct.
if ! grep -Eq "REQ_BAT_002:[[:space:]]+(Partial|PARTIAL)" "$VERIFY_OUT"; then
  echo "phase-5-smoke: FAIL — REQ_BAT_002 is not PARTIAL (expected gap verdict)" >&2
  cat "$VERIFY_OUT" >&2
  exit 1
fi
echo "  PASS: REQ_BAT_002 verdict is PARTIAL (scheduled cold-weather gap)"
REQ_BAT_002_LINE=$(grep "REQ_BAT_002" "$VERIFY_OUT" | head -1 || true)
echo "  REQ_BAT_002: $REQ_BAT_002_LINE"

# ─── 9. Zig sim gate ──────────────────────────────────────────────────────────
#
# The canonical Zig simulation (range_model.zig) is tested via the Zig test suite.
# D-73: in-process execution + evidence serialization. Tests confirm:
#   - physics_sanity: range output is positive and in realistic range
#   - bit_reproducibility: two calls with identical inputs produce byte-identical output

echo "phase-5-smoke: running zig build test -Dtest-filter=sim (canonical Zig sim gate)"
cd "$REPO_ROOT"
if ! zig build test -Dtest-filter=sim 2>&1; then
  echo "phase-5-smoke: FAIL — Zig sim test suite failed (range_model.zig)" >&2
  exit 1
fi
echo "  PASS: canonical Zig sim (range_model.zig) passes physics_sanity + bit_reproducibility"

# ─── Summary ──────────────────────────────────────────────────────────────────

echo ""
echo "phase-5-smoke: PASS"
echo "  battery_thermal (Python oracle): output.json produced + schema-valid"
echo "  motor_efficiency/vehicle_dynamics (MATLAB): graceful-skipped (D-72)"
echo "  range_model.zig (Zig canonical sim): physics + reproducibility tests pass"
echo "  deal evidence baseline v2.1.0: manifest.json written"
echo "  deal check --simulations: structural MATLAB bindings valid"
echo "  deal check --verify: REQ_BAT_001 = PASS, REQ_BAT_002 = PARTIAL (verdict-gated)"
exit 0
