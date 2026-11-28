// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the renewal clock.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::renewal_clock::{CalendarDate, Contract, RenewalClock};

fn day(year: i32, month: u32, day: u32) -> CalendarDate {
    CalendarDate::new(year, month, day)
}

/// Made-up contracts for a small studio, one in each state.
fn contracts() -> Vec<Contract> {
    vec![
        Contract::new(
            "office",
            "Office lease",
            day(2024, 12, 15),
            day(2026, 12, 15),
        )
        .notice_days(60)
        .note("$4,200 a month"),
        Contract::new(
            "insurance",
            "Studio insurance",
            day(2026, 1, 20),
            day(2027, 1, 20),
        )
        .notice_days(30)
        .note("$1,860 a year"),
        Contract::new("phones", "Phone plan", day(2025, 4, 1), day(2027, 4, 1))
            .notice_days(45)
            .note("$310 a month"),
        Contract::new(
            "cleaning",
            "Cleaning service",
            day(2026, 7, 1),
            day(2027, 7, 1),
        )
        .notice_days(30)
        .note("$540 a month"),
        Contract::new(
            "storage",
            "Archive storage",
            day(2025, 10, 10),
            day(2027, 10, 10),
        )
        .notice_days(90)
        .note("$95 a month"),
    ]
}

/// A studio's contracts on the clock, read on 28 November 2026.
pub struct RenewalClockStory {
    picked: SharedString,
}

impl RenewalClockStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            picked: "Pick a contract".into(),
        })
    }
}

impl Story for RenewalClockStory {
    fn title() -> &'static str {
        "Renewal clock"
    }

    fn description() -> &'static str {
        "Twelve months as a dial with one ring per contract: the term, the notice window before renewal, and today."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for RenewalClockStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let this = cx.entity();
        v_flex()
            .gap_6()
            .child(
                RenewalClock::new(
                    "studio-renewals",
                    "Contract renewals",
                    day(2026, 11, 28),
                    contracts(),
                )
                .subtitle("Five contracts, next twelve months")
                .on_select(move |id, _, cx| {
                    this.update(cx, |this, cx| {
                        this.picked = format!("Picked {id}").into();
                        cx.notify();
                    })
                }),
            )
            .child(div().text_sm().text_color(quiet).child(self.picked.clone()))
    }
}
