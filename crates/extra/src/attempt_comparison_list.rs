// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Attempt comparison list: the same task tried several ways at once.
//!
//! Each attempt is a row with what it tries, a bar while it runs, and a score
//! once it is done. The best score is marked and used unless someone picks
//! another one.

use std::{rc::Rc, time::Duration};

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// Where an attempt stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttemptState {
    /// Still working.
    Running,
    /// Finished with a result.
    Done,
    /// Gave up.
    Failed,
}

/// One way of trying the task.
#[derive(Clone)]
pub struct Attempt {
    id: SharedString,
    name: SharedString,
    approach: Option<SharedString>,
    icon: Option<IconName>,
    state: AttemptState,
    progress: f32,
    step: Option<SharedString>,
    score: Option<u32>,
    note: Option<SharedString>,
    took: Option<Duration>,
    cost: Option<f64>,
}

impl Attempt {
    /// Create a running attempt, usually named after what runs it.
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            approach: None,
            icon: None,
            state: AttemptState::Running,
            progress: 0.,
            step: None,
            score: None,
            note: None,
            took: None,
            cost: None,
        }
    }

    /// Say how this attempt goes about the task.
    pub fn approach(mut self, approach: impl Into<SharedString>) -> Self {
        self.approach = Some(approach.into());
        self
    }

    /// Set an icon for the tile. The first letter of the name otherwise.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Set where the attempt stands.
    pub fn state(mut self, state: AttemptState) -> Self {
        self.state = state;
        self
    }

    /// Set how far along it is, from 0 to 1. A failed attempt keeps where it
    /// stopped.
    pub fn progress(mut self, progress: f32) -> Self {
        self.progress = progress.clamp(0., 1.);
        self
    }

    /// Say what it is doing right now.
    pub fn step(mut self, step: impl Into<SharedString>) -> Self {
        self.step = Some(step.into());
        self
    }

    /// Set the score, from 0 to 100, once it is done.
    pub fn score(mut self, score: u32) -> Self {
        self.score = Some(score.min(100));
        self
    }

    /// Set a line about the outcome, or the error.
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// Set how long it took, or has taken so far.
    pub fn took(mut self, took: Duration) -> Self {
        self.took = Some(took);
        self
    }

    /// Set what it cost.
    pub fn cost(mut self, cost: f64) -> Self {
        self.cost = Some(cost);
        self
    }
}

type Format = Rc<dyn Fn(f64) -> String>;
type PickHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// Several attempts at one task, racing as rows, with the best one marked.
#[derive(IntoElement)]
pub struct AttemptComparisonList {
    id: ElementId,
    style: StyleRefinement,
    title: Option<SharedString>,
    task: Option<SharedString>,
    attempts: Vec<Attempt>,
    picked: Option<Option<SharedString>>,
    format_cost: Format,
    on_pick: Option<PickHandler>,
}

impl AttemptComparisonList {
    /// Create a list with no attempts.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: None,
            task: None,
            attempts: Vec::new(),
            picked: None,
            format_cost: Rc::new(|cost| format!("{cost:.2}")),
            on_pick: None,
        }
    }

    /// Set the heading. "Best of N" otherwise.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Say what every attempt tries to do.
    pub fn task(mut self, task: impl Into<SharedString>) -> Self {
        self.task = Some(task.into());
        self
    }

    /// Add an attempt after the ones already added.
    pub fn attempt(mut self, attempt: Attempt) -> Self {
        self.attempts.push(attempt);
        self
    }

    /// Control which attempt is used, by id. Without this, the best
    /// finished attempt is used until another one is picked.
    pub fn picked(mut self, id: Option<SharedString>) -> Self {
        self.picked = Some(id);
        self
    }

    /// Set how costs are written. Two decimals by default.
    pub fn format_cost(mut self, format: impl Fn(f64) -> String + 'static) -> Self {
        self.format_cost = Rc::new(format);
        self
    }

    /// Run when another attempt is picked.
    pub fn on_pick(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_pick = Some(Rc::new(handler));
        self
    }
}

