// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the tick gauge.

use crate::Story;
use isonomic_ui::component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    rolling_number::group_thousands,
    tag::Tag,
    v_flex,
};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window,
};
use isonomic_ui_extra::tick_gauge::TickGauge;

/// Weeks of a made-up parcel depot: share sent on time, late parcels, total.
const WEEKS: [(f32, u32, u32); 3] = [(0.931, 412, 5_980), (0.968, 201, 6_240), (0.994, 35, 6_105)];

/// A depot's on-time rate against its goal, and a plain level with no goal.
pub struct TickGaugeStory {
    week: usize,
}

impl TickGaugeStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self { week: 0 })
    }
}

impl Story for TickGaugeStory {
    fn title() -> &'static str {
        "Tick gauge"
    }

    fn description() -> &'static str {
        "A level as ticks around an arc, with a target marker and the shortfall to it."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for TickGaugeStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (rate, late, total) = WEEKS[self.week];
        let met = rate >= 0.99;
        v_flex()
            .gap_6()
            .child(
                h_flex()
                    .gap_6()
                    .items_start()
                    .child(
                        TickGauge::new("on-time", "Sent on time", rate)
                            .subtitle(format!("Week {}", self.week + 31))
                            .caption(format!("{late} of {} late", group_thousands(u64::from(total))))
                            .target(0.99, "Goal 99%")
                            .aside(if met {
                                Tag::success().child("On goal")
                            } else {
                                Tag::danger().child("Below goal")
                            }),
                    )
                    .child(
                        TickGauge::new("van-load", "Van 7 load", 0.62)
                            .figure("62%")
                            .ticks(28),
                    ),
            )
            .child(
                Button::new("next-week")
                    .secondary()
                    .label("Next week")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.week = (this.week + 1) % WEEKS.len();
                        cx.notify();
                    })),
            )
    }
}
