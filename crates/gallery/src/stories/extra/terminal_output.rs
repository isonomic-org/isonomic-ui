// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the terminal output.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::v_flex;
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window,
};
use isonomic_ui_extra::terminal_output::{RunStatus, TerminalLine, TerminalOutput};

/// How often the made-up build prints a line.
const TICK: Duration = Duration::from_millis(350);

fn build_lines() -> Vec<TerminalLine> {
    let mut lines = vec![
        TerminalLine::info("bakery-site build, 14 pages"),
        TerminalLine::new("reading recipes/*.md"),
    ];
    for page in [
        "index",
        "sourdough",
        "rye",
        "focaccia",
        "croissants",
        "opening-hours",
        "wholesale",
        "contact",
    ] {
        lines.push(TerminalLine::new(format!("  rendered /{page}.html")));
    }
    lines.extend([
        TerminalLine::error("warning: photo croissants.jpg is 6.2 MB, over the 2 MB budget"),
        TerminalLine::new(""),
        TerminalLine::new("optimising 31 images"),
        TerminalLine::new("writing sitemap.xml"),
        TerminalLine::info("built 14 pages in 5.6s"),
    ]);
    lines
}

/// A site build that prints as it goes, one that failed, and one yet to print.
pub struct TerminalOutputStory {
    shown: usize,
    stopped: bool,
}

impl TerminalOutputStory {
    /// Create the story view and start the made-up build.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(TICK).await;
                    let going = this
                        .update(cx, |this: &mut Self, cx| {
                            if this.stopped || this.shown >= build_lines().len() {
                                return false;
                            }
                            this.shown += 1;
                            cx.notify();
                            true
                        })
                        .unwrap_or(false);
                    if !going {
                        break;
                    }
                }
            })
            .detach();
            Self {
                shown: 2,
                stopped: false,
            }
        })
    }
}

impl Story for TerminalOutputStory {
    fn title() -> &'static str {
        "Terminal output"
    }

    fn description() -> &'static str {
        "One command and what it printed: follows new lines, marks errors, counts lines, copies and stops."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for TerminalOutputStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let all = build_lines();
        let done = self.shown >= all.len();
        let status = match (done, self.stopped) {
            (true, _) => RunStatus::Succeeded,
            (false, true) => RunStatus::Failed,
            (false, false) => RunStatus::Running,
        };
        let lines: Vec<TerminalLine> = all.into_iter().take(self.shown).collect();
        let this = cx.entity();

        v_flex()
            .gap_6()
            .child(
                TerminalOutput::new("build", "npm run build")
                    .cwd("~/sites/bakery")
                    .lines(lines)
                    .status(status)
                    .when_stopped(self.stopped)
                    .time(TICK * self.shown as u32)
                    .max_height(isonomic_ui::px(200.))
                    .on_stop(move |_, cx| {
                        this.update(cx, |this, cx| {
                            this.stopped = true;
                            cx.notify();
                        })
                    }),
            )
            .child(
                TerminalOutput::new("deploy", "rsync -az dist/ shop:/var/www")
                    .lines(vec![
                        TerminalLine::new("sending incremental file list"),
                        TerminalLine::error("rsync: connection unexpectedly closed"),
                        TerminalLine::error("rsync error: error in rsync protocol data stream"),
                    ])
                    .status(RunStatus::Failed)
                    .exit_code(12)
                    .time(Duration::from_millis(840)),
            )
            .child(TerminalOutput::new("tests", "npm test").cwd("~/sites/bakery"))
    }
}

/// Lets the story give a stopped run the exit code a signal leaves.
trait WhenStopped {
    fn when_stopped(self, stopped: bool) -> Self;
}

impl WhenStopped for TerminalOutput {
    fn when_stopped(self, stopped: bool) -> Self {
        if stopped { self.exit_code(130) } else { self }
    }
}