impl Styled for AttemptComparisonList {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// The finished attempt with the highest score. The earlier one wins a tie.
fn best(attempts: &[Attempt]) -> Option<usize> {
    let mut best: Option<(usize, u32)> = None;
    for (ix, attempt) in attempts.iter().enumerate() {
        if attempt.state != AttemptState::Done {
            continue;
        }
        let Some(score) = attempt.score else {
            continue;
        };
        if best.is_none_or(|(_, top)| score > top) {
            best = Some((ix, score));
        }
    }
    best.map(|(ix, _)| ix)
}

/// A duration in a few characters: "42 s", "2 m 06 s".
fn took_words(took: Duration) -> String {
    let seconds = took.as_secs();
    if seconds < 60 {
        format!("{seconds} s")
    } else {
        format!("{} m {:02} s", seconds / 60, seconds % 60)
    }
}

/// The letter on the tile of an attempt without an icon.
fn initial(name: &str) -> String {
    name.chars()
        .find(|ch| ch.is_alphanumeric())
        .map(|ch| ch.to_uppercase().collect())
        .unwrap_or_default()
}

struct ListState {
    picked: Option<SharedString>,
    hovered: Option<usize>,
}

impl RenderOnce for AttemptComparisonList {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
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

        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| ListState {
            picked: None,
            hovered: None,
        });
        let (kept, hovered) = {
            let state = state.read(cx);
            (state.picked.clone(), state.hovered)
        };
        let count = self.attempts.len();
        let best = best(&self.attempts);
        let picked = self.picked.clone().unwrap_or(kept);
        let in_use = picked
            .as_ref()
            .and_then(|id| self.attempts.iter().position(|attempt| attempt.id == *id))
            .or(best);
        let finished = self
            .attempts
            .iter()
            .filter(|attempt| attempt.state != AttemptState::Running)
            .count();
        let spent: f64 = self
            .attempts
            .iter()
            .filter_map(|attempt| attempt.cost)
            .sum();
        let title = self
            .title
            .clone()
            .unwrap_or_else(|| format!("Best of {count}").into());

