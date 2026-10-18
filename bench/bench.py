#!/usr/bin/env python3
"""Lemonate benchmark tracker.

Builds every src-changing commit in a pinned Rust container, times each binary on a
pinned CPU core with a fixed search suite over UCI, and stores one row per commit in
bench/history.jsonl. `report` turns that into an HTML "world record progression" page.

    bench.py doctor                 check podman, image, CPU pinning, noise
    bench.py history [--limit N]    build + measure every commit that is not measured yet
    bench.py run REV [REV...]       measure specific revisions (use WORKTREE for the checkout)
    bench.py compare A B            interleaved A/B with a bootstrap confidence interval
    bench.py list                   print the record progression in the terminal
    bench.py report                 write bench/report.html

Python stdlib only.
"""
import argparse
import hashlib
import io
import json
import math
import os
import platform
import queue
import random
import shutil
import statistics
import subprocess
import sys
import tarfile
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

BENCH = Path(__file__).resolve().parent
ROOT = BENCH.parent
WORK = BENCH / ".work"
HISTORY = BENCH / "history.jsonl"
ANCHOR = BENCH / "anchor.json"
SUITE = BENCH / "suite.epd"
CONFIG = json.loads((BENCH / "config.json").read_text())

# Bump when a harness change makes older rows untrustworthy.
# v2: commits whose library has the uci module but whose main.rs is not the UCI front-end are
#     built with the canonical one.
# v3: every build now starts with `cargo clean -p lemonate`. Before, a shared target dir plus
#     `git archive` mtimes (the commit date) let Cargo decide an older commit was "fresh" and
#     reuse the previous commit's binary, so v1/v2 results may belong to the wrong source.
HARNESS = 3
UCI_MAIN = "use lemonate::uci::UciEngine;\n\nfn main() {\n    let mut engine = UciEngine::new();\n    engine.run();\n}\n"

# A new version only counts as a record if it beats the standing one by at least this many
# percent. Timings repeat to within ~1%, so smaller "wins" are noise. Node counts are
# deterministic, so any reduction is real.
RECORD_MIN_PCT = {"ttd_ms": 1.5, "ttd_light_ms": 3.0, "nps": 1.5, "nodes": 0.0}

# Metric name -> (label, unit, lower_is_better)
METRICS = {
    "ttd_ms": ("Time to depth 12", "ms", True),
    "ttd_light_ms": ("Time to depth 7", "ms", True),
    "nps": ("Nodes per second", "nps", False),
    "nodes": ("Nodes to depth", "nodes", True),
}


# ----------------------------------------------------------------------------- git


def git(*args, text=True, check=True):
    r = subprocess.run(["git", "-C", str(ROOT), *args], capture_output=True, text=text)
    if check and r.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)} failed: {r.stderr.strip()}")
    return r.stdout


def needs_uci_overlay(commit):
    """True if the library has a uci module but src/main.rs is some other front-end."""
    if not git("ls-tree", "--name-only", commit, "--", "src/uci", check=False).strip():
        return False
    return "UciEngine" not in git("show", f"{commit}:src/main.rs", check=False)


def src_hash(commit):
    """Hash of everything that can change the compiled binary. Doc-only commits share it."""
    tree = git("ls-tree", commit, "--", "src", "Cargo.toml", "Cargo.lock", "build.rs", ".cargo",
               "rust-toolchain.toml", "rust-toolchain", check=False)
    if needs_uci_overlay(commit):
        tree += "|uci-main-overlay-v1"
    return hashlib.sha1(tree.encode()).hexdigest()[:16]


def list_commits(rev):
    fmt = "%H%x1f%at%x1f%an%x1f%s"
    out = git("log", "--reverse", "--topo-order", f"--format={fmt}", rev)
    rows = []
    for line in out.splitlines():
        h, at, author, subject = line.split("\x1f", 3)
        rows.append({"commit": h, "short": h[:7], "ts": int(at), "author": author, "subject": subject})
    return rows


def resolve(rev):
    return git("rev-parse", "--verify", rev + "^{commit}").strip()


# ----------------------------------------------------------------------------- build


def ensure_dirs():
    for d in ("bin", "src", "logs"):
        (WORK / d).mkdir(parents=True, exist_ok=True)


def bin_path(sh):
    return WORK / "bin" / sh


