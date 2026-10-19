"""Render bench/history.jsonl as a self-contained "world record progression" page.

No external assets: SVG + vanilla JS, so the file works offline and can be attached anywhere.
"""
import json
import subprocess
import time
from pathlib import Path


def repo_url(root):
    try:
        u = subprocess.run(["git", "-C", str(root), "config", "--get", "remote.origin.url"],
                           capture_output=True, text=True).stdout.strip()
    except OSError:
        return ""
    if u.startswith("git@github.com:"):
        u = "https://github.com/" + u[len("git@github.com:"):]
    return u[:-4] if u.endswith(".git") else u


def build_payload(rows, metrics, points, records, anchor, config, extra_rows=(), sprt=()):
    data = {}
    point_rows = list(rows) + list(extra_rows)
    for key, (label, unit, lower) in metrics.items():
        pts = points(point_rows, key)
        data[key] = {"label": label, "unit": unit, "lower": lower, "points": pts,
                     "records": records(pts, lower, key),
                     # Elo is an offset from nothing in particular: a 0-based axis would flatten it.
                     "zero_based": unit != "Elo"}
    ok = [r for r in rows if r.get("status") == "ok"]
    env = ok[-1]["env"] if ok else {}
    statuses = {}
    for r in rows:
        statuses[r.get("status", "?")] = statuses.get(r.get("status", "?"), 0) + 1
    return {
        "sprt": list(sprt),
        "metrics": data,
        "meta": {
            "generated": int(time.time()),
            "repo": repo_url(Path(__file__).resolve().parent.parent),
            "env": env,
            "anchor": anchor,
            "commits": len(rows),
            "statuses": statuses,
            "depth": config["depth"],
            "depth_light": config["depth_light"],
            "movetime_ms": config["movetime_ms"],
            "passes": config["passes"],
            "image": config["image"],
        },
    }


def to_fragment(html):
    """The artifact host wraps pages in its own doctype/html/head/body and head meta, so a page
    published there must not carry those tags."""
    for old in ('<!doctype html>\n', '<html lang="en">\n', '<head>\n', '<meta charset="utf-8">\n',
                '<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">\n',
                '</head>\n', '<body>\n', '</body>\n', '</html>\n'):
        assert old in html, old
        html = html.replace(old, "", 1)
    return html


def write(rows, out, metrics, points, records, anchor, config, fragment=False, extra_rows=(), sprt=()):
    payload = build_payload(rows, metrics, points, records, anchor, config, extra_rows, sprt)
    blob = json.dumps(payload, separators=(",", ":")).replace("</", "<\\/")
    html = TEMPLATE.replace("/*__DATA__*/null", blob)
    Path(out).write_text(to_fragment(html) if fragment else html)


TEMPLATE = r"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>Lemonate Record Progression</title>
<meta name="claude:viewport-fit-sides" content="cover">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Chakra+Petch:wght@500;600;700&family=IBM+Plex+Mono:wght@400;500;600&display=swap">
<style>
/* Layout: one column, summary cards first, then the record chart with a replay, then the table. */
:root {
  color-scheme: dark;
  --display: "Chakra Petch", ui-sans-serif, system-ui, sans-serif;
  --mono: "IBM Plex Mono", ui-monospace, "SF Mono", Menlo, Consolas, monospace;
  --on-gold: #1a1405;
  --bg: #0b0e14; --panel: #121722; --panel2: #171d2b; --line: #263042; --text: #d9e1ee; --dim: #7f8ca3;
  --gold: #f5c542; --gold-dim: #8a6f1d; --good: #4ade80; --bad: #f87171; --dot: #5b6b87; --accent: #7cc4ff;
}
@media (prefers-color-scheme: light) {
  :root:not([data-theme="dark"]) {
    color-scheme: light;
    --on-gold: #fffaf0;
    --bg: #f4f1ea; --panel: #fffdf8; --panel2: #f0ebe0; --line: #d8d0bf; --text: #20242e; --dim: #6b7280;
    --gold: #b7791f; --gold-dim: #e6c27a; --good: #15803d; --bad: #b91c1c; --dot: #8a94a6; --accent: #1d6fb8;
  }
}
:root[data-theme="light"] {
  color-scheme: light;
  --on-gold: #fffaf0;
  --bg: #f4f1ea; --panel: #fffdf8; --panel2: #f0ebe0; --line: #d8d0bf; --text: #20242e; --dim: #6b7280;
  --gold: #b7791f; --gold-dim: #e6c27a; --good: #15803d; --bad: #b91c1c; --dot: #8a94a6; --accent: #1d6fb8;
}
* { box-sizing: border-box; }
html, body { margin: 0; background: var(--bg); color: var(--text); }
body { font: 15px/1.5 ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif;
  padding-block: 0 64px;
  padding-left: max(16px, env(safe-area-inset-left, 0px));
  padding-right: max(16px, env(safe-area-inset-right, 0px)); }
