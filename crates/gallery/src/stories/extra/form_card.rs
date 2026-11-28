// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the form card.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div,
};
use isonomic_ui_extra::form_card::{FieldKind, FormCard, FormCardEvent, FormField, FormStatus};

/// An assistant asking for the details of a table booking.
pub struct FormCardStory {
    form: Entity<FormCard>,
    sent: SharedString,
    _events: Subscription,
}

impl FormCardStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let options = |list: &[&str]| {
                list.iter()
                    .map(|one| SharedString::from(one.to_string()))
                    .collect()
            };
            let fields = vec![
                FormField::new("name", "Name on the booking", FieldKind::Text)
                    .required()
                    .placeholder("Full name"),
                FormField::new("email", "Email", FieldKind::Email)
                    .required()
                    .description("The confirmation goes here.")
                    .placeholder("you@example.org"),
                FormField::new(
                    "guests",
                    "Guests",
                    FieldKind::Number {
                        min: Some(1.),
                        max: Some(12.),
                    },
                )
                .required()
                .placeholder("2"),
                FormField::new(
                    "seating",
                    "Seating",
                    FieldKind::Select(options(&["Window", "Terrace", "Bar", "No preference"])),
                ),
                FormField::new(
                    "sitting",
                    "Sitting",
                    FieldKind::Choice(options(&["18:00", "20:15"])),
                )
                .required(),
                FormField::new("notes", "Anything we should know", FieldKind::Note)
                    .placeholder("Allergies, a birthday, a pram"),
                FormField::new(
                    "reminder",
                    "Send a reminder the day before",
                    FieldKind::Switch,
                ),
                FormField::new(
                    "policy",
                    "I accept the 24-hour cancellation policy",
                    FieldKind::Checkbox,
                )
                .required(),
            ];
            let form = cx.new(|cx| {
                FormCard::new("Book a table", fields, window, cx)
                    .description("Saturday 14 June at the harbour kitchen")
                    .submit_label("Request booking")
            });
            let events = cx.subscribe(&form, |this: &mut Self, form, event, cx| {
                let FormCardEvent::Submit(values) = event;
                this.sent = format!("Sent {} answers", values.len()).into();
                cx.notify();
                let form = form.downgrade();
                cx.spawn(async move |_, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(900))
                        .await;
                    let _ = form.update(cx, |form, cx| form.set_status(FormStatus::Submitted, cx));
                })
                .detach();
            });
            Self {
                form,
                sent: "Nothing sent yet".into(),
                _events: events,
            }
        })
    }
}

impl Story for FormCardStory {
    fn title() -> &'static str {
        "Form card"
    }

    fn description() -> &'static str {
        "A short form inside a conversation: checked fields, a submit that waits, then a summary with a way back."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for FormCardStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().gap_4().child(self.form.clone()).child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(self.sent.clone()),
        )
    }
}
