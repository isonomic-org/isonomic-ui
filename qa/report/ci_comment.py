#!/usr/bin/env python3
"""Write the QA comment for one pull request from a CI run.

  ci_comment.py --run qa/out --pr 28 --sha <head sha> --evidence-url <base url> > comment.md

The comment starts with a hidden marker so the workflow can find and edit
the same comment on every run instead of adding a new one.
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from build_report import CHECK_NAMES, CHECKS, verdict  # noqa: E402

MARK = {"pass": "✅", "warn": "⚠️", "fail": "❌"}
MARKER = "<!-- isonomic-qa-comment -->"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", required=True)
    ap.add_argument("--pr", required=True)
    ap.add_argument("--sha", required=True)
    ap.add_argument("--evidence-url", default="")
    ap.add_argument("--issues", default="", help="JSON list of open qa issues: number, slug, sev, title")
    a = ap.parse_args()
    run = Path(a.run)
    issues = json.loads(Path(a.issues).read_text()) if a.issues and Path(a.issues).exists() else []
    out = [MARKER, f"## QA for `{a.sha[:10]}`", ""]
    results = sorted(run.glob("*/results.json"))
    if not results:
        out.append("No component story changed in this pull request, so there was nothing to render.")
    for res_path in results:
        res = json.loads(res_path.read_text())
        slug = res["slug"]
        mine = [i for i in issues if i.get("slug") == slug]
        v = verdict(res["checks"], mine)
        out.append(f"### {slug}: {v}")
        out.append("")
        for c in CHECKS:
            chk = res["checks"].get(c, {})
            st = chk.get("status")
            note = chk.get("note", "") if st in ("warn", "fail") else ""
            out.append(f"- {MARK.get(st, '➖')} **{c} {CHECK_NAMES[c]}**" + (f": {note[:220]}" if note else ""))
        if mine:
            out.append("")
            out += [f"- #{i['number']} (Sev{i['sev']}) {i['title']}" for i in mine]
        if a.evidence_url:
            out += ["", f"![{slug} contact sheet]({a.evidence_url}/{slug}/contact.png?raw=true)"]
        out.append("")
    out.append("_Updated in place on every push. Screenshots, logs and results are in this run's artifacts._")
    print("\n".join(out))


if __name__ == "__main__":
    main()
