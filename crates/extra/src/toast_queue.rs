// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Toast queue: short notices that stack in a corner, leave on their own after a while, wait while the pointer rests on them, and can carry one action such as Undo.
//!
//! The newest notice sits nearest the edge. Only a few show at once; the
//! rest wait behind a "+2 more" line and move up as others leave. Each
//! notice has a tone, a title, an optional line and an optional action;
//! errors stay until closed. The queue reports actions and dismissals.
//!
//! A stacked queue folds the older notices behind the newest one, showing
//! only their edges, and opens them out while the pointer is over it. While
//! the pointer rests on the queue no notice leaves. A notice can be swiped
//! sideways to close it: dragged past a third of its width it goes, short of
//! that it springs back. A notice can carry a second, quieter action.

use std::time::Duration;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::look;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    Context, ElementId, EventEmitter, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, Render, Role, SharedString,
    StatefulInteractiveElement as _, Styled as _, Task, Window, div, prelude::FluentBuilder as _,
    px,
};

use crate::common::{Tone, lifted_shadow};

/// One notice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    /// The colour and icon it carries.
    pub tone: Tone,
    /// The main line.
    pub title: SharedString,
    /// An optional second line.
    pub body: Option<SharedString>,
    /// An optional action label, such as "Undo".
    pub action: Option<SharedString>,
    /// An optional second, quieter action, such as "View".
    pub second_action: Option<SharedString>,
}

impl Toast {
    /// Create a notice with a title.
    pub fn new(tone: Tone, title: impl Into<SharedString>) -> Self {
        Self {
            tone,
            title: title.into(),
            body: None,
            action: None,
            second_action: None,
        }
    }

    /// Add a second line.
    pub fn body(mut self, body: impl Into<SharedString>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Add an action.
    pub fn action(mut self, label: impl Into<SharedString>) -> Self {
        self.action = Some(label.into());
        self
    }

    /// Add a second, quieter action, shown before the first.
    pub fn second_action(mut self, label: impl Into<SharedString>) -> Self {
        self.second_action = Some(label.into());
        self
    }
}

/// The width of a notice.
const CARD_WIDTH: f32 = 340.;
/// How much of an older notice shows below the one in front, folded.
const PEEK: f32 = 6.;
/// How much narrower each folded notice is than the one in front of it.
const PEEK_NARROWING: f32 = 16.;

/// Whether a sideways drag of `dx` closes a notice `width` wide: a third of
/// the way, either way.
pub fn swipe_dismisses(dx: f32, width: f32) -> bool {
    dx.abs() >= width / 3.
}

/// How opaque a notice is while dragged `dx` sideways: fading as it goes,
/// never below a fifth.
pub fn swipe_opacity(dx: f32, width: f32) -> f32 {
    if width <= 0. {
        return 1.;
    }
    (1. - dx.abs() / width).clamp(0.2, 1.)
}

/// The folded edges a stacked queue shows below its front notice: for each
/// older notice that shows, its width.
pub fn folded_widths(shown: usize, width: f32) -> Vec<f32> {
    (1..shown.min(3))
        .map(|rank| (width - PEEK_NARROWING * rank as f32).max(0.))
        .collect()
}

/// How long a notice stays: errors until closed, notices with an action longer.
pub fn stay_for(toast: &Toast) -> Option<Duration> {
    match (toast.tone, toast.action.is_some()) {
        (Tone::Danger, _) => None,
        (_, true) => Some(Duration::from_secs(8)),
        _ => Some(Duration::from_secs(5)),
    }
}

/// How many show and how many wait, for `count` notices and room for `room`.
pub fn shown_and_waiting(count: usize, room: usize) -> (usize, usize) {
    let shown = count.min(room.max(1));
    (shown, count - shown)
}

/// What the queue reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastEvent {
    /// A notice's action was clicked; the notice leaves.
    Action(u64),
    /// A notice's second action was clicked; the notice leaves.
    SecondAction(u64),
    /// A notice left, closed or timed out.
    Dismissed(u64),
}

struct Entry {
    id: u64,
    toast: Toast,
    _timer: Option<Task<()>>,
}

/// A stack of notices.
pub struct ToastQueue {
    entries: Vec<Entry>,
    next_id: u64,
    room: usize,
    resting: Option<u64>,
    /// Whether the pointer is anywhere over the queue.
    hovered: bool,
    stacked: bool,
    swipe: Option<Swipe>,
}

/// A notice being dragged sideways.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Swipe {
    id: u64,
    start: f32,
    dx: f32,
}

impl EventEmitter<ToastEvent> for ToastQueue {}

impl Default for ToastQueue {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            next_id: 1,
            room: 3,
            resting: None,
            hovered: false,
            stacked: false,
            swipe: None,
        }
    }
}