def fail_path(sh):
    return WORK / "bin" / (sh + ".fail")


def extract_commit(commit, dest):
    dest.mkdir(parents=True, exist_ok=True)
    # Read the whole archive first: streaming it through tarfile stops at the end-of-archive
    # marker and leaves `git archive` blocked on a full pipe, which hung the build step.
    data = subprocess.run(["git", "-C", str(ROOT), "archive", commit], capture_output=True, check=True).stdout
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:") as tf:
        tf.extractall(dest, filter="data")


def snapshot_worktree():
    """Copy the current working tree (tracked + untracked, minus ignored) and hash it."""
    files = git("ls-files", "-co", "--exclude-standard", "-z").split("\0")
    files = sorted(f for f in files if f and (ROOT / f).is_file())
    h = hashlib.sha1()
    for f in files:
        if f.startswith(("src/", "Cargo")) or f in ("build.rs",):
            h.update(f.encode())
            h.update((ROOT / f).read_bytes())
    sh = "wip-" + h.hexdigest()[:12]
    dest = WORK / "src" / sh
    if dest.exists():
        shutil.rmtree(dest)
    for f in files:
        t = dest / f
        t.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / f, t)
    return sh, dest


def podman_build(sh, srcdir, cpu, slot):
    """Build in the pinned image on one CPU. Returns True on success."""
    out = bin_path(sh)
    if out.exists():
        return True
    log = WORK / "logs" / f"{sh}.log"
    script = (
        f"set -e; T='taskset -c {cpu}'; $T cargo clean -p lemonate --release 2>&1 || true; "
        # A commit without a `lemonate` bin target still builds as a library; never let the
        # copy below pick up a binary left over from the previous commit in this slot.
        "rm -f /target/release/lemonate; "
        "$T cargo build --release --bin lemonate 2>&1 || $T cargo build --release 2>&1; "
        f"cp /target/release/lemonate /out/{sh}.tmp && mv /out/{sh}.tmp /out/{sh}"
    )
    cmd = [
        # Rootless podman has no cpuset cgroup controller, so pin with taskset inside the container.
        "podman", "run", "--rm",
        "-e", "CARGO_HOME=/cargo", "-e", "CARGO_TARGET_DIR=/target", "-e", "CARGO_INCREMENTAL=0",
        "-e", "CARGO_TERM_COLOR=never", "-e", f"RUSTFLAGS={CONFIG['rustflags']}",
        "-v", "lemonate-bench-cargo:/cargo", "-v", f"lemonate-bench-target-{slot}:/target",
        "-v", f"{srcdir}:/src", "-v", f"{WORK / 'bin'}:/out", "-w", "/src",
        CONFIG["image"], "sh", "-c", script,
    ]
    with open(log, "wb") as lf:
        r = subprocess.run(cmd, stdout=lf, stderr=subprocess.STDOUT)
    if r.returncode != 0 or not out.exists():
        fail_path(sh).write_text(f"build failed, see {log}\n")
        return False
    out.chmod(0o755)
    return True


def build_many(jobs):
    """jobs: list of (srchash, commit_or_None, prepared_dir_or_None). Parallel builds."""
    ensure_dirs()
    todo = [j for j in jobs if not bin_path(j[0]).exists() and not fail_path(j[0]).exists()]
    if not todo:
        return
    cpus = CONFIG["build_cpus"]
    slots = queue.Queue()
    for i, c in enumerate(cpus):
        slots.put((i, c))
    done = [0]
    lock = threading.Lock()

    def work(job):
        sh, commit, prepared = job
        slot, cpu = slots.get()
        try:
            if prepared is None:
                d = WORK / "src" / f"{sh}"
                if d.exists():
                    shutil.rmtree(d)
                extract_commit(commit, d)
                if needs_uci_overlay(commit):
                    (d / "src" / "main.rs").write_text(UCI_MAIN)
            else:
                d = prepared
            t0 = time.time()
            ok = podman_build(sh, d, cpu, slot)
            shutil.rmtree(d, ignore_errors=True)
            with lock:
                done[0] += 1
                print(f"  build [{done[0]}/{len(todo)}] {sh} {'ok' if ok else 'FAILED'} ({time.time() - t0:.0f}s)",
                      flush=True)
        finally:
            slots.put((slot, cpu))

    print(f"building {len(todo)} binaries on {len(cpus)} cores", flush=True)
    with ThreadPoolExecutor(max_workers=len(cpus)) as ex:
        list(ex.map(work, todo))


