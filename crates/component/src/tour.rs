// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! A product tour: a short sequence of cards, each pointing at a part of the
//! interface, with Back, Next and Skip.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use gpui::{
    Anchor, AnyElement, App, Bounds, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement, Pixels, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, Styled, Window, anchored, canvas, deferred, div,
    point, prelude::FluentBuilder as _, px,
};
use smallvec::SmallVec;

use crate::{
    ActiveTheme as _, Sizable as _, StyledExt as _, ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};

/// One card of a tour.
#[derive(Clone, Debug, PartialEq)]
pub struct TourStep {
    target: SharedString,
    title: SharedString,
    body: SharedString,
}

impl TourStep {
    /// A card that points at the [`TourTarget`] named `target`.
    pub fn new(
        target: impl Into<SharedString>,
        title: impl Into<SharedString>,
        body: impl Into<SharedString>,
    ) -> Self {
        Self {
            target: target.into(),
            title: title.into(),
            body: body.into(),
        }
    }
}

/// What a tour reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TourEvent {
    /// The tour moved to the step at this index.
    Step(usize),
    /// The person went through every step.
    Finished,
    /// The person left early, with Skip or Escape, on the step at this index.
    Skipped(usize),
}

/// The state of a tour: its steps, which one shows, and where each target
/// was last drawn.
pub struct TourState {
    steps: Vec<TourStep>,
    current: Option<usize>,
    targets: Rc<RefCell<HashMap<SharedString, Bounds<Pixels>>>>,
    focus_handle: FocusHandle,
}

impl EventEmitter<TourEvent> for TourState {}

impl TourState {
    /// A tour of these steps, not started.
    pub fn new(steps: Vec<TourStep>, cx: &mut Context<Self>) -> Self {
        Self {
            steps,
            current: None,
            targets: Rc::default(),
            focus_handle: cx.focus_handle(),
        }
    }

    /// The step showing, if the tour is running.
    pub fn current(&self) -> Option<usize> {
        self.current
    }

    /// The number of steps.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Whether the tour has no steps.
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Show the first step and move focus to its card.
    pub fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.go_to(0, window, cx);
    }

    /// Show the step at `index`.
    pub fn go_to(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.steps.len() {
            return;
        }
        self.current = Some(index);
        self.focus_handle.focus(window, cx);
        cx.emit(TourEvent::Step(index));
        cx.notify();
    }

    /// Show the next step, or finish after the last one.
    pub fn next(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.current {
            Some(ix) if ix + 1 < self.steps.len() => self.go_to(ix + 1, window, cx),
            Some(_) => {
                self.current = None;
                cx.emit(TourEvent::Finished);
                cx.notify();
            }
            None => {}
        }
    }

    /// Show the step before.
    pub fn back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self.current.filter(|ix| *ix > 0) {
            self.go_to(ix - 1, window, cx);
        }
    }

    /// End the tour early.
    pub fn skip(&mut self, cx: &mut Context<Self>) {
        if let Some(ix) = self.current.take() {
            cx.emit(TourEvent::Skipped(ix));
            cx.notify();
        }
    }

    fn current_step(&self) -> Option<(usize, &TourStep)> {
        self.current
            .and_then(|ix| self.steps.get(ix).map(|step| (ix, step)))
    }
}