.mono { font-family: var(--mono); font-variant-numeric: tabular-nums; }
header { max-width: 1100px; margin: 0 auto; padding: 36px 0 12px; }
h1 { margin: 0; font-family: var(--display); font-weight: 700; font-size: clamp(22px, 4vw, 34px); letter-spacing: .04em; text-transform: uppercase; text-wrap: balance; }
h1 span { color: var(--gold); }
.sub { color: var(--dim); margin-top: 4px; }
main { max-width: 1100px; margin: 0 auto; }
.tabs { display: flex; flex-wrap: wrap; gap: 8px; margin: 18px 0 14px; }
.tab { font-family: var(--display); font-weight: 500; background: var(--panel); color: var(--dim); border: 1px solid var(--line); border-radius: 999px; padding: 6px 14px; cursor: pointer; font: inherit; }
.tab[aria-selected="true"] { color: var(--bg); background: var(--gold); border-color: var(--gold); font-weight: 600; }
.stats { display: grid; grid-template-columns: repeat(auto-fit, minmax(190px, 1fr)); gap: 10px; margin-bottom: 14px; }
.stat { background: var(--panel); border: 1px solid var(--line); border-radius: 10px; padding: 12px 14px; }
.stat .k { font-family: var(--display); color: var(--dim); font-size: 12px; text-transform: uppercase; letter-spacing: .08em; }
.stat .v { font-family: var(--mono); font-weight: 500; font-size: 24px; margin-top: 2px; }
.stat .d { color: var(--dim); font-size: 12px; }
.card { background: var(--panel); border: 1px solid var(--line); border-radius: 12px; padding: 14px; }
.controls { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; margin-bottom: 8px; }
button.btn { background: var(--panel2); color: var(--text); border: 1px solid var(--line); border-radius: 8px; padding: 6px 12px; cursor: pointer; font: inherit; }
button.btn:hover { border-color: var(--gold); }
button.primary { font-family: var(--display); background: var(--gold); color: var(--on-gold); border-color: var(--gold); font-weight: 700; }
input[type=range] { flex: 1 1 200px; accent-color: var(--gold); }
label.chk { color: var(--dim); display: flex; align-items: center; gap: 6px; }
.chartwrap { position: relative; }
svg { width: 100%; height: auto; display: block; }
.axis text { fill: var(--dim); font-size: 11px; }
.axis line, .axis path { stroke: var(--line); }
.grid line { stroke: var(--line); stroke-opacity: .55; stroke-dasharray: 2 4; }
.dot { fill: var(--dot); fill-opacity: .55; }
.dot.reg { fill: none; stroke: var(--bad); stroke-width: 1.5; fill-opacity: 1; }
.rec-line { fill: none; stroke: var(--gold); stroke-width: 3; stroke-linejoin: round; }
.rec-mark { fill: var(--gold); stroke: var(--bg); stroke-width: 2; }
.rec-label { fill: var(--gold); font-size: 11px; font-weight: 700; }
.cursor { stroke: var(--accent); stroke-width: 1; stroke-dasharray: 4 4; }
.tip { position: absolute; pointer-events: none; background: var(--panel2); border: 1px solid var(--line); border-radius: 8px; padding: 8px 10px; font-size: 12px; max-width: 320px; display: none; z-index: 5; }
.tip b { color: var(--gold); }
.banner { position: absolute; left: 70px; top: 14px; right: 30px; pointer-events: none; opacity: 0; transition: opacity .25s; }
.banner.on { opacity: 1; }
.banner .box { display: inline-block; background: color-mix(in srgb, var(--panel) 92%, transparent); border: 1px solid var(--gold); border-radius: 12px; padding: 12px 18px; box-shadow: 0 8px 30px rgb(0 0 0 / .35); max-width: 100%; }
.banner .nr { font-family: var(--display); color: var(--gold); font-weight: 800; letter-spacing: .18em; font-size: 12px; text-transform: uppercase; }
.banner .big { font-family: var(--mono); font-size: clamp(28px, 6vw, 46px); font-weight: 600; line-height: 1.1; }
.banner .delta { color: var(--good); font-size: 15px; }
.banner .msg { font-size: 14px; margin-top: 4px; }
.banner .who { color: var(--dim); font-size: 12px; }
table { width: 100%; border-collapse: collapse; margin-top: 8px; }
th, td { text-align: left; padding: 7px 10px; border-bottom: 1px solid var(--line); font-size: 13px; vertical-align: top; }
th { font-family: var(--display); color: var(--dim); font-weight: 600; text-transform: uppercase; font-size: 11px; letter-spacing: .08em; }
td.num, th.num { text-align: right; }
tr.current td { background: color-mix(in srgb, var(--gold) 9%, transparent); }
tr.current td:first-child { box-shadow: inset 3px 0 var(--gold); }
a { color: var(--accent); text-decoration: none; }
a:hover { text-decoration: underline; }
.good { color: var(--good); }
h2 { font-family: var(--display); font-size: 15px; text-transform: uppercase; letter-spacing: .1em; color: var(--dim); margin: 26px 0 6px; }
.foot { color: var(--dim); font-size: 12px; margin-top: 28px; line-height: 1.7; }
.empty { padding: 40px; text-align: center; color: var(--dim); }
@media (max-width: 640px) { .hide-sm { display: none; } th, td { padding: 6px 6px; } }
@media (prefers-reduced-motion: reduce) { .banner { transition: none; } }
</style>
</head>
<body>
<header>
  <h1>Lemonate <span>record progression</span></h1>
  <div class="sub" id="sub"></div>
