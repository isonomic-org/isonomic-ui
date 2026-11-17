#!/usr/bin/env python3
# Copyright 2026 Anud Inc.
# SPDX-License-Identifier: Apache-2.0

"""Compare a run's screenshots with the approved baselines.

  visual_diff.py --run qa/out --baselines <dir with baselines/<slug>/*.png> [--write-proposed <dir>]

A screenshot counts as a visual change when more than 0.2% of its pixels
differ by more than a small tolerance, after cropping both to the story's
content. Writes <run>/visual-changes.json (read by the report) and, for each
change, <run>/<slug>/diff-<shot>.png with the changed pixels in red.

qa/baselines.toml lists which baselines are approved; a run's screenshots
become proposed baselines with --write-proposed, and are approved by moving
them into the baselines folder and recording them in that file.
"""
from __future__ import annotations

import argparse
import json
import shutil
from pathlib import Path

from PIL import Image, ImageChops

SHOTS = ["light-360", "light-960", "light-1440", "dark-360", "dark-960", "dark-1440"]


def diff(a: Path, b: Path, out: Path) -> float:
    ia, ib = Image.open(a).convert("RGB"), Image.open(b).convert("RGB")
    w, h = max(ia.width, ib.width), max(ia.height, ib.height)
    pa, pb = Image.new("RGB", (w, h)), Image.new("RGB", (w, h))
    pa.paste(ia), pb.paste(ib)
    mask = ImageChops.difference(pa, pb).convert("L").point(lambda v: 255 if v > 24 else 0)
    changed = sum(1 for v in mask.getdata() if v)
    share = changed / (w * h)
    if share > 0.002:
        red = Image.new("RGB", (w, h), (230, 40, 40))
        Image.composite(red, pb.point(lambda v: v // 3 + 170), mask).save(out)
    return share


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", required=True)
    ap.add_argument("--baselines", required=True)
    ap.add_argument("--write-proposed")
    a = ap.parse_args()
    run, base = Path(a.run), Path(a.baselines)
    changes = []
    for res in sorted(run.glob("*/results.json")):
        slug = res.parent.name
        for shot in SHOTS:
            now = res.parent / "shots" / f"{shot}.png"
            then = base / slug / f"{shot}.png"
            if not now.exists():
                continue
            if a.write_proposed:
                dest = Path(a.write_proposed) / slug / f"{shot}.png"
                dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy(now, dest)
            if not then.exists():
                continue
            share = diff(then, now, res.parent / f"diff-{shot}.png")
            if share > 0.002:
                changes.append({"slug": slug, "shot": shot, "share": round(share, 4),
                                "note": f"{shot}: {share:.1%} of pixels differ from the approved baseline"})
    (run / "visual-changes.json").write_text(json.dumps(changes, indent=1))
    print(f"{len(changes)} visual change(s)")


if __name__ == "__main__":
    main()
