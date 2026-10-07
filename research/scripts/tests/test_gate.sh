#!/usr/bin/env bash
source "$(dirname "${BASH_SOURCE[0]}")/flow_common.sh"

# Fake ssh: ignores the host, logs the remote command, answers by recognising it.
# FAKE_SSH_LOG: file to append commands to. FAKE_TEST_X_HIT: make `test -x` succeed (cache hit).
# FAKE_CUTECHESS_RC: exit status of the match, which still prints FAKE_CUTECHESS_OUTPUT.
cat > "$tmp/fakessh" <<'EOF'
#!/usr/bin/env bash
shift
cmd="$*"
[[ -z ${FAKE_SSH_LOG:-} ]] || printf '%s\n' "$cmd" >> "$FAKE_SSH_LOG"
case "$cmd" in
  *"rustc -vV"*)   [[ -n ${FAKE_RUSTC_EMPTY:-} ]] || echo "rustc 1.99.0-fake" ;;
  *cutechess-cli*) cat "$FAKE_CUTECHESS_OUTPUT"; exit "${FAKE_CUTECHESS_RC:-0}" ;;
  "test -x"*)      [[ -n ${FAKE_TEST_X_HIT:-} ]] ;;
  "cat "*)         echo '[Event "fake"]' ;;
  *tar*-x*)        cat > /dev/null ;;
  *)               ;;
esac
EOF
chmod +x "$tmp/fakessh"
export RESEARCH_SSH="$tmp/fakessh"

status_of() { jq -r .status "$R/experiments/$1/result.json"; }
SHIP_RE='tar.{1,2}-x'

run_case() { # id fixture
  "$R/scripts/attempt.sh" "$1" >/dev/null
  FAKE_SSH_LOG="$tmp/ssh-$1.log" FAKE_CUTECHESS_OUTPUT="$HERE/fixtures/$2" "$R/scripts/gate.sh" "$1" >/dev/null
}

run_case 0001-h1 h1.txt
assert_eq "$(status_of 0001-h1)" "accepted" "H1 -> accepted"
assert_eq "$(jq -r .sprt.verdict "$R/experiments/0001-h1/result.json")" "H1" "verdict recorded"
assert_eq "$(jq -r .config_hash "$R/experiments/0001-h1/result.json")" "$(python3 "$R/scripts/cfg.py" hash)" "config hash recorded"
[[ -f "$R/experiments/0001-h1/patch.diff" ]] || fail "patch.diff missing"
[[ -f "$R/experiments/0001-h1/games.pgn" ]] || fail "games.pgn missing"
grep -q '0001-h1' "$R/CATALOG.md" || fail "catalog not regenerated"

# The match is issued under the remote lock with the expected cutechess arguments.
log="$tmp/ssh-0001-h1.log"
match_cmd=$(grep -F 'cutechess-cli' "$log")
for want in 'flock' '-repeat' '-games 2' '-rounds 50' '-sprt elo0=0 elo1=5 alpha=0.05 beta=0.05' '-pgnout'; do
  grep -qF -- "$want" <<<"$match_cmd" || fail "match command lacks '$want'"
done
grep -q 'name=new.*name=old' <<<"$match_cmd" || fail "candidate must be listed first (name=new before name=old)"
# Both ship+build commands run under flock; binary names carry the config hash.
ship_lines=$(grep -E "$SHIP_RE" "$log")
assert_eq "$(wc -l <<<"$ship_lines")" "2" "baseline and candidate both shipped"
assert_eq "$(grep -vc flock <<<"$ship_lines" || true)" "0" "every ship+build runs under flock"
tc=$(echo "rustc 1.99.0-fake" | sha256sum | cut -c1-12)
cfgh=$(python3 "$R/scripts/cfg.py" hash)
grep -qF "cand-$(git -C "$repo" rev-parse --short=12 main)-$cfgh-$tc" <<<"$match_cmd" || fail "candidate name should carry config hash and toolchain hash"
grep -qF -- "-$cfgh-$tc" <<<"$match_cmd" || fail "binary names should carry the config hash and toolchain hash"
grep -qF -- "base-" <<<"$(grep -F 'test -x' "$log")" || fail "outer cache check missing"
grep -qF -- "-$cfgh-$tc" <<<"$(grep -F 'test -x' "$log")" || fail "outer cache check should use the keyed name"
grep -qF -- "-$cfgh-$tc" <<<"$ship_lines" || fail "build script should use the keyed names"
grep -q 'empty diff' "$R/experiments/0001-h1/result.json" || fail "empty-diff reason missing"

run_case 0002-h0 h0.txt
assert_eq "$(status_of 0002-h0)" "rejected" "H0 -> rejected"