</header>
<main>
  <div class="tabs" id="tabs" role="tablist"></div>
  <div class="stats" id="stats"></div>
  <div class="card">
    <div class="controls">
      <button class="btn primary" id="play">&#9654; Replay</button>
      <button class="btn" id="restart" title="Back to the start">&#8634;</button>
      <input type="range" id="scrub" min="0" max="0" step="1" value="0" aria-label="Scrub through records">
      <button class="btn" id="speed" title="Playback speed">1&times;</button>
      <label class="chk"><input type="checkbox" id="log"> log scale</label>
    </div>
    <div class="chartwrap" id="chartwrap">
      <svg id="chart" viewBox="0 0 1000 460" role="img" aria-label="Record progression chart"></svg>
      <div class="banner" id="banner"></div>
      <div class="tip" id="tip"></div>
    </div>
  </div>
  <h2>Record holders</h2>
  <div class="card" style="overflow-x:auto"><table id="table"></table></div>
  <h2>Confirmed head to head</h2>
  <div class="card" style="overflow-x:auto"><table id="sprt"></table></div>
  <div class="foot" id="sprtnote"></div>
  <div class="foot" id="foot"></div>
</main>
<script>
const DATA = /*__DATA__*/null;
const $ = (id) => document.getElementById(id);
const NS = "http://www.w3.org/2000/svg";
const W = 1000, H = 460, M = { l: 70, r: 30, t: 24, b: 40 };
let metric = Object.keys(DATA.metrics).find((k) => DATA.metrics[k].records.length) || Object.keys(DATA.metrics)[0];
let showUpTo = Infinity;       // timestamp cursor (replay)
let useLog = false;
let timer = null, speed = 1;

