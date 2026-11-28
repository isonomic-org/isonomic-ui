// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Memory manager panel: what an assistant remembers, to search, edit, pause or remove, with undo.
//!
//! Each remembered item shows what it says, where it came from and when it
//! last changed. Search narrows the list as you type. Edit opens the item in
//! place with Save and Cancel; pause keeps an item but stops it being used;
//! remove takes it out, with an undo line until the next change. The panel
//! reports each of these and shows whatever items its owner passes back.
//!
//! Items can carry a scope, such as one person, a workspace or a project.
//! When any do, tabs above the list filter by scope with a count each, every
//! row names its scope, editing offers to move an item to another scope,
//! and a filtered scope can be forgotten as a whole after a confirm step.
//! While editing, Escape cancels and the secondary key with Enter saves.

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    tab::{Tab, TabBar},
    tag::Tag,
    v_flex,
};
use isonomic_ui::{
    AppContext as _, Context, ElementId, Entity, EventEmitter, Focusable as _,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render, Role,
    SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::{Tone, resting_shadow};

/// One remembered item.
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryItem {
    /// Identifies it.
    pub id: SharedString,
    /// What it says.
    pub text: SharedString,
    /// Where it came from, such as a conversation's name.
    pub source: SharedString,
    /// When it last changed, as written.
    pub updated: SharedString,
    /// Whether it's kept but not used.
    pub paused: bool,
    /// Who or what it applies to, such as a person, a workspace or a
    /// project; empty for none.
    pub scope: SharedString,
}

impl MemoryItem {
    /// Create an item in use.
    pub fn new(
        id: impl Into<SharedString>,
        text: impl Into<SharedString>,
        source: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            source: source.into(),
            updated: SharedString::default(),
            paused: false,
            scope: SharedString::default(),
        }
    }

    /// Say when it last changed.
    pub fn updated(mut self, when: impl Into<SharedString>) -> Self {
        self.updated = when.into();
        self
    }

    /// Pause it.
    pub fn paused(mut self, paused: bool) -> Self {
        self.paused = paused;
        self
    }

    /// Say who or what it applies to.
    pub fn scope(mut self, scope: impl Into<SharedString>) -> Self {
        self.scope = scope.into();
        self
    }
}

/// What the panel reports.
#[derive(Debug, Clone, PartialEq)]
pub enum MemoryEvent {
    /// An item's text was changed.
    Edited {
        /// Which item.
        id: SharedString,
        /// Its new text.
        text: SharedString,
    },
    /// An item was paused (`true`) or resumed.
    Paused {
        /// Which item.
        id: SharedString,
        /// Whether it's paused now.
        paused: bool,
    },
    /// An item was removed.
    Removed(SharedString),
    /// The last removal was undone.
    Undo(SharedString),
    /// An item was moved to another scope while being edited.
    Rescoped {
        /// Which item.
        id: SharedString,
        /// Its new scope.
        scope: SharedString,
    },
    /// Everything in a scope was forgotten at once, after a confirm step.
    ForgotScope {
        /// Which scope.
        scope: SharedString,
        /// The items it held.
        ids: Vec<SharedString>,
    },
}

/// The scopes the items use, in first-seen order, with how many items each
/// holds. Items without a scope are not counted.
pub fn scopes_of(items: &[MemoryItem]) -> Vec<(SharedString, usize)> {
    let mut scopes: Vec<(SharedString, usize)> = Vec::new();
    for item in items.iter().filter(|i| !i.scope.is_empty()) {
        match scopes.iter_mut().find(|(scope, _)| *scope == item.scope) {
            Some((_, count)) => *count += 1,
            None => scopes.push((item.scope.clone(), 1)),
        }
    }
    scopes
}

/// What a key does while an item is being edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditKey {
    /// Keep the new text.
    Save,
    /// Throw it away.
    Cancel,
}

