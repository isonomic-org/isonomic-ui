// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Conversation list: every chat, newest first, in a narrow column.
//!
//! Pinned chats come first, then today's, yesterday's, the week before and
//! everything older. A field filters by title. Each row opens its chat and
//! carries a menu to pin, rename, archive or delete it; renaming happens in
//! the row itself. Archiving and deleting hide the row at once and leave an
//! undo for a few seconds: the owner hears of the removal only once that
//! time is up. A chat whose reply is still being written shows a spinner.
//! Up, Down, Home and End move between rows.

use std::{collections::HashMap, time::Duration};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::TestSupportExt as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle},
    h_flex,
    input::{Input, InputEvent, InputState},
    look,
    menu::{DropdownMenu as _, PopupMenu, PopupMenuItem},
    spinner::Spinner,
    v_flex,
};
use isonomic_ui::{
    Animation, AnimationExt as _, AnyElement, AppContext as _, Context, ElementId, Entity,
    EventEmitter, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, Render, Role, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Task, WeakEntity, Window, div, prelude::FluentBuilder as _, px,
};

use crate::common::{panel, shell, shell_header};

/// How long a row takes to open, and the undo line to appear.
const ENTER: Duration = Duration::from_millis(200);
/// How long a removed row takes to close.
const EXIT: Duration = Duration::from_millis(140);
/// A row and the hairline of space under it.
const ROW: f32 = 37.;
const DAY: i64 = 86_400;

/// One chat.
#[derive(Debug, Clone, PartialEq)]
pub struct Conversation {
    /// Identifies it.
    pub id: SharedString,
    /// Its title.
    pub title: SharedString,
    /// When it last changed, in seconds since the epoch (UTC).
    pub updated_at: i64,
    /// Kept at the top.
    pub pinned: bool,
    /// A reply is still being written in it.
    pub running: bool,
}

impl Conversation {
    /// A chat last changed at `updated_at`.
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        updated_at: i64,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            updated_at,
            pinned: false,
            running: false,
        }
    }

    /// Keep it at the top.
    pub fn pinned(mut self, pinned: bool) -> Self {
        self.pinned = pinned;
        self
    }

    /// A reply is still being written in it.
    pub fn running(mut self, running: bool) -> Self {
        self.running = running;
        self
    }
}

/// How much the list shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ListVariant {
    /// Search, group headings and rows.
    #[default]
    Sidebar,
    /// Titles only, in a narrower column.
    Compact,
}

/// The heading a chat falls under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DateGroup {
    /// Kept at the top, whatever its date.
    Pinned,
    /// Changed today.
    Today,
    /// Changed yesterday.
    Yesterday,
    /// Changed earlier in the seven days before today.
    Week,
    /// Changed before that.
    Older,
}

impl DateGroup {
    /// The heading's words.
    pub fn label(self) -> &'static str {
        match self {
            DateGroup::Pinned => "Pinned",
            DateGroup::Today => "Today",
            DateGroup::Yesterday => "Yesterday",
            DateGroup::Week => "Previous 7 days",
            DateGroup::Older => "Older",
        }
    }
}

/// The heading for a chat, by the reader's own midnight.
pub fn group_of(chat: &Conversation, now: i64, utc_offset_minutes: i32) -> DateGroup {
    if chat.pinned {
        return DateGroup::Pinned;
    }
    let shift = i64::from(utc_offset_minutes) * 60;
    let midnight = (now + shift).div_euclid(DAY) * DAY - shift;
    if chat.updated_at >= midnight {
        DateGroup::Today
    } else if chat.updated_at >= midnight - DAY {
        DateGroup::Yesterday
    } else if chat.updated_at >= midnight - 7 * DAY {
        DateGroup::Week
    } else {
        DateGroup::Older
    }
}

/// Whether a title contains what was typed, ignoring case and outer space.
pub fn title_matches(title: &str, typed: &str) -> bool {
    let typed = typed.trim().to_lowercase();
    typed.is_empty() || title.to_lowercase().contains(&typed)
}

