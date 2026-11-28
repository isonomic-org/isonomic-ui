// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Run schedule board: a week of hour slots where limited assistant sessions are placed by dragging, or by an auto mode.
//!
//! Days run across, hours down. A scheduled session is a block in its slot;
//! a session still to place waits in the tray above. Dragging a session onto
//! a slot schedules it there; dragging it back to the tray unschedules it.
//! Each day has a cap on hours of sessions, and a day past its cap turns
//! red in its header. The auto switch asks the owner to place everything.

use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, button::Button, h_flex, v_flex,
};
use isonomic_ui::{
    App, AppContext as _, Context, ElementId, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, RenderOnce, Role, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, px,
};

use crate::common::{Tone, resting_shadow};

/// One session to run.
#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledSession {
    /// Identifies it.
    pub id: SharedString,
    /// What it is.
    pub label: SharedString,
    /// How many hours it takes.
    pub hours: u32,
    /// Where it sits: day (0 is the first) and start hour, if placed.
    pub slot: Option<(usize, u32)>,
}

impl ScheduledSession {
    /// Create a session still to place.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>, hours: u32) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            hours: hours.max(1),
            slot: None,
        }
    }

    /// Place it.
    pub fn at(mut self, day: usize, hour: u32) -> Self {
        self.slot = Some((day, hour));
        self
    }
}

/// Hours booked on a day.
pub fn day_load(sessions: &[ScheduledSession], day: usize) -> u32 {
    sessions
        .iter()
        .filter(|s| s.slot.is_some_and(|(d, _)| d == day))
        .map(|s| s.hours)
        .sum()
}

/// Whether a session placed at `slot` would overlap another one.
pub fn clashes(sessions: &[ScheduledSession], id: &str, hours: u32, slot: (usize, u32)) -> bool {
    sessions.iter().any(|s| {
        s.id.as_ref() != id
            && s.slot
                .is_some_and(|(d, h)| d == slot.0 && h < slot.1 + hours && slot.1 < h + s.hours)
    })
}

#[derive(Clone)]
struct DraggedSession {
    id: SharedString,
    label: SharedString,
    fill: Hsla,
    ink: Hsla,
}

impl Render for DraggedSession {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded(px(6.))
            .bg(self.fill)
            .text_color(self.ink)
            .text_caption()
            .shadow_lg()
            .child(self.label.clone())
    }
}

type PlaceHandler = Rc<dyn Fn(&SharedString, Option<(usize, u32)>, &mut Window, &mut App)>;
type AutoHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A week of slots for limited sessions.
#[derive(IntoElement)]
pub struct RunScheduleBoard {
    id: ElementId,
    style: StyleRefinement,
    days: Vec<SharedString>,
    hours: (u32, u32),
    cap: u32,
    sessions: Vec<ScheduledSession>,
    on_place: Option<PlaceHandler>,
    on_auto: Option<AutoHandler>,
}

impl RunScheduleBoard {
    /// Create a board with day names, the first and last hour shown, and a daily cap in hours.
    pub fn new(
        id: impl Into<ElementId>,
        days: Vec<SharedString>,
        hours: (u32, u32),
        cap: u32,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            days,
            hours: (hours.0, hours.1.max(hours.0 + 1)),
            cap,
            sessions: Vec::new(),
            on_place: None,
            on_auto: None,
        }
    }

    /// Set the sessions.
    pub fn sessions(mut self, sessions: Vec<ScheduledSession>) -> Self {
        self.sessions = sessions;
        self
    }

    /// Run when a session is dropped on a slot, or back in the tray with `None`.
    pub fn on_place(
        mut self,
        handler: impl Fn(&SharedString, Option<(usize, u32)>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_place = Some(Rc::new(handler));
        self
    }

    /// Run when the auto button is clicked.
    pub fn on_auto(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_auto = Some(Rc::new(handler));
        self
    }
}

impl Styled for RunScheduleBoard {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

const ROW: f32 = 26.;

impl RenderOnce for RunScheduleBoard {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let accent = theme.primary;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;
        let danger = Tone::Danger.color(cx);
        let (first, last) = self.hours;
        let rows = last - first;
        let sessions = Rc::new(self.sessions.clone());

