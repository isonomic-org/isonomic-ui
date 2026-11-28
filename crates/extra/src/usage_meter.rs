// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Usage meter: how much of a plan's allowance is used, and on what.
//!
//! One card answers the questions a billing page gets: how much is gone, what
//! it went into, how fast it goes each day, and how close the other limits of
//! the plan are. The share tag and the button change before the allowance
//! runs out, in words as well as colour.
//!
//! Opt-in: a history of tokens and cost over the days of the period, split
//! by model, against a spending budget. Daily bars stack by model and
//! switch between tokens and cost; under them the running spend climbs
//! toward the budget line, with the pace carried on to the period's end.
//! Pointing at a day, or moving along with the arrow keys, reads it out.

use std::{rc::Rc, time::Duration};

use isonomic_ui::base::TestSupportExt as _;
use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, look,
    tab::{Tab, TabBar},
    tooltip::Tooltip,
    v_flex,
};
use isonomic_ui::{
    App, ElementId, FontWeight, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, PathBuilder, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, canvas, div, point,
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

/// One model's use on each day of the history.
#[derive(Debug, Clone)]
pub struct UsageModel {
    label: SharedString,
    days: Vec<(f64, f64)>,
    color: Option<Hsla>,
}

impl UsageModel {
    /// Create a model's line, with its tokens and cost on each day so far,
    /// oldest first.
    pub fn new(label: impl Into<SharedString>, days: impl IntoIterator<Item = (f64, f64)>) -> Self {
        Self {
            label: label.into(),
            days: days.into_iter().collect(),
            color: None,
        }
    }

    /// Set its colour. The theme's chart colours otherwise, in order.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }
}

/// What the history's bars count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UsageMetric {
    /// Tokens used.
    #[default]
    Tokens,
    /// Money spent.
    Cost,
}

/// Tokens and cost over the days of a period, by model, against a budget.
#[derive(Debug, Clone)]
pub struct UsageHistory {
    days: Vec<SharedString>,
    models: Vec<UsageModel>,
    budget: Option<f64>,
    currency: SharedString,
    metric: UsageMetric,
}

impl UsageHistory {
    /// Create a history over these days, named as they should be read
    /// ("Mar 3"). Days past the models' data are still to come.
    pub fn new(days: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        Self {
            days: days.into_iter().map(Into::into).collect(),
            models: Vec::new(),
            budget: None,
            currency: "$".into(),
            metric: UsageMetric::default(),
        }
    }

    /// Add a model. Up to five read well.
    pub fn model(mut self, model: UsageModel) -> Self {
        self.models.push(model);
        self
    }

    /// Set the spending budget for the whole period.
    pub fn budget(mut self, budget: f64) -> Self {
        self.budget = Some(budget);
        self
    }

    /// Set the sign written before money. "$" by default.
    pub fn currency(mut self, sign: impl Into<SharedString>) -> Self {
        self.currency = sign.into();
        self
    }

    /// Set what the bars count at first. Tokens by default.
    pub fn metric(mut self, metric: UsageMetric) -> Self {
        self.metric = metric;
        self
    }
}

/// Each day's amount per model, for the metric: `[day][model]`, as many
/// days as any model has data for.
pub fn stack_days(models: &[UsageModel], metric: UsageMetric) -> Vec<Vec<f64>> {
    let known = models.iter().map(|m| m.days.len()).max().unwrap_or(0);
    (0..known)
        .map(|day| {
            models
                .iter()
                .map(|m| {
                    m.days.get(day).map_or(0., |(tokens, cost)| match metric {
                        UsageMetric::Tokens => *tokens,
                        UsageMetric::Cost => *cost,
                    })
                })
                .collect()
        })
        .collect()
}

/// The running total of the daily cost, across every model.
pub fn running_cost(models: &[UsageModel]) -> Vec<f64> {
    let mut total = 0.;
    stack_days(models, UsageMetric::Cost)
        .iter()
        .map(|day| {
            total += day.iter().sum::<f64>();
            total
        })
        .collect()
}