/// Map a key pressed while editing: Escape cancels, Enter with the
/// secondary modifier saves; anything else is typing.
pub fn edit_key(key: &str, secondary: bool) -> Option<EditKey> {
    match (key, secondary) {
        ("escape", _) => Some(EditKey::Cancel),
        ("enter", true) => Some(EditKey::Save),
        _ => None,
    }
}

/// The spoken name of a row: its text, scope and state.
pub fn memory_words(item: &MemoryItem) -> String {
    let mut words = item.text.to_string();
    if !item.scope.is_empty() {
        words.push_str(&format!(", for {}", item.scope));
    }
    if item.paused {
        words.push_str(", paused");
    }
    words
}

/// Whether an item matches a search, ignoring case, by its text or source.
pub fn matches_search(item: &MemoryItem, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty()
        || item.text.to_lowercase().contains(&query)
        || item.source.to_lowercase().contains(&query)
}

/// What an assistant remembers, to look after.
pub struct MemoryManagerPanel {
    items: Vec<MemoryItem>,
    search: Entity<InputState>,
    editing: Option<(SharedString, Entity<TextareaState>)>,
    removed: Option<MemoryItem>,
    scope: Option<SharedString>,
    edit_scope: Option<SharedString>,
    confirm_forget: bool,
    _search_events: Subscription,
}

impl EventEmitter<MemoryEvent> for MemoryManagerPanel {}

impl MemoryManagerPanel {
    /// Create a panel for these items.
    pub fn new(items: Vec<MemoryItem>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search what's remembered"));
        let search_events = cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        Self {
            items,
            search,
            editing: None,
            removed: None,
            scope: None,
            edit_scope: None,
            confirm_forget: false,
            _search_events: search_events,
        }
    }

    /// Show these items.
    pub fn set_items(&mut self, items: Vec<MemoryItem>, cx: &mut Context<Self>) {
        self.items = items;
        cx.notify();
    }

    fn start_edit(&mut self, id: &SharedString, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = self.items.iter().find(|i| &i.id == id) else {
            return;
        };
        let text = item.text.clone();
        let field = cx.new(|cx| {
            let mut field = TextareaState::new(window, cx).auto_grow(2, 6);
            field.set_value(text, window, cx);
            field
        });
        field.focus_handle(cx).focus(window, cx);
        self.edit_scope = Some(item.scope.clone()).filter(|scope| !scope.is_empty());
        self.editing = Some((id.clone(), field));
        cx.notify();
    }

    fn save_edit(&mut self, cx: &mut Context<Self>) {
        if let Some((id, field)) = self.editing.take() {
            let text = field.read(cx).value();
            let text = text.trim().to_string();
            let before = self.items.iter().find(|i| i.id == id);
            let scope = self.edit_scope.take();
            if let (Some(scope), Some(before)) = (scope, before)
                && scope != before.scope
            {
                cx.emit(MemoryEvent::Rescoped {
                    id: id.clone(),
                    scope,
                });
            }
            if !text.is_empty() {
                cx.emit(MemoryEvent::Edited {
                    id,
                    text: text.into(),
                });
            }
        }
        cx.notify();
    }

    fn cancel_edit(&mut self, cx: &mut Context<Self>) {
        self.editing = None;
        self.edit_scope = None;
        cx.notify();
    }

    /// Show only the items in this scope, or every item.
    pub fn set_scope(&mut self, scope: Option<SharedString>, cx: &mut Context<Self>) {
        self.scope = scope;
        self.confirm_forget = false;
        cx.notify();
    }

    fn forget_scope(&mut self, cx: &mut Context<Self>) {
        let Some(scope) = self.scope.clone() else {
            return;
        };
        if !self.confirm_forget {
            self.confirm_forget = true;
            cx.notify();
            return;
        }
        let ids = self
            .items
            .iter()
            .filter(|i| i.scope == scope)
            .map(|i| i.id.clone())
            .collect();
        self.confirm_forget = false;
        self.scope = None;
        cx.emit(MemoryEvent::ForgotScope { scope, ids });
        cx.notify();
    }

