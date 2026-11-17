// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Variant comparison card: where an experiment stands, and which way to go.
//!
//! Closed, the card is two lines: the experiment, its status, and the variant
//! that is ahead of the control. Open, it lists the variants with their value
//! and their change against the control for the chosen metric, and one of
//! them can be picked. Once a variant is picked the last line offers to go
//! with it.

use std::rc::Rc;

use isonomic_ui::base::Button as Pressable;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    App, ElementId, FocusHandle, Hsla, InteractiveElement as _, IntoElement, ParentElement as _,
    RenderOnce, Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled,
    Window, div, prelude::FluentBuilder as _, px, relative,
};

use crate::common::{Tone, resting_shadow};

type Format = Rc<dyn Fn(f64) -> String>;

/// Something the variants are measured by.
#[derive(Clone)]
pub struct ComparisonMetric {
    label: SharedString,
    format: Format,
    lower_is_better: bool,
}

impl ComparisonMetric {
    /// Create a metric where more is better, written with one decimal.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            format: Rc::new(|value| format!("{value:.1}")),
            lower_is_better: false,
        }
    }

    /// Set how the metric's values are written.
    pub fn format(mut self, format: impl Fn(f64) -> String + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }

    /// Count a drop as the better result, as for a bounce rate.
    pub fn lower_is_better(mut self, lower_is_better: bool) -> Self {
        self.lower_is_better = lower_is_better;
        self
    }
}

/// One of the things being compared.
#[derive(Debug, Clone)]
pub struct ComparisonVariant {
    id: SharedString,
    key: SharedString,
    name: SharedString,
    values: Vec<f64>,
    control: bool,
    sample: Option<u64>,
    certainty: Vec<f32>,
    color: Option<Hsla>,
}

impl ComparisonVariant {
    /// Create a variant. `key` is the short letter on its badge.
    pub fn new(
        id: impl Into<SharedString>,
        key: impl Into<SharedString>,
        name: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            key: key.into(),
            name: name.into(),
            values: Vec::new(),
            control: false,
            sample: None,
            certainty: Vec::new(),
            color: None,
        }
    }

    /// Set the variant's value for each metric, in the metrics' order.
    pub fn values(mut self, values: impl IntoIterator<Item = f64>) -> Self {
        self.values = values.into_iter().collect();
        self
    }

    /// Mark the variant the others are measured against. The first variant
    /// otherwise.
    pub fn control(mut self, control: bool) -> Self {
        self.control = control;
        self
    }

    /// Say how many were in the variant's sample.
    pub fn sample(mut self, sample: u64) -> Self {
        self.sample = Some(sample);
        self
    }

    /// Say how certain the result is for each metric, in percent. The card
    /// shows what it is given and works nothing out.
    pub fn certainty(mut self, certainty: impl IntoIterator<Item = f32>) -> Self {
        self.certainty = certainty.into_iter().collect();
        self
    }

    /// Set the variant's colour. Without one the control is grey and the
    /// others take the theme's chart colours in order.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }
}

type MetricHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type PickHandler = Rc<dyn Fn(Option<&SharedString>, &mut Window, &mut App)>;
type ActionHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// A foldable card that compares variants and lets one be picked.
#[derive(IntoElement)]
pub struct VariantComparisonCard {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    variants: Vec<ComparisonVariant>,
    metrics: Vec<ComparisonMetric>,
    metric: Option<usize>,
    default_metric: usize,
    picked: Option<Option<SharedString>>,
    default_picked: Option<SharedString>,
    status: Option<(SharedString, Tone)>,
    note: Option<SharedString>,
    hint: SharedString,
    action_label: Option<SharedString>,
    default_open: bool,
    on_metric_change: Option<MetricHandler>,
    on_pick: Option<PickHandler>,
    on_action: Option<ActionHandler>,
}

