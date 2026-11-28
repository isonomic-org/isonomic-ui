// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Approval stack: pending requests as a deck of cards to approve, edit or reject.
//!
//! The front card shows one request in full: who asks, what they'd do,
//! the details that matter and how risky it is. The next two peek out
//! behind it so the reviewer can see how much is waiting. Deciding moves the
//! front card off; the owner removes it and the next comes forward. An undo
//! line names the last decision.
//!
//! Keyboard: the stack is a Tab stop. A approves, R rejects, E edits and U
//! undoes the last decision.

use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    RenderOnce, Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled,
    Window, div, prelude::FluentBuilder as _, px, relative,
};

use crate::common::{Tone, lifted_shadow, resting_shadow};

/// How risky a request is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RequestRisk {
    /// Easy to undo.
    Low,
    /// Worth a look.
    Medium,
    /// Moves money or can't be undone.
    High,
}

/// One pending request.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingRequest {
    /// Identifies it.
    pub id: SharedString,
    /// Who asks.
    pub from: SharedString,
    /// What they'd do.
    pub action: SharedString,
    /// Details as label and value pairs.
    pub details: Vec<(SharedString, SharedString)>,
    /// How risky it is.
    pub risk: RequestRisk,
    /// When it was asked, as written.
    pub asked: SharedString,
}

impl PendingRequest {
    /// Create a request.
    pub fn new(
        id: impl Into<SharedString>,
        from: impl Into<SharedString>,
        action: impl Into<SharedString>,
        risk: RequestRisk,
    ) -> Self {
        Self {
            id: id.into(),
            from: from.into(),
            action: action.into(),
            details: Vec::new(),
            risk,
            asked: SharedString::default(),
        }
    }

    /// Add a detail.
    pub fn detail(
        mut self,
        label: impl Into<SharedString>,
        value: impl Into<SharedString>,
    ) -> Self {
        self.details.push((label.into(), value.into()));
        self
    }

    /// Say when it was asked.
    pub fn asked(mut self, when: impl Into<SharedString>) -> Self {
        self.asked = when.into();
        self
    }
}

/// What the reviewer did with a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackDecision {
    /// Let it go ahead.
    Approve,
    /// Stop it.
    Reject,
    /// Change it before deciding.
    Edit,
}

type DecideHandler = Rc<dyn Fn(&SharedString, StackDecision, &mut Window, &mut App)>;
type UndoHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// Pending requests as a deck, front card first.
#[derive(IntoElement)]
pub struct ApprovalStack {
    id: ElementId,
    style: StyleRefinement,
    requests: Vec<PendingRequest>,
    decided: usize,
    last: Option<(SharedString, StackDecision)>,
    on_decide: Option<DecideHandler>,
    on_undo: Option<UndoHandler>,
}

impl ApprovalStack {
    /// Create a stack. The first request is the front card.
    pub fn new(id: impl Into<ElementId>, requests: Vec<PendingRequest>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            requests,
            decided: 0,
            last: None,
            on_decide: None,
            on_undo: None,
        }
    }

    /// How many were decided already this session, for the counter.
    pub fn decided(mut self, count: usize) -> Self {
        self.decided = count;
        self
    }

    /// The last decision, as the request's action and what was decided, for the undo line.
    pub fn last(mut self, last: Option<(SharedString, StackDecision)>) -> Self {
        self.last = last;
        self
    }

    /// Run with the front request and what was decided.
    pub fn on_decide(
        mut self,
        handler: impl Fn(&SharedString, StackDecision, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_decide = Some(Rc::new(handler));
        self
    }

    /// Run when the last decision is undone.
    pub fn on_undo(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_undo = Some(Rc::new(handler));
        self
    }
}

impl Styled for ApprovalStack {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

fn risk_tone(risk: RequestRisk) -> (Tone, &'static str) {
    match risk {
        RequestRisk::Low => (Tone::Success, "Low risk"),
        RequestRisk::Medium => (Tone::Warning, "Medium risk"),
        RequestRisk::High => (Tone::Danger, "High risk"),
    }
}

/// The words for a decision in the undo line.
pub fn decision_words(decision: StackDecision) -> &'static str {
    match decision {
        StackDecision::Approve => "Approved",
        StackDecision::Reject => "Rejected",
        StackDecision::Edit => "Sent back to edit",
    }
}