        let session_chip = |s: &ScheduledSession, tall: bool| {
            let dragged = DraggedSession {
                id: s.id.clone(),
                label: s.label.clone(),
                fill: accent,
                ink: fill,
            };
            div()
                .id(ElementId::NamedChild(
                    self.id.clone().into(),
                    format!("session-{}", s.id).into(),
                ))
                .px_1p5()
                .py_0p5()
                .rounded(px(5.))
                .bg(accent.opacity(0.18))
                .border_1()
                .border_color(accent.opacity(0.6))
                .cursor_grab()
                .overflow_hidden()
                .when(tall, |this| this.h(px(ROW * s.hours as f32 - 3.)))
                .role(Role::Group)
                .aria_label(format!("{}, {} hours", s.label, s.hours))
                .on_drag(dragged, |d, _, _, cx| cx.new(|_| d.clone()))
                .child(
                    div()
                        .text_caption()
                        .truncate()
                        .text_color(ink)
                        .child(s.label.clone()),
                )
                .when(tall && s.hours > 1, |this| {
                    this.child(
                        div()
                            .text_caption()
                            .text_color(quiet)
                            .child(format!("{}h", s.hours)),
                    )
                })
        };

        let waiting: Vec<_> = self.sessions.iter().filter(|s| s.slot.is_none()).collect();
        let tray = {
            let place = self.on_place.clone();
            h_flex()
                .id(ElementId::NamedChild(self.id.clone().into(), "tray".into()))
                .flex_wrap()
                .gap_1p5()
                .min_h(px(40.))
                .p_2()
                .rounded(radius)
                .border_1()
                .border_dashed()
                .border_color(line)
                .drag_over::<DraggedSession>(move |style, _, _, _| {
                    style.border_color(accent).bg(accent.opacity(0.05))
                })
                .on_drop(move |d: &DraggedSession, window, cx| {
                    if let Some(place) = &place {
                        place(&d.id, None, window, cx);
                    }
                })
                .when(waiting.is_empty(), |this| {
                    this.child(
                        div()
                            .text_caption()
                            .text_color(quiet)
                            .child("Everything is placed. Drag a session here to unschedule it."),
                    )
                })
                .children(waiting.iter().map(|s| {
                    session_chip(s, false).child(
                        div()
                            .text_caption()
                            .text_color(quiet)
                            .child(format!("{}h", s.hours)),
                    )
                }))
        };

        let columns: Vec<_> = self
            .days
            .iter()
            .enumerate()
            .map(|(day, name)| {
                let load = day_load(&self.sessions, day);
                let over = load > self.cap;
                let cells: Vec<_> = (first..last)
                    .map(|hour| {
                        let place = self.on_place.clone();
                        let check = sessions.clone();
                        let check_drop = sessions.clone();
                        let cap = self.cap;
                        div()
                            .id(ElementId::NamedChild(
                                self.id.clone().into(),
                                format!("slot-{day}-{hour}").into(),
                            ))
                            .h(px(ROW))
                            .border_t_1()
                            .border_color(line.opacity(0.6))
                            .drag_over::<DraggedSession>(move |style, d, _, _| {
                                let hours = check
                                    .iter()
                                    .find(|s| s.id == d.id)
                                    .map(|s| s.hours)
                                    .unwrap_or(1);
                                let already = check
                                    .iter()
                                    .find(|s| s.id == d.id)
                                    .and_then(|s| s.slot)
                                    .is_some_and(|(sd, _)| sd == day);
                                let load = day_load(&check, day) + if already { 0 } else { hours };
                                if clashes(&check, &d.id, hours, (day, hour)) || load > cap {
                                    style.bg(danger.opacity(0.12))
                                } else {
                                    style.bg(accent.opacity(0.12))
                                }
                            })
                            .on_drop(move |d: &DraggedSession, window, cx| {
                                let hours = check_drop
                                    .iter()
                                    .find(|s| s.id == d.id)
                                    .map(|s| s.hours)
                                    .unwrap_or(1);
                                if clashes(&check_drop, &d.id, hours, (day, hour)) {
                                    return;
                                }
                                if let Some(place) = &place {
                                    place(&d.id, Some((day, hour)), window, cx);
                                }
                            })
                    })
                    .collect();
                let blocks: Vec<_> = self
                    .sessions
                    .iter()
                    .filter_map(|s| s.slot.filter(|(d, _)| *d == day).map(|(_, h)| (s, h)))
                    .filter(|(_, h)| *h >= first && *h < last)
                    .map(|(s, h)| {
                        div()
                            .absolute()
                            .top(px(ROW * (h - first) as f32 + 1.))
                            .left(px(2.))
                            .right(px(2.))
                            .child(session_chip(s, true))
                    })
                    .collect();
                v_flex()
                    .flex_1()
                    .min_w(px(84.))
                    .border_l_1()
                    .border_color(line)
                    .child(
                        v_flex()
                            .h(px(38.))
                            .items_center()
                            .justify_center()
                            .child(div().text_caption().font_medium().child(name.clone()))
                            .child(
                                div()
                                    .text_caption()
                                    .text_color(if over { danger } else { quiet })
                                    .child(format!("{load} of {}h", self.cap)),
                            ),
                    )
                    .child(div().relative().children(cells).children(blocks))
            })
            .collect();