/// The chats that match what was typed, under their headings in order.
/// Within a heading the given order is kept.
pub fn grouped(
    chats: &[Conversation],
    typed: &str,
    now: i64,
    utc_offset_minutes: i32,
) -> Vec<(DateGroup, Vec<Conversation>)> {
    let mut groups: Vec<(DateGroup, Vec<Conversation>)> = Vec::new();
    for chat in chats
        .iter()
        .filter(|chat| title_matches(&chat.title, typed))
    {
        let group = group_of(chat, now, utc_offset_minutes);
        match groups.iter_mut().find(|(key, _)| *key == group) {
            Some((_, list)) => list.push(chat.clone()),
            None => groups.push((group, vec![chat.clone()])),
        }
    }
    groups.sort_by_key(|(group, _)| *group);
    groups
}

/// A way of removing a chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Removal {
    /// Put away, to be found again.
    Archive,
    /// Gone for good.
    Delete,
}

/// What the undo line says: "Archived “Title”".
pub fn removal_text(removal: Removal, title: &str) -> String {
    let word = match removal {
        Removal::Archive => "Archived",
        Removal::Delete => "Deleted",
    };
    format!("{word} \u{201c}{title}\u{201d}")
}

/// The row to move to from `current` among `count` rows, for a key.
pub fn step_row(current: Option<usize>, count: usize, key: &str) -> Option<usize> {
    if count == 0 {
        return None;
    }
    match (key, current) {
        ("home", _) => Some(0),
        ("end", _) => Some(count - 1),
        ("down", None) => Some(0),
        ("down", Some(at)) => (at + 1 < count).then_some(at + 1),
        ("up", Some(at)) => at.checked_sub(1),
        _ => None,
    }
}

/// What the list reports.
#[derive(Debug, Clone, PartialEq)]
pub enum ConversationListEvent {
    /// A chat was opened.
    Select(SharedString),
    /// Asked for a new chat.
    New,
    /// A chat was given a new title.
    Rename {
        /// The chat.
        id: SharedString,
        /// Its new title.
        title: SharedString,
    },
    /// A chat was pinned or unpinned.
    Pin {
        /// The chat.
        id: SharedString,
        /// Whether it is pinned now.
        pinned: bool,
    },
    /// A chat was archived and the time to undo is up.
    Archive(SharedString),
    /// A chat was deleted and the time to undo is up.
    Delete(SharedString),
}

/// Every chat in a column.
pub struct ConversationList {
    id: ElementId,
    chats: Vec<Conversation>,
    active: Option<SharedString>,
    variant: ListVariant,
    now: i64,
    utc_offset_minutes: i32,
    undo_window: Duration,
    new_button: bool,
    search: Entity<InputState>,
    rename: Entity<InputState>,
    renaming: Option<SharedString>,
    removed: Option<(Conversation, Removal)>,
    /// Counts removals and returns, so each row movement plays once.
    moves: usize,
    /// Rows that open with movement: returned by undo, or new to the list.
    fresh: Vec<SharedString>,
    row_focus: HashMap<SharedString, FocusHandle>,
    _undo: Option<Task<()>>,
    _events: Vec<Subscription>,
}

impl EventEmitter<ConversationListEvent> for ConversationList {}

impl ConversationList {
    /// Create the list. `now` is the present moment in seconds since the
    /// epoch (UTC); see [`Self::utc_offset`] for the reader's own clock.
    pub fn new(
        id: impl Into<ElementId>,
        chats: Vec<Conversation>,
        now: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search chats"));
        let rename = cx.new(|cx| InputState::new(window, cx));
        let events = vec![
            cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
            cx.subscribe(&rename, |this, _, event: &InputEvent, cx| match event {
                InputEvent::PressEnter { .. } | InputEvent::Blur => this.finish_rename(true, cx),
                _ => {}
            }),
        ];
        Self {
            id: id.into(),
            chats,
            active: None,
            variant: ListVariant::Sidebar,
            now,
            utc_offset_minutes: 0,
            undo_window: Duration::from_secs(5),
            new_button: true,
            search,
            rename,
            renaming: None,
            removed: None,
            moves: 0,
            fresh: Vec::new(),
            row_focus: HashMap::new(),
            _undo: None,
            _events: events,
        }
    }

