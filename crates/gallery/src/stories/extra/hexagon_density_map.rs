// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the hexagon density map.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div, px,
};
use isonomic_ui_extra::hexagon_density_map::HexagonDensityMap;

/// Made-up courier trips: distance in km against minutes taken, most near a
/// steady pace, a cloud of slow ones and a few quick ones.
fn trips() -> Vec<(f64, f64)> {
    let mut seed: u32 = 11;
    let mut next = move || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        f64::from(seed >> 8) / f64::from(1u32 << 24)
    };
    (0..900)
        .map(|ix| {
            let km = 1. + (next() + next() + next()) / 3. * 11.;
            let pace = if ix % 7 == 0 { 5.2 } else { 3.6 };
            let noise = (next() + next() - 1.) * 9.;
            (km, (km * pace + 4. + noise).max(2.))
        })
        .collect()
}

/// Trips against a promised time, and a small sample on a flat line.
pub struct HexagonDensityMapStory {
    picked: SharedString,
}

impl HexagonDensityMapStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            picked: "Point at a hexagon".into(),
        })
    }
}

impl Story for HexagonDensityMapStory {
    fn title() -> &'static str {
        "Hexagon density map"
    }

    fn description() -> &'static str {
        "Points binned into hexagons, shaded by how many fall in each and coloured by which side of a threshold line they are on."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for HexagonDensityMapStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let this = cx.entity();
        let small: Vec<(f64, f64)> = (0..60)
            .map(|ix| {
                let t = ix as f64;
                (t % 12., 20. + ((t * 1.7).sin() * 8.) + (t % 5.))
            })
            .collect();
        v_flex()
            .gap_6()
            .child(
                HexagonDensityMap::new("trips", "Courier trips", trips())
                    .subtitle("Time taken against distance, last month")
                    .axes("Distance (km)", "Minutes")
                    .threshold(4.2, 6., "Promised time")
                    .sides("Late", "On time")
                    .format(|value| format!("{value:.0}"))
                    .on_select(move |cell, _, cx| {
                        this.update(cx, |this, cx| {
                            this.picked = format!("{} trips in this hexagon", cell.count).into();
                            cx.notify();
                        })
                    }),
            )
            .child(
                HexagonDensityMap::new("ovens", "Oven readings", small)
                    .subtitle("Hours into the shift against temperature")
                    .axes("Hour", "°C over set")
                    .threshold(0., 24., "Alarm level")
                    .sides("Too hot", "Fine")
                    .radius(px(14.))
                    .height(px(180.))
                    .max_w(px(420.)),
            )
            .child(div().text_sm().text_color(quiet).child(self.picked.clone()))
    }
}
