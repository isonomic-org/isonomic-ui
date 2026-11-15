//! Gallery story for the writing assistant card.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div,
};
use isonomic_ui_extra::writing_assistant_card::{WritingAssistantCard, WritingAssistantEvent};

/// Made-up replies for a bookkeeping app's writing helper.
fn reply(message: &str) -> String {
    let asked = message.to_lowercase();
    if asked.contains("invoice") {
        "Here's a reminder you can send: \"Hi Mara, invoice 2291 for 1,840.00 was due on the 3rd. Could you let me know when it will be paid? Happy to resend it if it went astray.\"".into()
    } else if asked.contains("payroll") {
        "Here you go: \"The payroll file for October is ready. Could you look it over by Wednesday so it runs on time?\"".into()
    } else if asked.contains("short") {
        "Shorter: \"The September figures are in and match the bank. We can close on Friday.\""
            .into()
    } else if asked.contains("tone") {
        "Softer: \"Thanks for bearing with us on the quarter-end figures. They're checked now, and I'd be glad to talk them through.\"".into()
    } else {
        format!(
            "A first go at \"{}\": tell me who it's for and I'll tune it.",
            message.trim()
        )
    }
}

/// A writing helper with four suggestions and simulated dictation.
pub struct WritingAssistantCardStory {
    card: Entity<WritingAssistantCard>,
    last: SharedString,
    _events: Subscription,
}

impl WritingAssistantCardStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let card = cx.new(|cx| {
                WritingAssistantCard::new(window, cx)
                    .name("Scribe")
                    .tagline("Writes with your books in mind.")
                    .suggestion("Chase a late invoice")
                    .suggestion("Make it shorter")
                    .suggestion("Fix the tone")
                    .suggestion("Draft a reply")
                    .respond(reply)
            });
            let events = cx.subscribe(&card, |this: &mut Self, _, event, cx| {
                this.last = match event {
                    WritingAssistantEvent::Sent(message) => format!("Sent: {message}"),
                    WritingAssistantEvent::Replied(_) => "Reply finished".to_string(),
                    WritingAssistantEvent::NewChat => "New chat".to_string(),
                    WritingAssistantEvent::Listening(true) => "Listening".to_string(),
                    WritingAssistantEvent::Listening(false) => "Stopped listening".to_string(),
                }
                .into();
                cx.notify();
            });
            Self {
                card,
                last: "Nothing yet".into(),
                _events: events,
            }
        })
    }
}

impl Story for WritingAssistantCardStory {
    fn title() -> &'static str {
        "Writing assistant card"
    }

    fn description() -> &'static str {
        "A small helper for rewriting and drafting text."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for WritingAssistantCardStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().gap_3().child(self.card.clone()).child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(self.last.clone()),
        )
    }
}
