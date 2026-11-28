# Copyright 2026 Anud Inc.
# SPDX-License-Identifier: GPL-3.0-or-later

"""Reads the QA gallery's accessibility tree over the desktop accessibility bus and acts on it.

Usage (python3.12, inside the runner's private D-Bus session):
  atspi_probe.py tree                 -> JSON list of nodes
  atspi_probe.py action <id|label> [action-name]
Each node: id (element id), role, name, description, x, y, w, h,
states (focused, focusable, selected, checked, enabled, ...), actions, depth.
"""
import json
import sys

import pyatspi


def find_app():
    desktop = pyatspi.Registry.getDesktop(0)
    for i in range(desktop.childCount):
        app = desktop.getChildAtIndex(i)
        if app is not None and app.name and "qa-gallery" in app.name:
            return app
    # Fall back to the only application, if there is one.
    for i in range(desktop.childCount):
        app = desktop.getChildAtIndex(i)
        if app is not None:
            return app
    return None


def node(acc, depth):
    try:
        comp = acc.queryComponent()
        ext = comp.getExtents(pyatspi.DESKTOP_COORDS)
        box = [ext.x, ext.y, ext.width, ext.height]
    except Exception:
        box = [0, 0, 0, 0]
    try:
        states = [pyatspi.stateToString(s) for s in acc.getState().getStates()]
    except Exception:
        states = []
    actions = []
    try:
        act = acc.queryAction()
        actions = [act.getName(i) for i in range(act.nActions)]
    except Exception:
        pass
    try:
        attrs = dict(a.split(":", 1) for a in acc.getAttributes() if ":" in a)
    except Exception:
        attrs = {}
    return {
        "id": attrs.get("id") or attrs.get("element-id") or "",
        "role": acc.getRoleName(),
        "name": acc.name or "",
        "description": acc.description or "",
        "x": box[0], "y": box[1], "w": box[2], "h": box[3],
        "states": states,
        "actions": actions,
        "attrs": attrs,
        "depth": depth,
    }


def walk(acc, depth, out):
    out.append(node(acc, depth))
    for i in range(acc.childCount):
        child = acc.getChildAtIndex(i)
        if child is not None:
            walk(child, depth + 1, out)


def main():
    app = find_app()
    if app is None:
        print(json.dumps({"error": "no application on the accessibility bus"}))
        return 1
    if sys.argv[1] == "tree":
        out = []
        walk(app, 0, out)
        print(json.dumps(out))
        return 0
    if sys.argv[1] == "action":
        target, name = sys.argv[2], (sys.argv[3] if len(sys.argv) > 3 else None)
        out = []
        objs = []

        def collect(acc, depth):
            objs.append((acc, node(acc, depth)))
            for i in range(acc.childCount):
                c = acc.getChildAtIndex(i)
                if c is not None:
                    collect(c, depth + 1)

        collect(app, 0)
        for acc, n in objs:
            if target in (n["id"], n["name"]):
                act = acc.queryAction()
                for i in range(act.nActions):
                    if name is None or act.getName(i) == name:
                        act.doAction(i)
                        print(json.dumps({"ok": True, "node": n}))
                        return 0
        print(json.dumps({"ok": False, "error": f"no node {target!r} with action {name!r}"}))
        return 1
    return 2


if __name__ == "__main__":
    sys.exit(main())
