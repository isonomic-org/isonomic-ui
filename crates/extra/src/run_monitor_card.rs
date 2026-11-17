// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Run monitor card: one run's current step, time, spend and finished steps, foldable to a bar.
//!
//! Open, the card shows what the run is doing now, how long it has taken,
//! what it has spent against its budget, and the steps it has finished.
//! Folded, it shrinks to one bar with the current step and a progress line,
//! for a corner of the screen. Pause, resume, stop and folding go to the owner.

use std::rc::Rc;

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    Animation, AnimationExt as _, AnyElement, App, ElementId, InteractiveElement as _, IntoElement,
    ParentElement as _, RenderOnce, Role, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Transformation, Window, div, percentage, prelude::FluentBuilder as _,
    px, relative,
};

use crate::common::{Tone, resting_shadow};

/// Where the run is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorState {
    /// Working.
    Running,
    /// Held by someone.
    Paused,
    /// Finished.
    Done,
    /// Stopped with a problem.
    Failed,
}

impl MonitorState {
    fn tone(self) -> Tone {
        match self {
            Self::Running => Tone::Info,
            Self::Paused => Tone::Warning,
            Self::Done => Tone::Success,
            Self::Failed => Tone::Danger,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Paused => "Paused",
            Self::Done => "Done",
            Self::Failed => "Failed",
        }
    }
}

/// A finished step.
#[derive(Debug, Clone, PartialEq)]
pub struct FinishedStep {
    /// What it did.
    pub label: SharedString,
    /// How long it took, in seconds.
    pub secs: u32,
    /// Whether it went well.
    pub ok: bool,
}

impl FinishedStep {
    /// Create a step that went well.
    pub fn new(label: impl Into<SharedString>, secs: u32) -> Self {
        Self {
            label: label.into(),
            secs,
            ok: true,
        }
    }

    /// Mark it as having gone wrong.
    pub fn failed(mut self) -> Self {
        self.ok = false;
        self
    }
}

/// Write seconds as a clock: "42s", "3:05", "1:02:09".
pub fn clock(secs: u32) -> String {
    match secs {
        0..=59 => format!("{secs}s"),
        60..=3599 => format!("{}:{:02}", secs / 60, secs % 60),
        _ => format!("{}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60),
    }
}

/// What share of the run is done, from 0 to 1, when the step count is known.
pub fn run_share(done: usize, planned: Option<usize>, state: MonitorState) -> Option<f32> {
    if state == MonitorState::Done {
        return Some(1.);
    }
    planned
        .filter(|planned| *planned > 0)
        .map(|planned| (done as f32 / planned as f32).min(1.))
}

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

/// One run, watched.
#[derive(IntoElement)]
pub struct RunMonitorCard {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    state: MonitorState,
    current: SharedString,
    elapsed_secs: u32,
    spent_cents: u32,
    budget_cents: Option<u32>,
    steps: Vec<FinishedStep>,
    planned: Option<usize>,
    folded: bool,
    on_fold: Option<Handler>,
    on_pause: Option<Handler>,
    on_stop: Option<Handler>,
}

impl RunMonitorCard {
    /// Create a card for a running run.
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            state: MonitorState::Running,
            current: SharedString::default(),
            elapsed_secs: 0,
            spent_cents: 0,
            budget_cents: None,
            steps: Vec::new(),
            planned: None,
            folded: false,
            on_fold: None,
            on_pause: None,
            on_stop: None,
        }
    }

    /// Set where the run is and what it's doing now.
    pub fn state(mut self, state: MonitorState, current: impl Into<SharedString>) -> Self {
        self.state = state;
        self.current = current.into();
        self
    }

    /// Set the time taken and the spend, in seconds and cents.
    pub fn usage(mut self, elapsed_secs: u32, spent_cents: u32) -> Self {
        self.elapsed_secs = elapsed_secs;
        self.spent_cents = spent_cents;
        self
    }

    /// Set a spending budget, in cents.
    pub fn budget(mut self, cents: u32) -> Self {
        self.budget_cents = Some(cents);
        self
    }

    /// Set the finished steps and, if known, how many are planned in all.
    pub fn steps(mut self, steps: Vec<FinishedStep>, planned: Option<usize>) -> Self {
        self.steps = steps;
        self.planned = planned;
        self
    }

    /// Fold the card to a bar.
    pub fn folded(mut self, folded: bool) -> Self {
        self.folded = folded;
        self
    }

    /// Run when the card is folded or unfolded.
    pub fn on_fold(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_fold = Some(Rc::new(handler));
        self
    }

    /// Run when pause or resume is clicked.
    pub fn on_pause(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_pause = Some(Rc::new(handler));
        self
    }

    /// Run when stop is clicked.
    pub fn on_stop(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_stop = Some(Rc::new(handler));
        self
    }
}

