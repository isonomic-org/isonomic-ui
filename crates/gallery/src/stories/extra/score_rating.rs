// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the score rating.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div, px,
};
use isonomic_ui_extra::score_rating::ScoreRating;

/// Rating a bookkeeper after a month-end close, and read-only scores for three firms.
pub struct ScoreRatingStory {
    given: f32,
    half: f32,
}

impl ScoreRatingStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            given: 0.0,
            half: 3.5,
        })
    }
}

impl Story for ScoreRatingStory {
    fn title() -> &'static str {
        "Score rating"
    }

    fn description() -> &'static str {
        "A row of stars for giving or showing a score, in whole or half steps, with an optional count of ratings."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ScoreRatingStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let whole = cx.entity();
        let half = cx.entity();
        let row = |label: &'static str| div().w(px(220.)).text_sm().child(label);
        v_flex()
            .gap_4()
            .child(
                h_flex()
                    .gap_3()
                    .child(row("How did the March close go?"))
                    .child(
                        ScoreRating::new("close-score", self.given)
                            .large()
                            .on_change(move |score, _, cx| {
                                whole.update(cx, |this, cx| {
                                    this.given = score;
                                    cx.notify();
                                })
                            }),
                    ),
            )
            .child(
                h_flex().gap_3().child(row("Half steps")).child(
                    ScoreRating::new("half-score", self.half)
                        .half_steps(true)
                        .on_change(move |score, _, cx| {
                            half.update(cx, |this, cx| {
                                this.half = score;
                                cx.notify();
                            })
                        }),
                ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(quiet)
                    .child("Read-only, with counts"),
            )
            .child(
                h_flex()
                    .gap_3()
                    .child(row("Ledgerline Partners"))
                    .child(ScoreRating::new("firm-a", 4.6).half_steps(true).count(212)),
            )
            .child(
                h_flex()
                    .gap_3()
                    .child(row("Tally & Rowe"))
                    .child(ScoreRating::new("firm-b", 3.9).half_steps(true).count(48)),
            )
            .child(
                h_flex()
                    .gap_3()
                    .child(row("Northgate Books"))
                    .child(ScoreRating::new("firm-c", 0.0).count(0)),
            )
    }
}
