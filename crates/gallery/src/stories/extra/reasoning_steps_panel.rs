// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the reasoning steps panel.

use std::time::{Duration, Instant};

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Task, Window, div,
};
use isonomic_ui_extra::reasoning_steps_panel::{
    ReasoningSource, ReasoningStep, ReasoningStepsPanel,
};

/// How long the first run rests on its finished state before starting over.
const RESTART_AFTER: Duration = Duration::from_secs(3);

fn reconciling() -> Vec<ReasoningStep> {
    vec![
        ReasoningStep::new("Searching")
            .finished_label("Searched")
            .source(
                ReasoningSource::new("Ledger: March entries")
                    .detail("books.example/ledger/2026-03"),
            )
            .source(
                ReasoningSource::new("Bank feed: main account").detail("books.example/feeds/main"),
            )
            .source(ReasoningSource::new("Supplier statements"))
            .hidden_source(ReasoningSource::new("Payroll run 06"))
            .hidden_source(ReasoningSource::new("VAT return draft")),
        ReasoningStep::new("Matching 41 payments to invoices"),
        ReasoningStep::new("Checking the totals against the bank"),
        ReasoningStep::new("Writing up the differences"),
    ]
}

fn grouping() -> Vec<ReasoningStep> {
    vec![
        ReasoningStep::new("Reading the open invoices"),
        ReasoningStep::new("Grouping them by customer"),
    ]
}

/// A pretend run of work: a new step every `every`, done one beat after the
/// last. In an app the session says what happened; here a clock does.
struct Run {
    started: Instant,
    every: Duration,
    steps: usize,
    shown: usize,
    took: Option<Duration>,
}

impl Run {
    fn new(every: Duration, steps: usize) -> Self {
        Self {
            started: Instant::now(),
            every,
            steps,
            shown: 1,
            took: None,
        }
    }

    /// Move the run on to `now`. Returns whether it finished just now.
    fn advance(&mut self, now: Instant) -> bool {
        if self.took.is_some() {
            return false;
        }
        let elapsed = now.saturating_duration_since(self.started);
        let passed = (elapsed.as_millis() / self.every.as_millis().max(1)) as usize;
        self.shown = (passed + 1).min(self.steps);
        if passed >= self.steps {
            self.took = Some(elapsed);
            return true;
        }
        false
    }

    fn panel(&self, id: &'static str, steps: Vec<ReasoningStep>) -> ReasoningStepsPanel {
        let panel = steps
            .into_iter()
            .take(self.shown)
            .fold(ReasoningStepsPanel::new(id), |panel, step| panel.step(step))
            .started_at(self.started);
        match self.took {
            Some(took) => panel.done(took),
            None => panel,
        }
    }
}

/// Shows a looping run with a search step, and one that folds away when done.
pub struct ReasoningStepsPanelStory {
    looping: Run,
    folding: Run,
    last: SharedString,
    _clock: Task<()>,
}

impl ReasoningStepsPanelStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self {
            looping: Run::new(Duration::from_millis(1300), reconciling().len()),
            folding: Run::new(Duration::from_millis(900), grouping().len()),
            last: "The second run has not finished yet".into(),
            _clock: cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(100))
                        .await;
                    if this
                        .update(cx, |this: &mut Self, cx| this.tick(cx))
                        .is_err()
                    {
                        break;
                    }
                }
            }),
        })
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        let before = (self.looping.shown, self.folding.shown);
        let mut changed = self.looping.advance(now);
        let rested = self.looping.took.is_some_and(|took| {
            now.saturating_duration_since(self.looping.started) >= took + RESTART_AFTER
        });
        if rested {
            self.looping = Run::new(self.looping.every, self.looping.steps);
            changed = true;
        }
        if self.folding.advance(now) {
            let took = self.folding.took.unwrap_or_default();
            self.last = format!("The second run took {} ms", took.as_millis()).into();
            changed = true;
        }
        if changed || before != (self.looping.shown, self.folding.shown) {
            cx.notify();
        }
    }
}

impl Story for ReasoningStepsPanelStory {
    fn title() -> &'static str {
        "Reasoning steps panel"
    }

    fn description() -> &'static str {
        "Shows what an assistant is doing while it works."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ReasoningStepsPanelStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_6()
            .child(self.looping.panel("looping", reconciling()))
            .child(
                self.folding
                    .panel("folding", grouping())
                    .working_label("Reconciling")
                    .done_label("Books reconciled")
                    .collapse_when_done(true),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.last.clone()),
            )
    }
}
