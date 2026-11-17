// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Period picker: pick a span of days on two months side by side, or take one of the reporting periods people use most.
//!
//! The first click picks the start, the second the end; until then the
//! span follows the pointer. Presets cover the periods bookkeeping runs on:
//! this month, last month, this quarter, last quarter, year to date and the
//! last 30 days. The span's length is counted underneath.

use std::rc::Rc;

use isonomic_ui::component::{
    ActiveTheme as _, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};

use crate::common::resting_shadow;

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

/// A calendar day as year, month (1 to 12) and day.
pub type Ymd = (i32, u32, u32);

fn days_in(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        _ => 28,
    }
}

/// Days since 1 January 1970.
pub fn day_number((y, m, d): Ymd) -> i64 {
    let (y, m) = if m <= 2 {
        (y as i64 - 1, m as i64 + 9)
    } else {
        (y as i64, m as i64 - 3)
    };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + (153 * m + 2) / 5 + d as i64 - 1;
    era * 146_097 + doe - 719_468
}

/// The day for a day number.
pub fn from_day_number(n: i64) -> Ymd {
    let z = n + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((yoe + era * 400 + i64::from(m <= 2)) as i32, m, d)
}

fn month_shift((y, m): (i32, u32), n: i32) -> (i32, u32) {
    let total = y * 12 + m as i32 - 1 + n;
    (total.div_euclid(12), (total.rem_euclid(12) + 1) as u32)
}

/// The named reporting periods for a given today, each as label, start and end.
pub fn presets(today: Ymd) -> Vec<(&'static str, Ymd, Ymd)> {
    let (y, m, _) = today;
    let (py, pm) = month_shift((y, m), -1);
    let q = (m - 1) / 3;
    let (qy, qm) = (y, q * 3 + 1);
    let (lqy, lqm) = month_shift((qy, qm), -3);
    let (lqey, lqem) = month_shift((lqy, lqm), 2);
    let (qey, qem) = month_shift((qy, qm), 2);
    vec![
        ("This month", (y, m, 1), (y, m, days_in(y, m))),
        ("Last month", (py, pm, 1), (py, pm, days_in(py, pm))),
        ("This quarter", (qy, qm, 1), (qey, qem, days_in(qey, qem))),
        (
            "Last quarter",
            (lqy, lqm, 1),
            (lqey, lqem, days_in(lqey, lqem)),
        ),
        ("Year to date", (y, 1, 1), today),
        (
            "Last 30 days",
            from_day_number(day_number(today) - 29),
            today,
        ),
    ]
}

/// Write a span short, such as "1 Mar – 31 Mar 2026, 31 days".
pub fn span_words(a: Ymd, b: Ymd) -> String {
    let (a, b) = if day_number(a) <= day_number(b) {
        (a, b)
    } else {
        (b, a)
    };
    let days = day_number(b) - day_number(a) + 1;
    let short = |(y, m, d): Ymd, year: bool| {
        let name = &MONTHS[m as usize - 1][..3];
        if year {
            format!("{d} {name} {y}")
        } else {
            format!("{d} {name}")
        }
    };
    format!(
        "{} \u{2013} {}, {days} day{}",
        short(a, a.0 != b.0),
        short(b, true),
        if days == 1 { "" } else { "s" }
    )
}

type SpanHandler = Rc<dyn Fn(Option<Ymd>, Option<Ymd>, &mut Window, &mut App)>;
type HoverHandler = Rc<dyn Fn(Option<Ymd>, &mut Window, &mut App)>;
type MonthHandler = Rc<dyn Fn(i32, &mut Window, &mut App)>;

/// Two months to pick a span on.
#[derive(IntoElement)]
pub struct PeriodPicker {
    id: ElementId,
    style: StyleRefinement,
    today: Ymd,
    first_month: (i32, u32),
    start: Option<Ymd>,
    end: Option<Ymd>,
    hover: Option<Ymd>,
    on_span: Option<SpanHandler>,
    on_hover: Option<HoverHandler>,
    on_month: Option<MonthHandler>,
}