impl Focusable for TourState {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// Marks a part of the interface a tour can point at. It draws a ring
/// around its content while its step shows.
#[derive(IntoElement)]
pub struct TourTarget {
    state: Entity<TourState>,
    name: SharedString,
    children: SmallVec<[AnyElement; 1]>,
}

impl TourTarget {
    /// A target that [`TourStep`]s name as `name`.
    pub fn new(state: &Entity<TourState>, name: impl Into<SharedString>) -> Self {
        Self {
            state: state.clone(),
            name: name.into(),
            children: SmallVec::new(),
        }
    }
}

impl ParentElement for TourTarget {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for TourTarget {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let active = state
            .current_step()
            .is_some_and(|(_, step)| step.target == self.name);
        let targets = state.targets.clone();
        let name = self.name.clone();
        let ring = cx.theme().ring;
        let radius = cx.theme().radius;
        div()
            .relative()
            .children(self.children)
            .child(
                canvas(
                    move |bounds, window, _| {
                        let previous = targets.borrow_mut().insert(name.clone(), bounds);
                        // The card is placed from the bounds of the frame before,
                        // so draw again once a target moves.
                        if previous != Some(bounds) {
                            window.refresh();
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            .when(active, |this| {
                this.child(
                    div()
                        .absolute()
                        .top(px(-4.))
                        .left(px(-4.))
                        .right(px(-4.))
                        .bottom(px(-4.))
                        .border_2()
                        .border_color(ring)
                        .rounded(radius + px(4.)),
                )
            })
    }
}

/// The card of the step showing, placed under its target. Put one in the
/// view that holds the targets; it draws nothing while the tour is not
/// running.
#[derive(IntoElement)]
pub struct Tour {
    state: Entity<TourState>,
}

impl Tour {
    /// The card for this tour.
    pub fn new(state: &Entity<TourState>) -> Self {
        Self {
            state: state.clone(),
        }
    }
}

impl RenderOnce for Tour {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let Some((ix, step)) = state.current_step() else {
            return div().into_any_element();
        };
        let step = step.clone();
        let count = state.len();
        let last = ix + 1 == count;
        let target = state.targets.borrow().get(&step.target).copied();
        let focus = state.focus_handle.clone();
        let entity = self.state.clone();
        let theme = cx.theme();
        let position = match target {
            Some(bounds) => point(bounds.left(), bounds.bottom() + px(12.)),
            None => point(px(24.), px(24.)),
        };

        deferred(
            anchored()
                .position(position)
                .anchor(Anchor::TopLeft)
                .snap_to_window_with_margin(px(8.))
                .child(
                    v_flex()
                        .id("tour-card")
                        .track_focus(&focus)
                        .role(Role::Dialog)
                        .aria_label(format!("{}, step {} of {count}", step.title, ix + 1))
                        .aria_description(step.body.clone())
                        .w(px(300.))
                        .gap_2()
                        .p_4()
                        .popover_style(cx)
                        .occlude()
                        .on_key_down({
                            let entity = entity.clone();
                            move |event: &KeyDownEvent, window, cx| {
                                match event.keystroke.key.as_str() {
                                    "escape" => entity.update(cx, |s, cx| s.skip(cx)),
                                    "right" => entity.update(cx, |s, cx| s.next(window, cx)),
                                    "left" => entity.update(cx, |s, cx| s.back(window, cx)),
                                    _ => return,
                                }
                                cx.stop_propagation();
                            }
                        })
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(format!("{} of {count}", ix + 1)),
                        )
                        .child(div().text_sm().font_semibold().child(step.title))
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child(step.body),
                        )
                        .child(
                            h_flex()
                                .pt_2()
                                .gap_2()
                                .justify_between()
                                .child(
                                    Button::new("tour-skip")
                                        .ghost()
                                        .small()
                                        .label(if last { "Close" } else { "Skip tour" })
                                        .on_click({
                                            let entity = entity.clone();
                                            move |_, _, cx| entity.update(cx, |s, cx| s.skip(cx))
                                        }),
                                )
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .when(ix > 0, |this| {
                                            this.child(
                                                Button::new("tour-back")
                                                    .outline()
                                                    .small()
                                                    .label("Back")
                                                    .on_click({
                                                        let entity = entity.clone();
                                                        move |_, window, cx| {
                                                            entity.update(cx, |s, cx| {
                                                                s.back(window, cx)
                                                            })
                                                        }
                                                    }),
                                            )
                                        })
                                        .child(
                                            Button::new("tour-next")
                                                .primary()
                                                .small()
                                                .label(if last { "Done" } else { "Next" })
                                                .on_click(move |_, window, cx| {
                                                    entity.update(cx, |s, cx| s.next(window, cx))
                                                }),
                                        ),
                                ),
                        ),
                ),
        )
        .with_priority(2)
        .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{AppContext as _, TestAppContext};
    use std::cell::RefCell;

    fn steps() -> Vec<TourStep> {
        vec![
            TourStep::new("search", "Search", "Find anything."),
            TourStep::new("new", "New chat", "Start fresh."),
            TourStep::new("settings", "Settings", "Make it yours."),
        ]
    }

    #[gpui::test]
    fn walks_forward_and_back_then_finishes(cx: &mut TestAppContext) {
        cx.update(crate::init);
        let events = Rc::new(RefCell::new(Vec::new()));
        let window = cx.add_empty_window();
        let state = window.update(|_, cx| cx.new(|cx| TourState::new(steps(), cx)));
        window.update({
            let events = events.clone();
            let state = state.clone();
            move |_, cx| {
                cx.subscribe(&state, move |_, event: &TourEvent, _| {
                    events.borrow_mut().push(event.clone())
                })
                .detach()
            }
        });
        window.update(|window, cx| {
            state.update(cx, |s, cx| {
                s.start(window, cx);
                s.next(window, cx);
                s.back(window, cx);
                s.next(window, cx);
                s.next(window, cx);
                s.next(window, cx);
            })
        });
        assert_eq!(state.read_with(window, |s, _| s.current()), None);
        assert_eq!(
            events.borrow().as_slice(),
            &[
                TourEvent::Step(0),
                TourEvent::Step(1),
                TourEvent::Step(0),
                TourEvent::Step(1),
                TourEvent::Step(2),
                TourEvent::Finished,
            ]
        );
    }

    #[gpui::test]
    fn skip_reports_the_step(cx: &mut TestAppContext) {
        cx.update(crate::init);
        let window = cx.add_empty_window();
        let state = window.update(|_, cx| cx.new(|cx| TourState::new(steps(), cx)));
        let events = Rc::new(RefCell::new(Vec::new()));
        window.update({
            let events = events.clone();
            let state = state.clone();
            move |_, cx| {
                cx.subscribe(&state, move |_, event: &TourEvent, _| {
                    events.borrow_mut().push(event.clone())
                })
                .detach()
            }
        });
        window.update(|window, cx| {
            state.update(cx, |s, cx| {
                s.start(window, cx);
                s.next(window, cx);
                s.skip(cx);
                s.back(window, cx);
            })
        });
        assert_eq!(state.read_with(window, |s, _| s.current()), None);
        assert_eq!(events.borrow().last(), Some(&TourEvent::Skipped(1)));
    }
}
