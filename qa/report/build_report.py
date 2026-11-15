#!/usr/bin/env python3
"""Build the "Isonomic UI QA" report page from a run's results.

  build_report.py --run qa/out [--meta <dir>] --dest qa/out/report

`--meta` is optional: a folder with tested.json (the commit each component was
tested at), queue.json (the component queue) and oracle.json (each
component's stated behaviour). Without it the page is built from the results
alone.

Reads qa/out/<slug>/results.json, qa/review/notes.yaml, qa/out/issues.json and
qa/out/history.json, and writes index.html with every image beside it under img/.
"""
from __future__ import annotations

import argparse
import hashlib
import html
import json
import shutil
import time
from pathlib import Path

import yaml
from PIL import Image, ImageChops

QA = Path(__file__).resolve().parent.parent
CHECKS = ["C1", "C2", "C3", "C4", "C5", "C6", "C7", "C8", "C9", "C10", "C11"]
CHECK_NAMES = {"C1": "Renders", "C2": "Layout", "C3": "Theme", "C4": "Stated behaviour", "C5": "States",
               "C6": "Keyboard", "C7": "Content stress", "C8": "Motion and time", "C9": "Accessibility",
               "C10": "Performance", "C11": "Code health"}


def crop_content(src: Path, pad=24, keep_width=False) -> Image.Image:
    im = Image.open(src).convert("RGB")
    bg = im.getpixel((im.width - 2, im.height - 2))
    box = ImageChops.difference(im, Image.new("RGB", im.size, bg)).convert("L").point(
        lambda v: 255 if v > 12 else 0).getbbox()
    if not box:
        return im.crop((0, 0, im.width, min(im.height, 200)))
    right = im.width if keep_width else min(im.width, box[2] + pad)
    return im.crop((0, 0, right, min(im.height, box[3] + pad)))


INLINE = True


def save_jpeg(im: Image.Image, dest: Path, max_w: int | None = None) -> str:
    """Save a JPEG beside the page and return how the page refers to it.

    With INLINE the page carries the image as a data: URI, so the report is one file.
    """
    import base64
    from io import BytesIO
    dest.parent.mkdir(parents=True, exist_ok=True)
    if max_w and im.width > max_w:
        im = im.resize((max_w, round(im.height * max_w / im.width)), Image.LANCZOS)
    buf = BytesIO()
    im.save(buf, "JPEG", quality=72, optimize=True, progressive=True)
    dest.write_bytes(buf.getvalue())
    if INLINE:
        return "data:image/jpeg;base64," + base64.b64encode(buf.getvalue()).decode()
    return str(dest.relative_to(dest.parents[2]))


