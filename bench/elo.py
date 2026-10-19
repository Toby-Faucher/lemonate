#!/usr/bin/env python3
"""Rate versions of the engine on an absolute scale, against Stockfish at fixed strength.

An SPRT says whether one version beats another; it never says how strong either is. This plays
each version against Stockfish 15.1 limited to several UCI_Elo levels (the anchors) and fits the
version's rating from the scores. Matches queue on the same two machines and locks as sprt.py.

    elo.py start [REV ...]    build each version, queue a gauntlet (default: the speed record holders)
    elo.py status             games played and a provisional rating per version
    elo.py collect            write finished versions to bench/elo.jsonl, which `bench.py report` plots
    elo.py cancel LABEL

Rating: for a version with scores S_a over n_a games against anchors rated A_a, the rating R solves
sum_a n_a * E(R - A_a) = sum_a S_a with E(d) = 1 / (1 + 10^(-d / 400)). The error is the 95% interval
from the games' own score variance. The scale is Stockfish 15.1's UCI_Elo, not CCRL or FIDE.
"""
import argparse
import json
import math
import re
import shlex
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import sprt  # noqa: E402

BENCH = sprt.BENCH
ROOT = sprt.ROOT
WORK = BENCH / ".work" / "elo"
ELO_FILE = BENCH / "elo.jsonl"
ANCHORS = (1500, 1800, 2100, 2400)
ROUNDS = 50  # x 2 games = 100 games per anchor
STOCKFISH = "/usr/games/stockfish"  # on chess-test; copied to the local runner
# Speed record holders plus the engine as it stands, so the Elo chart lines up with the speed one.
DEFAULT_REVS = ["320c2ef", "a2108c3", "22f7923", "0ad4a98", "67dec33", "b8d1daf", "3b9c655", "main", "exp/0015-pvs"]


# ----------------------------------------------------------------------------- rating maths


def expected(d):
    return 1.0 / (1.0 + 10.0 ** (-d / 400.0))


def fit_rating(results):
    """results: {anchor_elo: (wins, losses, draws)}. Returns (rating, error95, games) or None."""
    games = sum(w + l + d for w, l, d in results.values())
    if not games:
        return None
    total = sum(w + d / 2 for w, l, d in results.values())
    lo, hi = 0.0, 4000.0
    for _ in range(80):
        mid = (lo + hi) / 2
        pred = sum((w + l + d) * expected(mid - a) for a, (w, l, d) in results.items())
        lo, hi = (mid, hi) if pred < total else (lo, mid)
    rating = (lo + hi) / 2
    # Variance of the total score from each anchor's own per-game variance.
    var = 0.0
    for w, l, d in results.values():
        n = w + l + d
        if n:
            s = (w + d / 2) / n
            var += n * (w * (1 - s) ** 2 + d * (0.5 - s) ** 2 + l * s ** 2) / n
    slope = sum((w + l + d) * math.log(10) / 400 * expected(rating - a) * (1 - expected(rating - a))
                for a, (w, l, d) in results.items())
    err = 1.96 * math.sqrt(var) / slope if slope else float("inf")
    return rating, err, games


def saturated(rating):
    """A rating pinned to the search bounds means every game was lost or won: the version is
    outside what these anchors can measure, so the number says nothing."""
    return rating < 100 or rating > 3900


def parse_log(log):
    """W-L-D of the version `v` against each anchor, from cutechess's per-game lines
    `Finished game N (white vs black): 1-0 {reason}`. (A tournament with more than two
    engines prints a ranking table instead of `Score of` lines, so count the games.)"""
    out = {}
    for white, black, result in re.findall(r"^Finished game \d+ \((\S+) vs (\S+)\): (1-0|0-1|1/2-1/2)", log, re.M):
        if white == "v":
            anchor, v_score = black, {"1-0": 1, "0-1": 0, "1/2-1/2": 0.5}[result]
        elif black == "v":
            anchor, v_score = white, {"0-1": 1, "1-0": 0, "1/2-1/2": 0.5}[result]
        else:
            continue
        m = re.fullmatch(r"sf(\d+)", anchor)
        if not m:
            continue
        w, l, d = out.get(int(m.group(1)), (0, 0, 0))
        out[int(m.group(1))] = (w + (v_score == 1), l + (v_score == 0), d + (v_score == 0.5))
    return out


# ----------------------------------------------------------------------------- jobs


def load_metas():
    return {p.stem: json.loads(p.read_text()) for p in sorted(WORK.glob("*.json"))}


