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

/// A customer form: a checked email, an optional reference and a capped note.
pub struct LabelledFieldStory {
    email: Entity<InputState>,
    reference: Entity<InputState>,
    note: Entity<TextareaState>,
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
            Self {
                email,
                reference,
                note,
                _events: vec![a, b, c],
            }
        })
    }
}

impl Story for LabelledFieldStory {
    fn title() -> &'static str {
        "Labelled field"
    }

    fn description() -> &'static str {
        "Any control with its label, a required or optional mark, a hint, an error and a character count, laid out the same way everywhere."
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
        v_flex()
            .w(px(380.))
            .gap_5()
            .child(
                LabelledField::new("email", "Billing email")
                    .required(true)
                    .hint("Invoices and receipts go here")
                    .error(email_error)
                    .child(Input::new(&self.email)),
            )
            .child(
                LabelledField::new("reference", "Their reference")
                    .required(false)
                    .hint("Printed beside the invoice number")
                    .child(Input::new(&self.reference)),
            )
            .child(
                LabelledField::new("note", "Invoice note")
                    .hint("Payment terms or bank details")
                    .count(note_len, 140)
                    .child(Textarea::new(&self.note)),
            )
    }
}
