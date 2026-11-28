// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Due date picker: a month calendar with one-click presets for the dates people actually pick, and days that can't be picked greyed out.
//!
//! Presets sit above the grid: today, in a week, in 30 days, end of month.
//! Days before the earliest allowed date, and weekends when they're off,
//! can't be picked. Arrow keys move a day or a week, Page Up and Down move a
//! month, and Enter picks. Today is ringed; the picked day is filled.

use isonomic_ui::base::PressableExt as _;
use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, IconName, Sizable as _, StyledExt as _, ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    RenderOnce, Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled,
    Window, div, prelude::FluentBuilder as _, px,
};

use crate::common::resting_shadow;

/// A calendar day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day {
    /// The year.
    pub year: i32,
    /// The month, 1 to 12.
    pub month: u32,
    /// The day of the month, from 1.
    pub day: u32,
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

impl Day {
    /// Create a day; out-of-range parts are pulled into range.
    pub fn new(year: i32, month: u32, day: u32) -> Self {
        let month = month.clamp(1, 12);
        Self {
            year,
            month,
            day: day.clamp(1, days_in_month(year, month)),
        }
    }

    /// Days since 1 January 1970, negative before.
    pub fn number(self) -> i64 {
        let (y, m) = if self.month <= 2 {
            (self.year as i64 - 1, self.month as i64 + 9)
        } else {
            (self.year as i64, self.month as i64 - 3)
        };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let doy = (153 * m + 2) / 5 + self.day as i64 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// The day for a day number.
    pub fn from_number(n: i64) -> Self {
        let z = n + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let year = (yoe + era * 400 + i64::from(month <= 2)) as i32;
        Self { year, month, day }
    }

    /// The day `n` days later.
    pub fn add_days(self, n: i64) -> Self {
        Self::from_number(self.number() + n)
    }

    /// The same day `n` months later, kept inside the month.
    pub fn add_months(self, n: i32) -> Self {
        let total = self.year * 12 + self.month as i32 - 1 + n;
        Self::new(
            total.div_euclid(12),
            (total.rem_euclid(12) + 1) as u32,
            self.day,
        )
    }

    /// Monday is 0, Sunday is 6.
    pub fn weekday(self) -> u32 {
        (self.number() + 3).rem_euclid(7) as u32
    }

    /// The last day of its month.
    pub fn end_of_month(self) -> Self {
        Self::new(self.year, self.month, days_in_month(self.year, self.month))
    }

    /// Written short, such as "Fri 3 April 2026".
    pub fn words(self) -> String {
        const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
        format!(
            "{} {} {} {}",
            DAYS[self.weekday() as usize],
            self.day,
            MONTHS[self.month as usize - 1],
            self.year
        )
    }
}

/// How many days a month has.
pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        _ => 28,
    }
}

/// The 42 days of a month's grid, starting on the Monday on or before the 1st.
pub fn month_grid(year: i32, month: u32) -> Vec<Day> {
    let first = Day::new(year, month, 1);
    let start = first.add_days(-(first.weekday() as i64));
    (0..42).map(|n| start.add_days(n)).collect()
}

type PickHandler = Rc<dyn Fn(Day, &mut Window, &mut App)>;

/// A month calendar with presets.
#[derive(IntoElement)]
pub struct DueDatePicker {
    id: ElementId,
    style: StyleRefinement,
    today: Day,
    shown: (i32, u32),
    picked: Option<Day>,
    cursor: Option<Day>,
    earliest: Option<Day>,
    weekends: bool,
    on_pick: Option<PickHandler>,
    on_cursor: Option<PickHandler>,
}

