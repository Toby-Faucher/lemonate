#!/usr/bin/env bash
# Shared helpers for attempt.sh and gate.sh. Source it; do not execute it.
set -euo pipefail

RESEARCH_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(git -C "$RESEARCH_DIR" rev-parse --show-toplevel)"
SCRIPTS="$RESEARCH_DIR/scripts"
EXP_DIR="$RESEARCH_DIR/experiments"
WORKTREES="$REPO_ROOT/.worktrees"

PROTECTED_PATHS=("research/scripts/" "research/config.toml" "tests/" "lean/")

die() { echo "error: $*" >&2; exit 1; }

cfg() { python3 "$SCRIPTS/cfg.py" get "$1"; }

# touched_protected BASE HEAD: print changed paths under a protected prefix, one per line.
touched_protected() {
  local changed p
  changed=$(git -C "$REPO_ROOT" diff --name-only "$1" "$2")
  for p in "${PROTECTED_PATHS[@]}"; do
    awk -v p="$p" 'index($0, p) == 1' <<<"$changed"
  done
}
