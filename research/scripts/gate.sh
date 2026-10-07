#!/usr/bin/env bash
# gate.sh <id>: preflight, local build/test/perft, remote SPRT, record result.json.
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

id=${1:-}
[[ $id =~ ^[0-9]{4}-[a-z0-9]+(-[a-z0-9]+)*$ ]] || die "usage: gate.sh NNNN-slug (invalid id: '$id')"
exp="$EXP_DIR/$id"
wt="$WORKTREES/$id"
[[ -f $exp/baseline_commit ]] || die "usage: gate.sh <id> (no such experiment: '$id')"
[[ -d $wt ]] || die "worktree $wt not found"
[[ -z $(git -C "$wt" status --porcelain) ]] || die "worktree has uncommitted changes; commit them first"

# Per-run artefacts from an earlier run must not sit beside (or stand in for) a new result.
rm -f "$exp"/{result.json,patch.diff,sprt.log,sprt.json,sprt.err,parse.err,games.pgn} \
      "$exp"/{build,test,perft}.log "$exp"/remote-build-*.log

baseline=$(<"$exp/baseline_commit")
candidate=$(git -C "$wt" rev-parse HEAD)
b12=${baseline:0:12}
c12=${candidate:0:12}
cfgh=$(python3 "$SCRIPTS/cfg.py" hash)

declare -A gate=([preflight]=skipped [build]=skipped [test]=skipped [perft]=skipped [sprt]=skipped)
reason=""
infra=0
sprt_file=""

# record: write patch.diff and result.json, refresh the catalog, print the outcome.
record() {
  git -C "$wt" diff "$baseline" "$candidate" > "$exp/patch.diff" 2>/dev/null || : > "$exp/patch.diff"
  local args=(--out "$exp/result.json" --baseline "$baseline" --candidate "$candidate"
              --config-hash "$cfgh")
  local s
  for s in "${!gate[@]}"; do args+=(--gate "$s=${gate[$s]}"); done
  if [[ -n $sprt_file ]]; then args+=(--sprt-file "$sprt_file"); fi
  if [[ -n $reason ]]; then args+=(--reason "$reason"); fi
  if (( infra )); then args+=(--infra-error); fi
  python3 "$SCRIPTS/result.py" "${args[@]}"
  "$SCRIPTS/catalog.sh"
  jq -r '"\(.status)  \(.reason)"' "$exp/result.json"
}

# --- 1. Preflight: an attempt must not touch the verifier. ---
preflight_fail() {
  gate[preflight]=fail
  reason="$1"
  record; exit 0
}
baseline_branch=$(cfg repo.baseline_branch)
[[ $baseline =~ ^[0-9a-f]{40}$ ]] || preflight_fail "baseline_commit is not a full commit sha"
git -C "$REPO_ROOT" cat-file -e "$baseline^{commit}" 2>/dev/null \
  || preflight_fail "baseline commit $b12 does not exist in the repository"
git -C "$REPO_ROOT" merge-base --is-ancestor "$baseline" "$candidate" \
  || preflight_fail "ancestry: candidate does not descend from the recorded baseline $b12"
branch_tip=$(git -C "$REPO_ROOT" rev-parse --verify "$baseline_branch" 2>/dev/null) \
  || preflight_fail "baseline branch '$baseline_branch' does not resolve"
git -C "$REPO_ROOT" merge-base --is-ancestor "$baseline" "$branch_tip" \
  || preflight_fail "ancestry: recorded baseline $b12 is not on branch $baseline_branch"
merges=$(git -C "$REPO_ROOT" rev-list --merges "$baseline..$candidate") \
  || preflight_fail "could not list commits $b12..$c12"
[[ -z $merges ]] || preflight_fail "candidate contains merge commits (merged another branch?)"
range_all=$(git -C "$REPO_ROOT" rev-list "$baseline..$candidate") \
  || preflight_fail "could not list commits $b12..$c12"
range_own=$(git -C "$REPO_ROOT" rev-list "$baseline..$candidate" "^$branch_tip") \
  || preflight_fail "could not list commits $b12..$c12"
[[ $range_all == "$range_own" ]] || preflight_fail "candidate contains commits already on $baseline_branch (rebased or fast-forwarded onto it?)"
bad=$(touched_protected "$baseline" "$candidate") \
  || preflight_fail "could not compute diff $b12..$c12 for the protected-path check"
if [[ -n $bad ]]; then
  preflight_fail "touches protected paths: $(tr '\n' ' ' <<<"$bad")"
fi
gate[preflight]=pass