impl VariantComparisonCard {
    /// Create a card for an experiment. It starts closed.
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            variants: Vec::new(),
            metrics: Vec::new(),
            metric: None,
            default_metric: 0,
            picked: None,
            default_picked: None,
            status: None,
            note: None,
            hint: "Choose the one to go with".into(),
            action_label: None,
            default_open: false,
            on_metric_change: None,
            on_pick: None,
            on_action: None,
        }
    }

    /// Add a variant. Up to five read well.
    pub fn variant(mut self, variant: ComparisonVariant) -> Self {
        self.variants.push(variant);
        self
    }

    /// Add a metric. With more than one, a switch appears over the variants.
    pub fn metric(mut self, metric: ComparisonMetric) -> Self {
        self.metrics.push(metric);
        self
    }

    /// Control the shown metric, by position. Pair it with
    /// `on_metric_change`.
    pub fn selected_metric(mut self, metric: usize) -> Self {
        self.metric = Some(metric);
        self
    }

    /// Set the metric shown at first, by position.
    pub fn default_metric(mut self, metric: usize) -> Self {
        self.default_metric = metric;
        self
    }

    /// Control the picked variant, by id. Pair it with `on_pick`.
    pub fn picked(mut self, id: Option<SharedString>) -> Self {
        self.picked = Some(id);
        self
    }

    /// Set the variant picked at first, when the card keeps it itself.
    pub fn default_picked(mut self, id: impl Into<SharedString>) -> Self {
        self.default_picked = Some(id.into());
        self
    }

    /// Set the tag under the title, such as "Running".
    pub fn status(mut self, label: impl Into<SharedString>, tone: Tone) -> Self {
        self.status = Some((label.into(), tone));
        self
    }

    /// Set a short line next to the status, such as how far along the
    /// experiment is.
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// Set the last line shown while nothing is picked or pointed at.
    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = hint.into();
        self
    }

    /// Set the button's label. It names the picked variant's key otherwise.
    pub fn action_label(mut self, label: impl Into<SharedString>) -> Self {
        self.action_label = Some(label.into());
        self
    }

    /// Start with the comparison open.
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// Run when another metric is chosen, with its position.
    pub fn on_metric_change(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_metric_change = Some(Rc::new(handler));
        self
    }

    /// Run when a variant is picked, or with `None` when the pick is taken
    /// back.
    pub fn on_pick(
        mut self,
        handler: impl Fn(Option<&SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_pick = Some(Rc::new(handler));
        self
    }

    /// Show a button once a variant is picked, and run with that variant's
    /// id when it is pressed.
    pub fn on_action(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(handler));
        self
    }
}

impl Styled for VariantComparisonCard {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// The change against the control in percent. None while the control is zero.
fn lift(value: f64, base: f64) -> Option<f64> {
    (base != 0.).then(|| (value - base) / base.abs() * 100.)
}

/// Whether a change is the better result for the metric.
fn is_better(lift: f64, lower_is_better: bool) -> bool {
    if lower_is_better {
        lift < 0.
    } else {
        lift > 0.
    }
}

/// The variant that beats the control by the most, if any does.
fn leader(values: &[f64], control: usize, lower_is_better: bool) -> Option<usize> {
    let base = *values.get(control)?;
    values
        .iter()
        .enumerate()
        .filter(|(ix, _)| *ix != control)
        .filter_map(|(ix, value)| {
            let lift = lift(*value, base).filter(|lift| is_better(*lift, lower_is_better))?;
            Some((ix, lift.abs()))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(ix, _)| ix)
}

/// A change with its sign and one decimal: "+11.9%".
fn lift_words(lift: f64) -> String {
    let tenths = (lift.abs() * 10.).round() / 10.;
    if tenths == 0. {
        "0%".into()
    } else if lift > 0. {
        format!("+{tenths:.1}%")
    } else {
        format!("\u{2212}{tenths:.1}%")
    }
}

/// A count with grouped thousands.
fn grouped(count: u64) -> String {
    let digits = count.to_string();
    let mut words = String::new();
    for (n, digit) in digits.chars().enumerate() {
        if n > 0 && (digits.len() - n).is_multiple_of(3) {
            words.push(',');
        }
        words.push(digit);
    }
    words
}

struct CardState {
    open: bool,
    metric: usize,
    picked: Option<SharedString>,
    /// The variant the pointer is on.
    hovered: Option<usize>,
    focus: FocusHandle,
}

impl RenderOnce for VariantComparisonCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let focus_tint = theme.ring.opacity(0.25);
        let accent = theme.info;
        let success = theme.success;
        let danger = theme.danger;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;
        let palette = [
            theme.chart_1,
            theme.chart_2,
            theme.chart_3,
            theme.chart_4,
            theme.chart_5,
        ];

        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| CardState {
            open: self.default_open,
            metric: self.default_metric,
            picked: self.default_picked.clone(),
            hovered: None,
            focus: cx.focus_handle(),
        });
        let (open, kept_metric, kept_picked, hovered, focus) = {
            let state = state.read(cx);
            (
                state.open,
                state.metric,
                state.picked.clone(),
                state.hovered,
                state.focus.clone(),
            )
        };
        let child_id = |name: String| ElementId::NamedChild(self.id.clone().into(), name.into());

