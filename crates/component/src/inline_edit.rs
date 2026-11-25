// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Text that turns into a field where it stands, such as a document title or
//! a row's name.

use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render, Role,
    SharedString, StatefulInteractiveElement as _, Styled, Subscription, Window, div,
    prelude::FluentBuilder as _,
};

use crate::{
    ActiveTheme as _, Icon, Sizable as _, h_flex,
    input::{Input, InputEvent, InputState},
};

/// What an [`InlineEdit`] reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineEditEvent {
    /// The person confirmed a new value with Enter or by moving focus away.
    Commit(SharedString),
    /// The person pressed Escape; the value is unchanged.
    Cancel,
}

/// Text that becomes a text field when pressed, and text again when
/// confirmed.
///
/// At rest it reads as plain text with a quiet edit hint on hover. Pressing
/// it, or Enter or Space while it has focus, opens a field with the text
/// selected. Enter or moving focus away commits; Escape cancels. An empty
/// value is refused unless [`InlineEdit::allow_empty`] is set, and the old
/// value comes back.
///
/// ```ignore
/// let title = cx.new(|cx| InlineEdit::new("title", "Q3 plan", window, cx));
/// cx.subscribe(&title, |this, _, event: &InlineEditEvent, cx| { ... });
/// ```
pub struct InlineEdit {
    id: SharedString,
    value: SharedString,
    label: SharedString,
    placeholder: SharedString,
    allow_empty: bool,
    editing: bool,
    input: Entity<InputState>,
    focus_handle: FocusHandle,
    _subscription: Subscription,
}

impl EventEmitter<InlineEditEvent> for InlineEdit {}

impl InlineEdit {
    /// Editable text with the given value. `label` names it for screen
    /// readers, such as "Document title".
    pub fn new(
        id: impl Into<SharedString>,
        value: impl Into<SharedString>,
        label: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let value = value.into();
        let input = cx.new(|cx| InputState::new(window, cx).default_value(value.clone()));
        let subscription =
            cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
                InputEvent::PressEnter { .. } => this.commit(window, cx),
                InputEvent::Blur if this.editing => this.commit(window, cx),
                _ => {}
            });
        Self {
            id: id.into(),
            value,
            label: label.into(),
            placeholder: SharedString::default(),
            allow_empty: false,
            editing: false,
            input,
            focus_handle: cx.focus_handle(),
            _subscription: subscription,
        }
    }

    /// Text shown when the value is empty, such as "Untitled".
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Accept an empty value. Off by default.
    pub fn allow_empty(mut self, allow: bool) -> Self {
        self.allow_empty = allow;
        self
    }

    /// The current value.
    pub fn value(&self) -> &SharedString {
        &self.value
    }

    /// Whether the field is open.
    pub fn is_editing(&self) -> bool {
        self.editing
    }

    /// Replace the value from outside, such as after a save elsewhere. An
    /// open field is closed without reporting.
    pub fn set_value(
        &mut self,
        value: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.value = value.into();
        self.editing = false;
        let value = self.value.clone();
        self.input
            .update(cx, |input, cx| input.set_value(value, window, cx));
        cx.notify();
    }

    /// Open the field with the text selected.
    pub fn start_editing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing {
            return;
        }
        self.editing = true;
        let value = self.value.clone();
        self.input.update(cx, |input, cx| {
            input.set_value(value, window, cx);
            input.focus(window, cx);
            input.select_all(window, cx);
        });
        cx.notify();
    }

    /// Close the field and report the new value, or restore the old one if
    /// the new one is empty and empty values are not allowed.
    pub fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editing {
            return;
        }
        self.editing = false;
        let next: SharedString = self.input.read(cx).value().trim().to_string().into();
        if next.is_empty() && !self.allow_empty {
            self.restore_focus(window, cx);
            cx.emit(InlineEditEvent::Cancel);
            cx.notify();
            return;
        }
        let changed = next != self.value;
        self.value = next.clone();
        self.restore_focus(window, cx);
        if changed {
            cx.emit(InlineEditEvent::Commit(next));
        }
        cx.notify();
    }

    /// Close the field without changing the value.
    pub fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editing {
            return;
        }
        self.editing = false;
        self.restore_focus(window, cx);
        cx.emit(InlineEditEvent::Cancel);
        cx.notify();
    }

    /// Give focus back to the text when the field was focused, so the
    /// keyboard does not lose its place.
    fn restore_focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.input.read(cx).focus_handle(cx).is_focused(window) {
            self.focus_handle.focus(window, cx);
        }
    }
}

