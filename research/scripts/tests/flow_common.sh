# Sourced by test_attempt.sh and test_gate.sh: builds a throwaway repo holding a copy of research/.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REAL_RESEARCH="$(cd "$HERE/../.." && pwd)"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
repo="$tmp/repo"
R="$repo/research"

fail() { echo "FAIL: $*" >&2; exit 1; }
assert_eq() { [[ "$1" == "$2" ]] || fail "$3: expected '$2', got '$1'"; }

write_config() { # path build_cmd
  cat > "$1" <<EOF
[repo]
baseline_branch = "main"
[match]
tc = "10+0.1"
book = "/book.epd"
concurrency = 2
game_cap = 100
[sprt]
elo0 = 0
elo1 = 5
alpha = 0.05
beta = 0.05
[remote]
host = "fakehost"
workdir = "/work"
engines_dir = "/engines"
lock = "/lock"
[gate]
build_cmd = "$2"
test_cmd = "true"
perft_cmd = "true"
EOF
}

git init -q -b main "$repo"
git -C "$repo" config user.email t@example.com
git -C "$repo" config user.name t
mkdir -p "$repo/src" "$repo/tests" "$R/experiments"
echo 'fn main() {}' > "$repo/src/main.rs"
echo '// t' > "$repo/tests/t.rs"
touch "$R/experiments/.gitkeep"
cp -r "$REAL_RESEARCH/scripts" "$REAL_RESEARCH/agent-prompt.md" "$R/"
printf '/.worktrees\n/bins\n' > "$repo/.gitignore"
mkdir "$repo/bins"; echo book > "$repo/bins/book.bin"
write_config "$R/config.toml" "true"
git -C "$repo" add -A
git -C "$repo" commit -q -m base

export RESEARCH_CONFIG="$R/config.toml"
