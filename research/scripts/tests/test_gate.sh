#!/usr/bin/env bash
source "$(dirname "${BASH_SOURCE[0]}")/flow_common.sh"

# Fake ssh: ignores the host, answers by recognising the remote command.
cat > "$tmp/fakessh" <<'EOF'
#!/usr/bin/env bash
shift
cmd="$*"
case "$cmd" in
  *cutechess-cli*) cat "$FAKE_CUTECHESS_OUTPUT" ;;
  "cat "*)         echo '[Event "fake"]' ;;
  "test -x"*)      exit 1 ;;
  *"tar -x"*)      cat > /dev/null ;;
  *)               ;;
esac
EOF
chmod +x "$tmp/fakessh"
export RESEARCH_SSH="$tmp/fakessh"

status_of() { jq -r .status "$R/experiments/$1/result.json"; }

run_case() { # id fixture
  "$R/scripts/attempt.sh" "$1" >/dev/null
  FAKE_CUTECHESS_OUTPUT="$HERE/fixtures/$2" "$R/scripts/gate.sh" "$1" >/dev/null
}

run_case 0001-h1 h1.txt
assert_eq "$(status_of 0001-h1)" "accepted" "H1 -> accepted"
assert_eq "$(jq -r .sprt.verdict "$R/experiments/0001-h1/result.json")" "H1" "verdict recorded"
assert_eq "$(jq -r .config_hash "$R/experiments/0001-h1/result.json")" "$(python3 "$R/scripts/cfg.py" hash)" "config hash recorded"
[[ -f "$R/experiments/0001-h1/patch.diff" ]] || fail "patch.diff missing"
[[ -f "$R/experiments/0001-h1/games.pgn" ]] || fail "games.pgn missing"
grep -q '0001-h1' "$R/CATALOG.md" || fail "catalog not regenerated"

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

echo "test_gate: OK"
