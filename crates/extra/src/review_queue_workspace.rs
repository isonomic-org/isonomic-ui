// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Review queue workspace: an assistant's proposals reviewed one at a time, riskiest first.
//!
//! The queue sits on the left, sorted by risk and then by age. The current
//! proposal fills the right: who proposed it, why, and each field it would
//! change from its old value to its new one. A decision moves on to the
//! next proposal still open, so a reviewer can work through the queue from
//! the keyboard alone.
//!
//! Keyboard: the workspace is a Tab stop. J and K (or Down and Up) move
//! through the queue, A approves, R rejects, S skips and U reopens.

use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    kbd::Kbd,
    v_flex,
};
use isonomic_ui::{
    App, ElementId, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent, Keystroke,
    ParentElement as _, RenderOnce, Role, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// How much could go wrong if a proposal is approved by mistake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProposalRisk {
    /// Easy to undo.
    Low,
    /// Worth a careful look.
    Medium,
    /// Moves money or can't be undone.
    High,
}

impl ProposalRisk {
    fn word(self) -> &'static str {
        match self {
            Self::Low => "Low risk",
            Self::Medium => "Medium risk",
            Self::High => "High risk",
        }
    }
}

/// What the reviewer decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewDecision {
    /// Go ahead.
    Approve,
    /// Don't.
    Reject,
    /// Leave it for later.
    Skip,
}

/// One field a proposal would change.
#[derive(Debug, Clone, PartialEq)]
pub struct ProposedChange {
    /// What the field is.
    pub label: SharedString,
    /// Its value now, or `None` for a new value.
    pub before: Option<SharedString>,
    /// Its value after.
    pub after: SharedString,
}

impl ProposedChange {
    /// Create a change from one value to another.
    pub fn new(
        label: impl Into<SharedString>,
        before: Option<&str>,
        after: impl Into<SharedString>,
    ) -> Self {
        Self {
            label: label.into(),
            before: before.map(|before| before.to_string().into()),
            after: after.into(),
        }
    }
}

/// One thing an assistant wants to do.
#[derive(Debug, Clone, PartialEq)]
pub struct Proposal {
    /// Identifies it.
    pub id: SharedString,
    /// What it would do, in a few words.
    pub title: SharedString,
    /// Why, in a sentence or two.
    pub reason: SharedString,
    /// Who proposed it.
    pub source: SharedString,
    /// How long ago, as written, such as "12 min".
    pub age: SharedString,
    /// Its rank among proposals of the same risk: lower is older.
    pub order: u32,
    /// How much could go wrong.
    pub risk: ProposalRisk,
    /// The fields it would change.
    pub changes: Vec<ProposedChange>,
    /// What was decided, if anything.
    pub decision: Option<ReviewDecision>,
}

impl Proposal {
    /// Create an open proposal.
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        risk: ProposalRisk,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            reason: SharedString::default(),
            source: SharedString::default(),
            age: SharedString::default(),
            order: 0,
            risk,
            changes: Vec::new(),
            decision: None,
        }
    }

    /// Say why.
    pub fn reason(mut self, reason: impl Into<SharedString>) -> Self {
        self.reason = reason.into();
        self
    }

    /// Say who proposed it and how long ago. `order` ranks it by age.
    pub fn from(
        mut self,
        source: impl Into<SharedString>,
        age: impl Into<SharedString>,
        order: u32,
    ) -> Self {
        self.source = source.into();
        self.age = age.into();
        self.order = order;
        self
    }

    /// Add a field it would change.
    pub fn change(mut self, change: ProposedChange) -> Self {
        self.changes.push(change);
        self
    }

    /// Set what was decided.
    pub fn decision(mut self, decision: Option<ReviewDecision>) -> Self {
        self.decision = decision;
        self
    }
}

/// The queue's order: riskiest first, then oldest.
pub fn queue_order(proposals: &[Proposal]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..proposals.len()).collect();
    order.sort_by(|a, b| {
        let (a, b) = (&proposals[*a], &proposals[*b]);
        b.risk.cmp(&a.risk).then(a.order.cmp(&b.order))
    });
    order
}

