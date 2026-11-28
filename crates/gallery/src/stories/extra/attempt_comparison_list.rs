// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the attempt comparison list.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::attempt_comparison_list::{Attempt, AttemptComparisonList, AttemptState};

/// Shows three ways of reconciling a bank account, one still running.
pub struct AttemptComparisonListStory {
    last: SharedString,
}

impl AttemptComparisonListStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            last: "The best one is in use".into(),
        })
    }
}

impl Story for AttemptComparisonListStory {
    fn title() -> &'static str {
        "Attempt comparison list"
    }

    fn description() -> &'static str {
        "The same task tried several ways at once."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for AttemptComparisonListStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let story = cx.entity();

        v_flex()
            .gap_3()
            .child(
                AttemptComparisonList::new("reconcile")
                    .task("Reconcile the March current account")
                    .attempt(
                        Attempt::new("strict", "Strict matcher")
                            .approach("Exact amount and date only")
                            .state(AttemptState::Done)
                            .score(71)
                            .note("212 of 240 lines matched, 28 left for review")
                            .took(Duration::from_secs(48))
                            .cost(0.09),
                    )
                    .attempt(
                        Attempt::new("fuzzy", "Fuzzy matcher")
                            .approach("Amount, then payee name within 5 days")
                            .state(AttemptState::Done)
                            .score(93)
                            .note("236 of 240 lines matched, 4 left for review")
                            .took(Duration::from_secs(126))
                            .cost(0.21),
                    )
                    .attempt(
                        Attempt::new("rules", "Rule learner")
                            .approach("Learns from last quarter's matches")
                            .progress(0.62)
                            .step("Scoring 150 of 240 lines")
                            .took(Duration::from_secs(95))
                            .cost(0.14),
                    )
                    .on_pick(move |id, _, cx| {
                        let line = format!("Now using {id}");
                        story.update(cx, |this, cx| {
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
