// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Comment thread: one conversation attached to a selection, a cell or a line.
//!
//! People reply in a field at the bottom. An assistant's replies can carry a
//! status line that folds open to show what it did, and while an answer is on
//! its way the thread shows a working row.
//!
//! The thread is a view. It emits [`CommentThreadEvent::Reply`] for every reply
//! posted; whoever owns it decides whether an assistant answers, and feeds the
//! answer back with [`CommentThread::push`].

use isonomic_ui::base::PressableExt as _;
use std::time::{Duration, SystemTime};

use isonomic_ui::base::Button as Pressable;
use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
    ThemeStyled as _,
    avatar::Avatar,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    shimmer::ShimmerText,
    v_flex,
};
use isonomic_ui::{
    AppContext as _, Context, Entity, EventEmitter, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, ScrollHandle, SharedString, StatefulInteractiveElement as _,
    Styled as _, Subscription, Task, Window, div, prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// How often the relative times are refreshed.
const REFRESH: Duration = Duration::from_secs(30);

/// Someone who writes in the thread.
#[derive(Debug, Clone)]
pub struct ThreadAuthor {
    name: SharedString,
    assistant: bool,
}

impl ThreadAuthor {
    /// A person.
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            assistant: false,
        }
    }

    /// An assistant, which can answer replies and report what it did.
    pub fn assistant(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            assistant: true,
        }
    }
}

/// One message in the thread.
#[derive(Debug, Clone)]
pub struct ThreadMessage {
    id: SharedString,
    author: ThreadAuthor,
    created_at: SystemTime,
    text: SharedString,
    status: Option<SharedString>,
    details: Vec<SharedString>,
}

impl ThreadMessage {
    /// Create a message written now.
    pub fn new(
        id: impl Into<SharedString>,
        author: ThreadAuthor,
        text: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            author,
            created_at: SystemTime::now(),
            text: text.into(),
            status: None,
            details: Vec::new(),
        }
    }

    /// Set when the message was written.
    pub fn created_at(mut self, created_at: SystemTime) -> Self {
        self.created_at = created_at;
        self
    }

    /// A short line saying what was done, such as "Edit finished".
    pub fn status(mut self, status: impl Into<SharedString>) -> Self {
        self.status = Some(status.into());
        self
    }

    /// Lines shown when the status is folded open.
    pub fn details(mut self, details: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.details = details.into_iter().map(Into::into).collect();
        self
    }
}

/// What the thread tells its owner.
#[derive(Debug, Clone)]
pub enum CommentThreadEvent {
    /// A reply was posted, with its text.
    Reply(SharedString),
    /// The close button was pressed.
    Close,
}

/// A single comment with its replies and a reply field.
pub struct CommentThread {
    title: SharedString,
    messages: Vec<ThreadMessage>,
    me: ThreadAuthor,
    assistant: Option<ThreadAuthor>,
    closable: bool,
    working: bool,
    /// Ids of the messages whose details are folded open.
    open: Vec<SharedString>,
    replies: usize,
    input: Entity<InputState>,
    scroll: ScrollHandle,
    _input_events: Subscription,
    _refresh: Task<()>,
}

impl EventEmitter<CommentThreadEvent> for CommentThread {}

