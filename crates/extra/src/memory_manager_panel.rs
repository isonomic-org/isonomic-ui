//! Memory manager panel: what an assistant remembers, to search, edit, pause or remove, with undo.
//!
//! Each remembered item shows what it says, where it came from and when it
//! last changed. Search narrows the list as you type. Edit opens the item in
//! place with Save and Cancel; pause keeps an item but stops it being used;
//! remove takes it out, with an undo line until the next change. The panel
//! reports each of these and shows whatever items its owner passes back.

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    v_flex,
};
use isonomic_ui::{
    AppContext as _, Context, ElementId, Entity, EventEmitter, Focusable as _,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, Role, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window, div,
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
        self.editing = Some((id.clone(), field));
        cx.notify();
    }

    fn save_edit(&mut self, cx: &mut Context<Self>) {
        if let Some((id, field)) = self.editing.take() {
            let text = field.read(cx).value();
            let text = text.trim().to_string();
            if !text.is_empty() {
                cx.emit(MemoryEvent::Edited {
                    id,
                    text: text.into(),
                });
            }
        }
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
        let shown: Vec<MemoryItem> = self
            .items
            .iter()
            .filter(|i| matches_search(i, &query))
            .cloned()
            .collect();
        let paused = self.items.iter().filter(|i| i.paused).count();

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
                    .aria_label(format!(
                        "{}{}",
                        item.text,
                        if item.paused { ", paused" } else { "" }
                    ));
                match editing {
                    Some(field) => row
                        .child(Textarea::new(&field).aria_label("Edit what's remembered"))
                        .child(
                            h_flex()
                                .gap_2()
                                .justify_end()
                                .child(
                                    Button::new(ElementId::Name(
                                        format!("cancel-{}", item.id).into(),
                                    ))
                                    .xsmall()
                                    .ghost()
                                    .label("Cancel")
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.editing = None;
                                            cx.notify();
                                        },
                                    )),
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
                                .text_xs()
                                .text_color(quiet)
                                .when(item.paused, |this| {
                                    this.child(
                                        div()
                                            .text_color(warning)
                                            .font_medium()
                                            .child("Paused \u{b7}"),
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
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
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
                            .child(div().text_xs().text_color(quiet).child(format!(
                                "{} items{}",
                                self.items.len(),
                                if paused > 0 {
                                    format!(", {paused} paused")
                                } else {
                                    String::new()
                                }
                            ))),
                    )
                    .child(Input::new(&self.search).small().cleanable(true)),
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
                    .text_xs()
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
}
