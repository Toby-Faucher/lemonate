#!/usr/bin/env bash
# gate.sh <id>: preflight, local build/test/perft, remote SPRT, record result.json.
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

id=${1:-}
exp="$EXP_DIR/$id"
wt="$WORKTREES/$id"
[[ -n $id && -f $exp/baseline_commit ]] || die "usage: gate.sh <id> (no such experiment: '$id')"
[[ -d $wt ]] || die "worktree $wt not found"
[[ -z $(git -C "$wt" status --porcelain) ]] || die "worktree has uncommitted changes; commit them first"

baseline=$(<"$exp/baseline_commit")
candidate=$(git -C "$wt" rev-parse HEAD)
b12=${baseline:0:12}
c12=${candidate:0:12}

declare -A gate=([preflight]=skipped [build]=skipped [test]=skipped [perft]=skipped [sprt]=skipped)
reason=""
infra=0
sprt_file=""

# record: write patch.diff and result.json, refresh the catalog, print the outcome.
record() {
  git -C "$wt" diff "$baseline" "$candidate" > "$exp/patch.diff"
  local args=(--out "$exp/result.json" --baseline "$baseline" --candidate "$candidate"
              --config-hash "$(python3 "$SCRIPTS/cfg.py" hash)")
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
bad=$(touched_protected "$baseline" "$candidate")
if [[ -n $bad ]]; then
  gate[preflight]=fail
  reason="touches protected paths: $(tr '\n' ' ' <<<"$bad")"
  record; exit 0
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

ship_and_build() { # commit name
  local dir="$work/$2"
  git -C "$REPO_ROOT" archive "$1" \
    | ssh_ "$host" "rm -rf '$dir' && mkdir -p '$dir' && tar -x -C '$dir'" \
    || infra_fail "could not ship $2 to $host"
  ssh_ "$host" ". ~/.cargo/env; cd '$dir' && $build_cmd && mkdir -p '$engines' && cp target/release/lemonate '$engines/$2'" \
    >"$exp/remote-build-$2.log" 2>&1 \
    || infra_fail "remote build of $2 failed (see remote-build-$2.log)"
}

ssh_ "$host" "test -x '$engines/base-$b12'" || ship_and_build "$baseline" "base-$b12"
ship_and_build "$candidate" "cand-$c12"

# --- 5. SPRT on the container, one match at a time (flock). ---
pgn="$work/$id.pgn"
match="rm -f '$pgn'; flock '$lock' cutechess-cli \
  -engine name=new cmd='$engines/cand-$c12' proto=uci \
  -engine name=old cmd='$engines/base-$b12' proto=uci \
  -each tc=$(cfg match.tc) -games 2 -repeat -rounds $(( $(cfg match.game_cap) / 2 )) \
  -concurrency $(cfg match.concurrency) \
  -openings file='$(cfg match.book)' format=epd order=random \
  -resign movecount=3 score=400 -draw movenumber=40 movecount=8 score=10 \
  -sprt elo0=$(cfg sprt.elo0) elo1=$(cfg sprt.elo1) alpha=$(cfg sprt.alpha) beta=$(cfg sprt.beta) \
  -pgnout '$pgn'"
ssh_ "$host" ". ~/.cargo/env; $match" >"$exp/sprt.log" 2>"$exp/sprt.err" \
  || infra_fail "remote match failed (see sprt.err)"
python3 "$SCRIPTS/sprt_parse.py" <"$exp/sprt.log" >"$exp/sprt.json" 2>"$exp/sprt.err" \
  || infra_fail "could not parse cutechess output (see sprt.log, sprt.err)"
ssh_ "$host" "cat '$pgn'" >"$exp/games.pgn" 2>/dev/null || true

# --- 6. Record. ---
verdict=$(jq -r .verdict "$exp/sprt.json")
case $verdict in
  H1) gate[sprt]=pass ;;
  *)  gate[sprt]=fail; reason="SPRT verdict: $verdict" ;;
esac
sprt_file="$exp/sprt.json"
record