    /// The chat that is open.
    pub fn active(mut self, id: impl Into<SharedString>) -> Self {
        self.active = Some(id.into());
        self
    }

    /// How much the list shows.
    pub fn variant(mut self, variant: ListVariant) -> Self {
        self.variant = variant;
        self
    }

    /// How far the reader's clock is from UTC, in minutes.
    pub fn utc_offset(mut self, minutes: i32) -> Self {
        self.utc_offset_minutes = minutes;
        self
    }

    /// How long a removal can be undone. Five seconds by default.
    pub fn undo_window(mut self, window: Duration) -> Self {
        self.undo_window = window;
        self
    }

    /// Whether the header offers a new chat. On by default.
    pub fn new_button(mut self, show: bool) -> Self {
        self.new_button = show;
        self
    }

    /// Replace the chats. Ones that were not there before open with movement.
    pub fn set_conversations(&mut self, chats: Vec<Conversation>, cx: &mut Context<Self>) {
        self.fresh = chats
            .iter()
            .filter(|chat| !self.chats.iter().any(|old| old.id == chat.id))
            .map(|chat| chat.id.clone())
            .collect();
        self.moves += 1;
        self.row_focus
            .retain(|id, _| chats.iter().any(|c| &c.id == id));
        self.chats = chats;
        cx.notify();
    }

    /// Mark the chat that is open.
    pub fn set_active(&mut self, id: Option<SharedString>, cx: &mut Context<Self>) {
        self.active = id;
        cx.notify();
    }

    /// Move the present moment, so the headings follow the clock.
    pub fn set_now(&mut self, now: i64, cx: &mut Context<Self>) {
        self.now = now;
        cx.notify();
    }

    /// The chat waiting out its undo time, and how it was removed.
    pub fn pending_removal(&self) -> Option<(&Conversation, Removal)> {
        self.removed
            .as_ref()
            .map(|(chat, removal)| (chat, *removal))
    }

    /// Open the title of a chat for editing in its row.
    pub fn start_rename(&mut self, id: &SharedString, window: &mut Window, cx: &mut Context<Self>) {
        let Some(chat) = self.chats.iter().find(|chat| &chat.id == id) else {
            return;
        };
        let title = chat.title.clone();
        self.renaming = Some(id.clone());
        self.rename.update(cx, |field, cx| {
            field.set_value(title, window, cx);
            field.focus(window, cx);
            field.select_all(window, cx);
        });
        cx.notify();
    }

    fn finish_rename(&mut self, keep: bool, cx: &mut Context<Self>) {
        let Some(id) = self.renaming.take() else {
            return;
        };
        let title = self.rename.read(cx).value().trim().to_string();
        let old = self.chats.iter().find(|chat| chat.id == id);
        if keep && !title.is_empty() && old.is_some_and(|chat| chat.title != title) {
            cx.emit(ConversationListEvent::Rename {
                id,
                title: title.into(),
            });
        }
        cx.notify();
    }

    /// Hide a chat and start its undo time. A removal already waiting is
    /// reported at once.
    pub fn remove(&mut self, id: &SharedString, removal: Removal, cx: &mut Context<Self>) {
        let Some(chat) = self.chats.iter().find(|chat| &chat.id == id).cloned() else {
            return;
        };
        self.commit_removal(cx);
        self.removed = Some((chat, removal));
        self.moves += 1;
        self.fresh.clear();
        let wait = self.undo_window;
        self._undo = Some(cx.spawn(async move |this: WeakEntity<Self>, cx| {
            cx.background_executor().timer(wait).await;
            this.update(cx, |this, cx| this.commit_removal(cx)).ok();
        }));
        cx.notify();
    }

    /// Bring back the chat that is waiting out its undo time.
    pub fn undo(&mut self, cx: &mut Context<Self>) {
        if let Some((chat, _)) = self.removed.take() {
            self._undo = None;
            self.moves += 1;
            self.fresh = vec![chat.id];
            cx.notify();
        }
    }

