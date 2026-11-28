// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the checkpoint timeline.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, IconName, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Task, Window, div,
};
use isonomic_ui_extra::checkpoint_timeline::{Checkpoint, CheckpointStatus, CheckpointTimeline};

/// How long the pretend agent takes to save the next checkpoint.
const WORK_FOR: Duration = Duration::from_millis(2500);

/// What the pretend agent saves after a rewind, in turn.
const RETRIES: [&str; 4] = [
    "Rebuilt the cash flow table",
    "Checked the totals against the ledger",
    "Rounded to whole currency units",
    "Exported the board pack",
];

/// A quarter-end report run whose last step failed. Rewind adds an attempt
/// that the pretend agent finishes a moment later.
pub struct CheckpointTimelineStory {
    checkpoints: Vec<Checkpoint>,
    head: SharedString,
    tries: usize,
    last: SharedString,
    _work: Option<Task<()>>,
}

impl CheckpointTimelineStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        let secs = Duration::from_secs;
        cx.new(|_| Self {
            checkpoints: vec![
                Checkpoint::new("brief", "Read the reporting brief")
                    .icon(IconName::FileText)
                    .at(secs(9))
                    .cost(0.02)
                    .turn(1)
                    .summary("Read what the board asked for and which periods to cover."),
                Checkpoint::new("pull", "Pulled the trial balance")
                    .after("brief")
                    .icon(IconName::HardDrive)
                    .at(secs(31))
                    .cost(0.05)
                    .turn(2)
                    .summary("Exported balances for the quarter from the ledger.")
                    .change("data/trial-balance-q1.csv", 412, 0)
                    .change("data/accounts.csv", 96, 0),
                Checkpoint::new("map", "Mapped accounts to report lines")
                    .after("pull")
                    .icon(IconName::Map)
                    .at(secs(58))
                    .cost(0.07)
                    .turn(3)
                    .summary("Grouped 96 accounts into the lines the report shows.")
                    .change("report/mapping.toml", 118, 4),
                Checkpoint::new("draft", "Drafted the profit and loss")
                    .after("map")
                    .icon(IconName::FileText)
                    .at(secs(95))
                    .cost(0.09)
                    .turn(4)
                    .summary("Wrote the profit and loss with notes on the three largest moves.")
                    .change("report/profit-and-loss.md", 84, 0)
                    .change("report/notes.md", 27, 3)
                    .change("report/mapping.toml", 6, 2),
                Checkpoint::new("check", "Balance check failed by 412.00")
                    .after("draft")
                    .icon(IconName::CircleAlert)
                    .status(CheckpointStatus::Failed)
                    .at(secs(121))
                    .cost(0.04)
                    .turn(5)
                    .summary("Assets and liabilities did not tie out; nothing was written."),
            ],
            head: "check".into(),
            tries: 0,
            last: "Nothing yet".into(),
            _work: None,
        })
    }

    fn rewind(&mut self, to: &Checkpoint, window: &mut Window, cx: &mut Context<Self>) {
        for checkpoint in &mut self.checkpoints {
            if checkpoint.current_status() == CheckpointStatus::Running {
                *checkpoint = checkpoint.clone().status(CheckpointStatus::Stopped);
            }
        }
        let title = RETRIES[self.tries % RETRIES.len()];
        self.tries += 1;
        let id: SharedString = format!("try-{}", self.tries).into();
        let at = Duration::from_secs(121 + 25 * self.tries as u64);
        self.checkpoints.push(
            Checkpoint::new(id.clone(), title)
                .after(to.id().clone())
                .icon(IconName::RefreshCw)
                .status(CheckpointStatus::Running)
                .at(at)
                .cost(0.06)
                .turn(5 + self.tries)
                .summary("Tried again from here after the rewind.")
                .change("report/cash-flow.md", 40 + 3 * self.tries, 12)
                .change("report/notes.md", 5, 1),
        );
        self.head = id.clone();
        self.last = format!("Rewound to {}", to.title()).into();
        self._work = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(WORK_FOR).await;
            this.update(cx, |this, cx| {
                for checkpoint in &mut this.checkpoints {
                    if checkpoint.id() == &id
                        && checkpoint.current_status() == CheckpointStatus::Running
                    {
                        *checkpoint = checkpoint.clone().status(CheckpointStatus::Done);
                    }
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }
}

impl Story for CheckpointTimelineStory {
    fn title() -> &'static str {
        "Checkpoint timeline"
    }

    fn description() -> &'static str {
        "An agent's run as a line of saved points."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for CheckpointTimelineStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let timeline = |id: &'static str, this: &Self, cx: &mut Context<Self>| {
            let story = cx.entity();
            this.checkpoints
                .iter()
                .cloned()
                .fold(
                    CheckpointTimeline::new(id).task("Quarter-end board pack"),
                    CheckpointTimeline::checkpoint,
                )
                .head(this.head.clone())
                .on_rewind(move |checkpoint, window, cx| {
                    story.update(cx, |this, cx| this.rewind(checkpoint, window, cx))
                })
        };
        let plain = timeline("report-checkpoints", self, cx);
        let scrubbed = timeline("report-scrub", self, cx)
            .title("Turns")
            .scrubber(true)
            .show_changes(true);

        v_flex()
            .gap_3()
            .child(plain)
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.last.clone()),
            )
            .child(scrubbed)
    }
}
