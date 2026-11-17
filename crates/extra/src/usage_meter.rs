// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Usage meter: how much of a plan's allowance is used, and on what.
//!
//! One card answers the questions a billing page gets: how much is gone, what
//! it went into, how fast it goes each day, and how close the other limits of
//! the plan are. The share tag and the button change before the allowance
//! runs out, in words as well as colour.

use std::{rc::Rc, time::Duration};

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    tooltip::Tooltip,
    v_flex,
};
use isonomic_ui::{
    App, ElementId, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// Something the allowance went into.
#[derive(Debug, Clone)]
pub struct UsagePart {
    label: SharedString,
    amount: f64,
    color: Option<Hsla>,
}

impl UsagePart {
    /// Create a part with the amount it used.
    pub fn new(label: impl Into<SharedString>, amount: f64) -> Self {
        Self {
            label: label.into(),
            amount,
            color: None,
        }
    }

    /// Set the colour. Without one, parts get lighter tints of the accent and
    /// then grey, in order.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }
}

/// Another limit of the plan, such as seats or storage.
#[derive(Debug, Clone)]
pub struct UsageLimit {
    label: SharedString,
    used: f64,
    cap: Option<f64>,
    unit: Option<SharedString>,
}

impl UsageLimit {
    /// Create a limit with no cap.
    pub fn new(label: impl Into<SharedString>, used: f64) -> Self {
        Self {
            label: label.into(),
            used,
            cap: None,
            unit: None,
        }
    }

    /// Set the most the plan allows.
    pub fn cap(mut self, cap: f64) -> Self {
        self.cap = Some(cap);
        self
    }

    /// Set the unit shown after the cap, such as "GB".
    pub fn unit(mut self, unit: impl Into<SharedString>) -> Self {
        self.unit = Some(unit.into());
        self
    }
}

/// How close the allowance is to running out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageLevel {
    /// There is room left.
    Fine,
    /// Past the warning share.
    Low,
    /// Nothing is left.
    Out,
}

/// The level for an amount used out of an allowance. `warn_at` is the share,
/// from 0 to 1, where "low" starts.
pub fn usage_level(used: f64, allowance: f64, warn_at: f32) -> UsageLevel {
    if allowance <= 0. {
        return UsageLevel::Fine;
    }
    // Compared as f32, so that a share of exactly `warn_at` counts as low.
    let share = (used / allowance) as f32;
    if share >= 1. {
        UsageLevel::Out
    } else if share >= warn_at {
        UsageLevel::Low
    } else {
        UsageLevel::Fine
    }
}

/// An amount with grouped thousands and at most one decimal: "7,420", "3.2".
fn amount_words(value: f64) -> String {
    let tenths = (value.abs() * 10.).round() as u64;
    let digits = (tenths / 10).to_string();
    let mut words = String::new();
    if value < 0. && tenths > 0 {
        words.push('-');
    }
    for (n, digit) in digits.chars().enumerate() {
        if n > 0 && (digits.len() - n).is_multiple_of(3) {
            words.push(',');
        }
        words.push(digit);
    }
    if !tenths.is_multiple_of(10) {
        words.push('.');
        words.push_str(&(tenths % 10).to_string());
    }
    words
}

/// A part as a whole percentage of the whole.
fn percent(part: f64, whole: f64) -> u32 {
    if whole <= 0. {
        return 0;
    }
    (part / whole * 100.).round().clamp(0., 999.) as u32
}

/// When the allowance starts over, in words.
fn renewal_words(left: Duration) -> String {
    match left.as_secs() / 86_400 {
        0 => "renews today".into(),
        1 => "renews in 1 day".into(),
        days => format!("renews in {days} days"),
    }
}

/// The mean of the days that have happened.
fn daily_average(days: &[Option<f64>]) -> Option<f64> {
    let known: Vec<f64> = days.iter().flatten().copied().collect();
    (!known.is_empty()).then(|| known.iter().sum::<f64>() / known.len() as f64)
}

/// How strong the accent is for the part at `ix`. Later parts are grey.
fn part_strength(ix: usize) -> Option<f32> {
    [1., 0.7, 0.45, 0.28].get(ix).copied()
}

type PeriodHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type ActionHandler = Rc<dyn Fn(UsageLevel, &mut Window, &mut App)>;

/// A card with an allowance, what used it, and the plan's other limits.
#[derive(IntoElement)]
pub struct UsageMeter {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    plan: Option<SharedString>,
    renews_in: Option<Duration>,
    allowance: f64,
    unit: SharedString,
    parts: Vec<UsagePart>,
    limits: Vec<UsageLimit>,
    periods: Vec<(SharedString, SharedString)>,
    period: Option<SharedString>,
    daily: Vec<Option<f64>>,
    daily_label: SharedString,
    warn_at: f32,
    action_label: SharedString,
    low_action_label: SharedString,
    on_period_change: Option<PeriodHandler>,
    on_action: Option<ActionHandler>,
}

impl UsageMeter {
    /// Create a meter for an allowance, counted in "credits" unless `unit`
    /// says otherwise.
    pub fn new(id: impl Into<ElementId>, allowance: f64) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: "Usage".into(),
            plan: None,
            renews_in: None,
            allowance,
            unit: "credits".into(),
            parts: Vec::new(),
            limits: Vec::new(),
            periods: Vec::new(),
            period: None,
            daily: Vec::new(),
            daily_label: "Each day".into(),
            warn_at: 0.8,
            action_label: "See plans".into(),
            low_action_label: "Top up".into(),
            on_period_change: None,
            on_action: None,
        }
    }

    /// Set the heading, such as the workspace name.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Name the plan under the heading.
    pub fn plan(mut self, plan: impl Into<SharedString>) -> Self {
        self.plan = Some(plan.into());
        self
    }

    /// Say how long until the allowance starts over.
    pub fn renews_in(mut self, left: Duration) -> Self {
        self.renews_in = Some(left);
        self
    }

    /// Set what the allowance is counted in.
    pub fn unit(mut self, unit: impl Into<SharedString>) -> Self {
        self.unit = unit.into();
        self
    }

    /// Add something the allowance went into. Up to five read well; group
    /// the small ones.
    pub fn part(mut self, part: UsagePart) -> Self {
        self.parts.push(part);
        self
    }

    /// Add another limit of the plan under the allowance.
    pub fn limit(mut self, limit: UsageLimit) -> Self {
        self.limits.push(limit);
        self
    }

    /// Offer a period to look at, by id and label. Pass the parts, days and
    /// limits of the chosen period along with `selected_period`.
    pub fn period(mut self, id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        self.periods.push((id.into(), label.into()));
        self
    }

    /// Mark the period the numbers belong to. The first one otherwise.
    pub fn selected_period(mut self, id: impl Into<SharedString>) -> Self {
        self.period = Some(id.into());
        self
    }

    /// Set the amount used on each day, oldest first. `None` is a day still
    /// to come.
    pub fn daily(mut self, days: impl IntoIterator<Item = Option<f64>>) -> Self {
        self.daily = days.into_iter().collect();
        self
    }

    /// Set the heading over the days.
    pub fn daily_label(mut self, label: impl Into<SharedString>) -> Self {
        self.daily_label = label.into();
        self
    }

    /// Set the share, from 0 to 1, where the allowance and the limits start
    /// to warn. 0.8 by default.
    pub fn warn_at(mut self, share: f32) -> Self {
        self.warn_at = share;
        self
    }

    /// Set the button's label while there is room left.
    pub fn action_label(mut self, label: impl Into<SharedString>) -> Self {
        self.action_label = label.into();
        self
    }

    /// Set the button's label once the allowance is low or used up.
    pub fn low_action_label(mut self, label: impl Into<SharedString>) -> Self {
        self.low_action_label = label.into();
        self
    }

    /// Run when another period is picked, with its id.
    pub fn on_period_change(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_period_change = Some(Rc::new(handler));
        self
    }

    /// Show the button, and run with the current level when it is pressed.
    pub fn on_action(
        mut self,
        handler: impl Fn(UsageLevel, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(handler));
        self
    }
}

