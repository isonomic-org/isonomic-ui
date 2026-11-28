// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the time slot picker.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div, px,
};
use isonomic_ui_extra::time_slot_picker::{SlotDay, SlotZone, TimeSlotPicker, write_span};

/// Booking a 30-minute review call with a client, with some slots taken,
/// and the same day written the 24-hour way.
///
/// Under them, the panel: a week of days with what's free on each, a time
/// zone menu for a client abroad, and the confirm step once a slot is
/// picked.
pub struct TimeSlotPickerStory {
    call: Option<u32>,
    payroll: Option<u32>,
    day: Option<SharedString>,
    zone: usize,
    slot: Option<u32>,
    confirming: bool,
    confirmed: bool,
}

fn week() -> Vec<SlotDay> {
    vec![
        SlotDay::new("mon", "Mon", "17", "Monday 17 November").booked([540, 570, 600, 630, 660]),
        SlotDay::new("tue", "Tue", "18", "Tuesday 18 November").booked([540, 630, 900]),
        SlotDay::new("wed", "Wed", "19", "Wednesday 19 November").booked(day_all()),
        SlotDay::new("thu", "Thu", "20", "Thursday 20 November").booked([600, 840, 870]),
        SlotDay::new("fri", "Fri", "21", "Friday 21 November").booked([780, 810, 840, 870, 900]),
    ]
}

/// Every slot of the day, for a day that's fully booked.
fn day_all() -> Vec<u32> {
    (9 * 60..=16 * 60).step_by(30).collect()
}

impl TimeSlotPickerStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            call: None,
            payroll: Some(14 * 60),
            day: Some("tue".into()),
            zone: 0,
            slot: Some(10 * 60 + 30),
            confirming: false,
            confirmed: false,
        })
    }
}

impl Story for TimeSlotPickerStory {
    fn title() -> &'static str {
        "Time slot picker"
    }

    fn description() -> &'static str {
        "Picks a start time from slots at a set step within working hours, greying out booked slots."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for TimeSlotPickerStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let call = cx.entity();
        let payroll = cx.entity();
        let row = |label: &'static str| div().w(px(170.)).text_sm().child(label);
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .gap_3()
                    .child(row("Review call, Tue 18 Nov"))
                    .child(
                        TimeSlotPicker::new("call", self.call)
                            .length(30)
                            .booked([9 * 60, 10 * 60 + 30, 11 * 60, 15 * 60])
                            .label("Review call start")
                            .on_pick(move |start, _, cx| {
                                call.update(cx, |this, cx| {
                                    this.call = Some(start);
                                    cx.notify();
                                })
                            }),
                    ),
            )
            .child(
                h_flex().gap_3().child(row("Payroll run cut-off")).child(
                    TimeSlotPicker::new("payroll", self.payroll)
                        .hours(6 * 60, 22 * 60, 60)
                        .twenty_four_hour()
                        .label("Payroll cut-off")
                        .on_pick(move |start, _, cx| {
                            payroll.update(cx, |this, cx| {
                                this.payroll = Some(start);
                                cx.notify();
                            })
                        }),
                ),
            )
            .child(div().text_xs().text_color(quiet).child(match self.call {
                Some(start) => format!("Call booked for {}.", write_span(start, 30, true)),
                None => "No call booked yet.".into(),
            }))
            .child(div().h(px(12.)))
            .child({
                let (day, zone, pick, confirm, back) = (
                    cx.entity(),
                    cx.entity(),
                    cx.entity(),
                    cx.entity(),
                    cx.entity(),
                );
                TimeSlotPicker::new("week", self.slot)
                    .label("Book a review call")
                    .hours(9 * 60, 16 * 60, 30)
                    .length(30)
                    .days(week())
                    .day(self.day.clone())
                    .zones(vec![
                        SlotZone::new("London", 0),
                        SlotZone::new("New York", -300),
                        SlotZone::new("Mumbai", 330),
                        SlotZone::new("Sydney", 660),
                    ])
                    .zone(self.zone)
                    .confirming(self.confirming)
                    .confirmed(self.confirmed)
                    .on_day(move |id, _, cx| {
                        day.update(cx, |this, cx| {
                            this.day = Some(id.clone());
                            this.slot = None;
                            cx.notify();
                        })
                    })
                    .on_zone(move |ix, _, cx| {
                        zone.update(cx, |this, cx| {
                            this.zone = ix;
                            cx.notify();
                        })
                    })
                    .on_pick(move |start, _, cx| {
                        pick.update(cx, |this, cx| {
                            this.slot = Some(start);
                            this.confirming = true;
                            this.confirmed = false;
                            cx.notify();
                        })
                    })
                    .on_confirm(move |_, _, _, cx| {
                        confirm.update(cx, |this, cx| {
                            this.confirmed = true;
                            cx.notify();
                        })
                    })
                    .on_back(move |_, cx| {
                        back.update(cx, |this, cx| {
                            this.confirming = false;
                            this.confirmed = false;
                            cx.notify();
                        })
                    })
            })
    }
}
