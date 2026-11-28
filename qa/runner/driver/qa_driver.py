#!/usr/bin/env python3
# Copyright 2026 Anud Inc.
# SPDX-License-Identifier: GPL-3.0-or-later

"""QA driver for isonomic-ui component stories.

Runs each story in the QA gallery under a virtual X display, takes the
screenshots, reads the accessibility tree, plays the story's script and
writes one results JSON per component. `cargo run -p qa-runner -- ...` calls
this file; it can also be run directly with the system Python.

A slug names a story. A component of `crates/extra` has its file name as its
slug (`model-picker`). Every other gallery story has `base-` and its name
(`base-button`).

  qa_driver.py run <slug>... | --all  [--step N] [--out DIR] [--themes ..] [--widths ..]
  qa_driver.py list

How it sees and drives the app (no extra automation code inside the app):
  * screenshots: X window capture of the gallery window (software rendering)
  * input: real X pointer and keyboard events
  * structure: the app's own accessibility tree, dumped as JSON on request,
    and the same tree over the desktop accessibility bus for on-screen bounds
    and actions
"""
from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

import yaml
from PIL import Image, ImageChops, ImageStat

HERE = Path(__file__).resolve().parent
QA = HERE.parent.parent  # qa/
REPO = QA.parent
STORIES_DIR = Path(os.environ.get("QA_STORIES_DIR", REPO / "crates/gallery/src/stories/extra"))
SCREENS_DIR = STORIES_DIR.parent / "screens"
COMPONENTS_DIR = REPO / "crates/extra/src"
BASE_DIR = REPO / "crates/component/src"
SCRIPTS_DIR = QA / "scripts"
GALLERY = Path(os.environ.get("QA_GALLERY", QA / "target/debug/qa-gallery"))
PROBE = QA / "runner/probe/atspi_probe.py"
# The accessibility bindings are built for the system Python.
PY = os.environ.get("QA_PYTHON") or next(
    (p for p in ("/usr/bin/python3.12", "/usr/bin/python3") if Path(p).exists()), sys.executable)
DISPLAY = os.environ.get("QA_DISPLAY", ":99")
SCREEN = (1600, 2600)
THEMES = ["light", "dark"]
WIDTHS = [360, 960, 1440]
HEIGHT = 2400

# Log lines that come from the virtual display, not from the component.
ENV_NOISE = [
    r"Found no xinput mouse pointers",
    r"DRI3",
    r"libEGL",
    r"system locale",
    r"SystemUIFont",
    r"zbus",
    r"portal",
    r"MESA",
    r"lavapipe",
    r"not a conformant vulkan implementation",
]


def log(*a):
    print(*a, file=sys.stderr, flush=True)


# ---------------------------------------------------------------- environment