/// Where the spend ends if it keeps the pace of the days so far, over a
/// period of `days` days.
pub fn projected_spend(running: &[f64], days: usize) -> f64 {
    match running.last() {
        Some(last) => last / running.len() as f64 * days.max(running.len()) as f64,
        None => 0.,
    }
}

/// Money in a few characters: "$4.20", "$1,240".
pub fn money_words(sign: &str, value: f64) -> String {
    if value.abs() >= 1000. {
        format!("{sign}{}", amount_words(value.round()))
    } else {
        format!("{sign}{value:.2}")
    }
}

/// Tokens in a few characters: "950", "48K", "1.2M".
pub fn token_words(tokens: f64) -> String {
    if tokens >= 1e6 {
        format!("{:.1}M", tokens / 1e6)
    } else if tokens >= 1e3 {
        format!("{:.0}K", tokens / 1e3)
    } else {
        format!("{tokens:.0}")
    }
}

/// The line under the budget plot: spent so far and where the pace leads.
pub fn pace_words(sign: &str, spent: f64, projected: f64, budget: Option<f64>) -> String {
    let mut words = format!(
        "{} spent, on pace for {}",
        money_words(sign, spent),
        money_words(sign, projected)
    );
    if let Some(budget) = budget.filter(|b| *b > 0.) {
        if spent >= budget {
            words.push_str(&format!(", over the {} budget", money_words(sign, budget)));
        } else if projected > budget {
            words.push_str(&format!(
                ", {} over the {} budget",
                money_words(sign, projected - budget),
                money_words(sign, budget)
            ));
        } else {
            words.push_str(&format!(
                ", within the {} budget",
                money_words(sign, budget)
            ));
        }
    }
    words
}

