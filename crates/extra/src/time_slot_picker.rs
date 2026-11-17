// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Time slot picker: picks a start time from slots at a set step within working hours, greys out booked slots, and writes times the 12- or 24-hour way.
//!
//! The field shows the picked time, and the end time too when a length is
//! set, such as "9:30 – 10:00 am". Its menu lists every slot from the first
//! to the last and ticks the picked one; booked slots show but can't be
//! picked.

use std::rc::Rc;

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::Button,
    h_flex,
    menu::{DropdownMenu as _, PopupMenuItem},
};
use isonomic_ui::{
    App, ElementId, IntoElement, ParentElement as _, RenderOnce, SharedString, StyleRefinement,
    Styled, Window, div, px,
};

/// Start times from `first` up to but not past `last`, `step` minutes apart, all in minutes after midnight.
pub fn day_slots(first: u32, last: u32, step: u32) -> Vec<u32> {
    let step = step.max(1);
    (first..=last.min(24 * 60 - 1))
        .step_by(step as usize)
        .collect()
}

/// A time in minutes after midnight, written "9:30 am" or "09:30".
pub fn write_time(minutes: u32, twelve_hour: bool) -> String {
    let minutes = minutes % (24 * 60);
    let (h, m) = (minutes / 60, minutes % 60);
    if twelve_hour {
        let half = if h < 12 { "am" } else { "pm" };
        let h = match h % 12 {
            0 => 12,
            h => h,
        };
        format!("{h}:{m:02} {half}")
    } else {
        format!("{h:02}:{m:02}")
    }
}

/// A start and end, writing the am or pm once when both share it: "9:30 – 10:00 am".
pub fn write_span(start: u32, length: u32, twelve_hour: bool) -> String {
    let end = start + length;
    let (a, b) = (write_time(start, twelve_hour), write_time(end, twelve_hour));
    if twelve_hour && a.ends_with(&b[b.len() - 2..]) {
        format!("{} \u{2013} {b}", &a[..a.len() - 3])
    } else {
        format!("{a} \u{2013} {b}")
    }
}

type PickHandler = Rc<dyn Fn(u32, &mut Window, &mut App)>;

/// Picks a start time.
#[derive(IntoElement)]
pub struct TimeSlotPicker {
    id: ElementId,
    style: StyleRefinement,
    picked: Option<u32>,
    first: u32,
    last: u32,
    step: u32,
    length: Option<u32>,
    booked: Vec<u32>,
    twelve_hour: bool,
    label: SharedString,
    on_pick: Option<PickHandler>,
}

impl TimeSlotPicker {
    /// Create a picker over 9 am to 5 pm in half hours.
    pub fn new(id: impl Into<ElementId>, picked: Option<u32>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            picked,
            first: 9 * 60,
            last: 17 * 60,
            step: 30,
            length: None,
            booked: Vec::new(),
            twelve_hour: true,
            label: "Start time".into(),
            on_pick: None,
        }
    }

    /// Offer slots from `first` to `last`, `step` minutes apart, in minutes after midnight.
    pub fn hours(mut self, first: u32, last: u32, step: u32) -> Self {
        self.first = first;
        self.last = last.max(first);
        self.step = step.max(1);
        self
    }

    /// Show the end time for a slot this many minutes long.
    pub fn length(mut self, minutes: u32) -> Self {
        self.length = Some(minutes);
        self
    }

    /// Grey out these start times.
    pub fn booked(mut self, starts: impl IntoIterator<Item = u32>) -> Self {
        self.booked = starts.into_iter().collect();
        self
    }

    /// Write times the 24-hour way.
    pub fn twenty_four_hour(mut self) -> Self {
        self.twelve_hour = false;
        self
    }

    /// Name the field for screen readers.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    /// Run with the start time picked, in minutes after midnight.
    pub fn on_pick(mut self, handler: impl Fn(u32, &mut Window, &mut App) + 'static) -> Self {
        self.on_pick = Some(Rc::new(handler));
        self
    }
}

impl Styled for TimeSlotPicker {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for TimeSlotPicker {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let twelve = self.twelve_hour;
        let length = self.length;
        let write = move |start: u32| match length {
            Some(len) => write_span(start, len, twelve),
            None => write_time(start, twelve),
        };
        let shown = self
            .picked
            .map(write)
            .unwrap_or_else(|| "Pick a time".into());
        let slots = day_slots(self.first, self.last, self.step);
        let booked = self.booked.clone();
        let picked = self.picked;
        let pick = self.on_pick.clone();

        Button::new(self.id.clone())
            .outline()
            .refine_style(&self.style)
            .child(
                h_flex()
                    .gap_2()
                    .child(Icon::new(IconName::Time).xsmall().text_color(quiet))
                    .child(div().min_w(px(110.)).child(shown.clone())),
            )
            .accessibility_label(format!(
                "{}: {}",
                self.label,
                if picked.is_some() {
                    shown.as_str()
                } else {
                    "none picked"
                }
            ))
            .dropdown_menu(move |mut menu, _, _| {
                for start in &slots {
                    let start = *start;
                    let taken = booked.contains(&start);
                    let pick = pick.clone();
                    let text = if taken {
                        format!("{}  \u{b7}  booked", write(start))
                    } else {
                        write(start)
                    };
                    menu = menu.item(
                        PopupMenuItem::new(text)
                            .checked(picked == Some(start))
                            .disabled(taken)
                            .on_click(move |_, window, cx| {
                                if let Some(pick) = &pick {
                                    pick(start, window, cx);
                                }
                            }),
                    );
                }
                menu.min_w(px(200.)).max_h(px(280.)).scrollable(true)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_and_times_are_written_plainly() {
        assert_eq!(day_slots(540, 600, 30), [540, 570, 600]);
        assert_eq!(day_slots(1410, 1500, 30), [1410]);
        assert_eq!(write_time(0, true), "12:00 am");
        assert_eq!(write_time(570, true), "9:30 am");
        assert_eq!(write_time(780, true), "1:00 pm");
        assert_eq!(write_time(570, false), "09:30");
        assert_eq!(write_span(570, 30, true), "9:30 \u{2013} 10:00 am");
        assert_eq!(write_span(690, 60, true), "11:30 am \u{2013} 12:30 pm");
        assert_eq!(write_span(570, 30, false), "09:30 \u{2013} 10:00");
    }
}