impl CommentThread {
    /// Create a thread in which `me` writes the replies.
    pub fn new(me: ThreadAuthor, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Add a comment…"));
        let input_events = cx.subscribe_in(&input, window, |this, _, event, window, cx| {
            match event {
                InputEvent::PressEnter { .. } => this.send(window, cx),
                // The send button follows whether there is anything to send.
                InputEvent::Change => cx.notify(),
                _ => {}
            }
        });
        let refresh = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(REFRESH).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        });

        Self {
            title: "Comment".into(),
            messages: Vec::new(),
            me,
            assistant: None,
            closable: false,
            working: false,
            open: Vec::new(),
            replies: 0,
            input,
            scroll: ScrollHandle::new(),
            _input_events: input_events,
            _refresh: refresh,
        }
    }

    /// Set the heading.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Start with these messages.
    pub fn messages(mut self, messages: impl IntoIterator<Item = ThreadMessage>) -> Self {
        self.messages = messages.into_iter().collect();
        self
    }

    /// Show a close button, which emits [`CommentThreadEvent::Close`].
    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }

    /// Name the assistant that can answer. It is shown in the working row and
    /// invited in the reply field's placeholder.
    pub fn set_assistant(
        &mut self,
        assistant: ThreadAuthor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let placeholder = format!("Reply, or ask @{}…", assistant.name);
        self.input.update(cx, |input, cx| {
            input.set_placeholder(placeholder, window, cx)
        });
        self.assistant = Some(assistant);
        cx.notify();
    }

    /// Add a message, for example an assistant's answer, and scroll to it.
    pub fn push(&mut self, message: ThreadMessage, cx: &mut Context<Self>) {
        self.messages.push(message);
        self.scroll.scroll_to_bottom();
        cx.notify();
    }

    /// Show or hide the working row. Replies cannot be sent while it shows.
    pub fn set_working(&mut self, working: bool, cx: &mut Context<Self>) {
        self.working = working;
        self.scroll.scroll_to_bottom();
        cx.notify();
    }

    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.input.read(cx).value().trim().to_string();
        if text.is_empty() || self.working {
            return;
        }
        self.input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.replies += 1;
        let text = SharedString::from(text);
        let reply = ThreadMessage::new(
            format!("reply-{}", self.replies),
            self.me.clone(),
            text.clone(),
        );
        self.push(reply, cx);
        cx.emit(CommentThreadEvent::Reply(text));
    }

    fn toggle_details(&mut self, id: &SharedString, cx: &mut Context<Self>) {
        match self.open.iter().position(|open| open == id) {
            Some(ix) => {
                self.open.remove(ix);
            }
            None => self.open.push(id.clone()),
        }
        cx.notify();
    }
}

/// How long ago something happened, in a few characters.
fn ago(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    let minutes = (seconds + 30) / 60;
    let hours = (minutes + 30) / 60;
    let days = (hours + 12) / 24;
    if seconds < 45 {
        "just now".into()
    } else if minutes < 60 {
        format!("{minutes} min ago")
    } else if hours < 24 {
        format!("{hours} h ago")
    } else if days < 30 {
        format!("{days} d ago")
    } else if days < 365 {
        format!("{} mo ago", (days + 15) / 30)
    } else {
        format!("{} y ago", (days + 182) / 365)
    }
}

/// Split a leading slash command, such as `/edit`, from the rest of the text.
fn split_command(text: &str) -> (Option<&str>, &str) {
    let text = text.trim();
    if !text.starts_with('/') {
        return (None, text);
    }
    let end = text.find(char::is_whitespace).unwrap_or(text.len());
    let (command, rest) = text.split_at(end);
    if command.len() < 2 {
        return (None, text);
    }
    (Some(command), rest.trim_start())
}

impl Render for CommentThread {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let success = theme.success;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;
        let mono = theme.mono_font_family.clone();
        let now = SystemTime::now();
        let can_send = !self.working && !self.input.read(cx).value().trim().is_empty();

