// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the chart card.

use crate::Story;
use isonomic_ui::component::{
    ActiveTheme as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div, px,
};
use isonomic_ui_extra::chart_card::{ChartCard, ChartKind, ChartSeries};

/// Made-up bakery sales, two seasons of six months each.
const SEASONS: [[[f64; 6]; 3]; 2] = [
    [
        [420., 460., 510., 480., 560., 610.],
        [210., 240., 230., 290., 310., 300.],
        [90., 120., 140., 130., 170., 210.],
    ],
    [
        [380., 520., 470., 600., 540., 690.],
        [260., 220., 300., 270., 350., 330.],
        [150., 110., 160., 190., 140., 240.],
    ],
];

/// Bakery sales by product, with a switch between two made-up seasons.
pub struct ChartCardStory {
    season: usize,
    last: SharedString,
}

impl ChartCardStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            season: 0,
            last: "Pick a chart type or a series".into(),
        })
    }
}

impl Story for ChartCardStory {
    fn title() -> &'static str {
        "Chart card"
    }

    fn description() -> &'static str {
        "Series as bars, lines, an area or a donut, with a type switch, legend toggles and values that glide when the data changes."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ChartCardStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let data = SEASONS[self.season];
        let names = ["Bread", "Pastry", "Coffee"];
        let months = if self.season == 0 {
            ["Jan", "Feb", "Mar", "Apr", "May", "Jun"]
        } else {
            ["Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]
        };
        let kind_view = cx.entity();
        let toggle_view = cx.entity();
        let mut sales = ChartCard::new("sales", "Sales by product", months)
            .subtitle(if self.season == 0 {
                "First half, in loaves and cups"
            } else {
                "Second half, in loaves and cups"
            })
            .on_kind_change(move |kind, _, cx| {
                kind_view.update(cx, |this, cx| {
                    this.last = format!("Showing the {}", kind.label().to_lowercase()).into();
                    cx.notify();
                })
            })
            .on_series_toggle(move |ix, shown, _, cx| {
                toggle_view.update(cx, |this, cx| {
                    this.last = format!(
                        "{} {}",
                        ["Bread", "Pastry", "Coffee"][ix],
                        if shown { "shown" } else { "hidden" }
                    )
                    .into();
                    cx.notify();
                })
            });
        for (name, values) in names.iter().zip(data) {
            sales = sales.series(ChartSeries::new(*name, values.to_vec()));
        }

        v_flex()
            .gap_6()
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("next-season")
                            .secondary()
                            .small()
                            .label("Change season")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.season = 1 - this.season;
                                cx.notify();
                            })),
                    )
                    .child(div().text_sm().text_color(quiet).child(self.last.clone())),
            )
            .child(sales)
            .child(
                ChartCard::new(
                    "visits",
                    "Visitors by door",
                    ["Mon", "Tue", "Wed", "Thu", "Fri"],
                )
                .subtitle("One week, front and side")
                .kind(ChartKind::Donut)
                .kinds([ChartKind::Donut, ChartKind::Line])
                .series(ChartSeries::new("Front", vec![120., 140., 90., 160., 210.]))
                .series(ChartSeries::new("Side", vec![40., 30., 55., 60., 80.]))
                .series(ChartSeries::new("Patio", vec![10., 12., 30., 28., 64.]))
                .hidden(2)
                .height(px(200.))
                .format(|people| format!("{people:.0}")),
            )
    }
}