    fn commit_removal(&mut self, cx: &mut Context<Self>) {
        if let Some((chat, removal)) = self.removed.take() {
            cx.emit(match removal {
                Removal::Archive => ConversationListEvent::Archive(chat.id),
                Removal::Delete => ConversationListEvent::Delete(chat.id),
            });
            cx.notify();
        }
    }

    fn part(&self, name: &'static str) -> ElementId {
        ElementId::NamedChild(self.id.clone().into(), name.into())
    }

    fn row_id(&self, chat: &SharedString) -> ElementId {
        ElementId::NamedChild(self.id.clone().into(), chat.clone())
    }

    fn row_menu(&self, chat: &Conversation, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let this = cx.entity().downgrade();
        let (id, pinned) = (chat.id.clone(), chat.pinned);
        Button::new(ElementId::NamedChild(
            self.row_id(&chat.id).into(),
            "more".into(),
        ))
        .ghost()
        .xsmall()
        .icon(Icon::new(CatalogIcon::OverflowMenuHorizontal))
        .accessibility_label(format!("Actions for {}", chat.title))
        .dropdown_menu(move |menu: PopupMenu, _, _| {
            let item = |label: &'static str, icon: CatalogIcon| {
                PopupMenuItem::new(label).icon(Icon::new(icon))
            };
            let (pin, rename, archive, delete) =
                (this.clone(), this.clone(), this.clone(), this.clone());
            let (pin_id, rename_id, archive_id, delete_id) =
                (id.clone(), id.clone(), id.clone(), id.clone());
            menu.item(
                if pinned {
                    item("Unpin", CatalogIcon::PinFilled)
                } else {
                    item("Pin", CatalogIcon::Pin)
                }
                .on_click(move |_, _, cx| {
                    pin.update(cx, |_, cx| {
                        cx.emit(ConversationListEvent::Pin {
                            id: pin_id.clone(),
                            pinned: !pinned,
                        })
                    })
                    .ok();
                }),
            )
            .item(
                item("Rename", CatalogIcon::Edit).on_click(move |_, window, cx| {
                    rename
                        .update(cx, |this, cx| this.start_rename(&rename_id, window, cx))
                        .ok();
                }),
            )
            .item(
                item("Archive", CatalogIcon::Archive).on_click(move |_, _, cx| {
                    archive
                        .update(cx, |this, cx| {
                            this.remove(&archive_id, Removal::Archive, cx)
                        })
                        .ok();
                }),
            )
            .separator()
            .item(
                item("Delete", CatalogIcon::TrashCan)
                    .destructive()
                    .on_click(move |_, _, cx| {
                        delete
                            .update(cx, |this, cx| this.remove(&delete_id, Removal::Delete, cx))
                            .ok();
                    }),
            )
        })
    }

    fn row(&mut self, chat: &Conversation, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let raised = theme.muted;
        let ink = theme.foreground;
        let accent = theme.info;
        let wash = look::ink::wash(cx);
        let wash_strong = look::ink::wash_strong(cx);
        let still = cx.reduce_motion();
        let row_id = self.row_id(&chat.id);
        let leaving = self
            .removed
            .as_ref()
            .is_some_and(|(removed, _)| removed.id == chat.id);
        let entering = self.fresh.contains(&chat.id);
        let move_id =
            ElementId::NamedChild(row_id.clone().into(), format!("move-{}", self.moves).into());

        let content = if self.renaming.as_ref() == Some(&chat.id) {
            div()
                .h(px(36.))
                .rounded(px(16.))
                .bg(wash_strong)
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    if event.keystroke.key == "escape" {
                        this.finish_rename(false, cx);
                        cx.stop_propagation();
                    }
                }))
                .child(
                    Input::new(&self.rename)
                        .appearance(false)
                        .h(px(36.))
                        .aria_label("Chat title"),
                )
                .into_any_element()
        } else {
            let active = self.active.as_ref() == Some(&chat.id);
            let focus = self
                .row_focus
                .entry(chat.id.clone())
                .or_insert_with(|| cx.focus_handle().tab_stop(true))
                .clone();
            let hover = SharedString::from(format!("chat-row-{}", chat.id));
            let id = chat.id.clone();
            h_flex()
                .group(hover.clone())
                .relative()
                .child(
                    h_flex()
                        .id(row_id)
                        .test_support()
                        .track_focus(&focus)
                        .w_full()
                        .h(px(36.))
                        .gap_2()
                        .pl(px(10.))
                        .pr(px(36.))
                        .rounded(px(16.))
                        .text_size(px(13.))
                        .line_height(px(20.))
                        .cursor_pointer()
                        .map(|this| {
                            if active {
                                this.bg(raised).text_color(ink).font_medium()
                            } else {
                                this.text_color(ink.opacity(0.8))
                                    .hover(move |this| this.bg(wash))
                            }
                        })
                        .focus_visible(|this| this.bg(wash_strong))
                        .role(Role::ListItem)
                        .aria_label(format!(
                            "{}{}{}",
                            chat.title,
                            if active { ", open" } else { "" },
                            if chat.running {
                                ", a reply is being written"
                            } else {
                                ""
                            },
                        ))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.emit(ConversationListEvent::Select(id.clone()))
                        }))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(chat.title.clone()),
                        )
                        .when(chat.running, |this| {
                            this.child(Spinner::new().xsmall().color(accent))
                        }),
                )
                .child(
                    h_flex()
                        .absolute()
                        .right(px(4.))
                        .top_0()
                        .bottom_0()
                        .when(!active, |this| {
                            this.opacity(0.).group_hover(hover, |this| this.opacity(1.))
                        })
                        .child(self.row_menu(chat, cx)),
                )
                .into_any_element()
        };

        let slot = div().h(px(ROW)).overflow_hidden().child(content);
        if leaving {
            if still {
                return div().into_any_element();
            }
            slot.with_animation(move_id, Animation::new(EXIT), |this, time| {
                let gone = 1. - (1. - time).powi(3);
                this.h(px(ROW * (1. - gone))).opacity(1. - gone)
            })
            .into_any_element()
        } else if entering && !still {
            slot.with_animation(move_id, Animation::new(ENTER), |this, time| {
                let shown = 1. - (1. - time).powi(3);
                this.h(px(ROW * shown)).opacity(shown)
            })
            .into_any_element()
        } else {
            slot.into_any_element()
        }
    }
}