def verdict(checks: dict, issues: list[dict]) -> str:
    if not checks:
        return "Blocked"
    open_issues = [i for i in issues if i.get("state", "open") == "open"]
    if checks.get("C1", {}).get("status") == "fail" or any(i["sev"] in (1, 2) for i in open_issues):
        return "Needs fixes"
    if "C4" not in checks:
        return "Pending"
    if open_issues or any(v.get("status") in ("warn", "fail") for v in checks.values()):
        return "Pass with polish"
    return "Pass"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", default=str(QA / "out"))
    ap.add_argument("--meta", default="")
    ap.add_argument("--dest", default=str(QA / "out/report"))
    ap.add_argument("--label", default="")
    args = ap.parse_args()
    run, dest = Path(args.run), Path(args.dest)
    meta = Path(args.meta) if args.meta else run / "meta"
    if dest.exists():
        shutil.rmtree(dest)
    (dest / "img").mkdir(parents=True)

    def meta_file(name, default):
        path = meta / name
        return json.loads(path.read_text()) if path.exists() else default

    tested = meta_file("tested.json", {})
    queue = meta_file("queue.json", [])
    oracle = meta_file("oracle.json", {})
    notes = yaml.safe_load((QA / "review/notes.yaml").read_text()) or {}
    issues_path = run / "issues.json"
    issues = json.loads(issues_path.read_text()) if issues_path.exists() else []
    hist_path = run / "history.json"
    history = json.loads(hist_path.read_text()) if hist_path.exists() else []
    built = meta_file("built.json", {})

    components = []
    for res_path in sorted(run.glob("*/results.json")):
        res = json.loads(res_path.read_text())
        slug = res["slug"]
        branch = "permission-prompt-v2" if slug == "permission-prompt" else slug
        t = tested.get(branch, {})
        orc = oracle.get(branch, {})
        checks = dict(res.get("checks", {}))
        for n in notes.get(slug, []) or []:
            c = n["check"]
            if n["status"] == "known":
                prev = checks.get(c, {})
                if prev.get("status") == "warn":
                    checks[c] = {"status": "pass", "note": "known limit: " + n["note"]}
                continue
            if n["status"] == "info":
                prev = checks.get(c)
                if prev:
                    prev["note"] = "; ".join(x for x in [prev.get("note", ""), n["note"]] if x)
                else:
                    checks[c] = {"status": None, "note": n["note"]}
                continue
            prev = checks.get(c, {"status": "pass", "note": ""})
            order = {"fail": 3, "warn": 2, "info": 1, "pass": 0}
            status = prev["status"] if order.get(prev["status"], 0) >= order.get(n["status"], 0) else n["status"]
            if status == "info":
                status = prev["status"] if prev.get("status") else "pass"
            joined = "; ".join(x for x in [prev.get("note", "") if prev.get("status") != "pass" else "", n["note"]] if x)
            checks[c] = {"status": status, "note": joined or prev.get("note", "")}
        comp_issues = [i for i in issues if i["slug"] == slug]
        imgs = {}
        for key in ["light-360", "light-960", "light-1440", "dark-360", "dark-960", "dark-1440"]:
            src = run / slug / "shots" / f"{key}.png"
            if src.exists():
                imgs[key] = save_jpeg(crop_content(src, keep_width=key.endswith("360")),
                                      dest / f"img/{slug}/{key}.jpg", 900)
        for key in ["light-960", "dark-960"]:
            src = run / slug / "shots" / f"{key}.png"
            if src.exists():
                imgs["thumb-" + key] = save_jpeg(crop_content(src), dest / f"img/{slug}/thumb-{key}.jpg", 400)
        for k, v in res.get("shots", {}).items():
            if k.startswith("step:"):
                src = run / v
                if src.exists():
                    imgs[k] = save_jpeg(crop_content(src), dest / f"img/{slug}/{k[5:]}.jpg", 900)
        if (run / slug / "contact.png").exists():
            imgs["contact"] = save_jpeg(Image.open(run / slug / "contact.png").convert("RGB"),
                                        dest / f"img/{slug}/contact.jpg", 1000)
        components.append({
            "slug": slug, "branch": branch, "pr": t.get("pr"), "sha": t.get("sha", ""),
            "checks": checks, "steps": res.get("steps", []), "a11y": res.get("a11y", {}),
            "behaviour": orc.get("behaviour", []), "limits": orc.get("not_done", []),
            "issues": comp_issues, "imgs": imgs, "verdict": verdict(checks, comp_issues),
            "tested_at": res.get("finished", ""),
        })

    by_slug = {c["slug"]: c for c in components}
    tier_of = {}
    for q in queue:
        tier_of[q["slug"]] = q["tier"]
    for c in components:
        c["tier"] = tier_of.get(c["branch"], tier_of.get(c["slug"], ""))
        c["name"] = next((q["name"] for q in queue if q["slug"] in (c["branch"], c["slug"])),
                         c["slug"].replace("-", " ").capitalize())

    counts = {v: sum(1 for c in components if c["verdict"] == v)
              for v in ["Pass", "Pass with polish", "Needs fixes", "Blocked", "Pending"]}
    integ = hashlib.sha1("\n".join(f"{k}:{v['sha']}" for k, v in sorted(tested.items())).encode()).hexdigest()[:12]
    run_info = {
        "date": time.strftime("%Y-%m-%d %H:%M UTC", time.gmtime()),
        "label": args.label,
        "integration": integ,
        "scaffold": tested.get("scaffold", {}).get("sha", "")[:10],
        "branches": len(tested),
        "built": built.get("count", len(components)),
        "tested": len(components),
        "queue": len(queue) or len(components),
        "counts": counts,
        "issues_open": sum(1 for i in issues if i.get("state", "open") == "open"),
        "not_tested": built.get("not_tested", []),
    }
    entry = {"date": run_info["date"], "label": args.label, "integration": integ, "tested": len(components),
             "counts": counts, "issues_open": run_info["issues_open"]}
    if not history or history[-1].get("label") != args.label:
        history.append(entry)
    else:
        history[-1] = entry
    hist_path.write_text(json.dumps(history, indent=1))

    visual = json.loads((run / "visual-changes.json").read_text()) if (run / "visual-changes.json").exists() else []

    data = {"run": run_info, "components": components, "history": history, "visual": visual,
            "checkNames": CHECK_NAMES, "checks": CHECKS,
            "repo": "https://github.com/isonomic-org/isonomic-ui"}
    page = TEMPLATE.replace("/*DATA*/null", json.dumps(data).replace("</", "<\\/"))
    (dest / "index.html").write_text(page)
    print(f"wrote {dest/'index.html'}: {len(components)} components, {sum(1 for _ in dest.rglob('*.jpg'))} images")