run_case 0003-inc inconclusive.txt
assert_eq "$(status_of 0003-inc)" "inconclusive" "no verdict -> inconclusive"

# A candidate that edits a protected path is broken at preflight and never reaches the match.
"$R/scripts/attempt.sh" 0004-protected >/dev/null
wt="$repo/.worktrees/0004-protected"
echo '// weakened' >> "$wt/tests/t.rs"
git -C "$wt" commit -qam "weaken tests"
FAKE_CUTECHESS_OUTPUT="$HERE/fixtures/h1.txt" "$R/scripts/gate.sh" 0004-protected >/dev/null
assert_eq "$(status_of 0004-protected)" "broken" "protected path -> broken"
assert_eq "$(jq -r .gate.preflight "$R/experiments/0004-protected/result.json")" "fail" "preflight failed"
assert_eq "$(jq -r .gate.sprt "$R/experiments/0004-protected/result.json")" "skipped" "sprt skipped"

# A failing local build is rejected without a match.
write_config "$tmp/config-badbuild.toml" "false"
"$R/scripts/attempt.sh" 0005-badbuild >/dev/null
RESEARCH_CONFIG="$tmp/config-badbuild.toml" FAKE_CUTECHESS_OUTPUT="$HERE/fixtures/h1.txt" \
  "$R/scripts/gate.sh" 0005-badbuild >/dev/null
assert_eq "$(status_of 0005-badbuild)" "rejected" "build failure -> rejected"
assert_eq "$(jq -r .gate.build "$R/experiments/0005-badbuild/result.json")" "fail" "build failed"

# Unparseable match output is an infrastructure failure, not a verdict.
echo "cutechess-cli: error: could not start engine" > "$tmp/garbage.txt"
"$R/scripts/attempt.sh" 0006-garbage >/dev/null
FAKE_CUTECHESS_OUTPUT="$tmp/garbage.txt" "$R/scripts/gate.sh" 0006-garbage >/dev/null
assert_eq "$(status_of 0006-garbage)" "broken" "unparseable output -> broken"

# Uncommitted work in the worktree is refused rather than silently measured.
"$R/scripts/attempt.sh" 0007-dirty >/dev/null
echo '// wip' >> "$repo/.worktrees/0007-dirty/src/main.rs"
if "$R/scripts/gate.sh" 0007-dirty >/dev/null 2>&1; then fail "dirty worktree should be refused"; fi

# Unknown experiment.
if "$R/scripts/gate.sh" 9999-none >/dev/null 2>&1; then fail "unknown id should fail"; fi

# Invalid id is refused before anything reaches a remote command.
if "$R/scripts/gate.sh" bad_id >/dev/null 2>&1; then fail "invalid id should fail"; fi

# Cache hit: a cached baseline is not shipped again.
"$R/scripts/attempt.sh" 0008-cached >/dev/null
FAKE_TEST_X_HIT=1 FAKE_SSH_LOG="$tmp/ssh-0008.log" FAKE_CUTECHESS_OUTPUT="$HERE/fixtures/h1.txt" \
  "$R/scripts/gate.sh" 0008-cached >/dev/null
assert_eq "$(status_of 0008-cached)" "accepted" "cache hit still accepted"
assert_eq "$(grep -cE "$SHIP_RE" "$tmp/ssh-0008.log")" "1" "only the candidate is shipped on a cache hit"
if grep -E "$SHIP_RE" "$tmp/ssh-0008.log" | grep -q 'base-'; then fail "baseline shipped despite cache hit"; fi

# Re-run safety: stale artefacts from an earlier run never survive into a new result.
"$R/scripts/attempt.sh" 0009-rerun >/dev/null
FAKE_CUTECHESS_OUTPUT="$HERE/fixtures/h1.txt" "$R/scripts/gate.sh" 0009-rerun >/dev/null
assert_eq "$(status_of 0009-rerun)" "accepted" "first run accepted"
RESEARCH_CONFIG="$tmp/config-badbuild.toml" FAKE_CUTECHESS_OUTPUT="$HERE/fixtures/h1.txt" \
  "$R/scripts/gate.sh" 0009-rerun >/dev/null
assert_eq "$(status_of 0009-rerun)" "rejected" "re-run with bad build -> rejected"
assert_eq "$(jq -r .gate.build "$R/experiments/0009-rerun/result.json")" "fail" "re-run build failed"
for f in sprt.json sprt.log games.pgn; do
  [[ ! -e "$R/experiments/0009-rerun/$f" ]] || fail "stale $f survived a re-run"
done