impl PeriodPicker {
    /// Create a picker showing last month and this month.
    pub fn new(id: impl Into<ElementId>, today: Ymd) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            today,
            first_month: month_shift((today.0, today.1), -1),
            start: None,
            end: None,
            hover: None,
            on_span: None,
            on_hover: None,
            on_month: None,
        }
    }

    /// Set the span; an end without a start is ignored.
    pub fn span(mut self, start: Option<Ymd>, end: Option<Ymd>) -> Self {
        self.start = start;
        self.end = end.filter(|_| start.is_some());
        self
    }

    /// The day under the pointer while the end is being picked.
    pub fn hover(mut self, day: Option<Ymd>) -> Self {
        self.hover = day;
        self
    }

    /// Show this month first.
    pub fn first_month(mut self, year: i32, month: u32) -> Self {
        self.first_month = (year, month.clamp(1, 12));
        self
    }

    /// Run with the new span: a start alone while picking, then both.
    pub fn on_span(
        mut self,
        handler: impl Fn(Option<Ymd>, Option<Ymd>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_span = Some(Rc::new(handler));
        self
    }

    /// Run as the pointer moves over days.
    pub fn on_hover(
        mut self,
        handler: impl Fn(Option<Ymd>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_hover = Some(Rc::new(handler));
        self
    }

    /// Run with -1 or 1 when the month buttons are clicked.
    pub fn on_month(mut self, handler: impl Fn(i32, &mut Window, &mut App) + 'static) -> Self {
        self.on_month = Some(Rc::new(handler));
        self
    }
}

impl Styled for PeriodPicker {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for PeriodPicker {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.primary;
        let radius_lg = theme.radius_lg;
        let reach = self.end.or(self
            .hover
            .filter(|_| self.start.is_some() && self.end.is_none()));
        let (lo, hi) = match (self.start, reach) {
            (Some(a), Some(b)) => {
                let (a, b) = (day_number(a), day_number(b));
                (Some(a.min(b)), Some(a.max(b)))
            }
            (Some(a), None) => (Some(day_number(a)), Some(day_number(a))),
            _ => (None, None),
        };

        let month_view = |(y, m): (i32, u32)| {
            let first = day_number((y, m, 1));
            let lead = (first + 3).rem_euclid(7);
            let cells: Vec<_> = (0..lead)
                .map(|_| div().size(px(32.)).into_any_element())
                .chain((1..=days_in(y, m)).map(|d| {
                    let ymd = (y, m, d);
                    let n = day_number(ymd);
                    let inside = lo.zip(hi).is_some_and(|(a, b)| n >= a && n <= b);
                    let edge = lo == Some(n) || hi == Some(n);
                    let is_today = ymd == self.today;
                    let span = self.on_span.clone();
                    let hover = self.on_hover.clone();
                    let (start, end) = (self.start, self.end);
                    div()
                        .id(ElementId::NamedChild(
                            self.id.clone().into(),
                            format!("d-{n}").into(),
                        ))
                        .size(px(32.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_sm()
                        .cursor_pointer()
                        .when(inside && !edge, |this| this.bg(accent.opacity(0.14)))
                        .when(edge, |this| {
                            this.bg(accent)
                                .text_color(fill)
                                .rounded(px(8.))
                                .font_semibold()
                        })
                        .when(is_today && !edge, |this| {
                            this.border_1().border_color(accent).rounded(px(8.))
                        })
                        .when(!inside, |this| this.rounded(px(8.)).hover(|s| s.bg(chip)))
                        .role(Role::GridCell)
                        .aria_label(format!("{d} {} {y}", MONTHS[m as usize - 1]))
                        .aria_selected(inside)
                        .on_hover(move |on, window, cx| {
                            if let Some(hover) = &hover {
                                hover(on.then_some(ymd), window, cx);
                            }
                        })
                        .on_click(move |_, window, cx| {
                            if let Some(span) = &span {
                                match (start, end) {
                                    (Some(a), None) => span(Some(a), Some(ymd), window, cx),
                                    _ => span(Some(ymd), None, window, cx),
                                }
                            }
                        })
                        .child(d.to_string())
                        .into_any_element()
                }))
                .collect();
            v_flex()
                .gap_1()
                .child(
                    div()
                        .text_center()
                        .font_medium()
                        .child(format!("{} {y}", MONTHS[m as usize - 1])),
                )
                .child(
                    h_flex().children(["M", "T", "W", "T", "F", "S", "S"].iter().map(|d| {
                        div()
                            .w(px(32.))
                            .text_center()
                            .text_xs()
                            .text_color(quiet)
                            .child(*d)
                    })),
                )
                .child(h_flex().flex_wrap().w(px(224.)).children(cells))
        };

        let preset_buttons: Vec<_> = presets(self.today)
            .into_iter()
            .map(|(label, a, b)| {
                let span = self.on_span.clone();
                let active = self.start == Some(a) && self.end == Some(b);
                Button::new(ElementId::NamedChild(
                    self.id.clone().into(),
                    format!("preset-{label}").into(),
                ))
                .xsmall()
                .map(|btn| if active { btn.primary() } else { btn.ghost() })
                .label(label)
                .on_click(move |_, window, cx| {
                    if let Some(span) = &span {
                        span(Some(a), Some(b), window, cx);
                    }
                })
            })
            .collect();

        let step = |n: i32| {
            let on_month = self.on_month.clone();
            let label = if n < 0 {
                "Earlier months"
            } else {
                "Later months"
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
                    if let Some(on_month) = &on_month {
                        on_month(n, window, cx);
                    }
                })
        };

        let summary: SharedString = match (self.start, self.end) {
            (Some(a), Some(b)) => span_words(a, b).into(),
            (Some(_), None) => "Now pick the last day.".into(),
            _ => "Pick the first day, or a period.".into(),
        };

        h_flex()
            .id(self.id.clone())
            .items_start()
            .flex_wrap()
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .shadow(resting_shadow(cx))
            .child(
                v_flex()
                    .gap_0p5()
                    .p_2()
                    .border_r_1()
                    .border_color(line)
                    .children(preset_buttons),
            )
            .child(
                v_flex()
                    .gap_2()
                    .p_3()
                    .child(
                        h_flex()
                            .items_start()
                            .gap_4()
                            .child(step(-1))
                            .child(month_view(self.first_month))
                            .child(month_view(month_shift(self.first_month, 1)))
                            .child(step(1)),
                    )
                    .child(div().text_xs().text_color(quiet).child(summary)),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_follow_the_calendar() {
        let p = presets((2026, 5, 14));
        assert_eq!(p[0].1, (2026, 5, 1));
        assert_eq!(p[1], ("Last month", (2026, 4, 1), (2026, 4, 30)));
        assert_eq!(p[2].1, (2026, 4, 1));
        assert_eq!(p[2].2, (2026, 6, 30));
        assert_eq!(p[3].1, (2026, 1, 1));
        assert_eq!(p[3].2, (2026, 3, 31));
        assert_eq!(p[5].1, (2026, 4, 15));
        assert_eq!(from_day_number(day_number((2024, 2, 29))), (2024, 2, 29));
    }

    #[test]
    fn spans_read_in_words() {
        assert_eq!(
            span_words((2026, 3, 1), (2026, 3, 31)),
            "1 Mar \u{2013} 31 Mar 2026, 31 days"
        );
        assert_eq!(
            span_words((2026, 4, 2), (2025, 12, 30)),
            "30 Dec 2025 \u{2013} 2 Apr 2026, 94 days"
        );
    }
}