impl DueDatePicker {
    /// Create a picker showing today's month.
    pub fn new(id: impl Into<ElementId>, today: Day) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            today,
            shown: (today.year, today.month),
            picked: None,
            cursor: None,
            earliest: None,
            weekends: true,
            on_pick: None,
            on_cursor: None,
        }
    }

    /// The picked day; its month is shown unless the cursor is elsewhere.
    pub fn picked(mut self, day: Option<Day>) -> Self {
        self.picked = day;
        if let Some(day) = day {
            self.shown = (day.year, day.month);
        }
        self
    }

    /// The day the keyboard is on; its month is shown.
    pub fn cursor(mut self, day: Option<Day>) -> Self {
        self.cursor = day;
        if let Some(day) = day {
            self.shown = (day.year, day.month);
        }
        self
    }

    /// The earliest day that can be picked.
    pub fn earliest(mut self, day: Day) -> Self {
        self.earliest = Some(day);
        self
    }

    /// Whether weekends can be picked. They can by default.
    pub fn weekends(mut self, allowed: bool) -> Self {
        self.weekends = allowed;
        self
    }

    /// Run when a day is picked.
    pub fn on_pick(mut self, handler: impl Fn(Day, &mut Window, &mut App) + 'static) -> Self {
        self.on_pick = Some(Rc::new(handler));
        self
    }

    /// Run when the keyboard or the month buttons move the cursor.
    pub fn on_cursor(mut self, handler: impl Fn(Day, &mut Window, &mut App) + 'static) -> Self {
        self.on_cursor = Some(Rc::new(handler));
        self
    }
}

impl Styled for DueDatePicker {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

fn allowed(day: Day, earliest: Option<Day>, weekends: bool) -> bool {
    earliest.is_none_or(|e| day >= e) && (weekends || day.weekday() < 5)
}

impl RenderOnce for DueDatePicker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let chip = theme.muted;
        let accent = theme.primary;
        let radius_lg = theme.radius_lg;
        let (year, month) = self.shown;
        let (earliest, weekends) = (self.earliest, self.weekends);
        let cursor = self
            .cursor
            .or(self.picked)
            .unwrap_or(Day::new(year, month, 1));

