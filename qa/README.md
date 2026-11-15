# QA for isonomic-ui

This folder is a separate Cargo workspace for testing every story in the
component gallery: the components of `crates/component` and of
`crates/extra`. Nothing here is a dependency of the library, so QA code and
tools never reach a release build.

Everything runs headless, under a virtual display. QA never drives a
person's own screen, pointer or keyboard.

## What is here

| Path | What it is |
| --- | --- |
| `gallery/` | `qa-gallery`, a bare window that shows one story of the component gallery (`--story "Model picker"`). It uses the gallery's own story list, so new stories show up without editing anything here. Over stdin it takes `theme`, `size`, `a11y`, `focus`, `ping`, `frames` and `tabstart`, which is how the runner drives it. |
| `runner/` | `qa-runner`, the command that runs the checks. The Rust binary builds the gallery and hands over to `runner/driver/qa_driver.py`. |
| `runner/driver/code_health.py` | C11 for the components of `crates/extra`: clippy with `-D warnings`, the unit tests and `rustfmt --edition 2024 --check`, attributed to each component file. |
| `runner/retest.py` | Retests every open QA issue listed in `review/issues.json`. |
| `runner/probe/atspi_probe.py` | Reads the gallery's accessibility tree through the desktop accessibility bus (roles, names, on-screen bounds, actions). |
| `scripts/<slug>.yaml` | One script per component: a step for each stated behaviour in its pull request, with the expected result. |
| `report/` | `build_report.py` builds the QA report page, `status.py` the status table and `ci_comment.py` a pull request comment. `visual_diff.py` compares screenshots with the baselines. |
| `review/` | Findings from reviewing the screenshots (`notes.yaml`), the filed defects (`defects.yaml`) and the issue list the retest uses (`issues.json`). |
| `baselines.toml` | Which baseline screenshots are approved. |

## Running it

Linux with an X server (a virtual one is fine), an X input tool, a screenshot
tool, an OCR engine, the desktop accessibility bus and the system Python 3. The
exact packages are the ones `.github/workflows/qa.yml` installs. The runner
starts a virtual display on `:99` and a window manager if none is running.

A slug names a story. A component of `crates/extra` has its file name as its
slug (`model-picker`); every other gallery story has `base-` and its name
(`base-button`). `cargo run -p qa-runner -- list` prints them all.

```sh
cd qa
cargo run -p qa-runner -- run card                                 # all checks for one component
cargo run -p qa-runner -- run card base-button                     # several
cargo run -p qa-runner -- run card --step 3                        # up to one script step
cargo run -p qa-runner -- run --all                                # everything
python3 runner/driver/code_health.py                               # C11 for the extra components
python3 report/status.py --run out --issues review/issues.json     # the status table
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
| C9 Accessibility | Every control with a click action has a role and a name: its own label, or the text inside it. An element with no role is not in the tree at all, so C9 cannot see it; C6 shows it as a Tab stop that reports no element. |
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

## The workflow

`.github/workflows/qa.yml` is manual only: it has no pull request trigger and
no schedule, so it adds nothing to any pull request's checks. Start it from
the Actions tab with the stories to test, or to retest the open QA issues.
Its macOS job is a separate switch, off by default, because macOS minutes
cost ten times Linux ones.