        let plain = [ComparisonMetric::new("Value")];
        let metrics: &[ComparisonMetric] = if self.metrics.is_empty() {
            &plain
        } else {
            &self.metrics
        };
        let metric_ix = self.metric.unwrap_or(kept_metric).min(metrics.len() - 1);
        let metric = &metrics[metric_ix];
        let picked = self.picked.clone().unwrap_or(kept_picked);
        let picked_ix = picked
            .as_ref()
            .and_then(|id| self.variants.iter().position(|variant| variant.id == *id));
        let hovered = hovered.filter(|ix| *ix < self.variants.len());

        let control = self
            .variants
            .iter()
            .position(|variant| variant.control)
            .unwrap_or(0);
        let values: Vec<f64> = self
            .variants
            .iter()
            .map(|variant| variant.values.get(metric_ix).copied().unwrap_or(0.))
            .collect();
        let base = values.get(control).copied().unwrap_or(0.);
        let peak = values.iter().copied().fold(0., f64::max);
        let ahead = leader(&values, control, metric.lower_is_better);
        // The control is grey; the others take the chart colours in order.
        let mut next_color = 0;
        let colors: Vec<Hsla> = self
            .variants
            .iter()
            .enumerate()
            .map(|(ix, variant)| {
                variant.color.unwrap_or_else(|| {
                    if ix == control {
                        quiet.opacity(0.55)
                    } else {
                        next_color += 1;
                        palette[(next_color - 1) % palette.len()]
                    }
                })
            })
            .collect();
        let lift_tag = |lift: f64| {
            let color = if lift == 0. {
                quiet
            } else if is_better(lift, metric.lower_is_better) {
                success
            } else {
                danger
            };
            div()
                .flex_none()
                .px_1p5()
                .rounded(px(5.))
                .bg(color.opacity(0.14))
                .text_xs()
                .font_medium()
                .child(lift_words(lift))
        };

