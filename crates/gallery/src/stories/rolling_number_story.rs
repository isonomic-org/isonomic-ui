// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

use isonomic_ui::{
    App, AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, px,
};

use isonomic_ui::component::{
    ActiveTheme as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    rolling_number::RollingNumber,
    v_flex,
};

use crate::section;

/// Figures from a made-up print shop, stepped through a few days of orders.
const DAYS: [(u64, &str, &str); 4] = [
    (1_284, "87.2k", "64%"),
    (1_309, "88.9k", "71%"),
    (998, "90.4k", "98%"),
    (12_045, "104k", "100%"),
];

pub struct RollingNumberStory {
    day: usize,
}

impl super::Story for RollingNumberStory {
    fn title() -> &'static str {
        "Rolling number"
    }

    fn description() -> &'static str {
        "A figure whose digits roll to their new values while units and separators stay put."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl RollingNumberStory {
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self { day: 0 })
    }
}

impl Render for RollingNumberStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (orders, sheets, on_time) = DAYS[self.day];
        let quiet = cx.theme().muted_foreground;
        let figure = |label: &'static str, number: RollingNumber| {
            v_flex()
                .gap_1()
                .child(
                    number
                        .text_size(px(24.))
                        .line_height(px(32.))
                        .font_semibold(),
                )
                .child(isonomic_ui::div().text_xs().text_color(quiet).child(label))
        };

        v_flex()
            .w_full()
            .items_center()
            .gap_6()
            .child(
                section("Figures")
                    .description("Press the button: the last digit turns first, and the unit stays where it is.")
                    .w(px(560.))
                    .child(
                        v_flex()
                            .w_full()
                            .items_center()
                            .gap_6()
                            .child(
                                h_flex()
                                    .gap_12()
                                    .child(figure(
                                        "Orders today",
                                        RollingNumber::grouped("orders", orders),
                                    ))
                                    .child(figure(
                                        "Sheets printed",
                                        RollingNumber::new("sheets", sheets),
                                    ))
                                    .child(figure(
                                        "Sent on time",
                                        RollingNumber::new("on-time", on_time),
                                    )),
                            )
                            .child(
                                Button::new("next-day")
                                    .secondary()
                                    .label("Next day")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.day = (this.day + 1) % DAYS.len();
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(
                section("In a line of text")
                    .description("At the body size the wheel is one 20px line tall.")
                    .w(px(560.))
                    .child(
                        h_flex()
                            .w_full()
                            .justify_center()
                            .gap_1()
                            .text_sm()
                            .child("Queue length")
                            .child(RollingNumber::grouped("queue", orders / 7).font_medium()),
                    ),
            )
    }
}
