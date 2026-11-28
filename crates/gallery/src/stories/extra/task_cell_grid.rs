// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the task cell grid.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div, px,
};
use isonomic_ui_extra::task_cell_grid::{CellStatus, GridTask, TaskCellGrid};

/// Shows 180 receipts being matched to payments, part way through.
pub struct TaskCellGridStory {
    last: SharedString,
}

impl TaskCellGridStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            last: "Nothing picked yet".into(),
        })
    }
}

/// Where task `n` of the sample job stands.
fn sample(n: usize) -> GridTask {
    let label = format!("Receipt {}", 4100 + n);
    let task = GridTask::new(format!("r{n}"), label).worker(format!("Matcher {}", n % 8 + 1));
    let start = Duration::from_millis(n as u64 * 90);
    match n {
        37 | 92 => task
            .status(CellStatus::Failed)
            .span(start, Some(start + Duration::from_millis(2600)))
            .attempts(3)
            .cost(0.012)
            .outcome("No payment within 30 days of the receipt"),
        64 => task
            .status(CellStatus::Retrying)
            .span(start, None)
            .attempts(2)
            .cost(0.004),
        n if n < 110 => task
            .status(CellStatus::Done)
            .span(
                start,
                Some(start + Duration::from_millis(900 + (n as u64 * 37) % 700)),
            )
            .cost(0.006)
            .outcome("Matched to a card payment"),
        n if n < 122 => task
            .status(CellStatus::Running)
            .span(start, None)
            .cost(0.002),
        _ => task,
    }
}

impl Story for TaskCellGridStory {
    fn title() -> &'static str {
        "Task cell grid"
    }

    fn description() -> &'static str {
        "One large job split across many workers."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for TaskCellGridStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let picks = cx.entity();
        let retries = picks.clone();

        v_flex()
            .gap_3()
            .max_w(px(620.))
            .child(
                TaskCellGrid::new("matching", "Matching")
                    .job("Match 180 receipts to payments")
                    .tasks((0..180).map(sample))
                    .workers(12)
                    .columns(32)
                    .now(Duration::from_secs(14))
                    .on_select(move |id, _, cx| {
                        let line = match id {
                            Some(id) => format!("Kept {id} open"),
                            None => "Let go".to_string(),
                        };
                        picks.update(cx, |this, cx| {
                            this.last = line.into();
                            cx.notify();
                        })
                    })
                    .on_retry(move |ids, _, cx| {
                        let line = format!("Retry asked for {} tasks", ids.len());
                        retries.update(cx, |this, cx| {
                            this.last = line.into();
                            cx.notify();
                        })
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.last.clone()),
            )
    }
}
