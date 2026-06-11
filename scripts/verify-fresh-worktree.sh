#!/usr/bin/env bash
#
# scripts/verify-fresh-worktree.sh <gate-step>
#
# Usage
# -----
#   verify-fresh-worktree.sh <gate-step>
#
# Arguments:
#   <gate-step>   The zig build step to run inside the ephemeral worktree.
#                 Examples: phase-1.5-gate, phase-2-gate, phase-3-gate, ...
#                 This argument is REQUIRED. If omitted the script exits 2 with
#                 a usage message.
#
# Purpose
# -------
# Phase-exit verification gate that mechanically exercises the committed git
# state — and ONLY the committed git state — by creating an ephemeral git
# worktree, running `git submodule update --init --recursive` inside it, and
# running `zig build <gate-step>` from the ephemeral tree. The developer's
# main checkout's untracked or locally-modified files cannot influence the
# result.
#
# Why this script exists
# ----------------------
# Phase 1.5's original GREEN claim (.planning/phases/01.5-.../01.5-VERIFICATION.md
# verified=2026-05-20T22:22:38Z) was made against the developer's main checkout,
# where `tests/showcase` existed as an UNTRACKED symlink to a sibling git repo
# (`/Users/dunnock/projects/deal-lang/spec`). On any fresh clone or worktree,
# 9 of the 33 unit tests failed with `error.FileNotFound`. See:
#   - .planning/phases/01.5-parser-strictness-test-gap-resolution-inserted/01.5-UAT.md
#   - .planning/debug/phase-1-5-gate-failure.md
#   - .planning/decisions/ADR-phase-1.5-fresh-worktree-verification.md
#
# Plan 01.5-05 introduced (a) a spec/ submodule + committed tests/showcase
# symlink so the showcase corpus is reproducibly available, and (b) THIS
# SCRIPT as a structural guard rail preventing recurrence of the false-GREEN
# class of bug.
#
# Plan 02-06 generalized the script to accept a gate-step argument so every
# phase (2..6) can reuse the same script with its own gate step name.
#
# Operational notes
# -----------------
# - The script refuses to run on a dirty main tree because the gate verifies
#   COMMITTED state. Running on a dirty tree would mask the exact bug class
#   we're guarding against.
# - The ephemeral worktree is created under $TMPDIR (mktemp -d) and removed
#   on EXIT (success or failure) via a `trap`. A periodic `git worktree prune`
#   in the main tree cleans stale registrations if the script is hard-killed.
# - The script invokes `zig build "$1"` with the caller-supplied gate step;
#   NEVER pass the -fresh sibling (that would be infinite recursion: the build
#   step shells out to this script, and this script would call back into the
#   build step that called it).
#
# Argument injection note (T-02-38)
# ----------------------------------
# "$1" is quoted throughout. Valid gate-step names are alphanumeric + hyphen only.
# If $1 contains shell metacharacters, `zig build` will reject the step name
# harmlessly (zig does not execute unknown step names).

set -euo pipefail

# Note: IFS is intentionally left at its default. All variable expansions in
# this script are quoted ("$VAR"), so word-splitting behavior is irrelevant.

# 0. Parse and validate the gate-step argument.
if [ -z "${1:-}" ]; then
  echo "verify-fresh-worktree: FATAL — missing required argument <gate-step>." >&2
  echo "  Usage: verify-fresh-worktree.sh <gate-step>" >&2
  echo "  Example: verify-fresh-worktree.sh phase-2-gate" >&2
  echo "  Known values: phase-1.5-gate, phase-2-gate, phase-3-gate, phase-4-gate, phase-5-gate, phase-05.2-wave0-gate, ..." >&2
  exit 2
fi
GATE_STEP="$1"

# 1. Resolve the deal repo root from this script's location.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

if [ ! -e "$REPO_ROOT/.git" ]; then
  echo "verify-fresh-worktree: FATAL — $REPO_ROOT/.git does not exist; not a git repo" >&2
  exit 2
fi

# 2. Reject dirty main tree.
if ! git -C "$REPO_ROOT" diff --quiet || ! git -C "$REPO_ROOT" diff --cached --quiet; then
  echo "verify-fresh-worktree: FATAL — main tree at $REPO_ROOT has uncommitted changes." >&2
  echo "  The fresh-worktree gate verifies COMMITTED state; running on a dirty tree" >&2
  echo "  would mask the false-GREEN class of bug this gate exists to prevent." >&2
  echo "  Commit or stash your changes (without using git stash inside a Claude" >&2
  echo "  worktree — see CLAUDE.md) and re-run." >&2
  exit 3
