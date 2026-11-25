// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Metric change list: a short recap of what moved over a period, one metric per row.

use std::time::Duration;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{ActiveTheme as _, Icon, IconName, StyledExt as _, h_flex, v_flex};
use isonomic_ui::{
    Animation, AnimationExt as _, App, ElementId, InteractiveElement as _, IntoElement,
    ParentElement as _, RenderOnce, Role, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, px, relative,
};

use crate::common::{Tone, resting_shadow};

/// Rows after this one appear together with it instead of waiting their turn.
const STAGGER_LIMIT: usize = 12;
const STAGGER_STEP_MS: f32 = 70.;
const ENTER_MS: f32 = 420.;

/// Which way a metric moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrendDirection {
    /// The value went up.
    Up,
    /// The value went down.
    Down,
}

/// One metric and what happened to it, in words.
#[derive(Debug, Clone)]
pub struct MetricChange {
    label: SharedString,
    change: SharedString,
    tone: Tone,
    direction: Option<TrendDirection>,
}

impl MetricChange {
    /// Create a change, for example `MetricChange::new("Churn", "Down to 2.1%")`.
    pub fn new(label: impl Into<SharedString>, change: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            change: change.into(),
            tone: Tone::Neutral,
            direction: None,
        }
    }

    /// Say whether the change is good, bad or neither. Neutral shows a dash.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// Point the arrow. A rise can be bad and a fall can be good, so this is
    /// separate from the tone. Defaults to down for a bad change and up otherwise.
    pub fn direction(mut self, direction: TrendDirection) -> Self {
        self.direction = Some(direction);
        self
    }

    /// The arrow to draw, or `None` for the neutral dash.
    fn arrow(&self) -> Option<TrendDirection> {
        match self.tone {
            Tone::Neutral => None,
            Tone::Danger => Some(self.direction.unwrap_or(TrendDirection::Down)),
            _ => Some(self.direction.unwrap_or(TrendDirection::Up)),
        }
    }
}

/// A change in words for screen readers, such as "Churn: Down to 2.1%, falling, good".
pub fn spoken_change(change: &MetricChange) -> String {
    let way = match change.arrow() {
        None => "no change",
        Some(TrendDirection::Up) => "rising",
        Some(TrendDirection::Down) => "falling",
    };
    let verdict = match change.tone {
        Tone::Success => ", good",
        Tone::Warning => ", needs attention",
        Tone::Danger => ", bad",
        Tone::Neutral | Tone::Info => "",
    };
    format!("{}: {}, {way}{verdict}", change.label, change.change)
}

/// A card that lists what changed since last time.
#[derive(IntoElement)]
pub struct MetricChangeList {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    period: Option<SharedString>,
    icon: Option<Icon>,
    changes: Vec<MetricChange>,
    stacked: bool,
}

impl MetricChangeList {
    /// Create an empty list.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: "Changes".into(),
            period: None,
            icon: None,
            changes: Vec::new(),
            stacked: false,
        }
    }

    /// Set the heading. Defaults to "Changes".
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Small text at the end of the heading, such as "This week".
    pub fn period(mut self, period: impl Into<SharedString>) -> Self {
        self.period = Some(period.into());
        self
    }

    /// Replace the icon in front of the heading.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Add one change.
    pub fn change(mut self, change: MetricChange) -> Self {
        self.changes.push(change);
        self
    }

    /// Add several changes.
    pub fn changes(mut self, changes: impl IntoIterator<Item = MetricChange>) -> Self {
        self.changes.extend(changes);
        self
    }

    /// Put each metric above its change instead of beside it. Use this in a
    /// narrow column so long text wraps instead of being cut off.
    pub fn stacked(mut self, stacked: bool) -> Self {
        self.stacked = stacked;
        self
    }
}

impl Styled for MetricChangeList {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for MetricChangeList {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let line = theme.border;
        let fill = theme.background;
        let radius = theme.radius_lg;
        let wash = theme.foreground.opacity(0.02);
        let animate = !cx.reduce_motion();
        let stacked = self.stacked;
        let inset = if stacked { px(14.) } else { px(16.) };