impl RenderOnce for ApprovalStack {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;

        let focus = window
            .use_keyed_state((self.id.clone(), "focus"), cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            })
            .read(cx)
            .clone();
        let is_focused = focus.is_focused(window);

        let front = self.requests.first().cloned();
        let waiting = self.requests.len();
        let total = waiting + self.decided;

        let key_down = {
            let front = front.as_ref().map(|r| r.id.clone());
            let decide = self.on_decide.clone();
            let undo = self.on_undo.clone();
            let can_undo = self.last.is_some();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                if event.keystroke.modifiers.modified() {
                    return;
                }
                let decision = match event.keystroke.key.as_str() {
                    "a" => StackDecision::Approve,
                    "r" => StackDecision::Reject,
                    "e" => StackDecision::Edit,
                    "u" => {
                        if let (true, Some(undo)) = (can_undo, &undo) {
                            undo(window, cx);
                            cx.stop_propagation();
                        }
                        return;
                    }
                    _ => return,
                };
                if let (Some(id), Some(decide)) = (&front, &decide) {
                    decide(id, decision, window, cx);
                    cx.stop_propagation();
                }
            }
        };

        let deck = match &front {
            Some(request) => {
                let (tone, risk_word) = risk_tone(request.risk);
                let tone_color = tone.color(cx);
                let button = |label: &'static str, decision: StackDecision| {
                    let decide = self.on_decide.clone();
                    let id = request.id.clone();
                    Button::new(ElementId::NamedChild(
                        self.id.clone().into(),
                        format!("{label}-{}", request.id).into(),
                    ))
                    .small()
                    .map(|this| match decision {
                        StackDecision::Approve => this.primary(),
                        StackDecision::Reject => this.outline().danger(),
                        StackDecision::Edit => this.outline(),
                    })
                    .label(label)
                    .accessibility_label(format!("{label}: {}", request.action))
                    .on_click(move |_, window, cx| {
                        if let Some(decide) = &decide {
                            decide(&id, decision, window, cx);
                        }
                    })
                };
                let peeks = (waiting.saturating_sub(1)).min(2);
                div()
                    .relative()
                    .pb(px(peeks as f32 * 8.))
                    .children((0..peeks).rev().map(|n| {
                        let depth = n + 1;
                        div()
                            .absolute()
                            .left(px(depth as f32 * 10.))
                            .right(px(depth as f32 * 10.))
                            .bottom(px((peeks - depth) as f32 * 8.))
                            .h(px(40.))
                            .rounded(radius_lg)
                            // Every card behind is a plain surface; only its edge shows.
                            .bg(fill)
                            .shadow(resting_shadow(cx))
                    }))
                    .child(
                        v_flex()
                            .relative()
                            .gap_3()
                            .p_4()
                            .rounded(radius_lg)
                            .bg(fill)
                            .shadow(lifted_shadow(cx))
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .size(px(24.))
                                            .flex_none()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .rounded(px(6.))
                                            .bg(chip)
                                            .child(
                                                Icon::new(IconName::Bot).xsmall().text_color(quiet),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .text_caption()
                                            .text_color(quiet)
                                            .child(if request.asked.is_empty() {
                                                request.from.to_string()
                                            } else {
                                                format!("{} \u{b7} {}", request.from, request.asked)
                                            }),
                                    )
                                    .child(
                                        h_flex()
                                            .flex_none()
                                            .gap_1()
                                            .px_1p5()
                                            .rounded(px(4.))
                                            .bg(tone.tint(cx))
                                            .text_caption()
                                            .text_color(tone_color)
                                            .child(div().size(px(6.)).rounded_full().bg(tone_color))
                                            .child(risk_word),
                                    ),
                            )
                            .child(
                                div()
                                    .text_base()
                                    .font_medium()
                                    .line_height(relative(1.3))
                                    .child(request.action.clone()),
                            )
                            .when(!request.details.is_empty(), |this| {
                                this.child(
                                    v_flex()
                                        .rounded(radius)
                                        .border_1()
                                        .border_color(line)
                                        .children(request.details.iter().enumerate().map(
                                            |(ix, (label, value))| {
                                                h_flex()
                                                    .gap_3()
                                                    .px_3()
                                                    .py_1p5()
                                                    .when(ix > 0, |this| {
                                                        this.border_t_1().border_color(line)
                                                    })
                                                    .child(
                                                        div()
                                                            .w(px(96.))
                                                            .flex_none()
                                                            .text_caption()
                                                            .text_color(quiet)
                                                            .child(label.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .min_w_0()
                                                            .child(value.clone()),
                                                    )
                                            },
                                        )),
                                )
                            })
                            .child(
                                h_flex()
                                    .flex_wrap()
                                    .gap_2()
                                    .child(button("Reject", StackDecision::Reject))
                                    .child(button("Edit", StackDecision::Edit))
                                    .child(div().flex_1())
                                    .child(button("Approve", StackDecision::Approve)),
                            ),
                    )
                    .into_any_element()
            }
            None => v_flex()
                .items_center()
                .gap_2()
                .py_8()
                .rounded(radius_lg)
                .border_1()
                .border_dashed()
                .border_color(line)
                .child(Icon::new(IconName::CircleCheck).text_color(Tone::Success.color(cx)))
                .child(div().font_medium().child("Nothing waiting"))
                .child(
                    div()
                        .text_caption()
                        .text_color(quiet)
                        .child(format!("{} decided", self.decided)),
                )
                .into_any_element(),
        };

        v_flex()
            .id(self.id.clone())
            .track_focus(&focus)
            .on_key_down(key_down)
            .role(Role::Region)
            .aria_label(match &front {
                Some(request) => format!("{waiting} requests waiting. Front: {}", request.action),
                None => "No requests waiting".to_string(),
            })
            .w_full()
            .max_w(px(440.))
            .gap_3()
            .p_1()
            .rounded(radius_lg)
            .text_body()
            .text_color(ink)
            .when(is_focused, |this| this.focus_ring_style(window, cx))
            .child(
                h_flex()
                    .gap_2()
                    .px_1()
                    .text_caption()
                    .text_color(quiet)
                    .child(Icon::new(IconName::Inbox).xsmall())
                    .child(div().flex_1().child(if waiting == 0 {
                        "All caught up".to_string()
                    } else {
                        format!("{waiting} waiting")
                    }))
                    .when(total > 0, |this| {
                        this.child(format!("{} of {total} decided", self.decided))
                    }),
            )
            .child(deck)
            .children(self.last.clone().map(|(action, decision)| {
                let undo = self.on_undo.clone();
                h_flex()
                    .gap_2()
                    .px_1()
                    .text_caption()
                    .text_color(quiet)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(format!("{}: {action}", decision_words(decision))),
                    )
                    .child(
                        Button::new(ElementId::NamedChild(self.id.clone().into(), "undo".into()))
                            .xsmall()
                            .ghost()
                            .label("Undo")
                            .on_click(move |_, window, cx| {
                                if let Some(undo) = &undo {
                                    undo(window, cx);
                                }
                            }),
                    )
            }))
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn risks_and_decisions_have_words() {
        assert_eq!(risk_tone(RequestRisk::High).1, "High risk");
        assert_eq!(decision_words(StackDecision::Edit), "Sent back to edit");
        assert!(RequestRisk::High > RequestRisk::Low);
    }

    #[test]
    fn requests_collect_details() {
        let request = PendingRequest::new("a", "Payables", "Pay a bill", RequestRisk::High)
            .detail("Amount", "$1,240.50")
            .detail("To", "Harbour Supply")
            .asked("3 min ago");
        assert_eq!(request.details.len(), 2);
        assert_eq!(request.asked.as_ref(), "3 min ago");
    }
}