# ----------------------------------------------------------------------------- engine


class Engine:
    def __init__(self, path, cpu):
        self.proc = subprocess.Popen(
            ["taskset", "-c", str(cpu), str(path)],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, bufsize=1,
        )
        self.q = queue.Queue()
        threading.Thread(target=self._pump, daemon=True).start()

    def _pump(self):
        for line in self.proc.stdout:
            self.q.put(line.rstrip("\n"))
        self.q.put(None)

    def send(self, s):
        try:
            self.proc.stdin.write(s + "\n")
            self.proc.stdin.flush()
        except (BrokenPipeError, OSError):
            pass

    def read_until(self, prefix, timeout):
        """Lines up to and including the first starting with prefix, or None on timeout/exit."""
        end = time.perf_counter() + timeout
        lines = []
        while True:
            left = end - time.perf_counter()
            if left <= 0:
                return None
            try:
                line = self.q.get(timeout=left)
            except queue.Empty:
                return None
            if line is None:
                return None
            lines.append(line)
            if line.startswith(prefix):
                return lines

    def handshake(self):
        self.send("uci")
        if self.read_until("uciok", 15) is None:
            return False
        self.send("isready")
        return self.read_until("readyok", 15) is not None

    def search(self, fen, go, cap):
        """Returns dict(wall_ms, nodes, depth, bestmove) or None if it did not finish in `cap` s."""
        self.send("ucinewgame")
        self.send("isready")
        if self.read_until("readyok", 10) is None:
            return None
        self.send(f"position fen {fen}")
        t0 = time.perf_counter()
        self.send(go)
        lines = self.read_until("bestmove", cap)
        wall = (time.perf_counter() - t0) * 1000
        if lines is None:
            self.send("stop")
            return None
        info = parse_info(lines)
        info["wall_ms"] = wall
        info["bestmove"] = lines[-1].split()[1] if len(lines[-1].split()) > 1 else "?"
        return info

    def close(self):
        self.send("quit")
        try:
            self.proc.wait(timeout=2)
        except subprocess.TimeoutExpired:
            self.proc.kill()


def parse_info(lines):
    """Last `info` line that carries a node count."""
    best = {"nodes": 0, "depth": 0}
    for line in lines:
        if not line.startswith("info"):
            continue
        t = line.split()
        d = {}
        for k in ("depth", "nodes"):
            if k in t:
                try:
                    d[k] = int(t[t.index(k) + 1])
                except (ValueError, IndexError):
                    pass
        if "nodes" in d:
            best.update(d)
    return best


def load_suite():
    positions = []
    for line in SUITE.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        fen, _, opt = line.partition(";")
        depth = CONFIG["depth"]
        if "d=" in opt:
            depth = int(opt.split("d=")[1].split()[0])
        positions.append((fen.strip(), depth))
    return positions


def suite_hash():
    blob = SUITE.read_bytes() + json.dumps(
        {k: CONFIG[k] for k in ("depth", "movetime_ms", "passes", "nps_passes", "rustflags", "image")},
        sort_keys=True).encode()
    return hashlib.sha1(blob).hexdigest()[:10]


def depth_pass(eng, positions, cap, depth_override=None):
    """One full suite pass. Returns (per_pos_ms list, nodes list, bestmoves, dnf_count)."""
    times, nodes, moves, dnf = [], [], [], 0
    for fen, depth in positions:
        depth = depth_override or depth
        r = eng.search(fen, f"go depth {depth}", cap)
        if r is None:
            dnf += 1
            times.append(cap * 1000.0)
            nodes.append(0)
            moves.append("dnf")
            # the engine may be wedged (old builds ignored `stop`): caller restarts it
            return times, nodes, moves, dnf
        times.append(r["wall_ms"])
        nodes.append(r["nodes"])
        moves.append(r["bestmove"])
    return times, nodes, moves, dnf


