//! Approval card: an assistant asks before it does something hard to undo.
//!
//! The card names the action, says how risky it is and previews what will
//! happen. Approve and Deny have shortcuts, a request can expire on its own,
//! and once decided the card folds into one resolved row.

use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    AnyElement, App, ElementId, InteractiveElement as _, IntoElement, Keystroke,
    ParentElement as _, RenderOnce, SharedString, StatefulInteractiveElement as _, StyleRefinement,
    Styled, Task, Window, div, prelude::FluentBuilder as _, px, relative,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex,
    kbd::Kbd,
    v_flex,
};

use crate::common::resting_shadow;

const SECOND: Duration = Duration::from_secs(1);
/// The countdown turns to the warning colour from here on.
const URGENT_FROM: Duration = Duration::from_secs(10);

/// How risky the action is. Set it from the action: anything external, paid
/// or impossible to undo is at least medium.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalRisk {
    /// Easy to take back.
    Low,
    /// Others will see it, or it costs something.
    Medium,
    /// Cannot be taken back.
    High,
}

impl ApprovalRisk {
    fn label(self) -> &'static str {
        match self {
            Self::Low => "Low risk",
            Self::Medium => "Medium risk",
            Self::High => "High risk",
        }
    }
}

/// Where a request stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalStatus {
    /// Waiting for an answer.
    Pending,
    /// Approved.
    Approved,
    /// Denied, or expired.
    Denied,
}

/// Why a request was denied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenyReason {
    /// A person said no.
    Declined,
    /// Nobody answered in time.
    Expired,
}

type ApproveHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;
type DenyHandler = Rc<dyn Fn(DenyReason, &mut Window, &mut App)>;
type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A request to approve or deny one action.
#[derive(IntoElement)]
pub struct ApprovalCard {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    description: Option<SharedString>,
    risk: Option<ApprovalRisk>,
    icon: IconName,
    details: Vec<(SharedString, SharedString)>,
    preview: Option<AnyElement>,
    status: Option<ApprovalStatus>,
    default_status: ApprovalStatus,
    expires_in: Option<Duration>,
    busy: bool,
    allow_always: bool,
    always_label: SharedString,
    approve_label: SharedString,
    deny_label: SharedString,
    shortcuts: bool,
    on_approve: Option<ApproveHandler>,
    on_deny: Option<DenyHandler>,
    on_edit: Option<Handler>,
}

