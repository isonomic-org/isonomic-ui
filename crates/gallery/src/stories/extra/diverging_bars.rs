// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the diverging bars.

use crate::Story;
use isonomic_ui::component::{
    ActiveTheme as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    rolling_number::group_thousands,
    v_flex,
};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::diverging_bars::{BarSeries, DivergingBars};

const DAYS: usize = 21;

/// Made-up loaves per day for one kind of bread: more at weekends.
fn loaves(seed: u32, base: f64, month: usize) -> Vec<f64> {
    let mut seed = seed.wrapping_add(month as u32 * 97);
    (0..DAYS)
        .map(|day| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let noise = f64::from(seed >> 24) / 255.;
            let weekend = if day % 7 >= 5 { 1.5 } else { 1. };
            (base * weekend * (0.75 + noise * 0.5)).round()
        })
        .collect()
}

fn days(month: &str) -> Vec<SharedString> {
    (1..=DAYS)
        .map(|day| format!("{month} {day}").into())
        .collect()
}

/// Three weeks of a made-up bakery, with unsold loaves below the line.
pub struct DivergingBarsStory {
    month: usize,
    picked: SharedString,
}

impl DivergingBarsStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            month: 0,
            picked: "Point at a day".into(),
        })
    }
}

impl Story for DivergingBarsStory {
    fn title() -> &'static str {
        "Diverging bars"
    }

    fn description() -> &'static str {
        "Stacked columns with one series below the zero line on its own scale, and a legend that switches series off."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for DivergingBarsStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let this = cx.entity();
        let month = self.month;
        let name = ["Mar", "Apr"][month];
        let wheat = loaves(3, 120., month);
        let rye = loaves(8, 60., month);
        let rolls = loaves(21, 210., month);
        let unsold: Vec<f64> = loaves(34, 9., month);
        let baked: f64 = wheat.iter().chain(&rye).chain(&rolls).sum();
        let left: f64 = unsold.iter().sum();
        v_flex()
            .gap_6()
            .child(
                DivergingBars::new("bakery", "Baked and unsold", days(name))
                    .subtitle("Corner bakery, three weeks")
                    .series([
                        BarSeries::new("Wheat", wheat),
                        BarSeries::new("Rye", rye),
                        BarSeries::new("Rolls", rolls),
                    ])
                    .below(BarSeries::new("Unsold", unsold))
                    .headline(group_thousands(baked as u64))
                    .caption("baked")
                    .aside(format!("{} unsold", group_thousands(left as u64)))
                    .on_select({
                        let this = this.clone();
                        move |day, _, cx| {
                            this.update(cx, |this, cx| {
                                this.picked = format!("Picked {name} {}", day + 1).into();
                                cx.notify();
                            })
                        }
                    }),
            )
            .child(
                h_flex()
                    .gap_3()
                    .child(
                        Button::new("other-month")
                            .secondary()
                            .small()
                            .label("Other month")
                            .on_click(move |_, _, cx| {
                                this.update(cx, |this, cx| {
                                    this.month = 1 - this.month;
                                    cx.notify();
                                })
                            }),
                    )
                    .child(div().text_sm().text_color(quiet).child(self.picked.clone())),
            )
            .child(
                DivergingBars::new(
                    "ovens",
                    "Oven hours",
                    ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
                        .map(SharedString::from)
                        .to_vec(),
                )
                .series([
                    BarSeries::new("Deck oven", [6.5, 7., 6., 7.5, 9., 11.]),
                    BarSeries::new("Rack oven", [3., 3.5, 2.5, 4., 5.5, 6.]),
                ])
                .format(|hours| format!("{hours:.1} h")),
            )
    }
}