# --- 2-3. Local build, tests, perft (inside the candidate worktree). ---
run_stage() { # name command
  if (cd "$wt" && bash -c "$2") >"$exp/$1.log" 2>&1; then
    gate[$1]=pass
  else
    gate[$1]=fail
    reason="$1 failed (see $1.log)"
    record; exit 0
  fi
}
build_cmd=$(cfg gate.build_cmd)
run_stage build "$build_cmd"
run_stage test "$(cfg gate.test_cmd)"
run_stage perft "$(cfg gate.perft_cmd)"

# --- 4. Ship the committed candidate and a cached baseline to the container. ---
host=$(cfg remote.host)
work=$(cfg remote.workdir)
engines=$(cfg remote.engines_dir)
lock=$(cfg remote.lock)
ssh_() { ${RESEARCH_SSH:-ssh} "$@"; }

infra_fail() {
  gate[sprt]=error
  infra=1
  reason="$1"
  record; exit 0
}

# The container's rustc identity is part of the cache key: a toolchain update invalidates binaries.
rustc_info=$(ssh_ "$host" ". ~/.cargo/env; rustc -vV") || infra_fail "could not query rustc on $host"
[[ -n ${rustc_info//[[:space:]]/} ]] || infra_fail "rustc -vV on $host returned nothing"
tc=$(sha256sum <<<"$rustc_info" | cut -c1-12)
base_name="base-$b12-$cfgh-$tc"
cand_name="cand-$c12-$cfgh-$tc"

# ship_and_build COMMIT NAME: ship the commit and build it into the engines dir, all under the
# remote lock so a build never disturbs a running match. The cache is re-checked inside the lock;
# the binary is smoke-tested and moved into place atomically.
ship_and_build() {
  local dir="$work/$2" q_dir q_eng q_bin q_tmp script
  printf -v q_dir '%q' "$dir"
  printf -v q_eng '%q' "$engines"
  printf -v q_bin '%q' "$engines/$2"
  printf -v q_tmp '%q' "$engines/$2.tmp"
  script="if [ -x $q_bin ]; then cat >/dev/null; exit 0; fi; "
  script+="rm -rf $q_dir && mkdir -p $q_dir && tar -x -C $q_dir && . ~/.cargo/env && cd $q_dir && $build_cmd"
  script+=" && mkdir -p $q_eng && cp target/release/lemonate $q_tmp"
  script+=" && printf 'uci\\nquit\\n' | timeout 10 $q_tmp | grep -q uciok && mv -f $q_tmp $q_bin"
  git -C "$REPO_ROOT" archive "$1" \
    | ssh_ "$host" "flock $(printf '%q' "$lock") bash -c $(printf '%q' "$script")" \
    >"$exp/remote-build-$2.log" 2>&1 \
    || infra_fail "ship/build of $2 failed (see remote-build-$2.log)"
}

ssh_ "$host" "test -x '$engines/$base_name'" || ship_and_build "$baseline" "$base_name"
ship_and_build "$candidate" "$cand_name"

# --- 5. SPRT on the container, one match at a time (flock). ---
pgn="$work/$id.pgn"
match="rm -f '$pgn'; flock '$lock' cutechess-cli \
  -engine name=new cmd='$engines/$cand_name' proto=uci \
  -engine name=old cmd='$engines/$base_name' proto=uci \
  -each tc=$(cfg match.tc) -games 2 -repeat -rounds $(( $(cfg match.game_cap) / 2 )) \
  -concurrency $(cfg match.concurrency) \
  -openings file='$(cfg match.book)' format=epd order=random \
  -resign movecount=3 score=400 -draw movenumber=40 movecount=8 score=10 \
  -sprt elo0=$(cfg sprt.elo0) elo1=$(cfg sprt.elo1) alpha=$(cfg sprt.alpha) beta=$(cfg sprt.beta) \
  -pgnout '$pgn'"
# The match's exit status does not decide the outcome: the parsed output does.
rc=0
ssh_ "$host" ". ~/.cargo/env; $match" >"$exp/sprt.log" 2>"$exp/sprt.err" || rc=$?
if ! python3 "$SCRIPTS/sprt_parse.py" <"$exp/sprt.log" >"$exp/sprt.json" 2>"$exp/parse.err"; then
  rm -f "$exp/sprt.json"
  infra_fail "could not parse cutechess output; match exited $rc (see sprt.log, sprt.err, parse.err)"
fi
ssh_ "$host" "cat '$pgn'" >"$exp/games.pgn" 2>/dev/null || true

# --- 6. Record. ---
verdict=$(jq -r .verdict "$exp/sprt.json")
case $verdict in
  H1) gate[sprt]=pass; (( rc == 0 )) || reason="match exited $rc" ;;
  *)  gate[sprt]=fail; reason="SPRT verdict: $verdict" ;;
esac
sprt_file="$exp/sprt.json"
if [[ -z $reason ]] && git -C "$wt" diff --quiet "$baseline" "$candidate"; then
  reason="empty diff (candidate equals baseline)"
fi
record
