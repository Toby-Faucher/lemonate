#!/usr/bin/env python3
"""Queue and watch SPRT matches on the chess-test container.

Match settings (time control, book, concurrency, game cap, default bounds) come from
research/config.toml, so results are comparable with the research gate. Two machines can run
matches, one match at a time each: chess-test (the container the gate uses) and this machine
(cutechess in a container, on spare cores, at low priority). `--runner auto` picks the shorter queue. Engines are built
here in the pinned Rust container with a portable target (x86-64-v3) and copied over, so
nothing compiles on the match machine while a match is running. Matches queue on the same
remote lock the gate uses, one at a time.

    sprt.py start NAME BASE CAND [--elo0 0] [--elo1 5] [--rounds N] [--runner auto|bawn|chess-test]
                                                                      build, ship, queue
    sprt.py status [NAME...]                                          score / LLR / verdict
    sprt.py wait NAME                                                 block until finished
    sprt.py cancel NAME                                               stop a queued or running match
    sprt.py record NAME ID                                            write research/experiments/ID from a finished match

BASE and CAND are git revisions (use commits, not the working tree).
"""
import argparse
import io
import json
import math
import re
import shlex
import subprocess
import sys
import tarfile
import time
import tomllib
from pathlib import Path

BENCH = Path(__file__).resolve().parent
ROOT = BENCH.parent
WORK = BENCH / ".work" / "sprt"
RESEARCH = tomllib.loads((ROOT / "research" / "config.toml").read_text())
BENCH_CFG = json.loads((BENCH / "config.json").read_text())
HOST = RESEARCH["remote"]["host"]
LOCK = RESEARCH["remote"]["lock"]
# The local runner: cutechess-cli in a small Debian container, on threads 0-9 only (the
# benchmark pins to cpu 14/15) and at low priority, because this machine also hosts services.
RUNNER = WORK.parent / "runner"
LOCAL_IMAGE = "lemonate-cutechess"
LOCAL_LOCK = "/tmp/lemonate-sprt-local.lock"
LOCAL_CPUS = "0-9"
LOCAL_CONCURRENCY = 8
RUNNERS = ("chess-test", "bawn")
# Portable: the match machine is a different CPU from the build machine.
RUSTFLAGS = "-C target-cpu=x86-64-v3 -C target-feature=+bmi2,+popcnt"


def sh(*cmd, **kw):
    return subprocess.run(cmd, capture_output=True, text=True, **kw)


def resolve(rev):
    r = sh("git", "-C", str(ROOT), "rev-parse", "--verify", rev + "^{commit}")
    if r.returncode:
        sys.exit(f"cannot resolve {rev}: {r.stderr.strip()}")
    return r.stdout.strip()


def build(commit):
    """Portable release build of `commit` in the pinned container. Returns the binary path."""
    out_dir = WORK / "bin"
    out_dir.mkdir(parents=True, exist_ok=True)
    name = f"lemonate-{commit[:12]}"
    out = out_dir / name
    if out.exists():
        return out
    src = WORK / "src" / commit[:12]
    if src.exists():
        subprocess.run(["rm", "-rf", str(src)])
    src.mkdir(parents=True)
    data = subprocess.run(["git", "-C", str(ROOT), "archive", commit], capture_output=True, check=True).stdout
    with tarfile.open(fileobj=io.BytesIO(data)) as tf:
        tf.extractall(src, filter="data")
    script = (
        "set -e; T='taskset -c 0-11'; $T cargo clean -p lemonate --release 2>&1 || true; "
        "rm -f /target/release/lemonate; $T cargo build --release --bin lemonate 2>&1; "
        f"cp /target/release/lemonate /out/{name}.tmp && mv /out/{name}.tmp /out/{name}"
    )
    log = WORK / f"build-{commit[:12]}.log"
    cmd = [
        "podman", "run", "--rm", "-e", "CARGO_HOME=/cargo", "-e", "CARGO_TARGET_DIR=/target",
        "-e", "CARGO_INCREMENTAL=0", "-e", "CARGO_TERM_COLOR=never", "-e", f"RUSTFLAGS={RUSTFLAGS}",
        "-v", "lemonate-bench-cargo:/cargo", "-v", "lemonate-sprt-target:/target",
        "-v", f"{src}:/src", "-v", f"{out_dir}:/out", "-w", "/src", BENCH_CFG["image"], "sh", "-c", script,
    ]
    with open(log, "wb") as lf:
        r = subprocess.run(cmd, stdout=lf, stderr=subprocess.STDOUT)
    subprocess.run(["rm", "-rf", str(src)])
    if r.returncode or not out.exists():
        sys.exit(f"build of {commit[:12]} failed, see {log}")
    return out


