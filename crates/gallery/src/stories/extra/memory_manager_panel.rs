// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the memory manager panel.

use crate::Story;
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Subscription, Window, div,
};
use isonomic_ui_extra::memory_manager_panel::{MemoryEvent, MemoryItem, MemoryManagerPanel};

/// What a bookkeeping assistant remembers about one business, its team and
/// its clients. The story keeps the items and applies every change the
/// panel reports, including moving an item to another scope and forgetting
/// a whole scope.
pub struct MemoryManagerPanelStory {
    panel: Entity<MemoryManagerPanel>,
    items: Vec<MemoryItem>,
    removed: Vec<(usize, MemoryItem)>,
    _events: Subscription,
}

fn items() -> Vec<MemoryItem> {
    vec![
        MemoryItem::new(
            "m1",
            "Suppliers are paid on Fridays, from the operating account.",
            "Payables setup",
        )
        .updated("2 weeks ago")
        .scope("Business"),
        MemoryItem::new(
            "m2",
            "Fuel receipts go under Vehicle costs, not Office supplies.",
            "Receipt filing",
        )
        .updated("5 days ago")
        .scope("Business"),
        MemoryItem::new("m3", "Maya approves anything over $1,000.", "Approvals")
            .updated("1 month ago")
            .scope("Team"),
        MemoryItem::new(
            "m4",
            "Northwind Dental pays late but always pays; send gentle reminders.",
            "Collections",
        )
        .updated("yesterday")
        .scope("Clients"),
        MemoryItem::new("m5", "The year end is 31 December.", "Onboarding")
            .updated("3 months ago")
            .scope("Business"),
        MemoryItem::new(
            "m6",
            "Round card matches to three days either side.",
            "March close",
        )
        .updated("today")
        .paused(true)
        .scope("Team"),
        MemoryItem::new(
            "m7",
            "Lena prefers invoices summarised in one line.",
            "Weekly review",
        )
        .updated("4 days ago")
        .scope("Team"),
    ]
}

impl MemoryManagerPanelStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let panel = cx.new(|cx| MemoryManagerPanel::new(items(), window, cx));
            let events = cx.subscribe(&panel, |this: &mut Self, panel, event, cx| {
                match event {
                    MemoryEvent::Edited { id, text } => {
                        if let Some(item) = this.items.iter_mut().find(|i| &i.id == id) {
                            item.text = text.clone();
                            item.updated = "just now".into();
                        }
                    }
                    MemoryEvent::Paused { id, paused } => {
                        if let Some(item) = this.items.iter_mut().find(|i| &i.id == id) {
                            item.paused = *paused;
                        }
                    }
                    MemoryEvent::Removed(id) => {
                        if let Some(at) = this.items.iter().position(|i| &i.id == id) {
                            let item = this.items.remove(at);
                            this.removed.push((at, item));
                        }
                    }
                    MemoryEvent::Undo(_) => {
                        if let Some((at, item)) = this.removed.pop() {
                            this.items.insert(at.min(this.items.len()), item);
                        }
                    }
                    MemoryEvent::Rescoped { id, scope } => {
                        if let Some(item) = this.items.iter_mut().find(|i| &i.id == id) {
                            item.scope = scope.clone();
                        }
                    }
                    MemoryEvent::ForgotScope { ids, .. } => {
                        this.items.retain(|i| !ids.contains(&i.id));
                    }
                }
                let items = this.items.clone();
                panel.update(cx, |panel, cx| panel.set_items(items, cx));
            });
            Self {
                panel,
                items: items(),
                removed: Vec::new(),
                _events: events,
            }
        })
    }
}

impl Story for MemoryManagerPanelStory {
    fn title() -> &'static str {
        "Memory manager panel"
    }

    fn description() -> &'static str {
        "What an assistant remembers, to search, edit, pause or remove, with undo."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for MemoryManagerPanelStory {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w_full().child(self.panel.clone())
    }
}
