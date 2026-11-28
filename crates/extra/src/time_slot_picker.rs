// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Time slot picker: picks a start time from slots at a set step within working hours, greys out booked slots, and writes times the 12- or 24-hour way.
//!
//! The field shows the picked time, and the end time too when a length is
//! set, such as "9:30 – 10:00 am". Its menu lists every slot from the first
//! to the last and ticks the picked one; booked slots show but can't be
//! picked.
//!
//! Given days, it opens up into a panel instead: a strip of days with how
//! many slots are free on each, the free slots of the chosen day as a grid,
//! a time zone menu that rewrites every time in that zone, and a confirm
//! step that says the pick back in full before Confirm. Every part is a
//! button, so Tab reaches it and Enter or Space presses it.

use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Selectable as _, Sizable as _,
    StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    menu::{DropdownMenu as _, PopupMenuItem},
    v_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// A day to offer slots on.
#[derive(Debug, Clone, PartialEq)]
pub struct SlotDay {
    /// Identifies it.
    pub id: SharedString,
    /// Its weekday, short: "Tue".
    pub weekday: SharedString,
    /// Its date, short: "18".
    pub date: SharedString,
    /// The whole day in words, for the confirm step: "Tuesday 18 November".
    pub long: SharedString,
    /// Start times already taken.
    pub booked: Vec<u32>,
}

impl SlotDay {
    /// Create a day with nothing booked.
    pub fn new(
        id: impl Into<SharedString>,
        weekday: impl Into<SharedString>,
        date: impl Into<SharedString>,
        long: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            weekday: weekday.into(),
            date: date.into(),
            long: long.into(),
            booked: Vec::new(),
        }
    }

    /// Mark these start times as taken.
    pub fn booked(mut self, starts: impl IntoIterator<Item = u32>) -> Self {
        self.booked = starts.into_iter().collect();
        self
    }
}

/// A time zone to show the times in.
#[derive(Debug, Clone, PartialEq)]
pub struct SlotZone {
    /// Its name: "London".
    pub name: SharedString,
    /// Minutes ahead of the zone the slots are given in; negative behind.
    pub offset: i32,
}

impl SlotZone {
    /// Create a zone that many minutes ahead of the slots' own zone.
    pub fn new(name: impl Into<SharedString>, offset: i32) -> Self {
        Self {
            name: name.into(),
            offset,
        }
    }
}

/// The start times of a day that aren't booked.
pub fn free_slots(slots: &[u32], booked: &[u32]) -> Vec<u32> {
    slots
        .iter()
        .copied()
        .filter(|start| !booked.contains(start))
        .collect()
}

/// A time moved by a zone's offset: the time on that clock and how many
/// days it moved, -1, 0 or 1.
pub fn zone_shift(minutes: u32, offset: i32) -> (u32, i32) {
    let day = 24 * 60;
    let at = minutes as i32 + offset;
    (at.rem_euclid(day) as u32, at.div_euclid(day))
}

/// A slot written on a zone's clock, saying when it falls on another day:
/// "9:30 – 10:00 pm, the day before".
pub fn zoned_span(start: u32, length: Option<u32>, offset: i32, twelve_hour: bool) -> String {
    let (at, days) = zone_shift(start, offset);
    let mut words = match length {
        Some(len) => write_span(at, len, twelve_hour),
        None => write_time(at, twelve_hour),
    };
    match days {
        d if d > 0 => words.push_str(", the next day"),
        d if d < 0 => words.push_str(", the day before"),
        _ => {}
    }
    words
}