def ensure_display():
    os.environ["DISPLAY"] = DISPLAY
    ok = subprocess.run(["xdpyinfo"], capture_output=True).returncode == 0 if shutil.which("xdpyinfo") else \
        subprocess.run(["xdotool", "getdisplaygeometry"], capture_output=True).returncode == 0
    if not ok:
        log(f"starting virtual display {DISPLAY}")
        subprocess.Popen(["Xvfb", DISPLAY, "-screen", "0", f"{SCREEN[0]}x{SCREEN[1]}x24", "-nolisten", "tcp"],
                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(50):
            time.sleep(0.1)
            if subprocess.run(["xdotool", "getdisplaygeometry"], capture_output=True).returncode == 0:
                break
    # A window manager gives the gallery window an active state, which keyboard focus needs.
    if shutil.which("openbox") and subprocess.run(["pgrep", "-x", "openbox"], capture_output=True).returncode != 0:
        subprocess.Popen(["openbox"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(1)
    geo = subprocess.run(["xdotool", "getdisplaygeometry"], capture_output=True, text=True).stdout.split()
    if geo and (int(geo[0]) < SCREEN[0] or int(geo[1]) < SCREEN[1]):
        log(f"warning: display is {geo[0]}x{geo[1]}; tall stories are cut at the screen edge")


def ensure_a11y():
    """The app only builds its accessibility tree once an assistive client asks."""
    if not os.environ.get("DBUS_SESSION_BUS_ADDRESS"):
        # Re-run inside a private session bus.
        os.execvp("dbus-run-session", ["dbus-run-session", "--", PY, *sys.argv])
    if os.environ.get("QA_A11Y_READY"):
        return
    subprocess.Popen(["/usr/libexec/at-spi-bus-launcher", "--launch-immediately"],
                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    for _ in range(50):
        r = subprocess.run(["gdbus", "call", "--session", "--dest", "org.a11y.Bus", "--object-path", "/org/a11y/bus",
                            "--method", "org.freedesktop.DBus.Properties.Set", "org.a11y.Status", "IsEnabled",
                            "<true>"], capture_output=True)
        if r.returncode == 0:
            break
        time.sleep(0.1)
    os.environ["QA_A11Y_READY"] = "1"


# ---------------------------------------------------------------- the app


class App:
    """One gallery process showing one story."""

    def __init__(self, slug: str, theme: str, width: int, out: Path):
        self.slug, self.theme, self.width, self.out = slug, theme, width, out
        self.stderr_path = out / "gallery.log"
        self.err = open(self.stderr_path, "ab")
        env = dict(os.environ, RUST_LOG=os.environ.get("RUST_LOG", "warn"), RUST_BACKTRACE="1")
        self.proc = subprocess.Popen([str(GALLERY), "--story", story_name(slug), "--theme", theme, "--width", str(width),
                                      "--height", str(HEIGHT)], stdin=subprocess.PIPE, stdout=subprocess.DEVNULL,
                                     stderr=self.err, env=env)
        self.wid = None
        deadline = time.time() + 60
        while time.time() < deadline and self.proc.poll() is None:
            r = subprocess.run(["xdotool", "search", "--onlyvisible", "--pid", str(self.proc.pid)],
                               capture_output=True, text=True)
            ids = r.stdout.split()
            if ids:
                self.wid = ids[-1]
                break
            time.sleep(0.2)
        self.settled_at_load = None
        if self.wid:
            subprocess.run(["xdotool", "windowmove", self.wid, "0", "0"], capture_output=True)
            subprocess.run(["xdotool", "windowactivate", "--sync", self.wid], capture_output=True, timeout=10)
            # Park the pointer in the window's bottom corner, away from the story.
            subprocess.run(["xdotool", "mousemove", "--window", self.wid, str(width - 4), str(HEIGHT - 4)],
                           capture_output=True)
            t0 = time.time()
            ok = self.settle(timeout=6.0)
            self.settled_at_load = round(time.time() - t0, 2) if ok else None

    def alive(self) -> bool:
        return self.proc.poll() is None

    def send(self, line: str):
        if not self.alive():
            return
        try:
            self.proc.stdin.write((line + "\n").encode())
            self.proc.stdin.flush()
        except BrokenPipeError:
            pass

    def ping(self, timeout=10.0) -> bool:
        path = self.out / ".ping"
        path.unlink(missing_ok=True)
        self.send(f"ping {path}")
        deadline = time.time() + timeout
        while time.time() < deadline:
            if path.exists():
                return True
            if not self.alive():
                return False
            time.sleep(0.03)
        return False

    settle_timeout = 4.0

    def settle(self, quiet=0.25, timeout=None):
        """Wait until two screenshots in a row match (animations at rest)."""
        self.ping()
        last = None
        deadline = time.time() + (timeout if timeout is not None else self.settle_timeout)
        while time.time() < deadline:
            img = self.grab()
            if img is not None and last is not None and not ImageChops.difference(img, last).getbbox():
                return True
            last = img
            time.sleep(quiet)
        return False

    def grab(self) -> Image.Image | None:
        if not self.wid or not self.alive():
            return None
        r = subprocess.run(["import", "-window", self.wid, "png:-"], capture_output=True)
        if r.returncode != 0 or not r.stdout:
            return None
        from io import BytesIO
        return Image.open(BytesIO(r.stdout)).convert("RGB")

    def shot(self, name: str) -> Path | None:
        img = self.grab()
        if img is None:
            return None
        path = self.out / "shots" / f"{name}.png"
        path.parent.mkdir(parents=True, exist_ok=True)
        img.save(path)
        return path

    def set_theme(self, theme: str):
        self.theme = theme
        self.send(f"theme {theme}")
        self.settle()

    def set_width(self, width: int):
        self.width = width
        self.send(f"size {width} {HEIGHT}")
        self.settle()

    def tree(self) -> list[dict]:
        """The app's own accessibility tree (ids, roles, labels, focus)."""
        path = self.out / ".a11y.json"
        path.unlink(missing_ok=True)
        self.send(f"a11y {path}")
        for _ in range(100):
            if path.exists() and path.stat().st_size:
                try:
                    data = json.loads(path.read_text())
                    return flatten_tree(data)
                except json.JSONDecodeError:
                    pass
            time.sleep(0.05)
        return []

    def atspi(self) -> list[dict]:
        r = subprocess.run([PY, "-I", str(PROBE), "tree"], capture_output=True, text=True, timeout=30)
        try:
            data = json.loads(r.stdout)
            return data if isinstance(data, list) else []
        except json.JSONDecodeError:
            return []

    # -- input -----------------------------------------------------------
    def click_at(self, x: int, y: int):
        subprocess.run(["xdotool", "mousemove", "--window", self.wid, str(x), str(y), "click", "1"])
        self.settle()

    def hover_at(self, x: int, y: int):
        subprocess.run(["xdotool", "mousemove", "--window", self.wid, str(x), str(y)])
        self.settle()

    def focus_window(self):
        subprocess.run(["xdotool", "windowfocus", "--sync", self.wid], capture_output=True)

    def key(self, *keys: str, settle=True):
        self.focus_window()
        subprocess.run(["xdotool", "key", "--delay", "40", *keys])
        if settle:
            self.settle()

    def type(self, text: str):
        self.focus_window()
        subprocess.run(["xdotool", "type", "--delay", "20", text])
        self.settle()

    def frames(self, n=120) -> list[float]:
        path = self.out / ".frames.json"
        path.unlink(missing_ok=True)
        self.send(f"frames {path} {n}")
        for _ in range(400):
            if path.exists() and path.stat().st_size:
                try:
                    return json.loads(path.read_text())
                except json.JSONDecodeError:
                    pass
            time.sleep(0.05)
        return []

    def close(self):
        self.send("quit")
        try:
            self.proc.wait(5)
        except subprocess.TimeoutExpired:
            self.proc.kill()
            self.proc.wait(5)
        # Make sure the window is really gone before another one opens on top of it.
        for _ in range(50):
            if not self.wid or subprocess.run(["xdotool", "getwindowname", self.wid],
                                              capture_output=True).returncode != 0:
                break
            time.sleep(0.1)
        self.err.close()


def flatten_tree(data) -> list[dict]:
    """Flatten the app's accessibility JSON into a list of nodes in tree order.

    The dump is {"root": key, "gpui_focus": ..., "frame": {...}, "nodes": {key: node}}
    where each node has element_id, aria {role, label, on_action, ...} and children keys.
    """
    if isinstance(data, list):
        data = data[0] if data else {}
    nodes = data.get("nodes", {})
    focus = data.get("gpui_focus")
    out = []

    def walk(key, depth):
        n = nodes.get(key)
        if not n:
            return
        aria = n.get("aria", {})
        out.append({"key": key, "element_id": n.get("element_id", ""), "role": aria.get("role", ""),
                    "label": aria.get("label") or aria.get("value") or "", "on_action": aria.get("on_action", []),
                    "aria": aria, "view": n.get("view", ""), "depth": depth,
                    "focused": focus is not None and (focus == key or focus == n.get("element_id")
                                                      or (isinstance(focus, dict) and focus.get("node") == key))})
        for c in n.get("children", []) or []:
            walk(c, depth + 1)

    walk(data.get("root"), 0)
    if out:
        out[0]["frame"] = data.get("frame", {})
        out[0]["gpui_focus"] = focus
    return out


# ---------------------------------------------------------------- checks


def panic_lines(log_path: Path) -> list[str]:
    try:
        text = log_path.read_text(errors="replace")
    except FileNotFoundError:
        return []
    lines = []
    for line in text.splitlines():
        if any(re.search(p, line) for p in ENV_NOISE):
            continue
        if "panicked at" in line or " ERROR " in line or line.startswith("[ERROR") or "error" in line.lower():
            lines.append(line.strip())
    return lines[:20]


def background_of(img: Image.Image):
    return img.getpixel((img.width - 2, img.height - 2))


def content_box(img: Image.Image):
    bg = Image.new("RGB", img.size, background_of(img))
    diff = ImageChops.difference(img, bg).convert("L").point(lambda v: 255 if v > 12 else 0)
    return diff.getbbox()


def blank_ratio(img: Image.Image) -> float:
    bg = Image.new("RGB", img.size, background_of(img))
    diff = ImageChops.difference(img, bg).convert("L").point(lambda v: 255 if v > 12 else 0)
    return ImageStat.Stat(diff).mean[0] / 255.0


def luminance_bands(img: Image.Image, box):
    """Share of very light and very dark pixels inside the content box."""
    crop = img.crop(box).convert("L") if box else img.convert("L")
    hist = crop.histogram()
    total = max(1, sum(hist))
    return sum(hist[230:]) / total, sum(hist[:30]) / total


COLOR_LITERAL = re.compile(
    r"\b(hsla|rgba|rgb|hsl)\s*\(|gpui::(black|white)\(\)|\b(black|white)\(\)|\b0x[0-9a-fA-F]{6,8}\b")


def color_literals(path: Path) -> list[dict]:
    hits = []
    if not path.exists():
        return hits
    in_tests = False
    for n, line in enumerate(path.read_text().splitlines(), 1):
        if line.startswith("#[cfg(test)]"):
            in_tests = True
        if in_tests or line.strip().startswith("//"):
            continue
        if COLOR_LITERAL.search(line) and "transparent_black" not in line:
            hits.append({"line": n, "text": line.strip()})
    return hits


def overflow_nodes(nodes: list[dict], width: int) -> list[dict]:
    """On-screen elements that reach past the window's right edge or start left of it.

    The window sits at (0, 0), so AT-SPI bounds are window coordinates.
    """
    out = []
    for n in nodes:
        if n.get("role") in ("application", "frame", "window") or n.get("w", 0) <= 0 or n.get("h", 0) <= 0:
            continue
        right = n["x"] + n["w"]
        if right > width + 1 or n["x"] < -1:
            out.append({"role": n.get("role"), "name": n.get("name"), "x": n["x"], "w": n["w"], "right": right})
    return out


def slugify(name: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-")


STORIES: dict[str, str] = {}


def stories() -> dict[str, str]:
    """Every story the gallery lists, as slug -> story name, in gallery order.

    The gallery lists its own stories first, then one story per file in
    `stories/extra`, in file-name order, and last one per file in
    `stories/screens`. Screens are compositions, not components, so they
    are left out here.
    """
    if STORIES:
        return STORIES
    r = subprocess.run([str(GALLERY), "--list"], capture_output=True, text=True, timeout=120)
    names = [line.split("\t", 1)[1] for line in r.stdout.splitlines() if line.startswith("story\t")]
    if not names:
        log("the QA gallery listed no stories:", r.stderr[-2000:])
        return STORIES
    extra = sorted(p.stem for p in STORIES_DIR.glob("*.rs") if p.stem != "mod")
    screens = [p for p in SCREENS_DIR.glob("*.rs") if p.stem != "mod"]
    names = names[: len(names) - len(screens)]
    split = len(names) - len(extra)
    for name in names[:split]:
        STORIES["base-" + slugify(name)] = name
    for stem, name in zip(extra, names[split:]):
        STORIES[stem.replace("_", "-")] = name
    twice = {n for n in names if names.count(n) > 1}
    if twice:
        log("warning: stories that share a name can only show the first:", ", ".join(sorted(twice)))
    return STORIES


def story_slugs() -> list[str]:
    return list(stories())


def story_name(slug: str) -> str:
    return slug if slug.startswith("__") else stories().get(slug, slug)


def component_file(slug: str) -> Path:
    if slug.startswith("base-"):
        return BASE_DIR / f"{slug[5:].replace('-', '_')}.rs"
    return COMPONENTS_DIR / f"{slug.replace('-', '_')}.rs"


def contact_sheet(shots: dict[str, Path], dest: Path):
    """Two rows (light, dark) by three widths, each cropped to the story's content."""
    tiles = []
    for theme in THEMES:
        for width in WIDTHS:
            p = shots.get(f"{theme}-{width}")
            if not p or not p.exists():
                continue
            img = Image.open(p).convert("RGB")
            box = content_box(img)
            if box:
                l, t, r, b = box
                keep_w = img.width if width == 360 else min(img.width, max(360, r + 24))
                img = img.crop((0, 0, keep_w, min(img.height, b + 24)))
            tile = dest.parent / f".tile-{theme}-{width}.png"
            img.save(tile)
            tiles.append((f"{theme} · {width}", tile))
    if not tiles:
        return None
    args = ["montage"]
    for label, tile in tiles:
        args += ["-label", label, str(tile)]
    args += ["-tile", "3x2", "-geometry", "480x900>+12+12", "-pointsize", "14", "-background", "#f4f4f5",
             str(dest)]
    subprocess.run(args, capture_output=True)
    for _, tile in tiles:
        tile.unlink(missing_ok=True)
    return dest if dest.exists() else None


def off_screen(nodes: list[dict], label: str) -> bool:
    """Whether every on-screen element with this name lies outside the test window."""
    if not label:
        return False
    boxes = [n for n in nodes if n.get("name") == label and n.get("h", 0) > 0]
    return bool(boxes) and all(n["y"] >= HEIGHT or n["y"] + n["h"] <= 0 for n in boxes)


def keyboard_walk(app: App) -> dict:
    """C6: walk the story with Tab and Shift+Tab from a fresh start."""
    tree = app.tree()
    stops = (tree[0].get("frame", {}) if tree else {}).get("tab_stop_count", 0) or 0
    actionable = [n["element_id"] for n in tree if set(n.get("on_action") or []) & {"Click", "Focus"}]
    seq, ring, names, labels = [], [], {}, {}
    prev = app.grab()
    for i in range(stops + 2):
        if i == 0:
            # Nothing has focus in a fresh window; start where an app's initial focus would.
            app.send("tabstart")
            app.settle()
        else:
            app.key("Tab")
        f = focused(app)
        cur = app.grab()
        # Identify stops by their node key: two copies of a component reuse the same local element ids.
        seq.append(f["key"] if f else None)
        if f:
            names[f["key"]] = f["element_id"]
            labels[f["key"]] = f.get("label") or ""
        ring.append(prev is not None and cur is not None and ImageChops.difference(prev, cur).getbbox() is not None)
        prev = cur
    back = None
    if len(seq) >= 2 and seq[-1] is not None:
        app.key("shift+Tab")
        f = focused(app)
        back = f["key"] if f else None
    on_screen = app.atspi()
    app.key("Escape")
    reached = [x for x in dict.fromkeys(seq) if x]
    # The first stop is given focus without a key press, which does not count as
    # keyboard focus, so an element is only flagged if it never shows a change
    # on any press that lands on it.
    arrivals = [i for i in range(len(seq)) if seq[i] and (i == 0 or seq[i] != seq[i - 1])]
    shown = {seq[i] for i in arrivals if ring[i] and i > 0}
    no_ring = list(dict.fromkeys(seq[i] for i in arrivals if not ring[i] and seq[i] not in shown))
    # A control below the bottom of the test window cannot show anything, so it is not judged.
    below = [k for k in no_ring if off_screen(on_screen, labels.get(k, ""))]
    no_ring = [k for k in no_ring if k not in below]
    wraps = len(seq) > 1 and seq[0] is not None and seq[0] in seq[1:]
    nm = lambda k: names.get(k, k) if k else None
    res = {"tab_stops": stops, "sequence": [nm(k) for k in seq], "reached": [nm(k) for k in reached],
           "no_visible_change": [nm(k) for k in no_ring], "shift_tab_back": nm(back), "wraps": wraps}
    # One control can hold several Tab stops (the parts of a date, the cells of
    # a grid), so a repeated element is not a trap in itself. Focus is stuck
    # when a press moves nothing on screen, or when one element holds it for
    # more presses than any such control has parts.
    stuck = None
    run = 1
    for i in range(1, len(seq)):
        run = run + 1 if seq[i] is not None and seq[i] == seq[i - 1] else 1
        if stops > 1 and seq[i] is not None and seq[i] == seq[i - 1] and (not ring[i] or run > 6):
            stuck = seq[i]
            break
    problems = []
    if stops and not reached:
        problems.append(f"Tab reaches none of the {stops} tab stops")
    elif stuck:
        problems.append(f"Tab stops moving at {nm(stuck)} (possible focus trap)")
    silent = sum(1 for k in seq[:stops] if k is None)
    if silent and reached:
        # Focus moved somewhere with no role: a screen reader announces nothing there.
        problems.append(f"{silent} Tab stop(s) are not in the accessibility tree")
    if no_ring:
        problems.append("no visible focus change on " + ", ".join(nm(k) for k in no_ring[:4]))
    if stuck and len(seq) >= 2 and seq[-1] and back == seq[-1]:
        problems.append("Shift+Tab does not move focus back")
    res["below_window"] = [nm(k) for k in below]
    res["status"] = "warn" if problems else ("pass" if stops else "info")
    res["note"] = "; ".join(problems) or (("Tab reaches the one stop, focus is visible" if stops == 1 else
                                           f"Tab moves through all {stops} stops, focus is visible, Shift+Tab goes back")
                                          if stops else "no tab stops in this story")
    return res


def content_name(tree: list[dict], node: dict) -> str:
    """The text inside a control, which a screen reader reads as its name when it has no label.

    A list row or a tab with a text label inside it is named by that text.
    """
    start = next((i for i, n in enumerate(tree) if n is node), None)
    if start is None:
        return ""
    words = []
    for n in tree[start + 1:]:
        if n["depth"] <= node["depth"]:
            break
        if n.get("role") in ("Label", "StaticText", "Paragraph", "Heading") and n.get("label"):
            words.append(n["label"])
    return " ".join(words)


def a11y_check(tree: list[dict]) -> dict:
    """C9: every control has a role and a name."""
    clickable = [n for n in tree if "Click" in (n.get("on_action") or [])]
    unnamed = [n["element_id"] for n in clickable if not n.get("label") and not content_name(tree, n)]
    roleless = [n["element_id"] for n in clickable if n.get("role") in ("", "GenericContainer", "Group", "Unknown")]
    problems = []
    if unnamed:
        problems.append(f"{len(unnamed)} clickable element(s) without a name: " + ", ".join(unnamed[:5]))
    if roleless:
        problems.append(f"{len(roleless)} clickable element(s) without a control role: " + ", ".join(roleless[:5]))
    return {"status": "warn" if problems else "pass",
            "note": "; ".join(problems) or f"{len(clickable)} controls, all with a role and a name",
            "unnamed": unnamed, "roleless": roleless}


BASELINE: dict = {}


def frame_times(app: App) -> list[float]:
    app.send(f"size {app.width} 900")
    app.settle()
    times = app.frames(120)[2:]
    app.send(f"size {app.width} {HEIGHT}")
    app.settle()
    return times


def frame_baseline(out: Path) -> dict:
    """Redraw time of an empty window, so the component's own cost can be told apart."""
    if BASELINE:
        return BASELINE
    app = App("__empty__", "light", 960, out)
    times = frame_times(app)
    app.close()
    if times:
        BASELINE.update(avg=sum(times) / len(times), worst=max(times))
    return BASELINE


def frame_check(app: App, base: dict) -> dict:
    """C10: time 120 back-to-back redraws of the story at rest, 960 x 900, against an empty window."""
    times = frame_times(app)
    if not times or not base:
        return {"status": None, "note": "frame timing not available"}
    worst, avg = max(times), sum(times) / len(times)
    extra = avg - base["avg"]
    status = "warn" if (extra > 20 or worst > base["worst"] * 3 + 16) else "pass"
    return {"status": status, "worst_ms": round(worst, 1), "avg_ms": round(avg, 1), "extra_ms": round(extra, 1),
            "note": f"redraw {avg:.1f} ms on average ({extra:+.1f} ms over an empty window), worst {worst:.1f} ms; "
                    "dev build under software rendering, so compare against the empty window, not 16 ms"}


# ---------------------------------------------------------------- scripts


def load_script(slug: str) -> dict | None:
    p = SCRIPTS_DIR / f"{slug}.yaml"
    if not p.exists():
        return None
    return yaml.safe_load(p.read_text())


def find_node(app: App, sel: dict) -> dict | None:
    """Find an on-screen node by name, role and index over AT-SPI."""
    nodes = app.atspi()
    name = sel.get("name")
    role = sel.get("role")
    contains = sel.get("contains")
    if name is None and role is None and contains is None:
        return None
    matches = [n for n in nodes
               if (name is None or n.get("name") == name)
               and (contains is None or contains.lower() in (n.get("name", "") + " " + n.get("description", "")).lower())
               and (role is None or n.get("role") == role)
               and n.get("w", 0) > 0]
    ix = sel.get("index", 0)
    return matches[ix] if len(matches) > ix else None


def node_center(app: App, n: dict):
    return n["x"] + n["w"] // 2, n["y"] + n["h"] // 2


def focused(app: App) -> dict | None:
    for n in app.tree():
        if n.get("focused"):
            return n
    return None


def run_step(app: App, step: dict, ctx: dict) -> tuple[bool, str]:
    """Play one step. Returns (passed, note)."""
    kind = next(iter(k for k in step if k not in ("name", "expect", "check", "shot", "note")), None)
    arg = step.get(kind) if kind else None
    before = app.grab()
    note = ""
    if kind == "click":
        n = None if isinstance(arg, dict) and "at" in arg else find_node(
            app, arg if isinstance(arg, dict) else {"name": arg})
        if isinstance(arg, dict) and "at" in arg:
            app.click_at(*arg["at"])
        elif n is None:
            return False, f"nothing on screen matches {arg}"
        else:
            app.click_at(*node_center(app, n))
    elif kind == "hover":
        n = None if isinstance(arg, dict) and "at" in arg else find_node(
            app, arg if isinstance(arg, dict) else {"name": arg})
        if isinstance(arg, dict) and "at" in arg:
            app.hover_at(*arg["at"])
        elif n is None:
            return False, f"nothing on screen matches {arg}"
        else:
            app.hover_at(*node_center(app, n))
    elif kind == "tabstart":
        app.send("tabstart")
        app.settle()
    elif kind == "tab":
        for _ in range(int(arg)):
            app.key("Tab", settle=False)
        app.settle()
    elif kind == "key":
        app.key(*([arg] if isinstance(arg, str) else arg))
    elif kind == "type":
        app.type(str(arg))
    elif kind == "wait":
        time.sleep(float(arg))
        app.settle()
    elif kind == "theme":
        app.set_theme(arg)
    elif kind == "width":
        app.set_width(int(arg))
    elif kind is None:
        pass
    else:
        return False, f"unknown step kind {kind}"

    if step.get("shot"):
        ctx["shots"][step["shot"]] = app.shot(f"{ctx['prefix']}{step['shot']}")
    if not app.alive():
        return False, "the app exited (see gallery.log)"
    after = app.grab()
    checks = step.get("expect") or []
    if isinstance(checks, dict):
        checks = [checks]
    for c in checks:
        ok, why = check(app, c, before, after)
        if not ok:
            return False, why
        note = why or note
    return True, note


def check(app: App, c: dict, before, after) -> tuple[bool, str]:
    if "changed" in c:
        changed = before is not None and after is not None and ImageChops.difference(before, after).getbbox() is not None
        return (changed == bool(c["changed"])), ("" if changed == bool(c["changed"]) else
                                                 ("nothing on screen changed" if c["changed"] else "the screen changed"))
    if "present" in c:
        sel = c["present"] if isinstance(c["present"], dict) else {"contains": c["present"]}
        n = find_node(app, sel)
        if n is None and "contains" in sel:
            # Text that is drawn but not in the accessibility tree: read it off the screen.
            text = ocr(after)
            if sel["contains"].lower() in text.lower():
                return True, "found on screen by text recognition"
        return (n is not None), ("" if n is not None else f"expected {sel} on screen; not found")
    if "absent" in c:
        sel = c["absent"] if isinstance(c["absent"], dict) else {"contains": c["absent"]}
        n = find_node(app, sel)
        if n is None and "contains" in sel and sel["contains"].lower() in ocr(after).lower():
            n = {"ocr": True}
        return (n is None), ("" if n is None else f"expected {sel} gone; still there")
    if "focused" in c:
        f = focused(app)
        want = c["focused"]
        got = json.dumps(f) if f else ""
        ok = f is not None and (want is True or str(want).lower() in str(f.get("element_id", "")).lower())
        return ok, ("" if ok else f"expected focus on {want}; focus is on {f.get('element_id') if f else 'nothing'}")
    if "text" in c:
        text = ocr(after)
        ok = c["text"].lower() in text.lower()
        return ok, ("" if ok else f"expected the text {c['text']!r} on screen")
    if "no_text" in c:
        text = ocr(after)
        ok = c["no_text"].lower() not in text.lower()
        return ok, ("" if ok else f"did not expect the text {c['no_text']!r} on screen")
    if "alive" in c:
        return app.alive(), ""
    return False, f"unknown check {c}"


def ocr(img) -> str:
    if img is None or not shutil.which("tesseract"):
        return ""
    tmp = Path("/tmp/qa-ocr.png")
    big = img.resize((img.width * 2, img.height * 2))
    big.save(tmp)
    r = subprocess.run(["tesseract", str(tmp), "-", "--psm", "11"], capture_output=True, text=True)
    return r.stdout


# ---------------------------------------------------------------- one component


def run_component(slug: str, out_root: Path, only_step: int | None, themes, widths) -> dict:
    out = out_root / slug
    if out.exists() and only_step is None:
        shutil.rmtree(out)
    (out / "shots").mkdir(parents=True, exist_ok=True)
    result = {"slug": slug, "started": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "checks": {},
              "shots": {}, "steps": [], "notes": []}
    comp = component_file(slug)

    app = App(slug, themes[0], widths[0], out)
    renders = {}
    if not app.wid:
        result["checks"]["C1"] = {"status": "fail", "note": "no window appeared within 60 s"}
        result["panic"] = panic_lines(app.stderr_path)
        app.close()
        return finish(result, out)

    # Render matrix: both themes at all three widths.
    overflow = {}
    bands = {}
    for theme in themes:
        app.set_theme(theme)
        for width in widths:
            app.set_width(width)
            key = f"{theme}-{width}"
            p = app.shot(key)
            renders[key] = p
            if p:
                img = Image.open(p).convert("RGB")
                box = content_box(img)
                renders[key + ":blank"] = 0.0 if box is None else (box[2] - box[0]) * (box[3] - box[1])
                bands[key] = luminance_bands(img, content_box(img))
            if theme == themes[0]:
                ov = overflow_nodes(app.atspi(), width)
                if ov:
                    overflow[width] = ov[:6]
    result["shots"] = {k: str(v.relative_to(out_root)) for k, v in renders.items() if isinstance(v, Path)}

    # Accessibility snapshot at the default width.
    app.set_width(960)
    tree = app.tree()
    atspi = app.atspi()
    (out / "a11y.json").write_text(json.dumps({"app": tree, "atspi": atspi}, indent=1))
    result["a11y"] = a11y_summary(tree, atspi)

    # C1 renders
    panics = panic_lines(app.stderr_path)
    blank = [k for k in renders if k.endswith(":blank") and renders[k] < 400]
    if not app.alive() or any("panicked" in p for p in panics):
        result["checks"]["C1"] = {"status": "fail", "note": "panicked: " + "; ".join(panics[:3])}
    elif blank:
        result["checks"]["C1"] = {"status": "fail", "note": "blank render: " + ", ".join(b.split(":")[0] for b in blank)}
    else:
        result["checks"]["C1"] = {"status": "pass", "note": "renders in both themes at 360, 960 and 1440"}
    if panics:
        result["log"] = panics

    # C2 layout (automatic part: nothing past the window edge)
    if overflow:
        worst = min(overflow)
        result["checks"]["C2"] = {"status": "warn", "note": f"content reaches past the window edge at {worst} px",
                                  "detail": overflow}
    else:
        result["checks"]["C2"] = {"status": "pass", "note": "nothing reaches past the window edge"}

    # C3 theme
    lits = color_literals(comp)
    dark_light = [k for k, (light, dark) in bands.items() if k.startswith("dark") and light > 0.35]
    light_dark = [k for k, (light, dark) in bands.items() if k.startswith("light") and dark > 0.35]
    notes = []
    if lits:
        notes.append(f"{len(lits)} colour literal(s) in the component file")
    if dark_light:
        notes.append("large light areas in the dark theme")
    if light_dark:
        notes.append("large dark areas in the light theme")
    result["checks"]["C3"] = {"status": "warn" if notes else "pass",
                              "note": "; ".join(notes) or "theme colours only; both themes read",
                              "literals": lits}

    # C8: does the story come to rest after it loads?
    if app.settled_at_load is not None:
        result["checks"]["C8"] = {"status": "pass", "note": f"comes to rest {app.settled_at_load:.1f} s after loading"}
    else:
        result["checks"]["C8"] = {"status": "info", "note": "still moving 6 s after loading"}

    # C9 and C6 at 960 in the light theme
    app.set_theme("light")
    result["checks"]["C9"] = a11y_check(tree)
    result["checks"]["C10"] = frame_check(app, frame_baseline(out.parent))
    kb = keyboard_walk(app)
    result["keyboard"] = kb
    result["checks"]["C6"] = {"status": kb["status"], "note": kb["note"]}
    app.close()
    app = App(slug, "light", 960, out)

    # Script steps (C4 and on)
    script = load_script(slug)
    if script:
        play_script(app, script, result, out, only_step)
    app.close()

    sheet = contact_sheet({k: v for k, v in renders.items() if isinstance(v, Path)}, out / "contact.png")
    if sheet:
        result["contact"] = str(sheet.relative_to(out_root))
    return finish(result, out)


def play_script(app: App, script: dict, result: dict, out: Path, only_step: int | None):
    steps = script.get("steps", [])
    ctx = {"shots": {}, "prefix": "step-"}
    if "settle" in script:
        # Stories with a clock that never stops would otherwise wait the full timeout on every step.
        app.settle_timeout = float(script["settle"])
    app.set_theme(script.get("theme", "light"))
    app.set_width(int(script.get("width", 960)))
    # Start with the pointer away from the story, so the first hover is a real move.
    app.hover_at(int(script.get("width", 960)) - 4, HEIGHT - 4)
    for n, step in enumerate(steps, 1):
        if only_step is not None and n > only_step:
            break
        ok, note = run_step(app, step, ctx)
        rec = {"n": n, "name": step.get("name", ""), "check": step.get("check", "C4"), "pass": ok, "note": note}
        if only_step is None or n == only_step:
            result["steps"].append(rec)
        if not app.alive():
            result["steps"].append({"n": n, "name": "app exited", "check": "C1", "pass": False,
                                    "note": "; ".join(panic_lines(app.stderr_path)[:3])})
            break
    for k, v in ctx["shots"].items():
        if v:
            result["shots"]["step:" + k] = str(v.relative_to(out.parent))
    by = {}
    for s in result["steps"]:
        by.setdefault(s["check"], []).append(s)
    order = {None: 0, "info": 0, "pass": 1, "warn": 2, "fail": 3}
    for c, items in by.items():
        failed = [s for s in items if not s["pass"]]
        status = "fail" if failed else "pass"
        note = ("; ".join(f"step {s['n']} {s['name']}: {s['note']}" for s in failed)
                or f"{len(items)} scripted step(s) pass")
        prev = result["checks"].get(c)
        if prev:
            # Keep what the automatic check found next to what the script found.
            status = max(status, prev.get("status"), key=lambda v: order.get(v, 0))
            note = "; ".join(x for x in [prev.get("note", ""), note] if x)
        result["checks"][c] = {**(prev or {}), "status": status, "note": note}


def a11y_summary(tree: list[dict], atspi: list[dict]) -> dict:
    actionable = [n for n in tree if set(n.get("on_action") or []) - {"Focus"}]
    unnamed = [n for n in actionable if not n.get("label") and not content_name(tree, n)]
    return {"nodes": len(tree), "actionable": len(actionable), "unnamed_actionable": len(unnamed),
            "unnamed": [n.get("element_id") or n.get("id") for n in unnamed][:20],
            "atspi_nodes": len(atspi)}


def finish(result: dict, out: Path) -> dict:
    result["finished"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    (out / "results.json").write_text(json.dumps(result, indent=1))
    return result


# ---------------------------------------------------------------- entry


def main():
    ap = argparse.ArgumentParser(prog="qa-runner")
    sub = ap.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("run")
    r.add_argument("slugs", nargs="*")
    r.add_argument("--all", action="store_true")
    r.add_argument("--step", type=int)
    r.add_argument("--out", default=str(QA / "out"))
    r.add_argument("--themes", default=",".join(THEMES))
    r.add_argument("--widths", default=",".join(map(str, WIDTHS)))
    sub.add_parser("list")
    args = ap.parse_args()

    ensure_display()
    if args.cmd == "list":
        print("\n".join(story_slugs()))
        return 0

    ensure_a11y()
    slugs = story_slugs() if args.all else args.slugs
    if not slugs:
        ap.error("name a story or pass --all")
    unknown = [s for s in slugs if s not in stories()]
    if unknown:
        ap.error("no such story: " + ", ".join(unknown) + " (see `list`)")
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    summary = []
    for slug in slugs:
        log(f"== {slug}")
        t0 = time.time()
        res = run_component(slug, out, args.step, args.themes.split(","), [int(w) for w in args.widths.split(",")])
        line = " ".join(f"{c}:{v['status']}" for c, v in sorted(res["checks"].items()))
        log(f"   {line}  ({time.time() - t0:.0f}s)")
        summary.append({"slug": slug, "checks": {c: v["status"] for c, v in res["checks"].items()}})
    (out / "summary.json").write_text(json.dumps(summary, indent=1))
    failed = any(v == "fail" for s in summary for v in s["checks"].values())
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
