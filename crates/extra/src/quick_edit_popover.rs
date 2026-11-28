// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Quick edit popover: a value shown in place that opens a small popover to change it, with Save and Cancel, without leaving the page.
//!
//! Click the value to open the popover with the field focused and filled.
//! Enter or Save reports the new text; Escape or Cancel closes without
//! change. While the owner saves, the buttons wait; if the save fails its
//! reason shows in the popover and the text stays for another go.

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    popover::Popover,
    v_flex,
};
use isonomic_ui::{
    AppContext as _, Context, ElementId, Entity, EventEmitter, InteractiveElement as _,
    IntoElement, KeyDownEvent, ParentElement as _, Render, SharedString, Styled as _, Subscription,
    Window, div, prelude::FluentBuilder as _, px,
};

/// Whether a save is worth reporting: the text changed and isn't blank when blanks aren't allowed.
pub fn worth_saving(before: &str, after: &str, allow_blank: bool) -> bool {
    let after = after.trim();
    after != before.trim() && (allow_blank || !after.is_empty())
}

/// What the popover reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuickEditEvent {
    /// Save this text.
    Save(SharedString),
}

/// A value with a popover to change it.
pub struct QuickEditPopover {
    id: ElementId,
    label: SharedString,
    value: SharedString,
    field: Entity<InputState>,
    open: bool,
    saving: bool,
    error: Option<SharedString>,
    allow_blank: bool,
    _events: Subscription,
}

impl EventEmitter<QuickEditEvent> for QuickEditPopover {}

impl QuickEditPopover {
    /// Create a value named `label`, such as "Invoice title".
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        value: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let field = cx.new(|cx| InputState::new(window, cx));
        let events = cx.subscribe_in(
            &field,
            window,
            |this: &mut Self, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.save(window, cx);
                }
            },
        );
        Self {
            id: id.into(),
            label: label.into(),
            value: value.into(),
            field,
            open: false,
            saving: false,
            error: None,
            allow_blank: false,
            _events: events,
        }
    }

    /// Let the value be saved empty.
    pub fn allow_blank(mut self) -> Self {
        self.allow_blank = true;
        self
    }

    /// The current value.
    pub fn value(&self) -> &SharedString {
        &self.value
    }

    /// The owner saved the new value.
    pub fn saved(&mut self, value: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.value = value.into();
        self.saving = false;
        self.error = None;
        self.open = false;
        cx.notify();
    }

    /// The owner couldn't save, and why.
    pub fn save_failed(&mut self, why: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.saving = false;
        self.error = Some(why.into());
        cx.notify();
    }

    fn set_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        if open == self.open || self.saving {
            return;
        }
        self.open = open;
        self.error = None;
        if open {
            let value = self.value.clone();
            self.field.update(cx, |field, cx| {
                field.set_value(value, window, cx);
                field.focus(window, cx);
            });
        }
        cx.notify();
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let typed = self.field.read(cx).value();
        if worth_saving(&self.value, &typed, self.allow_blank) {
            self.saving = true;
            self.error = None;
            cx.emit(QuickEditEvent::Save(typed.trim().to_string().into()));
            cx.notify();
        } else {
            self.set_open(false, window, cx);
        }
    }
}

impl Render for QuickEditPopover {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let danger = cx.theme().danger;
        let this = cx.entity();
        let field = self.field.clone();
        let label = self.label.clone();
        let saving = self.saving;
        let error = self.error.clone();
        let shown = if self.value.is_empty() {
            SharedString::from("Add\u{2026}")
        } else {
            self.value.clone()
        };
        let toggle = cx.entity();

        Popover::new(self.id.clone())
            .open(self.open)
            .on_open_change(move |open, window, cx| {
                toggle.update(cx, |this, cx| this.set_open(*open, window, cx))
            })
            .trigger(
                Button::new(ElementId::NamedChild(
                    self.id.clone().into(),
                    "value".into(),
                ))
                .small()
                .ghost()
                .label(shown)
                .icon(Icon::new(IconName::ChevronDown).xsmall().text_color(quiet))
                .accessibility_label(format!("{}: {}. Edit", self.label, self.value)),
            )
            .content(move |_, _, _| {
                let save = this.clone();
                let cancel = this.clone();
                let keys = this.clone();
                v_flex()
                    .w(px(280.))
                    .gap_2()
                    .on_key_down(move |event: &KeyDownEvent, window, cx| {
                        if event.keystroke.key == "escape" {
                            keys.update(cx, |this, cx| this.set_open(false, window, cx));
                        }
                    })
                    .child(
                        div()
                            .text_label_small()
                            .text_color(quiet)
                            .child(label.clone()),
                    )
                    .child(
                        Input::new(&field)
                            .small()
                            .disabled(saving)
                            .aria_label(label.clone()),
                    )
                    .when_some(error.clone(), |this, error| {
                        this.child(
                            h_flex()
                                .gap_1()
                                .text_caption()
                                .text_color(danger)
                                .child(Icon::new(IconName::CircleAlert).xsmall())
                                .child(error),
                        )
                    })
                    .child(
                        h_flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("quick-edit-cancel")
                                    .xsmall()
                                    .ghost()
                                    .label("Cancel")
                                    .disabled(saving)
                                    .on_click(move |_, window, cx| {
                                        cancel
                                            .update(cx, |this, cx| this.set_open(false, window, cx))
                                    }),
                            )
                            .child(
                                Button::new("quick-edit-save")
                                    .xsmall()
                                    .primary()
                                    .label(if saving { "Saving\u{2026}" } else { "Save" })
                                    .disabled(saving)
                                    .on_click(move |_, window, cx| {
                                        save.update(cx, |this, cx| this.save(window, cx))
                                    }),
                            ),
                    )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_real_changes_are_saved() {
        assert!(worth_saving("March retainer", "April retainer", false));
        assert!(!worth_saving("March retainer", " March retainer ", false));
        assert!(!worth_saving("March retainer", "  ", false));
        assert!(worth_saving("March retainer", "", true));
    }
}