impl ToastQueue {
    /// Create an empty queue showing three at a time.
    pub fn new() -> Self {
        Self::default()
    }

    /// Show this many at a time.
    pub fn room(mut self, room: usize) -> Self {
        self.room = room.max(1);
        self
    }

    /// Fold older notices behind the newest, opening them out under the
    /// pointer.
    pub fn stacked(mut self, stacked: bool) -> Self {
        self.stacked = stacked;
        self
    }

    /// Add a notice and get its id.
    pub fn push(&mut self, toast: Toast, cx: &mut Context<Self>) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let timer = stay_for(&toast).map(|wait| self.timer(id, wait, cx));
        self.entries.push(Entry {
            id,
            toast,
            _timer: timer,
        });
        cx.notify();
        id
    }

    /// Take a notice away.
    pub fn dismiss(&mut self, id: u64, cx: &mut Context<Self>) {
        let before = self.entries.len();
        self.entries.retain(|e| e.id != id);
        if self.entries.len() != before {
            cx.emit(ToastEvent::Dismissed(id));
            cx.notify();
        }
    }

    /// How many notices are queued.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn timer(&self, id: u64, wait: Duration, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(
            async move |this: isonomic_ui::WeakEntity<Self>, cx: &mut isonomic_ui::AsyncApp| {
                cx.background_executor().timer(wait).await;
                this.update(cx, |this, cx| {
                    if this.resting == Some(id) || this.hovered {
                        let again = this.timer(id, Duration::from_secs(2), cx);
                        if let Some(entry) = this.entries.iter_mut().find(|e| e.id == id) {
                            entry._timer = Some(again);
                        }
                    } else {
                        this.dismiss(id, cx);
                    }
                })
                .ok();
            },
        )
    }
}

impl ToastQueue {
    fn start_swipe(&mut self, id: u64, event: &MouseDownEvent, cx: &mut Context<Self>) {
        self.swipe = Some(Swipe {
            id,
            start: event.position.x.into(),
            dx: 0.,
        });
        cx.notify();
    }

    fn move_swipe(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(swipe) = self.swipe.as_mut() else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            self.swipe = None;
            cx.notify();
            return;
        }
        let x: f32 = event.position.x.into();
        swipe.dx = x - swipe.start;
        cx.notify();
    }

    fn end_swipe(&mut self, _: &MouseUpEvent, cx: &mut Context<Self>) {
        let Some(swipe) = self.swipe.take() else {
            return;
        };
        if swipe_dismisses(swipe.dx, CARD_WIDTH) {
            self.dismiss(swipe.id, cx);
        }
        cx.notify();
    }
}

impl Render for ToastQueue {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let fill = theme.popover;
        let quiet = theme.muted_foreground;
        let shadow = lifted_shadow(cx);
        let (shown, waiting) = shown_and_waiting(self.entries.len(), self.room);
        let start = self.entries.len() - shown;
        // Folded, only the newest shows in full; the pointer opens them out.
        let open = look::hover_fade("toast-queue", !self.stacked || self.hovered, window, cx);
        let folded = self.stacked && open < 1.;
        let swipe = self.swipe;

