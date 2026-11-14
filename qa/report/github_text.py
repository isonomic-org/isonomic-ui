#!/usr/bin/env python3
"""Write the QA comment for each PR and the "QA: component status" table.

  github_text.py --run qa/out --meta <dir> --dest <dir> [--label "Q1 full pass"]

Reads the run's results.json files, issues.json, tested.json and queue.json.
Writes <dest>/pr-<n>.md for every tested PR and <dest>/status.md.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from build_report import CHECK_NAMES, CHECKS, verdict  # noqa: E402

REPORT = "https://claude.ai/artifact/Kv5Krhjvw2kjiikPAQeaAj"
MARK = {"pass": "✅", "warn": "⚠️", "fail": "❌", None: "➖", "info": "➖"}
MARKER = "<!-- isonomic-qa-comment -->"



def tidy(note: str) -> str:
    """Readable element ids and plurals for a check note."""
    note = re.sub(r'NamedChild\((?:Name|NamedInteger)\("([^"]+)"(?:, (\d+))?\), "([^"]+)"\)',
                  lambda m: f"`{m.group(1)}{'#' + m.group(2) if m.group(2) else ''}/{m.group(3)}`", note)
    note = re.sub(r'NamedInteger\("([^"]+)", (\d+)\)', lambda m: f"`{m.group(1)}#{m.group(2)}`", note)
    note = re.sub(r'Name\("([^"]+)"\)', lambda m: f"`{m.group(1)}`", note)
    note = re.sub(r"\b1 clickable element\(s\)", "1 clickable element", note)
    note = note.replace("element(s)", "elements").replace("step(s)", "steps")
    note = note.replace("Tab reaches all 1 stops in order, focus is visible, Shift+Tab goes back",
                        "Tab reaches the one stop, focus is visible")
    return note if len(note) <= 320 else note[:317].rsplit(" ", 1)[0] + " …"

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", required=True)
    ap.add_argument("--meta", required=True)
    ap.add_argument("--dest", required=True)
    ap.add_argument("--label", default="")
    a = ap.parse_args()
    run, meta, dest = Path(a.run), Path(a.meta), Path(a.dest)
    dest.mkdir(parents=True, exist_ok=True)
    tested = json.loads((meta / "tested.json").read_text())
    queue = json.loads((meta / "queue.json").read_text())
    issues = json.loads((meta / "issues.json").read_text())
    date = time.strftime("%Y-%m-%d", time.gmtime())
    import yaml
    moved = json.loads((meta / "moved.json").read_text()) if (meta / "moved.json").exists() else {}
    notes = yaml.safe_load((Path(__file__).resolve().parent.parent / "review/notes.yaml").read_text()) or {}

    rows = {}
    for res_path in sorted(run.glob("*/results.json")):
        res = json.loads(res_path.read_text())
        slug = res["slug"]
        for n in notes.get(slug, []) or []:
            if n["status"] == "info":
                prev = res["checks"].get(n["check"], {})
                res["checks"][n["check"]] = {**prev, "status": "pass" if prev.get("status") in (None, "info", "pass") else prev["status"],
                                             "reviewed": n["note"]}
            if n["status"] == "known" and res["checks"].get(n["check"], {}).get("status") == "warn":
                res["checks"][n["check"]] = {"status": "pass", "note": "known limit: " + n["note"]}
            if n["status"] == "warn":
                prev = res["checks"].get(n["check"], {})
                generic = prev.get("note", "").startswith("content reaches past the window edge")
                text = (n["note"] if prev.get("status") in (None, "pass", "info") or generic
                        else prev.get("note", "") + "; " + n["note"])
                res["checks"][n["check"]] = {**prev, "status": "fail" if prev.get("status") == "fail" else "warn",
                                             "note": text}
        branch = "permission-prompt-v2" if slug == "permission-prompt" else slug
        t = tested.get(branch, {})
        mine = [i for i in issues if i["slug"] == slug and i.get("state", "open") == "open"]
        fixed = [i for i in issues if i["slug"] == slug and i.get("state") == "verified"]
        v = verdict(res["checks"], mine)
        lines = [MARKER, f"### QA: {v}", "",
                 f"Tested `extra/{branch}` at `{t.get('sha', '')[:10]}` in run {a.label} ({date}), "
                 "Linux under a virtual display, light and dark at 360, 960 and 1440 px.", ""]
        mv = moved.get(branch) or moved.get(slug)
        if mv:
            lines += [f"⏳ **Retest pending.** New commits up to `{mv['now'][:10]}`: {mv['note']}. "
                      "The results below are for the tested SHA above. This QA sandbox cannot build the foundation "
                      "yet; the retest runs as soon as it can, and issue verdicts change only after that retest.", ""]
        lines.append(" · ".join(f"{MARK.get(res['checks'].get(c, {}).get('status'), '➖')} {c}" for c in CHECKS))
        lines.append("")
        for c in CHECKS:
            chk = res["checks"].get(c, {})
            if chk.get("status") in ("warn", "fail") or chk.get("note", "").startswith("known limit"):
                lines.append(f"- **{c} {CHECK_NAMES[c]}**: {tidy(chk.get('note', ''))}")
            elif chk.get("reviewed"):
                lines.append(f"- **{c} {CHECK_NAMES[c]}** (reviewed by eye): {chk['reviewed']}")
        lines.append("")
        lines.append("✅ pass · ⚠️ needs a look · ❌ fails · ➖ not checked yet (C5 and C7 are partly manual)")
        lines.append("")
        if mine:
            lines.append("**Open QA issues**")
            for i in sorted(mine, key=lambda i: (i["sev"], i["number"])):
                lines.append(f"- #{i['number']} (Sev{i['sev']}) {i['title']}")
            lines.append("")
        if fixed:
            lines.append("**Verified fixed on this branch, open until merged to main**")
            for i in fixed:
                lines.append(f"- #{i['number']} (Sev{i['sev']}) {i['title']}, verified at `{i.get('verified_at', '')}`")
            lines.append("")
        lines.append(f"Screenshots, contact sheet and every step: [QA report → {slug}]({REPORT}#c-{slug}) "
                     "(images will be attached here by the QA workflow).")
        lines.append("")
        lines.append("_This comment is updated in place on each QA run._")
        if t.get("pr"):
            (dest / f"pr-{t['pr']}.md").write_text("\n".join(lines))
        counts = {s: sum(1 for i in mine if i["sev"] == s) for s in (1, 2, 3, 4)}
        rows[branch] = (t.get("pr"), v, counts, t.get("sha", "")[:7], date)
        rows[slug] = rows[branch]

    sev1 = sorted((i for i in issues if i["sev"] == 1 and i.get("state", "open") in ("open", "verified")),
                  key=lambda i: i["number"])
    out = ["## Sev1", ""]
    if sev1:
        for i in sev1:
            where = ("verified fixed on its branch at `" + i.get("verified_at", "") + "`, open until merged to main"
                     if i.get("state") == "verified" else "open")
            br = "permission-prompt-v2" if i["slug"] == "permission-prompt" else i["slug"]
            if br in moved:
                where += f"; the component has since moved onto the foundation at `{moved[br]['now'][:7]}`, where it is re-verified before closing"
            out.append(f"- #{i['number']} {i['title']} ({where})")
    else:
        out.append("No Sev1 issues are open.")
    pending = json.loads((meta / "built.json").read_text()).get("not_tested_prs", {})
    tested_rows = {q["slug"]: rows[q["slug"]] for q in queue if q["slug"] in rows}
    verdicts = [r[1] for r in tested_rows.values()]
    out += ["", f"**{len(tested_rows) + len(pending)} of {len(queue)} built, {len(tested_rows)} tested:** "
            f"{verdicts.count('Pass')} Pass, {verdicts.count('Pass with polish')} Pass with polish, "
            f"{verdicts.count('Needs fixes')} Needs fixes, {verdicts.count('Blocked')} Blocked; "
            f"{len(pending)} built and not tested yet."]
    if moved:
        out += ["", "## Moved onto the foundation, retest pending", ""]
        for br, mv in moved.items():
            claims = ", ".join(f"#{n}" for n in mv.get("claims", []))
            out.append(f"- #{mv['pr']} `extra/{br}` at `{mv['now'][:7]}` (tested at `{mv['tested'][:7]}`): {mv['note']}"
                       + (f"; fixes to retest: {claims}" if claims else "") + ".")
    out += ["", "## Components", "",
            "Tracking issue for QA of the components in #1, in the queue's order. "
            f"Updated by the QA session after each run. Full report: {REPORT}", "",
           "| # | Component | Tier | PR | Verdict | Sev1 | Sev2 | Sev3 | Sev4 | Tested SHA | Date |",
           "|---|---|---|---|---|---|---|---|---|---|---|"]
    for n, q in enumerate(queue, 1):
        r = rows.get(q["slug"])
        if r:
            pr, v, c, sha, d = r
            out.append(f"| {n} | {q['name']} | {q['tier']} | #{pr} | {v} | {c[1] or ''} | {c[2] or ''} | "
                       f"{c[3] or ''} | {c[4] or ''} | `{sha}` | {d} |")
        elif q["name"] in pending:
            out.append(f"| {n} | {q['name']} | {q['tier']} | #{pending[q['name']]} | Built, not tested yet |  |  |  |  |  |  |")
        else:
            out.append(f"| {n} | {q['name']} | {q['tier']} |  | Not built |  |  |  |  |  |  |")
    (dest / "status.md").write_text("\n".join(out))
    print(f"wrote {len(list(dest.glob('pr-*.md')))} PR comments and status.md")


if __name__ == "__main__":
    main()