fi

# 3. Create ephemeral worktree.
WORKTREE_DIR="$(mktemp -d -t deal-verify-XXXXXX)"
# Trap removes the worktree on EXIT (success and failure) AND cleans up its
# filesystem directory. The `|| true` on `worktree remove` avoids tripping
# `set -e` when the trap runs after worktree-add has not yet completed.
trap 'git -C "$REPO_ROOT" worktree remove --force "$WORKTREE_DIR" 2>/dev/null || true; rm -rf "$WORKTREE_DIR"' EXIT

echo "verify-fresh-worktree: creating ephemeral worktree at $WORKTREE_DIR"

# 4. Attach the worktree at HEAD via `git worktree add`.
git -C "$REPO_ROOT" worktree add "$WORKTREE_DIR" HEAD

# 5. Initialize submodules INSIDE the worktree.
#    This is the operation that materializes spec/ and (transitively) the
#    showcase corpus that tests/showcase -> ../spec/examples/showcase reads.
#
# protocol.file.allow=always note (CVE-2022-39253 mitigation):
# Git 2.38.1+ blocks `file://` submodule clones by default to prevent
# malicious .gitmodules pointing at sibling-controlled paths. Some dev
# environments override submodule.spec.url in .git/config to point at a
# sibling working copy (e.g., /Users/dev/projects/deal-lang/spec) for
# fast iteration — that override is benign for this gate (the dev's own
# worktree just spawned this subprocess) but trips the CVE check.
#
# We pass `-c protocol.file.allow=always` SCOPED to this single invocation
# so the global git config stays at its CVE-safe default. The threat model
# (T-3-07): an attacker who can write to the dev's local .git/config can
# already do far worse than smuggle a file:// submodule path; this scoped
# override does not expand that surface.
echo "verify-fresh-worktree: running git submodule update --init --recursive"
if ! git -C "$WORKTREE_DIR" -c protocol.file.allow=always submodule update --init --recursive; then
  echo "verify-fresh-worktree: FAILED — submodule update could not populate" >&2
  echo "  the spec/ submodule. Check network / SSH key / .gitmodules URL." >&2
  exit 4
fi

# 6. Sanity-check showcase materialization.
if [ ! -f "$WORKTREE_DIR/tests/showcase/packages/vehicle/battery.deal" ]; then
  echo "verify-fresh-worktree: FAILED — tests/showcase materialization failed." >&2
  echo "  Expected $WORKTREE_DIR/tests/showcase/packages/vehicle/battery.deal" >&2
  echo "  to exist via the submodule mount. The spec/ submodule did not populate" >&2
  echo "  as expected." >&2
  exit 5
fi

# 6.5. Materialize sibling-repo dependencies for gates that cross repo
#      boundaries (phase-3-gate steps 5-7 invoke `cd ../tree-sitter-deal`,
#      `cd ../vscode-deal` per RESEARCH section 13).
#
# Phase 3 introduced cross-repo gate steps because the editor-intelligence
# deliverable spans 3 repos (deal/, tree-sitter-deal/, vscode-deal/). The
# build.zig commands assume sibling-repo layout — true in the dev's main
# checkout, NOT true in the ephemeral worktree (mktemp -d under TMPDIR).
#
# We symlink the dev's actual sibling repos into the ephemeral worktree's
# parent so `cd ../<sibling>` resolves. The symlink does NOT mutate the
# sibling repos (gate steps only run their own test/package commands which
# do not modify source); if a sibling repo has dev-local uncommitted state,
# that state is what the gate sees — same as in the main-checkout case for
# those sibling repos (the binding ADR-phase-1.5-fresh-worktree-verification
# invariant applies to deal/, not to its siblings, since they are separately
# versioned repositories with their own gates).
#
# On a real CI runner (per release.yml job package-vsix) each sibling is
# fetched via actions/checkout into a known path; the script's local
# equivalent symlinks the dev's working copies, which is operationally
# equivalent for the gate's purpose.
WORKTREE_PARENT="$(dirname "$WORKTREE_DIR")"
DEAL_PARENT="$(cd "$REPO_ROOT/.." && pwd)"
for sibling in tree-sitter-deal vscode-deal deal-stdlib deal-lang.org deal-sim; do
  if [ -d "$DEAL_PARENT/$sibling" ]; then
    ln -sfn "$DEAL_PARENT/$sibling" "$WORKTREE_PARENT/$sibling"
    echo "verify-fresh-worktree: linked $sibling into $WORKTREE_PARENT"
  else
    echo "verify-fresh-worktree: WARNING — sibling repo $sibling not found at $DEAL_PARENT/$sibling" >&2
    echo "  Cross-repo gate steps that reference ../$sibling will fail." >&2
  fi
