#!/usr/bin/env bash
#
# scripts/phase-3-smoke.sh — Phase 3 integration smoke.
#
# Step 8 of `zig build phase-3-gate` (RESEARCH section 13 lines 955-966).
# Builds deal-lsp + lsp-smoke from source, then runs lsp-smoke against the
# 19-file showcase to exercise the 5 LSP capabilities + semantic tokens.
#
# Exit 0 on full pass; non-zero on any capability failure.
#
# Why this script is the final integration gate:
# ----------------------------------------------
# Individual Rust unit tests (lsp/tests/showcase.rs — 8 tests from Plans
# 03 + 04 + 05) cover each capability in isolation. This script is the
# "everything together" check: spawn the full Backend stack, walk the
# entire workspace, exercise all 5 capabilities + semantic tokens against
# the canonical battery.deal showcase. If any wiring regresses (e.g., the
# eager_parse task fails to populate the index), this script catches it
# before the gate goes green.
#
# Argument-injection note (T-3-07):
# ---------------------------------
# This script takes NO arguments — the showcase path is hardcoded as
# `tests/showcase` (the symlink to ../spec/examples/showcase mounted by
# the spec/ submodule). No untrusted input.

set -euo pipefail

# Resolve the deal/ repo root from this script's location.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

# Verify the showcase symlink/submodule is materialized — same sanity check
# the verify-fresh-worktree.sh script does for the ephemeral worktree case.
if [ ! -f "tests/showcase/packages/vehicle/battery.deal" ]; then
  echo "phase-3-smoke: FATAL — tests/showcase/packages/vehicle/battery.deal not found." >&2
  echo "  The spec/ submodule must be initialized (git submodule update --init --recursive)." >&2
  exit 6
fi

echo "phase-3-smoke: building deal-lsp + lsp-smoke (release)"
cargo build --release -p deal-lsp -p lsp-smoke

echo "phase-3-smoke: running lsp-smoke against tests/showcase"
cargo run --release -p lsp-smoke -- tests/showcase

echo "phase-3-smoke: PASS"
exit 0
