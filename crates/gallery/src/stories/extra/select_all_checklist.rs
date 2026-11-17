// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the select-all checklist.

use crate::Story;
use isonomic_ui::{App, AppContext as _, Context, Entity, IntoElement, Render, Window};
use isonomic_ui_extra::select_all_checklist::{ChecklistItem, SelectAllChecklist};

/// Bills picked for Friday's supplier run; one is locked by a dispute. The
/// story applies every change.
pub struct SelectAllChecklistStory {
    items: Vec<ChecklistItem>,
}

impl SelectAllChecklistStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            items: vec![
                ChecklistItem::new("b1", "Harbour Supply, invoice 2291")
                    .note("$1,240.50")
                    .checked(true),
                ChecklistItem::new("b2", "Northline Fuel, March")
                    .note("$612.08")
                    .checked(true),
                ChecklistItem::new("b3", "Kestrel Print, 400 flyers").note("$289.00"),
                ChecklistItem::new("b4", "Bayview Rent, April")
                    .note("$4,200.00")
                    .checked(true),
                ChecklistItem::new("b5", "Oak & Iron, disputed")
                    .note("$980.00")
                    .locked(),
            ],
        })
    }
}

impl Story for SelectAllChecklistStory {
    fn title() -> &'static str {
        "Select-all checklist"
    }

    fn description() -> &'static str {
        "The kit's checkboxes under a parent box that's ticked, empty or part-way, with a count."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for SelectAllChecklistStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let change = cx.entity();
        SelectAllChecklist::new("friday-run", "Pay on Friday", self.items.clone()).on_change(
            move |changes, _, cx| {
                change.update(cx, |this, cx| {
                    for (id, checked) in changes {
                        if let Some(item) = this.items.iter_mut().find(|i| i.id == id) {
                            item.checked = checked;
                        }
                    }
                    cx.notify();
                })
            },
        )
    }
}
