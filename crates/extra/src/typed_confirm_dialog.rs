//! Typed confirm dialog: a confirmation for actions that can't be undone, listing what will happen and asking for a word to be typed first.
//!
//! The dialog names the action, lists its consequences, and asks the person
//! to type a word, such as the invoice number, before the action button
//! turns on. Escape or Cancel backs out. It holds only the typed text; the
//! owner shows and hides it.

use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div, prelude::FluentBuilder as _, px, relative,
};

use crate::common::{Tone, lifted_shadow};

/// Whether what's typed matches the word, ignoring case and spaces at the ends.
pub fn typed_matches(typed: &str, word: &str) -> bool {
    typed.trim().eq_ignore_ascii_case(word.trim())
}

/// What the dialog reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmEvent {
    /// Go ahead.
    Confirm,
    /// Back out.
    Cancel,
}

/// A confirmation that needs a word typed first.
pub struct TypedConfirmDialog {
    title: SharedString,
    consequences: Vec<SharedString>,
    word: SharedString,
    action: SharedString,
    field: Entity<InputState>,
    _events: Subscription,
}

impl EventEmitter<ConfirmEvent> for TypedConfirmDialog {}

impl Focusable for TypedConfirmDialog {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.field.focus_handle(cx)
    }
}

impl TypedConfirmDialog {
    /// Create a dialog for an action, the word to type, and the action button's label.
    pub fn new(
        title: impl Into<SharedString>,
        word: impl Into<SharedString>,
        action: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let word = word.into();
        let placeholder = format!("Type {word}");
        let field = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let events = cx.subscribe(&field, |this: &mut Self, field, event: &InputEvent, cx| match event {
            InputEvent::Change => cx.notify(),
            InputEvent::PressEnter { .. } => {
                if typed_matches(&field.read(cx).value(), &this.word) {
                    cx.emit(ConfirmEvent::Confirm);
                }
            }
            _ => {}
        });
        Self {
            title: title.into(),
            consequences: Vec::new(),
            word,
            action: action.into(),
            field,
            _events: events,
        }
    }

    /// List what will happen.
    pub fn set_consequences(&mut self, consequences: Vec<SharedString>, cx: &mut Context<Self>) {
        self.consequences = consequences;
        cx.notify();
    }

    /// Empty the field, for the next time it's shown.
    pub fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.field.update(cx, |f, cx| f.set_value("", window, cx));
    }
}

impl Render for TypedConfirmDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let danger = Tone::Danger.color(cx);
        let ready = typed_matches(&self.field.read(cx).value(), &self.word);
        v_flex()
            .w(px(420.))
            .max_w_full()
            .gap_4()
            .p_5()
            .rounded(theme.radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .shadow(lifted_shadow(cx))
            .on_key_down(cx.listener(|_, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    cx.emit(ConfirmEvent::Cancel);
                }
            }))
            .child(
                h_flex()
                    .items_start()
                    .gap_3()
                    .child(
                        div()
                            .size(px(32.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .bg(danger.opacity(0.12))
                            .child(Icon::new(IconName::TriangleAlert).small().text_color(danger)),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(div().text_base().font_semibold().child(self.title.clone()))
                            .child(div().text_color(quiet).child("This can't be undone.")),
                    ),
            )
            .when(!self.consequences.is_empty(), |this| {
                this.child(v_flex().gap_1p5().pl(px(44.)).children(self.consequences.iter().map(|c| {
                    h_flex()
                        .items_start()
                        .gap_2()
                        .child(div().mt(px(7.)).size(px(4.)).flex_none().rounded_full().bg(quiet))
                        .child(div().line_height(relative(1.4)).child(c.clone()))
                })))
            })
            .child(
                v_flex()
                    .gap_1p5()
                    .child(div().text_xs().text_color(quiet).child(format!("To confirm, type {} below.", self.word)))
                    .child(Input::new(&self.field)),
            )
            .child(
                h_flex()
                    .gap_2()
                    .justify_end()
                    .child(Button::new("confirm-cancel").small().ghost().label("Cancel").on_click(cx.listener(|_, _, _, cx| cx.emit(ConfirmEvent::Cancel))))
                    .child(
                        Button::new("confirm-go")
                            .small()
                            .danger()
                            .label(self.action.clone())
                            .disabled(!ready)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(ConfirmEvent::Confirm))),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_word_matches_loosely() {
        assert!(typed_matches(" inv-2240 ", "INV-2240"));
        assert!(!typed_matches("2240", "INV-2240"));
    }
}