# cutechess exits non-zero but printed a parseable result: verdict kept, exit code noted.
"$R/scripts/attempt.sh" 0010-rc >/dev/null
FAKE_CUTECHESS_RC=3 FAKE_CUTECHESS_OUTPUT="$HERE/fixtures/h1.txt" "$R/scripts/gate.sh" 0010-rc >/dev/null
assert_eq "$(status_of 0010-rc)" "accepted" "non-zero exit with parseable H1 -> accepted"
grep -q 'exited 3' "$R/experiments/0010-rc/result.json" || fail "reason should mention the exit code"

# cutechess exits non-zero with garbage output: infrastructure failure.
"$R/scripts/attempt.sh" 0011-rcgarbage >/dev/null
FAKE_CUTECHESS_RC=3 FAKE_CUTECHESS_OUTPUT="$tmp/garbage.txt" "$R/scripts/gate.sh" 0011-rcgarbage >/dev/null
assert_eq "$(status_of 0011-rcgarbage)" "broken" "non-zero exit with garbage -> broken"

# Toolchain probe failing (empty output) is an infrastructure failure.
"$R/scripts/attempt.sh" 0012-notc >/dev/null
FAKE_RUSTC_EMPTY=1 FAKE_CUTECHESS_OUTPUT="$HERE/fixtures/h1.txt" "$R/scripts/gate.sh" 0012-notc >/dev/null
assert_eq "$(status_of 0012-notc)" "broken" "empty rustc -vV -> broken"

# Integrity: a tampered or unusable baseline_commit records broken (never aborts).
"$R/scripts/attempt.sh" 0013-badhex >/dev/null
echo "notahex" > "$R/experiments/0013-badhex/baseline_commit"
FAKE_CUTECHESS_OUTPUT="$HERE/fixtures/h1.txt" "$R/scripts/gate.sh" 0013-badhex >/dev/null
assert_eq "$(status_of 0013-badhex)" "broken" "non-hex baseline -> broken"
assert_eq "$(jq -r .gate.preflight "$R/experiments/0013-badhex/result.json")" "fail" "non-hex baseline preflight"
"$R/scripts/attempt.sh" 0014-nosuch >/dev/null
printf 'a%.0s' {1..40} > "$R/experiments/0014-nosuch/baseline_commit"
FAKE_CUTECHESS_OUTPUT="$HERE/fixtures/h1.txt" "$R/scripts/gate.sh" 0014-nosuch >/dev/null
assert_eq "$(status_of 0014-nosuch)" "broken" "unresolvable baseline -> broken"
assert_eq "$(jq -r .gate.preflight "$R/experiments/0014-nosuch/result.json")" "fail" "unresolvable baseline preflight"

# Integrity: a baseline that is not on the baseline branch is refused.
"$R/scripts/attempt.sh" 0015-side >/dev/null
side=$(git -C "$repo" commit-tree "main^{tree}" -p main -m side)
git -C "$repo/.worktrees/0015-side" reset -q --hard "$side"
echo "$side" > "$R/experiments/0015-side/baseline_commit"
FAKE_CUTECHESS_OUTPUT="$HERE/fixtures/h1.txt" "$R/scripts/gate.sh" 0015-side >/dev/null
assert_eq "$(status_of 0015-side)" "broken" "baseline off the baseline branch -> broken"
grep -q 'ancestry' "$R/experiments/0015-side/result.json" || fail "reason should mention ancestry"

# Integrity: a candidate that does not descend from the recorded baseline is refused.
"$R/scripts/attempt.sh" 0016-anc >/dev/null
orphan=$(git -C "$repo" commit-tree "main^{tree}" -m orphan)
git -C "$repo/.worktrees/0016-anc" reset -q --hard "$orphan"
FAKE_CUTECHESS_OUTPUT="$HERE/fixtures/h1.txt" "$R/scripts/gate.sh" 0016-anc >/dev/null
assert_eq "$(status_of 0016-anc)" "broken" "non-descendant candidate -> broken"
assert_eq "$(jq -r .gate.preflight "$R/experiments/0016-anc/result.json")" "fail" "non-descendant preflight"
grep -q 'ancestry' "$R/experiments/0016-anc/result.json" || fail "reason should mention ancestry"

# Non-ASCII protected path is not bypassed by git's path quoting.
"$R/scripts/attempt.sh" 0018-utf >/dev/null
echo '// x' > "$repo/.worktrees/0018-utf/tests/é.rs"
git -C "$repo/.worktrees/0018-utf" add -A
git -C "$repo/.worktrees/0018-utf" commit -qm "utf8 test file"
FAKE_CUTECHESS_OUTPUT="$HERE/fixtures/h1.txt" "$R/scripts/gate.sh" 0018-utf >/dev/null
assert_eq "$(status_of 0018-utf)" "broken" "non-ASCII protected path -> broken"

echo "test_gate: OK"
