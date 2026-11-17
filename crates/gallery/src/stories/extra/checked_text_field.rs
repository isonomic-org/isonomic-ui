// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the checked text field.

use crate::Story;
use isonomic_ui::component::{
    ActiveTheme as _,
    button::{Button, ButtonVariants as _},
    v_flex,
};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div, px,
};
use isonomic_ui_extra::checked_text_field::{CheckedTextField, FieldRules, ValueShape};

/// A new-client form: a billing email, a website, and a nine-digit tax number.
pub struct CheckedTextFieldStory {
    email: Entity<CheckedTextField>,
    site: Entity<CheckedTextField>,
    tax: Entity<CheckedTextField>,
    saved: Option<bool>,
}

impl CheckedTextFieldStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let email = cx.new(|cx| {
                CheckedTextField::new(
                    "Billing email",
                    FieldRules {
                        required: true,
                        shape: ValueShape::Email,
                        ..FieldRules::default()
                    },
                    window,
                    cx,
                )
                .placeholder("accounts@client.com", window, cx)
                .hint("Invoices and receipts go here.")
            });
            let site = cx.new(|cx| {
                CheckedTextField::new(
                    "Website",
                    FieldRules {
                        shape: ValueShape::WebAddress,
                        ..FieldRules::default()
                    },
                    window,
                    cx,
                )
                .placeholder("https://", window, cx)
            });
            let tax = cx.new(|cx| {
                CheckedTextField::new(
                    "Tax number",
                    FieldRules {
                        required: true,
                        min_len: Some(9),
                        max_len: Some(9),
                        shape: ValueShape::Digits,
                    },
                    window,
                    cx,
                )
                .hint("Nine digits, no dashes.")
            });
            Self {
                email,
                site,
                tax,
                saved: None,
            }
        })
    }
}

impl Story for CheckedTextFieldStory {
    fn title() -> &'static str {
        "Checked text field"
    }

    fn description() -> &'static str {
        "A single-line field that checks what's typed against simple rules and says what's wrong once the person leaves it."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for CheckedTextFieldStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (quiet, success) = (theme.muted_foreground, theme.success);
        v_flex()
            .w(px(360.))
            .gap_4()
            .child(self.email.clone())
            .child(self.site.clone())
            .child(self.tax.clone())
            .child(
                Button::new("save-client")
                    .primary()
                    .label("Save client")
                    .on_click(cx.listener(|this, _, _, cx| {
                        let fields = [this.email.clone(), this.site.clone(), this.tax.clone()];
                        let ok = fields.iter().fold(true, |ok, field| {
                            field.update(cx, |f, cx| f.check_now(cx)) && ok
                        });
                        this.saved = Some(ok);
                        cx.notify();
                    })),
            )
            .children(self.saved.map(|ok| {
                if ok {
                    div().text_sm().text_color(success).child("Client saved.")
                } else {
                    div()
                        .text_sm()
                        .text_color(quiet)
                        .child("Fix the fields marked in red.")
                }
            }))
    }
}