        let focus = window
            .use_keyed_state((self.id.clone(), "focus"), cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            })
            .read(cx)
            .clone();
        let is_focused = focus.is_focused(window);

        let presets = [
            ("Today", self.today),
            ("In a week", self.today.add_days(7)),
            ("In 30 days", self.today.add_days(30)),
            ("End of month", self.today.end_of_month()),
        ];
        let preset_buttons: Vec<_> = presets
            .iter()
            .map(|(label, day)| {
                let pick = self.on_pick.clone();
                let day = *day;
                let ok = allowed(day, earliest, weekends);
                Button::new(ElementId::NamedChild(
                    self.id.clone().into(),
                    format!("preset-{label}").into(),
                ))
                .xsmall()
                .map(|b| {
                    if self.picked == Some(day) {
                        b.primary()
                    } else {
                        b.outline()
                    }
                })
                .label(*label)
                .tooltip(day.words())
                .disabled(!ok)
                .on_click(move |_, window, cx| {
                    if let Some(pick) = &pick {
                        pick(day, window, cx);
                    }
                })
            })
            .collect();

        let step = |n: i32| {
            let move_to = self.on_cursor.clone();
            let label = if n < 0 {
                "Previous month"
            } else {
                "Next month"
            };
            Button::new(ElementId::NamedChild(self.id.clone().into(), label.into()))
                .xsmall()
                .ghost()
                .icon(if n < 0 {
                    IconName::ChevronLeft
                } else {
                    IconName::ChevronRight
                })
                .accessibility_label(label)
                .on_click(move |_, window, cx| {
                    if let Some(move_to) = &move_to {
                        move_to(cursor.add_months(n), window, cx);
                    }
                })
        };

        let days: Vec<_> = month_grid(year, month)
            .into_iter()
            .map(|day| {
                let in_month = day.month == month;
                let ok = allowed(day, earliest, weekends);
                let picked = self.picked == Some(day);
                let is_today = day == self.today;
                let at_cursor = is_focused && day == cursor;
                let pick = self.on_pick.clone();
                div()
                    .id(ElementId::NamedChild(
                        self.id.clone().into(),
                        format!("day-{}", day.number()).into(),
                    ))
                    .size(px(34.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(8.))
                    .text_body()
                    .when(!in_month, |this| this.text_color(quiet.opacity(0.6)))
                    .when(!ok, |this| {
                        this.text_color(quiet.opacity(0.4)).line_through()
                    })
                    .when(is_today && !picked, |this| {
                        this.border_1().border_color(accent)
                    })
                    .when(picked, |this| {
                        this.bg(accent).text_color(fill).font_medium()
                    })
                    .when(at_cursor && !picked, |this| this.bg(chip))
                    .when(!picked, |this| this.pressable(ok))
                    .when(ok && !picked, |this| this.hover(|s| s.bg(chip)))
                    .role(Role::GridCell)
                    .aria_label(format!(
                        "{}{}{}",
                        day.words(),
                        if is_today { ", today" } else { "" },
                        if ok { "" } else { ", not available" }
                    ))
                    .aria_selected(picked)
                    .when(ok, |this| {
                        this.on_click(move |_, window, cx| {
                            if let Some(pick) = &pick {
                                pick(day, window, cx);
                            }
                        })
                    })
                    .child(day.day.to_string())
            })
            .collect();

        let key_down = {
            let move_to = self.on_cursor.clone();
            let pick = self.on_pick.clone();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                let next = match event.keystroke.key.as_str() {
                    "left" => cursor.add_days(-1),
                    "right" => cursor.add_days(1),
                    "up" => cursor.add_days(-7),
                    "down" => cursor.add_days(7),
                    "pageup" => cursor.add_months(-1),
                    "pagedown" => cursor.add_months(1),
                    "enter" | "space" => {
                        if allowed(cursor, earliest, weekends) {
                            if let Some(pick) = &pick {
                                pick(cursor, window, cx);
                            }
                        }
                        cx.stop_propagation();
                        return;
                    }
                    _ => return,
                };
                if let Some(move_to) = &move_to {
                    move_to(next, window, cx);
                }
                cx.stop_propagation();
            }
        };

        v_flex()
            .id(self.id.clone())
            .track_focus(&focus)
            .on_key_down(key_down)
            .role(Role::Grid)
            .aria_label(match self.picked {
                Some(day) => format!("Due date, {}", day.words()),
                None => "Due date, none picked".to_string(),
            })
            .w(px(280.))
            .gap_3()
            .p_3()
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .shadow(resting_shadow(cx))
            .when(is_focused, |this| this.focus_ring_style(window, cx))
            .child(h_flex().flex_wrap().gap_1().children(preset_buttons))
            .child(
                h_flex()
                    .gap_1()
                    .child(step(-1))
                    .child(
                        div()
                            .flex_1()
                            .text_center()
                            .font_medium()
                            .child(format!("{} {year}", MONTHS[month as usize - 1])),
                    )
                    .child(step(1)),
            )
            .child(
                h_flex().children(["M", "T", "W", "T", "F", "S", "S"].iter().map(|d| {
                    div()
                        .w(px(34.))
                        .text_center()
                        .text_caption()
                        .text_color(quiet)
                        .child(*d)
                })),
            )
            .child(h_flex().flex_wrap().w(px(238.)).children(days))
            .child(
                div()
                    .text_caption()
                    .text_color(quiet)
                    .child(match self.picked {
                        Some(day) => SharedString::from(format!("Due {}", day.words())),
                        None => "Pick a day, or a preset.".into(),
                    }),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_count_and_wrap_correctly() {
        let day = Day::new(2026, 3, 31);
        assert_eq!(Day::from_number(day.number()), day);
        assert_eq!(day.add_days(1), Day::new(2026, 4, 1));
        assert_eq!(Day::new(2026, 1, 31).add_months(1), Day::new(2026, 2, 28));
        assert_eq!(Day::new(2024, 2, 1).end_of_month().day, 29);
        assert_eq!(Day::new(1970, 1, 1).weekday(), 3);
        assert_eq!(Day::new(2026, 11, 15).weekday(), 6);
        let grid = month_grid(2026, 4);
        assert_eq!(grid.len(), 42);
        assert_eq!(grid[0].weekday(), 0);
        assert!(grid[0] <= Day::new(2026, 4, 1));
    }
}