        let rows = self.changes.into_iter().enumerate().map(|(ix, change)| {
            let arrow = change.arrow();
            let spoken = spoken_change(&change);
            let mark = match arrow {
                None => IconName::Minus,
                Some(TrendDirection::Up) => IconName::ArrowUp,
                Some(TrendDirection::Down) => IconName::ArrowDown,
            };
            let row = div()
                .id(ix)
                .role(Role::ListItem)
                .aria_label(spoken)
                .relative()
                .flex()
                .px(inset)
                .border_t_1()
                .border_color(line)
                .hover(|style| style.bg(wash))
                .map(|this| {
                    if stacked {
                        this.flex_col().gap_1().py_2p5()
                    } else {
                        this.items_center().gap_4().min_h(px(48.))
                    }
                })
                .child(
                    div()
                        .font_medium()
                        .map(|this| {
                            if stacked {
                                this
                            } else {
                                this.w(relative(0.4)).flex_none().truncate()
                            }
                        })
                        .child(change.label),
                )
                .child(
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_2()
                        .when(stacked, |this| this.items_start())
                        .text_color(quiet)
                        .child(
                            div()
                                .flex()
                                .flex_none()
                                .items_center()
                                .justify_center()
                                .size_5()
                                .rounded(px(5.))
                                .bg(change.tone.tint(cx))
                                .text_color(change.tone.color(cx))
                                .child(Icon::new(mark).size_3()),
                        )
                        .child(div().flex_1().min_w_0().line_clamp(3).child(change.change)),
                );

            if !animate {
                return row.into_any_element();
            }

            // Rows arrive one after another. Each waits its turn inside one
            // animation, so a row never flashes in before its delay is over.
            let delay = ix.min(STAGGER_LIMIT) as f32 * STAGGER_STEP_MS;
            let total = delay + ENTER_MS;
            row.with_animation(
                ("enter", ix),
                Animation::new(Duration::from_millis(total as u64)),
                move |this, time| {
                    let progress = ((time * total - delay) / ENTER_MS).clamp(0., 1.);
                    let eased = 1. - (1. - progress).powi(3);
                    this.opacity(eased).top(px(4. * (1. - eased)))
                },
            )
            .into_any_element()
        });

        let heading = match &self.period {
            Some(period) => format!("{}, {period}", self.title),
            None => self.title.to_string(),
        };
        v_flex()
            .id(self.id)
            .role(Role::List)
            .aria_label(heading)
            .w_full()
            .max_w(px(480.))
            .overflow_hidden()
            .rounded(radius)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .line_height(relative(1.5))
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .justify_between()
                    .gap_3()
                    .h(px(44.))
                    .px(inset)
                    .child(
                        h_flex()
                            .gap_2()
                            .min_w_0()
                            .font_medium()
                            .child(
                                self.icon
                                    .unwrap_or_else(|| Icon::new(IconName::Replace))
                                    .size_4()
                                    .flex_none()
                                    .text_color(quiet),
                            )
                            .child(div().min_w_0().truncate().child(self.title)),
                    )
                    .when_some(self.period, |this, period| {
                        this.child(
                            div()
                                .max_w(relative(0.5))
                                .truncate()
                                .text_caption()
                                .text_color(quiet)
                                .child(period),
                        )
                    }),
            )
            .children(rows)
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_read_in_words() {
        let churn = MetricChange::new("Churn", "Down to 2.1%")
            .tone(Tone::Success)
            .direction(TrendDirection::Down);
        assert_eq!(spoken_change(&churn), "Churn: Down to 2.1%, falling, good");
        assert_eq!(
            spoken_change(&MetricChange::new("Seats", "Same as last week")),
            "Seats: Same as last week, no change"
        );
    }

    #[test]
    fn a_neutral_change_draws_a_dash() {
        let change = MetricChange::new("Seats", "Unchanged at 40");
        assert_eq!(change.arrow(), None);
        // A direction does not turn a neutral change into an arrow.
        assert_eq!(change.direction(TrendDirection::Up).arrow(), None);
    }

    #[test]
    fn the_arrow_defaults_from_the_tone() {
        let good = MetricChange::new("Signups", "Up 8%").tone(Tone::Success);
        let bad = MetricChange::new("Refunds", "Up 3%").tone(Tone::Danger);
        assert_eq!(good.arrow(), Some(TrendDirection::Up));
        assert_eq!(bad.arrow(), Some(TrendDirection::Down));
    }

    #[test]
    fn direction_is_independent_of_tone() {
        let falling_and_good = MetricChange::new("Churn", "Down to 2%")
            .tone(Tone::Success)
            .direction(TrendDirection::Down);
        let rising_and_bad = MetricChange::new("Tickets", "Up 14%")
            .tone(Tone::Danger)
            .direction(TrendDirection::Up);
        assert_eq!(falling_and_good.arrow(), Some(TrendDirection::Down));
        assert_eq!(rising_and_bad.arrow(), Some(TrendDirection::Up));
    }
}