function fmtVal(v, m) {
  if (v == null) return "–";
  if (m.unit === "Elo") return Math.round(v) + " Elo";
  if (m.unit === "ms") return v >= 1000 ? (v / 1000).toFixed(2) + " s" : Math.round(v) + " ms";
  if (m.unit === "nps") return v >= 1e6 ? (v / 1e6).toFixed(2) + "M nps" : Math.round(v / 1000) + "k nps";
  if (m.unit === "nodes") return v >= 1e6 ? (v / 1e6).toFixed(2) + "M" : Math.round(v / 1000) + "k";
  return String(v);
}
// How much a record beat the previous one by: a percentage for speeds, points for Elo.
const gain = (r, m) => m.unit === "Elo" ? r.improvement_abs : r.improvement_pct;
const gainText = (r, m, d) => (m.lower ? "−" : "+") + (m.unit === "Elo" ? Math.round(gain(r, m)) + " Elo" : gain(r, m).toFixed(d) + "%");
const fmtDate = (ts) => new Date(ts * 1000).toISOString().slice(0, 10);
const commitLink = (p) => DATA.meta.repo ? `<a href="${DATA.meta.repo}/commit/${p.commit}" target="_blank" rel="noopener">${p.short}</a>` : p.short;
const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));

function el(name, attrs, parent) {
  const e = document.createElementNS(NS, name);
  for (const k in attrs) e.setAttribute(k, attrs[k]);
  if (parent) parent.appendChild(e);
  return e;
}

function niceTicks(lo, hi, n) {
  const span = hi - lo || 1, step0 = span / n, mag = Math.pow(10, Math.floor(Math.log10(step0)));
  const step = [1, 2, 2.5, 5, 10].map((s) => s * mag).find((s) => s >= step0) || mag * 10;
  const out = [];
  for (let v = Math.ceil(lo / step) * step; v <= hi + 1e-9; v += step) out.push(v);
  return out;
}

function tabs() {
  const t = $("tabs");
  t.innerHTML = "";
  for (const k in DATA.metrics) {
    const b = document.createElement("button");
    b.className = "tab"; b.role = "tab"; b.textContent = DATA.metrics[k].label;
    b.setAttribute("aria-selected", k === metric);
    b.onclick = () => { stop(); metric = k; showUpTo = Infinity; draw(); };
    t.appendChild(b);
  }
}

function stats(m) {
  const recs = m.records, s = $("stats");
  if (recs.length < 1) { s.innerHTML = ""; return; }
  const first = recs[0], cur = recs[recs.length - 1];
  const ratio = m.lower ? first.value / cur.value : cur.value / first.value;
  let big = recs.filter((r) => gain(r, m) != null).sort((a, b) => gain(b, m) - gain(a, m))[0];
  let long = recs.slice(0, -1).sort((a, b) => (b.held_days || 0) - (a.held_days || 0))[0];
  const card = (k, v, d) => `<div class="stat"><div class="k">${k}</div><div class="v mono">${v}</div><div class="d">${d || ""}</div></div>`;
  s.innerHTML =
    card("Current record", fmtVal(cur.value, m), `${commitLink(cur)} · ${fmtDate(cur.ts)}`) +
    card("Since the first measurement", ratio.toFixed(2) + "&times;", `${fmtVal(first.value, m)} &rarr; ${fmtVal(cur.value, m)}`) +
    card("Records broken", recs.length, `${m.points.length} versions measured`) +
    (big ? card("Biggest single jump", gainText(big, m, 1), esc(big.subject.slice(0, 44))) : "") +
    (long ? card("Longest-standing record", Math.round(long.held_days) + " days", `${fmtVal(long.value, m)} · ${commitLink(long)}`) : "");
}

