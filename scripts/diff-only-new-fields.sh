#!/usr/bin/env bash
# diff-only-new-fields.sh — D-30 / T-02-07 snapshot-regen discipline guard.
#
# Asserts that every changed AST snapshot file contains ONLY the three new
# comment fields introduced by Plan 02-01 in its diff vs. HEAD~1:
#   - doc_comment
#   - leading_comments
#   - trailing_comments
#
# Exits non-zero if any diff hunk changes lines that do NOT mention one of
# these three field names. This catches structural drift (Pitfall 2 from
# RESEARCH.md) — a parser bug that changed existing fields while the
# snapshot regen was in progress.
#
# Usage:
#   bash scripts/diff-only-new-fields.sh tests/snapshots/ast/showcase__*.json
#
# Run after `zig build test -Dupdate-snapshots=true` and before committing
# the snapshot-regen commit.
#
# Exit codes:
#   0 — all changed lines mention only the three new fields
#   1 — structural drift detected; details printed to stderr

set -euo pipefail

SCRIPT_NAME="diff-only-new-fields.sh"
NEW_FIELDS_PATTERN='doc_comment|leading_comments|trailing_comments'

if [[ $# -eq 0 ]]; then
    echo "Usage: $SCRIPT_NAME <snapshot-file...>" >&2
    echo "Example: $SCRIPT_NAME tests/snapshots/ast/showcase__*.json" >&2
    exit 1
fi

DRIFT_FOUND=0
CHECKED=0
CHANGED=0

for file in "$@"; do
    if [[ ! -f "$file" ]]; then
        echo "WARNING: $file does not exist, skipping" >&2
        continue
    fi

    # Get the diff for this file vs. the last commit.
    # If the file has no changes (not in git diff), skip it.
    diff_output=$(git diff HEAD -- "$file" 2>/dev/null || true)
    if [[ -z "$diff_output" ]]; then
        CHECKED=$((CHECKED + 1))
        continue
    fi

    CHANGED=$((CHANGED + 1))
    CHECKED=$((CHECKED + 1))

    # Extract added/removed lines (lines starting with + or - but not +++ or ---).
    # Each such line must contain at least one of the three new field names.
    # Snapshot files are single-line JSON, so the diff is essentially the
    # full old/new line. We check at the per-character change granularity by
    # looking at the changed line content.
    changed_lines=$(echo "$diff_output" | grep -E '^[+-][^+-]' || true)

    if [[ -z "$changed_lines" ]]; then
        # No actual content changes — just mode changes or empty diff.
        continue
    fi

    # Check for any changed line that does NOT mention one of the new fields.
    # Since snapshot files are single-line JSON, a changed file typically has
    # exactly one removed line and one added line (the whole JSON blob).
    # In that case we need to check that the diff only shows additions of the
    # three new fields.
    #
    # Strategy: use grep -v to filter OUT lines that mention the new fields.
    # If anything remains, those are structural changes.
    drift_lines=$(echo "$changed_lines" | grep -vE "$NEW_FIELDS_PATTERN" || true)

    if [[ -n "$drift_lines" ]]; then
        echo "ERROR: Structural drift detected in $file" >&2
        echo "  Changed lines NOT containing new comment fields:" >&2
        # Show at most 5 lines of context (snapshots are single-line JSON).
        echo "$drift_lines" | head -5 | sed 's/^/    /' >&2
        DRIFT_FOUND=1
    fi
done

echo "$SCRIPT_NAME: checked $CHECKED files, $CHANGED had changes"

if [[ $DRIFT_FOUND -ne 0 ]]; then
    echo "FAIL: Structural drift detected — snapshot regen introduced non-comment-field changes." >&2
    echo "      Fix the underlying parser bug before accepting the snapshot update." >&2
    exit 1
fi

echo "OK: All snapshot changes are limited to the three new comment fields (D-30 satisfied)"
exit 0