        let rows = self.attempts.iter().enumerate().map(|(ix, attempt)| {
            let is_used = in_use == Some(ix);
            let is_best = best == Some(ix);
            let steps_back = in_use.is_some() && !is_used && hovered != Some(ix);
            let (bar_color, under, under_color) = match attempt.state {
                AttemptState::Running => (accent, attempt.step.clone(), quiet),
                AttemptState::Done => (success, attempt.note.clone(), quiet),
                AttemptState::Failed => (danger, attempt.note.clone(), danger),
            };
            let bar_share = match attempt.state {
                AttemptState::Done => 1.,
                _ => attempt.progress,
            };
            let mut facts: Vec<String> = Vec::new();
            if let Some(took) = attempt.took {
                facts.push(took_words(took));
            }
            if let Some(cost) = attempt.cost {
                facts.push((self.format_cost)(cost));
            }
            let id = attempt.id.clone();

            h_flex()
                .id(("attempt", ix))
                .gap_3()
                .px_3()
                .py_2p5()
                .rounded(radius)
                .border_1()
                .border_color(if is_used { success.opacity(0.45) } else { line })
                .when(is_used, |this| this.bg(success.opacity(0.07)))
                .when(steps_back, |this| this.opacity(0.6))
                .on_hover({
                    let state = state.clone();
                    move |hovered, _, cx| {
                        state.update(cx, |state, cx| {
                            if *hovered {
                                state.hovered = Some(ix);
                                cx.notify();
                            } else if state.hovered == Some(ix) {
                                state.hovered = None;
                                cx.notify();
                            }
                        })
                    }
                })
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(px(28.))
                        .rounded(radius)
                        .border_1()
                        .border_color(line)
                        .bg(fill)
                        .text_label_small()
                        .text_color(quiet)
                        .map(|this| match attempt.icon.clone() {
                            Some(icon) => this.child(Icon::new(icon).small()),
                            None => this.child(initial(&attempt.name)),
                        }),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_1()
                        .child(
                            h_flex()
                                .gap_1p5()
                                .child(div().flex_none().font_medium().child(attempt.name.clone()))
                                .children(attempt.approach.clone().map(|approach| {
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .text_color(quiet)
                                        .child(format!("\u{b7} {approach}"))
                                })),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_caption()
                                .text_color(under_color)
                                .child(under.unwrap_or_default()),
                        )
                        .child(
                            div().w_full().h(px(4.)).rounded(px(2.)).bg(chip).child(
                                div()
                                    .h_full()
                                    .w(relative(bar_share))
                                    .rounded(px(2.))
                                    .bg(bar_color.opacity(0.7)),
                            ),
                        ),
                )
                .child(
                    v_flex()
                        .flex_none()
                        .w(px(112.))
                        .items_end()
                        .gap_0p5()
                        .child(
                            h_flex()
                                .gap_1p5()
                                .when(is_best, |this| {
                                    this.child(
                                        div()
                                            .px_1p5()
                                            .rounded(px(5.))
                                            .bg(success.opacity(0.14))
                                            .text_label_small()
                                            .child("Best"),
                                    )
                                })
                                .map(|this| match (attempt.state, attempt.score) {
                                    (AttemptState::Failed, _) => this.child(
                                        div().font_medium().text_color(danger).child("Failed"),
                                    ),
                                    (AttemptState::Done, Some(score)) => this.child(
                                        div().text_lg().font_medium().child(score.to_string()),
                                    ),
                                    _ => this.child(div().text_color(quiet).child("\u{2013}")),
                                }),
                        )
                        .child(
                            div()
                                .text_caption()
                                .text_color(quiet)
                                .child(facts.join(", ")),
                        )
                        .map(|this| {
                            if is_used {
                                this.child(
                                    h_flex()
                                        .gap_1()
                                        .text_caption()
                                        .text_color(success)
                                        .child(Icon::new(IconName::Check).xsmall())
                                        .child("In use"),
                                )
                            } else if attempt.state == AttemptState::Done {
                                let state = state.clone();
                                let on_pick = self.on_pick.clone();
                                this.child(
                                    Button::new(("use", ix))
                                        .ghost()
                                        .xsmall()
                                        .label("Use this")
                                        .on_click(move |_, window, cx| {
                                            state.update(cx, |state, cx| {
                                                state.picked = Some(id.clone());
                                                cx.notify();
                                            });
                                            if let Some(handler) = &on_pick {
                                                handler(&id, window, cx);
                                            }
                                        }),
                                )
                            } else {
                                this
                            }
                        }),
                )
        });
        let rows: Vec<_> = rows.collect();

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(640.))
            .gap_2()
            .text_body()
            .line_height(relative(1.45))
            .text_color(ink)
            .child(
                h_flex()
                    .gap_2()
                    .px_1()
                    .child(Icon::new(IconName::Menu).small().text_color(quiet))
                    .child(div().flex_none().font_medium().child(title))
                    .children(
                        self.task
                            .map(|task| div().min_w_0().truncate().text_color(quiet).child(task)),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .flex_none()
                            .text_color(quiet)
                            .child(format!("{finished} of {count} done")),
                    )
                    .child(
                        div()
                            .flex_none()
                            .font_medium()
                            .child((self.format_cost)(spent)),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .p_2()
                    .rounded(radius_lg)
                    .bg(fill)
                    .shadow(resting_shadow(cx))
                    .children(rows),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn done(score: u32) -> Attempt {
        Attempt::new("a", "A")
            .state(AttemptState::Done)
            .score(score)
    }

    #[test]
    fn the_best_is_the_highest_finished_score() {
        let attempts = [done(78), done(94), Attempt::new("c", "C").score(99)];
        assert_eq!(best(&attempts), Some(1));
    }

    #[test]
    fn a_tie_goes_to_the_earlier_attempt() {
        assert_eq!(best(&[done(80), done(80)]), Some(0));
    }

    #[test]
    fn failed_and_running_attempts_are_never_best() {
        let attempts = [
            Attempt::new("a", "A")
                .state(AttemptState::Failed)
                .score(100),
            Attempt::new("b", "B").progress(0.4),
        ];
        assert_eq!(best(&attempts), None);
    }

    #[test]
    fn durations_switch_to_minutes() {
        assert_eq!(took_words(Duration::from_secs(42)), "42 s");
        assert_eq!(took_words(Duration::from_secs(126)), "2 m 06 s");
    }

    #[test]
    fn the_tile_shows_the_first_letter() {
        assert_eq!(initial("ledger bot"), "L");
        assert_eq!(initial(""), "");
    }
}