function draw() {
  tabs();
  const m = DATA.metrics[metric], svg = $("chart");
  svg.innerHTML = "";
  stats(m);
  table(m);
  sprtTable();
  const pts = m.points;
  if (!pts.length) {
    $("banner").classList.remove("on");
    svg.innerHTML = '<text x="500" y="230" text-anchor="middle" fill="currentColor" style="fill:var(--dim)">No data for this category yet. Run: bench/bench.py history</text>';
    $("scrub").max = 0;
    return;
  }
  const t0 = pts[0].ts, t1 = Math.max(pts[pts.length - 1].ts, DATA.meta.generated);
  const vals = pts.map((p) => p.value);
  let lo = Math.min(...vals), hi = Math.max(...vals);
  const logOk = useLog && lo > 0;
  const f = (v) => logOk ? Math.log10(v) : v;
  const zero = m.zero_based !== false;
  const base = zero ? 0 : Math.floor((lo - 60) / 100) * 100;
  let ylo = logOk ? f(lo) - 0.05 : base, yhi = f(hi) + (logOk ? 0.05 : (hi - lo) * 0.06 + 1e-9);
  const X = (ts) => M.l + (ts - t0) / (t1 - t0 || 1) * (W - M.l - M.r);
  const Y = (v) => H - M.b - (f(v) - ylo) / (yhi - ylo || 1) * (H - M.t - M.b);

  // grid + axes
  const g = el("g", { class: "grid axis" }, svg);
  const yt = logOk ? [] : niceTicks(base, hi, 5);
  if (logOk) { for (let e = Math.floor(f(lo)); e <= Math.ceil(f(hi)); e++) for (const k of [1, 2, 5]) { const v = k * Math.pow(10, e); if (v >= lo * 0.9 && v <= hi * 1.1) yt.push(v); } }
  yt.forEach((v) => {
    el("line", { x1: M.l, x2: W - M.r, y1: Y(v), y2: Y(v) }, g);
    const tx = el("text", { x: M.l - 8, y: Y(v) + 4, "text-anchor": "end" }, g); tx.textContent = fmtVal(v, m);
  });
  const d0 = new Date(t0 * 1000), months = [];
  for (let d = new Date(Date.UTC(d0.getUTCFullYear(), d0.getUTCMonth() + 1, 1)); d.getTime() / 1000 < t1; d.setUTCMonth(d.getUTCMonth() + 1)) months.push(new Date(d));
  const every = months.length > 14 ? 3 : 1;
  months.forEach((d, i) => {
    if (i % every) return;
    const x = X(d.getTime() / 1000);
    el("line", { x1: x, x2: x, y1: H - M.b, y2: H - M.b + 5 }, g);
    const tx = el("text", { x, y: H - M.b + 18, "text-anchor": "middle" }, g);
    tx.textContent = d.toISOString().slice(0, 7);
  });

  const visible = pts.filter((p) => p.ts <= showUpTo);
  const recs = m.records.filter((r) => r.ts <= showUpTo);
  const recSet = new Set(recs.map((r) => r.commit));
  // regressions: noticeably worse than the previous measured version
  let prev = null;
  visible.forEach((p) => {
    const worse = prev && (m.lower ? p.value > prev.value * 1.08 : p.value < prev.value * 0.92);
    p._reg = !!worse; prev = p;
  });
  const tip = $("tip"), wrap = $("chartwrap");
  const showTip = (ev, p) => {
    const r = wrap.getBoundingClientRect();
    tip.innerHTML = `<b>${esc(p.short)}</b> &middot; ${fmtDate(p.ts)}<br>${esc(p.subject)}<br><span class="mono">${fmtVal(p.value, m)}</span> &middot; ${esc(p.author)}`;
    tip.style.display = "block";
    tip.style.left = Math.min(ev.clientX - r.left + 14, r.width - 330) + "px";
    tip.style.top = (ev.clientY - r.top + 14) + "px";
  };
  visible.forEach((p) => {
    if (recSet.has(p.commit)) return;
    const c = el("circle", { cx: X(p.ts), cy: Y(p.value), r: p._reg ? 4.5 : 3, class: "dot" + (p._reg ? " reg" : "") }, svg);
    c.addEventListener("mousemove", (e) => showTip(e, p));
    c.addEventListener("mouseleave", () => (tip.style.display = "none"));
  });

  // stepped record line
  if (recs.length) {
    const end = Math.min(showUpTo, t1);
    let d = `M ${X(recs[0].ts)} ${Y(recs[0].value)}`;
    for (let i = 1; i < recs.length; i++) d += ` H ${X(recs[i].ts)} V ${Y(recs[i].value)}`;
    d += ` H ${X(end)}`;
    el("path", { d, class: "rec-line" }, svg);
    recs.forEach((r, i) => {
      const c = el("circle", { cx: X(r.ts), cy: Y(r.value), r: 6.5, class: "rec-mark" }, svg);
      c.addEventListener("mousemove", (e) => showTip(e, r));
      c.addEventListener("mouseleave", () => (tip.style.display = "none"));
      if (gain(r, m) != null && (m.unit === "Elo" ? gain(r, m) >= 20 : gain(r, m) >= 3)) {
        const up = m.lower;
        const tx = el("text", { x: X(r.ts), y: Y(r.value) + (up ? 22 : -14), "text-anchor": "middle", class: "rec-label" }, svg);
        tx.textContent = gainText(r, m, 0);
      }
    });
  }
  if (isFinite(showUpTo) && showUpTo < t1) el("line", { x1: X(showUpTo), x2: X(showUpTo), y1: M.t, y2: H - M.b, class: "cursor" }, svg);
  $("scrub").max = m.records.length - 1;
  $("sub").textContent = `${m.label} · ${m.lower ? "lower" : "higher"} is better · ${pts.length} versions · ${fmtDate(t0)} → ${fmtDate(pts[pts.length - 1].ts)}`;
}

