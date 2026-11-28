// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the evaluation track.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::v_flex;
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window,
};
use isonomic_ui_extra::evaluation_track::{EvaluationTrack, TrackRun};

/// How often the made-up runs report progress.
const TICK: Duration = Duration::from_millis(400);

/// Made-up runs: name, where they start, laps gained per report, and score.
const RUNS: [(&str, f32, f32, f32); 5] = [
    ("Baseline, March", 2.10, 0.035, 0.712),
    ("Longer context", 2.42, 0.041, 0.748),
    ("Smaller batch", 1.86, 0.052, 0.731),
    ("Retrieval on", 2.31, 0.030, 0.766),
    ("Distilled", 1.40, 0.060, 0.689),
];

/// Five runs racing through a five-lap evaluation, and a finished pair.
pub struct EvaluationTrackStory {
    reports: u32,
}

impl EvaluationTrackStory {
    /// Create the story view and start the race.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(TICK).await;
                    let going = this
                        .update(cx, |this: &mut Self, cx| {
                            if this.reports >= 60 {
                                return false;
                            }
                            this.reports += 1;
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
            Self { reports: 0 }
        })
    }
}

impl Story for EvaluationTrackStory {
    fn title() -> &'static str {
        "Evaluation track"
    }

    fn description() -> &'static str {
        "Evaluation runs lapping an oval by progress, with standings, places and gaps to the leader."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for EvaluationTrackStory {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let reports = self.reports as f32;
        v_flex()
            .gap_6()
            .child(
                EvaluationTrack::new("nightly", "Nightly evaluation", 5)
                    .subtitle("Five passes over 1,200 support questions")
                    .score_label("Accuracy")
                    .runs(RUNS.iter().map(|&(name, start, pace, score)| {
                        let progress = (start + pace * reports).min(5.);
                        let drift = 0.004 * (reports * 0.7 + start * 9.).sin();
                        TrackRun::new(name, progress).score(format!("{:.3}", score + drift))
                    })),
            )
            .child(
                EvaluationTrack::new("pair", "Prompt A against B", 3)
                    .subtitle("Finished")
                    .run(TrackRun::new("Prompt A", 3.))
                    .run(TrackRun::new("Prompt B", 2.62)),
            )
    }
}
