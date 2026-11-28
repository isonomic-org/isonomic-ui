// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the typed confirm dialog.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, Sizable as _, button::Button, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div, prelude::FluentBuilder as _,
};
use isonomic_ui_extra::typed_confirm_dialog::{ConfirmEvent, TypedConfirmDialog};

/// Voiding an invoice. The story shows and hides the dialog and says what
/// was chosen.
pub struct TypedConfirmDialogStory {
    dialog: Entity<TypedConfirmDialog>,
    open: bool,
    result: Option<&'static str>,
    _events: Subscription,
}

impl TypedConfirmDialogStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let dialog = cx.new(|cx| {
                let mut dialog = TypedConfirmDialog::new(
                    "Void invoice INV-2240?",
                    "INV-2240",
                    "Void invoice",
                    window,
                    cx,
                );
                dialog.set_consequences(
                    vec![
                        SharedString::from("Harbour Supply's balance drops by $1,240.50."),
                        "The March figures sent on the 3rd will no longer match.".into(),
                        "The invoice number can't be used again.".into(),
                    ],
                    cx,
                );
                dialog
            });
            let events = cx.subscribe(&dialog, |this: &mut Self, _, event, cx| {
                this.open = false;
                this.result = Some(match event {
                    ConfirmEvent::Confirm => "Voided",
                    ConfirmEvent::Cancel => "Backed out",
                });
                cx.notify();
            });
            Self {
                dialog,
                open: true,
                result: None,
                _events: events,
            }
        })
    }
}

impl Story for TypedConfirmDialogStory {
    fn title() -> &'static str {
        "Typed confirm dialog"
    }

    fn description() -> &'static str {
        "A confirmation for actions that can't be undone, listing what will happen and asking for a word to be typed first."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for TypedConfirmDialogStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        v_flex()
            .gap_3()
            .when(self.open, |this| this.child(self.dialog.clone()))
            .when(!self.open, |this| {
                this.child(
                    Button::new("reopen")
                        .small()
                        .outline()
                        .label("Void invoice INV-2240\u{2026}")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.dialog.update(cx, |d, cx| d.reset(window, cx));
                            this.open = true;
                            cx.notify();
                        })),
                )
            })
            .children(
                self.result
                    .map(|r| div().text_xs().text_color(quiet).child(r)),
            )
    }
}