def measure_binary(path, label=""):
    """Full measurement of one binary. Returns a metrics dict with a `status`."""
    cpu, cap = CONFIG["measure_cpu"], CONFIG["cap_s"]
    positions = load_suite()
    eng = Engine(path, cpu)
    if not eng.handshake():
        eng.close()
        return {"status": "no-uci"}
    try:
        # Warm-up: lazy tables, page faults, CPU ramp. Not recorded.
        for fen, _ in positions[:3]:
            eng.search(fen, "go depth 5", 10)
        # Shallow category first: every engine that can search at all should finish it.
        light = []
        for _ in range(CONFIG["passes"]):
            times, _n, _m, d = depth_pass(eng, positions, cap, CONFIG["depth_light"])
            if d:
                eng.close()
                return {"status": "dnf", "dnf": d}
            light.append(sum(times))
        pass_totals, pass_pos, node_sets, move_sets, dnf = [], [], [], [], 0
        for _ in range(CONFIG["passes"]):
            times, nodes, moves, d = depth_pass(eng, positions, cap)
            if d:
                dnf = d
                break
            pass_totals.append(sum(times))
            pass_pos.append(times)
            node_sets.append(nodes)
            move_sets.append(moves)
        if dnf:  # too slow for the main category: restart the engine, it may be wedged
            eng.close()
            eng = Engine(path, cpu)
            eng.handshake()
        # Fixed-time pass for raw speed, independent of search strength.
        nps_runs = []
        for _ in range(CONFIG["nps_passes"]):
            tot_nodes, tot_ms = 0, 0.0
            for fen, _d in positions:
                r = eng.search(fen, f"go movetime {CONFIG['movetime_ms']}", 10)
                if r:
                    tot_nodes += r["nodes"]
                    tot_ms += r["wall_ms"]
            nps_runs.append(tot_nodes / (tot_ms / 1000) if tot_ms else 0)
        eng.close()
    except Exception as e:  # engine crashed mid-run
        eng.close()
        return {"status": "crash", "error": str(e)}

    out = {
        "status": "ok",
        "ttd_light_ms": round(min(light), 2),
        "nps": round(statistics.median(nps_runs)),
        "passes": CONFIG["passes"],
    }
    if dnf:
        out.update({"dnf": dnf, "ttd_ms": None, "nodes": None, "deterministic": True})
        return out
    best_per_pos = [min(p[i] for p in pass_pos) for i in range(len(positions))]
    out.update({
        "ttd_ms": round(sum(best_per_pos), 2),
        "ttd_med_ms": round(statistics.median(pass_totals), 2),
        "noise_pct": round((max(pass_totals) - min(pass_totals)) / min(pass_totals) * 100, 2),
        "nodes": sum(node_sets[0]),
        "deterministic": all(n == node_sets[0] for n in node_sets) and all(m == move_sets[0] for m in move_sets),
        "per_position_ms": [round(x, 2) for x in best_per_pos],
        "bestmoves": move_sets[0],
    })
    return out


# ----------------------------------------------------------------------------- history


def stale(row):
    """Rows that should be measured again: failed builds, and verdicts from an older harness."""
    if row.get("status") in (None, "build-failed"):
        return True
    return row.get("h", 1) < HARNESS


def read_history():
    if not HISTORY.exists():
        return []
    return [json.loads(l) for l in HISTORY.read_text().splitlines() if l.strip()]


def append_history(row):
    with open(HISTORY, "a") as f:
        f.write(json.dumps(row, separators=(",", ":")) + "\n")


def env_info():
    cpu = "?"
    try:
        for l in open("/proc/cpuinfo"):
            if l.startswith("model name"):
                cpu = l.split(":", 1)[1].strip()
                break
    except OSError:
        pass
    gov = "?"
    try:
        gov = Path("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor").read_text().strip()
    except OSError:
        pass
    return {
        "host": platform.node(), "cpu": cpu, "governor": gov, "kernel": platform.release(),
        "image": CONFIG["image"], "load1": round(os.getloadavg()[0], 2), "pinned_cpu": CONFIG["measure_cpu"],
    }


def anchor_load():
    return json.loads(ANCHOR.read_text()) if ANCHOR.exists() else None


def measure_anchor(sh):
    """Re-time the anchor binary to correct for machine drift between sessions."""
    a = anchor_load()
    if a is None:
        return 1.0
    p = bin_path(a["srchash"])
    if not p.exists():
        return 1.0
    m = measure_binary(p)
    if m["status"] != "ok" or not m.get("ttd_ms"):
        return 1.0
    return a["ttd_ms"] / m["ttd_ms"]  # >1 means the machine is currently faster than at calibration


