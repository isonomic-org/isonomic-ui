// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the isometric stacked bars.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::isometric_stacked_bars::{IsoBar, IsoRow, IsometricStackedBars};

/// One made-up week of harvest from a garden bed, in kilograms.
fn week(name: &str, rows: [[f64; 3]; 7]) -> IsoRow {
    let days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    IsoRow::new(
        name.to_string(),
        days.iter()
            .zip(rows)
            .map(|(day, values)| IsoBar::new(*day, values.to_vec()))
            .collect(),
    )
}

/// Two garden beds over a week, and a small two-row sample.
pub struct IsometricStackedBarsStory {
    picked: SharedString,
}

impl IsometricStackedBarsStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            picked: "Point at a bar".into(),
        })
    }
}

impl Story for IsometricStackedBarsStory {
    fn title() -> &'static str {
        "Isometric stacked bars"
    }

    fn description() -> &'static str {
        "Two rows of stacked bars drawn as boxes, with shaded faces, hover and keyboard picking, and a readout."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for IsometricStackedBarsStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let this = cx.entity();
        v_flex()
            .gap_6()
            .child(
                IsometricStackedBars::new(
                    "harvest",
                    "Harvest by bed",
                    ["Tomatoes", "Beans", "Squash"],
                )
                .subtitle("One week, back bed and front bed")
                .unit("kg")
                .row(week(
                    "Back",
                    [
                        [4., 2., 1.],
                        [5., 2.5, 1.5],
                        [3., 3., 2.],
                        [6., 2., 2.5],
                        [7., 3.5, 1.],
                        [5.5, 4., 3.],
                        [4., 2., 2.],
                    ],
                ))
                .row(week(
                    "Front",
                    [
                        [2., 1., 3.],
                        [3., 1.5, 2.],
                        [2.5, 2., 4.],
                        [4., 1., 3.5],
                        [3.5, 2.5, 2.],
                        [5., 3., 4.5],
                        [3., 1.5, 2.5],
                    ],
                ))
                .on_select(move |r, c, _, cx| {
                    this.update(cx, |this, cx| {
                        this.picked = format!("Row {}, column {}", r + 1, c + 1).into();
                        cx.notify();
                    })
                }),
            )
            .child(
                IsometricStackedBars::new("stock", "Shelf stock", ["Jars", "Tins"])
                    .cell(isonomic_ui::px(44.))
                    .rise(isonomic_ui::px(96.))
                    .row(IsoRow::new(
                        "Upper",
                        vec![
                            IsoBar::new("A", vec![12., 4.]),
                            IsoBar::new("B", vec![6., 9.]),
                            IsoBar::new("C", vec![3., 2.]),
                        ],
                    ))
                    .row(IsoRow::new(
                        "Lower",
                        vec![
                            IsoBar::new("A", vec![8., 8.]),
                            IsoBar::new("B", vec![14., 1.]),
                            IsoBar::new("C", vec![5., 6.]),
                        ],
                    )),
            )
            .child(div().text_sm().text_color(quiet).child(self.picked.clone()))
    }
}
