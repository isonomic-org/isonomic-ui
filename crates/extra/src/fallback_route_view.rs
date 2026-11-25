// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Fallback route view: one request finding a target that answers.
//!
//! A request can go to several targets in a fixed order: models, providers,
//! regions. This view lists them down a rail between the request and the
//! response, each with what happened to it, so it is plain which one answered
//! and why the ones before it did not. Every state is written out, not only
//! coloured.

use std::{rc::Rc, time::Duration};

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, ThemeStyled as _,
    button::Button, h_flex, spinner::Spinner, v_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// What happened to one target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttemptStatus {
    /// Not tried yet.
    Waiting,
    /// Being tried now.
    Trying,
    /// Tried, and it did not answer.
    Failed,
    /// Tried, and it answered.
    Answered,
    /// Never tried, because an earlier target answered.
    Skipped,
}

/// One target of the route, in the order it is tried.
#[derive(Debug, Clone)]
pub struct RouteAttempt {
    name: SharedString,
    provider: Option<SharedString>,
    status: AttemptStatus,
    note: Option<SharedString>,
    elapsed: Option<Duration>,
    cost: Option<SharedString>,
}

impl RouteAttempt {
    /// Create a target that waits for its turn.
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            provider: None,
            status: AttemptStatus::Waiting,
            note: None,
            elapsed: None,
            cost: None,
        }
    }

    /// Say where the target runs.
    pub fn provider(mut self, provider: impl Into<SharedString>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    /// Set what happened to the target.
    pub fn status(mut self, status: AttemptStatus) -> Self {
        self.status = status;
        self
    }

    /// Say why, in place of the plain status words: the error of a failed
    /// attempt, for example.
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// Set how long the attempt took, or has taken so far.
    pub fn elapsed(mut self, elapsed: Duration) -> Self {
        self.elapsed = Some(elapsed);
        self
    }

    /// Set what the attempt cost, already formatted.
    pub fn cost(mut self, cost: impl Into<SharedString>) -> Self {
        self.cost = Some(cost.into());
        self
    }
}

/// Where the whole route stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// Nothing has been tried.
    Idle,
    /// The target at this position is being tried.
    Trying(usize),
    /// The target at this position answered.
    Answered(usize),
    /// Every target was tried and none answered.
    AllFailed,
    /// Some failed and the rest have not been tried yet.
    Between(usize),
}

/// Where the route stands, from the targets' statuses.
fn outcome(statuses: &[AttemptStatus]) -> Outcome {
    if let Some(ix) = statuses
        .iter()
        .position(|status| *status == AttemptStatus::Answered)
    {
        return Outcome::Answered(ix);
    }
    if let Some(ix) = statuses
        .iter()
        .position(|status| *status == AttemptStatus::Trying)
    {
        return Outcome::Trying(ix);
    }
    let failed = statuses
        .iter()
        .filter(|status| **status == AttemptStatus::Failed)
        .count();
    if failed == 0 {
        Outcome::Idle
    } else if failed == statuses.len() {
        Outcome::AllFailed
    } else {
        Outcome::Between(failed)
    }
}

/// A duration in a few characters: "660 ms", "1.6 s", "1 m 05 s".
fn elapsed_words(elapsed: Duration) -> String {
    let millis = elapsed.as_millis();
    if millis < 1000 {
        format!("{millis} ms")
    } else if millis < 60_000 {
        format!("{:.1} s", elapsed.as_secs_f32())
    } else {
        let seconds = elapsed.as_secs();
        format!("{} m {:02} s", seconds / 60, seconds % 60)
    }
}

type RetryHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A request, the targets it tries in order, and the response.
#[derive(IntoElement)]
pub struct FallbackRouteView {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    route: Option<SharedString>,
    attempts: Vec<RouteAttempt>,
    total_cost: Option<SharedString>,
    on_retry: Option<RetryHandler>,
}

impl FallbackRouteView {
    /// Create a view with a heading: what was asked.
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            route: None,
            attempts: Vec::new(),
            total_cost: None,
            on_retry: None,
        }
    }

    /// Name the route the request takes, shown on the request row.
    pub fn route(mut self, route: impl Into<SharedString>) -> Self {
        self.route = Some(route.into());
        self
    }

    /// Add a target after the ones already added.
    pub fn attempt(mut self, attempt: RouteAttempt) -> Self {
        self.attempts.push(attempt);
        self
    }

    /// Set what the whole request has cost, already formatted.
    pub fn total_cost(mut self, cost: impl Into<SharedString>) -> Self {
        self.total_cost = Some(cost.into());
        self
    }

    /// Offer to try again once every target has failed.
    pub fn on_retry(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_retry = Some(Rc::new(handler));
        self
    }
}

