// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the command palette.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, IconName, Size, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div, px,
};
use isonomic_ui_extra::command_palette::{CommandPalette, PaletteCommand, PaletteEvent};

/// A bookkeeping app's commands. The story moves what's run to the front of
/// the recent list and says what ran.
pub struct CommandPaletteStory {
    palette: Entity<CommandPalette>,
    /// A palette with every option set: a chat search.
    search: Entity<CommandPalette>,
    recent: Vec<SharedString>,
    note: String,
    _events: Subscription,
}

fn commands() -> Vec<PaletteCommand> {
    vec![
        PaletteCommand::new("new-invoice", "Invoices", "New invoice").keys("cmd-i"),
        PaletteCommand::new("remind", "Invoices", "Send payment reminders"),
        PaletteCommand::new("void", "Invoices", "Void an invoice"),
        PaletteCommand::new("reconcile", "Bank", "Reconcile an account").keys("cmd-shift-r"),
        PaletteCommand::new("import", "Bank", "Import a statement"),
        PaletteCommand::new("post", "Ledger", "Post matched lines").unavailable("March is locked"),
        PaletteCommand::new("unlock", "Ledger", "Unlock a period"),
        PaletteCommand::new("journal", "Ledger", "New journal entry").keys("cmd-j"),
        PaletteCommand::new("pnl", "Reports", "Open profit and loss"),
        PaletteCommand::new("close", "Reports", "Draft the close note"),
    ]
}

impl CommandPaletteStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let palette = cx.new(|cx| {
                let mut palette = CommandPalette::new(commands(), window, cx);
                palette.set_recent(vec!["reconcile".into(), "remind".into()], cx);
                palette
            });
            let events = cx.subscribe(&palette, |this: &mut Self, palette, event, cx| {
                match event {
                    PaletteEvent::Run(id) => {
                        this.recent.retain(|r| r != id);
                        this.recent.insert(0, id.clone());
                        this.recent.truncate(3);
                        this.note = format!("Ran: {id}");
                        let recent = this.recent.clone();
                        palette.update(cx, |p, cx| p.set_recent(recent, cx));
                    }
                    PaletteEvent::Dismiss => this.note = "Asked to close".into(),
                }
                cx.notify();
            });
            let search = cx.new(|cx| {
                let mut search = CommandPalette::new(
                    vec![
                        PaletteCommand::new("new", "Actions", "New chat")
                            .icon(IconName::Plus)
                            .keys("cmd-n"),
                        // Any icon in the catalog, not only the component set.
                        PaletteCommand::new("c1", "Today", "March close checklist")
                            .icon(isonomic_ui::assets::IconName::Chat)
                            .meta("2 h ago"),
                        PaletteCommand::new("c2", "Today", "Chase the overdue invoices")
                            .icon(IconName::Inbox)
                            .meta("5 h ago"),
                        PaletteCommand::new("c3", "Earlier", "Reconcile the card spend")
                            .icon(IconName::Inbox)
                            .meta("12 Mar"),
                    ],
                    window,
                    cx,
                );
                search.set_width(px(640.), cx);
                search.set_placeholder("Search chats", window, cx);
                search.set_input_size(Size::Large, cx);
                search.set_escape_hint(true, cx);
                search.set_row_height(px(36.), cx);
                search.set_keep_groups(true, cx);
                search.set_footer_hints(["Enter to open", "Esc to close"], cx);
                search.set_select_first(false, cx);
                search.set_empty_text("No chats match.", cx);
                search.set_hold_focus(true, cx);
                search
            });
            Self {
                palette,
                search,
                recent: vec!["reconcile".into(), "remind".into()],
                note: String::new(),
                _events: events,
            }
        })
    }
}

impl Story for CommandPaletteStory {
    fn title() -> &'static str {
        "Command palette"
    }

    fn description() -> &'static str {
        "Every action in one searchable list, grouped, with recent ones first and each one's shortcut."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for CommandPaletteStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(self.palette.clone())
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(if self.note.is_empty() {
                        "Type \u{201c}rec\u{201d}, then Enter.".to_string()
                    } else {
                        self.note.clone()
                    }),
            )
            .child(
                div()
                    .pt_4()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        "Every option set: wider, a larger field with an Esc key, icons and \
                     notes on 36 px rows, groups kept while typing, its own hints, no row \
                     selected until a key is pressed, and Tab held inside",
                    ),
            )
            .child(self.search.clone())
    }
}
