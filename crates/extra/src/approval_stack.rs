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
//!
//! Opt in to grant scopes, so approving says for how long: once, for this
//! session or always (A, S and W). Or show the requests as a triage queue:
//! riskiest first under a risk filter, with a box on every row to pick many
//! and act on them together. In the queue Up and Down move, Space picks the
//! row, A, S and W grant, R rejects, F steps the filter and U undoes.

use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Selectable as _, Sizable as _,
    StyledExt as _, ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex, look, v_flex,
};
use isonomic_ui::{
    AnyElement, App, ElementId, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, RenderOnce, Role, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, px, relative,
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

/// How long an approval lasts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GrantScope {
    /// This one request only.
    Once,
    /// Requests like it until the session ends.
    Session,
    /// Requests like it from now on.
    Always,
}

impl GrantScope {
    /// Every scope, shortest first.
    pub const ALL: [GrantScope; 3] = [Self::Once, Self::Session, Self::Always];

    /// The button's words.
    pub fn label(self) -> &'static str {
        match self {
            Self::Once => "Allow once",
            Self::Session => "This session",
            Self::Always => "Always",
        }
    }

    /// The words after "Allowed", as in "Allowed for this session".
    pub fn words(self) -> &'static str {
        match self {
            Self::Once => "once",
            Self::Session => "for this session",
            Self::Always => "always",
        }
    }

    /// The key that grants with this scope.
    pub fn key(self) -> &'static str {
        match self {
            Self::Once => "a",
            Self::Session => "s",
            Self::Always => "w",
        }
    }

    fn of_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|scope| scope.key() == key)
    }
}

/// The undo line after a grant: "Allowed for this session: Pay a bill", or
/// "Allowed 3 requests always" for many.
pub fn grant_note(actions: &[SharedString], scope: GrantScope) -> String {
    match actions {
        [] => String::new(),
        [action] => format!("Allowed {}: {action}", scope.words()),
        many => format!("Allowed {} requests {}", many.len(), scope.words()),
    }
}

/// The undo line after rejecting many: "Rejected 2 requests".
pub fn reject_note(actions: &[SharedString]) -> String {
    match actions {
        [] => String::new(),
        [action] => format!("Rejected: {action}"),
        many => format!("Rejected {} requests", many.len()),
    }
}

