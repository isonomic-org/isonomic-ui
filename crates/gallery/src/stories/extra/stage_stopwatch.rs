// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the stage stopwatch.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::h_flex;
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window,
};
use isonomic_ui_extra::stage_stopwatch::{StageStopwatch, StopwatchStage};

/// How often the made-up watch moves on.
const TICK: Duration = Duration::from_millis(100);

/// A made-up site deploy: each stage, how long it takes this run, and how
/// long it took last run, in tenths of a second.
const DEPLOY: [(&str, u64, u64); 5] = [
    ("Fetch sources", 34, 41),
    ("Install packages", 96, 88),
    ("Build pages", 122, 130),
    ("Check links", 58, 52),
    ("Upload", 40, 45),
];

/// A deploy timed live from part way through, and a finished nightly backup.
pub struct StageStopwatchStory {
    /// Tenths of a second on the watch.
    tenths: u64,
    running: bool,
}

impl StageStopwatchStory {
    /// Create the story view and start the watch.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(TICK).await;
                    let alive = this
                        .update(cx, |this: &mut Self, cx| {
                            let total: u64 = DEPLOY.iter().map(|stage| stage.1).sum();
                            if this.running && this.tenths < total {
                                this.tenths += 1;
                                if this.tenths >= total {
                                    this.running = false;
                                }
                                cx.notify();
                            }
                        })
                        .is_ok();
                    if !alive {
                        break;
                    }
                }
            })
            .detach();
            Self {
                tenths: 172,
                running: true,
            }
        })
    }

    fn stages(&self) -> Vec<StopwatchStage> {
        let mut left = self.tenths;
        DEPLOY
            .iter()
            .map(|&(name, this_run, last_run)| {
                let stage =
                    StopwatchStage::new(name).previous(Duration::from_millis(last_run * 100));
                if left >= this_run {
                    left -= this_run;
                    stage.split(Duration::from_millis(this_run * 100))
                } else {
                    left = 0;
                    stage
                }
            })
            .collect()
    }
}

impl Story for StageStopwatchStory {
    fn title() -> &'static str {
        "Stage stopwatch"
    }

    fn description() -> &'static str {
        "A pipeline timed stage by stage on a stopwatch dial, with splits and changes against last run."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for StageStopwatchStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let start = cx.entity();
        let stop = cx.entity();
        let reset = cx.entity();
        let backup = [
            ("Snapshot", 210, 240),
            ("Compress", 640, 600),
            ("Encrypt", 180, 185),
        ];
        h_flex()
            .gap_6()
            .items_start()
            .flex_wrap()
            .child(
                StageStopwatch::new("deploy", "Site deploy", self.stages())
                    .subtitle("main at 4f2c9e1")
                    .elapsed(Duration::from_millis(self.tenths * 100))
                    .running(self.running)
                    .on_start(move |_, cx| {
                        start.update(cx, |this, cx| {
                            this.running = true;
                            cx.notify();
                        })
                    })
                    .on_stop(move |_, cx| {
                        stop.update(cx, |this, cx| {
                            this.running = false;
                            cx.notify();
                        })
                    })
                    .on_reset(move |_, cx| {
                        reset.update(cx, |this, cx| {
                            this.tenths = 0;
                            this.running = false;
                            cx.notify();
                        })
                    }),
            )
            .child(
                StageStopwatch::new(
                    "backup",
                    "Nightly backup",
                    backup
                        .iter()
                        .map(|&(name, this_run, last_run)| {
                            StopwatchStage::new(name)
                                .split(Duration::from_millis(this_run * 100))
                                .previous(Duration::from_millis(last_run * 100))
                        })
                        .collect(),
                )
                .subtitle("Finished at 02:14")
                .elapsed(Duration::from_millis(1_030 * 100)),
            )
    }
}
