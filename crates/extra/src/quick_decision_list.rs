// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Quick decision list: open questions that each take one of two answers.
//!
//! The heading counts what is still waiting. Picking an answer confirms it for
//! a moment, then the row leaves and the next folded decision opens.

use std::{rc::Rc, time::Duration};

use isonomic_ui::assets::IconName as AssetIcon;
use isonomic_ui::base::Button as Pressable;
use isonomic_ui::base::{Easing, MotionReveal, Presence, transition};
use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, StyledExt as _, ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    h_flex, look, v_flex,
};
use isonomic_ui::{
    AnyElement, App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _,
    RenderOnce, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Task,
    Window, div, prelude::FluentBuilder as _, px, relative,
};

use crate::common::{ease, ease_out, over, panel, shell};

/// How long the confirmation stays before the row leaves.
const CONFIRM: Duration = Duration::from_millis(1100);

/// Which of the two answers was picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecisionAnswer {
    /// The main answer, shown as the filled button.
    Primary,
    /// The other answer, shown as the outlined button.
    Secondary,
}

/// One question with two answers.
#[derive(Debug, Clone)]
pub struct QuickDecision {
    id: SharedString,
    title: SharedString,
    detail: Option<SharedString>,
    primary: SharedString,
    secondary: SharedString,
    collapsed: bool,
}

impl QuickDecision {
    /// Create a decision answered with "Yes" or "No" unless [`Self::answers`] says otherwise.
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            detail: None,
            primary: "Yes".into(),
            secondary: "No".into(),
            collapsed: false,
        }
    }

    /// Short context under the title.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Name the two answers, the main one first.
    pub fn answers(
        mut self,
        primary: impl Into<SharedString>,
        secondary: impl Into<SharedString>,
    ) -> Self {
        self.primary = primary.into();
        self.secondary = secondary.into();
        self
    }

    /// Start folded to one quiet line that opens when pressed.
    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }
}

type DecideHandler = Rc<dyn Fn(&SharedString, DecisionAnswer, &mut Window, &mut App)>;

/// A card of open decisions.
#[derive(IntoElement)]
pub struct QuickDecisionList {
    id: ElementId,
    style: StyleRefinement,
    decisions: Vec<QuickDecision>,
    on_decide: Option<DecideHandler>,
}

impl QuickDecisionList {
    /// Create an empty list.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            decisions: Vec::new(),
            on_decide: None,
        }
    }

    /// Add one decision.
    pub fn decision(mut self, decision: QuickDecision) -> Self {
        self.decisions.push(decision);
        self
    }

    /// Add several decisions.
    pub fn decisions(mut self, decisions: impl IntoIterator<Item = QuickDecision>) -> Self {
        self.decisions.extend(decisions);
        self
    }

    /// Run when an answer is picked, with the decision's id and the answer.
    pub fn on_decide(
        mut self,
        handler: impl Fn(&SharedString, DecisionAnswer, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_decide = Some(Rc::new(handler));
        self
    }
}

impl Styled for QuickDecisionList {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// Where one decision stands.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Row {
    open: bool,
    /// The label of the answer picked, shown as the confirmation.
    answered: Option<SharedString>,
    /// The confirmation is over and the row has left.
    gone: bool,
}

#[derive(Default)]
struct ListState {
    rows: Vec<(SharedString, Row)>,
    /// Timers that end confirmations. Dropped with the list, which cancels them.
    timers: Vec<Task<()>>,
}

impl ListState {
    /// Make sure every decision has a row, keeping the ones already known.
    fn sync(&mut self, decisions: &[QuickDecision]) {
        for decision in decisions {
            if !self.rows.iter().any(|(id, _)| id == &decision.id) {
                let row = Row {
                    open: !decision.collapsed,
                    answered: None,
                    gone: false,
                };
                self.rows.push((decision.id.clone(), row));
            }
        }
    }

    fn row(&mut self, id: &SharedString) -> Option<&mut Row> {
        self.rows
            .iter_mut()
            .find(|(row_id, _)| row_id == id)
            .map(|(_, row)| row)
    }

    /// The confirmation is over: the row leaves and the next folded one opens.
    fn finish(&mut self, id: &SharedString) {
        if let Some(row) = self.row(id) {
            row.gone = true;
        }
        if let Some((_, next)) = self
            .rows
            .iter_mut()
            .find(|(_, row)| !row.open && row.answered.is_none())
        {
            next.open = true;
        }
    }

    fn waiting(&self) -> usize {
        self.rows
            .iter()
            .filter(|(_, row)| row.answered.is_none())
            .count()
    }
}

