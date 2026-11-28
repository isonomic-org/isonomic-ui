// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Renewal clock: the next twelve months as a dial, one ring per contract.
//!
//! The dial starts at the top with the current month and runs clockwise. On
//! each ring an arc covers the contract's term, a highlighted stretch marks
//! its notice window (the days before renewal when it is too late to give
//! notice) and a dot marks the renewal date. A needle marks today. Pick a
//! contract on the dial, in the list beside it, or with the arrow keys to read
//! its dates and how many days are left.

use std::{cell::Cell, f32::consts::TAU, rc::Rc};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::{Presence, TestSupportExt as _, Transition};
use isonomic_ui::component::{
    ActiveTheme as _, Icon, Sizable as _, StyledExt as _, TextRole as _, h_flex, look, tag::Tag,
    v_flex,
};
use isonomic_ui::{
    App, Bounds, ElementId, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, ParentElement as _, PathBuilder, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, canvas, div, point,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::{panel, shell, shell_header};

/// The dial is laid out on a 340 box (300 plus room for month labels) and
/// scaled to its width.
const BOX: f32 = 340.;
const PAD: f32 = 20.;
const CENTRE: f32 = 150.;
/// The outermost ring's radius and the room the rings may take inward.
const OUTER: f32 = 134.;
const DEPTH: f32 = 72.;
/// A notice deadline this close reads as a warning.
pub const SOON_DAYS: i64 = 30;

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// A day on the calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CalendarDate {
    /// Year, such as 2026.
    pub year: i32,
    /// Month, 1 to 12.
    pub month: u32,
    /// Day of the month, 1 to 31.
    pub day: u32,
}

impl CalendarDate {
    /// A date. The month and day are kept in range.
    pub fn new(year: i32, month: u32, day: u32) -> Self {
        Self {
            year,
            month: month.clamp(1, 12),
            day: day.clamp(1, 31),
        }
    }

    /// Days since 1 January 1970.
    pub fn days(self) -> i64 {
        let year = i64::from(self.year) - i64::from(self.month <= 2);
        let era = year.div_euclid(400);
        let of_era = year - era * 400;
        let month = i64::from(self.month);
        let of_year =
            (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + i64::from(self.day) - 1;
        let of_cycle = of_era * 365 + of_era / 4 - of_era / 100 + of_year;
        era * 146_097 + of_cycle - 719_468
    }

    /// The date `days` after 1 January 1970.
    pub fn from_days(days: i64) -> Self {
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let of_era = z - era * 146_097;
        let year_of_era = (of_era - of_era / 1460 + of_era / 36_524 - of_era / 146_096) / 365;
        let of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let mp = (5 * of_year + 2) / 153;
        let day = (of_year - (153 * mp + 2) / 5 + 1) as u32;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let year = (year_of_era + era * 400 + i64::from(month <= 2)) as i32;
        Self { year, month, day }
    }

    /// The date `days` later (or earlier, when negative).
    pub fn add_days(self, days: i64) -> Self {
        Self::from_days(self.days() + days)
    }

    /// The first day of the month `months` after this date's month.
    pub fn month_start(self, months: i32) -> Self {
        let index = self.year * 12 + self.month as i32 - 1 + months;
        Self::new(index.div_euclid(12), index.rem_euclid(12) as u32 + 1, 1)
    }

    /// Written out, such as "14 Mar 2027".
    pub fn label(self) -> String {
        format!(
            "{} {} {}",
            self.day,
            MONTHS[self.month as usize - 1],
            self.year
        )
    }

    /// Written without the year, such as "14 Mar".
    pub fn short(self) -> String {
        format!("{} {}", self.day, MONTHS[self.month as usize - 1])
    }
}

/// One contract on the clock.
#[derive(Debug, Clone, PartialEq)]
pub struct Contract {
    /// Stable id, reported back when picked.
    pub id: SharedString,
    /// What the reader calls it.
    pub name: SharedString,
    /// When the current term began.
    pub start: CalendarDate,
    /// When it renews.
    pub renews: CalendarDate,
    /// How many days' notice it needs before renewal.
    pub notice_days: i64,
    /// A short extra line, such as its price.
    pub note: Option<SharedString>,
}

impl Contract {
    /// A contract with its current term. Thirty days' notice by default.
    pub fn new(
        id: impl Into<SharedString>,
        name: impl Into<SharedString>,
        start: CalendarDate,
        renews: CalendarDate,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            start,
            renews,
            notice_days: 30,
            note: None,
        }
    }