        // A few columns that stand for the variants. Everything they show is
        // in the rows.
        let glyph = h_flex()
            .items_end()
            .justify_center()
            .gap(px(2.))
            .size(px(36.))
            .pb(px(8.))
            .flex_none()
            .rounded(radius)
            .border_1()
            .border_color(line)
            .children(values.iter().enumerate().map(|(ix, value)| {
                let share = if peak > 0. { (value / peak) as f32 } else { 0. };
                let faded = hovered.is_some_and(|hovered| hovered != ix);
                div()
                    .w(px(4.))
                    .h(px(4. + 14. * share.clamp(0., 1.)))
                    .rounded(px(1.))
                    .bg(colors[ix].opacity(if faded { 0.35 } else { 1. }))
            }));
        let is_focused = focus.is_focused(window);
        let head = Pressable::new((self.id.clone(), "head"))
            .accessibility_label(self.title.clone())
            .aria_expanded(open)
            .track_focus(&focus)
            .on_click({
                let state = state.clone();
                move |_, _, cx| {
                    state.update(cx, |state, cx| {
                        state.open = !state.open;
                        cx.notify();
                    })
                }
            })
            .w_full()
            .justify_start()
            .gap_3()
            .p_3()
            .rounded(radius_lg)
            .cursor_pointer()
            .when(is_focused, |this| this.focus_ring_style(window, cx))
            .child(glyph)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .items_start()
                    .gap_0p5()
                    // The pressable sets a tight line height; give the two
                    // lines their own so they never overlap.
                    .line_height(relative(1.4))
                    .child(div().w_full().truncate().font_medium().child(self.title))
                    .child(
                        h_flex()
                            .gap_2()
                            .text_xs()
                            .children(self.status.map(|(label, tone)| {
                                h_flex()
                                    .flex_none()
                                    .gap_1()
                                    .px_1p5()
                                    .rounded(px(5.))
                                    .bg(tone.tint(cx))
                                    .font_medium()
                                    .child(
                                        div()
                                            .size(px(6.))
                                            .rounded_full_style(cx)
                                            .bg(tone.color(cx)),
                                    )
                                    .child(label)
                            }))
                            .children(
                                self.note
                                    .map(|note| div().truncate().text_color(quiet).child(note)),
                            ),
                    ),
            )
            .children(ahead.and_then(|ix| {
                let lift = lift(values[ix], base)?;
                Some(
                    h_flex()
                        .flex_none()
                        .gap_1()
                        .px_1p5()
                        .rounded(px(5.))
                        .bg(success.opacity(0.14))
                        .text_xs()
                        .font_medium()
                        .child(self.variants[ix].key.clone())
                        .child(lift_words(lift)),
                )
            }))
            .child(
                Icon::new(if open {
                    IconName::ChevronUp
                } else {
                    IconName::ChevronDown
                })
                .xsmall()
                .flex_none()
                .text_color(quiet),
            );

        let card = v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(360.))
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .line_height(relative(1.45))
            .shadow(resting_shadow(cx))
            .child(head);
        if !open {
            return card.refine_style(&self.style);
        }

        let switch = (metrics.len() > 1).then(|| {
            h_flex()
                .id(child_id("metrics".into()))
                .role(Role::TabList)
                .aria_label("Measure")
                .gap_1()
                .children(metrics.iter().enumerate().map(|(ix, option)| {
                    let state = state.clone();
                    let on_metric_change = self.on_metric_change.clone();
                    div()
                        .id(child_id(format!("metric-{ix}")))
                        // Each measure is a tab a screen reader can find and
                        // the keyboard can reach and press.
                        .role(Role::Tab)
                        .aria_label(option.label.clone())
                        .aria_selected(ix == metric_ix)
                        .tab_index(0)
                        .focus_visible(move |style| style.bg(focus_tint))
                        .flex()
                        .items_center()
                        .h(px(26.))
                        .px_2()
                        .rounded(radius)
                        .cursor_pointer()
                        .map(|this| {
                            if ix == metric_ix {
                                this.bg(chip).font_medium()
                            } else {
                                this.text_color(quiet)
                            }
                        })
                        .on_click(move |_, window, cx| {
                            if ix == metric_ix {
                                return;
                            }
                            state.update(cx, |state, cx| {
                                state.metric = ix;
                                cx.notify();
                            });
                            if let Some(handler) = &on_metric_change {
                                handler(ix, window, cx);
                            }
                        })
                        .child(option.label.clone())
                }))
        });

        let row_focus: Vec<FocusHandle> = (0..self.variants.len())
            .map(|ix| {
                window
                    .use_keyed_state(child_id(format!("variant-focus-{ix}")), cx, |_, cx| {
                        cx.focus_handle()
                    })
                    .read(cx)
                    .clone()
            })
            .collect();
        let rows = self.variants.iter().enumerate().map(|(ix, variant)| {
            let is_picked = picked_ix == Some(ix);
            let row_focused = row_focus[ix].is_focused(window);
            let mut name = format!(
                "{}, {}, {}",
                variant.key,
                variant.name,
                (metric.format)(values[ix])
            );
            if ix == control {
                name.push_str(", control");
            } else if let Some(lift) = lift(values[ix], base) {
                name.push_str(&format!(", {}", lift_words(lift)));
            }
            let faded = picked_ix.is_some_and(|picked| picked != ix) && hovered != Some(ix);
            let share = if peak > 0. {
                (values[ix] / peak).clamp(0., 1.) as f32
            } else {
                0.
            };
            let id = variant.id.clone();
            Pressable::new(child_id(format!("variant-{ix}")))
                .accessibility_label(name)
                .aria_selected(is_picked)
                .track_focus(&row_focus[ix])
                .on_click({
                    let state = state.clone();
                    let on_pick = self.on_pick.clone();
                    move |_, window, cx| {
                        // Pressing the picked variant takes the pick back.
                        let next = (!is_picked).then(|| id.clone());
                        state.update(cx, |state, cx| {
                            state.picked = next.clone();
                            cx.notify();
                        });
                        if let Some(handler) = &on_pick {
                            handler(next.as_ref(), window, cx);
                        }
                    }
                })
                .on_hover({
                    let state = state.clone();
                    move |hovered, _, cx| {
                        let next = hovered.then_some(ix);
                        state.update(cx, |state, cx| {
                            if *hovered || state.hovered == Some(ix) {
                                state.hovered = next;
                                cx.notify();
                            }
                        })
                    }
                })
                .w_full()
                .justify_start()
                .items_start()
                .gap_2p5()
                .px_2()
                .py_1p5()
                .rounded(radius)
                .border_1()
                .border_color(if is_picked {
                    accent.opacity(0.5)
                } else {
                    isonomic_ui::transparent_black()
                })
                .cursor_pointer()
                .when(is_picked, |this| this.bg(accent.opacity(0.08)))
                .when(faded, |this| this.opacity(0.55))
                .when(row_focused, |this| this.focus_ring_style(window, cx))
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(px(20.))
                        .rounded(px(5.))
                        .bg(colors[ix].opacity(0.18))
                        .text_xs()
                        .font_medium()
                        .map(|this| {
                            if is_picked {
                                this.child(Icon::new(IconName::Check).xsmall())
                            } else {
                                this.child(variant.key.clone())
                            }
                        }),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_1()
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .text_left()
                                        .child(variant.name.clone()),
                                )
                                .child(
                                    div()
                                        .flex_none()
                                        .font_medium()
                                        .child((metric.format)(values[ix])),
                                )
                                .map(|this| {
                                    if ix == control {
                                        this.child(
                                            div()
                                                .flex_none()
                                                .px_1p5()
                                                .rounded(px(5.))
                                                .bg(chip)
                                                .text_xs()
                                                .text_color(quiet)
                                                .child("Control"),
                                        )
                                    } else {
                                        this.children(lift(values[ix], base).map(&lift_tag))
                                    }
                                }),
                        )
                        .child(
                            div().w_full().h(px(3.)).rounded(px(2.)).bg(chip).child(
                                div()
                                    .h_full()
                                    .w(relative(share))
                                    .rounded(px(2.))
                                    .bg(colors[ix]),
                            ),
                        ),
                )
        });
        let rows: Vec<_> = rows.collect();

        // The last line is about the variant pointed at, else the picked one.
        let about = hovered.or(picked_ix).map(|ix| {
            let variant = &self.variants[ix];
            let mut facts: Vec<String> = Vec::new();
            if let Some(sample) = variant.sample {
                facts.push(format!("{} in sample", grouped(sample)));
            }
            if ix == control {
                facts.push("control".into());
            } else if let Some(certainty) = variant.certainty.get(metric_ix) {
                facts.push(format!("{certainty:.0}% certain"));
            }
            h_flex()
                .gap_1p5()
                .child(
                    div()
                        .flex_none()
                        .font_medium()
                        .text_color(ink)
                        .child(variant.key.clone()),
                )
                .child(div().min_w_0().truncate().child(facts.join(", ")))
        });
        let button = picked_ix.zip(self.on_action.clone()).map(|(ix, handler)| {
            let variant = &self.variants[ix];
            let id = variant.id.clone();
            let label = self
                .action_label
                .clone()
                .unwrap_or_else(|| format!("Go with {}", variant.key).into());
            Button::new((self.id.clone(), "action"))
                .primary()
                .small()
                .label(label)
                .on_click(move |_, window, cx| handler(&id, window, cx))
        });

        card.child(
            v_flex()
                .gap_1p5()
                .px_2()
                .pb_2()
                .children(switch.map(|switch| switch.px_1().pb_1()))
                .children(rows),
        )
        .child(
            h_flex()
                .gap_3()
                .min_h(px(44.))
                .px_3()
                .py_2()
                .border_t_1()
                .border_color(line)
                .text_xs()
                .text_color(quiet)
                .child(div().flex_1().min_w_0().map(|this| match about {
                    Some(about) => this.child(about),
                    None => this.truncate().child(self.hint.clone()),
                }))
                .children(button),
        )
        .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_change_is_measured_against_the_control() {
        assert_eq!(lift(4.7, 4.2).map(lift_words).as_deref(), Some("+11.9%"));
        assert_eq!(
            lift(4., 4.2).map(lift_words).as_deref(),
            Some("\u{2212}4.8%")
        );
        assert_eq!(lift(3., 0.), None);
        assert_eq!(lift_words(0.04), "0%");
    }

    #[test]
    fn a_drop_can_be_the_better_result() {
        assert!(is_better(5., false));
        assert!(!is_better(-5., false));
        assert!(is_better(-5., true));
        assert!(!is_better(0., true));
    }

    #[test]
    fn the_leader_beats_the_control_by_the_most() {
        assert_eq!(leader(&[4.2, 4.7, 4.0], 0, false), Some(1));
        assert_eq!(leader(&[4.2, 4.7, 4.9], 0, false), Some(2));
        // Nothing beats the control.
        assert_eq!(leader(&[4.2, 4.0, 4.1], 0, false), None);
        // With a metric where less is better the lowest one leads.
        assert_eq!(leader(&[40., 37., 44.], 0, true), Some(1));
        assert_eq!(leader(&[], 0, false), None);
    }

    #[test]
    fn counts_group_thousands() {
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(4790), "4,790");
        assert_eq!(grouped(1_000_000), "1,000,000");
    }
}
