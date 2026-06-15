#!/usr/bin/env bash
# spec-bump.sh — advance the `spec` submodule to its origin/main and stage the gitlink.
#
# Phase-1 Option B workflow (single source of truth):
#   1. Author grammar / examples / IR in the ROOT `spec/` working clone.
#   2. Commit + push there to origin/main.
#   3. Run this script from the `deal/` repo to consume the new spec.
#   4. Regenerate + REVIEW goldens, run tests, then commit the staged gitlink.
#
# The companion CI guard (.github/workflows/spec-freshness.yml) fails the build
# if the pinned submodule ever falls behind origin/main, so drift can't go silent.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"   # deal/ repo root

echo "==> Fetching + updating spec submodule to origin/main…"
git submodule update --remote --checkout spec

NEW=$(git -C spec rev-parse --short HEAD)
echo "==> spec now at: $(git -C spec log --oneline -1)"

git add spec
echo
echo "Staged gitlink bump to ${NEW}."
echo "NEXT:"
echo "  1) regenerate + REVIEW goldens (gen_golden / UPDATE_GOLDEN)"
echo "  2) cargo test --workspace && zig build test"
echo "  3) git commit -m \"deps: bump spec submodule to ${NEW}\""
