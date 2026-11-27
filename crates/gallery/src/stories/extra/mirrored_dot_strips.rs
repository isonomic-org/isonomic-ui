// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the mirrored dot strips.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::mirrored_dot_strips::{MirroredDotStrips, StripMark, StripRow};

/// Sailings of a made-up ferry, every half hour from 06:00.
fn sailings() -> Vec<SharedString> {
    (0..34)
        .map(|ix| {
            format!(
                "{:02}:{}",
                6 + ix / 2,
                if ix % 2 == 0 { "00" } else { "30" }
            )
            .into()
        })
        .collect()
}

/// Made-up counts per sailing: busiest around two peaks, calmer each week.
fn counts(seed: u32, calm: u32, most: u32) -> Vec<u32> {
    let mut seed = seed;
    (0..34)
        .map(|ix: u32| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let noise = (seed >> 24) % 3;
            let peak = if ix.abs_diff(5) < 4 {
                4 - ix.abs_diff(5)
            } else if ix.abs_diff(23) < 5 {
                5 - ix.abs_diff(23)
            } else {
                0
            };
            ((peak + noise) * most / 7).saturating_sub(calm)
        })
        .collect()
}

/// Four weeks of a ferry's queue, and a busier chart where dots share.
pub struct MirroredDotStripsStory {
    picked: SharedString,
}

impl MirroredDotStripsStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            picked: "Point at a sailing".into(),
        })
    }
}

impl Story for MirroredDotStripsStory {
    fn title() -> &'static str {
        "Mirrored dot strips"
    }

    fn description() -> &'static str {
        "Two series mirrored round a line, one row per round, with pinned columns and a count per column."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for MirroredDotStripsStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let this = cx.entity();
        let names = sailings();
        v_flex()
            .gap_6()
            .child(
                MirroredDotStrips::new(
                    "ferry",
                    "Cars at the ramp",
                    sailings(),
                    "Boarded late",
                    "Left behind",
                )
                .subtitle("Harbour crossing, four weeks")
                .rows([
                    StripRow::new("Wk 31", counts(3, 0, 5), counts(11, 1, 5)),
                    StripRow::new("Wk 32", counts(5, 0, 5), counts(13, 1, 4)),
                    StripRow::new("Wk 33", counts(7, 1, 5), counts(17, 2, 4)),
                    StripRow::new("Wk 34", counts(9, 1, 4), counts(19, 2, 3))
                        .sublabel("this week")
                        .current(),
                ])
                .marks([
                    StripMark::new(5, "school run").note("School run, the 08:30 sailing"),
                    StripMark::new(23, "market day"),
                ])
                .on_select(move |column, _, cx| {
                    let name = names[column].clone();
                    this.update(cx, |this, cx| {
                        this.picked = format!("Picked the {name} sailing").into();
                        cx.notify();
                    })
                }),
            )
            .child(
                MirroredDotStrips::new(
                    "tills",
                    "Tills at the kiosk",
                    ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
                        .map(SharedString::from)
                        .to_vec(),
                    "Tickets sold",
                    "Refunds",
                )
                .rows([
                    StripRow::new(
                        "North",
                        [41, 38, 52, 47, 88, 120, 96],
                        [3, 2, 6, 4, 9, 14, 8],
                    ),
                    StripRow::new("South", [22, 25, 19, 30, 61, 74, 58], [1, 0, 2, 2, 5, 7, 3]),
                ])
                .unit("tickets"),
            )
            .child(div().text_sm().text_color(quiet).child(self.picked.clone()))
    }
}
