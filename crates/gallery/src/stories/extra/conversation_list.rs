// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the conversation list.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div, px,
};
use isonomic_ui_extra::conversation_list::{
    Conversation, ConversationList, ConversationListEvent, ListVariant,
};

/// Thursday 12 March 2026, 09:00 UTC: the made-up present of this story.
const NOW: i64 = 1_773_306_000;
const HOUR: i64 = 3600;
const DAY: i64 = 24 * HOUR;

fn studio_chats() -> Vec<Conversation> {
    vec![
        Conversation::new("hours", "Opening hours for the spring fair", NOW - 3 * DAY).pinned(true),
        Conversation::new("glaze", "Celadon glaze keeps crawling", NOW - HOUR).running(true),
        Conversation::new("handles", "Mug handles that do not crack", NOW - 2 * HOUR),
        Conversation::new("order", "Clay order for April", NOW - 20 * HOUR),
        Conversation::new("kiln", "Kiln schedule with the new shelves", NOW - 4 * DAY),
        Conversation::new("class", "Saturday class sign-up sheet", NOW - 6 * DAY),
        Conversation::new("labels", "Price labels for seconds", NOW - 30 * DAY),
    ]
}

/// A pottery studio's chats: the full column, a compact one, and an empty one.
pub struct ConversationListStory {
    chats: Vec<Conversation>,
    list: Entity<ConversationList>,
    compact: Entity<ConversationList>,
    empty: Entity<ConversationList>,
    last: SharedString,
    made: usize,
    _events: Subscription,
}

impl ConversationListStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let chats = studio_chats();
            let list = cx.new(|cx| {
                ConversationList::new("chats", chats.clone(), NOW, window, cx)
                    .active("handles")
                    .utc_offset(60)
            });
            let compact = cx.new(|cx| {
                ConversationList::new("compact", chats[..4].to_vec(), NOW, window, cx)
                    .variant(ListVariant::Compact)
                    .active("order")
                    .new_button(false)
            });
            let empty = cx.new(|cx| ConversationList::new("none", Vec::new(), NOW, window, cx));
            let events = cx.subscribe(&list, Self::on_event);
            Self {
                chats,
                list,
                compact,
                empty,
                last: "Nothing happened yet".into(),
                made: 0,
                _events: events,
            }
        })
    }

    fn on_event(
        &mut self,
        list: Entity<ConversationList>,
        event: &ConversationListEvent,
        cx: &mut Context<Self>,
    ) {
        let mut active = None;
        self.last = match event {
            ConversationListEvent::Select(id) => {
                active = Some(id.clone());
                format!("Opened {id}")
            }
            ConversationListEvent::New => {
                self.made += 1;
                let id = SharedString::from(format!("new-{}", self.made));
                self.chats
                    .insert(0, Conversation::new(id.clone(), "Untitled chat", NOW));
                active = Some(id);
                "Started a chat".to_string()
            }
            ConversationListEvent::Rename { id, title } => {
                if let Some(chat) = self.chats.iter_mut().find(|chat| &chat.id == id) {
                    chat.title = title.clone();
                }
                format!("Renamed to {title}")
            }
            ConversationListEvent::Pin { id, pinned } => {
                if let Some(chat) = self.chats.iter_mut().find(|chat| &chat.id == id) {
                    chat.pinned = *pinned;
                }
                format!("{} {id}", if *pinned { "Pinned" } else { "Unpinned" })
            }
            ConversationListEvent::Archive(id) => {
                self.chats.retain(|chat| &chat.id != id);
                format!("Archived {id} for good")
            }
            ConversationListEvent::Delete(id) => {
                self.chats.retain(|chat| &chat.id != id);
                format!("Deleted {id} for good")
            }
        }
        .into();
        let chats = self.chats.clone();
        list.update(cx, |list, cx| {
            list.set_conversations(chats, cx);
            if active.is_some() {
                list.set_active(active, cx);
            }
        });
        cx.notify();
    }
}

impl Story for ConversationListStory {
    fn title() -> &'static str {
        "Conversation list"
    }

    fn description() -> &'static str {
        "Chats in a column: search, pinned and dated groups, rename in the row, and archive or delete with an undo."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ConversationListStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_4()
            .child(
                h_flex()
                    .items_start()
                    .gap_6()
                    .child(div().w(px(288.)).child(self.list.clone()))
                    .child(div().w(px(240.)).child(self.compact.clone()))
                    .child(div().w(px(288.)).child(self.empty.clone())),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.last.clone()),
            )
    }
}
