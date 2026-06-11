#!/usr/bin/env bash
#
# scripts/phase-4-smoke.sh — Phase 4 E2E integration smoke.
#
# REQ-phase-4-gate acceptance evidence: the D-69 new-user moment end-to-end.
# Exercises the full new-user flow: deal init → deal install → deal check →
# deal build --target sysml-v2 → deal build --target reqif, asserting each
# step exits 0 and produces expected artefacts.
#
# Air-gap safety (T-4-23 mitigation):
#   The `deal init` starter model declares deal-std as a git dep pointing at
#   github.com.  This smoke overrides that to a local file:// URL pointing at
#   ../deal-stdlib (the sibling repo) so `deal install` resolves offline and
#   deterministically — no network access required.
#
# Argument injection note:
#   This script takes NO arguments.  DEAL_BIN is the only external input and
#   it is validated (must be executable) before use.
#
# Exit codes:
#   0  — all steps pass
#   1  — a step failed (non-zero exit or expected artefact absent)
#   2  — pre-flight failure (deal binary not found / build failed)
#   6  — deal-stdlib sibling not found

set -euo pipefail

# ── Locate repos ──────────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

# Locate the deal-stdlib sibling repo (parallel to the deal/ checkout).
DEAL_STDLIB_PATH="$(cd "$REPO_ROOT/../deal-stdlib" 2>/dev/null && pwd)" || {
  echo "phase-4-smoke: FATAL — sibling repo deal-stdlib not found at ${REPO_ROOT}/../deal-stdlib" >&2
  echo "  deal-stdlib must be checked out as a sibling of the deal/ repo." >&2
  exit 6
}
echo "phase-4-smoke: using deal-stdlib at $DEAL_STDLIB_PATH"

# ── Build / locate the deal binary ────────────────────────────────────────────

DEAL_BIN="${DEAL_BIN:-$REPO_ROOT/target/release/deal}"

if [ ! -x "$DEAL_BIN" ]; then
  echo "phase-4-smoke: deal binary not found at $DEAL_BIN — building (release)..."
  cargo build --release -p deal --manifest-path "$REPO_ROOT/Cargo.toml" 2>&1
  if [ ! -x "$DEAL_BIN" ]; then
    echo "phase-4-smoke: FATAL — cargo build succeeded but binary still missing at $DEAL_BIN" >&2
    exit 2
  fi
fi
echo "phase-4-smoke: using deal binary at $DEAL_BIN"

# ── Ephemeral temp dir + EXIT cleanup ─────────────────────────────────────────

WORK_DIR="$(mktemp -d -t deal-smoke-XXXXXX)"
trap 'rm -rf "$WORK_DIR"' EXIT
echo "phase-4-smoke: working dir $WORK_DIR (removed on exit)"

# ── Step 1: deal init smoke-proj ──────────────────────────────────────────────

PROJ="smoke-proj"
PROJ_DIR="$WORK_DIR/$PROJ"

echo ""
echo "phase-4-smoke: step 1 — deal init $PROJ"
(cd "$WORK_DIR" && "$DEAL_BIN" init "$PROJ") 2>&1
if [ ! -f "$PROJ_DIR/deal.toml" ]; then
  echo "phase-4-smoke: FAIL — step 1: $PROJ_DIR/deal.toml not created" >&2
  exit 1
fi
# Assert deal.toml contains the deal-std git dependency declaration (D-67).
if ! grep -q 'deal-std' "$PROJ_DIR/deal.toml"; then
  echo "phase-4-smoke: FAIL — step 1: deal.toml missing deal-std dependency" >&2
  exit 1
fi
if ! grep -q 'git' "$PROJ_DIR/deal.toml"; then
  echo "phase-4-smoke: FAIL — step 1: deal.toml missing git dep key" >&2
  exit 1
fi
echo "phase-4-smoke: step 1 PASS — deal.toml created with deal-std git dep"

# ── Override deal-std dep to use file:// local path (T-4-23 / air-gap) ────────
#
# The scaffolded deal.toml points at https://github.com/deal-lang/deal-stdlib.
# We rewrite it to point at the local sibling via a file:// URL so the smoke
# does not require network access (air-gap-safe, deterministic).
#
# The file:// scheme is in the resolver's accepted-scheme allowlist (T-4-03).