impl ApprovalCard {
    /// Create a request. Word the title as the action: "Send the newsletter".
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            description: None,
            risk: None,
            icon: IconName::CircleAlert,
            details: Vec::new(),
            preview: None,
            status: None,
            default_status: ApprovalStatus::Pending,
            expires_in: None,
            busy: false,
            allow_always: true,
            always_label: "Always allow this action".into(),
            approve_label: "Approve".into(),
            deny_label: "Deny".into(),
            shortcuts: true,
            on_approve: None,
            on_deny: None,
            on_edit: None,
        }
    }

    /// One line of context under the title.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Show a risk badge in the header.
    pub fn risk(mut self, risk: ApprovalRisk) -> Self {
        self.risk = Some(risk);
        self
    }

    /// Set the icon in the header tile.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = icon;
        self
    }

    /// Add a labelled value to the preview: the recipients, the amount, the target.
    pub fn detail(
        mut self,
        label: impl Into<SharedString>,
        value: impl Into<SharedString>,
    ) -> Self {
        self.details.push((label.into(), value.into()));
        self
    }

    /// Show custom content above the details, such as a list of recipients.
    pub fn preview(mut self, preview: impl IntoElement) -> Self {
        self.preview = Some(preview.into_any_element());
        self
    }

    /// Control where the request stands. Setting it back to pending starts a
    /// fresh request with a fresh countdown.
    pub fn status(mut self, status: ApprovalStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Set where the request stands at first, when the card keeps track itself.
    pub fn default_status(mut self, status: ApprovalStatus) -> Self {
        self.default_status = status;
        self
    }

    /// Let the request expire after this long. The countdown pauses while the
    /// card is pointed at.
    pub fn expires_in(mut self, duration: Duration) -> Self {
        self.expires_in = Some(duration);
        self
    }

    /// Lock the card and show Approve busy while the approved action starts.
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    /// Whether the "always allow" checkbox shows. On by default.
    pub fn allow_always(mut self, allow: bool) -> Self {
        self.allow_always = allow;
        self
    }

    /// Set the label of the "always allow" checkbox.
    pub fn always_label(mut self, label: impl Into<SharedString>) -> Self {
        self.always_label = label.into();
        self
    }

    /// Set the label of the Approve button.
    pub fn approve_label(mut self, label: impl Into<SharedString>) -> Self {
        self.approve_label = label.into();
        self
    }

    /// Set the label of the Deny button.
    pub fn deny_label(mut self, label: impl Into<SharedString>) -> Self {
        self.deny_label = label.into();
        self
    }

    /// Whether the shortcut hints show on the buttons. The shortcuts work
    /// either way.
    pub fn shortcuts(mut self, shortcuts: bool) -> Self {
        self.shortcuts = shortcuts;
        self
    }

    /// Run on Approve, with whether "always allow" was ticked.
    pub fn on_approve(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_approve = Some(Rc::new(handler));
        self
    }

    /// Run on Deny or Escape, and when the request expires.
    pub fn on_deny(
        mut self,
        handler: impl Fn(DenyReason, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_deny = Some(Rc::new(handler));
        self
    }

    /// Show an Edit button before Deny.
    pub fn on_edit(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_edit = Some(Rc::new(handler));
        self
    }
}

impl Styled for ApprovalCard {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// One second passes. Returns the time left, or `None` when the request has
/// just run out. A paused countdown stands still.
fn tick(remaining: Duration, paused: bool) -> Option<Duration> {
    if paused {
        return Some(remaining);
    }
    remaining.checked_sub(SECOND).filter(|left| !left.is_zero())
}

/// The time left as a clock: "0:42", "1:05".
fn clock(remaining: Duration) -> String {
    let seconds = remaining.as_secs();
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// How long ago a decision was made, for the resolved row.
fn ago(elapsed: Duration) -> String {
    let minutes = elapsed.as_secs() / 60;
    match minutes {
        0 => "just now".into(),
        1..60 => format!("{minutes} min ago"),
        _ => format!("{} h ago", minutes / 60),
    }
}

struct CardState {
    status: ApprovalStatus,
    expired: bool,
    always: bool,
    remaining: Option<Duration>,
    hovered: bool,
    decided_at: Option<Instant>,
    // Copied from the element on every draw, for the countdown to read.
    paused: bool,
    on_deny: Option<DenyHandler>,
    countdown: Option<Task<()>>,
}

impl CardState {
    fn decide(&mut self, status: ApprovalStatus, expired: bool) {
        self.status = status;
        self.expired = expired;
        self.decided_at = Some(Instant::now());
        self.countdown = None;
    }
}

impl RenderOnce for ApprovalCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let warning = theme.warning;
        let danger = theme.danger;
        let success = theme.success;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;

        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| CardState {
            status: self.default_status,
            expired: false,
            always: false,
            remaining: self.expires_in,
            hovered: false,
            decided_at: None,
            paused: false,
            on_deny: None,
            countdown: None,
        });
        let controlled = self.status.is_some();
        let (status, expired, always, remaining, decided_at) = state.update(cx, |state, cx| {
            // A request set back to pending from outside starts fresh.
            if self.status == Some(ApprovalStatus::Pending) && state.decided_at.is_some() {
                state.decided_at = None;
                state.expired = false;
                state.always = false;
                state.remaining = self.expires_in;
            }
            if let Some(status) = self.status {
                if status != ApprovalStatus::Pending && state.decided_at.is_none() {
                    state.decide(status, false);
                }
                state.status = status;
            }
            state.paused = state.hovered || self.busy;
            state.on_deny = self.on_deny.clone();

            let counting = state.status == ApprovalStatus::Pending && state.remaining.is_some();
            if !counting {
                state.countdown = None;
            } else if state.countdown.is_none() {
                state.countdown = Some(cx.spawn_in(window, async move |this, cx| {
                    loop {
                        cx.background_executor().timer(SECOND).await;
                        let running = cx.update(|window, cx| {
                            let Some(this) = this.upgrade() else {
                                return false;
                            };
                            let expired = this.update(cx, |state: &mut CardState, cx| {
                                let left =
                                    state.remaining.and_then(|left| tick(left, state.paused));
                                state.remaining = left;
                                cx.notify();
                                left.is_none().then(|| state.on_deny.clone())
                            });
                            let Some(on_deny) = expired else {
                                return true;
                            };
                            if !controlled {
                                this.update(cx, |state, _| {
                                    state.decide(ApprovalStatus::Denied, true);
                                });
                            } else {
                                this.update(cx, |state, _| state.expired = true);
                            }
                            if let Some(handler) = on_deny {
                                handler(DenyReason::Expired, window, cx);
                            }
                            false
                        });
                        if !running.unwrap_or(false) {
                            break;
                        }
                    }
                }));
            }
            (
                state.status,
                state.expired,
                state.always,
                state.remaining,
                state.decided_at,
            )
        });

        let card = v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(440.))
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .line_height(relative(1.45))
            .shadow(resting_shadow(cx));

        if status != ApprovalStatus::Pending {
            let approved = status == ApprovalStatus::Approved;
            let word = match (approved, expired) {
                (true, _) => "Approved",
                (false, true) => "Expired",
                (false, false) => "Denied",
            };
            let when = decided_at.map(|at| ago(at.elapsed())).unwrap_or_default();
            return card
                .child(
                    h_flex()
                        .gap_2p5()
                        .h(px(44.))
                        .px_3()
                        .child(
                            Icon::new(if approved {
                                IconName::CircleCheck
                            } else {
                                IconName::CircleX
                            })
                            .small()
                            .flex_none()
                            .text_color(if approved {
                                success
                            } else {
                                quiet
                            }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .font_medium()
                                .child(self.title),
                        )
                        .child(
                            div()
                                .flex_none()
                                .text_color(quiet)
                                .child(format!("{word} · {when}")),
                        ),
                )
                .refine_style(&self.style);
        }

        let approve: Handler = {
            let state = state.clone();
            let on_approve = self.on_approve.clone();
            let busy = self.busy;
            Rc::new(move |window, cx| {
                if busy {
                    return;
                }
                if !controlled {
                    state.update(cx, |state, cx| {
                        state.decide(ApprovalStatus::Approved, false);
                        cx.notify();
                    });
                }
                if let Some(handler) = &on_approve {
                    handler(always, window, cx);
                }
            })
        };
        let deny: Handler = {
            let state = state.clone();
            let on_deny = self.on_deny.clone();
            let busy = self.busy;
            Rc::new(move |window, cx| {
                if busy {
                    return;
                }
                if !controlled {
                    state.update(cx, |state, cx| {
                        state.decide(ApprovalStatus::Denied, false);
                        cx.notify();
                    });
                }
                if let Some(handler) = &on_deny {
                    handler(DenyReason::Declined, window, cx);
                }
            })
        };

        let (badge_color, badge_fill) = match self.risk {
            Some(ApprovalRisk::High) => (danger, danger.opacity(0.14)),
            Some(ApprovalRisk::Medium) => (warning, warning.opacity(0.18)),
            _ => (quiet, chip),
        };
        let hint = |keys: &'static str| {
            self.shortcuts
                .then(|| Keystroke::parse(keys).ok().map(Kbd::new))
                .flatten()
        };
        let has_preview = self.preview.is_some() || !self.details.is_empty();

        card.on_hover({
            let state = state.clone();
            move |hovered, _, cx| {
                state.update(cx, |state, _| state.hovered = *hovered);
            }
        })
        .on_key_down({
            let approve = approve.clone();
            let deny = deny.clone();
            move |event, window, cx| {
                let keystroke = &event.keystroke;
                if keystroke.key == "enter" && keystroke.modifiers.secondary(cx) {
                    approve(window, cx);
                    cx.stop_propagation();
                } else if keystroke.key == "escape" && !keystroke.modifiers.modified() {
                    deny(window, cx);
                    cx.stop_propagation();
                }
            }
        })
        .child(
            h_flex()
                .items_start()
                .gap_3()
                .p_4()
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size_8()
                        .rounded(radius)
                        .border_1()
                        .border_color(line)
                        .text_color(quiet)
                        .child(Icon::new(self.icon).small()),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(div().truncate().font_medium().child(self.title))
                        .children(self.description.map(|description| {
                            div().truncate().text_color(quiet).child(description)
                        })),
                )
                .children(self.risk.map(|risk| {
                    h_flex()
                        .flex_none()
                        .gap_1()
                        .px_1p5()
                        .rounded(px(5.))
                        .bg(badge_fill)
                        .text_xs()
                        .font_medium()
                        .child(div().size(px(6.)).rounded(px(3.)).bg(badge_color))
                        .child(risk.label())
                })),
        )
        .when(has_preview, |this| {
            this.child(
                v_flex()
                    .gap_2()
                    .mx_4()
                    .py_3()
                    .border_t_1()
                    .border_color(line)
                    .children(self.preview)
                    .children(self.details.into_iter().map(|(label, value)| {
                        h_flex()
                            .items_start()
                            .gap_3()
                            .child(
                                div()
                                    .flex_none()
                                    .w(px(72.))
                                    .truncate()
                                    .text_color(quiet)
                                    .child(label),
                            )
                            .child(div().flex_1().min_w_0().truncate().child(value))
                    })),
            )
        })
        .child(
            h_flex()
                .flex_wrap()
                .gap_2()
                .px_4()
                .py_3()
                .border_t_1()
                .border_color(line)
                .child(div().flex_1().min_w_0().when(self.allow_always, |this| {
                    this.child(
                        Checkbox::new((self.id.clone(), "always"))
                            .label(self.always_label)
                            .checked(always)
                            .disabled(self.busy)
                            .on_click({
                                let state = state.clone();
                                move |checked, _, cx| {
                                    state.update(cx, |state, cx| {
                                        state.always = *checked;
                                        cx.notify();
                                    })
                                }
                            }),
                    )
                }))
                .children(remaining.map(|left| {
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(if left <= URGENT_FROM { warning } else { quiet })
                        .child(format!("Expires in {}", clock(left)))
                }))
                .when_some(self.on_edit, |this, on_edit| {
                    this.child(
                        Button::new((self.id.clone(), "edit"))
                            .ghost()
                            .small()
                            .label("Edit")
                            .disabled(self.busy)
                            .on_click(move |_, window, cx| on_edit(window, cx)),
                    )
                })
                .child(
                    Button::new((self.id.clone(), "deny"))
                        .ghost()
                        .small()
                        .disabled(self.busy)
                        .child(
                            h_flex()
                                .gap_1p5()
                                .child(self.deny_label)
                                .children(hint("escape")),
                        )
                        .on_click(move |_, window, cx| deny(window, cx)),
                )
                .child(
                    Button::new((self.id.clone(), "approve"))
                        .primary()
                        .small()
                        .loading(self.busy)
                        .child(
                            h_flex()
                                .gap_1p5()
                                .child(self.approve_label)
                                .children(hint("secondary-enter")),
                        )
                        .on_click(move |_, window, cx| approve(window, cx)),
                ),
        )
        .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_countdown_loses_a_second_unless_paused() {
        let ten = Duration::from_secs(10);
        assert_eq!(tick(ten, false), Some(Duration::from_secs(9)));
        assert_eq!(tick(ten, true), Some(ten));
    }

    #[test]
    fn the_request_runs_out_at_zero() {
        assert_eq!(tick(Duration::from_secs(1), false), None);
        assert_eq!(tick(Duration::from_millis(400), false), None);
        // Paused at the last second, it holds.
        assert_eq!(tick(SECOND, true), Some(SECOND));
    }

    #[test]
    fn the_clock_shows_minutes_and_seconds() {
        assert_eq!(clock(Duration::from_secs(42)), "0:42");
        assert_eq!(clock(Duration::from_secs(65)), "1:05");
        assert_eq!(clock(Duration::from_secs(600)), "10:00");
    }

    #[test]
    fn a_decision_ages_in_words() {
        assert_eq!(ago(Duration::from_secs(20)), "just now");
        assert_eq!(ago(Duration::from_secs(150)), "2 min ago");
        assert_eq!(ago(Duration::from_secs(2 * 3600 + 60)), "2 h ago");
    }
}
