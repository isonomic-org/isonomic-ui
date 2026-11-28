// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the confirmation card.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::v_flex;
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window,
};
use isonomic_ui_extra::confirmation_card::{
    ConfirmationCard, ConfirmationItem, ConfirmationStatus,
};

const UNDO_SECONDS: u32 = 20;

/// An assistant about to order shelving, from review to done and undone.
pub struct ConfirmationCardStory {
    shelves: u32,
    brackets: Option<u32>,
    status: ConfirmationStatus,
    undo_left: u32,
}

impl ConfirmationCardStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            shelves: 2,
            brackets: Some(8),
            status: ConfirmationStatus::Review,
            undo_left: UNDO_SECONDS,
        })
    }

    fn items(&self) -> Vec<ConfirmationItem> {
        let mut items = vec![
            ConfirmationItem::new("shelf", "Oak wall shelf, 90 cm", 64.)
                .detail("Oiled, two per box")
                .quantity(self.shelves)
                .editable(),
        ];
        if let Some(brackets) = self.brackets {
            items.push(
                ConfirmationItem::new("brackets", "Steel brackets", 7.25)
                    .detail("Black, with screws")
                    .quantity(brackets)
                    .editable()
                    .removable(),
            );
        }
        items.push(ConfirmationItem::new("oil", "Hard-wax oil, 250 ml", 19.9));
        items
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        self.status = ConfirmationStatus::Confirming;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1200))
                .await;
            let _ = this.update(cx, |this, cx| {
                this.status = ConfirmationStatus::Done;
                this.undo_left = UNDO_SECONDS;
                cx.notify();
            });
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let going = this
                    .update(cx, |this, cx| {
                        if this.status != ConfirmationStatus::Done || this.undo_left == 0 {
                            return false;
                        }
                        this.undo_left -= 1;
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !going {
                    break;
                }
            }
        })
        .detach();
    }
}

impl Story for ConfirmationCardStory {
    fn title() -> &'static str {
        "Confirmation card"
    }

    fn description() -> &'static str {
        "An itemised action to confirm: quantities, totals, warnings, then a done row with a timed undo."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ConfirmationCardStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quantity = cx.entity();
        let remove = cx.entity();
        let confirm = cx.entity();
        let undo = cx.entity();
        v_flex()
            .gap_6()
            .child(
                ConfirmationCard::new("order", "Order shelving for the hallway")
                    .subtitle("From the timber yard, delivered Thursday")
                    .items(self.items())
                    .fee("Delivery", 12.)
                    .fee("Trade discount", -15.)
                    .warning("The yard only refunds unopened boxes.")
                    .reference("TY-48213")
                    .confirm_label("Place order")
                    .done_label("Ordered")
                    .status(self.status)
                    .undo_window(self.undo_left, UNDO_SECONDS)
                    .on_quantity(move |id, count, _, cx| {
                        quantity.update(cx, |this, cx| {
                            if id == "shelf" {
                                this.shelves = count;
                            } else {
                                this.brackets = Some(count);
                            }
                            cx.notify();
                        })
                    })
                    .on_remove(move |_, _, cx| {
                        remove.update(cx, |this, cx| {
                            this.brackets = None;
                            cx.notify();
                        })
                    })
                    .on_cancel(|_, _| {})
                    .on_edit(|_, _| {})
                    .on_confirm(move |_, cx| confirm.update(cx, |this, cx| this.confirm(cx)))
                    .on_undo(move |_, cx| {
                        undo.update(cx, |this, cx| {
                            this.status = ConfirmationStatus::Review;
                            cx.notify();
                        })
                    }),
            )
            .child(
                ConfirmationCard::new("booked", "Book the van for Saturday")
                    .items(vec![ConfirmationItem::new("van", "Van, half day", 55.)])
                    .status(ConfirmationStatus::Done)
                    .reference("VAN-0912")
                    .done_note("Booked today at 14:05")
                    .done_label("Booked"),
            )
    }
}