        let meta = |author: &ThreadAuthor, time: Option<String>| {
            h_flex()
                .gap_2()
                .child(if author.assistant {
                    Avatar::new().placeholder(IconName::Bot).xsmall()
                } else {
                    Avatar::new().name(author.name.clone()).xsmall()
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font_medium()
                        .child(author.name.clone()),
                )
                .children(time.map(|time| {
                    div()
                        .flex_none()
                        .text_caption()
                        .text_color(quiet)
                        .child(time)
                }))
        };

        let messages = self.messages.iter().enumerate().map(|(ix, message)| {
            let elapsed = now.duration_since(message.created_at).unwrap_or_default();
            let (command, text) = split_command(&message.text);
            let is_open = self.open.contains(&message.id);
            let has_details = !message.details.is_empty();

            let status = message.status.clone().map(|status| {
                let id = message.id.clone();
                // Keyed by the thread too, so two threads in one window
                // never share a status line's id.
                let thread = cx.entity_id();
                let focus_handle = window
                    .use_keyed_state(
                        SharedString::from(format!("status-focus-{thread}-{ix}")),
                        cx,
                        |_, cx| cx.focus_handle(),
                    )
                    .read(cx)
                    .clone();
                let is_focused = focus_handle.is_focused(window);
                v_flex()
                    .gap_1()
                    .child(
                        Pressable::new(SharedString::from(format!("status-{thread}-{ix}")))
                            .accessibility_label(status.clone())
                            .when(has_details, |this| this.aria_expanded(is_open))
                            .track_focus(&focus_handle)
                            .disabled(!has_details)
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.toggle_details(&id, cx)),
                            )
                            .justify_start()
                            .gap_1p5()
                            .rounded(radius)
                            .line_height(relative(1.4))
                            .pressable(has_details)
                            .when(is_focused, |this| this.focus_ring_style(window, cx))
                            .child(
                                Icon::new(IconName::CircleCheck)
                                    .small()
                                    .flex_none()
                                    .text_color(success),
                            )
                            .child(div().min_w_0().truncate().font_medium().child(status))
                            .when(has_details, |this| {
                                this.child(
                                    Icon::new(if is_open {
                                        IconName::ChevronUp
                                    } else {
                                        IconName::ChevronDown
                                    })
                                    .xsmall()
                                    .flex_none()
                                    .text_color(quiet),
                                )
                            }),
                    )
                    .when(is_open && has_details, |this| {
                        this.child(
                            v_flex().pl(px(22.)).gap_0p5().text_color(quiet).children(
                                message
                                    .details
                                    .iter()
                                    .map(|detail| div().child(detail.clone())),
                            ),
                        )
                    })
            });

            v_flex()
                .gap_1()
                .child(meta(&message.author, Some(ago(elapsed))))
                .child(
                    v_flex()
                        .pl(px(28.))
                        .gap_1p5()
                        .when(!message.text.is_empty(), |this| {
                            this.child(
                                h_flex()
                                    .flex_wrap()
                                    .items_start()
                                    .gap_1p5()
                                    .children(command.map(|command| {
                                        div()
                                            .flex_none()
                                            .px_1()
                                            .rounded(px(4.))
                                            .bg(chip)
                                            .text_color(quiet)
                                            .font_family(mono.clone())
                                            .text_caption()
                                            .child(command.to_string())
                                    }))
                                    .child(div().flex_1().min_w_0().child(text.to_string())),
                            )
                        })
                        .children(status),
                )
        });
        let messages: Vec<_> = messages.collect();

        let working = self
            .assistant
            .clone()
            .filter(|_| self.working)
            .map(|assistant| {
                v_flex().gap_1().child(meta(&assistant, None)).child(
                    div()
                        .pl(px(28.))
                        .text_color(quiet)
                        .child(ShimmerText::new("Working on it…")),
                )
            });

        let count = match self.messages.len() {
            1 => "1 message".to_string(),
            n => format!("{n} messages"),
        };

        v_flex()
            .w_full()
            .max_w(px(420.))
            .overflow_hidden()
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .line_height(relative(1.5))
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .flex_none()
                    .gap_2()
                    .h(px(48.))
                    .pl_4()
                    .pr_2()
                    .border_b_1()
                    .border_color(line)
                    .child(div().font_medium().child(self.title.clone()))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_caption()
                            .text_color(quiet)
                            .child(count),
                    )
                    .when(self.closable, |this| {
                        this.child(
                            Button::new("close")
                                .ghost()
                                .xsmall()
                                .icon(IconName::Close)
                                .accessibility_label("Close comment")
                                .on_click(
                                    cx.listener(|_, _, _, cx| cx.emit(CommentThreadEvent::Close)),
                                ),
                        )
                    }),
            )
            .child(
                v_flex()
                    .id("messages")
                    .gap_3()
                    .p_4()
                    .max_h(px(320.))
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .children(messages)
                    .children(working),
            )
            .child(
                h_flex()
                    .flex_none()
                    .gap_1()
                    .pl_1()
                    .pr_2()
                    .pb_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Input::new(&self.input).appearance(false)),
                    )
                    .child(
                        Button::new("send")
                            .small()
                            .icon(IconName::ArrowUp)
                            .accessibility_label("Send reply")
                            .disabled(!can_send)
                            .on_click(cx.listener(|this, _, window, cx| this.send(window, cx))),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_read_in_the_nearest_unit() {
        assert_eq!(ago(Duration::from_secs(20)), "just now");
        assert_eq!(ago(Duration::from_secs(170)), "3 min ago");
        assert_eq!(ago(Duration::from_secs(2 * 3600)), "2 h ago");
        assert_eq!(ago(Duration::from_secs(3 * 86_400)), "3 d ago");
        assert_eq!(ago(Duration::from_secs(61 * 86_400)), "2 mo ago");
        assert_eq!(ago(Duration::from_secs(800 * 86_400)), "2 y ago");
    }

    #[test]
    fn a_leading_command_is_split_off() {
        assert_eq!(
            split_command("/edit rename the column"),
            (Some("/edit"), "rename the column")
        );
        assert_eq!(split_command("  /undo"), (Some("/undo"), ""));
    }

    #[test]
    fn ordinary_text_has_no_command() {
        assert_eq!(split_command("see /docs later"), (None, "see /docs later"));
        assert_eq!(split_command("/ alone"), (None, "/ alone"));
    }
}
