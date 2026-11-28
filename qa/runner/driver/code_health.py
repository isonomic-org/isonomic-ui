#!/usr/bin/env python3
# Copyright 2026 Anud Inc.
# SPDX-License-Identifier: GPL-3.0-or-later

"""C11 code health for every component in the integration build.

  code_health.py [--out qa/out]

Runs, once for the whole crate, and attributes results to each component file:
  * cargo clippy --all-targets -- -D warnings  (diagnostics by file)
  * cargo test --lib                           (unit tests by module)
  * rustfmt --edition 2024 --check             (per component and story file)
Writes qa/out/code-health.json and merges a C11 entry into each results.json.
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
QA = HERE.parent.parent
REPO = QA.parent
SRC = REPO / "crates/extra/src"
STORIES = REPO / "crates/gallery/src/stories/extra"


def clippy() -> dict[str, list[dict]]:
    r = subprocess.run(["cargo", "clippy", "-p", "isonomic-ui-extra", "--all-targets", "--message-format=json",
                        "--", "-D", "warnings"], cwd=REPO, capture_output=True, text=True)
    by_file: dict[str, list[dict]] = {}
    for line in r.stdout.splitlines():
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue
        if msg.get("reason") != "compiler-message":
            continue
        m = msg["message"]
        if m.get("level") not in ("warning", "error") or not m.get("spans"):
            continue
        span = next((s for s in m["spans"] if s.get("is_primary")), m["spans"][0])
        name = Path(span["file_name"]).name
        by_file.setdefault(name, []).append({"level": m["level"], "code": (m.get("code") or {}).get("code"),
                                             "message": m["message"], "line": span["line_start"]})
    return by_file


def tests() -> dict[str, dict]:
    r = subprocess.run(["cargo", "test", "-p", "isonomic-ui-extra", "--lib", "--", "--test-threads=2"], cwd=REPO,
                       capture_output=True, text=True)
    by_mod: dict[str, dict] = {}
    for line in (r.stdout + r.stderr).splitlines():
        m = re.match(r"test (\w+)::tests::(\w+) \.\.\. (ok|FAILED|ignored)", line)
        if m:
            d = by_mod.setdefault(m.group(1), {"ok": 0, "failed": [], "ignored": 0})
            if m.group(3) == "ok":
                d["ok"] += 1
            elif m.group(3) == "FAILED":
                d["failed"].append(m.group(2))
            else:
                d["ignored"] += 1
    if r.returncode != 0 and not by_mod:
        by_mod["_build"] = {"ok": 0, "failed": ["test build failed"], "ignored": 0,
                            "log": (r.stderr or "")[-2000:]}
    return by_mod


def fmt(path: Path) -> bool:
    if not path.exists():
        return True
    return subprocess.run(["rustfmt", "--edition", "2024", "--check", str(path)], capture_output=True).returncode == 0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=str(QA / "out"))
    args = ap.parse_args()
    out = Path(args.out)
    lints = clippy()
    unit = tests()
    report = {}
    for comp in sorted(SRC.glob("*.rs")):
        if comp.stem in ("lib", "common"):
            continue
        slug = comp.stem.replace("_", "-")
        story = STORIES / comp.name
        findings = lints.get(comp.name, []) + [dict(d, file="story") for d in lints.get(story.name, [])
                                               if story.exists()]
        t = unit.get(comp.stem, {"ok": 0, "failed": [], "ignored": 0})
        fmt_ok = fmt(comp) and fmt(story)
        problems = []
        if findings:
            problems.append(f"{len(findings)} clippy finding(s) with -D warnings")
        if t["failed"]:
            problems.append("failing tests: " + ", ".join(t["failed"]))
        if t["ok"] == 0:
            problems.append("no unit tests")
        if not fmt_ok:
            problems.append("rustfmt --edition 2024 --check reports changes")
        status = "fail" if (findings or t["failed"] or not fmt_ok) else ("warn" if t["ok"] == 0 else "pass")
        entry = {"status": status,
                 "note": "; ".join(problems) or f"clippy clean, {t['ok']} unit tests pass, formatted",
                 "clippy": findings, "tests": t, "fmt": fmt_ok}
        report[slug] = entry
        res_path = out / slug / "results.json"
        if res_path.exists():
            res = json.loads(res_path.read_text())
            res["checks"]["C11"] = entry
            res_path.write_text(json.dumps(res, indent=1))
    shared = {k: v for k, v in lints.items() if k in ("lib.rs", "common.rs")}
    (out / "code-health.json").write_text(json.dumps({"components": report, "shared": shared,
                                                      "test_build": unit.get("_build")}, indent=1))
    for slug, e in report.items():
        print(f"{slug:28} {e['status']:5} {e['note']}")


if __name__ == "__main__":
    main()