    /// How many days' notice it needs.
    pub fn notice_days(mut self, days: i64) -> Self {
        self.notice_days = days.max(0);
        self
    }

    /// A short extra line, such as its price.
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// The last day to give notice.
    pub fn deadline(&self) -> CalendarDate {
        self.renews.add_days(-self.notice_days)
    }
}

/// Where a contract stands today.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenewalStatus {
    /// Notice can still be given, with this many days left.
    Open(i64),
    /// Notice can still be given, but the deadline is near.
    Soon(i64),
    /// Too late to give notice; it renews in this many days.
    Locked(i64),
    /// The renewal date has passed.
    Renewed,
}

impl RenewalStatus {
    /// A short pill label.
    pub fn pill(self) -> String {
        match self {
            RenewalStatus::Open(days) | RenewalStatus::Soon(days) => {
                format!("Notice in {}", day_count(days))
            }
            RenewalStatus::Locked(days) => format!("Renews in {}", day_count(days)),
            RenewalStatus::Renewed => "Renewed".into(),
        }
    }
}

/// Where a contract stands on `today`.
pub fn status(contract: &Contract, today: CalendarDate) -> RenewalStatus {
    let to_deadline = contract.deadline().days() - today.days();
    let to_renewal = contract.renews.days() - today.days();
    if to_renewal <= 0 {
        RenewalStatus::Renewed
    } else if to_deadline < 0 {
        RenewalStatus::Locked(to_renewal)
    } else if to_deadline <= SOON_DAYS {
        RenewalStatus::Soon(to_deadline)
    } else {
        RenewalStatus::Open(to_deadline)
    }
}

/// "1 day", "12 days".
pub fn day_count(days: i64) -> String {
    if days.abs() == 1 {
        format!("{days} day")
    } else {
        format!("{days} days")
    }
}

/// How far round the dial a date sits: 0 at the window's start, 1 at its end.
pub fn turn_of(date: CalendarDate, from: CalendarDate, to: CalendarDate) -> f32 {
    let span = (to.days() - from.days()).max(1) as f32;
    (date.days() - from.days()) as f32 / span
}

/// The part of `from`..`to` that falls inside the window, as turns.
pub fn clip_arc(
    from: CalendarDate,
    to: CalendarDate,
    window: (CalendarDate, CalendarDate),
) -> Option<(f32, f32)> {
    let start = turn_of(from, window.0, window.1).max(0.);
    let end = turn_of(to, window.0, window.1).min(1.);
    (end > start).then_some((start, end))
}

/// The radius of ring `ix` of `count`, and the width of its arcs.
pub fn ring_radius(ix: usize, count: usize) -> (f32, f32) {
    let gap = (DEPTH / count.max(1) as f32).min(18.);
    (OUTER - gap * ix as f32, (gap * 0.55).clamp(3., 9.))
}

/// The ring under a distance from the middle of the dial, if any.
pub fn ring_at(distance: f32, count: usize) -> Option<usize> {
    (0..count).find(|ix| {
        let (radius, _) = ring_radius(*ix, count);
        let gap = (DEPTH / count.max(1) as f32).min(18.);
        (distance - radius).abs() <= gap / 2.
    })
}

/// A point `turn` of the way round (0 at the top, clockwise), `radius` from
/// the middle of the 300 dial.
pub fn dial_point(turn: f32, radius: f32) -> (f32, f32) {
    let angle = turn * TAU - TAU / 4.;
    (CENTRE + radius * angle.cos(), CENTRE + radius * angle.sin())
}

/// A contract's dates and days left, in words.
pub fn spoken(contract: &Contract, today: CalendarDate) -> String {
    let to_renewal = contract.renews.days() - today.days();
    let notice = match status(contract, today) {
        RenewalStatus::Open(days) | RenewalStatus::Soon(days) => format!(
            "give notice by {}, {} left",
            contract.deadline().label(),
            day_count(days)
        ),
        RenewalStatus::Locked(_) => {
            format!("notice deadline {} has passed", contract.deadline().label())
        }
        RenewalStatus::Renewed => "already renewed".into(),
    };
    format!(
        "{}: renews {}, in {}; {notice}",
        contract.name,
        contract.renews.label(),
        day_count(to_renewal.max(0))
    )
}

type SelectHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// Twelve months as a dial with one ring per contract.
#[derive(IntoElement)]
pub struct RenewalClock {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    subtitle: Option<SharedString>,
    today: CalendarDate,
    contracts: Vec<Contract>,
    on_select: Option<SelectHandler>,
}

impl RenewalClock {
    /// Create a clock for the twelve months from the start of `today`'s
    /// month. Contracts are drawn outermost first, in the order given.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        today: CalendarDate,
        contracts: Vec<Contract>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            subtitle: None,
            today,
            contracts,
            on_select: None,
        }
    }

    /// A quieter line under the title.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Run with a contract's id when it is picked.
    pub fn on_select(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Styled for RenewalClock {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

fn status_tag(status: RenewalStatus) -> Tag {
    match status {
        RenewalStatus::Open(_) => Tag::success(),
        RenewalStatus::Soon(_) => Tag::warning(),
        RenewalStatus::Locked(_) => Tag::danger(),
        RenewalStatus::Renewed => Tag::secondary(),
    }
}

impl RenderOnce for RenewalClock {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let part = |name: String| ElementId::NamedChild(self.id.clone().into(), name.into());
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let data = theme.chart_1;
        let warning = theme.warning;
        let danger = theme.danger;
        let ring_color = theme.ring;
        let border = theme.border;
        let mono = theme.mono_font_family.clone();
        let wash = look::ink::wash_strong(cx);
        let count = self.contracts.len();
        let today = self.today;
        let window_range = (today.month_start(0), today.month_start(12));

        let active = window.use_keyed_state(part("active".into()), cx, |_, _| None::<usize>);
        let picked = (*active.read(cx)).filter(|ix| *ix < count);
        let dial_bounds = window
            .use_keyed_state(part("bounds".into()), cx, |_, _| {
                Rc::new(Cell::new(Bounds::default()))
            })
            .read(cx)
            .clone();
        let focus = window
            .use_keyed_state(part("focus".into()), cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            })
            .read(cx)
            .clone();
        let focused = focus.is_focused(window);
        let still = cx.reduce_motion();
        let drawn = if still {
            1.
        } else {
            Presence::new(part("appear".into()), true)
                .transition(Transition::new(std::time::Duration::from_millis(600)))
                .sample(window, cx)
                .progress
        };

        // What to draw on each ring: term, notice window and renewal date.
        struct Ring {
            term: Option<(f32, f32)>,
            notice: Option<(f32, f32)>,
            renews: Option<f32>,
            notice_color: Hsla,
        }
        let rings: Vec<Ring> = self
            .contracts
            .iter()
            .map(|contract| {
                let renews = turn_of(contract.renews, window_range.0, window_range.1);
                Ring {
                    term: clip_arc(contract.start, contract.renews, window_range),
                    notice: clip_arc(contract.deadline(), contract.renews, window_range),
                    renews: (0. ..=1.).contains(&renews).then_some(renews),
                    notice_color: match status(contract, today) {
                        RenewalStatus::Locked(_) => danger,
                        _ => warning,
                    },
                }
            })
            .collect();
        let today_turn = turn_of(today, window_range.0, window_range.1);

        let seen = dial_bounds.clone();
        let dial = canvas(
            move |bounds, _, _| seen.set(bounds),
            move |bounds, _, window, _| {
                let scale = f32::from(bounds.size.width) / BOX;
                let at = |(x, y): (f32, f32)| {
                    point(
                        bounds.left() + px((PAD + x) * scale),
                        bounds.top() + px((PAD + y) * scale),
                    )
                };
                let arc = |window: &mut Window,
                           from: f32,
                           to: f32,
                           radius: f32,
                           wide: f32,
                           color: Hsla| {
                    let steps = ((to - from) * 180.).ceil().max(1.) as usize;
                    let mut path = PathBuilder::stroke(px(wide * scale));
                    path.move_to(at(dial_point(from, radius)));
                    for step in 1..=steps {
                        let turn = from + (to - from) * step as f32 / steps as f32;
                        path.line_to(at(dial_point(turn, radius)));
                    }
                    if let Ok(path) = path.build() {
                        window.paint_path(path, color);
                    }
                };
                let dot = |window: &mut Window, centre: (f32, f32), r: f32, color: Hsla| {
                    let ring: Vec<_> = (0..14)
                        .map(|step| {
                            let angle = step as f32 / 14. * TAU;
                            at((centre.0 + r * angle.cos(), centre.1 + r * angle.sin()))
                        })
                        .collect();
                    let mut path = PathBuilder::fill();
                    path.add_polygon(&ring, true);
                    if let Ok(path) = path.build() {
                        window.paint_path(path, color);
                    }
                };

                // Month ticks across the rings.
                for month in 0..12 {
                    let turn = month as f32 / 12.;
                    let mut tick = PathBuilder::stroke(px(1. * scale));
                    tick.move_to(at(dial_point(turn, OUTER - DEPTH - 8.)));
                    tick.line_to(at(dial_point(turn, OUTER + 10.)));
                    if let Ok(tick) = tick.build() {
                        window.paint_path(tick, ink.opacity(if month == 0 { 0.3 } else { 0.1 }));
                    }
                }

                for (ix, ring) in rings.iter().enumerate() {
                    let (radius, wide) = ring_radius(ix, count);
                    let dim = if picked.is_some_and(|at| at != ix) {
                        0.4
                    } else {
                        1.
                    };
                    arc(window, 0., 1., radius, wide, ink.opacity(0.06));
                    let reach = drawn;
                    if let Some((from, to)) = ring.term {
                        let to = from + (to - from) * reach;
                        let term = if picked == Some(ix) {
                            data
                        } else {
                            ink.opacity(0.3)
                        };
                        arc(window, from, to, radius, wide, term.opacity(term.a * dim));
                    }
                    if let Some((from, to)) = ring.notice {
                        arc(
                            window,
                            from,
                            to,
                            radius,
                            wide,
                            ring.notice_color.opacity(dim * reach),
                        );
                    }
                    if let Some(turn) = ring.renews {
                        let spot = dial_point(turn, radius);
                        dot(window, spot, wide * 0.5 + 2.5, ink.opacity(dim * reach));
                    }
                }

                // Today's needle.
                let mut needle = PathBuilder::stroke(px(2. * scale));
                needle.move_to(at(dial_point(today_turn, OUTER - DEPTH - 10.)));
                needle.line_to(at(dial_point(today_turn, OUTER + 12.)));
                if let Ok(needle) = needle.build() {
                    window.paint_path(needle, data);
                }
                dot(window, dial_point(today_turn, OUTER + 12.), 3.5, data);
            },
        )
        .w_full()
        .aspect_ratio(1.);

        let month_labels = (0..12).map(|month| {
            let date = window_range.0.month_start(month);
            let (x, y) = dial_point((month as f32 + 0.5) / 12., OUTER + 16.);
            div()
                .absolute()
                .left(relative((PAD + x) / BOX))
                .top(relative((PAD + y) / BOX))
                .w(px(0.))
                .h(px(0.))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .flex_none()
                        .whitespace_nowrap()
                        .font_family(mono.clone())
                        .text_size(px(10.))
                        .text_color(if month == 0 { ink } else { quiet })
                        .child(MONTHS[date.month as usize - 1]),
                )
        });

        let soonest = (0..count)
            .filter(|ix| self.contracts[*ix].renews > today)
            .min_by_key(|ix| self.contracts[*ix].renews);
        let middle = v_flex()
            .absolute()
            .left(relative(0.3))
            .right(relative(0.3))
            .top_0()
            .bottom_0()
            .items_center()
            .justify_center()
            .map(|this| {
                let shown = picked.or(soonest);
                match shown {
                    Some(ix) => {
                        let contract = &self.contracts[ix];
                        let days = (contract.renews.days() - today.days()).max(0);
                        this.child(div().text_caption().text_color(quiet).child(
                            if picked.is_some() {
                                "Renews in"
                            } else {
                                "Next renewal"
                            },
                        ))
                        .child(
                            div()
                                .text_size(px(24.))
                                .line_height(px(30.))
                                .font_medium()
                                .child(day_count(days)),
                        )
                        .child(
                            div()
                                .w_full()
                                .text_center()
                                .truncate()
                                .text_caption()
                                .text_color(quiet)
                                .child(contract.name.clone()),
                        )
                    }
                    None => this.child(
                        div()
                            .text_caption()
                            .text_color(quiet)
                            .child("No renewals ahead"),
                    ),
                }
            });

        let spoken_all = match picked {
            Some(ix) => spoken(&self.contracts[ix], today),
            None => format!(
                "{}: {} contracts, today {}. Arrow keys pick a contract.",
                self.title,
                count,
                today.label()
            ),
        };

        let pick = |state: &isonomic_ui::Entity<Option<usize>>,
                    next: Option<usize>,
                    contracts: &[Contract],
                    select: &Option<SelectHandler>,
                    window: &mut Window,
                    cx: &mut App| {
            state.update(cx, |state, cx| {
                *state = next;
                cx.notify();
            });
            if let (Some(select), Some(ix)) = (select, next) {
                select(&contracts[ix].id, window, cx);
            }
        };
        let pick = Rc::new(pick);

        let key_state = active.clone();
        let key_select = self.on_select.clone();
        let key_contracts = Rc::new(self.contracts.clone());
        let row_contracts = key_contracts.clone();
        let key_pick = pick.clone();
        let click_state = active.clone();
        let click_select = self.on_select.clone();
        let click_contracts = key_contracts.clone();
        let click_pick = pick.clone();
        let click_focus = focus.clone();

        let dial_box = div()
            .id(part("dial".into()))
            .test_support()
            .track_focus(&focus)
            .relative()
            .flex_none()
            .w(px(300.))
            .max_w_full()
            .role(Role::Group)
            .aria_label(spoken_all)
            .on_mouse_down(
                MouseButton::Left,
                move |event: &MouseDownEvent, window, cx| {
                    window.focus(&click_focus, cx);
                    let bounds = dial_bounds.get();
                    let scale = f32::from(bounds.size.width) / BOX;
                    if scale <= 0. {
                        return;
                    }
                    let x = f32::from(event.position.x - bounds.left()) / scale - PAD - CENTRE;
                    let y = f32::from(event.position.y - bounds.top()) / scale - PAD - CENTRE;
                    let next = ring_at((x * x + y * y).sqrt(), count);
                    if next.is_some() {
                        click_pick(
                            &click_state,
                            next,
                            &click_contracts,
                            &click_select,
                            window,
                            cx,
                        );
                    }
                },
            )
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                if count == 0 {
                    return;
                }
                let now = *key_state.read(cx);
                let next = match event.keystroke.key.as_str() {
                    "down" | "right" => Some(now.map_or(0, |at| (at + 1) % count)),
                    "up" | "left" => Some(now.map_or(count - 1, |at| (at + count - 1) % count)),
                    "home" => Some(0),
                    "end" => Some(count - 1),
                    "escape" => None,
                    _ => return,
                };
                key_pick(&key_state, next, &key_contracts, &key_select, window, cx);
                cx.stop_propagation();
            })
            .child(dial)
            .children(month_labels)
            .child(middle)
            .when(focused, |this| {
                this.child(
                    div()
                        .absolute()
                        .inset_0()
                        .rounded_full()
                        .border_2()
                        .border_color(ring_color),
                )
            });

        let hovered_state =
            window.use_keyed_state(part("hovered".into()), cx, |_, _| None::<usize>);
        let hovered = *hovered_state.read(cx);
        let fades: Vec<f32> = (0..count)
            .map(|ix| {
                look::hover_fade(
                    part(format!("row-fade-{ix}")),
                    hovered == Some(ix) || picked == Some(ix),
                    window,
                    cx,
                )
            })
            .collect();
        let rows = self.contracts.iter().enumerate().map(|(ix, contract)| {
            let hover_state = hovered_state.clone();
            let now = status(contract, today);
            let state = active.clone();
            let select = self.on_select.clone();
            let contracts = row_contracts.clone();
            let pick = pick.clone();
            h_flex()
                .id(part(format!("row-{ix}")))
                .test_support()
                .gap_2()
                .px_2()
                .py(px(6.))
                .rounded(px(8.))
                .cursor_pointer()
                .role(Role::Button)
                .aria_label(spoken(contract, today))
                .tab_index(0)
                .bg(wash.opacity(wash.a * fades[ix]))
                .on_hover(move |over, _, cx| {
                    let next = over.then_some(ix);
                    if *over || *hover_state.read(cx) == Some(ix) {
                        hover_state.update(cx, |state, cx| {
                            *state = next;
                            cx.notify();
                        });
                    }
                })
                .focus_visible(move |style| style.border_2().border_color(ring_color))
                .on_click(move |_, window, cx| {
                    let next = if *state.read(cx) == Some(ix) {
                        None
                    } else {
                        Some(ix)
                    };
                    pick(&state, next, &contracts, &select, window, cx)
                })
                .child(
                    div()
                        .flex_none()
                        .w(px(18.))
                        .font_family(mono.clone())
                        .text_caption()
                        .text_color(quiet)
                        .child(format!("{}", ix + 1)),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(div().truncate().text_label().child(contract.name.clone()))
                        .child(
                            div()
                                .text_caption()
                                .text_color(quiet)
                                .child(format!("Renews {}", contract.renews.short())),
                        ),
                )
                .child(status_tag(now).small().rounded_full().child(now.pill()))
        });

        let detail = picked.map(|ix| {
            let contract = &self.contracts[ix];
            let now = status(contract, today);
            let to_renewal = (contract.renews.days() - today.days()).max(0);
            let fact = |label: &'static str, value: String| {
                v_flex()
                    .min_w(px(132.))
                    .flex_1()
                    .child(div().text_caption().text_color(quiet).child(label))
                    .child(div().text_label_small().child(value))
            };
            v_flex()
                .id(part("detail".into()))
                .mt_3()
                .pt_3()
                .gap_2()
                .border_t_1()
                .border_color(border)
                .role(Role::Status)
                .aria_label(spoken(contract, today))
                .child(
                    h_flex()
                        .gap_2()
                        .child(div().text_label().child(contract.name.clone()))
                        .when_some(contract.note.clone(), |this, note| {
                            this.child(div().text_caption().text_color(quiet).child(note))
                        }),
                )
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap_3()
                        .child(fact(
                            "Term",
                            format!("{} – {}", contract.start.label(), contract.renews.label()),
                        ))
                        .child(fact(
                            "Give notice by",
                            format!(
                                "{} ({} notice)",
                                contract.deadline().label(),
                                day_count(contract.notice_days)
                            ),
                        ))
                        .child(fact(
                            "Days left",
                            match now {
                                RenewalStatus::Open(days) | RenewalStatus::Soon(days) => format!(
                                    "{} to give notice, {} to renewal",
                                    days,
                                    day_count(to_renewal)
                                ),
                                RenewalStatus::Locked(_) => {
                                    format!("Notice closed, {} to renewal", day_count(to_renewal))
                                }
                                RenewalStatus::Renewed => "Renewed".into(),
                            },
                        )),
                )
        });

        let legend = h_flex()
            .mt_3()
            .flex_wrap()
            .gap_x_4()
            .gap_y_1()
            .text_caption()
            .text_color(quiet)
            .child(
                h_flex()
                    .gap(px(6.))
                    .child(
                        div()
                            .w(px(12.))
                            .h(px(4.))
                            .rounded_full()
                            .bg(ink.opacity(0.3)),
                    )
                    .child("Term"),
            )
            .child(
                h_flex()
                    .gap(px(6.))
                    .child(div().w(px(12.)).h(px(4.)).rounded_full().bg(warning))
                    .child("Notice window"),
            )
            .child(
                h_flex()
                    .gap(px(6.))
                    .child(div().size(px(8.)).rounded_full().bg(ink))
                    .child("Renewal"),
            )
            .child(
                h_flex()
                    .gap(px(6.))
                    .child(div().w(px(2.)).h(px(12.)).bg(data))
                    .child(format!("Today, {}", today.short())),
            );

        shell(cx)
            .id(self.id.clone())
            .max_w(px(672.))
            .child(shell_header(
                Some(Icon::new(CatalogIcon::Renew)),
                self.title.clone(),
                self.subtitle.clone().map(|line| line.into_any_element()),
                Some(
                    div()
                        .font_family(mono.clone())
                        .text_caption()
                        .text_color(quiet)
                        .child(format!(
                            "{} – {}",
                            window_range.0.short(),
                            window_range.1.add_days(-1).short()
                        ))
                        .into_any_element(),
                ),
                cx,
            ))
            .child(
                panel(cx)
                    .p_4()
                    .child(
                        h_flex()
                            .flex_wrap()
                            .items_center()
                            .gap_4()
                            .child(dial_box)
                            .child(v_flex().flex_1().min_w(px(220.)).gap_1().children(rows)),
                    )
                    .child(legend)
                    .children(detail),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_count_days_both_ways() {
        assert_eq!(CalendarDate::new(1970, 1, 1).days(), 0);
        assert_eq!(CalendarDate::new(2000, 3, 1).days(), 11_017);
        assert_eq!(CalendarDate::new(1969, 12, 31).days(), -1);
        for days in [-800, -1, 0, 59, 11_016, 20_513, 21_000] {
            assert_eq!(CalendarDate::from_days(days).days(), days);
        }
        assert_eq!(
            CalendarDate::new(2026, 12, 20).add_days(14),
            CalendarDate::new(2027, 1, 3)
        );
        // 2028 is a leap year.
        assert_eq!(
            CalendarDate::new(2028, 2, 28).add_days(1),
            CalendarDate::new(2028, 2, 29)
        );
    }

    #[test]
    fn months_step_across_years() {
        let today = CalendarDate::new(2026, 11, 28);
        assert_eq!(today.month_start(0), CalendarDate::new(2026, 11, 1));
        assert_eq!(today.month_start(2), CalendarDate::new(2027, 1, 1));
        assert_eq!(today.month_start(12), CalendarDate::new(2027, 11, 1));
        assert_eq!(today.month_start(-11), CalendarDate::new(2025, 12, 1));
        assert_eq!(CalendarDate::new(2027, 3, 14).label(), "14 Mar 2027");
        assert_eq!(CalendarDate::new(2027, 3, 14).short(), "14 Mar");
    }

    #[test]
    fn arcs_are_clipped_to_the_window() {
        let window = (CalendarDate::new(2026, 1, 1), CalendarDate::new(2027, 1, 1));
        assert_eq!(turn_of(window.0, window.0, window.1), 0.);
        assert_eq!(turn_of(window.1, window.0, window.1), 1.);
        let (from, to) = clip_arc(
            CalendarDate::new(2025, 6, 1),
            CalendarDate::new(2026, 7, 2),
            window,
        )
        .unwrap();
        assert_eq!(from, 0.);
        assert!((to - 0.5).abs() < 0.01);
        assert_eq!(
            clip_arc(
                CalendarDate::new(2024, 1, 1),
                CalendarDate::new(2025, 1, 1),
                window
            ),
            None
        );
    }

    #[test]
    fn status_follows_the_notice_deadline() {
        let contract = Contract::new(
            "a",
            "A",
            CalendarDate::new(2026, 3, 1),
            CalendarDate::new(2027, 3, 1),
        )
        .notice_days(30);
        assert_eq!(contract.deadline(), CalendarDate::new(2027, 1, 30));
        let on = |y, m, d| status(&contract, CalendarDate::new(y, m, d));
        assert_eq!(on(2026, 11, 28), RenewalStatus::Open(63));
        assert_eq!(on(2027, 1, 10), RenewalStatus::Soon(20));
        assert_eq!(on(2027, 2, 10), RenewalStatus::Locked(19));
        assert_eq!(on(2027, 3, 1), RenewalStatus::Renewed);
        assert_eq!(RenewalStatus::Soon(1).pill(), "Notice in 1 day");
        assert_eq!(RenewalStatus::Locked(19).pill(), "Renews in 19 days");
        assert_eq!(
            spoken(&contract, CalendarDate::new(2026, 11, 28)),
            "A: renews 1 Mar 2027, in 93 days; give notice by 30 Jan 2027, 63 days left"
        );
    }

    #[test]
    fn rings_step_inward_and_are_found_by_distance() {
        assert_eq!(ring_radius(0, 4), (OUTER, 9.));
        assert_eq!(ring_radius(1, 4).0, OUTER - 18.);
        assert_eq!(ring_at(OUTER + 3., 4), Some(0));
        assert_eq!(ring_at(OUTER - 18., 4), Some(1));
        assert_eq!(ring_at(10., 4), None);
        let (x, y) = dial_point(0.25, 100.);
        assert!((x - 250.).abs() < 0.01 && (y - 150.).abs() < 0.01);
    }
}