impl Styled for FallbackRouteView {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for FallbackRouteView {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.info;
        let success = theme.success;
        let danger = theme.danger;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;

        let statuses: Vec<AttemptStatus> =
            self.attempts.iter().map(|attempt| attempt.status).collect();
        let count = statuses.len();
        let outcome = outcome(&statuses);
        let total: Duration = self
            .attempts
            .iter()
            .filter_map(|attempt| attempt.elapsed)
            .sum();
        // The first target still waiting is the one that goes next.
        let next = statuses
            .iter()
            .position(|status| *status == AttemptStatus::Waiting)
            .filter(|_| !matches!(outcome, Outcome::Answered(_)));

        let (summary, summary_color) = match outcome {
            Outcome::Idle => ("Not started".to_string(), quiet),
            Outcome::Trying(ix) => (format!("Trying {} of {count}", ix + 1), accent),
            Outcome::Between(failed) => (format!("{failed} of {count} failed so far"), quiet),
            Outcome::Answered(ix) => (format!("Answered on try {} of {count}", ix + 1), success),
            Outcome::AllFailed => (format!("All {count} failed"), danger),
        };

        // Every row hangs on the rail by a marker.
        let row =
            |marker: isonomic_ui::AnyElement, joined: bool, content: isonomic_ui::AnyElement| {
                h_flex()
                    .items_stretch()
                    .gap_3()
                    .child(
                        v_flex()
                            .flex_none()
                            .w(px(22.))
                            .items_center()
                            .child(
                                div()
                                    .flex()
                                    .flex_none()
                                    .items_center()
                                    .justify_center()
                                    .size(px(22.))
                                    .child(marker),
                            )
                            .when(joined, |this| this.child(div().flex_1().w(px(1.)).bg(line))),
                    )
                    .child(div().flex_1().min_w_0().pb_3().child(content))
            };
        let end = |icon: IconName, name: &'static str, detail: SharedString, mono: bool| {
            h_flex()
                .gap_2()
                .min_h(px(22.))
                .child(div().flex_none().font_medium().child(name))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(quiet)
                        .when(mono, |this| {
                            this.font_family(cx.theme().mono_font_family.clone())
                        })
                        .child(detail),
                )
                .child(div().flex_1())
                .child(Icon::new(icon).small().flex_none().text_color(quiet))
        };
        let dot = |color| {
            div()
                .size(px(8.))
                .rounded_full_style(cx)
                .bg(color)
                .into_any_element()
        };

        let attempts = self.attempts.iter().enumerate().map(|(ix, attempt)| {
            let (color, words) = match attempt.status {
                AttemptStatus::Waiting if next == Some(ix) => (quiet, "Next in line"),
                AttemptStatus::Waiting => (quiet, "Waiting"),
                AttemptStatus::Trying => (accent, "Trying now"),
                AttemptStatus::Failed => (danger, "Failed"),
                AttemptStatus::Answered => (success, "Answered"),
                AttemptStatus::Skipped => (quiet, "Not reached"),
            };
            let words = attempt.note.clone().unwrap_or_else(|| words.into());
            let rests = matches!(
                attempt.status,
                AttemptStatus::Waiting | AttemptStatus::Skipped
            );
            let marker = match attempt.status {
                AttemptStatus::Trying => Spinner::new().small().color(accent).into_any_element(),
                AttemptStatus::Failed => Icon::new(IconName::CircleX)
                    .small()
                    .text_color(danger)
                    .into_any_element(),
                AttemptStatus::Answered => Icon::new(IconName::CircleCheck)
                    .small()
                    .text_color(success)
                    .into_any_element(),
                AttemptStatus::Waiting | AttemptStatus::Skipped => div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(18.))
                    .rounded_full_style(cx)
                    .border_1()
                    .border_color(line)
                    .text_caption()
                    .text_color(quiet)
                    .child((ix + 1).to_string())
                    .into_any_element(),
            };
            let content = h_flex()
                .items_start()
                .gap_3()
                .px_2p5()
                .py_1p5()
                .rounded(radius)
                .border_1()
                .border_color(if attempt.status == AttemptStatus::Trying {
                    accent.opacity(0.5)
                } else {
                    line
                })
                .when(rests, |this| this.opacity(0.6))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .font_medium()
                                        .child(attempt.name.clone()),
                                )
                                .children(attempt.provider.clone().map(|provider| {
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .text_caption()
                                        .text_color(quiet)
                                        .child(provider)
                                })),
                        )
                        .child(div().truncate().text_caption().text_color(color).child(words)),
                )
                .child(
                    v_flex()
                        .flex_none()
                        .items_end()
                        .text_caption()
                        .child(
                            div()
                                .font_medium()
                                .child(attempt.elapsed.map_or("\u{2013}".into(), elapsed_words)),
                        )
                        .children(
                            attempt
                                .cost
                                .clone()
                                .map(|cost| div().text_color(quiet).child(cost)),
                        ),
                )
                .into_any_element();
            row(marker, true, content)
        });
        let attempts: Vec<_> = attempts.collect();

        let answer: SharedString = match outcome {
            Outcome::Answered(ix) => format!("from {}", self.attempts[ix].name).into(),
            Outcome::AllFailed => "no answer".into(),
            _ => "waiting for an answer".into(),
        };
        let retry = self
            .on_retry
            .clone()
            .filter(|_| outcome == Outcome::AllFailed && count > 0)
            .map(|handler| {
                Button::new((self.id.clone(), "retry"))
                    .outline()
                    .small()
                    .icon(IconName::RotateCw)
                    .label("Try again")
                    .on_click(move |_, window, cx| handler(window, cx))
            });

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(480.))
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .line_height(relative(1.45))
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .gap_3()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(line)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_medium()
                            .child(self.title),
                    )
                    .child(
                        h_flex()
                            .flex_none()
                            .gap_1p5()
                            .px_1p5()
                            .rounded(px(5.))
                            .bg(chip)
                            .text_caption()
                            .child(div().size(px(6.)).rounded_full_style(cx).bg(summary_color))
                            .child(summary),
                    )
                    .when(!total.is_zero(), |this| {
                        this.child(
                            div()
                                .flex_none()
                                .text_caption()
                                .text_color(quiet)
                                .child(elapsed_words(total)),
                        )
                    })
                    .children(
                        self.total_cost
                            .map(|cost| div().flex_none().text_caption().text_color(quiet).child(cost)),
                    ),
            )
            .child(
                v_flex()
                    .px_4()
                    .pt_4()
                    .pb_1()
                    .child(row(
                        dot(quiet),
                        true,
                        end(
                            IconName::ArrowRight,
                            "Request",
                            self.route.unwrap_or_default(),
                            true,
                        )
                        .into_any_element(),
                    ))
                    .children(attempts)
                    .child(row(
                        dot(match outcome {
                            Outcome::Answered(_) => success,
                            Outcome::AllFailed => danger,
                            _ => quiet,
                        }),
                        false,
                        end(IconName::Inbox, "Response", answer, false).into_any_element(),
                    )),
            )
            .children(retry.map(|retry| {
                h_flex()
                    .justify_end()
                    .px_4()
                    .py_2p5()
                    .border_t_1()
                    .border_color(line)
                    .child(retry)
            }))
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use AttemptStatus::{Answered, Failed, Skipped, Trying, Waiting};

    #[test]
    fn the_route_stands_where_its_targets_stand() {
        assert_eq!(outcome(&[Waiting, Waiting]), Outcome::Idle);
        assert_eq!(outcome(&[Failed, Trying, Waiting]), Outcome::Trying(1));
        assert_eq!(outcome(&[Failed, Waiting, Waiting]), Outcome::Between(1));
        assert_eq!(outcome(&[Failed, Answered, Skipped]), Outcome::Answered(1));
        assert_eq!(outcome(&[Failed, Failed]), Outcome::AllFailed);
        assert_eq!(outcome(&[]), Outcome::Idle);
    }

    #[test]
    fn an_answer_counts_even_while_another_target_is_marked_trying() {
        assert_eq!(outcome(&[Trying, Answered]), Outcome::Answered(1));
    }

    #[test]
    fn durations_read_in_the_right_unit() {
        assert_eq!(elapsed_words(Duration::from_millis(660)), "660 ms");
        assert_eq!(elapsed_words(Duration::from_millis(1600)), "1.6 s");
        assert_eq!(elapsed_words(Duration::from_secs(65)), "1 m 05 s");
    }
}
