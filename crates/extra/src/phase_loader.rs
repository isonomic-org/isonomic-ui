// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Phase loader: a small pill that says what is being worked on right now.
//!
//! A spinner sits next to a label that walks through phases, either on a timer
//! or driven by the caller. When the work is done the spinner becomes a check
//! and the label settles on a closing line, by default how long it took.

use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable, Size, StyledExt as _, h_flex, shimmer::ShimmerText,
    spinner::Spinner,
};
use isonomic_ui::{
    Animation, AnimationExt as _, App, ElementId, InteractiveElement as _, IntoElement,
    ParentElement as _, RenderOnce, Role, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Task, Window, div, prelude::FluentBuilder as _, px,
};

use crate::common::resting_shadow;

const DEFAULT_INTERVAL: Duration = Duration::from_millis(2200);
const SHORTEST_INTERVAL: Duration = Duration::from_millis(200);
const ROLL: Duration = Duration::from_millis(360);

/// How the loader is framed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum PhaseLoaderVariant {
    /// A bordered capsule that stands on its own.
    #[default]
    Pill,
    /// Only the spinner and the label, for use inside a message or next to a button.
    Plain,
}

#[derive(Clone)]
enum DoneLabel {
    Text(SharedString),
    Elapsed(Rc<dyn Fn(Duration) -> SharedString>),
}

/// A working indicator with labelled phases and a finished state.
#[derive(IntoElement)]
pub struct PhaseLoader {
    id: ElementId,
    style: StyleRefinement,
    phases: Vec<SharedString>,
    interval: Duration,
    phase: Option<usize>,
    looping: bool,
    done: bool,
    done_label: DoneLabel,
    size: Size,
    variant: PhaseLoaderVariant,
}

impl PhaseLoader {
    /// Create a loader. With no phases it shows "Working".
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            phases: Vec::new(),
            interval: DEFAULT_INTERVAL,
            phase: None,
            looping: false,
            done: false,
            done_label: DoneLabel::Elapsed(Rc::new(|elapsed| {
                format!("Done in {}", format_elapsed(elapsed)).into()
            })),
            size: Size::Medium,
            variant: PhaseLoaderVariant::Pill,
        }
    }

    /// The labels to walk through, one after another. Keep them to a few words.
    pub fn phases(mut self, phases: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.phases = phases.into_iter().map(Into::into).collect();
        self
    }

    /// Time spent on each phase when the loader advances by itself.
    pub fn interval(mut self, interval: Duration) -> Self {
        self.interval = interval.max(SHORTEST_INTERVAL);
        self
    }

    /// Show this phase and stop the timer. Use it to follow real events, such
    /// as a tool call starting or the first result arriving.
    pub fn phase(mut self, phase: usize) -> Self {
        self.phase = Some(phase);
        self
    }

    /// Start over after the last phase instead of holding it.
    pub fn looping(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }

    /// Mark the work finished: the spinner becomes a check and the label the
    /// done label. Setting it back to `false` starts over.
    pub fn done(mut self, done: bool) -> Self {
        self.done = done;
        self
    }

    /// The label once done.
    pub fn done_label(mut self, label: impl Into<SharedString>) -> Self {
        self.done_label = DoneLabel::Text(label.into());
        self
    }

    /// Build the label once done from how long the loader ran.
    pub fn done_label_with(mut self, label: impl Fn(Duration) -> SharedString + 'static) -> Self {
        self.done_label = DoneLabel::Elapsed(Rc::new(label));
        self
    }

    /// Choose between the capsule and the bare spinner and label.
    pub fn variant(mut self, variant: PhaseLoaderVariant) -> Self {
        self.variant = variant;
        self
    }
}