/// Step the picked day with a key over `known` days with data.
pub fn step_day(at: Option<usize>, key: &str, known: usize) -> Option<Option<usize>> {
    if known == 0 {
        return None;
    }
    let last = known - 1;
    Some(match key {
        "right" => Some(at.map_or(0, |at| (at + 1).min(last))),
        "left" => Some(at.map_or(last, |at| at.saturating_sub(1))),
        "home" => Some(0),
        "end" => Some(last),
        "escape" => None,
        _ => return None,
    })
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
    history: Option<UsageHistory>,
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
            history: None,
        }
    }

    /// Show tokens and cost over the days of the period, by model, against
    /// a budget.
    pub fn history(mut self, history: UsageHistory) -> Self {
        self.history = Some(history);
        self
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
    /// The day picked in the history.
    day: Option<usize>,
    /// What the history counts, once switched.
    metric: Option<UsageMetric>,
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
            day: None,
            metric: None,
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
                        .text_caption()
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
        .text_label_small();

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
                            .text_caption()
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

        let history = self.history.as_ref().map(|history| {
            let (day, metric) = {
                let s = state.read(cx);
                (s.day, s.metric.unwrap_or(history.metric))
            };
            let theme = cx.theme();
            let palette = [
                theme.chart_1,
                theme.chart_2,
                theme.chart_3,
                theme.chart_4,
                theme.chart_5,
            ];
            let ring = theme.ring;
            let model_colors: Vec<Hsla> = history
                .models
                .iter()
                .enumerate()
                .map(|(ix, m)| m.color.unwrap_or(palette[ix % palette.len()]))
                .collect();
            let stacks = stack_days(&history.models, metric);
            let known = stacks.len();
            let day = day.filter(|d| *d < known);
            let total_days = history.days.len().max(known).max(1);
            let peak = stacks
                .iter()
                .map(|d| d.iter().sum::<f64>())
                .fold(0., f64::max)
                .max(1e-9);
            let sign = history.currency.clone();
            let amount = {
                let sign = sign.clone();
                move |v: f64| match metric {
                    UsageMetric::Tokens => token_words(v),
                    UsageMetric::Cost => money_words(&sign, v),
                }
            };
            const BARS: f32 = 72.;
            let bars = (0..total_days).map(|ix| {
                let stack = stacks.get(ix);
                let total: f64 = stack.map_or(0., |d| d.iter().sum());
                let dim = day.is_some_and(|d| d != ix);
                let set_day = state.clone();
                div()
                    .id(("history-day", ix))
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .justify_end()
                    .when(day == Some(ix), |this| {
                        this.rounded(px(2.)).bg(look::ink::wash(cx))
                    })
                    .on_hover(move |over, _, cx| {
                        set_day.update(cx, |state, cx| {
                            if *over && ix < known {
                                state.day = Some(ix);
                                cx.notify();
                            } else if !*over && state.day == Some(ix) {
                                state.day = None;
                                cx.notify();
                            }
                        })
                    })
                    .child(match stack {
                        Some(stack) => v_flex()
                            .mx(px(1.))
                            .h(relative((total / peak) as f32))
                            .min_h(px(2.))
                            .rounded_t(px(2.))
                            .overflow_hidden()
                            .children(stack.iter().enumerate().rev().map(|(m, v)| {
                                div()
                                    .w_full()
                                    .flex_basis(relative((*v / total.max(1e-9)) as f32))
                                    .flex_shrink_0()
                                    .bg(model_colors[m].opacity(if dim { 0.35 } else { 0.9 }))
                            }))
                            .into_any_element(),
                        None => div()
                            .mx(px(1.))
                            .h(px(2.))
                            .rounded(px(1.))
                            .bg(line)
                            .into_any_element(),
                    })
            });
            let running = running_cost(&history.models);
            let spent = running.last().copied().unwrap_or(0.);
            let projected = projected_spend(&running, total_days);
            let budget = history.budget;
            let top = budget.unwrap_or(0.).max(projected).max(spent).max(1e-9);
            let over = budget.is_some_and(|b| projected > b);
            let pace_color = if budget.is_some_and(|b| spent >= b) {
                danger
            } else if over {
                warning
            } else {
                accent
            };
            let plot_running = running;
            let budget_plot = canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let w = f32::from(bounds.size.width);
                    let h = f32::from(bounds.size.height);
                    let step = w / total_days as f32;
                    let at = |ix: f32, v: f64| {
                        point(
                            bounds.left() + px(step * (ix + 0.5)),
                            bounds.top() + px(h - 2. - (v / top) as f32 * (h - 4.)),
                        )
                    };
                    if let Some(budget) = budget {
                        let y = bounds.top() + px(h - 2. - (budget / top) as f32 * (h - 4.));
                        let mut path = PathBuilder::stroke(px(1.)).dash_array(&[px(3.), px(3.)]);
                        path.move_to(point(bounds.left(), y));
                        path.line_to(point(bounds.right(), y));
                        if let Ok(path) = path.build() {
                            window.paint_path(path, quiet);
                        }
                    }
                    if plot_running.len() > 1 {
                        let mut path = PathBuilder::stroke(px(2.));
                        path.move_to(at(0., plot_running[0]));
                        for (ix, v) in plot_running.iter().enumerate().skip(1) {
                            path.line_to(at(ix as f32, *v));
                        }
                        if let Ok(path) = path.build() {
                            window.paint_path(path, pace_color);
                        }
                    }
                    if let Some(last) = plot_running.last()
                        && plot_running.len() < total_days
                    {
                        let mut path = PathBuilder::stroke(px(1.5)).dash_array(&[px(2.), px(3.)]);
                        path.move_to(at(plot_running.len() as f32 - 1., *last));
                        path.line_to(at(total_days as f32 - 1., projected));
                        if let Ok(path) = path.build() {
                            window.paint_path(path, pace_color.opacity(0.7));
                        }
                    }
                },
            )
            .size_full();
            let readout = match day {
                Some(d) => {
                    let stack = &stacks[d];
                    let total: f64 = stack.iter().sum();
                    let name = history.days.get(d).cloned().unwrap_or_default();
                    let mut words = format!("{name}: {}", amount(total));
                    for (m, v) in stack.iter().enumerate() {
                        if *v > 0. {
                            words.push_str(&format!(
                                ", {} {}",
                                history.models[m].label,
                                amount(*v)
                            ));
                        }
                    }
                    words
                }
                None => {
                    let total: f64 = stacks.iter().flatten().sum();
                    format!("{} so far over {known} of {total_days} days", amount(total))
                }
            };
            let metric_state = state.clone();
            let key_state = state.clone();
            let legend = history.models.iter().enumerate().map(|(m, model)| {
                let total: f64 = stacks.iter().map(|d| d[m]).sum();
                h_flex()
                    .gap(px(6.))
                    .child(div().size(px(8.)).rounded(px(2.)).bg(model_colors[m]))
                    .child(div().text_color(quiet).child(model.label.clone()))
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(ink)
                            .child(amount(total)),
                    )
            });
            let first = history.days.first().cloned().unwrap_or_default();
            let last_day = history.days.last().cloned().unwrap_or_default();
            v_flex()
                .gap_2()
                .pt_3()
                .border_t_1()
                .border_color(line)
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .text_caption()
                                .text_color(quiet)
                                .child("By model, each day"),
                        )
                        .child(
                            TabBar::new((self.id.clone(), "history-metric"))
                                .segmented()
                                .small()
                                .selected_index(usize::from(metric == UsageMetric::Cost))
                                .on_click(move |ix: &usize, _, cx| {
                                    metric_state.update(cx, |state, cx| {
                                        state.metric = Some(if *ix == 0 {
                                            UsageMetric::Tokens
                                        } else {
                                            UsageMetric::Cost
                                        });
                                        cx.notify();
                                    })
                                })
                                .child(Tab::new().label("Tokens"))
                                .child(Tab::new().label("Cost")),
                        ),
                )
                .child(
                    h_flex()
                        .id((self.id.clone(), "history-plot"))
                        .test_support()
                        .h(px(BARS))
                        .items_end()
                        .rounded(px(4.))
                        .tab_index(0)
                        .focus_visible(move |s| s.border_1().border_color(ring))
                        .role(Role::Group)
                        .aria_label(format!(
                            "{} by model over the period. {readout}",
                            match metric {
                                UsageMetric::Tokens => "Tokens",
                                UsageMetric::Cost => "Cost",
                            }
                        ))
                        .on_key_down(move |event: &KeyDownEvent, _, cx| {
                            let now = key_state.read(cx).day;
                            let Some(next) = step_day(now, event.keystroke.key.as_str(), known)
                            else {
                                return;
                            };
                            key_state.update(cx, |state, cx| {
                                state.day = next;
                                cx.notify();
                            });
                            cx.stop_propagation();
                        })
                        .children(bars),
                )
                .child(
                    h_flex()
                        .text_size(px(10.))
                        .line_height(px(16.))
                        .text_color(quiet)
                        .child(div().flex_1().child(first))
                        .child(last_day),
                )
                .child(
                    div()
                        .text_caption()
                        .text_color(ink)
                        .min_h(px(16.))
                        .child(readout),
                )
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap_x_4()
                        .gap_y_1()
                        .text_caption()
                        .children(legend),
                )
                .child(
                    div()
                        .id((self.id.clone(), "budget-plot"))
                        .relative()
                        .h(px(44.))
                        .mt_1()
                        .role(Role::Image)
                        .aria_label(pace_words(&sign, spent, projected, budget))
                        .child(budget_plot)
                        .when_some(budget, |this, budget| {
                            this.child(
                                div()
                                    .absolute()
                                    .right_0()
                                    .top(px((44. - 2. - (budget / top) as f32 * 40.) - 16.))
                                    .text_size(px(10.))
                                    .line_height(px(16.))
                                    .text_color(quiet)
                                    .child(format!("Budget {}", money_words(&sign, budget))),
                            )
                        }),
                )
                .child(
                    h_flex()
                        .gap_1()
                        .text_caption()
                        .text_color(if over { warning } else { quiet })
                        .when(over, |this| {
                            this.child(Icon::new(IconName::TriangleAlert).size_3())
                        })
                        .child(pace_words(&sign, spent, projected, budget)),
                )
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
                                .text_caption()
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
            .bg(fill)
            .text_color(ink)
            .text_body()
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
                                        div()
                                            .truncate()
                                            .text_caption()
                                            .text_color(quiet)
                                            .child(caption)
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
                                    .font_medium()
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
                    .children(history)
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

    fn models() -> Vec<UsageModel> {
        vec![
            UsageModel::new("Large", [(1000., 2.), (3000., 6.)]),
            UsageModel::new("Small", [(500., 0.5), (100., 0.1), (200., 0.2)]),
        ]
    }

    #[test]
    fn days_stack_by_model_for_either_metric() {
        let tokens = stack_days(&models(), UsageMetric::Tokens);
        assert_eq!(
            tokens,
            [vec![1000., 500.], vec![3000., 100.], vec![0., 200.]]
        );
        let cost = stack_days(&models(), UsageMetric::Cost);
        assert_eq!(cost[1], [6., 0.1]);
        assert!(stack_days(&[], UsageMetric::Cost).is_empty());
    }

    #[test]
    fn the_running_cost_projects_to_the_period_end() {
        let running = running_cost(&models());
        assert_eq!(running.len(), 3);
        assert!((running[2] - 8.8).abs() < 1e-9);
        // 8.8 over 3 days kept up for 30 days.
        assert!((projected_spend(&running, 30) - 88.).abs() < 1e-9);
        assert_eq!(projected_spend(&[], 30), 0.);
        // A period shorter than the data does not shrink the spend.
        assert!((projected_spend(&running, 2) - 8.8).abs() < 1e-9);
    }

    #[test]
    fn pace_words_compare_against_the_budget() {
        assert_eq!(
            pace_words("$", 40., 90., Some(100.)),
            "$40.00 spent, on pace for $90.00, within the $100.00 budget"
        );
        assert_eq!(
            pace_words("$", 60., 130., Some(100.)),
            "$60.00 spent, on pace for $130.00, $30.00 over the $100.00 budget"
        );
        assert_eq!(
            pace_words("$", 120., 150., Some(100.)),
            "$120.00 spent, on pace for $150.00, over the $100.00 budget"
        );
        assert_eq!(
            pace_words("€", 1500., 3000., None),
            "€1,500 spent, on pace for €3,000"
        );
        assert_eq!(token_words(1_260_000.), "1.3M");
        assert_eq!(token_words(48_200.), "48K");
    }

    #[test]
    fn arrow_keys_step_through_known_days() {
        assert_eq!(step_day(None, "right", 5), Some(Some(0)));
        assert_eq!(step_day(None, "left", 5), Some(Some(4)));
        assert_eq!(step_day(Some(4), "right", 5), Some(Some(4)));
        assert_eq!(step_day(Some(0), "left", 5), Some(Some(0)));
        assert_eq!(step_day(Some(2), "end", 5), Some(Some(4)));
        assert_eq!(step_day(Some(2), "escape", 5), Some(None));
        assert_eq!(step_day(Some(2), "x", 5), None);
        assert_eq!(step_day(None, "right", 0), None);
    }

    #[test]
    fn later_parts_fade_and_then_turn_grey() {
        assert_eq!(part_strength(0), Some(1.));
        assert!(part_strength(3) < part_strength(2));
        assert_eq!(part_strength(4), None);
    }
}