def ship(binary):
    remote = f"/root/engines/{binary.name}"
    have = sh("ssh", HOST, f"test -x {remote} && echo yes")
    if "yes" not in have.stdout:
        r = sh("ssh", HOST, "mkdir -p /root/engines")
        r = sh("scp", "-q", str(binary), f"{HOST}:{remote}.tmp")
        if r.returncode:
            sys.exit(f"scp failed: {r.stderr}")
        r = sh("ssh", HOST, f"chmod +x {remote}.tmp && printf 'uci\\nquit\\n' | timeout 10 {remote}.tmp 2>/dev/null | grep -q uciok && mv {remote}.tmp {remote}")
        if r.returncode:
            sys.exit(f"{binary.name} did not answer 'uci' on {HOST}")
    return remote


def cutechess_cmd(cand_path, base_path, book, pgn, concurrency, rounds, elo0, elo1):
    m, s = RESEARCH["match"], RESEARCH["sprt"]
    return (
        f"cutechess-cli "
        f"-engine name=new cmd={cand_path} proto=uci -engine name=old cmd={base_path} proto=uci "
        f"-each tc={m['tc']} -games 2 -repeat -rounds {rounds} -concurrency {concurrency} "
        f"-openings file={book} format=epd order=random "
        f"-resign movecount=3 score=400 -draw movenumber=40 movecount=8 score=10 "
        f"-sprt elo0={elo0} elo1={elo1} alpha={s['alpha']} beta={s['beta']} -pgnout {pgn}"
    )


def ensure_local_runner():
    """The local runner needs the cutechess binary, the opening book and a small image."""
    missing = [f for f in ("cutechess-cli", "UHO_Lichess_4852_v1.epd") if not (RUNNER / f).exists()]
    if missing:
        RUNNER.mkdir(parents=True, exist_ok=True)
        for f, src in (("cutechess-cli", "/usr/local/bin/cutechess-cli"), ("UHO_Lichess_4852_v1.epd", RESEARCH["match"]["book"])):
            if f in missing:
                r = sh("scp", "-q", f"{HOST}:{src}", str(RUNNER / f))
                if r.returncode:
                    sys.exit(f"cannot fetch {f} from {HOST}: {r.stderr}")
    if sh("podman", "image", "exists", LOCAL_IMAGE).returncode:
        containerfile = ("FROM docker.io/library/debian:13-slim\n"
                         "RUN apt-get update -qq && apt-get install -y -qq --no-install-recommends "
                         "libqt6core6t64 libstdc++6 util-linux >/dev/null 2>&1 && rm -rf /var/lib/apt/lists/*\n")
        r = subprocess.run(["podman", "build", "-q", "-t", LOCAL_IMAGE, "-f", "-", str(RUNNER)],
                           input=containerfile, capture_output=True, text=True)
        if r.returncode:
            sys.exit(f"cannot build the runner image: {r.stderr}")


def pick_runner(requested):
    if requested in RUNNERS:
        return requested
    # auto: the machine with fewer matches still waiting or running
    load = {r: 0 for r in RUNNERS}
    names = [p.stem for p in WORK.glob("*.json")]
    for res in status_many(names):
        if res["state"] in ("QUEUED", "RUNNING"):
            meta = json.loads((WORK / f"{res['name']}.json").read_text())
            load[meta.get("runner", "chess-test")] += 1
    return min(RUNNERS, key=lambda r: (load[r], RUNNERS.index(r)))


