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
# Fails (non-zero) if git cannot compute the diff, so callers can fail closed.
touched_protected() {
  local tmpf p q
  tmpf=$(mktemp) || return 1
  # -z and core.quotePath=false: paths with non-ASCII bytes are neither git-quoted nor split.
  if ! git -C "$REPO_ROOT" -c core.quotePath=false diff --no-renames --name-only -z "$1" "$2" >"$tmpf"; then
    rm -f "$tmpf"
    return 1
  fi
  while IFS= read -r -d '' p; do
    for q in "${PROTECTED_PATHS[@]}"; do
      if [[ $p == "$q"* ]]; then printf '%s\n' "$p"; break; fi
    done
  done <"$tmpf"
  rm -f "$tmpf"
}