impl Styled for UsageMeter {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

struct MeterState {
    /// The part the pointer is on, in the bar or in the list.
    hovered: Option<usize>,
    period: Option<SharedString>,
}

impl RenderOnce for UsageMeter {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.info;
        let warning = theme.warning;
        let danger = theme.danger;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;

        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| MeterState {
            hovered: None,
            period: None,
        });
        let (hovered, kept_period) = {
            let state = state.read(cx);
            (state.hovered, state.period.clone())
        };
        let period = self
            .period
            .clone()
            .or(kept_period)
            .or_else(|| self.periods.first().map(|(id, _)| id.clone()));

        let used: f64 = self.parts.iter().map(|part| part.amount).sum();
        let level = usage_level(used, self.allowance, self.warn_at);
        let share = percent(used, self.allowance);
        let colors: Vec<Hsla> = self
            .parts
            .iter()
            .enumerate()
            .map(|(ix, part)| {
                part.color.unwrap_or_else(|| match part_strength(ix) {
                    Some(strength) => accent.opacity(strength),
                    None => quiet.opacity(0.45),
                })
            })
            .collect();
        let hover = |ix: usize| {
            let state = state.clone();
            move |hovered: &bool, _: &mut Window, cx: &mut App| {
                let next = hovered.then_some(ix);
                state.update(cx, |state, cx| {
                    if *hovered || state.hovered == Some(ix) {
                        state.hovered = next;
                        cx.notify();
                    }
                })
            }
        };

        let caption = match (self.plan.clone(), self.renews_in.map(renewal_words)) {
            (Some(plan), Some(renewal)) => Some(format!("{plan}, {renewal}")),
            (Some(plan), None) => Some(plan.to_string()),
            (None, Some(renewal)) => Some(renewal),
            (None, None) => None,
        };
        let switch = (!self.periods.is_empty()).then(|| {
            h_flex()
                .flex_none()
                .gap_1()
                .children(self.periods.iter().enumerate().map(|(ix, (id, label))| {
                    let id = id.clone();
                    let is_selected = period.as_ref() == Some(&id);
                    div()
                        .id(("period", ix))
                        .flex()
                        .items_center()
                        .h(px(26.))
                        .px_2()
                        .rounded(radius)
                        .border_1()
                        .border_color(line)
                        .text_xs()
                        .cursor_pointer()
                        .map(|this| {
                            if is_selected {
                                this.bg(chip).font_medium()
                            } else {
                                this.text_color(quiet)
                            }
                        })
                        .on_click({
                            let state = state.clone();
                            let on_period_change = self.on_period_change.clone();
                            move |_, window, cx| {
                                if is_selected {
                                    return;
                                }
                                state.update(cx, |state, cx| {
                                    state.period = Some(id.clone());
                                    cx.notify();
                                });
                                if let Some(handler) = &on_period_change {
                                    handler(&id, window, cx);
                                }
                            }
                        })
                        .child(label.clone())
                }))
        });

        let tag = match level {
            UsageLevel::Fine => h_flex().bg(chip).child(format!("{share}%")),
            UsageLevel::Low => h_flex()
                .bg(warning.opacity(0.18))
                .child(
                    Icon::new(IconName::TriangleAlert)
                        .size_3()
                        .text_color(warning),
                )
                .child(format!("{share}%, running low")),
            UsageLevel::Out => h_flex()
                .bg(danger.opacity(0.16))
                .child(Icon::new(IconName::CircleAlert).size_3().text_color(danger))
                .child(format!("{share}%, used up")),
        }
        .flex_none()
        .gap_1()
        .px_1p5()
        .rounded(px(5.))
        .text_xs()
        .font_medium();

