#!/usr/bin/env python3
# Copyright 2026 Anud Inc.
# SPDX-License-Identifier: Apache-2.0

"""Retest every open QA issue against the current integration build.

  retest.py --issues qa/review/issues.json --out qa/out-retest [--sha <commit>]

Each entry in issues.json names the issue, the component, the check and,
where there is one, the script step that shows the defect. The component's
script is run; an issue whose check now passes is reported as fixed. Writes
<out>/retest.json; the workflow turns that into "Verified fixed at <sha>"
comments and the qa:verified label. Issues are closed only once the fix is
merged to main, which the workflow checks separately.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--issues", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--sha", default="")
    a = ap.parse_args()
    issues = [i for i in json.loads(Path(a.issues).read_text()) if i.get("state", "open") == "open"]
    slugs = sorted({i["slug"] for i in issues})
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    subprocess.run([sys.executable, "-I", str(HERE / "driver/qa_driver.py"), "run", *slugs, "--out", str(out)])
    verdicts = []
    for i in issues:
        res_path = out / i["slug"] / "results.json"
        if not res_path.exists():
            verdicts.append({**i, "result": "not run"})
            continue
        res = json.loads(res_path.read_text())
        check = res["checks"].get(i.get("check", ""), {})
        step = i.get("step")
        if i.get("check") == "manual":
            verdicts.append({**i, "result": "needs a person", "sha": a.sha, "note": "layout finding; compare the screenshots"})
            continue
        if step:
            hit = next((s for s in res["steps"] if s["n"] == step), None)
            fixed = bool(hit and hit["pass"])
        else:
            fixed = check.get("status") == "pass"
        verdicts.append({**i, "result": "fixed" if fixed else "still open", "sha": a.sha,
                         "note": check.get("note", "")})
    (out / "retest.json").write_text(json.dumps(verdicts, indent=1))
    for v in verdicts:
        print(f"#{v['number']:>4} {v['result']:10} {v['title']}")


if __name__ == "__main__":
    main()