def set_anchor(sh, m):
    if not ANCHOR.exists() and m.get("status") == "ok" and m.get("ttd_ms"):
        ANCHOR.write_text(json.dumps({"srchash": sh, "ttd_ms": m["ttd_ms"], "nps": m["nps"],
                                      "set": time.strftime("%Y-%m-%d %H:%M:%S")}, indent=2))


def measure_rows(entries, anchor_every=8):
    """entries: list of dicts with commit meta + srchash. Appends rows to history."""
    cfg = suite_hash()
    done_by_sh = {r["srchash"]: r for r in read_history() if r.get("cfg") == cfg and not stale(r)}
    env = env_info()
    drift = measure_anchor(None)
    since_anchor = 0
    for i, e in enumerate(entries, 1):
        sh = e["srchash"]
        base = {k: e[k] for k in ("commit", "short", "ts", "author", "subject")}
        base.update({"srchash": sh, "cfg": cfg, "h": HARNESS, "measured": int(time.time())})
        if sh in done_by_sh and not e.get("force"):
            src = done_by_sh[sh]
            row = {**src, **base, "dup_of": src.get("dup_of") or src["short"]}
            append_history(row)
            print(f"  [{i}/{len(entries)}] {e['short']} same source as {row['dup_of']}", flush=True)
            continue
        if fail_path(sh).exists() or not bin_path(sh).exists():
            row = {**base, "status": "build-failed", "env": env}
            append_history(row)
            done_by_sh[sh] = row
            print(f"  [{i}/{len(entries)}] {e['short']} build failed", flush=True)
            continue
        if since_anchor >= anchor_every:
            drift = measure_anchor(None)
            since_anchor = 0
        m = measure_binary(bin_path(sh))
        since_anchor += 1
        set_anchor(sh, m)
        if m["status"] == "ok":
            m["ttd_light_norm_ms"] = round(m["ttd_light_ms"] / drift, 2)
            if m.get("ttd_ms"):
                m["ttd_norm_ms"] = round(m["ttd_ms"] / drift, 2)
            m["nps_norm"] = round(m["nps"] * drift)
        row = {**base, **m, "env": {**env, "drift": round(drift, 4)}}
        append_history(row)
        done_by_sh[sh] = row
        if m["status"] == "ok":
            full = f"{m['ttd_ms']:.0f} ms, {m['nodes']:,} nodes" if m.get("ttd_ms") else "d12 DNF"
            print(f"  [{i}/{len(entries)}] {e['short']} d7 {m['ttd_light_ms']:.0f} ms | {full} | "
                  f"{m['nps']:,} nps{'' if m['deterministic'] else ' NONDETERMINISTIC'}", flush=True)
        else:
            print(f"  [{i}/{len(entries)}] {e['short']} {m['status']}", flush=True)


def cmd_history(args):
    ensure_dirs()
    commits = list_commits(args.rev or CONFIG["branch"])
    if args.since:
        commits = [c for c in commits if c["ts"] >= int(time.mktime(time.strptime(args.since, "%Y-%m-%d")))]
    for c in commits:
        c["srchash"] = src_hash(c["commit"])
    cfg = suite_hash()
    have = {r["commit"] for r in read_history() if r.get("cfg") == cfg and not stale(r)}
    todo = [c for c in commits if c["commit"] not in have]
    if args.limit:
        todo = todo[: args.limit]
    # Builds: one per distinct source state.
    seen, jobs = set(), []
    for c in todo:
        if c["srchash"] not in seen:
            seen.add(c["srchash"])
            jobs.append((c["srchash"], c["commit"], None))
    print(f"{len(todo)} commits to record, {len(jobs)} distinct source states")
    build_many(jobs)
    print("measuring (pinned to cpu %d)" % CONFIG["measure_cpu"])
    measure_rows(todo)


def cmd_run(args):
    ensure_dirs()
    entries, jobs = [], []
    for rev in args.revs:
        if rev == "WORKTREE":
            sh, d = snapshot_worktree()
            e = {"commit": "WORKTREE", "short": sh[:11], "ts": int(time.time()), "author": "you",
                 "subject": "working tree", "srchash": sh, "force": True}
            jobs.append((sh, None, d))
        else:
            c = resolve(rev)
            line = git("log", "-1", "--format=%at%x1f%an%x1f%s", c).strip().split("\x1f")
            e = {"commit": c, "short": c[:7], "ts": int(line[0]), "author": line[1], "subject": line[2],
                 "srchash": src_hash(c), "force": args.force}
            jobs.append((e["srchash"], c, None))
        entries.append(e)
    build_many(jobs)
    measure_rows(entries)