TEMPLATE = r"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Isonomic UI QA</title>
<style>
:root {
  /* Layout: one column; summary strip, then what is broken, then the component grid. Detail opens on its own hash. */
  --bg: #f7f7f8; --surface: #ffffff; --ink: #18181b; --quiet: #6b6b76; --line: #e4e4e9; --accent: #4b5bd6;
  --ok: #1f8a4c; --warn: #b26b00; --bad: #c2362f; --idle: #a1a1aa;
  --ok-bg: #e7f5ec; --warn-bg: #fdf3e1; --bad-bg: #fbe9e7; --idle-bg: #f0f0f3;
  --display: system-ui, sans-serif;
  --body: system-ui, sans-serif;
  --mono: ui-monospace, monospace;
  color-scheme: light;
}
@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) {
  --bg: #111114; --surface: #18181c; --ink: #ececf1; --quiet: #9a9aa6; --line: #2a2a31; --accent: #8f9bff;
  --ok: #4cc07c; --warn: #e3a63c; --bad: #f0746b; --idle: #6b6b76;
  --ok-bg: #16281e; --warn-bg: #2e2412; --bad-bg: #321a18; --idle-bg: #222228; color-scheme: dark; } }
:root[data-theme="dark"] {
  --bg: #111114; --surface: #18181c; --ink: #ececf1; --quiet: #9a9aa6; --line: #2a2a31; --accent: #8f9bff;
  --ok: #4cc07c; --warn: #e3a63c; --bad: #f0746b; --idle: #6b6b76;
  --ok-bg: #16281e; --warn-bg: #2e2412; --bad-bg: #321a18; --idle-bg: #222228; color-scheme: dark; }
* { box-sizing: border-box; }
html, body { margin: 0; background: var(--bg); color: var(--ink); font: 14px/1.5 var(--body); }
a { color: var(--accent); text-decoration: none; } a:hover { text-decoration: underline; }
a:focus-visible, button:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; border-radius: 4px; }
.wrap { max-width: 1240px; margin: 0 auto; padding: 28px 20px 64px; display: grid; gap: 28px; }
header { display: grid; gap: 6px; }
h1 { font: 600 22px/1.25 var(--display); margin: 0; letter-spacing: -0.01em; }
h2 { font: 600 15px/1.3 var(--display); margin: 0 0 10px; }
h3 { font: 600 13px/1.3 var(--display); margin: 0 0 6px; color: var(--quiet); text-transform: uppercase; letter-spacing: .04em; }
.meta { color: var(--quiet); font-size: 13px; display: flex; flex-wrap: wrap; gap: 4px 16px; }
.mono { font-family: var(--mono); font-size: 12px; }
.strip { display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 1px; background: var(--line); border: 1px solid var(--line); border-radius: 10px; overflow: hidden; }
.tile { background: var(--surface); padding: 14px 16px; display: grid; gap: 2px; }
.tile b { font: 600 22px/1.2 var(--display); font-variant-numeric: tabular-nums; }
.tile span { color: var(--quiet); font-size: 12px; }
.coverage { font-size: 13px; color: var(--quiet); }
.coverage strong { color: var(--ink); font-weight: 500; }
.bar { height: 6px; border-radius: 3px; background: var(--idle-bg); display: flex; overflow: hidden; margin-top: 8px; }
.bar i { display: block; height: 100%; }
.panel { background: var(--surface); border: 1px solid var(--line); border-radius: 10px; padding: 16px; }
.broken { display: grid; gap: 8px; margin: 0; padding: 0; list-style: none; }
.broken li { display: grid; grid-template-columns: 96px 1fr auto; gap: 12px; align-items: center; padding: 8px; border-radius: 8px; border: 1px solid var(--line); }
.broken img { width: 96px; height: 56px; object-fit: cover; object-position: top left; border-radius: 4px; border: 1px solid var(--line); }
.sev { font: 500 11px/1 var(--body); padding: 3px 6px; border-radius: 4px; white-space: nowrap; }
.sev1, .sev2 { background: var(--bad-bg); color: var(--bad); } .sev3 { background: var(--warn-bg); color: var(--warn); } .sev4 { background: var(--idle-bg); color: var(--quiet); }
.filters { display: flex; flex-wrap: wrap; gap: 6px; margin-bottom: 12px; }
.filters button { font: inherit; font-size: 12px; padding: 4px 10px; border-radius: 999px; border: 1px solid var(--line); background: var(--surface); color: var(--ink); cursor: pointer; }
.filters button[aria-pressed="true"] { background: var(--ink); color: var(--bg); border-color: var(--ink); }
.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(270px, 1fr)); gap: 12px; }
.card { background: var(--surface); border: 1px solid var(--line); border-radius: 10px; overflow: hidden; display: grid; grid-template-rows: auto 1fr; color: inherit; }
.card:hover { border-color: var(--quiet); text-decoration: none; }
.thumbs { display: grid; grid-template-columns: 1fr 1fr; gap: 1px; background: var(--line); height: 120px; }
.thumbs img { width: 100%; height: 120px; object-fit: cover; object-position: top left; display: block; background: var(--bg); }
.card .body { padding: 10px 12px 12px; display: grid; gap: 8px; }
.row { display: flex; align-items: center; gap: 8px; justify-content: space-between; }
.name { font-weight: 500; }
.pill { font-size: 11px; font-weight: 500; padding: 2px 8px; border-radius: 999px; white-space: nowrap; }
.v-Pass { background: var(--ok-bg); color: var(--ok); } .v-Pass-with-polish { background: var(--warn-bg); color: var(--warn); }
.v-Needs-fixes { background: var(--bad-bg); color: var(--bad); } .v-Blocked, .v-Pending { background: var(--idle-bg); color: var(--quiet); }
.dots { display: flex; gap: 3px; }
.dot { width: 18px; height: 18px; border-radius: 4px; font: 500 9px/18px var(--mono); text-align: center; color: var(--surface); }
.d-pass { background: var(--ok); } .d-warn { background: var(--warn); } .d-fail { background: var(--bad); } .d-none { background: var(--idle-bg); color: var(--quiet); }
.small { font-size: 12px; color: var(--quiet); }
.detail { display: grid; gap: 20px; }
.back { font-size: 13px; }
.shots { display: grid; grid-template-columns: repeat(auto-fill, minmax(320px, 1fr)); gap: 12px; }
figure { margin: 0; display: grid; gap: 6px; }
figure img { width: 100%; border: 1px solid var(--line); border-radius: 6px; background: var(--bg); }
figcaption { font-size: 12px; color: var(--quiet); }
table { width: 100%; border-collapse: collapse; font-size: 13px; }
td, th { text-align: left; padding: 8px 6px; border-top: 1px solid var(--line); vertical-align: top; }
th { font-weight: 500; color: var(--quiet); font-size: 12px; border-top: 0; }
td.num { font-variant-numeric: tabular-nums; }
.scroll { overflow-x: auto; }
ul.plain { margin: 0; padding-left: 18px; display: grid; gap: 4px; }
.empty { color: var(--quiet); font-size: 13px; padding: 4px 0; }
@media (max-width: 520px) { .broken li { grid-template-columns: 1fr; } .broken img { width: 100%; height: 90px; } }
@media (prefers-reduced-motion: reduce) { * { transition: none !important; } }
</style>
</head>
<body>
<div class="wrap" id="app"></div>
<script>
const DATA = /*DATA*/null;
const esc = s => String(s ?? "").replace(/[&<>"]/g, c => ({"&":"&amp;","<":"&lt;",">":"&gt;","\"":"&quot;"}[c]));
const vclass = v => "v-" + v.replace(/ /g, "-");
const R = DATA.repo;
function dots(c) {
  return `<div class="dots" aria-label="checks">` + DATA.checks.map(k => {
    const s = c.checks[k]?.status; const cls = s === "pass" ? "d-pass" : s === "warn" ? "d-warn" : s === "fail" ? "d-fail" : "d-none";
    return `<span class="dot ${cls}" title="${k} ${esc(DATA.checkNames[k])}: ${esc(s || "not run yet")}">${k.slice(1)}</span>`;
  }).join("") + `</div>`;
}
function overview(filter) {
  const run = DATA.run, comps = DATA.components;
  const k = run.counts;
  const issues = comps.flatMap(c => c.issues.filter(i => (i.state||"open") === "open").map(i => ({...i, c})))
    .sort((a, b) => a.sev - b.sev || a.number - b.number);
  const total = run.queue;
  const seg = (n, color) => `<i style="width:${(100*n/total).toFixed(2)}%;background:var(${color})"></i>`;
  const shown = comps.filter(c => !filter || filter === "All" || c.verdict === filter);
  return `
  <header>
    <h1>Isonomic UI QA</h1>
    <div class="meta"><span>Run ${esc(run.label)} · ${esc(run.date)}</span>
      <span>Integration set <span class="mono">${esc(run.integration)}</span>: extra/scaffold <span class="mono">${esc(run.scaffold)}</span> + ${run.branches - 1} component branches</span>
      <span>Linux, virtual display, software rendering</span></div>
  </header>
  <section>
    <div class="strip">
      <div class="tile"><b>${run.built}</b><span>built (open PRs)</span></div>
      <div class="tile"><b>${run.tested}</b><span>tested this run</span></div>
      <div class="tile"><b>${k["Pass"]}</b><span>Pass</span></div>
      <div class="tile"><b>${k["Pass with polish"]}</b><span>Pass with polish</span></div>
      <div class="tile"><b>${k["Needs fixes"]}</b><span>Needs fixes</span></div>
      <div class="tile"><b>${k["Pending"] + k["Blocked"]}</b><span>Pending or blocked</span></div>
      <div class="tile"><b>${run.issues_open}</b><span>open QA issues</span></div>
    </div>
    <div class="coverage" style="margin-top:10px"><strong>${run.built} of ${total} built · ${run.tested} tested · ${k["Pass"]} pass</strong>
      ${run.not_tested.length ? ` · not yet tested: ${run.not_tested.map(esc).join(", ")}` : ""}
      <div class="bar" role="img" aria-label="coverage">${seg(k["Pass"], "--ok")}${seg(k["Pass with polish"], "--warn")}${seg(k["Needs fixes"], "--bad")}${seg(k["Pending"]+k["Blocked"], "--idle")}${seg(run.built - run.tested, "--line")}</div>
    </div>
  </section>
  <section class="panel">
    <h2>What's broken</h2>
    ${issues.length ? `<ul class="broken">${issues.map(i => `<li>
      ${i.c.imgs["thumb-light-960"] ? `<img src="${i.c.imgs["thumb-light-960"]}" alt="">` : "<span></span>"}
      <div><div><span class="sev sev${i.sev}">Sev${i.sev}</span> <a href="#c-${i.c.slug}">${esc(i.title)}</a></div>
      <div class="small"><a href="${R}/issues/${i.number}">#${i.number}</a> · PR <a href="${R}/pull/${i.c.pr}">#${i.c.pr}</a> · ${esc(i.kind)}</div></div>
      <span class="small">${esc(i.c.name)}</span></li>`).join("")}</ul>`
      : `<p class="empty">No open QA issues. ${DATA.run.label.startsWith("Q0") ? "This run covers rendering, layout, theme and code health (C1–C3, C11); behaviour scripts (C4–C10) come in the next run." : ""}</p>`}
  </section>
  <section>
    <h2>Components</h2>
    <div class="filters" role="group" aria-label="Filter by verdict">${["All","Needs fixes","Pass with polish","Pass","Pending","Blocked"].map(f =>
      `<button type="button" data-f="${f}" aria-pressed="${(filter||"All")===f}">${f}</button>`).join("")}</div>
    <div class="grid">${shown.map(c => `
      <a class="card" href="#c-${c.slug}">
        <div class="thumbs">${c.imgs["thumb-light-960"] ? `<img src="${c.imgs["thumb-light-960"]}" alt="${esc(c.name)}, light theme">` : "<span></span>"}
          ${c.imgs["thumb-dark-960"] ? `<img src="${c.imgs["thumb-dark-960"]}" alt="${esc(c.name)}, dark theme">` : "<span></span>"}</div>
        <div class="body">
          <div class="row"><span class="name">${esc(c.name)}</span><span class="pill ${vclass(c.verdict)}">${esc(c.verdict)}</span></div>
          ${dots(c)}
          <div class="small">${esc(c.tier)} · PR #${c.pr} · <span class="mono">${esc(c.sha.slice(0,7))}</span>${c.issues.length ? ` · ${c.issues.length} issue${c.issues.length>1?"s":""}` : ""}</div>
        </div></a>`).join("")}</div>
  </section>
  <section class="panel">
    <h2>Visual changes since last run</h2>
    ${DATA.visual.length ? `<ul class="plain">${DATA.visual.map(v => `<li><a href="#c-${v.slug}">${esc(v.slug)}</a>: ${esc(v.note)}</li>`).join("")}</ul>`
      : `<p class="empty">This is the first full run; its screenshots are the proposed baselines. Later runs compare against them pixel by pixel.</p>`}
  </section>
  <section class="panel">
    <h2>Run history</h2>
    <div class="scroll"><table><thead><tr><th>Run</th><th>Date</th><th>Integration set</th><th>Tested</th><th>Pass</th><th>Polish</th><th>Needs fixes</th><th>Pending</th><th>Open issues</th></tr></thead>
    <tbody>${DATA.history.slice().reverse().map(h => `<tr><td>${esc(h.label)}</td><td>${esc(h.date)}</td><td class="mono">${esc(h.integration)}</td>
      <td class="num">${h.tested}</td><td class="num">${h.counts["Pass"]}</td><td class="num">${h.counts["Pass with polish"]}</td><td class="num">${h.counts["Needs fixes"]}</td>
      <td class="num">${h.counts["Pending"]+h.counts["Blocked"]}</td><td class="num">${h.issues_open}</td></tr>`).join("")}</tbody></table></div>
  </section>`;
}
function detail(c) {
  const keys = ["light-360","light-960","light-1440","dark-360","dark-960","dark-1440"];
  const stepShots = Object.keys(c.imgs).filter(k => k.startsWith("step:"));
  return `<div class="detail">
  <a class="back" href="#overview">← All components</a>
  <header><h1>${esc(c.name)} <span class="pill ${vclass(c.verdict)}" style="vertical-align:middle">${esc(c.verdict)}</span></h1>
    <div class="meta"><span>${esc(c.tier)}</span><a href="${R}/pull/${c.pr}">PR #${c.pr}</a><span>branch <span class="mono">extra/${esc(c.branch)}</span> @ <span class="mono">${esc(c.sha.slice(0,10))}</span></span>
    <span>tested ${esc(c.tested_at)}</span></div></header>
  <section class="panel"><h2>Checks</h2><div class="scroll"><table><thead><tr><th></th><th>Check</th><th>Result</th><th>Notes</th></tr></thead><tbody>
    ${DATA.checks.map(k => { const v = c.checks[k]; const s = v?.status;
      return `<tr><td><span class="dot ${s==="pass"?"d-pass":s==="warn"?"d-warn":s==="fail"?"d-fail":"d-none"}">${k.slice(1)}</span></td><td>${k} ${esc(DATA.checkNames[k])}</td>
      <td>${esc(s || "not run yet")}</td><td>${esc(v?.note || "")}</td></tr>`; }).join("")}
  </tbody></table></div></section>
  ${c.issues.length ? `<section class="panel"><h2>Issues</h2><ul class="plain">${c.issues.map(i => `<li><span class="sev sev${i.sev}">Sev${i.sev}</span> <a href="${R}/issues/${i.number}">#${i.number} ${esc(i.title)}</a> · ${esc(i.state||"open")}</li>`).join("")}</ul></section>` : ""}
  <section><h2>Screenshots</h2><div class="shots">${keys.filter(k => c.imgs[k]).map(k => `<figure><img loading="lazy" src="${c.imgs[k]}" alt="${esc(c.name)}, ${k}"><figcaption>${k.replace("-", " theme, ")} px</figcaption></figure>`).join("")}</div></section>
  ${stepShots.length ? `<section><h2>States from the script</h2><div class="shots">${stepShots.map(k => `<figure><img loading="lazy" src="${c.imgs[k]}" alt="${esc(k.slice(5))}"><figcaption>${esc(k.slice(5))}</figcaption></figure>`).join("")}</div></section>` : ""}
  ${c.steps.length ? `<section class="panel"><h2>Script steps</h2><div class="scroll"><table><thead><tr><th>#</th><th>Step</th><th>Check</th><th>Result</th><th>Notes</th></tr></thead><tbody>
    ${c.steps.map(s => `<tr><td class="num">${s.n}</td><td>${esc(s.name)}</td><td>${esc(s.check)}</td><td>${s.pass ? "pass" : "fail"}</td><td>${esc(s.note)}</td></tr>`).join("")}</tbody></table></div></section>` : ""}
  <section class="panel"><h2>Stated behaviour (from the PR)</h2>${c.behaviour.length ? `<ul class="plain">${c.behaviour.map(b => `<li>${esc(b)}</li>`).join("")}</ul>` : `<p class="empty">None listed.</p>`}
    <h3 style="margin-top:14px">Known limits (not counted as defects)</h3>${c.limits.length ? `<ul class="plain">${c.limits.map(b => `<li>${esc(b)}</li>`).join("")}</ul>` : `<p class="empty">None listed.</p>`}</section>
  ${c.imgs.contact ? `<section><h2>Contact sheet</h2><figure><img loading="lazy" src="${c.imgs.contact}" alt="Contact sheet"><figcaption>Light and dark at 360, 960 and 1440 px</figcaption></figure></section>` : ""}
  </div>`;
}
let filter = "All";
function render() {
  const h = location.hash.slice(1);
  const app = document.getElementById("app");
  const c = h.startsWith("c-") && DATA.components.find(x => x.slug === h.slice(2));
  app.innerHTML = c ? detail(c) : overview(filter);
  app.querySelectorAll("[data-f]").forEach(b => b.addEventListener("click", () => { filter = b.dataset.f; render(); }));
  if (c) window.scrollTo(0, 0);
}
window.addEventListener("hashchange", render);
render();
</script>
</body>
</html>
"""

if __name__ == "__main__":
    main()