        let bar = h_flex()
            .w_full()
            .h(px(6.))
            .gap(px(2.))
            .rounded(px(3.))
            .overflow_hidden()
            .bg(chip)
            .children(self.parts.iter().enumerate().map(|(ix, part)| {
                let width = if self.allowance > 0. {
                    (part.amount / self.allowance).clamp(0., 1.) as f32
                } else {
                    0.
                };
                let dimmed = hovered.is_some_and(|hovered| hovered != ix);
                let words = format!(
                    "{}: {} {}",
                    part.label,
                    amount_words(part.amount),
                    self.unit
                );
                div()
                    .id(("part", ix))
                    .h_full()
                    .w(relative(width))
                    .bg(colors[ix].opacity(if dimmed { 0.35 } else { 1. }))
                    .on_hover(hover(ix))
                    .tooltip(move |window, cx| Tooltip::new(words.clone()).build(window, cx))
            }));

        let list = self.parts.iter().enumerate().map(|(ix, part)| {
            h_flex()
                .id(("row", ix))
                .gap_2()
                .h(px(28.))
                .px_1()
                .rounded(radius)
                .when(hovered == Some(ix), |this| this.bg(chip))
                .on_hover(hover(ix))
                .child(div().flex_none().size_2().rounded(px(2.)).bg(colors[ix]))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(part.label.clone()),
                )
                .child(
                    div()
                        .flex_none()
                        .font_medium()
                        .child(amount_words(part.amount)),
                )
                .child(
                    div()
                        .flex_none()
                        .w(px(36.))
                        .text_right()
                        .text_color(quiet)
                        .child(format!("{}%", percent(part.amount, used))),
                )
        });
        let list: Vec<_> = list.collect();

        let peak = self.daily.iter().flatten().copied().fold(0., f64::max);
        let days =
            (!self.daily.is_empty()).then(|| {
                v_flex()
                    .gap_1p5()
                    .child(
                        h_flex()
                            .text_xs()
                            .text_color(quiet)
                            .child(div().flex_1().child(self.daily_label.clone()))
                            .children(daily_average(&self.daily).map(|average| {
                                format!("{} a day on average", amount_words(average.round()))
                            })),
                    )
                    .child(h_flex().items_end().gap(px(2.)).h(px(32.)).children(
                        self.daily.iter().map(|day| {
                            let height = match day {
                                Some(amount) if peak > 0. => (amount / peak * 32.).max(2.) as f32,
                                _ => 2.,
                            };
                            div()
                                .flex_1()
                                .h(px(height))
                                .rounded(px(1.))
                                .bg(if day.is_some() {
                                    quiet.opacity(0.3)
                                } else {
                                    line
                                })
                        }),
                    ))
            });

        let limits = (!self.limits.is_empty()).then(|| {
            h_flex()
                .items_start()
                .gap_4()
                .pt_3()
                .border_t_1()
                .border_color(line)
                .children(self.limits.iter().map(|limit| {
                    let share = limit
                        .cap
                        .filter(|cap| *cap > 0.)
                        .map(|cap| (limit.used / cap).clamp(0., 1.) as f32);
                    let near = share.is_some_and(|share| share >= self.warn_at);
                    let of = match (limit.cap, &limit.unit) {
                        (Some(cap), Some(unit)) => format!("of {} {unit}", amount_words(cap)),
                        (Some(cap), None) => format!("of {}", amount_words(cap)),
                        (None, _) => "no cap".into(),
                    };
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_1()
                        .child(
                            h_flex()
                                .gap_1()
                                .text_xs()
                                .text_color(quiet)
                                .child(div().truncate().child(limit.label.clone()))
                                .when(near, |this| {
                                    this.child(
                                        Icon::new(IconName::TriangleAlert)
                                            .size_3()
                                            .text_color(warning),
                                    )
                                }),
                        )
                        .child(
                            h_flex()
                                .gap_1()
                                .child(
                                    div()
                                        .flex_none()
                                        .font_medium()
                                        .child(amount_words(limit.used)),
                                )
                                .child(div().truncate().text_color(quiet).child(of)),
                        )
                        .map(|this| match share {
                            Some(share) => this.child(
                                div().w_full().h(px(3.)).rounded(px(2.)).bg(chip).child(
                                    div()
                                        .h_full()
                                        .w(relative(share))
                                        .rounded(px(2.))
                                        .bg(if near { warning } else { accent }),
                                ),
                            ),
                            // No cap: a dashed line where the track would be.
                            None => this.child(
                                div()
                                    .w_full()
                                    .border_t_2()
                                    .border_dashed()
                                    .border_color(line),
                            ),
                        })
                }))
        });

        let left = (self.allowance - used).max(0.);
        let remaining = match level {
            UsageLevel::Out => format!("No {} remaining", self.unit),
            _ => format!("{} {} remaining", amount_words(left), self.unit),
        };
        let button = self.on_action.clone().map(|handler| {
            let button = Button::new((self.id.clone(), "action")).small();
            let button = match level {
                UsageLevel::Fine => button.outline().label(self.action_label.clone()),
                _ => button.primary().label(self.low_action_label.clone()),
            };
            button.on_click(move |_, window, cx| handler(level, window, cx))
        });

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(460.))
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .line_height(relative(1.45))
            .shadow(resting_shadow(cx))
            .child(
                v_flex()
                    .gap_3()
                    .p_4()
                    .child(
                        h_flex()
                            .items_start()
                            .gap_3()
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .child(div().truncate().font_medium().child(self.title))
                                    .children(caption.map(|caption| {
                                        div().truncate().text_xs().text_color(quiet).child(caption)
                                    })),
                            )
                            .children(switch),
                    )
                    .child(
                        h_flex()
                            .items_end()
                            .gap_2()
                            .child(
                                div()
                                    .text_2xl()
                                    .font_semibold()
                                    .line_height(relative(1.1))
                                    .child(amount_words(used)),
                            )
                            .child(div().flex_1().min_w_0().truncate().text_color(quiet).child(
                                format!("of {} {}", amount_words(self.allowance), self.unit),
                            ))
                            .child(tag),
                    )
                    .child(bar)
                    .child(v_flex().children(list))
                    .children(days)
                    .children(limits),
            )
            .child(
                h_flex()
                    .gap_3()
                    .min_h(px(44.))
                    .px_4()
                    .py_2()
                    .border_t_1()
                    .border_color(line)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(quiet)
                            .child(remaining),
                    )
                    .children(button),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_level_follows_the_share_used() {
        assert_eq!(usage_level(790., 1000., 0.8), UsageLevel::Fine);
        assert_eq!(usage_level(800., 1000., 0.8), UsageLevel::Low);
        assert_eq!(usage_level(1000., 1000., 0.8), UsageLevel::Out);
        assert_eq!(usage_level(1200., 1000., 0.8), UsageLevel::Out);
        // With nothing to measure against there is nothing to warn about.
        assert_eq!(usage_level(5., 0., 0.8), UsageLevel::Fine);
    }

    #[test]
    fn amounts_group_thousands_and_keep_one_decimal() {
        assert_eq!(amount_words(0.), "0");
        assert_eq!(amount_words(7420.), "7,420");
        assert_eq!(amount_words(1_250_000.), "1,250,000");
        assert_eq!(amount_words(3.2), "3.2");
        assert_eq!(amount_words(999.96), "1,000");
        assert_eq!(amount_words(-12.5), "-12.5");
    }

    #[test]
    fn percentages_round_and_survive_an_empty_whole() {
        assert_eq!(percent(3980., 7420.), 54);
        assert_eq!(percent(1., 3.), 33);
        assert_eq!(percent(5., 0.), 0);
    }

    #[test]
    fn the_renewal_counts_whole_days() {
        assert_eq!(renewal_words(Duration::from_secs(3600)), "renews today");
        assert_eq!(
            renewal_words(Duration::from_secs(86_400)),
            "renews in 1 day"
        );
        assert_eq!(
            renewal_words(Duration::from_secs(12 * 86_400 + 60)),
            "renews in 12 days"
        );
    }

    #[test]
    fn the_average_skips_days_to_come() {
        assert_eq!(daily_average(&[Some(10.), Some(20.), None]), Some(15.));
        assert_eq!(daily_average(&[None, None]), None);
        assert_eq!(daily_average(&[]), None);
    }

    #[test]
    fn later_parts_fade_and_then_turn_grey() {
        assert_eq!(part_strength(0), Some(1.));
        assert!(part_strength(3) < part_strength(2));
        assert_eq!(part_strength(4), None);
    }
}
