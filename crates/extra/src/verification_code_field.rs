// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Verification code field: boxes for a one-time code, one digit each, that fill as you type or paste and report the code when every box is full.
//!
//! Typing a digit fills the next box, Backspace empties the last one, and
//! pasting drops in every digit from the clipboard, ignoring spaces and
//! dashes. Boxes can be split into groups, such as 3 and 3. While the owner
//! checks the code the boxes lock; a wrong code shows its message and the
//! boxes clear for another go. A resend link counts down before it opens.

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex, v_flex,
};
use isonomic_ui::{
    App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement as _, IntoElement,
    KeyDownEvent, ParentElement as _, Render, Role, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

/// The code after `incoming` is typed or pasted: digits only, at most `len` of them.
pub fn accept_digits(current: &str, incoming: &str, len: usize) -> String {
    current
        .chars()
        .chain(incoming.chars())
        .filter(|c| c.is_ascii_digit())
        .take(len)
        .collect()
}

/// The resend link's words for `seconds` left.
pub fn resend_label(seconds: u32) -> String {
    if seconds == 0 {
        "Resend code".into()
    } else {
        format!("Resend in {}:{:02}", seconds / 60, seconds % 60)
    }
}

/// What the field reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeEvent {
    /// Every box is full.
    Complete(String),
    /// The resend link was clicked.
    Resend,
}

/// Boxes for a one-time code.
pub struct VerificationCodeField {
    focus: FocusHandle,
    code: String,
    len: usize,
    groups: Vec<usize>,
    checking: bool,
    error: Option<SharedString>,
    resend_in: Option<u32>,
}

impl EventEmitter<CodeEvent> for VerificationCodeField {}

impl Focusable for VerificationCodeField {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl VerificationCodeField {
    /// Create boxes for a code of `len` digits.
    pub fn new(len: usize, cx: &mut Context<Self>) -> Self {
        let len = len.max(1);
        Self {
            focus: cx.focus_handle(),
            code: String::new(),
            len,
            groups: vec![len],
            checking: false,
            error: None,
            resend_in: None,
        }
    }

    /// Split the boxes into groups, such as `[3, 3]`.
    pub fn groups(mut self, groups: Vec<usize>) -> Self {
        if groups.iter().sum::<usize>() == self.len {
            self.groups = groups;
        }
        self
    }

    /// The digits typed so far.
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Lock the boxes while the code is checked.
    pub fn set_checking(&mut self, checking: bool, cx: &mut Context<Self>) {
        self.checking = checking;
        cx.notify();
    }

    /// Show why the code was refused and clear the boxes, or clear the message.
    pub fn set_error(&mut self, error: Option<SharedString>, cx: &mut Context<Self>) {
        if error.is_some() {
            self.code.clear();
        }
        self.error = error;
        self.checking = false;
        cx.notify();
    }

    /// Show the resend link, counting down from `seconds`. `None` hides it.
    pub fn set_resend_in(&mut self, seconds: Option<u32>, cx: &mut Context<Self>) {
        self.resend_in = seconds;
        cx.notify();
    }

    fn take(&mut self, incoming: &str, cx: &mut Context<Self>) {
        let next = accept_digits(&self.code, incoming, self.len);
        if next == self.code {
            return;
        }
        self.code = next;
        self.error = None;
        if self.code.len() == self.len {
            cx.emit(CodeEvent::Complete(self.code.clone()));
        }
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.checking {
            return;
        }
        let keys = &event.keystroke;
        if (keys.modifiers.platform || keys.modifiers.control) && keys.key == "v" {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                self.take(&text, cx);
            }
            cx.stop_propagation();
            return;
        }
        match keys.key.as_str() {
            "backspace" => {
                self.code.pop();
                self.error = None;
                cx.notify();
                cx.stop_propagation();
            }
            "escape" => {
                self.code.clear();
                cx.notify();
            }
            _ => {
                if let Some(ch) = keys.key_char.as_deref() {
                    self.take(ch, cx);
                }
            }
        }
    }
}

impl Render for VerificationCodeField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let line = theme.border;
        let ring = theme.ring;
        let danger = theme.danger;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let link = theme.link;
        let focused = self.focus.is_focused(window);
        let digits: Vec<char> = self.code.chars().collect();
        let active = digits.len().min(self.len - 1);
        let error = self.error.clone();

        let mut start = 0;
        let groups = self
            .groups
            .clone()
            .into_iter()
            .enumerate()
            .map(|(g, size)| {
                let boxes = (start..start + size).map(|ix| {
                    let digit = digits.get(ix).copied();
                    let here = focused && !self.checking && ix == active;
                    div()
                        .size(px(40.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(8.))
                        .border_1()
                        .border_color(if error.is_some() {
                            danger
                        } else if here {
                            ring
                        } else {
                            line
                        })
                        .when(here, |this| this.border_2())
                        .bg(fill)
                        .text_lg()
                        .font_medium()
                        .when(self.checking, |this| this.opacity(0.6))
                        .children(digit.map(|d| d.to_string()))
                });
                start += size;
                h_flex()
                    .gap_1p5()
                    .children(boxes)
                    .when(g + 1 < self.groups.len(), |this| {
                        this.child(div().w(px(10.)).h(px(2.)).mx_1().bg(line))
                    })
            });

        let resend = self.resend_in.map(|seconds| {
            let label = resend_label(seconds);
            if seconds == 0 {
                div()
                    .id("resend")
                    .text_body()
                    .text_color(link)
                    .cursor_pointer()
                    .child(label)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.code.clear();
                        cx.emit(CodeEvent::Resend);
                        cx.notify();
                    }))
                    .into_any_element()
            } else {
                div()
                    .text_body()
                    .text_color(quiet)
                    .child(label)
                    .into_any_element()
            }
        });

        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .id("code-boxes")
                    .track_focus(&self.focus)
                    .gap_1()
                    .cursor_text()
                    .role(Role::TextInput)
                    .aria_label(format!(
                        "Verification code, {} of {} digits",
                        digits.len(),
                        self.len
                    ))
                    .on_click(cx.listener(|this, _, window, cx| {
                        window.focus(&this.focus, cx);
                    }))
                    .on_key_down(cx.listener(Self::key_down))
                    .children(groups),
            )
            .when(self.checking, |this| {
                this.child(
                    div()
                        .text_body()
                        .text_color(quiet)
                        .child("Checking\u{2026}"),
                )
            })
            .when_some(error, |this, error| {
                this.child(
                    h_flex()
                        .gap_1()
                        .text_body()
                        .text_color(danger)
                        .child(Icon::new(IconName::CircleAlert).xsmall())
                        .child(error),
                )
            })
            .children(resend)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits_fill_and_resend_counts_down() {
        assert_eq!(accept_digits("", "12 3-456", 6), "123456");
        assert_eq!(accept_digits("12", "34567890", 6), "123456");
        assert_eq!(accept_digits("12", "a", 6), "12");
        assert_eq!(resend_label(84), "Resend in 1:24");
        assert_eq!(resend_label(5), "Resend in 0:05");
        assert_eq!(resend_label(0), "Resend code");
    }
}
