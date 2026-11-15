//! Gallery story for the toast queue.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, Sizable as _, button::Button, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Subscription, Window, div, px,
};
use isonomic_ui_extra::common::Tone;
use isonomic_ui_extra::toast_queue::{Toast, ToastEvent, ToastQueue};

/// Bookkeeping notices: an archived invoice you can undo, a paid invoice, a
/// failed bank sync that stays until closed. Rest the pointer on a notice
/// to keep it.
pub struct ToastQueueStory {
    queue: Entity<ToastQueue>,
    archived: usize,
    undone: usize,
    _events: Subscription,
}

impl ToastQueueStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let queue = cx.new(|_| ToastQueue::new());
            let events = cx.subscribe(&queue, |this: &mut Self, _, event: &ToastEvent, cx| {
                if let ToastEvent::Action(_) = event {
                    this.undone += 1;
                    cx.notify();
                }
            });
            Self {
                queue,
                archived: 0,
                undone: 0,
                _events: events,
            }
        })
    }

    fn push(&mut self, toast: Toast, cx: &mut Context<Self>) {
        self.queue.update(cx, |queue, cx| queue.push(toast, cx));
    }
}

impl Story for ToastQueueStory {
    fn title() -> &'static str {
        "Toast queue"
    }

    fn description() -> &'static str {
        "Short notices that stack in a corner, leave on their own, wait under the pointer, and can carry one action."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ToastQueueStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (line, quiet) = (theme.border, theme.muted_foreground);
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("archive")
                            .small()
                            .outline()
                            .label("Archive an invoice")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.archived += 1;
                                let number = 2040 + this.archived;
                                this.push(
                                    Toast::new(Tone::Success, format!("INV-{number} archived"))
                                        .action("Undo"),
                                    cx,
                                );
                            })),
                    )
                    .child(
                        Button::new("paid")
                            .small()
                            .outline()
                            .label("Record a payment")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.push(
                                    Toast::new(Tone::Info, "Payment recorded")
                                        .body("$4,200.00 from Harbor Dental"),
                                    cx,
                                );
                            })),
                    )
                    .child(
                        Button::new("fail")
                            .small()
                            .outline()
                            .label("Fail a sync")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.push(
                                    Toast::new(Tone::Danger, "Bank sync failed").body(
                                        "The bank asked to sign in again. Reconnect in Settings.",
                                    ),
                                    cx,
                                );
                            })),
                    ),
            )
            .child(div().text_xs().text_color(quiet).child(format!(
                "Undone {} of {} archives.",
                self.undone, self.archived
            )))
            .child(
                div()
                    .relative()
                    .w(px(560.))
                    .h(px(320.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(line)
                    .child(
                        div()
                            .absolute()
                            .bottom_3()
                            .right_3()
                            .child(self.queue.clone()),
                    ),
            )
    }
}