impl Styled for RunMonitorCard {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

fn money(cents: u32) -> String {
    format!("${}.{:02}", cents / 100, cents % 100)
}

impl RenderOnce for RunMonitorCard {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let danger = theme.danger;
        let radius_lg = theme.radius_lg;
        let tone = self.state.tone();
        let tone_color = tone.color(cx);
        let tint = tone.tint(cx);
        let share = run_share(self.steps.len(), self.planned, self.state);
        let over = self.budget_cents.is_some_and(|b| self.spent_cents > b);
        let live = matches!(self.state, MonitorState::Running | MonitorState::Paused);

        let mark: AnyElement = match self.state {
            MonitorState::Running => Icon::new(IconName::LoaderCircle)
                .small()
                .text_color(tone_color)
                .with_animation(
                    ElementId::NamedChild(self.id.clone().into(), "spin".into()),
                    Animation::new(std::time::Duration::from_secs(1)).repeat(),
                    |icon, delta| icon.transform(Transformation::rotate(percentage(delta))),
                )
                .into_any_element(),
            MonitorState::Paused => Icon::new(IconName::Pause)
                .small()
                .text_color(tone_color)
                .into_any_element(),
            MonitorState::Done => Icon::new(IconName::CircleCheck)
                .small()
                .text_color(tone_color)
                .into_any_element(),
            MonitorState::Failed => Icon::new(IconName::CircleX)
                .small()
                .text_color(tone_color)
                .into_any_element(),
        };

        let progress = div().h(px(3.)).w_full().rounded_full().bg(chip).child(
            div()
                .h_full()
                .rounded_full()
                .bg(tone_color)
                .w(relative(share.unwrap_or(0.))),
        );

        let fold_button = {
            let on_fold = self.on_fold.clone();
            Button::new(ElementId::NamedChild(self.id.clone().into(), "fold".into()))
                .xsmall()
                .ghost()
                .icon(if self.folded {
                    IconName::ChevronUp
                } else {
                    IconName::ChevronDown
                })
                .accessibility_label(if self.folded {
                    "Show run details"
                } else {
                    "Fold run to a bar"
                })
                .on_click(move |_, window, cx| {
                    if let Some(on_fold) = &on_fold {
                        on_fold(window, cx);
                    }
                })
        };