STDLIB_FILE_URL="file://${DEAL_STDLIB_PATH}"
STDLIB_TAG="v0.4.0"

# Build the overridden deal.toml with the local file:// path.
printf '[project]\nname = "%s"\nversion = "0.1.0"\nschema = "deal/0.1"\nmarking = "Unclassified"\ndescription = "A DEAL project"\n\n[workspace]\npackages = ["packages/*"]\n\n[dependencies]\ndeal-std = { git = "%s", tag = "%s" }\n' \
  "$PROJ" "$STDLIB_FILE_URL" "$STDLIB_TAG" > "$PROJ_DIR/deal.toml"

echo "phase-4-smoke: deal-std dep overridden to file:// local path (air-gap-safe)"

# ── Step 2: deal install ───────────────────────────────────────────────────────

echo ""
echo "phase-4-smoke: step 2 — deal install"
(cd "$PROJ_DIR" && "$DEAL_BIN" install) 2>&1
if [ ! -d "$PROJ_DIR/.deal/deps/deal-std" ]; then
  echo "phase-4-smoke: FAIL — step 2: .deal/deps/deal-std/ not populated after install" >&2
  exit 1
fi
if [ ! -f "$PROJ_DIR/deal.lock" ]; then
  echo "phase-4-smoke: FAIL — step 2: deal.lock not created after install" >&2
  exit 1
fi
echo "phase-4-smoke: step 2 PASS — .deal/deps/deal-std/ populated, deal.lock written"

# ── Step 3: deal check ────────────────────────────────────────────────────────
#
# D-69: deal check must pass immediately after deal init + deal install.
# The starter model (packages/starter.deal) has no deal.std imports so it
# does not require cross-file name resolution — the check passes cleanly.

echo ""
echo "phase-4-smoke: step 3 — deal check packages/"
if ! (cd "$PROJ_DIR" && "$DEAL_BIN" check packages/) 2>&1; then
  echo "phase-4-smoke: FAIL — step 3: deal check exited non-zero" >&2
  exit 1
fi
echo "phase-4-smoke: step 3 PASS — deal check exit 0 (D-69)"

# ── Step 4: deal build --target sysml-v2 ──────────────────────────────────────

echo ""
echo "phase-4-smoke: step 4 — deal build --target sysml-v2 packages/"
if ! (cd "$PROJ_DIR" && "$DEAL_BIN" build --target sysml-v2 packages/) 2>&1; then
  echo "phase-4-smoke: FAIL — step 4: deal build --target sysml-v2 exited non-zero" >&2
  exit 1
fi
# Assert at least one .json output was produced.
SYSML_JSON=$(find "$PROJ_DIR" -name "*.sysml-v2.json" 2>/dev/null | head -1)
if [ -z "$SYSML_JSON" ]; then
  echo "phase-4-smoke: FAIL — step 4: no *.sysml-v2.json output found" >&2
  exit 1
fi
echo "phase-4-smoke: step 4 PASS — SysML v2 JSON produced: $SYSML_JSON"

# ── Step 5: deal build --target reqif ─────────────────────────────────────────

echo ""
echo "phase-4-smoke: step 5 — deal build --target reqif packages/"
if ! (cd "$PROJ_DIR" && "$DEAL_BIN" build --target reqif packages/) 2>&1; then
  echo "phase-4-smoke: FAIL — step 5: deal build --target reqif exited non-zero" >&2
  exit 1
fi
# Assert a .reqifz archive was produced.
REQIFZ=$(find "$PROJ_DIR" -name "*.reqifz" 2>/dev/null | head -1)
if [ -z "$REQIFZ" ]; then
  echo "phase-4-smoke: FAIL — step 5: no *.reqifz output found" >&2
  exit 1
fi
echo "phase-4-smoke: step 5 PASS — ReqIF archive produced: $REQIFZ"

# ── All steps passed ──────────────────────────────────────────────────────────

echo ""
echo "phase-4-smoke: PASS — all 5 steps (init → install → check → sysml-v2 → reqif) passed"
exit 0
