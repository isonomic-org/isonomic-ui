// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the stat grid.

use crate::Story;
use isonomic_ui::component::{
    ActiveTheme as _,
    button::{Button, ButtonVariants as _},
    v_flex,
};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div, px,
};
use isonomic_ui_extra::stat_grid::{Stat, StatGrid};

/// Two weeks at a made-up plant nursery.
pub struct StatGridStory {
    week: usize,
    picked: SharedString,
}

impl StatGridStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            week: 0,
            picked: "Pick a card".into(),
        })
    }

    fn stats(week: usize) -> Vec<Stat> {
        if week == 0 {
            vec![
                Stat::new("sales", "Plants sold", "1,204")
                    .delta(8.4)
                    .hint("vs last week")
                    .trend([820., 910., 870., 1010., 990., 1110., 1204.])
                    .goal(0.8, "Goal 1,500"),
                Stat::new("wait", "Wait at the till", "3.4 min")
                    .delta(-12.)
                    .lower_is_better()
                    .hint("vs last week")
                    .trend([4.6, 4.4, 4.1, 4.2, 3.9, 3.6, 3.4]),
                Stat::new("returns", "Returns", "38")
                    .delta(15.2)
                    .lower_is_better()
                    .hint("mostly ferns")
                    .trend([22., 25., 24., 30., 29., 33., 38.]),
                Stat::new("stock", "Seedlings in stock", "12.6k")
                    .delta(0.)
                    .hint("no change"),
            ]
        } else {
            vec![
                Stat::new("sales", "Plants sold", "1,531")
                    .delta(27.2)
                    .hint("vs last week")
                    .trend([910., 870., 1010., 990., 1110., 1204., 1531.])
                    .goal(1.02, "Goal 1,500"),
                Stat::new("wait", "Wait at the till", "4.1 min")
                    .delta(20.6)
                    .lower_is_better()
                    .hint("vs last week")
                    .trend([4.4, 4.1, 4.2, 3.9, 3.6, 3.4, 4.1]),
                Stat::new("returns", "Returns", "29")
                    .delta(-23.7)
                    .lower_is_better()
                    .hint("ferns restocked")
                    .trend([25., 24., 30., 29., 33., 38., 29.]),
                Stat::new("stock", "Seedlings in stock", "11.9k")
                    .delta(-5.6)
                    .hint("spring orders"),
            ]
        }
    }
}

impl Story for StatGridStory {
    fn title() -> &'static str {
        "Stat grid"
    }

    fn description() -> &'static str {
        "Headline figures side by side, each with a rolling value, its change, a trend line and progress toward a goal."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for StatGridStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let this = cx.entity();
        v_flex()
            .gap_6()
            .child(
                StatGrid::new("nursery", Self::stats(self.week))
                    .title("This week at the nursery")
                    .subtitle(if self.week == 0 { "Week 14" } else { "Week 15" })
                    .on_select(move |id, _, cx| {
                        this.update(cx, |this, cx| {
                            this.picked = format!("Picked: {id}").into();
                            cx.notify();
                        })
                    }),
            )
            .child(
                StatGrid::new("nursery-narrow", Self::stats(self.week)[..2].to_vec())
                    .columns(1)
                    .without_trends()
                    .w(px(328.)),
            )
            .child(
                Button::new("next-week")
                    .secondary()
                    .label("Other week")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.week = 1 - this.week;
                        cx.notify();
                    })),
            )
            .child(div().text_sm().text_color(quiet).child(self.picked.clone()))
    }
}