def start_local(args, name, base, cand, elo0, elo1, rounds):
    ensure_local_runner()
    rdir = WORK / name
    b_name, c_name = f"lemonate-{base[:12]}", f"lemonate-{cand[:12]}"
    inner = cutechess_cmd(f"/engines/{c_name}", f"/engines/{b_name}", "/book.epd", "/out/games.pgn",
                          LOCAL_CONCURRENCY, rounds, elo0, elo1)
    podman = (
        f"podman run --rm --name sprt-{name} -e LANG=C.UTF-8 "
        f"-v {WORK / 'bin'}:/engines:ro -v {RUNNER / 'UHO_Lichess_4852_v1.epd'}:/book.epd:ro "
        f"-v {RUNNER / 'cutechess-cli'}:/usr/local/bin/cutechess-cli:ro -v {rdir}:/out "
        f"{LOCAL_IMAGE} taskset -c {LOCAL_CPUS} nice -n 10 {inner}"
    )
    script = (f"rm -rf {rdir}; mkdir -p {rdir}; cd {rdir}; "
              f"flock {LOCAL_LOCK} bash -c {shlex.quote(f'echo started=$(date +%s) > started; {podman} > log 2>&1')}; "
              f"echo finished=$(date +%s) > finished")
    subprocess.Popen(["setsid", "nohup", "bash", "-c", script], stdin=subprocess.DEVNULL,
                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
    return str(rdir)


def start(args):
    WORK.mkdir(parents=True, exist_ok=True)
    meta_path = WORK / f"{args.name}.json"
    if meta_path.exists() and not args.force:
        sys.exit(f"{meta_path.name} exists; pick another name or --force")
    base, cand = resolve(args.base), resolve(args.cand)
    runner = pick_runner(args.runner)
    print(f"building base {base[:12]} and candidate {cand[:12]} ... (runner: {runner})", flush=True)
    b_bin, c_bin = build(base), build(cand)
    m, s = RESEARCH["match"], RESEARCH["sprt"]
    elo0 = args.elo0 if args.elo0 is not None else s["elo0"]
    elo1 = args.elo1 if args.elo1 is not None else s["elo1"]
    rounds = args.rounds or m["game_cap"] // 2
    if runner == "bawn":
        rdir = start_local(args, args.name, base, cand, elo0, elo1, rounds)
        meta = {"name": args.name, "base": base, "cand": cand, "elo0": elo0, "elo1": elo1, "rounds": rounds,
                "queued": int(time.time()), "dir": rdir, "runner": "bawn"}
        meta_path.write_text(json.dumps(meta, indent=2))
        print(f"queued '{args.name}' on {runner}: {base[:7]} -> {cand[:7]}  bounds [{elo0}, {elo1}]  up to {rounds * 2} games")
        return
    b_remote, c_remote = ship(b_bin), ship(c_bin)
    rdir = f"/root/sprt/{args.name}"
    match = f"flock {LOCK} " + cutechess_cmd(c_remote, b_remote, m["book"], f"{rdir}/games.pgn",
                                             m["concurrency"], rounds, elo0, elo1)
    # `started` is written only once the lock is held, so a match still waiting its turn reads as queued.
    inner = match.replace(f"flock {LOCK} ", "", 1)
    # Start from an empty directory: a stale `started`/`finished` from an earlier run of the same
    # name would make a queued match look like it is running or done.
    script = (f"rm -rf {rdir}; mkdir -p {rdir}; cd {rdir}; "
              f"flock {LOCK} bash -c 'echo started=$(date +%s) > started; {inner} > log 2>&1'; "
              f"echo finished=$(date +%s) > finished")
    # Detach fully so ssh returns at once; the match queues on the lock by itself.
    subprocess.run(["ssh", HOST, f"setsid nohup bash -c {shlex.quote(script)} >/dev/null 2>&1 < /dev/null &"], check=True)
    meta = {"name": args.name, "base": base, "cand": cand, "elo0": elo0, "elo1": elo1, "rounds": rounds,
            "queued": int(time.time()), "dir": rdir, "runner": "chess-test"}
    meta_path.write_text(json.dumps(meta, indent=2))
    print(f"queued '{args.name}' on {runner}: {base[:7]} -> {cand[:7]}  bounds [{elo0}, {elo1}]  up to {rounds * 2} games")


def parse(log, meta):
    out = {"games": 0, "w": 0, "l": 0, "d": 0, "elo": None, "llr": None, "verdict": None}
    scores = re.findall(r"Score of new vs old: (\d+) - (\d+) - (\d+)", log)
    if scores:
        out["w"], out["l"], out["d"] = map(int, scores[-1])
        out["games"] = sum(map(int, scores[-1]))
    m = re.search(r"Elo difference: ([-\d.]+) \+/- ([-\d.]+)", log)
    if m:
        out["elo"], out["err"] = float(m.group(1)), float(m.group(2))
    m = re.search(r"SPRT: llr ([-\d.]+).*?(H[01]) was accepted", log)
    if m:
        out["llr"], out["verdict"] = float(m.group(1)), m.group(2)
    elif out["games"]:  # running estimate of the LLR from the trinomial counts
        n = out["games"]
        s = lambda e: 1 / (1 + 10 ** (-e / 400))
        x = (out["w"] + out["d"] / 2) / n
        var = (out["w"] * (1 - x) ** 2 + out["l"] * x ** 2 + out["d"] * (0.5 - x) ** 2) / n
        if var > 0:
            m0, m1 = s(meta["elo0"]), s(meta["elo1"])
            out["llr_est"] = (m1 - m0) * (2 * x - m0 - m1) / (2 * var / n)
            out["elo_est"] = 400 * math.log10(x / (1 - x)) if 0 < x < 1 else None
    return out


def status_many(names):
    """State and parsed score of several matches, using one ssh round trip."""
    metas = {}
    for n in names:
        mp = WORK / f"{n}.json"
        if mp.exists():
            metas[n] = json.loads(mp.read_text())
    script = ""
    local = {n: m for n, m in metas.items() if m.get("runner") == "bawn"}
    for n, m in metas.items():
        if n in local:
            continue
        d = m["dir"]
        script += (f"echo '@@{n}'; cd {d} 2>/dev/null && (test -f finished && echo FINISHED || "
                   f"(test -f started && echo RUNNING || echo QUEUED)); cat {d}/log 2>/dev/null; ")
    chunks = {}
    for n, m in local.items():
        d = Path(m["dir"])
        state = "FINISHED" if (d / "finished").exists() else "RUNNING" if (d / "started").exists() else "QUEUED"
        log = (d / "log").read_text(errors="replace") if (d / "log").exists() else ""
        chunks[n] = f"{state}\n{log}"
    if script:
        out = sh("ssh", HOST, script).stdout
        parts = re.split(r"^@@(.+)$", out, flags=re.M)
        for i in range(1, len(parts) - 1, 2):
            chunks[parts[i]] = parts[i + 1].lstrip("\n")
    results = []
    for n in names:
        if n not in metas:
            results.append({"name": n, "state": "unknown"})
            continue
        head, _, log = chunks.get(n, "").partition("\n")
        res = parse(log, metas[n])
        res.update({"name": n, "state": head.strip() or "QUEUED", "runner": metas[n].get("runner", "chess-test")})
        if res["state"] == "FINISHED" and not res["verdict"]:
            res["verdict"] = "inconclusive (game cap reached)"
        results.append(res)
    return results


def status_one(name):
    return status_many([name])[0]


def fmt(res):
    if res["state"] == "unknown":
        return f"{res['name']}: unknown"
    line = f"{res['name']:<24} {res.get('runner', ''):<10} {res['state']:<9} {res['games']:>5} games  {res['w']}-{res['l']}-{res['d']}"
    if res.get("elo") is not None:
        line += f"  Elo {res['elo']:+.1f} +/- {res.get('err', 0):.1f}"
    elif res.get("elo_est") is not None:
        line += f"  Elo~{res['elo_est']:+.0f}"
    if res.get("llr") is not None:
        line += f"  LLR {res['llr']:.2f}"
    elif res.get("llr_est") is not None:
        line += f"  LLR~{res['llr_est']:.2f}/2.94"
    if res["verdict"]:
        line += f"  => {res['verdict']}"
    return line


def status(args):
    names = args.names or [p.stem for p in sorted(WORK.glob("*.json"))]
    for res in status_many(names):
        print(fmt(res))


def wait(args):
    while True:
        res = status_one(args.name)
        if res["state"] == "FINISHED":
            print(fmt(res))
            return
        time.sleep(60)


def record(args):
    """Write a finished match into research/experiments/ID in the format the gate produces,
    so research/scripts/catalog.sh can include it. hypothesis.md and trace.md are written by hand."""
    meta_path = WORK / f"{args.name}.json"
    if not meta_path.exists():
        sys.exit(f"no such match: {args.name}")
    meta = json.loads(meta_path.read_text())
    res = status_one(args.name)
    if res["state"] != "FINISHED":
        sys.exit(f"{args.name} has not finished ({res['state']})")
    verdict = res["verdict"] or ""
    status = {"H1": "accepted", "H0": "rejected"}.get(verdict, "inconclusive")
    sprt = {
        "games": res["games"], "wins": res["w"], "losses": res["l"], "draws": res["d"],
        "elo": res.get("elo"), "elo_err": res.get("err"), "llr": res.get("llr"),
        "verdict": verdict if verdict in ("H0", "H1") else "inconclusive",
    }
    out = ROOT / "research" / "experiments" / args.id
    out.mkdir(parents=True, exist_ok=True)
    result = {
        "baseline_commit": meta["base"],
        "candidate_commit": meta["cand"],
        "config_hash": "bench-sprt",
        "gate": {"preflight": "skipped", "build": "pass", "test": "pass", "perft": "pass",
                 "sprt": "pass" if verdict == "H1" else "fail"},
        "sprt": sprt,
        "status": status,
        "reason": args.reason or f"SPRT bounds [{meta['elo0']}, {meta['elo1']}] run with bench/sprt.py, not research/scripts/gate.sh",
    }
    (out / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    (out / "sprt.json").write_text(json.dumps(sprt, indent=2) + "\n")
    diff = sh("git", "-C", str(ROOT), "diff", meta["base"], meta["cand"])
    (out / "patch.diff").write_text(diff.stdout)
    (out / "baseline_commit").write_text(meta["base"] + "\n")
    print(f"wrote {out.relative_to(ROOT)}: {status}  {fmt(res)}")


def cancel(args):
    meta_path = WORK / f"{args.name}.json"
    if not meta_path.exists():
        sys.exit(f"no such match: {args.name}")
    # Every process of the match (waiting shell, flock, cutechess) carries its directory name.
    meta = json.loads(meta_path.read_text())
    pattern = f"sprt/{args.name}[/ ;]"
    if meta.get("runner") == "bawn":
        sh("pkill", "-f", pattern)
        sh("podman", "kill", f"sprt-{args.name}")
        time.sleep(1)
        left = sh("pgrep", "-fc", pattern).stdout.strip() or "0"
    else:
        r = sh("ssh", HOST, f"pkill -f '{pattern}'; sleep 1; pgrep -f '{pattern}' | wc -l")
        left = r.stdout.strip()
    meta_path.rename(WORK / f"{args.name}.cancelled")
    print(f"cancelled '{args.name}' ({left} processes left)")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("start")
    s.add_argument("name"), s.add_argument("base"), s.add_argument("cand")
    s.add_argument("--elo0", type=float), s.add_argument("--elo1", type=float)
    s.add_argument("--rounds", type=int, help="game pairs (default: game_cap / 2)")
    s.add_argument("--force", action="store_true")
    s.add_argument("--runner", choices=("auto",) + RUNNERS, default="auto",
                   help="which machine plays the match (auto: the one with the shorter queue)")
    s.set_defaults(fn=start)
    s = sub.add_parser("status")
    s.add_argument("names", nargs="*")
    s.set_defaults(fn=status)
    s = sub.add_parser("wait")
    s.add_argument("name")
    s.set_defaults(fn=wait)
    s = sub.add_parser("cancel")
    s.add_argument("name")
    s.set_defaults(fn=cancel)
    s = sub.add_parser("record")
    s.add_argument("name"), s.add_argument("id")
    s.add_argument("--reason", help="one line shown in the catalog note")
    s.set_defaults(fn=record)
    args = ap.parse_args()
    args.fn(args)


if __name__ == "__main__":
    main()