/// The next open proposal after `from` in the queue's order, wrapping.
/// Skipped proposals count as open only when nothing else is.
pub fn next_open(proposals: &[Proposal], from: Option<&str>) -> Option<SharedString> {
    let order = queue_order(proposals);
    let start = from
        .and_then(|id| order.iter().position(|ix| proposals[*ix].id.as_ref() == id))
        .map(|at| at + 1)
        .unwrap_or(0);
    let walk = |skipped_too: bool| {
        (0..order.len())
            .map(|step| &proposals[order[(start + step) % order.len()]])
            .find(|p| {
                p.decision.is_none() || skipped_too && p.decision == Some(ReviewDecision::Skip)
            })
            .map(|p| p.id.clone())
    };
    if order.is_empty() {
        return None;
    }
    walk(false).or_else(|| walk(true))
}

type SelectHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type DecideHandler = Rc<dyn Fn(&SharedString, Option<ReviewDecision>, &mut Window, &mut App)>;

/// A queue of proposals and the one being reviewed.
#[derive(IntoElement)]
pub struct ReviewQueueWorkspace {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    proposals: Vec<Proposal>,
    current: Option<SharedString>,
    on_select: Option<SelectHandler>,
    on_decide: Option<DecideHandler>,
}

impl ReviewQueueWorkspace {
    /// Create a workspace for these proposals.
    pub fn new(id: impl Into<ElementId>, proposals: Vec<Proposal>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: "Review queue".into(),
            proposals,
            current: None,
            on_select: None,
            on_decide: None,
        }
    }

    /// Set the heading.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// The proposal being reviewed. The first open one when not given.
    pub fn current(mut self, id: Option<SharedString>) -> Self {
        self.current = id;
        self
    }

    /// Run when a proposal is picked from the queue or reached by key.
    pub fn on_select(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Run with a decision, or `None` when one is reopened. The owner moves
    /// on with [`next_open`].
    pub fn on_decide(
        mut self,
        handler: impl Fn(&SharedString, Option<ReviewDecision>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_decide = Some(Rc::new(handler));
        self
    }
}

impl Styled for ReviewQueueWorkspace {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

fn risk_color(risk: ProposalRisk, cx: &App) -> Hsla {
    let theme = cx.theme();
    match risk {
        ProposalRisk::Low => theme.success,
        ProposalRisk::Medium => theme.warning,
        ProposalRisk::High => theme.danger,
    }
}

fn key(text: &'static str) -> Option<Kbd> {
    Keystroke::parse(text).ok().map(Kbd::new)
}

impl RenderOnce for ReviewQueueWorkspace {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let success = theme.success;
        let danger = theme.danger;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;

        let focus = window
            .use_keyed_state((self.id.clone(), "focus"), cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            })
            .read(cx)
            .clone();
        let is_focused = focus.is_focused(window);

        let order = queue_order(&self.proposals);
        let current = self
            .current
            .clone()
            .filter(|id| self.proposals.iter().any(|p| &p.id == id))
            .or_else(|| next_open(&self.proposals, None))
            .or_else(|| order.first().map(|ix| self.proposals[*ix].id.clone()));
        let total = self.proposals.len();
        let decided = self
            .proposals
            .iter()
            .filter(|p| {
                matches!(
                    p.decision,
                    Some(ReviewDecision::Approve | ReviewDecision::Reject)
                )
            })
            .count();
        let count = |decision| {
            self.proposals
                .iter()
                .filter(|p| p.decision == Some(decision))
                .count()
        };
        let (approved, rejected, skipped) = (
            count(ReviewDecision::Approve),
            count(ReviewDecision::Reject),
            count(ReviewDecision::Skip),
        );
        let high_open = self
            .proposals
            .iter()
            .filter(|p| p.risk == ProposalRisk::High && p.decision.is_none())
            .count();

        let key_down = {
            let proposals = self.proposals.clone();
            let current = current.clone();
            let select = self.on_select.clone();
            let decide = self.on_decide.clone();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                if event.keystroke.modifiers.modified() {
                    return;
                }
                let order = queue_order(&proposals);
                let at = current
                    .as_ref()
                    .and_then(|id| order.iter().position(|ix| &proposals[*ix].id == id));
                let decision = match event.keystroke.key.as_str() {
                    "a" => Some(Some(ReviewDecision::Approve)),
                    "r" => Some(Some(ReviewDecision::Reject)),
                    "s" => Some(Some(ReviewDecision::Skip)),
                    "u" => Some(None),
                    "j" | "down" | "k" | "up" => {
                        let down = matches!(event.keystroke.key.as_str(), "j" | "down");
                        let next = match at {
                            None => 0,
                            Some(at) if down => (at + 1).min(order.len().saturating_sub(1)),
                            Some(at) => at.saturating_sub(1),
                        };
                        if let (Some(ix), Some(select)) = (order.get(next), &select) {
                            select(&proposals[*ix].id, window, cx);
                        }
                        cx.stop_propagation();
                        return;
                    }
                    _ => return,
                };
                if let (Some(decision), Some(id), Some(decide)) = (decision, &current, &decide) {
                    decide(id, decision, window, cx);
                    cx.stop_propagation();
                }
            }
        };

        let rows: Vec<_> = order
            .iter()
            .map(|ix| {
                let proposal = &self.proposals[*ix];
                let is_current = current.as_ref() == Some(&proposal.id);
                let select = self.on_select.clone();
                let id = proposal.id.clone();
                let mark = match proposal.decision {
                    Some(ReviewDecision::Approve) => {
                        Some(Icon::new(IconName::Check).xsmall().text_color(success))
                    }
                    Some(ReviewDecision::Reject) => {
                        Some(Icon::new(IconName::Close).xsmall().text_color(danger))
                    }
                    Some(ReviewDecision::Skip) => {
                        Some(Icon::new(IconName::ArrowRight).xsmall().text_color(quiet))
                    }
                    None => None,
                };
                let decided = proposal.decision.is_some();
                h_flex()
                    .id(ElementId::NamedChild(
                        self.id.clone().into(),
                        format!("row-{}", proposal.id).into(),
                    ))
                    .gap_2()
                    .h(px(36.))
                    .px_2()
                    .rounded(radius)
                    .cursor_pointer()
                    .when(is_current, |this| this.bg(chip))
                    .when(!is_current, |this| {
                        this.hover(|style| style.bg(chip.opacity(0.5)))
                    })
                    .role(Role::ListItem)
                    .aria_label(format!(
                        "{}, {}{}",
                        proposal.title,
                        proposal.risk.word(),
                        match proposal.decision {
                            Some(ReviewDecision::Approve) => ", approved",
                            Some(ReviewDecision::Reject) => ", rejected",
                            Some(ReviewDecision::Skip) => ", skipped",
                            None => "",
                        }
                    ))
                    .aria_selected(is_current)
                    .on_click(move |_, window, cx| {
                        if let Some(select) = &select {
                            select(&id, window, cx);
                        }
                    })
                    .child(
                        div()
                            .flex_none()
                            .size(px(8.))
                            .rounded_full()
                            .bg(risk_color(proposal.risk, cx)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .when(decided, |this| this.text_color(quiet))
                            .when(is_current, |this| this.font_medium())
                            .child(proposal.title.clone()),
                    )
                    .children(mark)
                    .child(
                        div()
                            .flex_none()
                            .text_caption()
                            .text_color(quiet)
                            .child(proposal.age.clone()),
                    )
            })
            .collect();

        let detail = current
            .as_ref()
            .and_then(|id| self.proposals.iter().find(|p| &p.id == id))
            .cloned();
        let detail = detail.map(|proposal| {
            let decide = self.on_decide.clone();
            let button = |label: &'static str,
                          hint: &'static str,
                          decision: Option<ReviewDecision>,
                          primary: bool| {
                let decide = decide.clone();
                let id = proposal.id.clone();
                Button::new(ElementId::NamedChild(
                    self.id.clone().into(),
                    format!("decide-{label}").into(),
                ))
                .small()
                .map(|this| {
                    if primary {
                        this.primary()
                    } else {
                        this.outline()
                    }
                })
                .child(
                    h_flex()
                        .gap_1p5()
                        .child(label)
                        .children(key(hint).map(|kbd| kbd.appearance(false))),
                )
                .accessibility_label(format!("{label} {}", proposal.title))
                .on_click(move |_, window, cx| {
                    if let Some(decide) = &decide {
                        decide(&id, decision, window, cx);
                    }
                })
            };
            let state = match proposal.decision {
                Some(ReviewDecision::Approve) => Some(("Approved", success)),
                Some(ReviewDecision::Reject) => Some(("Rejected", danger)),
                Some(ReviewDecision::Skip) => Some(("Skipped for now", quiet)),
                None => None,
            };
            v_flex()
                .flex_1()
                .min_w(px(260.))
                .gap_3()
                .p_4()
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            h_flex()
                                .flex_none()
                                .gap_1()
                                .px_1p5()
                                .rounded(px(5.))
                                .bg(risk_color(proposal.risk, cx).opacity(0.14))
                                .text_label_small()
                                .child(
                                    div()
                                        .size(px(6.))
                                        .rounded_full()
                                        .bg(risk_color(proposal.risk, cx)),
                                )
                                .child(proposal.risk.word()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_caption()
                                .text_color(quiet)
                                .child(format!("{} \u{b7} {} ago", proposal.source, proposal.age)),
                        ),
                )
                .child(
                    div()
                        .text_base()
                        .font_medium()
                        .line_height(relative(1.3))
                        .child(proposal.title.clone()),
                )
                .when(!proposal.reason.is_empty(), |this| {
                    this.child(div().text_color(quiet).child(proposal.reason.clone()))
                })
                .when(!proposal.changes.is_empty(), |this| {
                    this.child(
                        v_flex()
                            .rounded(radius)
                            .border_1()
                            .border_color(line)
                            .children(proposal.changes.iter().enumerate().map(|(ix, change)| {
                                h_flex()
                                    .gap_3()
                                    .px_3()
                                    .py_2()
                                    .when(ix > 0, |this| this.border_t_1().border_color(line))
                                    .child(
                                        div()
                                            .w(px(110.))
                                            .flex_none()
                                            .text_caption()
                                            .text_color(quiet)
                                            .child(change.label.clone()),
                                    )
                                    .child(
                                        h_flex()
                                            .flex_1()
                                            .min_w_0()
                                            .flex_wrap()
                                            .gap_x_2()
                                            .children(change.before.clone().map(|before| {
                                                div()
                                                    .text_color(danger)
                                                    .line_through()
                                                    .child(before)
                                            }))
                                            .children(change.before.as_ref().map(|_| {
                                                Icon::new(IconName::ArrowRight)
                                                    .xsmall()
                                                    .text_color(quiet)
                                            }))
                                            .child(
                                                div()
                                                    .text_color(success)
                                                    .font_medium()
                                                    .child(change.after.clone()),
                                            ),
                                    )
                            })),
                    )
                })
                .child(h_flex().flex_wrap().gap_2().pt_1().map(|this| {
                    match state {
                        Some((word, color)) => this
                            .child(div().text_color(color).font_medium().child(word))
                            .child(button("Reopen", "u", None, false)),
                        None => this
                            .child(button("Approve", "a", Some(ReviewDecision::Approve), true))
                            .child(button("Reject", "r", Some(ReviewDecision::Reject), false))
                            .child(button("Skip", "s", Some(ReviewDecision::Skip), false)),
                    }
                }))
        });

        let done = total > 0 && decided == total;
        let progress = if total == 0 {
            0.
        } else {
            decided as f32 / total as f32
        };

        v_flex()
            .id(self.id.clone())
            .track_focus(&focus)
            .on_key_down(key_down)
            .role(Role::Region)
            .aria_label(format!("{}, {} of {total} decided", self.title, decided))
            .w_full()
            .max_w(px(820.))
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .line_height(relative(1.45))
            .shadow(resting_shadow(cx))
            .when(is_focused, |this| this.focus_ring_style(window, cx))
            .child(
                v_flex()
                    .gap_2()
                    .px_4()
                    .pt_3()
                    .pb_2()
                    .border_b_1()
                    .border_color(line)
                    .child(
                        h_flex()
                            .gap_2()
                            .child(Icon::new(IconName::Inbox).small().text_color(quiet))
                            .child(div().flex_1().font_medium().child(self.title))
                            .child(div().text_caption().text_color(quiet).child(if done {
                                format!("All {total} reviewed")
                            } else if high_open > 0 {
                                format!("{} to go, {high_open} high risk", total - decided)
                            } else {
                                format!("{} to go", total - decided)
                            })),
                    )
                    .child(
                        div().h(px(4.)).rounded_full().bg(chip).child(
                            div()
                                .h_full()
                                .rounded_full()
                                .bg(success)
                                .w(relative(progress)),
                        ),
                    ),
            )
            .child(
                h_flex()
                    .items_start()
                    .flex_wrap()
                    .child(
                        v_flex()
                            .w(px(280.))
                            .flex_grow(1.)
                            .max_w_full()
                            .gap_0p5()
                            .p_2()
                            .children(rows),
                    )
                    .child(
                        div()
                            .flex_grow(2.)
                            .min_w(px(260.))
                            .border_l_1()
                            .border_color(line)
                            .map(|this| match detail {
                                Some(detail) => this.child(detail),
                                None => this.child(
                                    div().p_4().text_color(quiet).child("Nothing to review"),
                                ),
                            }),
                    ),
            )
            .when(done, |this| {
                this.child(
                    h_flex()
                        .gap_2()
                        .px_4()
                        .py_2()
                        .border_t_1()
                        .border_color(line)
                        .text_caption()
                        .text_color(quiet)
                        .child(
                            Icon::new(IconName::CircleCheck)
                                .xsmall()
                                .text_color(success),
                        )
                        .child(format!(
                            "{approved} approved, {rejected} rejected{}",
                            if skipped > 0 {
                                format!(", {skipped} skipped")
                            } else {
                                String::new()
                            }
                        )),
                )
            })
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn queue() -> Vec<Proposal> {
        vec![
            Proposal::new("a", "Rename a category", ProposalRisk::Low).from("Tidy", "2 h", 1),
            Proposal::new("b", "Pay a supplier", ProposalRisk::High).from("Payables", "5 min", 3),
            Proposal::new("c", "Void an invoice", ProposalRisk::High).from("Billing", "1 h", 2),
            Proposal::new("d", "Merge two contacts", ProposalRisk::Medium).from("CRM", "3 h", 0),
        ]
    }

    #[test]
    fn the_queue_puts_the_riskiest_and_oldest_first() {
        let proposals = queue();
        let ids: Vec<_> = queue_order(&proposals)
            .into_iter()
            .map(|ix| proposals[ix].id.to_string())
            .collect();
        assert_eq!(ids, ["c", "b", "d", "a"]);
    }

    #[test]
    fn moving_on_skips_decided_proposals() {
        let mut proposals = queue();
        proposals[1].decision = Some(ReviewDecision::Approve);
        assert_eq!(next_open(&proposals, Some("c")).as_deref(), Some("d"));
        assert_eq!(next_open(&proposals, None).as_deref(), Some("c"));
    }

    #[test]
    fn skipped_proposals_come_back_last() {
        let mut proposals = queue();
        for (ix, proposal) in proposals.iter_mut().enumerate() {
            proposal.decision = Some(if ix == 3 {
                ReviewDecision::Skip
            } else {
                ReviewDecision::Reject
            });
        }
        assert_eq!(next_open(&proposals, Some("c")).as_deref(), Some("d"));
        proposals[3].decision = Some(ReviewDecision::Approve);
        assert_eq!(next_open(&proposals, None), None);
    }
}