/// The heading for a number of waiting decisions.
fn heading(waiting: usize) -> SharedString {
    match waiting {
        0 => "All caught up".into(),
        n => format!("{n} need your input").into(),
    }
}

/// What the list says once nothing is waiting.
const EMPTY: &str = "Nothing waiting on you \u{2013} new decisions show up here.";

impl RenderOnce for QuickDecisionList {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let line = look::ink::hairline(cx);
        let warning = theme.warning;
        let success = theme.success;

        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| ListState::default());
        let (rows, waiting) = state.update(cx, |state, _| {
            state.sync(&self.decisions);
            (state.rows.clone(), state.waiting())
        });
        let on_decide = self.on_decide;
        let list_id = self.id.clone();
        let part = |name: String| ElementId::NamedChild(list_id.clone().into(), name.into());

        let mut items: Vec<AnyElement> = Vec::new();
        for decision in self.decisions {
            let Some(row) = rows
                .iter()
                .find(|(id, _)| id == &decision.id)
                .map(|(_, row)| row.clone())
            else {
                continue;
            };
            let id = decision.id.clone();
            let row_id = ElementId::NamedChild(list_id.clone().into(), id.clone());

            // A row that has left folds away before it is dropped.
            let stays = transition(
                part(format!("{id}-stays")),
                if row.gone { 0.0_f32 } else { 1. },
                over(460, ease_out()),
                window,
                cx,
            );
            let visible = transition(
                part(format!("{id}-visible")),
                if row.gone { 0.0_f32 } else { 1. },
                over(300, ease()),
                window,
                cx,
            );
            if row.gone && stays == 0. {
                continue;
            }
            let opened = transition(
                part(format!("{id}-opened")),
                if row.open { 1.0_f32 } else { 0. },
                over(420, ease_out()),
                window,
                cx,
            );
            let first = items.is_empty();

            let title: AnyElement = if row.open {
                div()
                    .font_medium()
                    .child(decision.title.clone())
                    .into_any_element()
            } else {
                // Folded: one quiet line that opens the decision.
                let focus_handle = window
                    .use_keyed_state((row_id.clone(), "focus"), cx, |_, cx| cx.focus_handle())
                    .read(cx)
                    .clone();
                let is_focused = focus_handle.is_focused(window);
                let state = state.clone();
                let id = id.clone();
                Pressable::new(row_id.clone())
                    .accessibility_label(decision.title.clone())
                    .aria_expanded(false)
                    .track_focus(&focus_handle)
                    .on_click(move |_, _, cx| {
                        state.update(cx, |state, cx| {
                            if let Some(row) = state.row(&id) {
                                row.open = true;
                            }
                            cx.notify();
                        })
                    })
                    .justify_start()
                    .w_full()
                    .text_size(px(16.))
                    .line_height(px(24.))
                    .text_color(quiet)
                    .hover(|style| style.text_color(ink))
                    .when(is_focused, |this| this.focus_ring_style(window, cx))
                    .child(div().min_w_0().truncate().child(decision.title.clone()))
                    .into_any_element()
            };

            let body = (row.open || opened > 0.).then(|| {
                let actions = match row.answered.clone() {
                    Some(answer) => {
                        let entered = Presence::new(part(format!("{id}-answered")), true)
                            .transition(over(360, Easing::Linear))
                            .sample(window, cx)
                            .progress;
                        h_flex()
                            .h(px(32.))
                            .gap_1p5()
                            .opacity(entered)
                            .text_color(success)
                            .child(Icon::new(AssetIcon::CheckmarkFilled).size(px(16.)))
                            .child(answer)
                            .into_any_element()
                    }
                    None => {
                        let pick = |answer: DecisionAnswer, label: SharedString| {
                            let state = state.clone();
                            let on_decide = on_decide.clone();
                            let id = id.clone();
                            move |_: &isonomic_ui::ClickEvent, window: &mut Window, cx: &mut App| {
                                state.update(cx, |state, cx| {
                                    if let Some(row) = state.row(&id) {
                                        row.answered = Some(label.clone());
                                    }
                                    let id = id.clone();
                                    let timer = cx.spawn(async move |this, cx| {
                                        cx.background_executor().timer(CONFIRM).await;
                                        _ = this.update(cx, |state, cx| {
                                            state.finish(&id);
                                            cx.notify();
                                        });
                                    });
                                    state.timers.push(timer);
                                    cx.notify();
                                });
                                if let Some(on_decide) = &on_decide {
                                    on_decide(&id, answer, window, cx);
                                }
                            }
                        };
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new((row_id.clone(), "primary"))
                                    .primary()
                                    .h(px(32.))
                                    .text_size(px(14.))
                                    .label(decision.primary.clone())
                                    .on_click(pick(DecisionAnswer::Primary, decision.primary)),
                            )
                            .child(
                                Button::new((row_id.clone(), "secondary"))
                                    .outline()
                                    .h(px(32.))
                                    .text_size(px(14.))
                                    .label(decision.secondary.clone())
                                    .on_click(pick(DecisionAnswer::Secondary, decision.secondary)),
                            )
                            .into_any_element()
                    }
                };
                MotionReveal::new(
                    part(format!("{id}-body")),
                    opened,
                    v_flex()
                        .w_full()
                        .opacity(opened)
                        .when_some(decision.detail, |this, detail| {
                            this.child(div().pt_0p5().text_color(quiet).child(detail))
                        })
                        .child(div().pt_3().child(actions))
                        .into_any_element(),
                )
            });

            let content = v_flex()
                .w_full()
                .px_4()
                .py_3()
                .when(!first, |this| this.border_t_1().border_color(line))
                .opacity(visible)
                .child(title)
                .children(body);
            // Only a row on its way out needs measuring; the rest lay out as is.
            items.push(if row.gone {
                MotionReveal::new(part(format!("{id}-row")), stays, content.into_any_element())
                    .into_any_element()
            } else {
                content.into_any_element()
            });
        }
        let is_empty = items.is_empty();

        let caught_up = transition(
            part("caught-up".into()),
            if waiting == 0 { 1.0_f32 } else { 0. },
            over(300, ease()),
            window,
            cx,
        );
        let head_color = look::mix(warning, success, caught_up);
        let empty = Presence::new(part("empty".into()), is_empty)
            .transition(over(420, ease()).delay(Duration::from_millis(300)))
            .sample(window, cx);

        shell(cx)
            .id(self.id)
            .max_w(px(440.))
            .text_color(ink)
            .text_body()
            .line_height(relative(1.5))
            .child(
                h_flex()
                    .gap_2()
                    .h(px(36.))
                    .px(px(10.))
                    .mb(px(6.))
                    .font_medium()
                    .text_color(head_color)
                    .child(
                        Icon::new(if waiting > 0 {
                            AssetIcon::WarningAltFilled
                        } else {
                            AssetIcon::CheckmarkFilled
                        })
                        .size(px(16.)),
                    )
                    .child(heading(waiting)),
            )
            .child(panel(cx).overflow_hidden().children(items).when(
                is_empty && empty.should_render(),
                |this| {
                    this.child(
                        div()
                            .px_4()
                            .py_3()
                            .opacity(empty.progress)
                            .text_color(quiet)
                            .child(EMPTY),
                    )
                },
            ))
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(decisions: &[QuickDecision]) -> ListState {
        let mut state = ListState::default();
        state.sync(decisions);
        state
    }

    #[test]
    fn headings_count_what_is_waiting() {
        assert_eq!(heading(0).as_ref(), "All caught up");
        assert_eq!(heading(1).as_ref(), "1 need your input");
        assert_eq!(heading(12).as_ref(), "12 need your input");
    }

    #[test]
    fn an_answered_decision_stops_waiting_before_it_leaves() {
        let mut state = state(&[QuickDecision::new("a", "A"), QuickDecision::new("b", "B")]);
        state.row(&"a".into()).unwrap().answered = Some("Yes".into());
        assert_eq!(state.waiting(), 1);
        assert!(!state.rows[0].1.gone);
    }

    #[test]
    fn finishing_opens_the_next_folded_decision() {
        let mut state = state(&[
            QuickDecision::new("a", "A"),
            QuickDecision::new("b", "B").collapsed(true),
            QuickDecision::new("c", "C").collapsed(true),
        ]);
        state.row(&"a".into()).unwrap().answered = Some("Yes".into());
        state.finish(&"a".into());
        assert!(state.rows[0].1.gone);
        assert!(state.rows[1].1.open);
        assert!(!state.rows[2].1.open);
    }

    #[test]
    fn syncing_again_keeps_what_was_answered() {
        let decisions = [QuickDecision::new("a", "A")];
        let mut state = state(&decisions);
        state.row(&"a".into()).unwrap().answered = Some("No".into());
        state.sync(&decisions);
        assert_eq!(state.rows.len(), 1);
        assert_eq!(state.waiting(), 0);
    }
}