def ensure_local_stockfish():
    sprt.ensure_local_runner()
    target = sprt.RUNNER / "stockfish"
    if not target.exists():
        r = sprt.sh("scp", "-q", f"{sprt.HOST}:{STOCKFISH}", str(target))
        if r.returncode:
            sys.exit(f"cannot fetch stockfish from {sprt.HOST}: {r.stderr}")
        target.chmod(0o755)


def gauntlet_cmd(engine, stockfish, book, pgn, concurrency):
    tc = sprt.RESEARCH["match"]["tc"]
    parts = ["cutechess-cli", "-tournament", "gauntlet", f"-engine name=v cmd={engine} proto=uci"]
    for a in ANCHORS:
        parts.append(f"-engine name=sf{a} cmd={stockfish} proto=uci option.UCI_LimitStrength=true "
                     f"option.UCI_Elo={a} option.Threads=1 option.Hash=64")
    parts += [
        f"-each tc={tc} -games 2 -repeat -rounds {ROUNDS} -concurrency {concurrency}",
        f"-openings file={book} format=epd order=random",
        "-resign movecount=3 score=400 -draw movenumber=40 movecount=8 score=10",
        f"-pgnout {pgn}",
    ]
    return " ".join(parts)


def commit_info(commit):
    at, author, subject = sprt.sh("git", "-C", str(ROOT), "log", "-1", "--format=%at%x1f%an%x1f%s", commit).stdout.strip().split("\x1f")
    return {"ts": int(at), "author": author, "subject": subject}