function table(m) {
  const recs = m.records, t = $("table");
  if (!recs.length) { t.innerHTML = '<tr><td class="empty">No records yet.</td></tr>'; return; }
  let h = '<tr><th>#</th><th>Date</th><th>Commit</th><th class="num">Result</th><th class="num">Change</th><th class="num hide-sm">Held</th><th>What changed</th></tr>';
  recs.forEach((r, i) => {
    const imp = gain(r, m) == null ? "–" : `<span class="good">${gainText(r, m, 1)}</span>`;
    h += `<tr class="${r.current ? "current" : ""}"><td class="mono">${i + 1}</td><td class="mono">${fmtDate(r.ts)}</td><td class="mono">${commitLink(r)}</td>` +
      `<td class="num mono">${fmtVal(r.value, m)}</td><td class="num mono">${imp}</td>` +
      `<td class="num mono hide-sm">${r.current ? "now" : Math.round(r.held_days || 0) + " d"}</td><td>${esc(r.subject)}<div class="who">${esc(r.author)}</div></td></tr>`;
  });
  t.innerHTML = h;
}

function sprtTable() {
  const t = $("sprt"), rows = DATA.sprt || [];
  $("sprtnote").textContent = "Each row is one change played against the version just before it (SPRT, 95% error bars). " +
    "This is the sharp test for small gains; the Elo tab above only shows the overall trend, with about \u00b140 Elo of error per version.";
  if (!rows.length) { t.innerHTML = '<tr><td class="empty">No head-to-head results yet.</td></tr>'; return; }
  let h = '<tr><th>Experiment</th><th>Commit</th><th class="num">Games</th><th class="num">Elo</th><th>Verdict</th></tr>';
  rows.forEach((r) => {
    const elo = r.elo == null ? "\u2013" : `${r.elo > 0 ? "+" : ""}${r.elo.toFixed(0)} \u00b1 ${r.elo_err.toFixed(0)}`;
    const ok = r.verdict === "H1", cls = ok ? "good" : "";
    const word = ok ? "better" : r.verdict === "H0" ? "no gain" : "inconclusive";
    h += `<tr><td>${esc(r.name)}</td><td class="mono">${esc(r.baseline)} &rarr; ${esc(r.candidate)}</td><td class="num mono">${r.games}</td>` +
      `<td class="num mono ${cls}">${elo}</td><td class="${cls}">${word}</td></tr>`;
  });
  t.innerHTML = h;
}