        let hour_labels: Vec<_> = (first..last)
            .map(|hour| {
                div()
                    .h(px(ROW))
                    .pr_2()
                    .text_caption()
                    .text_right()
                    .text_color(quiet)
                    .child(format!(
                        "{}{}",
                        if hour % 12 == 0 { 12 } else { hour % 12 },
                        if hour < 12 { "am" } else { "pm" }
                    ))
            })
            .collect();

        let placed: u32 = self
            .sessions
            .iter()
            .filter(|s| s.slot.is_some())
            .map(|s| s.hours)
            .sum();
        let total: u32 = self.sessions.iter().map(|s| s.hours).sum();
        let auto = self.on_auto.clone();

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(900.))
            .gap_3()
            .p_3()
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .gap_2()
                    .child(Icon::new(IconName::Calendar).small().text_color(quiet))
                    .child(
                        div()
                            .flex_1()
                            .font_medium()
                            .child(format!("{placed} of {total} hours placed")),
                    )
                    .child(
                        Button::new(ElementId::NamedChild(self.id.clone().into(), "auto".into()))
                            .small()
                            .outline()
                            .icon(IconName::Asterisk)
                            .label("Place for me")
                            .on_click(move |_, window, cx| {
                                if let Some(auto) = &auto {
                                    auto(window, cx);
                                }
                            }),
                    ),
            )
            .child(tray)
            .child(
                div()
                    .id(ElementId::NamedChild(
                        self.id.clone().into(),
                        "scroll".into(),
                    ))
                    .overflow_x_scroll()
                    .child(
                        h_flex()
                            .items_start()
                            .min_w(px(560.))
                            .child(
                                v_flex()
                                    .w(px(44.))
                                    .flex_none()
                                    .pt(px(38.))
                                    .children(hour_labels),
                            )
                            .children(columns),
                    ),
            )
            .child(div().text_caption().text_color(quiet).child(format!(
                "Each day takes at most {} hours of sessions across {rows} hours shown.",
                self.cap
            )))
            .refine_style(&self.style)
    }
}

/// Place every unplaced session in the first slot that fits, day by day,
/// keeping under the cap and clear of clashes. A plain auto mode for owners
/// that don't have their own.
pub fn auto_place(sessions: &mut [ScheduledSession], days: usize, hours: (u32, u32), cap: u32) {
    for ix in 0..sessions.len() {
        if sessions[ix].slot.is_some() {
            continue;
        }
        let need = sessions[ix].hours;
        let id = sessions[ix].id.clone();
        'search: for day in 0..days {
            if day_load(sessions, day) + need > cap {
                continue;
            }
            for hour in hours.0..=hours.1.saturating_sub(need) {
                if !clashes(sessions, &id, need, (day, hour)) {
                    sessions[ix].slot = Some((day, hour));
                    break 'search;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clashes_and_loads() {
        let sessions = vec![
            ScheduledSession::new("a", "A", 2).at(0, 9),
            ScheduledSession::new("b", "B", 1).at(0, 13),
            ScheduledSession::new("c", "C", 3),
        ];
        assert_eq!(day_load(&sessions, 0), 3);
        assert!(clashes(&sessions, "c", 3, (0, 8)));
        assert!(!clashes(&sessions, "c", 2, (0, 11)));
        assert!(!clashes(&sessions, "a", 2, (0, 10)));
    }

    #[test]
    fn auto_place_keeps_under_the_cap() {
        let mut sessions = vec![
            ScheduledSession::new("a", "A", 3).at(0, 9),
            ScheduledSession::new("b", "B", 2),
            ScheduledSession::new("c", "C", 2),
        ];
        auto_place(&mut sessions, 2, (9, 17), 4);
        assert_eq!(sessions[1].slot, Some((1, 9)));
        assert_eq!(sessions[2].slot, Some((1, 11)));
    }
}