done
# The symlinks live in $WORKTREE_PARENT (the TMPDIR), which itself is NOT
# removed by the EXIT trap (only $WORKTREE_DIR is). Add explicit cleanup
# of just the symlinks so we don't leak them after a successful run.
trap 'git -C "$REPO_ROOT" worktree remove --force "$WORKTREE_DIR" 2>/dev/null || true; rm -rf "$WORKTREE_DIR"; for s in tree-sitter-deal vscode-deal deal-stdlib deal-lang.org deal-sim; do [ -L "$WORKTREE_PARENT/$s" ] && rm -f "$WORKTREE_PARENT/$s"; done' EXIT

# 6.6. Ensure deal-sim is importable in the ephemeral environment so Python sims
#      can `import deal_sim`. The gate invariant (T-05-19) is *importability*, not a
#      fresh install on every run.
#      Guarded by directory existence — deal-sim may not exist on all dev machines.
#      Phase 5 Plan 01 (D-77): build + local/editable install; defer PyPI publish.
if [ -d "$WORKTREE_PARENT/deal-sim" ]; then
  # Short-circuit if deal_sim already imports (dev already ran `pip install -e
  # ../deal-sim` in their interpreter) — re-installing is unnecessary and, under a
  # PEP-668 externally-managed interpreter (Homebrew Python), would hard-fail.
  if python3 -c "import deal_sim" >/dev/null 2>&1; then
    echo "verify-fresh-worktree: deal_sim already importable — skipping reinstall"
  else
    echo "verify-fresh-worktree: installing deal-sim (editable) from $WORKTREE_PARENT/deal-sim"
    # Use `python3 -m pip` (not bare `pip`): in the non-interactive shell spawned by
    # `zig build`, the dev's `alias pip=pip3` does not expand, so bare `pip` fails with
    # "command not found". The module form is alias-independent and portable.
    #
    # PEP 668: a Homebrew/system interpreter is "externally managed" and refuses a
    # bare editable install. Retry with --break-system-packages --user as the
    # documented override so the gate works on stock macOS Homebrew Python. deal-sim
    # is the local sibling package (D-72/T-05-SC), never a registry install.
    if ! python3 -m pip install -e "$WORKTREE_PARENT/deal-sim" --quiet; then
      echo "verify-fresh-worktree: retrying deal-sim install with PEP-668 override (--break-system-packages --user)" >&2
      python3 -m pip install -e "$WORKTREE_PARENT/deal-sim" --quiet --user --break-system-packages
    fi
  fi
  # Hard assert the gate invariant: deal_sim MUST be importable after this step.
  if ! python3 -c "import deal_sim" >/dev/null 2>&1; then
    echo "verify-fresh-worktree: FATAL — deal_sim not importable after install step" >&2
    exit 1
  fi
  echo "verify-fresh-worktree: deal_sim import OK"
else
  echo "verify-fresh-worktree: WARNING — deal-sim not found; Python sims will fail to import deal_sim" >&2
fi

# 7. Run the gate from the ephemeral worktree.
echo "verify-fresh-worktree: running zig build \"$GATE_STEP\" from $WORKTREE_DIR"
LOG_PATH="$REPO_ROOT/.zig-cache/verify-fresh-worktree.log"
mkdir -p "$(dirname "$LOG_PATH")"

GATE_EXIT=0
( cd "$WORKTREE_DIR" && zig build "$GATE_STEP" ) 2>&1 | tee "$LOG_PATH" || GATE_EXIT=${PIPESTATUS[0]}

if [ "$GATE_EXIT" -ne 0 ]; then
  echo "verify-fresh-worktree: fresh-worktree gate FAILED — exit $GATE_EXIT" >&2
  echo "  Worktree: $WORKTREE_DIR" >&2
  echo "  Log:      $LOG_PATH" >&2
  exit "$GATE_EXIT"
fi

# 8. Success.
echo "verify-fresh-worktree: fresh-worktree gate PASSED"
echo "  Worktree: $WORKTREE_DIR (will be removed by EXIT trap)"
echo "  Log:      $LOG_PATH"
exit 0