    fn remove(&mut self, id: &SharedString, cx: &mut Context<Self>) {
        self.removed = self.items.iter().find(|i| &i.id == id).cloned();
        cx.emit(MemoryEvent::Removed(id.clone()));
        cx.notify();
    }
}

impl Render for MemoryManagerPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let radius_lg = theme.radius_lg;
        let warning = Tone::Warning.color(cx);
        let query = self.search.read(cx).value().to_string();
        let scopes = scopes_of(&self.items);
        let scope = self
            .scope
            .clone()
            .filter(|scope| scopes.iter().any(|(s, _)| s == scope));
        let shown: Vec<MemoryItem> = self
            .items
            .iter()
            .filter(|i| matches_search(i, &query))
            .filter(|i| scope.as_ref().is_none_or(|scope| &i.scope == scope))
            .cloned()
            .collect();
        let paused = self.items.iter().filter(|i| i.paused).count();
        let scope_tabs = (!scopes.is_empty()).then(|| {
            let at = scope
                .as_ref()
                .and_then(|scope| scopes.iter().position(|(s, _)| s == scope))
                .map_or(0, |ix| ix + 1);
            let names: Vec<SharedString> = scopes.iter().map(|(s, _)| s.clone()).collect();
            TabBar::new("memory-scopes")
                .segmented()
                .small()
                .selected_index(at)
                .on_click(cx.listener(move |this, ix: &usize, _, cx| {
                    let next = ix.checked_sub(1).and_then(|ix| names.get(ix).cloned());
                    this.set_scope(next, cx);
                }))
                .child(Tab::new().label(format!("All {}", self.items.len())))
                .children(
                    scopes
                        .iter()
                        .map(|(name, count)| Tab::new().label(format!("{name} {count}"))),
                )
        });
        let forget = scope.map(|scope| {
            let count = scopes
                .iter()
                .find(|(s, _)| *s == scope)
                .map_or(0, |(_, n)| *n);
            let confirm = self.confirm_forget;
            h_flex()
                .gap_1()
                .when(confirm, |this| {
                    this.child(
                        Button::new("memory-forget-cancel")
                            .xsmall()
                            .ghost()
                            .label("Keep them")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.confirm_forget = false;
                                cx.notify();
                            })),
                    )
                })
                .child(
                    Button::new("memory-forget-scope")
                        .xsmall()
                        .when(confirm, |this| this.danger())
                        .when(!confirm, |this| this.ghost())
                        .icon(IconName::Delete)
                        .label(if confirm {
                            format!("Forget {count} for good")
                        } else {
                            format!("Forget all in {scope}")
                        })
                        .on_click(cx.listener(|this, _, _, cx| this.forget_scope(cx))),
                )
        });

        let rows: Vec<_> = shown
            .iter()
            .enumerate()
            .map(|(ix, item)| {
                let editing = self
                    .editing
                    .as_ref()
                    .filter(|(id, _)| id == &item.id)
                    .map(|(_, f)| f.clone());
                let id = item.id.clone();
                let row = v_flex()
                    .id(ElementId::Name(format!("memory-{}", item.id).into()))
                    .gap_1p5()
                    .px_4()
                    .py_2p5()
                    .when(ix > 0, |this| this.border_t_1().border_color(line))
                    .role(Role::ListItem)
                    .aria_label(memory_words(item));
                match editing {
                    Some(field) => row
                        .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                            let secondary = event.keystroke.modifiers.secondary(cx);
                            match edit_key(event.keystroke.key.as_str(), secondary) {
                                Some(EditKey::Save) => this.save_edit(cx),
                                Some(EditKey::Cancel) => this.cancel_edit(cx),
                                None => return,
                            }
                            cx.stop_propagation();
                        }))
                        .child(Textarea::new(&field).aria_label("Edit what's remembered"))
                        .child(
                            h_flex()
                                .gap_2()
                                .justify_end()
                                .when(!scopes.is_empty(), |this| {
                                    let names: Vec<SharedString> =
                                        scopes.iter().map(|(s, _)| s.clone()).collect();
                                    let at = self
                                        .edit_scope
                                        .as_ref()
                                        .and_then(|s| names.iter().position(|n| n == s))
                                        .unwrap_or(0);
                                    this.child(
                                        div().text_caption().text_color(quiet).child("Applies to"),
                                    )
                                    .child(
                                        TabBar::new(ElementId::Name(
                                            format!("scope-{}", item.id).into(),
                                        ))
                                        .segmented()
                                        .small()
                                        .selected_index(at)
                                        .on_click(cx.listener(move |this, ix: &usize, _, cx| {
                                            this.edit_scope = names.get(*ix).cloned();
                                            cx.notify();
                                        }))
                                        .children(
                                            scopes
                                                .iter()
                                                .map(|(name, _)| Tab::new().label(name.clone())),
                                        ),
                                    )
                                    .child(div().flex_1())
                                })
                                .child(
                                    Button::new(ElementId::Name(
                                        format!("cancel-{}", item.id).into(),
                                    ))
                                    .xsmall()
                                    .ghost()
                                    .label("Cancel")
                                    .on_click(cx.listener(|this, _, _, cx| this.cancel_edit(cx))),
                                )
                                .child(
                                    Button::new(ElementId::Name(
                                        format!("save-{}", item.id).into(),
                                    ))
                                    .xsmall()
                                    .primary()
                                    .label("Save")
                                    .on_click(cx.listener(|this, _, _, cx| this.save_edit(cx))),
                                ),
                        ),
                    None => {
                        let (edit_id, pause_id, remove_id) = (id.clone(), id.clone(), id);
                        let was_paused = item.paused;
                        row.child(
                            div()
                                .line_height(relative(1.45))
                                .when(item.paused, |this| this.text_color(quiet))
                                .child(item.text.clone()),
                        )
                        .child(
                            h_flex()
                                .gap_1()
                                .text_caption()
                                .text_color(quiet)
                                .when(item.paused, |this| {
                                    this.child(
                                        div()
                                            .text_color(warning)
                                            .font_medium()
                                            .child("Paused \u{b7}"),
                                    )
                                })
                                .when(!item.scope.is_empty(), |this| {
                                    this.child(
                                        Tag::secondary()
                                            .xsmall()
                                            .rounded_full()
                                            .child(item.scope.clone()),
                                    )
                                })
                                .child(div().flex_1().min_w_0().truncate().child(
                                    if item.updated.is_empty() {
                                        item.source.to_string()
                                    } else {
                                        format!("{} \u{b7} {}", item.source, item.updated)
                                    },
                                ))
                                .child(
                                    Button::new(ElementId::Name(
                                        format!("edit-{}", item.id).into(),
                                    ))
                                    .xsmall()
                                    .ghost()
                                    .icon(IconName::Replace)
                                    .accessibility_label(format!("Edit: {}", item.text))
                                    .on_click(cx.listener(
                                        move |this, _, window, cx| {
                                            this.start_edit(&edit_id, window, cx)
                                        },
                                    )),
                                )
                                .child(
                                    Button::new(ElementId::Name(
                                        format!("pause-{}", item.id).into(),
                                    ))
                                    .xsmall()
                                    .ghost()
                                    .icon(if was_paused {
                                        IconName::Play
                                    } else {
                                        IconName::Pause
                                    })
                                    .accessibility_label(format!(
                                        "{}: {}",
                                        if was_paused { "Use again" } else { "Pause" },
                                        item.text
                                    ))
                                    .on_click(cx.listener(
                                        move |_, _, _, cx| {
                                            cx.emit(MemoryEvent::Paused {
                                                id: pause_id.clone(),
                                                paused: !was_paused,
                                            });
                                        },
                                    )),
                                )
                                .child(
                                    Button::new(ElementId::Name(
                                        format!("remove-{}", item.id).into(),
                                    ))
                                    .xsmall()
                                    .ghost()
                                    .icon(IconName::Delete)
                                    .accessibility_label(format!("Remove: {}", item.text))
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| this.remove(&remove_id, cx),
                                    )),
                                ),
                        )
                    }
                }
            })
            .collect();

        let undo = self
            .removed
            .clone()
            .filter(|gone| !self.items.iter().any(|i| i.id == gone.id));

        v_flex()
            .w_full()
            .max_w(px(560.))
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .shadow(resting_shadow(cx))
            .child(
                v_flex()
                    .gap_2()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(line)
                    .child(
                        h_flex()
                            .gap_2()
                            .child(Icon::new(IconName::MemoryStick).small().text_color(quiet))
                            .child(div().flex_1().font_medium().child("What's remembered"))
                            .child(div().text_caption().text_color(quiet).child(format!(
                                "{} items{}",
                                self.items.len(),
                                if paused > 0 {
                                    format!(", {paused} paused")
                                } else {
                                    String::new()
                                }
                            ))),
                    )
                    .child(Input::new(&self.search).small().cleanable(true))
                    .when_some(scope_tabs, |this, tabs| {
                        this.child(
                            h_flex()
                                .gap_2()
                                .flex_wrap()
                                .child(tabs)
                                .child(div().flex_1())
                                .children(forget),
                        )
                    }),
            )
            .child(
                v_flex()
                    .id("memory-list")
                    .max_h(px(380.))
                    .overflow_y_scroll()
                    .children(rows)
                    .when(shown.is_empty(), |this| {
                        this.child(div().px_4().py_4().text_color(quiet).child(
                            if query.is_empty() {
                                "Nothing remembered yet.".to_string()
                            } else {
                                format!("Nothing matches \u{201c}{query}\u{201d}.")
                            },
                        ))
                    }),
            )
            .children(undo.map(|gone| {
                let id = gone.id.clone();
                h_flex()
                    .gap_2()
                    .px_4()
                    .py_2()
                    .border_t_1()
                    .border_color(line)
                    .bg(chip.opacity(0.5))
                    .text_caption()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(format!("Removed \u{201c}{}\u{201d}", gone.text)),
                    )
                    .child(
                        Button::new("memory-undo")
                            .xsmall()
                            .ghost()
                            .label("Undo")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.removed = None;
                                cx.emit(MemoryEvent::Undo(id.clone()));
                                cx.notify();
                            })),
                    )
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_reads_text_and_source() {
        let item = MemoryItem::new("a", "Pays suppliers on Fridays", "March close");
        assert!(matches_search(&item, "friday"));
        assert!(matches_search(&item, "MARCH"));
        assert!(matches_search(&item, "  "));
        assert!(!matches_search(&item, "payroll"));
    }

    #[test]
    fn scopes_count_in_first_seen_order() {
        let items = [
            MemoryItem::new("a", "A", "x").scope("Team"),
            MemoryItem::new("b", "B", "x"),
            MemoryItem::new("c", "C", "x").scope("Me"),
            MemoryItem::new("d", "D", "x").scope("Team"),
        ];
        assert_eq!(
            scopes_of(&items),
            [("Team".into(), 2), ("Me".into(), 1)] as [(SharedString, usize); 2]
        );
        assert!(scopes_of(&items[1..2]).is_empty());
    }

    #[test]
    fn edit_keys_save_and_cancel() {
        assert_eq!(edit_key("escape", false), Some(EditKey::Cancel));
        assert_eq!(edit_key("enter", true), Some(EditKey::Save));
        assert_eq!(edit_key("enter", false), None);
        assert_eq!(edit_key("a", true), None);
    }

    #[test]
    fn rows_say_scope_and_state() {
        let item = MemoryItem::new("a", "Year end is December", "Setup")
            .scope("Workspace")
            .paused(true);
        assert_eq!(
            memory_words(&item),
            "Year end is December, for Workspace, paused"
        );
        assert_eq!(memory_words(&MemoryItem::new("b", "Hi", "x")), "Hi");
    }
}