        if self.folded {
            return h_flex()
                .id(self.id.clone())
                .w_full()
                .max_w(px(420.))
                .gap_2()
                .pl_3()
                .pr_1()
                .py_1p5()
                .rounded(radius_lg)
                .border_1()
                .border_color(line)
                .bg(fill)
                .text_color(ink)
                .text_sm()
                .shadow(resting_shadow(cx))
                .role(Role::Group)
                .aria_label(format!(
                    "{}, {}, {}",
                    self.title,
                    self.state.word(),
                    self.current
                ))
                .child(mark)
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
                                        .child(self.current.clone()),
                                )
                                .child(
                                    div()
                                        .flex_none()
                                        .text_xs()
                                        .text_color(quiet)
                                        .child(clock(self.elapsed_secs)),
                                ),
                        )
                        .child(progress),
                )
                .child(fold_button)
                .refine_style(&self.style)
                .into_any_element();
        }

        let pause = {
            let on_pause = self.on_pause.clone();
            let paused = self.state == MonitorState::Paused;
            Button::new(ElementId::NamedChild(
                self.id.clone().into(),
                "pause".into(),
            ))
            .small()
            .outline()
            .icon(if paused {
                IconName::Play
            } else {
                IconName::Pause
            })
            .label(if paused { "Resume" } else { "Pause" })
            .on_click(move |_, window, cx| {
                if let Some(on_pause) = &on_pause {
                    on_pause(window, cx);
                }
            })
        };
        let stop = {
            let on_stop = self.on_stop.clone();
            Button::new(ElementId::NamedChild(self.id.clone().into(), "stop".into()))
                .small()
                .ghost()
                .danger()
                .icon(IconName::Square)
                .label("Stop")
                .on_click(move |_, window, cx| {
                    if let Some(on_stop) = &on_stop {
                        on_stop(window, cx);
                    }
                })
        };

        let stat = |label: &'static str, value: String, warn: bool| {
            v_flex()
                .flex_1()
                .min_w(px(80.))
                .child(div().text_xs().text_color(quiet).child(label))
                .child(
                    div()
                        .font_semibold()
                        .when(warn, |this| this.text_color(danger))
                        .child(value),
                )
        };

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(420.))
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .shadow(resting_shadow(cx))
            .role(Role::Group)
            .aria_label(format!("{}, {}", self.title, self.state.word()))
            .child(
                h_flex()
                    .gap_2()
                    .pl_4()
                    .pr_2()
                    .py_2p5()
                    .border_b_1()
                    .border_color(line)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_medium()
                            .child(self.title.clone()),
                    )
                    .child(
                        div()
                            .px_1p5()
                            .rounded(px(4.))
                            .bg(tint)
                            .text_xs()
                            .text_color(tone_color)
                            .child(self.state.word()),
                    )
                    .child(fold_button),
            )
            .child(
                v_flex()
                    .gap_3()
                    .p_4()
                    .child(
                        h_flex()
                            .items_start()
                            .gap_2()
                            .child(div().pt_0p5().child(mark))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .child(div().text_xs().text_color(quiet).child(
                                        match self.state {
                                            MonitorState::Done => "Finished with",
                                            MonitorState::Failed => "Stopped at",
                                            _ => "Now",
                                        },
                                    ))
                                    .child(
                                        div()
                                            .line_height(relative(1.35))
                                            .child(self.current.clone()),
                                    ),
                            ),
                    )
                    .child(progress)
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_3()
                            .child(stat("Time", clock(self.elapsed_secs), false))
                            .child(stat(
                                "Steps",
                                match self.planned {
                                    Some(planned) => format!("{} of {planned}", self.steps.len()),
                                    None => self.steps.len().to_string(),
                                },
                                false,
                            ))
                            .child(stat(
                                "Spent",
                                match self.budget_cents {
                                    Some(budget) => {
                                        format!("{} of {}", money(self.spent_cents), money(budget))
                                    }
                                    None => money(self.spent_cents),
                                },
                                over,
                            )),
                    )
                    .when(!self.steps.is_empty(), |this| {
                        this.child(
                            v_flex()
                                .gap_1()
                                .pt_1()
                                .border_t_1()
                                .border_color(line)
                                .child(
                                    div()
                                        .pt_2()
                                        .text_xs()
                                        .font_medium()
                                        .text_color(quiet)
                                        .child("Finished"),
                                )
                                .children(self.steps.iter().rev().take(5).map(|step| {
                                    let tone = if step.ok { Tone::Success } else { Tone::Danger };
                                    h_flex()
                                        .gap_2()
                                        .text_xs()
                                        .child(
                                            Icon::new(if step.ok {
                                                IconName::Check
                                            } else {
                                                IconName::Close
                                            })
                                            .xsmall()
                                            .text_color(tone.color(cx)),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .truncate()
                                                .child(step.label.clone()),
                                        )
                                        .child(div().text_color(quiet).child(clock(step.secs)))
                                }))
                                .when(self.steps.len() > 5, |this| {
                                    this.child(
                                        div()
                                            .text_xs()
                                            .text_color(quiet)
                                            .child(format!("and {} earlier", self.steps.len() - 5)),
                                    )
                                }),
                        )
                    }),
            )
            .when(live, |this| {
                this.child(
                    h_flex()
                        .gap_2()
                        .justify_end()
                        .px_4()
                        .py_2()
                        .border_t_1()
                        .border_color(line)
                        .child(stop)
                        .child(pause),
                )
            })
            .refine_style(&self.style)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clocks_read_naturally() {
        assert_eq!(clock(42), "42s");
        assert_eq!(clock(185), "3:05");
        assert_eq!(clock(3729), "1:02:09");
    }

    #[test]
    fn share_is_known_only_with_a_plan() {
        assert_eq!(run_share(3, Some(6), MonitorState::Running), Some(0.5));
        assert_eq!(run_share(3, None, MonitorState::Running), None);
        assert_eq!(run_share(3, None, MonitorState::Done), Some(1.));
        assert_eq!(run_share(9, Some(6), MonitorState::Running), Some(1.));
    }
}
