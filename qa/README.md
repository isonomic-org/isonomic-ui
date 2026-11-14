# QA for isonomic-ui

This folder is a separate Cargo workspace for testing the components in
`crates/extra`. Nothing here is a dependency of the library, so QA code,
patches and tools never reach a release build.

## What is here

| Path | What it is |
| --- | --- |
| `gallery/` | `qa-gallery`, a window that shows one story (`--story card`) or a browsable list of all of them, with a light/dark switch and 360/960/1440 px width buttons. A build script finds every `*Story` under `crates/gallery/src/stories/extra/`, so new stories show up without editing anything here. Over stdin it takes `theme`, `size`, `a11y`, `focus`, `ping`, `frames` and `tabstart`, which is how the runner drives it. |
| `runner/` | `qa-runner`, the command that runs the checks. The Rust binary builds the gallery and hands over to `runner/driver/qa_driver.py`. |
| `runner/driver/code_health.py` | C11: clippy with `-D warnings`, the unit tests and `rustfmt --edition 2024 --check`, attributed to each component file. |
| `runner/retest.py` | Retests every open QA issue listed in `review/issues.json`. |
| `runner/probe/atspi_probe.py` | Reads the gallery's accessibility tree through the desktop accessibility bus (roles, names, on-screen bounds, actions). |
| `scripts/<slug>.yaml` | One script per component: a step for each stated behaviour in its pull request, with the expected result. |
| `report/` | Builds the QA report page, the pull request comments and the status table. `visual_diff.py` compares screenshots with the baselines. |
| `review/` | Findings from reviewing the screenshots (`notes.yaml`), the filed defects (`defects.yaml`) and the issue list the retest uses (`issues.json`). |
| `baselines.toml` | Which baseline screenshots are approved. |
| `ci/qa.yml` | The QA workflow, mirrored at `.github/workflows/qa.yml`. |

## Running it

Linux with an X server (a virtual one is fine), an X input tool, a screenshot
tool, an OCR engine, the desktop accessibility bus and Python 3. The exact
packages are the ones the workflow installs in `ci/qa.yml`. The runner starts a
virtual display on `:99` and a window manager if none is running.

```sh
cd qa
cargo run -p qa-gallery -- --story card --theme dark --width 360   # look at one story
cargo run -p qa-runner -- run card                                 # all checks for one component
cargo run -p qa-runner -- run card --step 3                        # up to one script step
cargo run -p qa-runner -- run --all                                # everything
python3 runner/driver/code_health.py                               # C11 for every component
```

Results go to `qa/out/<slug>/results.json`, with screenshots in
`qa/out/<slug>/shots/` and a contact sheet in `qa/out/<slug>/contact.png`.

## How the checks work

The runner drives the real gallery window: it takes screenshots of it,
sends real pointer and keyboard input through the X server, and reads the
window's own accessibility tree (dumped by the gallery on request) and the
same tree through the desktop accessibility bus. No test code is compiled into the components.

| Check | How |
| --- | --- |
| C1 Renders | The story opens in both themes at 360, 960 and 1440 px without a panic or an empty window. |
| C2 Layout | Controls that reach past the window edge, from their on-screen bounds; the rest by reviewing the screenshots. |
| C3 Theme | Colour literals in the component file, and large light or dark areas in the wrong theme. |
| C4 Stated behaviour | The component's script: one step per Behaviour bullet, checked on screen. |
| C5 States | Script steps for empty, disabled, error and finished states. |
| C6 Keyboard | Tab through the story from the first stop: every stop reached, the focus visibly moves, Tab cycles, and Shift+Tab goes back. Script steps cover arrows, Enter, Space and Escape. |
| C7 Content stress | Not automated yet. |
| C8 Motion and time | Does the story come to rest after loading, and after its script? |
| C9 Accessibility | Every control with a click action has a role and a name. |
| C10 Performance | 120 back-to-back redraws at 960 × 900, compared with an empty window. The figures come from a dev build under software rendering, so only the difference from the empty window means anything. |
| C11 Code health | `code_health.py`. |

Text on screen is read with OCR where the accessibility tree does not carry it.
OCR can miss text, so a failing text check is always confirmed against the
screenshot before a defect is filed.

## Baselines

The first full run's screenshots are the proposed baselines, kept on the
`qa-evidence` branch under `baselines/<slug>/`. `report/visual_diff.py`
compares each run with them and lists every visual change in the report.
A baseline counts as approved once it is listed in `baselines.toml`.

## Layouts

The QA gallery builds `crates/extra` as it was before the component pull
requests moved onto the foundation: on the published base crates, with
stories that have their own `view` constructor. On the foundation layout
(`crates/extra` on `isonomic-ui`, stories implementing the gallery's `Story`
trait) it does not build yet, and the workflow skips the QA run with a notice
instead of reporting a false failure. Porting it means depending on
`isonomic-ui` and `isonomic-ui-extra` by path, carrying the workspace's
`gpui-pre-macros` patch, and declaring a `Story` trait at the crate root that
matches the gallery's. That port needs a build of the full workspace, which
the QA session could not fetch.
