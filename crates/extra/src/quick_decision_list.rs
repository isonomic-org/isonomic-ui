//! Quick decision list: open questions that each take one of two answers.
//!
//! The heading counts what is still waiting. Picking an answer confirms it for
//! a moment, then the row leaves and the next folded decision opens.

use std::{rc::Rc, time::Duration};

use isonomic_ui::base::Button as Pressable;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Task, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

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
        0 => "All answered".into(),
        1 => "1 decision waiting".into(),
        n => format!("{n} decisions waiting").into(),
    }
}

impl RenderOnce for QuickDecisionList {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let radius = theme.radius_lg;
        let warning = theme.warning;
        let success = theme.success;

        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| ListState::default());
        let (rows, waiting) = state.update(cx, |state, _| {
            state.sync(&self.decisions);
            (state.rows.clone(), state.waiting())
        });
        let on_decide = self.on_decide;
        let list_id = self.id.clone();

        let items = self.decisions.into_iter().filter_map(|decision| {
            let row = rows
                .iter()
                .find(|(id, _)| id == &decision.id)
                .map(|(_, row)| row.clone())?;
            if row.gone {
                return None;
            }
            let id = decision.id.clone();
            let row_id = ElementId::NamedChild(list_id.clone().into(), id.clone());

            if !row.open {
                // Folded: one quiet line that opens the decision.
                let focus_handle = window
                    .use_keyed_state((row_id.clone(), "focus"), cx, |_, cx| cx.focus_handle())
                    .read(cx)
                    .clone();
                let is_focused = focus_handle.is_focused(window);
                let state = state.clone();
                return Some(
                    Pressable::new(row_id)
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
                        .px_4()
                        .py_3()
                        .border_t_1()
                        .border_color(line)
                        .line_height(relative(1.5))
                        .text_color(quiet)
                        .cursor_pointer()
                        .hover(|style| style.text_color(ink))
                        .when(is_focused, |this| this.focus_ring_style(window, cx))
                        .child(div().min_w_0().truncate().child(decision.title))
                        .into_any_element(),
                );
            }

            let actions = match row.answered {
                Some(answer) => h_flex()
                    .h(px(28.))
                    .gap_1p5()
                    .font_medium()
                    .text_color(success)
                    .child(Icon::new(IconName::CircleCheck).small())
                    .child(answer)
                    .into_any_element(),
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
                                .small()
                                .label(decision.primary.clone())
                                .on_click(pick(DecisionAnswer::Primary, decision.primary)),
                        )
                        .child(
                            Button::new((row_id, "secondary"))
                                .outline()
                                .small()
                                .label(decision.secondary.clone())
                                .on_click(pick(DecisionAnswer::Secondary, decision.secondary)),
                        )
                        .into_any_element()
                }
            };

            Some(
                v_flex()
                    .px_4()
                    .py_3()
                    .gap_0p5()
                    .border_t_1()
                    .border_color(line)
                    .child(div().font_medium().child(decision.title))
                    .when_some(decision.detail, |this, detail| {
                        this.child(div().text_color(quiet).child(detail))
                    })
                    .child(div().mt_2().child(actions))
                    .into_any_element(),
            )
        });
        let items: Vec<_> = items.collect();
        let is_empty = items.is_empty();

        v_flex()
            .id(self.id)
            .w_full()
            .max_w(px(440.))
            .overflow_hidden()
            .rounded(radius)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .line_height(relative(1.5))
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .gap_2()
                    .h(px(44.))
                    .px_4()
                    .font_medium()
                    .child(if waiting > 0 {
                        Icon::new(IconName::TriangleAlert)
                            .small()
                            .text_color(warning)
                    } else {
                        Icon::new(IconName::CircleCheck).small().text_color(success)
                    })
                    .child(heading(waiting)),
            )
            .children(items)
            .when(is_empty, |this| {
                this.child(
                    div()
                        .px_4()
                        .py_3()
                        .border_t_1()
                        .border_color(line)
                        .text_color(quiet)
                        .child("New decisions will appear here."),
                )
            })
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
        assert_eq!(heading(0).as_ref(), "All answered");
        assert_eq!(heading(1).as_ref(), "1 decision waiting");
        assert_eq!(heading(12).as_ref(), "12 decisions waiting");
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