def launch(label, commit, runner):
    binary = sprt.build(commit)
    if runner == "bawn":
        ensure_local_stockfish()
        rdir = WORK / label
        inner = gauntlet_cmd(f"/engines/{binary.name}", "/usr/games/stockfish", "/book.epd", "/out/games.pgn",
                             sprt.LOCAL_CONCURRENCY)
        podman = (
            f"podman run --rm --name elo-{label} -e LANG=C.UTF-8 "
            f"-v {sprt.WORK / 'bin'}:/engines:ro -v {sprt.RUNNER / 'UHO_Lichess_4852_v1.epd'}:/book.epd:ro "
            f"-v {sprt.RUNNER / 'cutechess-cli'}:/usr/local/bin/cutechess-cli:ro "
            f"-v {sprt.RUNNER / 'stockfish'}:/usr/games/stockfish:ro -v {rdir}:/out "
            f"{sprt.LOCAL_IMAGE} taskset -c {sprt.LOCAL_CPUS} nice -n 10 {inner}"
        )
        script = (f"rm -rf {rdir}; mkdir -p {rdir}; cd {rdir}; "
                  f"flock {sprt.LOCAL_LOCK} bash -c {shlex.quote(f'echo started=$(date +%s) > started; {podman} > log 2>&1')}; "
                  f"echo finished=$(date +%s) > finished")
        subprocess.Popen(["setsid", "nohup", "bash", "-c", script], stdin=subprocess.DEVNULL,
                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
        return str(rdir)
    remote = sprt.ship(binary)
    rdir = f"/root/elo/{label}"
    match = gauntlet_cmd(remote, STOCKFISH, sprt.RESEARCH["match"]["book"], f"{rdir}/games.pgn",
                         sprt.RESEARCH["match"]["concurrency"])
    script = (f"rm -rf {rdir}; mkdir -p {rdir}; cd {rdir}; "
              f"flock {sprt.LOCK} bash -c {shlex.quote(f'echo started=$(date +%s) > started; {match} > log 2>&1')}; "
              f"echo finished=$(date +%s) > finished")
    subprocess.run(["ssh", sprt.HOST, f"setsid nohup bash -c {shlex.quote(script)} >/dev/null 2>&1 < /dev/null &"], check=True)
    return rdir


def start(args):
    WORK.mkdir(parents=True, exist_ok=True)
    revs = args.revs or DEFAULT_REVS
    done = {m["commit"] for m in load_metas().values()}
    queued = 0
    for rev in revs:
        commit = sprt.resolve(rev)
        label = commit[:7]
        if commit in done and not args.force:
            print(f"{label} ({rev}) already queued, skipping")
            continue
        # Alternate machines so both play at once.
        runner = args.runner if args.runner != "auto" else sprt.RUNNERS[queued % 2]
        print(f"{label} ({rev}) -> {runner} ...", flush=True)
        rdir = launch(label, commit, runner)
        meta = {"label": label, "rev": rev, "commit": commit, "runner": runner, "dir": rdir,
                "queued": int(time.time()), **commit_info(commit)}
        (WORK / f"{label}.json").write_text(json.dumps(meta, indent=2))
        queued += 1
    print(f"queued {queued} version(s): {len(ANCHORS)} anchors x {ROUNDS * 2} games each")


def read_states(metas):
    """State and log text per label; one ssh round trip for the remote ones."""
    out = {}
    remote_script = ""
    for label, m in metas.items():
        if m["runner"] == "bawn":
            d = Path(m["dir"])
            state = "FINISHED" if (d / "finished").exists() else "RUNNING" if (d / "started").exists() else "QUEUED"
            out[label] = (state, (d / "log").read_text(errors="replace") if (d / "log").exists() else "")
        else:
            d = m["dir"]
            remote_script += (f"echo '@@{label}'; cd {d} 2>/dev/null && (test -f finished && echo FINISHED || "
                              f"(test -f started && echo RUNNING || echo QUEUED)); cat {d}/log 2>/dev/null; ")
    if remote_script:
        text = sprt.sh("ssh", sprt.HOST, remote_script).stdout
        parts = re.split(r"^@@(.+)$", text, flags=re.M)
        for i in range(1, len(parts) - 1, 2):
            head, _, log = parts[i + 1].lstrip("\n").partition("\n")
            out[parts[i]] = (head.strip() or "QUEUED", log)
    return out


def status(args):
    metas = load_metas()
    if not metas:
        print("nothing queued")
        return
    states = read_states(metas)
    for label, m in metas.items():
        state, log = states.get(label, ("QUEUED", ""))
        results = parse_log(log)
        fit = fit_rating(results)
        games = sum(w + l + d for w, l, d in results.values())
        rating = f"~{fit[0]:.0f} +/- {fit[1]:.0f}" if fit and not saturated(fit[0]) else "-"
        print(f"{label}  {m['runner']:<10} {state:<9} {games:>4}/{len(ANCHORS) * ROUNDS * 2} games  Elo {rating:<14} {m['subject'][:50]}")


def collect(args):
    metas = load_metas()
    states = read_states(metas)
    rows = {}
    if ELO_FILE.exists():
        for line in ELO_FILE.read_text().splitlines():
            if line.strip():
                r = json.loads(line)
                rows[r["commit"]] = r
    added = 0
    for label, m in metas.items():
        state, log = states.get(label, ("QUEUED", ""))
        if state != "FINISHED":
            continue
        results = parse_log(log)
        fit = fit_rating(results)
        if not fit:
            print(f"{label}: finished but no scores in the log")
            continue
        rating, err, games = fit
        short = [a for a in ANCHORS if sum(results.get(a, (0, 0, 0))) < ROUNDS * 2]
        if short:
            print(f"{label}: finished but incomplete (anchors short of {ROUNDS * 2} games: {short}); not recorded.")
            continue
        if saturated(rating):
            print(f"{label}: outside the anchor range (lost or won every game); not recorded. Add anchors.")
            continue
        rows[m["commit"]] = {
            "commit": m["commit"], "short": label, "ts": m["ts"], "author": m["author"], "subject": m["subject"],
            "elo": round(rating, 1), "elo_err": round(err, 1), "games": games,
            "anchors": {str(a): list(r) for a, r in sorted(results.items())},
            "runner": m["runner"], "measured": int(time.time()),
        }
        added += 1
        print(f"{label}: {rating:.0f} +/- {err:.0f} over {games} games  {m['subject'][:50]}")
    ELO_FILE.write_text("".join(json.dumps(r, separators=(",", ":")) + "\n" for r in sorted(rows.values(), key=lambda r: r["ts"])))
    print(f"wrote {ELO_FILE.relative_to(ROOT)}: {len(rows)} versions ({added} updated)")


def cancel(args):
    metas = load_metas()
    m = metas.get(args.label)
    if not m:
        sys.exit(f"no such version: {args.label}")
    pattern = f"elo/{args.label}[/ ;]"
    if m["runner"] == "bawn":
        sprt.sh("pkill", "-f", pattern)
        sprt.sh("podman", "kill", f"elo-{args.label}")
    else:
        sprt.sh("ssh", sprt.HOST, f"pkill -f '{pattern}'")
    (WORK / f"{args.label}.json").rename(WORK / f"{args.label}.cancelled")
    print(f"cancelled {args.label}")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("start")
    s.add_argument("revs", nargs="*")
    s.add_argument("--runner", choices=("auto",) + sprt.RUNNERS, default="auto")
    s.add_argument("--force", action="store_true")
    s.set_defaults(fn=start)
    sub.add_parser("status").set_defaults(fn=status)
    sub.add_parser("collect").set_defaults(fn=collect)
    s = sub.add_parser("cancel")
    s.add_argument("label")
    s.set_defaults(fn=cancel)
    args = ap.parse_args()
    args.fn(args)


if __name__ == "__main__":
    main()