impl Sizable for PhaseLoader {
    /// `Small` and below give the compact loader, anything else the regular one.
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl Styled for PhaseLoader {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// Write a duration the way a person would say it: `42s`, `1m 15s`, `2h 05m`, `3d 4h`.
pub fn format_elapsed(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs().max(1);
    let minutes = seconds / 60;
    let hours = minutes / 60;
    if seconds < 60 {
        format!("{seconds}s")
    } else if minutes < 60 {
        format!("{minutes}m {}s", seconds % 60)
    } else if hours < 24 {
        format!("{hours}h {:02}m", minutes % 60)
    } else {
        format!("{}d {}h", hours / 24, hours % 24)
    }
}

/// The step after `step`, holding on the last phase unless looping.
fn next_step(step: usize, phases: usize, looping: bool) -> usize {
    if step + 1 < phases {
        step + 1
    } else if looping {
        0
    } else {
        step
    }
}

/// What the timer was started with, so it restarts only when one of these changes.
type Ticking = (Duration, usize, bool);

struct LoaderState {
    started: Instant,
    step: usize,
    /// Frozen when the work finishes, so the done label does not keep counting.
    elapsed: Option<Duration>,
    shown: SharedString,
    leaving: Option<SharedString>,
    /// Counts label changes. It keys the roll animations so each change replays them.
    changes: usize,
    ticking: Option<Ticking>,
    _timer: Option<Task<()>>,
}

impl RenderOnce for PhaseLoader {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let accent = theme.info;
        let fill = theme.background;
        let line = theme.border;
        let radius = theme.radius;
        let animate = !cx.reduce_motion();

        let phases = if self.phases.is_empty() {
            vec![SharedString::from("Working")]
        } else {
            self.phases
        };
        let done = self.done;
        let controlled = self.phase;
        let done_label = self.done_label;
        let ticking: Option<Ticking> = (!done && controlled.is_none() && phases.len() > 1)
            .then_some((self.interval, phases.len(), self.looping));

        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| LoaderState {
            started: Instant::now(),
            step: 0,
            elapsed: None,
            shown: SharedString::default(),
            leaving: None,
            changes: 0,
            ticking: None,
            _timer: None,
        });
        state.update(cx, |state, cx| {
            if done && state.elapsed.is_none() {
                state.elapsed = Some(state.started.elapsed());
            } else if !done && state.elapsed.is_some() {
                // Work started again: restart the clock and the phases.
                state.elapsed = None;
                state.started = Instant::now();
                state.step = 0;
            }

            if state.ticking != ticking {
                state.ticking = ticking;
                state._timer = ticking.map(|(interval, phases, looping)| {
                    cx.spawn(async move |this, cx| {
                        loop {
                            cx.background_executor().timer(interval).await;
                            let advanced = this.update(cx, |state, cx| {
                                state.step = next_step(state.step, phases, looping);
                                cx.notify();
                            });
                            if advanced.is_err() {
                                break;
                            }
                        }
                    })
                });
            }

            let index = controlled.unwrap_or(state.step).min(phases.len() - 1);
            let label = match (&done_label, state.elapsed) {
                (DoneLabel::Text(text), Some(_)) => text.clone(),
                (DoneLabel::Elapsed(build), Some(elapsed)) => build(elapsed),
                (_, None) => phases[index].clone(),
            };
            if label != state.shown {
                let previous = std::mem::replace(&mut state.shown, label);
                // The very first label has nothing to roll out of.
                if !previous.is_empty() {
                    state.leaving = Some(previous);
                    state.changes += 1;
                }
            }
        });
        let (shown, leaving, changes) = {
            let state = state.read(cx);
            (state.shown.clone(), state.leaving.clone(), state.changes)
        };

        let compact = matches!(self.size, Size::XSmall | Size::Small);
        let (height, line_height, icon_size) = if compact {
            (px(26.), px(16.), Size::XSmall)
        } else {
            (px(32.), px(20.), Size::Small)
        };
        let travel = line_height * 0.7;

        let icon = if done {
            let check = Icon::new(IconName::Check)
                .with_size(icon_size)
                .text_color(accent);
            if animate {
                check
                    .with_animation("check", Animation::new(ROLL), |this, time| {
                        this.opacity(time)
                    })
                    .into_any_element()
            } else {
                check.into_any_element()
            }
        } else {
            Spinner::new()
                .with_size(icon_size)
                .color(accent)
                .into_any_element()
        };

        // The label that is arriving sets the width. While work is going on
        // a band of light sweeps over it.
        let arriving = if !done && animate {
            ShimmerText::new(shown.clone())
                .id(("shine", changes))
                .into_any_element()
        } else {
            shown.clone().into_any_element()
        };
        let arriving = div().relative().child(arriving);
        let arriving = if animate && changes > 0 {
            arriving
                .with_animation(("in", changes), Animation::new(ROLL), move |this, time| {
                    let eased = 1. - (1. - time).powi(3);
                    this.opacity(eased).top(travel * (1. - eased))
                })
                .into_any_element()
        } else {
            arriving.into_any_element()
        };
        // The label that is leaving rolls up and out over the arriving one.
        let leaving = leaving.filter(|_| animate && changes > 0).map(|text| {
            div().absolute().left_0().child(text).with_animation(
                ("out", changes),
                Animation::new(ROLL),
                move |this, time| {
                    let eased = 1. - (1. - time).powi(3);
                    this.opacity(1. - eased).top(-travel * eased)
                },
            )
        });

        h_flex()
            .id(self.id)
            .role(Role::Status)
            .aria_label(shown)
            .flex_none()
            .max_w_full()
            .font_medium()
            .line_height(line_height)
            .text_color(if done { ink } else { quiet })
            .map(|this| {
                if compact {
                    this.text_xs()
                } else {
                    this.text_sm()
                }
            })
            .map(|this| {
                if compact {
                    this.gap(px(5.))
                } else {
                    this.gap(px(6.))
                }
            })
            .when(self.variant == PhaseLoaderVariant::Pill, |this| {
                this.h(height)
                    .rounded(radius)
                    .border_1()
                    .border_color(line)
                    .bg(fill)
                    .shadow(resting_shadow(cx))
                    .map(|this| {
                        if compact {
                            this.pl(px(6.)).pr(px(8.))
                        } else {
                            this.pl(px(8.)).pr(px(10.))
                        }
                    })
            })
            .child(div().flex_none().child(icon))
            .child(
                div()
                    .relative()
                    .min_w_0()
                    .h(line_height)
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(arriving)
                    .children(leaving),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elapsed_time_reads_like_speech() {
        assert_eq!(format_elapsed(Duration::from_secs(42)), "42s");
        assert_eq!(format_elapsed(Duration::from_secs(75)), "1m 15s");
        assert_eq!(
            format_elapsed(Duration::from_secs(2 * 3600 + 5 * 60)),
            "2h 05m"
        );
        assert_eq!(format_elapsed(Duration::from_secs(76 * 3600)), "3d 4h");
    }

    #[test]
    fn an_instant_finish_still_counts_as_a_second() {
        assert_eq!(format_elapsed(Duration::from_millis(120)), "1s");
    }

    #[test]
    fn the_last_phase_holds_unless_looping() {
        assert_eq!(next_step(0, 3, false), 1);
        assert_eq!(next_step(2, 3, false), 2);
        assert_eq!(next_step(2, 3, true), 0);
    }

    #[test]
    fn intervals_have_a_floor() {
        let loader = PhaseLoader::new("loader").interval(Duration::ZERO);
        assert_eq!(loader.interval, SHORTEST_INTERVAL);
    }
}