impl Focusable for InlineEdit {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for InlineEdit {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        if self.editing {
            return div()
                .id(self.id.clone())
                .w_full()
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" {
                        this.cancel(window, cx);
                        cx.stop_propagation();
                    }
                }))
                .child(Input::new(&self.input).small())
                .into_any_element();
        }

        let empty = self.value.is_empty();
        let shown = if empty {
            self.placeholder.clone()
        } else {
            self.value.clone()
        };
        let focused = self.focus_handle.is_focused(window);
        let ring = theme.ring;
        h_flex()
            .id(self.id.clone())
            .track_focus(&self.focus_handle)
            .tab_index(0)
            .role(Role::Button)
            .aria_label(format!("{}: {}. Press to edit", self.label, self.value))
            .group("inline-edit")
            .w_full()
            .min_w_0()
            .h_7()
            .px_1p5()
            .gap_1p5()
            .rounded(theme.radius)
            .cursor_text()
            .hover(|this| this.bg(theme.muted))
            .when(focused, |this| this.border_1().border_color(ring))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(if empty {
                        theme.muted_foreground
                    } else {
                        theme.foreground
                    })
                    .child(shown),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .invisible()
                    .group_hover("inline-edit", |this| this.visible())
                    .when(focused, |this| this.visible())
                    .child(
                        Icon::new(isonomic_ui_assets::IconName::Edit)
                            .xsmall()
                            .text_color(theme.muted_foreground),
                    ),
            )
            // A focused element with a click handler turns Enter and Space
            // into a click, so the keyboard opens the field the same way.
            .on_click(cx.listener(|this, _, window, cx| this.start_editing(window, cx)))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{TestAppContext, VisualTestContext};
    use std::{cell::RefCell, rc::Rc};

    fn setup(
        cx: &mut TestAppContext,
    ) -> (
        Entity<InlineEdit>,
        &mut VisualTestContext,
        Rc<RefCell<Vec<InlineEditEvent>>>,
    ) {
        cx.update(crate::init);
        let events = Rc::new(RefCell::new(Vec::new()));
        let (edit, cx) = cx.add_window_view(|window, cx| {
            InlineEdit::new("title", "Q3 plan", "Document title", window, cx)
        });
        cx.update({
            let events = events.clone();
            let edit = edit.clone();
            move |_, cx| {
                cx.subscribe(&edit, move |_, event: &InlineEditEvent, _| {
                    events.borrow_mut().push(event.clone())
                })
                .detach();
            }
        });
        (edit, cx, events)
    }

    #[gpui::test]
    fn commit_reports_a_changed_value(cx: &mut TestAppContext) {
        let (edit, cx, events) = setup(cx);
        cx.update(|window, cx| {
            edit.update(cx, |edit, cx| {
                edit.start_editing(window, cx);
                edit.input
                    .update(cx, |input, cx| input.set_value("Q4 plan", window, cx));
                edit.commit(window, cx);
            })
        });
        assert_eq!(
            edit.read_with(cx, |e, _| e.value().clone()).as_ref(),
            "Q4 plan"
        );
        assert_eq!(
            events.borrow().as_slice(),
            &[InlineEditEvent::Commit("Q4 plan".into())]
        );
    }

    #[gpui::test]
    fn an_empty_value_is_refused(cx: &mut TestAppContext) {
        let (edit, cx, events) = setup(cx);
        cx.update(|window, cx| {
            edit.update(cx, |edit, cx| {
                edit.start_editing(window, cx);
                edit.input
                    .update(cx, |input, cx| input.set_value("   ", window, cx));
                edit.commit(window, cx);
            })
        });
        assert_eq!(
            edit.read_with(cx, |e, _| e.value().clone()).as_ref(),
            "Q3 plan"
        );
        assert_eq!(events.borrow().as_slice(), &[InlineEditEvent::Cancel]);
    }

    #[gpui::test]
    fn cancel_keeps_the_value(cx: &mut TestAppContext) {
        let (edit, cx, events) = setup(cx);
        cx.update(|window, cx| {
            edit.update(cx, |edit, cx| {
                edit.start_editing(window, cx);
                edit.input
                    .update(cx, |input, cx| input.set_value("Draft", window, cx));
                edit.cancel(window, cx);
            })
        });
        assert!(!edit.read_with(cx, |e, _| e.is_editing()));
        assert_eq!(
            edit.read_with(cx, |e, _| e.value().clone()).as_ref(),
            "Q3 plan"
        );
        assert_eq!(events.borrow().as_slice(), &[InlineEditEvent::Cancel]);
    }

    #[gpui::test]
    fn an_unchanged_value_reports_nothing(cx: &mut TestAppContext) {
        let (edit, cx, events) = setup(cx);
        cx.update(|window, cx| {
            edit.update(cx, |edit, cx| {
                edit.start_editing(window, cx);
                edit.commit(window, cx);
            })
        });
        assert!(events.borrow().is_empty());
    }
}