// ---- replay
function banner(r, m, from) {
  const b = $("banner");
  const imp = gain(r, m) == null ? "First measurement" : `${fmtVal(r.previous, m)} &rarr; ${fmtVal(r.value, m)} &nbsp;(${gainText(r, m, 1)})`;
  const heldPrev = (m.records[m.records.indexOf(r) - 1] || {}).held_days;
  b.innerHTML = `<div class="box"><div class="nr">${gain(r, m) == null ? "Baseline" : "New record"} #${m.records.indexOf(r) + 1}</div>` +
    `<div class="big mono" id="bignum">${fmtVal(from, m)}</div><div class="delta mono">${imp}</div>` +
    `<div class="msg">${esc(r.subject)}</div><div class="who">${esc(r.author)} · ${commitLink(r)} · ${fmtDate(r.ts)}` +
    `${heldPrev != null ? " · previous record stood for " + Math.round(heldPrev) + " days" : ""}</div></div>`;
  b.classList.add("on");
  // count the headline number down/up to the new value
  const t0 = performance.now(), dur = 700 / speed, to = r.value;
  const tick = (now) => {
    const k = Math.min(1, (now - t0) / dur), e = 1 - Math.pow(1 - k, 3);
    const n = $("bignum"); if (!n) return;
    n.textContent = fmtVal(from + (to - from) * e, m);
    if (k < 1) requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
}

function stop() {
  if (timer) clearTimeout(timer);
  timer = null;
  $("play").innerHTML = "&#9654; Replay";
  $("banner").classList.remove("on");
}

function tween(from, to, ms, step, done) {
  const t0 = performance.now();
  const f = (now) => {
    if (!timer) return;
    const k = Math.min(1, (now - t0) / (ms / speed));
    step(from + (to - from) * k);
    if (k < 1) requestAnimationFrame(f); else done();
  };
  requestAnimationFrame(f);
}

function play(startIdx) {
  const m = DATA.metrics[metric];
  if (!m.records.length) return;
  stop();
  timer = setTimeout(() => {}, 0);
  $("play").innerHTML = "&#10074;&#10074; Pause";
  const recs = m.records, firstTs = m.points[0].ts;
  let i = startIdx || 0;
  showUpTo = i === 0 ? firstTs : recs[i - 1].ts;
  draw();
  const next = () => {
    if (!timer) return;
    if (i >= recs.length) {
      // run on to "now" and finish
      tween(showUpTo, DATA.meta.generated, 1200, (v) => { showUpTo = v; draw(); }, () => { showUpTo = Infinity; draw(); stop(); });
      return;
    }
    const r = recs[i], from = i ? recs[i - 1].value : r.value;
    $("banner").classList.remove("on");
    tween(showUpTo, r.ts, i === 0 ? 300 : 1300, (v) => { showUpTo = v; draw(); }, () => {
      showUpTo = r.ts; draw(); $("scrub").value = i;
      banner(r, m, from);
      i++;
      timer = setTimeout(next, (i >= recs.length ? 3200 : 2400) / speed);
    });
  };
  next();
}

$("play").onclick = () => { if (timer) { stop(); showUpTo = Infinity; draw(); } else play(0); };
$("restart").onclick = () => { stop(); showUpTo = Infinity; draw(); };
$("speed").onclick = () => { speed = speed === 1 ? 2 : speed === 2 ? 4 : 1; $("speed").innerHTML = speed + "&times;"; };
$("log").onchange = (e) => { useLog = e.target.checked; draw(); };
$("scrub").oninput = (e) => {
  stop();
  const m = DATA.metrics[metric], r = m.records[+e.target.value];
  if (!r) return;
  showUpTo = r.ts; draw();
  banner(r, m, r.previous == null ? r.value : r.previous);
};

(function init() {
  const mt = DATA.meta, e = mt.env || {};
  const st = Object.entries(mt.statuses).map(([k, v]) => `${v} ${k}`).join(", ");
  $("foot").innerHTML =
    `Suite: 12 positions, depth ${mt.depth} (and depth ${mt.depth_light} for the light category), fixed-time ${mt.movetime_ms} ms per position for NPS, min of ${mt.passes} passes, one thread.<br>` +
    `Machine: ${esc(e.cpu || "?")} · governor ${esc(e.governor || "?")} · pinned to cpu ${esc(e.pinned_cpu ?? "?")} · kernel ${esc(e.kernel || "?")}<br>` +
    `Toolchain: ${esc(mt.image)} · rows: ${mt.commits} commits (${st})<br>` +
    `Speed and node counts are measured separately on purpose: a smarter search can reach a depth with fewer nodes at the same NPS, and a faster engine can do it at the same node count. ` +
    `Timings are corrected for machine drift against an anchor binary re-timed every few commits. Generated ${new Date(mt.generated * 1000).toISOString().slice(0, 16).replace("T", " ")} UTC.`;
  draw();
})();
</script>
</body>
</html>
"""
