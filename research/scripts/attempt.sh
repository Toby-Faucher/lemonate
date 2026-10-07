#!/usr/bin/env bash
# attempt.sh NNNN-slug: create a worktree and experiment record, print the agent prompt.
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

id=${1:-}
[[ $id =~ ^[0-9]{4}-[a-z0-9]+(-[a-z0-9]+)*$ ]] || die "usage: attempt.sh NNNN-slug (e.g. 0001-aspiration-windows)"
exp="$EXP_DIR/$id"
wt="$WORKTREES/$id"
[[ ! -e $exp ]] || die "experiment $id already exists"

baseline=$(git -C "$REPO_ROOT" rev-parse "$(cfg repo.baseline_branch)")
mkdir -p "$exp" "$WORKTREES"
git -C "$REPO_ROOT" worktree add -q -b "exp/$id" "$wt" "$baseline"
# Gitignored assets that tests need (bins/Perfect2021.bin) are absent from a fresh worktree.
[[ -d $REPO_ROOT/bins ]] && ln -s "$REPO_ROOT/bins" "$wt/bins"
echo "$baseline" > "$exp/baseline_commit"
cat > "$exp/hypothesis.md" <<'EOF'
# <one-line hypothesis>

## Change

## Mechanism

## Expected Elo
EOF

"$SCRIPTS/catalog.sh" >/dev/null

sed -e "s|{{ID}}|$id|g" \
    -e "s|{{WORKTREE}}|$wt|g" \
    -e "s|{{EXPERIMENT}}|$exp|g" \
    -e "s|{{REPO}}|$REPO_ROOT|g" \
    -e "s|{{RESEARCH}}|$RESEARCH_DIR|g" \
    "$RESEARCH_DIR/agent-prompt.md"
