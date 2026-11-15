//! Toast queue: short notices that stack in a corner, leave on their own after a while, wait while the pointer rests on them, and can carry one action such as Undo.
//!
//! The newest notice sits nearest the edge. Only a few show at once; the
//! rest wait behind a "+2 more" line and move up as others leave. Each
//! notice has a tone, a title, an optional line and an optional action;
//! errors stay until closed. The queue reports actions and dismissals.

use std::time::Duration;

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    Context, ElementId, EventEmitter, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, Role, SharedString, StatefulInteractiveElement as _, Styled as _, Task, Window, div,
    prelude::FluentBuilder as _, px,
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
}

impl Toast {
    /// Create a notice with a title.
    pub fn new(tone: Tone, title: impl Into<SharedString>) -> Self {
        Self {
            tone,
            title: title.into(),
            body: None,
            action: None,
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
}

impl EventEmitter<ToastEvent> for ToastQueue {}

impl Default for ToastQueue {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            next_id: 1,
            room: 3,
            resting: None,
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
                    if this.resting == Some(id) {
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

impl Render for ToastQueue {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let fill = theme.popover;
        let line = theme.border;
        let quiet = theme.muted_foreground;
        let shadow = lifted_shadow(cx);
        let (shown, waiting) = shown_and_waiting(self.entries.len(), self.room);
        let start = self.entries.len() - shown;

        let cards: Vec<_> = self.entries[start..]
            .iter()
            .rev()
            .map(|entry| {
                let id = entry.id;
                let toast = &entry.toast;
                let tone = toast.tone.color(cx);
                let icon = match toast.tone {
                    Tone::Success => IconName::CircleCheck,
                    Tone::Danger => IconName::CircleX,
                    Tone::Warning => IconName::TriangleAlert,
                    _ => IconName::Info,
                };
                h_flex()
                    .id(ElementId::Name(format!("toast-{id}").into()))
                    .w(px(340.))
                    .items_start()
                    .gap_2()
                    .p_3()
                    .rounded(px(10.))
                    .border_1()
                    .border_color(line)
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
                    .on_hover(cx.listener(move |this, hovered: &bool, _, _| {
                        this.resting = if *hovered { Some(id) } else { None };
                    }))
                    .child(Icon::new(icon).small().text_color(tone).mt_0p5())
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(div().text_sm().font_medium().child(toast.title.clone()))
                            .when_some(toast.body.clone(), |this, body| {
                                this.child(div().text_xs().text_color(quiet).child(body))
                            }),
                    )
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

        v_flex()
            .gap_2()
            .items_end()
            .children(cards)
            .when(waiting > 0, |this| {
                this.child(
                    div()
                        .text_xs()
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
}