impl Render for ConversationList {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let still = cx.reduce_motion();
        let compact = self.variant == ListVariant::Compact;
        let typed = if compact {
            SharedString::default()
        } else {
            self.search.read(cx).value()
        };
        let hidden = self.removed.as_ref().map(|(chat, _)| chat.id.clone());
        let groups: Vec<(Option<DateGroup>, Vec<Conversation>)> = if compact {
            let all: Vec<Conversation> = self.chats.clone();
            if all.is_empty() {
                Vec::new()
            } else {
                vec![(None, all)]
            }
        } else {
            grouped(&self.chats, &typed, self.now, self.utc_offset_minutes)
                .into_iter()
                .map(|(group, list)| (Some(group), list))
                .collect()
        };
        // A heading stays only while it has a row that is not on its way out.
        let groups: Vec<_> = groups
            .into_iter()
            .filter(|(_, list)| list.iter().any(|chat| Some(&chat.id) != hidden.as_ref()))
            .collect();
        let order: Vec<SharedString> = groups
            .iter()
            .flat_map(|(_, list)| list.iter())
            .filter(|chat| Some(&chat.id) != hidden.as_ref())
            .map(|chat| chat.id.clone())
            .collect();

        let sections: Vec<AnyElement> = groups
            .iter()
            .map(|(group, list)| {
                let name = group.map(DateGroup::label);
                v_flex()
                    .when_some(name, |this, name| {
                        this.child(
                            div()
                                .px(px(10.))
                                .py_1()
                                .text_size(px(12.))
                                .line_height(px(16.))
                                .font_medium()
                                .text_color(quiet)
                                .child(name),
                        )
                    })
                    .children(list.iter().map(|chat| self.row(chat, cx)))
                    .into_any_element()
            })
            .collect();

