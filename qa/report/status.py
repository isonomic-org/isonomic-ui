#!/usr/bin/env python3
"""Write the status table for a QA run, as Markdown.

  status.py --run qa/out [--issues qa/review/issues.json] [--only slug,slug] > status.md

One row per tested story: its verdict, each check's mark, and the open QA
issues filed against it. `--only` keeps the listed slugs, in that order, and
shows a row for a listed slug that has no result yet.
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import yaml

sys.path.insert(0, str(Path(__file__).resolve().parent))
from build_report import CHECKS, verdict  # noqa: E402

MARK = {"pass": "✅", "warn": "⚠️", "fail": "❌"}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", required=True)
    ap.add_argument("--issues", default="")
    ap.add_argument("--only", default="")
    a = ap.parse_args()
    run = Path(a.run)
    issues = json.loads(Path(a.issues).read_text()) if a.issues and Path(a.issues).exists() else []
    results = {p.parent.name: json.loads(p.read_text()) for p in sorted(run.glob("*/results.json"))}
    # A finding reviewed and accepted in review/notes.yaml (`known`) counts as a pass.
    notes = yaml.safe_load((Path(__file__).resolve().parent.parent / "review/notes.yaml").read_text()) or {}
    for slug, res in results.items():
        for n in notes.get(slug, []) or []:
            chk = res["checks"].get(n["check"])
            if n["status"] == "known" and chk and chk.get("status") == "warn":
                res["checks"][n["check"]] = {"status": "pass", "note": "known limit: " + n["note"]}
    slugs = [s for s in a.only.split(",") if s] or list(results)
    out = ["| Story | Verdict | " + " | ".join(CHECKS) + " | Open issues |",
           "|---|---|" + "---|" * len(CHECKS) + "---|"]
    counts: dict[str, int] = {}
    for slug in slugs:
        res = results.get(slug)
        mine = [i for i in issues if i.get("slug") == slug and i.get("state", "open") == "open"]
        v = verdict(res["checks"], mine) if res else "Not tested"
        counts[v] = counts.get(v, 0) + 1
        marks = [MARK.get((res or {}).get("checks", {}).get(c, {}).get("status"), "➖") for c in CHECKS]
        refs = ", ".join(f"#{i['number']}" for i in mine)
        out.append(f"| `{slug}` | {v} | " + " | ".join(marks) + f" | {refs} |")
    out += ["", "✅ pass · ⚠️ needs a look · ❌ fails · ➖ not checked", "",
            ", ".join(f"{n} {v}" for v, n in counts.items()) + "."]
    print("\n".join(out))


if __name__ == "__main__":
    main()