        let cards: Vec<_> = self.entries[start..]
            .iter()
            .rev()
            .enumerate()
            .filter(|(rank, _)| !folded || *rank == 0)
            .map(|(rank, entry)| {
                let id = entry.id;
                let toast = &entry.toast;
                let tone = toast.tone.color(cx);
                let icon = match toast.tone {
                    Tone::Success => IconName::CircleCheck,
                    Tone::Danger => IconName::CircleX,
                    Tone::Warning => IconName::TriangleAlert,
                    _ => IconName::Info,
                };
                let dragged = swipe.filter(|s| s.id == id).map_or(0., |s| s.dx);
                // Released short of closing, it springs back.
                let offset: f32 = isonomic_ui::base::spring(
                    (ElementId::Name(format!("toast-{id}-swipe").into()), "x"),
                    px(dragged),
                    cx.theme().motion_tokens().spring_move,
                    window,
                    cx,
                )
                .into();
                h_flex()
                    .id(ElementId::Name(format!("toast-{id}").into()))
                    .relative()
                    .left(px(offset))
                    .opacity(swipe_opacity(offset, CARD_WIDTH) * if rank == 0 { 1. } else { open })
                    .w(px(CARD_WIDTH))
                    .items_start()
                    .gap_2()
                    .p_3()
                    .rounded(px(10.))
                    .bg(fill)
                    .shadow(shadow.clone())
                    .role(if toast.tone == Tone::Danger {
                        Role::Alert
                    } else {
                        Role::Status
                    })
                    .aria_label(match &toast.body {
                        Some(body) => format!("{}. {body}", toast.title),
                        None => toast.title.to_string(),
                    })
                    .aria_description("Swipe sideways to close")
                    .on_hover(cx.listener(move |this, hovered: &bool, _, _| {
                        this.resting = if *hovered { Some(id) } else { None };
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            this.start_swipe(id, event, cx)
                        }),
                    )
                    .child(Icon::new(icon).small().text_color(tone).mt_0p5())
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(div().text_body().font_medium().child(toast.title.clone()))
                            .when_some(toast.body.clone(), |this, body| {
                                this.child(div().text_caption().text_color(quiet).child(body))
                            }),
                    )
                    .when_some(toast.second_action.clone(), |this, label| {
                        this.child(
                            Button::new(ElementId::Name(format!("toast-{id}-second").into()))
                                .xsmall()
                                .ghost()
                                .label(label)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    cx.emit(ToastEvent::SecondAction(id));
                                    this.entries.retain(|e| e.id != id);
                                    cx.notify();
                                })),
                        )
                    })
                    .when_some(toast.action.clone(), |this, label| {
                        this.child(
                            Button::new(ElementId::Name(format!("toast-{id}-action").into()))
                                .xsmall()
                                .outline()
                                .label(label)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    cx.emit(ToastEvent::Action(id));
                                    this.entries.retain(|e| e.id != id);
                                    cx.notify();
                                })),
                        )
                    })
                    .child(
                        Button::new(ElementId::Name(format!("toast-{id}-close").into()))
                            .xsmall()
                            .ghost()
                            .icon(IconName::Close)
                            .accessibility_label("Close notice")
                            .on_click(cx.listener(move |this, _, _, cx| this.dismiss(id, cx))),
                    )
            })
            .collect();
        let edges = if folded {
            folded_widths(shown, CARD_WIDTH)
        } else {
            Vec::new()
        };

        v_flex()
            .id("toast-queue")
            .gap_2()
            .items_end()
            .on_hover(cx.listener(|this, hovered: &bool, window, cx| {
                this.hovered = *hovered;
                window.refresh();
                cx.notify();
            }))
            .on_mouse_move(
                cx.listener(|this, event: &MouseMoveEvent, _, cx| this.move_swipe(event, cx)),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &MouseUpEvent, _, cx| this.end_swipe(event, cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, event: &MouseUpEvent, _, cx| this.end_swipe(event, cx)),
            )
            .children(cards)
            .when(!edges.is_empty(), |this| {
                // The folded notices: edges tucked under the front one.
                this.child(
                    v_flex()
                        .items_center()
                        .w(px(CARD_WIDTH))
                        .mt(px(-8.))
                        .children(edges.into_iter().enumerate().map(|(ix, width)| {
                            div()
                                .w(px(width))
                                .h(px(PEEK + 4.))
                                .mt(px(-4.))
                                .rounded_b(px(10.))
                                .bg(fill)
                                .shadow(shadow.clone())
                                .opacity(1. - 0.25 * (ix + 1) as f32)
                        })),
                )
            })
            .when(waiting > 0, |this| {
                this.child(
                    div()
                        .text_caption()
                        .text_color(quiet)
                        .child(format!("+{waiting} more")),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notices_stay_by_kind_and_overflow_waits() {
        assert_eq!(stay_for(&Toast::new(Tone::Danger, "Sync failed")), None);
        assert_eq!(
            stay_for(&Toast::new(Tone::Success, "Archived").action("Undo")),
            Some(Duration::from_secs(8))
        );
        assert_eq!(
            stay_for(&Toast::new(Tone::Info, "Saved").body("All changes")),
            Some(Duration::from_secs(5))
        );
        assert_eq!(shown_and_waiting(5, 3), (3, 2));
        assert_eq!(shown_and_waiting(2, 3), (2, 0));
        assert_eq!(shown_and_waiting(1, 0), (1, 0));
        let queue = ToastQueue::new().room(2);
        assert!(queue.is_empty() && queue.len() == 0 && queue.room == 2);
    }

    #[test]
    fn a_swipe_past_a_third_closes() {
        assert!(!swipe_dismisses(0., 340.));
        assert!(!swipe_dismisses(100., 340.));
        assert!(swipe_dismisses(120., 340.));
        assert!(swipe_dismisses(-120., 340.));
        assert_eq!(swipe_opacity(0., 340.), 1.);
        assert_eq!(swipe_opacity(170., 340.), 0.5);
        assert_eq!(swipe_opacity(-500., 340.), 0.2);
        assert_eq!(swipe_opacity(10., 0.), 1.);
    }

    #[test]
    fn folded_edges_narrow_and_stop_at_two() {
        assert!(folded_widths(1, 340.).is_empty());
        assert_eq!(folded_widths(2, 340.), vec![324.]);
        assert_eq!(folded_widths(5, 340.), vec![324., 308.]);
    }
}
