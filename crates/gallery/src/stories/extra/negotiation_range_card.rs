// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the negotiation range card.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::negotiation_range_card::{NegotiationRangeCard, Side, compact};

/// Made-up salary offers for a role, bunched near the middle with a long top.
fn salaries() -> Vec<f64> {
    let mut seed: u32 = 11;
    let mut next = move || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        f64::from(seed >> 8) / f64::from(1u32 << 24)
    };
    (0..64)
        .map(|ix| {
            let bell = (next() + next() + next()) / 3.;
            let base = 92_000. + bell * 48_000.;
            let value = if ix % 9 == 0 { base + 18_000. } else { base };
            (value / 500.).round() * 500.
        })
        .collect()
}

/// Made-up quotes from suppliers for one fit-out job.
fn quotes() -> Vec<f64> {
    vec![
        41_000., 43_500., 44_000., 45_000., 45_500., 46_000., 46_500., 47_000., 47_000., 48_000.,
        49_000., 49_500., 51_000., 52_000., 55_000., 58_000.,
    ]
}

/// A salary offer on the selling side and a supplier quote on the buying
/// side.
pub struct NegotiationRangeCardStory {
    picked: SharedString,
}

impl NegotiationRangeCardStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            picked: "Point at a band".into(),
        })
    }
}

impl Story for NegotiationRangeCardStory {
    fn title() -> &'static str {
        "Negotiation range card"
    }

    fn description() -> &'static str {
        "The market as columns of dots, with walk-away, target and ask marked, the offer on the table, and where it sits in words."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for NegotiationRangeCardStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let this = cx.entity();
        let money = |value: f64| format!("${}", compact(value));
        v_flex()
            .gap_6()
            .child(
                NegotiationRangeCard::new("salary", "Senior designer offer", salaries(), 118_000.)
                    .subtitle("Comparable offers in the last six months")
                    .marks(105_000., 125_000., 140_000.)
                    .band(2_500.)
                    .format(money)
                    .on_select(move |from, to, count, _, cx| {
                        this.update(cx, |this, cx| {
                            this.picked =
                                format!("{} to {}: {count} offers", money(from), money(to)).into();
                            cx.notify();
                        })
                    }),
            )
            .child(
                NegotiationRangeCard::new("fit-out", "Office fit-out quote", quotes(), 44_500.)
                    .subtitle("You are buying: lower is better")
                    .side(Side::Buying)
                    .marks(52_000., 46_000., 42_000.)
                    .band(1_000.)
                    .unit("quotes")
                    .format(money),
            )
            .child(div().text_sm().text_color(quiet).child(self.picked.clone()))
    }
}