# ----------------------------------------------------------------------------- compare


def bootstrap_ratio(a, b, n=4000, seed=1):
    """95% CI of median(a)/median(b)."""
    rng = random.Random(seed)
    rs = []
    for _ in range(n):
        ra = statistics.median(rng.choices(a, k=len(a)))
        rb = statistics.median(rng.choices(b, k=len(b)))
        rs.append(ra / rb)
    rs.sort()
    return rs[int(n * 0.025)], rs[int(n * 0.975)]


def cmd_compare(args):
    ensure_dirs()
    sides = []
    jobs = []
    for rev in (args.a, args.b):
        if rev == "WORKTREE":
            sh, d = snapshot_worktree()
            jobs.append((sh, None, d))
            sides.append((rev, sh))
        else:
            c = resolve(rev)
            sh = src_hash(c)
            jobs.append((sh, c, None))
            sides.append((rev, sh))
    build_many(jobs)
    for _, sh in sides:
        if not bin_path(sh).exists():
            sys.exit(f"build failed for {sh}; see {WORK / 'logs' / (sh + '.log')}")
    positions = load_suite()
    cpu, cap = CONFIG["measure_cpu"], CONFIG["cap_s"]
    engines = [Engine(bin_path(sh), cpu) for _, sh in sides]
    for e in engines:
        if not e.handshake():
            sys.exit("engine did not complete the UCI handshake")
        for fen, _ in positions[:3]:
            e.search(fen, "go depth 5", 10)
    totals = [[], []]
    nodes = [None, None]
    order = list(range(2))
    print(f"comparing {sides[0][0]} (A) vs {sides[1][0]} (B), {args.runs} interleaved runs each")
    for r in range(args.runs):
        random.shuffle(order)  # alternate who goes first so ordering effects cancel
        for i in order:
            t, n, _m, d = depth_pass(engines[i], positions, cap)
            if d:
                sys.exit(f"{sides[i][0]} did not finish within the cap")
            totals[i].append(sum(t))
            nodes[i] = sum(n)
        print(f"  run {r + 1}: A {totals[0][-1]:.0f} ms   B {totals[1][-1]:.0f} ms", flush=True)
    for e in engines:
        e.close()
    ma, mb = statistics.median(totals[0]), statistics.median(totals[1])
    lo, hi = bootstrap_ratio(totals[0], totals[1])
    print(f"\nA median {ma:.0f} ms | B median {mb:.0f} ms")
    print(f"B is {(ma / mb - 1) * 100:+.1f}% faster than A (A/B ratio 95% CI {lo:.3f} .. {hi:.3f})")
    if lo > 1.0:
        print("verdict: B is FASTER (CI excludes 1.0)")
    elif hi < 1.0:
        print("verdict: B is SLOWER (CI excludes 1.0)")
    else:
        print("verdict: no significant difference")
    if nodes[0] == nodes[1]:
        print(f"nodes identical ({nodes[0]:,}): same search tree, pure speed change")
    else:
        print(f"nodes differ: A {nodes[0]:,} vs B {nodes[1]:,} ({(nodes[1] / nodes[0] - 1) * 100:+.1f}%): search behaviour changed, "
              "so 'faster' may mean 'searches less' - check strength with the SPRT gate")


# ----------------------------------------------------------------------------- records


def points(rows, metric):
    """Unique-source points in time order, with their value for `metric`."""
    pts = []
    for r in rows:
        if r.get("status") != "ok" or r.get("dup_of"):
            continue
        key = {"ttd_ms": "ttd_norm_ms", "ttd_light_ms": "ttd_light_norm_ms", "nps": "nps_norm"}.get(metric, metric)
        v = r.get(key, r.get(metric))
        if v is None:
            continue
        pts.append({"commit": r["commit"], "short": r["short"], "ts": r["ts"], "author": r["author"],
                    "subject": r["subject"], "value": v})
    pts.sort(key=lambda p: p["ts"])
    return pts