        let empty = sections.is_empty().then(|| {
            let searching = !typed.trim().is_empty();
            let icon = if searching {
                CatalogIcon::Search
            } else {
                CatalogIcon::Chat
            };
            Empty::new().py_6().header(
                EmptyHeader::new()
                    .media(EmptyMedia::new().child(Icon::new(icon)))
                    .title(EmptyTitle::new().child(if searching {
                        "No chats found"
                    } else {
                        "No chats yet"
                    }))
                    .when(searching, |this| {
                        this.description(EmptyDescription::new().child("Try a different search."))
                    }),
            )
        });

        let undo = self.removed.as_ref().map(|(chat, removal)| {
            let line = h_flex()
                .id(self.part("undo-line"))
                .relative()
                .gap_2()
                .px_3()
                .pt(px(10.))
                .pb(px(6.))
                .text_size(px(13.))
                .line_height(px(20.))
                .role(Role::Status)
                .aria_label(format!(
                    "{}. Undo available.",
                    removal_text(*removal, &chat.title)
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(quiet)
                        .child(removal_text(*removal, &chat.title)),
                )
                .child(
                    Button::new(self.part("undo"))
                        .secondary()
                        .small()
                        .icon(Icon::new(CatalogIcon::Undo))
                        .label("Undo")
                        .h(px(28.))
                        .px(px(10.))
                        .rounded_full()
                        .on_click(cx.listener(|this, _, _, cx| this.undo(cx))),
                );
            if still {
                line.into_any_element()
            } else {
                line.with_animation(
                    ElementId::NamedChild(
                        self.id.clone().into(),
                        format!("undo-in-{}", self.moves).into(),
                    ),
                    Animation::new(ENTER),
                    |this, time| {
                        let shown = 1. - (1. - time).powi(3);
                        this.opacity(shown).top(px(6. * (1. - shown)))
                    },
                )
                .into_any_element()
            }
        });

        let new_chat = self.new_button.then(|| {
            Button::new(self.part("new"))
                .secondary()
                .icon(Icon::new(CatalogIcon::AddComment))
                .tooltip("New chat")
                .accessibility_label("New chat")
                .on_click(cx.listener(|_, _, _, cx| cx.emit(ConversationListEvent::New)))
                .into_any_element()
        });
        let focused = order.iter().position(|id| {
            self.row_focus
                .get(id)
                .is_some_and(|handle| handle.is_focused(window))
        });
        let count = order.len();

        shell(cx)
            .id(self.id.clone())
            .max_w(if compact { px(240.) } else { px(288.) })
            .role(Role::Navigation)
            .aria_label("Chats")
            .child(shell_header(
                Some(Icon::new(CatalogIcon::Chat)),
                "Chats",
                None,
                new_chat,
                cx,
            ))
            .when(!compact, |this| {
                this.child(
                    div().px(px(6.)).pb(px(10.)).child(
                        Input::new(&self.search)
                            .h(px(36.))
                            .rounded(px(18.))
                            .prefix(
                                Icon::new(CatalogIcon::Search)
                                    .size(px(14.))
                                    .text_color(quiet),
                            )
                            .cleanable(true)
                            .aria_label("Search chats"),
                    ),
                )
            })
            .child(
                panel(cx)
                    .id(self.part("rows"))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p(px(6.))
                    .role(Role::List)
                    .aria_label("Chats")
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                        if this.renaming.is_some() {
                            return;
                        }
                        let Some(next) = step_row(focused, count, event.keystroke.key.as_str())
                        else {
                            return;
                        };
                        if let Some(handle) = order.get(next).and_then(|id| this.row_focus.get(id))
                        {
                            handle.focus(window, cx);
                            cx.stop_propagation();
                            cx.notify();
                        }
                    }))
                    .children(empty)
                    .children(sections),
            )
            .children(undo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 12 March 2026, 09:00 UTC.
    const NOW: i64 = 1_773_306_000;
    const HOUR: i64 = 3600;

    fn chat(id: &str, title: &str, ago: i64) -> Conversation {
        Conversation::new(id.to_string(), title.to_string(), NOW - ago)
    }

    #[test]
    fn chats_fall_under_headings_by_the_readers_midnight() {
        let group = |ago: i64, offset: i32| group_of(&chat("a", "A", ago), NOW, offset);
        assert_eq!(group(HOUR, 0), DateGroup::Today);
        assert_eq!(group(9 * HOUR, 0), DateGroup::Today);
        assert_eq!(group(9 * HOUR + 1, 0), DateGroup::Yesterday);
        assert_eq!(group(33 * HOUR + 1, 0), DateGroup::Week);
        assert_eq!(group(9 * HOUR + 7 * DAY, 0), DateGroup::Week);
        assert_eq!(group(9 * HOUR + 7 * DAY + 1, 0), DateGroup::Older);
        // Five hours to the west it is four in the morning: midnight was four hours ago.
        assert_eq!(group(5 * HOUR, -300), DateGroup::Yesterday);
        assert_eq!(group(3 * HOUR, -300), DateGroup::Today);
        let pinned = chat("p", "P", 40 * DAY).pinned(true);
        assert_eq!(group_of(&pinned, NOW, 0), DateGroup::Pinned);
    }

    #[test]
    fn headings_come_in_order_and_keep_the_order_inside() {
        let chats = [
            chat("old", "Kiln schedule", 20 * DAY),
            chat("today-1", "Glaze test tiles", HOUR),
            chat("pin", "Opening hours", 3 * DAY).pinned(true),
            chat("today-2", "Mug handles", 2 * HOUR),
            chat("yesterday", "Clay order", 20 * HOUR),
        ];
        let groups = grouped(&chats, "", NOW, 0);
        let shape: Vec<(DateGroup, Vec<&str>)> = groups
            .iter()
            .map(|(group, list)| (*group, list.iter().map(|c| c.id.as_ref()).collect()))
            .collect();
        assert_eq!(
            shape,
            [
                (DateGroup::Pinned, vec!["pin"]),
                (DateGroup::Today, vec!["today-1", "today-2"]),
                (DateGroup::Yesterday, vec!["yesterday"]),
                (DateGroup::Older, vec!["old"]),
            ]
        );
    }

    #[test]
    fn typing_filters_by_title_whatever_the_case() {
        assert!(title_matches("Glaze test tiles", "  GLAZE "));
        assert!(title_matches("Glaze test tiles", ""));
        assert!(!title_matches("Glaze test tiles", "kiln"));
        let chats = [
            chat("a", "Kiln schedule", HOUR),
            chat("b", "Clay order", HOUR),
        ];
        let found = grouped(&chats, "clay", NOW, 0);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].1[0].id.as_ref(), "b");
        assert!(grouped(&chats, "wheel", NOW, 0).is_empty());
    }

    #[test]
    fn the_undo_line_names_what_happened() {
        assert_eq!(
            removal_text(Removal::Archive, "Clay order"),
            "Archived \u{201c}Clay order\u{201d}"
        );
        assert_eq!(
            removal_text(Removal::Delete, "Mug handles"),
            "Deleted \u{201c}Mug handles\u{201d}"
        );
    }

    #[test]
    fn arrow_keys_stop_at_the_ends() {
        assert_eq!(step_row(None, 3, "down"), Some(0));
        assert_eq!(step_row(Some(0), 3, "down"), Some(1));
        assert_eq!(step_row(Some(2), 3, "down"), None);
        assert_eq!(step_row(Some(0), 3, "up"), None);
        assert_eq!(step_row(Some(1), 3, "end"), Some(2));
        assert_eq!(step_row(Some(2), 3, "home"), Some(0));
        assert_eq!(step_row(Some(1), 3, "enter"), None);
        assert_eq!(step_row(None, 0, "home"), None);
    }
}
