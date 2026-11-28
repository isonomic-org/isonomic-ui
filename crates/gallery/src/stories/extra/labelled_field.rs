// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the labelled field.

use crate::Story;
use isonomic_ui::component::{
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    v_flex,
};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, px,
};
use isonomic_ui_extra::labelled_field::LabelledField;

/// A customer form: a checked email, an optional reference, a search, a
/// capped note and a note with an error.
pub struct LabelledFieldStory {
    email: Entity<InputState>,
    reference: Entity<InputState>,
    lookup: Entity<InputState>,
    note: Entity<TextareaState>,
    terms: Entity<TextareaState>,
    _events: Vec<Subscription>,
}

impl LabelledFieldStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let email = cx.new(|cx| {
                let mut s = InputState::new(window, cx).placeholder("name@example.com");
                s.set_value("accounts@northwind", window, cx);
                s
            });
            let reference = cx.new(|cx| InputState::new(window, cx).placeholder("PO number"));
            let lookup = cx.new(|cx| {
                let mut s = InputState::new(window, cx).placeholder("Name or account number");
                s.set_value("Harbour", window, cx);
                s
            });
            let terms = cx.new(|cx| {
                let mut s = TextareaState::new(window, cx).auto_grow(2, 4);
                s.set_value("Net 90", window, cx);
                s
            });
            let note = cx.new(|cx| {
                TextareaState::new(window, cx)
                    .auto_grow(2, 5)
                    .placeholder("Shown on every invoice")
            });
            let refresh = |cx: &mut Context<Self>, e: &Entity<InputState>| {
                cx.subscribe(e, |_, _, _: &InputEvent, cx| cx.notify())
            };
            let a = refresh(cx, &email);
            let b = refresh(cx, &reference);
            let c = cx.subscribe(&note, |_, _, _: &InputEvent, cx| cx.notify());
            let d = cx.subscribe(&terms, |_, _, _: &InputEvent, cx| cx.notify());
            Self {
                email,
                reference,
                lookup,
                note,
                terms,
                _events: vec![a, b, c, d],
            }
        })
    }
}

impl Story for LabelledFieldStory {
    fn title() -> &'static str {
        "Labelled field"
    }

    fn description() -> &'static str {
        "Any control with its label, a required or optional mark, a description, a hint, an error and a character count, wired into the text field it labels."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for LabelledFieldStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let email = self.email.read(cx).value().to_string();
        let email_error: Option<SharedString> = (!email.is_empty()
            && !(email.contains('@') && email.rsplit('@').next().is_some_and(|d| d.contains('.'))))
        .then(|| "That doesn't look like a full address".into());
        let note_len = self.note.read(cx).value().chars().count();
        let terms = self.terms.read(cx).value().to_string();
        let terms_error: Option<SharedString> = (terms.trim().chars().count() < 12)
            .then(|| "Spell out the terms, at least 12 characters".into());
        v_flex()
            .w(px(380.))
            .gap_5()
            .child(
                LabelledField::new("email", "Billing email")
                    .required(true)
                    .hint("Invoices and receipts go here")
                    .error(email_error)
                    .input(Input::new(&self.email)),
            )
            .child(
                LabelledField::new("reference", "Their reference")
                    .required(false)
                    .description("Their purchase order or cost centre")
                    .hint("Printed beside the invoice number")
                    .input(Input::new(&self.reference)),
            )
            .child(
                LabelledField::new("lookup", "Find a customer")
                    .hint("Escape clears the search")
                    .input(Input::new(&self.lookup).search(true)),
            )
            .child(
                LabelledField::new("note", "Invoice note")
                    .hint("Payment terms or bank details")
                    .count(note_len, 140)
                    .textarea(Textarea::new(&self.note)),
            )
            .child(
                LabelledField::new("terms", "Payment terms")
                    .required(true)
                    .description("Shown in the footer of every invoice")
                    .error(terms_error)
                    .textarea(Textarea::new(&self.terms)),
            )
    }
}
