//! Gallery story for the verification code field.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Subscription, Task, Window, div,
};
use isonomic_ui_extra::verification_code_field::{CodeEvent, VerificationCodeField};

/// Signing in to a bookkeeping workspace. The code that works is 246810;
/// anything else is refused. The resend link opens after 30 seconds.
pub struct VerificationCodeFieldStory {
    field: Entity<VerificationCodeField>,
    signed_in: bool,
    countdown: u32,
    _ticker: Task<()>,
    _events: Subscription,
}

const GOOD: &str = "246810";

impl VerificationCodeFieldStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let field = cx.new(|cx| {
                let mut field = VerificationCodeField::new(6, cx).groups(vec![3, 3]);
                field.set_resend_in(Some(30), cx);
                field
            });
            let events = cx.subscribe(&field, |this: &mut Self, field, event: &CodeEvent, cx| match event {
                CodeEvent::Complete(code) => {
                    field.update(cx, |f, cx| f.set_checking(true, cx));
                    let good = code == GOOD;
                    let field = field.clone();
                    cx.spawn(async move |this: isonomic_ui::WeakEntity<Self>, cx: &mut isonomic_ui::AsyncApp| {
                        cx.background_executor().timer(Duration::from_millis(900)).await;
                        this.update(cx, |this, cx| {
                            if good {
                                this.signed_in = true;
                                field.update(cx, |f, cx| f.set_checking(false, cx));
                            } else {
                                field.update(cx, |f, cx| f.set_error(Some("That code didn't match. Check the latest email.".into()), cx));
                            }
                            cx.notify();
                        })
                        .ok();
                    })
                    .detach();
                }
                CodeEvent::Resend => {
                    this.countdown = 30;
                    field.update(cx, |f, cx| {
                        f.set_error(None, cx);
                        f.set_resend_in(Some(30), cx);
                    });
                }
            });
            let ticker = cx.spawn(async move |this: isonomic_ui::WeakEntity<Self>, cx: &mut isonomic_ui::AsyncApp| {
                loop {
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    let alive = this.update(cx, |this, cx| {
                        if this.countdown > 0 {
                            this.countdown -= 1;
                            let left = this.countdown;
                            this.field.update(cx, |f, cx| f.set_resend_in(Some(left), cx));
                        }
                    });
                    if alive.is_err() {
                        break;
                    }
                }
            });
            Self { field, signed_in: false, countdown: 30, _ticker: ticker, _events: events }
        })
    }
}

impl Story for VerificationCodeFieldStory {
    fn title() -> &'static str {
        "Verification code field"
    }

    fn description() -> &'static str {
        "Boxes for a one-time code that fill as you type or paste and report the code when every box is full."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for VerificationCodeFieldStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let success = cx.theme().success;
        v_flex()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .child("Enter the 6-digit code sent to your work email."),
            )
            .child(self.field.clone())
            .child(if self.signed_in {
                div().text_sm().text_color(success).child("Signed in.")
            } else {
                div().text_xs().text_color(quiet).child(format!(
                    "Click the boxes and type or paste. The working code is {GOOD}."
                ))
            })
    }
}