def records(pts, lower_is_better, metric=None):
    """Running record progression: each time a point beats the standing record by the
    metric's noise floor (RECORD_MIN_PCT)."""
    out, best, prev_ts = [], None, None
    margin = RECORD_MIN_PCT.get(metric, 0.0) / 100.0
    for p in pts:
        if best is None:
            better = True
        elif lower_is_better:
            better = p["value"] < best * (1 - margin) if margin else p["value"] < best
        else:
            better = p["value"] > best * (1 + margin) if margin else p["value"] > best
        if better:
            rec = dict(p)
            rec["previous"] = best
            rec["improvement_pct"] = None if best is None else abs(p["value"] / best - 1) * 100
            if out:
                out[-1]["held_days"] = round((p["ts"] - out[-1]["ts"]) / 86400, 1)
            out.append(rec)
            best = p["value"]
    if out:
        out[-1]["held_days"] = round((time.time() - out[-1]["ts"]) / 86400, 1)
        out[-1]["current"] = True
    return out


def cmd_list(args):
    rows = read_history()
    for metric, (label, unit, lower) in METRICS.items():
        recs = records(points(rows, metric), lower, metric)
        print(f"\n== {label} ({'lower' if lower else 'higher'} is better) ==")
        for r in recs:
            d = time.strftime("%Y-%m-%d", time.gmtime(r["ts"]))
            imp = "" if r["improvement_pct"] is None else f"  -{r['improvement_pct']:.1f}%" if lower else f"  +{r['improvement_pct']:.1f}%"
            print(f"  {d}  {r['short']}  {r['value']:>12,.0f} {unit}{imp:>9}  held {r.get('held_days', 0):>6} d  {r['subject'][:60]}")


# ----------------------------------------------------------------------------- doctor


def cmd_doctor(args):
    ok = True

    def check(name, good, detail=""):
        nonlocal ok
        ok &= bool(good)
        print(f"  [{'ok' if good else '!!'}] {name} {detail}")

    check("podman", shutil.which("podman"))
    check("taskset", shutil.which("taskset"))
    r = subprocess.run(["podman", "image", "exists", CONFIG["image"]])
    check("rust image present", r.returncode == 0, CONFIG["image"] + ("" if r.returncode == 0 else "  (podman pull it)"))
    env = env_info()
    check("governor", env["governor"] in ("performance",), f"{env['governor']} (performance is steadier; changing it needs root)")
    check("load", env["load1"] < 2.0, f"load1={env['load1']}")
    cpu = CONFIG["measure_cpu"]
    sib = ""
    try:
        sib = Path(f"/sys/devices/system/cpu/cpu{cpu}/topology/thread_siblings_list").read_text().strip()
    except OSError:
        pass
    print(f"  measuring on cpu {cpu} (siblings: {sib}); keep the sibling idle for best results")
    print(f"  suite hash {suite_hash()}, depth {CONFIG['depth']}, {len(load_suite())} positions")
    sys.exit(0 if ok else 1)


# ----------------------------------------------------------------------------- report


def cmd_report(args):
    import report as rpt  # sibling module

    rows = read_history()
    out = Path(args.output)
    rpt.write(rows, out, METRICS, points, records, anchor_load(), CONFIG, fragment=args.fragment)
    print(f"wrote {out}")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("history")
    s.add_argument("--rev")
    s.add_argument("--limit", type=int)
    s.add_argument("--since", help="YYYY-MM-DD")
    s.set_defaults(fn=cmd_history)
    s = sub.add_parser("run")
    s.add_argument("revs", nargs="+")
    s.add_argument("--force", action="store_true")
    s.set_defaults(fn=cmd_run)
    s = sub.add_parser("compare")
    s.add_argument("a")
    s.add_argument("b")
    s.add_argument("--runs", type=int, default=8)
    s.set_defaults(fn=cmd_compare)
    sub.add_parser("list").set_defaults(fn=cmd_list)
    sub.add_parser("doctor").set_defaults(fn=cmd_doctor)
    s = sub.add_parser("report")
    s.add_argument("-o", "--output", default=str(BENCH / "report.html"))
    s.add_argument("--fragment", action="store_true",
                   help="omit doctype/html/head/body, for hosts that add their own (e.g. Artifacts)")
    s.set_defaults(fn=cmd_report)
    args = ap.parse_args()
    sys.path.insert(0, str(BENCH))
    args.fn(args)


if __name__ == "__main__":
    main()