type DecideHandler = Rc<dyn Fn(&SharedString, StackDecision, &mut Window, &mut App)>;
type UndoHandler = Rc<dyn Fn(&mut Window, &mut App)>;
type GrantHandler = Rc<dyn Fn(&[SharedString], GrantScope, &mut Window, &mut App)>;
type ManyHandler = Rc<dyn Fn(&[SharedString], &mut Window, &mut App)>;
type KeyHandler = Rc<dyn Fn(&KeyDownEvent, &mut Window, &mut App)>;

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
    grant_scopes: bool,
    queue: bool,
    note: Option<SharedString>,
    on_grant: Option<GrantHandler>,
    on_reject_many: Option<ManyHandler>,
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
            grant_scopes: false,
            queue: false,
            note: None,
            on_grant: None,
            on_reject_many: None,
        }
    }

    /// Approve with a scope: once, for this session or always. The front
    /// card's Approve becomes three buttons, and A, S and W grant.
    pub fn grant_scopes(mut self, grant_scopes: bool) -> Self {
        self.grant_scopes = grant_scopes;
        self
    }

    /// Show every request as a triage queue instead of a deck: riskiest
    /// first, filtered by risk, with a box on each row to act on many.
    /// The queue always grants with a scope.
    pub fn queue(mut self, queue: bool) -> Self {
        self.queue = queue;
        self
    }

    /// Words for the undo line in place of the last decision, such as
    /// [`grant_note`] gives.
    pub fn note(mut self, note: Option<SharedString>) -> Self {
        self.note = note;
        self
    }

    /// Run with the requests granted and for how long. Without it a grant
    /// counts as approving each request in turn.
    pub fn on_grant(
        mut self,
        handler: impl Fn(&[SharedString], GrantScope, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_grant = Some(Rc::new(handler));
        self
    }

    /// Run with the requests rejected together. Without it each is rejected
    /// in turn.
    pub fn on_reject_many(
        mut self,
        handler: impl Fn(&[SharedString], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_reject_many = Some(Rc::new(handler));
        self
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

/// The requests a queue shows: those at the filter's risk, or all of them,
/// riskiest first and otherwise in the order they came.
fn triage(requests: &[PendingRequest], filter: Option<RequestRisk>) -> Vec<usize> {
    let mut shown: Vec<usize> = (0..requests.len())
        .filter(|ix| filter.is_none_or(|risk| requests[*ix].risk == risk))
        .collect();
    shown.sort_by_key(|ix| std::cmp::Reverse(requests[*ix].risk));
    shown
}

/// How many requests wait at each risk, high first.
fn risk_counts(requests: &[PendingRequest]) -> [usize; 3] {
    let count = |risk| requests.iter().filter(|r| r.risk == risk).count();
    [
        count(RequestRisk::High),
        count(RequestRisk::Medium),
        count(RequestRisk::Low),
    ]
}

/// The filter after the current one: all, then high, medium and low.
fn next_filter(filter: Option<RequestRisk>) -> Option<RequestRisk> {
    match filter {
        None => Some(RequestRisk::High),
        Some(RequestRisk::High) => Some(RequestRisk::Medium),
        Some(RequestRisk::Medium) => Some(RequestRisk::Low),
        Some(RequestRisk::Low) => None,
    }
}

/// The row an arrow key moves the queue's cursor to; it stops at the ends.
fn queue_step(current: Option<usize>, count: usize, key: &str) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let at = current.unwrap_or(0).min(count - 1);
    match key {
        "down" => Some(if current.is_none() {
            0
        } else {
            (at + 1).min(count - 1)
        }),
        "up" => Some(at.saturating_sub(1)),
        "home" => Some(0),
        "end" => Some(count - 1),
        _ => None,
    }
}

/// Pick a request, or put it back.
fn toggle_pick(picked: &mut Vec<SharedString>, id: &SharedString) {
    match picked.iter().position(|p| p == id) {
        Some(at) => {
            picked.remove(at);
        }
        None => picked.push(id.clone()),
    }
}

/// Pick every shown request, or none when all of them are picked already.
fn toggle_all(picked: &mut Vec<SharedString>, shown: &[SharedString]) {
    if !shown.is_empty() && shown.iter().all(|id| picked.contains(id)) {
        picked.retain(|id| !shown.contains(id));
    } else {
        for id in shown {
            if !picked.contains(id) {
                picked.push(id.clone());
            }
        }
    }
}

/// What a queue action applies to: the picked requests that are shown, in
/// the order shown, or else the row under the cursor.
fn targets(
    picked: &[SharedString],
    shown: &[SharedString],
    cursor: Option<&SharedString>,
) -> Vec<SharedString> {
    let chosen: Vec<SharedString> = shown
        .iter()
        .filter(|id| picked.contains(id))
        .cloned()
        .collect();
    if chosen.is_empty() {
        cursor.into_iter().cloned().collect()
    } else {
        chosen
    }
}

/// What a screen reader hears for a queue row.
fn row_words(request: &PendingRequest, picked: bool) -> String {
    let mut words = format!(
        "{}, {}, from {}",
        request.action,
        risk_tone(request.risk).1.to_lowercase(),
        request.from
    );
    if !request.asked.is_empty() {
        words.push_str(&format!(", {}", request.asked));
    }
    words.push_str(if picked { ", picked" } else { ", not picked" });
    words
}

/// Hand a grant to the owner: in one call when it takes them together, or
/// as approvals one by one.
fn send_grant(
    ids: &[SharedString],
    scope: GrantScope,
    on_grant: &Option<GrantHandler>,
    on_decide: &Option<DecideHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    if ids.is_empty() {
        return;
    }
    match (on_grant, on_decide) {
        (Some(grant), _) => grant(ids, scope, window, cx),
        (None, Some(decide)) => {
            for id in ids {
                decide(id, StackDecision::Approve, window, cx);
            }
        }
        (None, None) => {}
    }
}

/// Hand a rejection of many to the owner.
fn send_reject(
    ids: &[SharedString],
    on_reject_many: &Option<ManyHandler>,
    on_decide: &Option<DecideHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    if ids.is_empty() {
        return;
    }
    match (on_reject_many, on_decide) {
        (Some(reject), _) => reject(ids, window, cx),
        (None, Some(decide)) => {
            for id in ids {
                decide(id, StackDecision::Reject, window, cx);
            }
        }
        (None, None) => {}
    }
}

/// What the queue remembers between frames.
#[derive(Default)]
struct QueueUi {
    picked: Vec<SharedString>,
    cursor: Option<SharedString>,
    filter: Option<RequestRisk>,
    hovered: Option<SharedString>,
}

impl ApprovalStack {
    /// The triage queue, its keys, and what a screen reader hears for it.
    fn queue_view(
        &self,
        keyboard: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> (AnyElement, KeyHandler, String) {
        let child_id = |name: String| ElementId::NamedChild(self.id.clone().into(), name.into());
        let ui = window.use_keyed_state(child_id("queue".into()), cx, |_, _| QueueUi::default());
        let requests = Rc::new(self.requests.clone());
        let (filter, cursor, picked, hovered) = ui.update(cx, |ui, _| {
            // Forget requests that were decided elsewhere.
            ui.picked.retain(|id| requests.iter().any(|r| &r.id == id));
            if ui
                .cursor
                .as_ref()
                .is_some_and(|id| !requests.iter().any(|r| &r.id == id))
            {
                ui.cursor = None;
            }
            (
                ui.filter,
                ui.cursor.clone(),
                ui.picked.clone(),
                ui.hovered.clone(),
            )
        });
        let order = triage(&requests, filter);
        let shown: Rc<Vec<SharedString>> =
            Rc::new(order.iter().map(|ix| requests[*ix].id.clone()).collect());
        let cursor_at = cursor
            .as_ref()
            .and_then(|id| shown.iter().position(|s| s == id));
        let chosen = targets(&picked, &shown, cursor.as_ref());
        let counts = risk_counts(&requests);
        let picked_shown = shown.iter().filter(|id| picked.contains(id)).count();

        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let chip = theme.muted;
        let selected_fill = theme.info.opacity(0.08);
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;

        let grant = {
            let on_grant = self.on_grant.clone();
            let on_decide = self.on_decide.clone();
            let ui = ui.clone();
            Rc::new(
                move |ids: &[SharedString],
                      scope: GrantScope,
                      window: &mut Window,
                      cx: &mut App| {
                    ui.update(cx, |ui, cx| {
                        ui.picked.retain(|id| !ids.contains(id));
                        cx.notify();
                    });
                    send_grant(ids, scope, &on_grant, &on_decide, window, cx);
                },
            )
        };
        let reject = {
            let on_reject_many = self.on_reject_many.clone();
            let on_decide = self.on_decide.clone();
            let ui = ui.clone();
            Rc::new(
                move |ids: &[SharedString], window: &mut Window, cx: &mut App| {
                    ui.update(cx, |ui, cx| {
                        ui.picked.retain(|id| !ids.contains(id));
                        cx.notify();
                    });
                    send_reject(ids, &on_reject_many, &on_decide, window, cx);
                },
            )
        };

        // Filter buttons: all, then each risk with how many wait at it.
        let filter_button = |label: String, risk: Option<RequestRisk>, count: usize| {
            let ui = ui.clone();
            let name = match risk {
                Some(risk) => risk_tone(risk).1.to_string(),
                None => "All risks".to_string(),
            };
            Button::new(child_id(format!(
                "filter-{}",
                risk.map(|r| risk_tone(r).1).unwrap_or("all")
            )))
            .ghost()
            .xsmall()
            .selected(filter == risk)
            .label(label)
            .accessibility_label(format!(
                "{name}, {count} waiting{}",
                if filter == risk { ", shown" } else { "" }
            ))
            .on_click(move |_, _, cx| {
                ui.update(cx, |ui, cx| {
                    ui.filter = risk;
                    cx.notify();
                })
            })
        };
        let filters = h_flex()
            .gap_1()
            .flex_wrap()
            .child(filter_button(
                format!("All {}", requests.len()),
                None,
                requests.len(),
            ))
            .children(
                [RequestRisk::High, RequestRisk::Medium, RequestRisk::Low]
                    .into_iter()
                    .zip(counts)
                    .map(|(risk, count)| {
                        let word = match risk {
                            RequestRisk::High => "High",
                            RequestRisk::Medium => "Medium",
                            RequestRisk::Low => "Low",
                        };
                        filter_button(format!("{word} {count}"), Some(risk), count)
                    }),
            );

        // The bar that acts on the picked rows, or on the cursor's row.
        let all_picked = !shown.is_empty() && picked_shown == shown.len();
        let none_chosen = chosen.is_empty();
        let bar = h_flex()
            .flex_wrap()
            .gap_2()
            .px_2()
            .py_1p5()
            .rounded(radius)
            .bg(chip)
            .child(
                Checkbox::new(child_id("pick-all".into()))
                    .checked(all_picked)
                    .disabled(shown.is_empty())
                    .label(if picked_shown == 0 {
                        "Pick all".to_string()
                    } else {
                        format!("{picked_shown} picked")
                    })
                    .accessibility_label(if all_picked {
                        "Pick all shown, all picked".to_string()
                    } else {
                        format!("Pick all shown, {picked_shown} of {} picked", shown.len())
                    })
                    .on_click({
                        let ui = ui.clone();
                        let shown = shown.clone();
                        move |_, _, cx| {
                            ui.update(cx, |ui, cx| {
                                toggle_all(&mut ui.picked, &shown);
                                cx.notify();
                            })
                        }
                    }),
            )
            .child(div().flex_1())
            .child(
                Button::new(child_id("reject-picked".into()))
                    .xsmall()
                    .outline()
                    .danger()
                    .label("Reject")
                    .disabled(none_chosen)
                    .accessibility_label(format!("Reject {}", count_words(chosen.len())))
                    .on_click({
                        let reject = reject.clone();
                        let chosen = chosen.clone();
                        move |_, window, cx| reject(&chosen, window, cx)
                    }),
            )
            .children(GrantScope::ALL.into_iter().map(|scope| {
                let grant = grant.clone();
                let chosen = chosen.clone();
                Button::new(child_id(format!("grant-picked-{}", scope.key())))
                    .xsmall()
                    .map(|this| match scope {
                        GrantScope::Once => this.primary(),
                        _ => this.outline(),
                    })
                    .label(scope.label())
                    .disabled(none_chosen)
                    .accessibility_label(format!(
                        "Allow {} {}",
                        count_words(chosen.len()),
                        scope.words()
                    ))
                    .on_click(move |_, window, cx| grant(&chosen, scope, window, cx))
            }));

        // The rows, under a heading for each risk while every risk shows.
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut last_risk = None;
        for (at, ix) in order.iter().enumerate() {
            let request = &requests[*ix];
            if filter.is_none() && last_risk != Some(request.risk) {
                last_risk = Some(request.risk);
                let count = counts[2 - request.risk as usize];
                rows.push(
                    div()
                        .px_2()
                        .pt_2()
                        .pb_0p5()
                        .text_eyebrow()
                        .text_color(quiet)
                        .child(format!(
                            "{} \u{b7} {count}",
                            risk_tone(request.risk).1.to_uppercase()
                        ))
                        .into_any_element(),
                );
            }
            let (tone, risk_word) = risk_tone(request.risk);
            let tone_color = tone.color(cx);
            let is_picked = picked.contains(&request.id);
            let is_cursor = keyboard && cursor_at == Some(at);
            let hover = look::hover_fade(
                child_id(format!("row-hover-{}", request.id)),
                hovered.as_ref() == Some(&request.id) || is_cursor,
                window,
                cx,
            );
            let rest = if is_picked { selected_fill } else { fill };
            let id = request.id.clone();
            rows.push(
                h_flex()
                    .id(child_id(format!("row-{}", request.id)))
                    .role(Role::ListItem)
                    .aria_label(row_words(request, is_picked))
                    .aria_selected(is_picked)
                    .gap_2p5()
                    .px_2()
                    .py_1p5()
                    .rounded(radius)
                    .bg(look::mix(rest, rest.blend(chip), hover))
                    .when(is_cursor, |this| this.focus_ring_style(window, cx))
                    .on_hover({
                        let ui = ui.clone();
                        let id = id.clone();
                        move |over, _, cx| {
                            ui.update(cx, |ui, cx| {
                                if *over {
                                    ui.hovered = Some(id.clone());
                                } else if ui.hovered.as_ref() == Some(&id) {
                                    ui.hovered = None;
                                }
                                cx.notify();
                            })
                        }
                    })
                    .on_click({
                        let ui = ui.clone();
                        let id = id.clone();
                        move |_, _, cx| {
                            ui.update(cx, |ui, cx| {
                                ui.cursor = Some(id.clone());
                                cx.notify();
                            })
                        }
                    })
                    .child(
                        Checkbox::new(child_id(format!("pick-{}", request.id)))
                            .checked(is_picked)
                            .tab_stop(false)
                            .accessibility_label(format!(
                                "Pick {}, {}",
                                request.action,
                                if is_picked { "picked" } else { "not picked" }
                            ))
                            .on_click({
                                let ui = ui.clone();
                                move |_, _, cx| {
                                    ui.update(cx, |ui, cx| {
                                        toggle_pick(&mut ui.picked, &id);
                                        cx.notify();
                                    })
                                }
                            }),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().truncate().child(request.action.clone()))
                            .child(div().truncate().text_caption().text_color(quiet).child(
                                if request.asked.is_empty() {
                                    request.from.to_string()
                                } else {
                                    format!("{} \u{b7} {}", request.from, request.asked)
                                },
                            )),
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
                    )
                    .into_any_element(),
            );
        }
        let empty = rows.is_empty();

        let view = v_flex()
            .gap_2()
            .p_2()
            .rounded(radius_lg)
            .bg(fill)
            .shadow(resting_shadow(cx))
            .child(filters)
            .child(bar)
            .child(
                v_flex()
                    .id(child_id("rows".into()))
                    .role(Role::List)
                    .aria_label(format!("{} shown", count_words(shown.len())))
                    .gap_0p5()
                    .children(rows)
                    .when(empty, |this| {
                        this.child(
                            div()
                                .py_6()
                                .text_center()
                                .text_caption()
                                .text_color(quiet)
                                .child(if requests.is_empty() {
                                    "Nothing waiting"
                                } else {
                                    "Nothing waiting at this risk"
                                }),
                        )
                    }),
            )
            .into_any_element();

        let keys: KeyHandler = {
            let ui = ui.clone();
            let undo = self.on_undo.clone();
            let can_undo = self.last.is_some() || self.note.is_some();
            Rc::new(
                move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                    if event.keystroke.modifiers.modified() {
                        return;
                    }
                    let key = event.keystroke.key.as_str();
                    if let Some(next) = queue_step(cursor_at, shown.len(), key) {
                        let id = shown[next].clone();
                        ui.update(cx, |ui, cx| {
                            ui.cursor = Some(id);
                            cx.notify();
                        });
                        cx.stop_propagation();
                        return;
                    }
                    match key {
                        "space" => {
                            if let Some(id) = cursor_at.and_then(|at| shown.get(at)).cloned() {
                                ui.update(cx, |ui, cx| {
                                    toggle_pick(&mut ui.picked, &id);
                                    cx.notify();
                                });
                            }
                        }
                        "f" => ui.update(cx, |ui, cx| {
                            ui.filter = next_filter(ui.filter);
                            ui.cursor = None;
                            cx.notify();
                        }),
                        "r" => reject(&chosen, window, cx),
                        "u" => {
                            if let (true, Some(undo)) = (can_undo, &undo) {
                                undo(window, cx);
                            }
                        }
                        key => match GrantScope::of_key(key) {
                            Some(scope) => grant(&chosen, scope, window, cx),
                            None => return,
                        },
                    }
                    cx.stop_propagation();
                },
            )
        };

        let words = format!(
            "{} waiting, {} high risk. {}. Up and Down move, Space picks, A, S or W allow, R \
             rejects, F changes the filter",
            requests.len(),
            counts[0],
            match filter {
                Some(risk) => format!("Showing {}", risk_tone(risk).1.to_lowercase()),
                None => "Showing all risks".to_string(),
            }
        );
        (view, keys, words)
    }
}

/// "1 request" or "3 requests".
fn count_words(count: usize) -> String {
    match count {
        1 => "1 request".to_string(),
        n => format!("{n} requests"),
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
            let can_undo = self.last.is_some() || self.note.is_some();
            let scoped = self.grant_scopes;
            let on_grant = self.on_grant.clone();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                if event.keystroke.modifiers.modified() {
                    return;
                }
                let key = event.keystroke.key.as_str();
                if scoped && let Some(scope) = GrantScope::of_key(key) {
                    if let Some(id) = &front {
                        send_grant(
                            std::slice::from_ref(id),
                            scope,
                            &on_grant,
                            &decide,
                            window,
                            cx,
                        );
                        cx.stop_propagation();
                    }
                    return;
                }
                let decision = match key {
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
                                    .map(|this| {
                                        if self.grant_scopes {
                                            this.children(GrantScope::ALL.into_iter().map(
                                                |scope| {
                                                    let on_grant = self.on_grant.clone();
                                                    let on_decide = self.on_decide.clone();
                                                    let id = request.id.clone();
                                                    Button::new(ElementId::NamedChild(
                                                        self.id.clone().into(),
                                                        format!("grant-{}-{}", scope.key(), id)
                                                            .into(),
                                                    ))
                                                    .small()
                                                    .map(|this| match scope {
                                                        GrantScope::Once => this.primary(),
                                                        _ => this.outline(),
                                                    })
                                                    .label(scope.label())
                                                    .accessibility_label(format!(
                                                        "Allow {}: {}",
                                                        scope.words(),
                                                        request.action
                                                    ))
                                                    .on_click(move |_, window, cx| {
                                                        send_grant(
                                                            std::slice::from_ref(&id),
                                                            scope,
                                                            &on_grant,
                                                            &on_decide,
                                                            window,
                                                            cx,
                                                        )
                                                    })
                                                },
                                            ))
                                        } else {
                                            this.child(button("Approve", StackDecision::Approve))
                                        }
                                    }),
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
        let (deck, key_down, words): (AnyElement, KeyHandler, String) = if self.queue {
            self.queue_view(is_focused, window, cx)
        } else {
            (
                deck,
                Rc::new(key_down),
                match &front {
                    Some(request) => {
                        format!("{waiting} requests waiting. Front: {}", request.action)
                    }
                    None => "No requests waiting".to_string(),
                },
            )
        };

        v_flex()
            .id(self.id.clone())
            .track_focus(&focus)
            .on_key_down(move |event, window, cx| key_down(event, window, cx))
            .role(Role::Region)
            .aria_label(words)
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
            .children(
                self.note
                    .clone()
                    .map(|note| note.to_string())
                    .or_else(|| {
                        self.last.clone().map(|(action, decision)| {
                            format!("{}: {action}", decision_words(decision))
                        })
                    })
                    .map(|words| {
                        let undo = self.on_undo.clone();
                        h_flex()
                            .gap_2()
                            .px_1()
                            .text_caption()
                            .text_color(quiet)
                            .child(div().flex_1().min_w_0().truncate().child(words))
                            .child(
                                Button::new(ElementId::NamedChild(
                                    self.id.clone().into(),
                                    "undo".into(),
                                ))
                                .xsmall()
                                .ghost()
                                .label("Undo")
                                .on_click(move |_, window, cx| {
                                    if let Some(undo) = &undo {
                                        undo(window, cx);
                                    }
                                }),
                            )
                    }),
            )
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

    fn queue() -> Vec<PendingRequest> {
        vec![
            PendingRequest::new("a", "x", "Read a file", RequestRisk::Low),
            PendingRequest::new("b", "x", "Pay a bill", RequestRisk::High),
            PendingRequest::new("c", "x", "Send mail", RequestRisk::Medium),
            PendingRequest::new("d", "x", "Delete a branch", RequestRisk::High),
        ]
    }

    #[test]
    fn triage_puts_the_riskiest_first_and_keeps_arrival_order() {
        assert_eq!(triage(&queue(), None), [1, 3, 2, 0]);
        assert_eq!(triage(&queue(), Some(RequestRisk::High)), [1, 3]);
        assert_eq!(triage(&queue(), Some(RequestRisk::Low)), [0]);
        assert_eq!(risk_counts(&queue()), [2, 1, 1]);
    }

    #[test]
    fn the_filter_steps_through_every_risk_and_back() {
        let mut filter = None;
        let mut seen = Vec::new();
        for _ in 0..4 {
            filter = next_filter(filter);
            seen.push(filter);
        }
        assert_eq!(
            seen,
            [
                Some(RequestRisk::High),
                Some(RequestRisk::Medium),
                Some(RequestRisk::Low),
                None
            ]
        );
    }

    #[test]
    fn the_cursor_stops_at_the_ends() {
        assert_eq!(queue_step(None, 3, "down"), Some(0));
        assert_eq!(queue_step(Some(0), 3, "down"), Some(1));
        assert_eq!(queue_step(Some(2), 3, "down"), Some(2));
        assert_eq!(queue_step(Some(0), 3, "up"), Some(0));
        assert_eq!(queue_step(Some(1), 3, "end"), Some(2));
        assert_eq!(queue_step(Some(1), 3, "x"), None);
        assert_eq!(queue_step(None, 0, "down"), None);
    }

    #[test]
    fn picking_toggles_one_or_all() {
        let shown: Vec<SharedString> = vec!["b".into(), "d".into()];
        let mut picked = Vec::new();
        toggle_pick(&mut picked, &"b".into());
        assert_eq!(picked, ["b"]);
        toggle_all(&mut picked, &shown);
        assert_eq!(picked, ["b", "d"]);
        // All shown were picked, so the second press clears them.
        toggle_all(&mut picked, &shown);
        assert!(picked.is_empty());
        toggle_pick(&mut picked, &"b".into());
        toggle_pick(&mut picked, &"b".into());
        assert!(picked.is_empty());
    }

    #[test]
    fn actions_apply_to_the_picked_rows_or_the_cursor() {
        let shown: Vec<SharedString> = vec!["b".into(), "d".into(), "c".into()];
        let picked: Vec<SharedString> = vec!["c".into(), "b".into(), "gone".into()];
        // Picked rows in the order shown; ones not shown are left alone.
        assert_eq!(targets(&picked, &shown, Some(&"d".into())), ["b", "c"]);
        assert_eq!(targets(&[], &shown, Some(&"d".into())), ["d"]);
        assert!(targets(&[], &shown, None).is_empty());
    }

    #[test]
    fn grants_and_rows_read_in_words() {
        let one: Vec<SharedString> = vec!["Pay a bill".into()];
        let three: Vec<SharedString> = vec!["a".into(), "b".into(), "c".into()];
        assert_eq!(
            grant_note(&one, GrantScope::Session),
            "Allowed for this session: Pay a bill"
        );
        assert_eq!(
            grant_note(&three, GrantScope::Always),
            "Allowed 3 requests always"
        );
        assert_eq!(reject_note(&three), "Rejected 3 requests");
        assert_eq!(GrantScope::of_key("w"), Some(GrantScope::Always));
        assert_eq!(GrantScope::of_key("q"), None);
        let request = PendingRequest::new("b", "Payables", "Pay a bill", RequestRisk::High)
            .asked("2 min ago");
        assert_eq!(
            row_words(&request, true),
            "Pay a bill, high risk, from Payables, 2 min ago, picked"
        );
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
