#!/usr/bin/env bash
# Shared helpers for attempt.sh and gate.sh. Source it; do not execute it.
set -euo pipefail

RESEARCH_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(git -C "$RESEARCH_DIR" rev-parse --show-toplevel)"
SCRIPTS="$RESEARCH_DIR/scripts"
EXP_DIR="$RESEARCH_DIR/experiments"
WORKTREES="$REPO_ROOT/.worktrees"

# Prefixes are matched at the repo root. The Cargo/toolchain entries stop an attempt from switching
# off or redirecting the verifier (a `.cargo/config.toml` runner override, a `[[test]]` redirect in
# Cargo.toml, a build.rs); `rust-toolchain` also covers `rust-toolchain.toml`.
PROTECTED_PATHS=("research/scripts/" "research/config.toml" "tests/" "lean/"
                 ".cargo/" "Cargo.toml" "Cargo.lock" "build.rs" "rust-toolchain")

die() { echo "error: $*" >&2; exit 1; }

cfg() { python3 "$SCRIPTS/cfg.py" get "$1"; }

# weakened_tests BASE HEAD: print a line if the diff removes more `#[test]` functions or `assert`
# lines from src/ than it adds, so an attempt cannot pass the gate by deleting the unit tests that
# would catch its change. Tests live inside src/ files, so a path prefix cannot protect them.
# Fails (non-zero) if git cannot compute the diff.
weakened_tests() {
  local d t_rm t_add a_rm a_add
  d=$(git -C "$REPO_ROOT" diff --no-renames -U0 "$1" "$2" -- src) || return 1
  t_rm=$(grep -c '^-[[:space:]]*#\[test\]' <<<"$d" || true)
  t_add=$(grep -c '^+[[:space:]]*#\[test\]' <<<"$d" || true)
  a_rm=$(grep -cE '^-[[:space:]]*(debug_)?assert(_eq|_ne)?!' <<<"$d" || true)
  a_add=$(grep -cE '^\+[[:space:]]*(debug_)?assert(_eq|_ne)?!' <<<"$d" || true)
  if (( t_rm > t_add )); then echo "removes $((t_rm - t_add)) #[test] function(s) from src/"; fi
  if (( a_rm > a_add + 3 )); then echo "removes $((a_rm - a_add)) assert lines from src/"; fi
}

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
