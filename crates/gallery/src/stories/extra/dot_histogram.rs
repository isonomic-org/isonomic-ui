// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the dot histogram.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::dot_histogram::DotHistogram;

/// Made-up delivery times in minutes: most near 24, a tail of slow ones.
fn delivery_minutes() -> Vec<f64> {
    let mut seed: u32 = 7;
    let mut next = move || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        f64::from(seed >> 8) / f64::from(1u32 << 24)
    };
    (0..420)
        .map(|ix| {
            let bell = (next() + next() + next() + next()) / 4.;
            let base = 12. + bell * 26.;
            if ix % 11 == 0 {
                base + 14. + next() * 22.
            } else {
                base
            }
        })
        .collect()
}

/// Delivery times against a 45-minute promise, and a small plain sample.
pub struct DotHistogramStory {
    picked: SharedString,
}

impl DotHistogramStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            picked: "Point at a column".into(),
        })
    }
}

impl Story for DotHistogramStory {
    fn title() -> &'static str {
        "Dot histogram"
    }

    fn description() -> &'static str {
        "A spread of samples as columns of dots, with the median, a limit, and the tail past it marked."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for DotHistogramStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let this = cx.entity();
        v_flex()
            .gap_6()
            .child(
                DotHistogram::new("deliveries", "Delivery time", delivery_minutes())
                    .subtitle("Last week, door to door")
                    .limit(45., "late")
                    .unit("orders")
                    .format(|minutes| format!("{minutes:.0} min"))
                    .on_select(move |bin, _, cx| {
                        this.update(cx, |this, cx| {
                            this.picked = format!(
                                "{:.0} to {:.0} min: {} orders",
                                bin.from, bin.to, bin.count
                            )
                            .into();
                            cx.notify();
                        })
                    }),
            )
            .child(
                DotHistogram::new(
                    "bake",
                    "Loaf weight",
                    vec![
                        0.78, 0.8, 0.8, 0.81, 0.82, 0.82, 0.82, 0.83, 0.84, 0.84, 0.85, 0.87,
                    ],
                )
                .columns(16)
                .unit("loaves")
                .format(|kg| format!("{kg:.2} kg")),
            )
            .child(div().text_sm().text_color(quiet).child(self.picked.clone()))
    }
}
