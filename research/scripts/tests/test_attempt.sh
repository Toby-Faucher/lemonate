#!/usr/bin/env bash
source "$(dirname "${BASH_SOURCE[0]}")/flow_common.sh"

base=$(git -C "$repo" rev-parse main)

out=$("$R/scripts/attempt.sh" 0001-demo)
assert_eq "$(<"$R/experiments/0001-demo/baseline_commit")" "$base" "baseline_commit"
[[ -d "$repo/.worktrees/0001-demo/src" ]] || fail "worktree missing"
[[ -L "$repo/.worktrees/0001-demo/bins" ]] || fail "bins/ should be symlinked into the worktree"
assert_eq "$(git -C "$repo/.worktrees/0001-demo" status --porcelain)" "" "symlinked bins must not dirty the worktree"
assert_eq "$(git -C "$repo/.worktrees/0001-demo" branch --show-current)" "exp/0001-demo" "branch"
assert_eq "$(git -C "$repo/.worktrees/0001-demo" rev-parse HEAD)" "$base" "worktree at baseline"
grep -q '^# <one-line hypothesis>' "$R/experiments/0001-demo/hypothesis.md" || fail "hypothesis stub"
grep -qF "$repo/.worktrees/0001-demo" <<<"$out" || fail "prompt should name the worktree path"
grep -qF "$R/experiments/0001-demo/trace.md" <<<"$out" || fail "prompt should name the trace path"
grep -q '{{' <<<"$out" && fail "unfilled placeholder in prompt"

if "$R/scripts/attempt.sh" 0001-demo >/dev/null 2>&1; then fail "duplicate id should fail"; fi
if "$R/scripts/attempt.sh" bad_id >/dev/null 2>&1; then fail "bad id should fail"; fi
if "$R/scripts/attempt.sh" 12-short >/dev/null 2>&1; then fail "short number should fail"; fi

# A rename out of a protected path must still be flagged (git lists only the new path unless --no-renames).
git -C "$repo" checkout -q -b rename-test
git -C "$repo" mv tests/t.rs src/t.rs
git -C "$repo" commit -qm "move test out of tests/"
source "$R/scripts/lib.sh"
assert_eq "$(touched_protected main rename-test)" "tests/t.rs" "rename out of tests/ is flagged"
git -C "$repo" checkout -q main

# touched_protected fails closed on an unresolvable ref.
if touched_protected main deadbeefdeadbeefdeadbeefdeadbeefdeadbeef >/dev/null 2>&1; then fail "bad ref should make touched_protected fail"; fi

# Non-ASCII protected paths are reported verbatim, not git-quoted.
git -C "$repo" checkout -q -b utf-test
echo x > "$repo/tests/é.rs"
git -C "$repo" add tests
git -C "$repo" commit -qm "utf8"
assert_eq "$(touched_protected main utf-test)" "tests/é.rs" "non-ASCII protected path is flagged"
git -C "$repo" checkout -q main

# attempt.sh makes the experiment visible in the catalog right away.
grep -q '0001-demo' "$R/CATALOG.md" || fail "attempt.sh should add a pending catalog row"
grep -q 'Pending' "$R/CATALOG.md" || fail "pending section missing"

echo "test_attempt: OK"
