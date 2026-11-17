// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the time slot picker.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div, px,
};
use isonomic_ui_extra::time_slot_picker::{TimeSlotPicker, write_span};

/// Booking a 30-minute review call with a client, with some slots taken,
/// and the same day written the 24-hour way.
pub struct TimeSlotPickerStory {
    call: Option<u32>,
    payroll: Option<u32>,
}

impl TimeSlotPickerStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            call: None,
            payroll: Some(14 * 60),
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
    }
}