/// A zone's offset from the slots' own zone in words: "same time",
/// "+5:30", "-8".
pub fn offset_words(offset: i32) -> String {
    if offset == 0 {
        return "same time".into();
    }
    let sign = if offset > 0 { '+' } else { '-' };
    let (h, m) = (offset.abs() / 60, offset.abs() % 60);
    if m == 0 {
        format!("{sign}{h}h")
    } else {
        format!("{sign}{h}:{m:02}")
    }
}

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
type ConfirmHandler = Rc<dyn Fn(&SharedString, u32, &mut Window, &mut App)>;
type DayHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type ZoneHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

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
    days: Vec<SlotDay>,
    day: Option<SharedString>,
    zones: Vec<SlotZone>,
    zone: usize,
    confirming: bool,
    confirmed: bool,
    on_day: Option<DayHandler>,
    on_zone: Option<ZoneHandler>,
    on_confirm: Option<ConfirmHandler>,
    on_back: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
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
            days: Vec::new(),
            day: None,
            zones: Vec::new(),
            zone: 0,
            confirming: false,
            confirmed: false,
            on_day: None,
            on_zone: None,
            on_confirm: None,
            on_back: None,
        }
    }

    /// Open up into a panel over these days, each with its own bookings.
    pub fn days(mut self, days: Vec<SlotDay>) -> Self {
        self.days = days;
        self
    }

    /// The day showing, by id. The first day with a free slot otherwise.
    pub fn day(mut self, id: Option<SharedString>) -> Self {
        self.day = id;
        self
    }

    /// Run with a day's id when it is chosen in the strip.
    pub fn on_day(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_day = Some(Rc::new(handler));
        self
    }

    /// Offer these time zones in a menu; the first is the slots' own.
    pub fn zones(mut self, zones: Vec<SlotZone>) -> Self {
        self.zones = zones;
        self
    }

    /// The zone the times show in, by place in the list.
    pub fn zone(mut self, ix: usize) -> Self {
        self.zone = ix;
        self
    }

    /// Run with a zone's place in the list when it is picked.
    pub fn on_zone(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_zone = Some(Rc::new(handler));
        self
    }

    /// Show the confirm step for the picked slot.
    pub fn confirming(mut self, confirming: bool) -> Self {
        self.confirming = confirming;
        self
    }

    /// Say the pick is confirmed: the confirm step shows it done.
    pub fn confirmed(mut self, confirmed: bool) -> Self {
        self.confirmed = confirmed;
        self
    }

    /// Run with the day and start when Confirm is pressed.
    pub fn on_confirm(
        mut self,
        handler: impl Fn(&SharedString, u32, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_confirm = Some(Rc::new(handler));
        self
    }

    /// Run when the confirm step's Change is pressed, to go back to picking.
    pub fn on_back(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_back = Some(Rc::new(handler));
        self
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

impl TimeSlotPicker {
    fn render_panel(self, cx: &mut App) -> isonomic_ui::AnyElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let success = theme.success;
        let radius_lg = theme.radius_lg;
        let part = |name: String| ElementId::NamedChild(self.id.clone().into(), name.into());
        let slots = day_slots(self.first, self.last, self.step);
        let free_of = |day: &SlotDay| free_slots(&slots, &day.booked);
        let day = self
            .day
            .as_ref()
            .and_then(|id| self.days.iter().find(|d| &d.id == id))
            .or_else(|| self.days.iter().find(|d| !free_of(d).is_empty()))
            .or(self.days.first())
            .cloned();
        let zone = self.zones.get(self.zone).cloned();
        let offset = zone.as_ref().map_or(0, |z| z.offset);
        let (twelve, length) = (self.twelve_hour, self.length);
        let words = move |start: u32| zoned_span(start, length, offset, twelve);

        let zone_menu = (!self.zones.is_empty()).then(|| {
            let zones = self.zones.clone();
            let current = self.zone;
            let on_zone = self.on_zone.clone();
            Button::new(part("zone".into()))
                .ghost()
                .small()
                .icon(Icon::new(IconName::Globe))
                .label(zone.as_ref().map(|z| z.name.clone()).unwrap_or_default())
                .accessibility_label(format!(
                    "Time zone: {}",
                    zone.as_ref().map(|z| z.name.clone()).unwrap_or_default()
                ))
                .dropdown_menu(move |mut menu, _, _| {
                    for (ix, z) in zones.iter().enumerate() {
                        let on_zone = on_zone.clone();
                        menu = menu.item(
                            PopupMenuItem::new(format!(
                                "{}  \u{b7}  {}",
                                z.name,
                                offset_words(z.offset)
                            ))
                            .checked(ix == current)
                            .on_click(move |_, window, cx| {
                                if let Some(on_zone) = &on_zone {
                                    on_zone(ix, window, cx);
                                }
                            }),
                        );
                    }
                    menu.min_w(px(200.))
                })
        });

        let strip = h_flex().gap_1().children(self.days.iter().map(|d| {
            let free = free_of(d).len();
            let here = day.as_ref().is_some_and(|day| day.id == d.id);
            let on_day = self.on_day.clone();
            let id = d.id.clone();
            Button::new(part(format!("day-{}", d.id)))
                .ghost()
                .selected(here)
                .flex_1()
                .h(px(54.))
                .disabled(free == 0)
                .accessibility_label(format!(
                    "{}, {}{}",
                    d.long,
                    match free {
                        0 => "fully booked".to_string(),
                        1 => "1 free slot".to_string(),
                        n => format!("{n} free slots"),
                    },
                    if here { ", showing" } else { "" }
                ))
                .on_click(move |_, window, cx| {
                    if let Some(on_day) = &on_day {
                        on_day(&id, window, cx);
                    }
                })
                .child(
                    v_flex()
                        .items_center()
                        .child(
                            div()
                                .text_caption()
                                .text_color(quiet)
                                .child(d.weekday.clone()),
                        )
                        .child(div().text_title().child(d.date.clone()))
                        .child(
                            div()
                                .text_size(px(10.))
                                .line_height(px(12.))
                                .text_color(if free == 0 { quiet } else { success })
                                .child(if free == 0 {
                                    "full".to_string()
                                } else {
                                    format!("{free} free")
                                }),
                        ),
                )
        }));

        let body = match (&day, self.confirming, self.picked) {
            (Some(day), true, Some(start)) => {
                let on_back = self.on_back.clone();
                let on_confirm = self.on_confirm.clone();
                let day_id = day.id.clone();
                let summary = format!(
                    "{}, {}{}",
                    day.long,
                    words(start),
                    zone.as_ref()
                        .map(|z| format!(" ({})", z.name))
                        .unwrap_or_default()
                );
                v_flex()
                    .id(part("confirm".into()))
                    .gap_3()
                    .role(Role::Group)
                    .aria_label(if self.confirmed {
                        format!("Booked: {summary}")
                    } else {
                        format!("Confirm: {summary}")
                    })
                    .child(
                        h_flex()
                            .gap_2()
                            .items_start()
                            .child(
                                Icon::new(if self.confirmed {
                                    IconName::CircleCheck
                                } else {
                                    IconName::Calendar
                                })
                                .small()
                                .text_color(if self.confirmed { success } else { quiet }),
                            )
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .child(div().font_medium().child(if self.confirmed {
                                        "Booked"
                                    } else {
                                        "Check the time"
                                    }))
                                    .child(div().child(summary))
                                    .when_some(
                                        zone.as_ref().filter(|z| z.offset != 0),
                                        |this, _| {
                                            let own = self.zones.first().map(|z| z.name.clone());
                                            this.child(
                                                div().text_caption().text_color(quiet).child(
                                                    format!(
                                                        "{} in {}",
                                                        zoned_span(start, length, 0, twelve),
                                                        own.unwrap_or_else(
                                                            || "the host's zone".into()
                                                        )
                                                    ),
                                                ),
                                            )
                                        },
                                    ),
                            ),
                    )
                    .when(!self.confirmed, |this| {
                        this.child(
                            h_flex()
                                .gap_2()
                                .justify_end()
                                .child(
                                    Button::new(part("back".into()))
                                        .ghost()
                                        .small()
                                        .label("Change")
                                        .on_click(move |_, window, cx| {
                                            if let Some(on_back) = &on_back {
                                                on_back(window, cx);
                                            }
                                        }),
                                )
                                .child(
                                    Button::new(part("confirm-button".into()))
                                        .primary()
                                        .small()
                                        .label("Confirm")
                                        .on_click(move |_, window, cx| {
                                            if let Some(on_confirm) = &on_confirm {
                                                on_confirm(&day_id, start, window, cx);
                                            }
                                        }),
                                ),
                        )
                    })
                    .into_any_element()
            }
            (Some(day), _, _) => {
                let free = free_of(day);
                let taken = slots.len() - free.len();
                v_flex()
                    .gap_2()
                    .child(
                        h_flex()
                            .text_caption()
                            .text_color(quiet)
                            .child(div().flex_1().child(day.long.clone()))
                            .when(taken > 0, |this| this.child(format!("{taken} taken"))),
                    )
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_1p5()
                            .children(free.into_iter().map(|start| {
                                let pick = self.on_pick.clone();
                                Button::new(part(format!("slot-{}-{start}", day.id)))
                                    .outline()
                                    .small()
                                    .selected(self.picked == Some(start))
                                    .w(relative(0.31))
                                    .label(match length {
                                        Some(_) => zoned_span(start, None, offset, twelve),
                                        None => words(start),
                                    })
                                    .accessibility_label(format!(
                                        "{}, free{}",
                                        words(start),
                                        if self.picked == Some(start) {
                                            ", picked"
                                        } else {
                                            ""
                                        }
                                    ))
                                    .on_click(move |_, window, cx| {
                                        if let Some(pick) = &pick {
                                            pick(start, window, cx);
                                        }
                                    })
                            })),
                    )
                    .when(free_of(day).is_empty(), |this| {
                        this.child(
                            div()
                                .py_4()
                                .text_center()
                                .text_color(quiet)
                                .child("Nothing free on this day."),
                        )
                    })
                    .into_any_element()
            }
            (None, _, _) => div()
                .py_4()
                .text_color(quiet)
                .child("No days to pick from.")
                .into_any_element(),
        };

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(380.))
            .gap_3()
            .p_4()
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .shadow(resting_shadow(cx))
            .role(Role::Group)
            .aria_label(self.label.clone())
            .child(
                h_flex()
                    .gap_2()
                    .child(Icon::new(IconName::Time).small().text_color(quiet))
                    .child(div().flex_1().font_medium().child(self.label.clone()))
                    .when_some(length, |this, len| {
                        this.child(
                            div()
                                .text_caption()
                                .text_color(quiet)
                                .child(format!("{len} min")),
                        )
                    })
                    .children(zone_menu),
            )
            .when(!self.confirming, |this| this.child(strip))
            .child(div().h(px(1.)).bg(line))
            .child(body)
            .refine_style(&self.style)
            .into_any_element()
    }
}

impl RenderOnce for TimeSlotPicker {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        if !self.days.is_empty() {
            return self.render_panel(cx);
        }
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
            .into_any_element()
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

    #[test]
    fn free_slots_leave_out_the_booked() {
        let slots = day_slots(540, 660, 30);
        assert_eq!(free_slots(&slots, &[570, 630]), [540, 600, 660]);
        assert!(free_slots(&slots, &slots).is_empty());
    }

    #[test]
    fn zones_move_times_across_midnight() {
        assert_eq!(zone_shift(540, 330), (870, 0));
        assert_eq!(zone_shift(1380, 120), (60, 1));
        assert_eq!(zone_shift(60, -480), (1020, -1));
        assert_eq!(zoned_span(570, Some(30), 0, true), "9:30 \u{2013} 10:00 am");
        assert_eq!(
            zoned_span(570, Some(30), -660, true),
            "10:30 \u{2013} 11:00 pm, the day before"
        );
        assert_eq!(zoned_span(1380, None, 120, false), "01:00, the next day");
        assert_eq!(offset_words(0), "same time");
        assert_eq!(offset_words(330), "+5:30");
        assert_eq!(offset_words(-480), "-8h");
    }
}
