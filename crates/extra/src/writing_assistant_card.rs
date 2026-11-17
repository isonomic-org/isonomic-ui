// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Writing assistant card: a small helper for rewriting and drafting text.
//!
//! The card opens on a welcome line with suggestions over a faint glow.
//! Sending a suggestion or a typed message turns it into a short chat: the
//! message shows on the right, a "Writing…" line holds the place, then the
//! reply streams in word by word. The mic dictates into the field and sends
//! the finished sentence on its own.

use std::{rc::Rc, time::Duration};

use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{InputEvent, Textarea, TextareaState},
    v_flex,
};
use isonomic_ui::{
    Animation, AnimationExt as _, App, AppContext as _, Context, Entity, EventEmitter, FocusHandle,
    Focusable, InteractiveElement as _, IntoElement, ParentElement as _, Render, Role,
    SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Task, Window, div,
    ease_in_out, linear_color_stop, linear_gradient, prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// What happened in the card.
#[derive(Debug, Clone)]
pub enum WritingAssistantEvent {
    /// A message was sent, typed, picked or dictated.
    Sent(SharedString),
    /// A reply finished streaming in.
    Replied(SharedString),
    /// The chat was cleared with New chat.
    NewChat,
    /// The mic started or stopped listening. Start your own recogniser here
    /// and pass what it hears to [`WritingAssistantCard::hear`].
    Listening(bool),
}

type Responder = Rc<dyn Fn(&str) -> String>;

/// One turn of the chat.
#[derive(Clone)]
struct Turn {
    mine: bool,
    text: SharedString,
}

/// A reply for when the owner gives no way to answer: it reads the request
/// for a few common asks and writes a short sample.
fn sample_reply(message: &str) -> String {
    let asked = message.to_lowercase();
    if asked.contains("short") {
        "Here's a tighter version: \"Thanks for sending the figures so quickly. They look right, so let's close the month on Friday.\"".into()
    } else if asked.contains("tone") {
        "A warmer take: \"Thanks for your patience while we checked the numbers. Everything is in order now, and I'm happy to walk you through it.\"".into()
    } else if asked.contains("reply") {
        "Here's a reply you can send: \"Thanks for the reminder. I'll have the report with you by Friday afternoon.\"".into()
    } else {
        format!(
            "Here's a first draft for \"{}\". Tell me what to change and I'll rework it.",
            message.trim()
        )
    }
}

/// The words of a reply streamed so far.
fn streamed(text: &str, words: usize) -> &str {
    let mut end = 0;
    for (count, word) in text.split_inclusive(' ').enumerate() {
        if count == words {
            break;
        }
        end += word.len();
    }
    &text[..end]
}

/// Where the card is.
#[derive(Clone, Copy, PartialEq)]
enum State {
    Idle,
    /// Waiting for the reply to start.
    Thinking,
    /// The reply is streaming in: this many words are showing.
    Replying(usize),
}

/// A small chat card for rewriting and drafting text.
pub struct WritingAssistantCard {
    name: SharedString,
    tagline: SharedString,
    greeting: SharedString,
    suggestions: Vec<SharedString>,
    respond: Responder,
    dictation: Option<SharedString>,
    turns: Vec<Turn>,
    state: State,
    reply: SharedString,
    listening: bool,
    field: Entity<TextareaState>,
    focus: FocusHandle,
    _worker: Option<Task<()>>,
    _listener: Option<Task<()>>,
    _field_events: Subscription,
}

impl EventEmitter<WritingAssistantEvent> for WritingAssistantCard {}

impl Focusable for WritingAssistantCard {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl WritingAssistantCard {
    /// Create the card.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let field = cx.new(|cx| {
            let mut field = TextareaState::new(window, cx)
                .auto_grow(1, 4)
                .placeholder("Ask anything\u{2026}");
            field.set_submit_on_enter(true, cx);
            field
        });
        let field_events =
            cx.subscribe_in(&field, window, |this, _, event, window, cx| match event {
                InputEvent::PressEnter { shift: false, .. } => this.send_field(window, cx),
                InputEvent::Change => cx.notify(),
                _ => {}
            });
        Self {
            name: "Writer".into(),
            tagline: "Helps you write.".into(),
            greeting: "What should we write?".into(),
            suggestions: Vec::new(),
            respond: Rc::new(sample_reply),
            dictation: Some("Write a short note that the payroll file is ready for review".into()),
            turns: Vec::new(),
            state: State::Idle,
            reply: SharedString::default(),
            listening: false,
            field,
            focus: cx.focus_handle(),
            _worker: None,
            _listener: None,
            _field_events: field_events,
        }
    }

    /// Name the assistant.
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = name.into();
        self
    }

    /// Set the line under the name.
    pub fn tagline(mut self, tagline: impl Into<SharedString>) -> Self {
        self.tagline = tagline.into();
        self
    }

    /// Set the welcome line.
    pub fn greeting(mut self, greeting: impl Into<SharedString>) -> Self {
        self.greeting = greeting.into();
        self
    }

    /// Offer a suggestion that sends itself as a message.
    pub fn suggestion(mut self, suggestion: impl Into<SharedString>) -> Self {
        self.suggestions.push(suggestion.into());
        self
    }

    /// Answer messages your way. A sample writer answers otherwise.
    pub fn respond(mut self, respond: impl Fn(&str) -> String + 'static) -> Self {
        self.respond = Rc::new(respond);
        self
    }

    /// Set the sentence the mic types out when it simulates dictation, or
    /// `None` to feed it yourself through [`WritingAssistantCard::hear`].
    pub fn dictation(mut self, sentence: Option<SharedString>) -> Self {
        self.dictation = sentence;
        self
    }

    /// Show what a recogniser heard in the field. When `finished`, listening
    /// stops and the sentence is sent.
    pub fn hear(
        &mut self,
        text: &str,
        finished: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.listening {
            return;
        }
        let text = text.to_string();
        self.field
            .update(cx, |field, cx| field.set_value(text, window, cx));
        if finished {
            self.stop_listening(window, cx);
        }
        cx.notify();
    }

    /// Send a message as if it had been typed.
    pub fn send(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        let message: SharedString = message.into();
        if message.trim().is_empty() || self.state != State::Idle {
            return;
        }
        self.turns.push(Turn {
            mine: true,
            text: message.clone(),
        });
        cx.emit(WritingAssistantEvent::Sent(message.clone()));
        let reply: SharedString = (self.respond)(&message).into();
        self.reply = reply.clone();
        self.state = State::Thinking;
        cx.notify();
        let words = reply.split_inclusive(' ').count();
        self._worker = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(900))
                .await;
            for shown in 1..=words {
                let alive = this
                    .update(cx, |this, cx| {
                        this.state = State::Replying(shown);
                        cx.notify();
                    })
                    .is_ok();
                if !alive {
                    return;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(45))
                    .await;
            }
            this.update(cx, |this, cx| {
                this.turns.push(Turn {
                    mine: false,
                    text: reply.clone(),
                });
                this.state = State::Idle;
                cx.emit(WritingAssistantEvent::Replied(reply.clone()));
                cx.notify();
            })
            .ok();
        }));
    }

    /// Clear the chat and go back to the welcome.
    pub fn new_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self._worker = None;
        self._listener = None;
        self.listening = false;
        self.turns.clear();
        self.state = State::Idle;
        self.field
            .update(cx, |field, cx| field.set_value("", window, cx));
        cx.emit(WritingAssistantEvent::NewChat);
        cx.notify();
    }

    fn send_field(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.field.read(cx).value().trim().to_string();
        if text.is_empty() || self.state != State::Idle {
            return;
        }
        self.field
            .update(cx, |field, cx| field.set_value("", window, cx));
        self.send(text, cx);
    }

    fn toggle_mic(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.listening {
            self.stop_listening(window, cx);
            return;
        }
        self.listening = true;
        self.field
            .update(cx, |field, cx| field.set_value("", window, cx));
        cx.emit(WritingAssistantEvent::Listening(true));
        cx.notify();
        let Some(sentence) = self.dictation.clone() else {
            return;
        };
        // A simulated take: words arrive one by one, then a pause ends it.
        let window_handle = window.window_handle();
        self._listener = Some(cx.spawn(async move |this, cx| {
            let words: Vec<&str> = sentence.split_inclusive(' ').collect();
            let mut heard = String::new();
            for word in words {
                cx.background_executor()
                    .timer(Duration::from_millis(220))
                    .await;
                heard.push_str(word);
                let text = heard.clone();
                let alive = cx
                    .update_window(window_handle, |_, window, cx| {
                        this.update(cx, |this, cx| {
                            this.field
                                .update(cx, |field, cx| field.set_value(text, window, cx));
                            cx.notify();
                        })
                        .is_ok()
                    })
                    .unwrap_or(false);
                if !alive {
                    return;
                }
            }
            cx.background_executor()
                .timer(Duration::from_millis(700))
                .await;
            cx.update_window(window_handle, |_, window, cx| {
                this.update(cx, |this, cx| this.stop_listening(window, cx))
                    .ok();
            })
            .ok();
        }));
    }

    fn stop_listening(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.listening {
            return;
        }
        self.listening = false;
        self._listener = None;
        cx.emit(WritingAssistantEvent::Listening(false));
        self.send_field(window, cx);
        cx.notify();
    }
}

impl Render for WritingAssistantCard {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.magenta;
        let listen = theme.blue;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;

        let chatting = !self.turns.is_empty() || self.state != State::Idle;
        let typed = !self.field.read(cx).value().trim().is_empty();
        let busy = self.state != State::Idle;

        let glow = div()
            .absolute()
            .inset_0()
            .bg(linear_gradient(
                180.,
                linear_color_stop(fill.opacity(0.), 0.35),
                linear_color_stop(accent.opacity(if chatting { 0.04 } else { 0.10 }), 1.),
            ))
            .child(div().absolute().inset_0().bg(linear_gradient(
                135.,
                linear_color_stop(fill.opacity(0.), 0.5),
                linear_color_stop(listen.opacity(if chatting { 0.03 } else { 0.08 }), 1.),
            )));

        let welcome = (!chatting).then(|| {
            v_flex()
                .gap_3()
                .child(
                    div()
                        .text_base()
                        .font_semibold()
                        .child(self.greeting.clone()),
                )
                .child(h_flex().flex_wrap().gap_1p5().children(
                    self.suggestions.iter().enumerate().map(|(ix, suggestion)| {
                        let text = suggestion.clone();
                        Button::new(("assistant-suggestion", ix))
                            .outline()
                            .small()
                            .label(suggestion.clone())
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.send(text.clone(), cx)),
                            )
                    }),
                ))
        });

        let bubble = |turn: &Turn| {
            if turn.mine {
                h_flex().justify_end().child(
                    div()
                        .max_w(relative(0.8))
                        .px_2p5()
                        .py_1()
                        .rounded(radius_lg)
                        .bg(chip)
                        .child(turn.text.clone()),
                )
            } else {
                h_flex().child(div().flex_1().min_w_0().child(turn.text.clone()))
            }
        };
        let pending = match self.state {
            State::Idle => None,
            State::Thinking => Some(
                h_flex()
                    .gap_1p5()
                    .text_color(quiet)
                    .child(
                        div().size(px(6.)).rounded_full().bg(accent).with_animation(
                            "assistant-thinking",
                            Animation::new(Duration::from_millis(900))
                                .repeat()
                                .with_easing(ease_in_out),
                            |this, delta| this.opacity(0.3 + 0.7 * (1. - (delta * 2. - 1.).abs())),
                        ),
                    )
                    .child("Writing\u{2026}")
                    .into_any_element(),
            ),
            State::Replying(words) => Some(
                div()
                    .child(streamed(&self.reply, words).to_string())
                    .into_any_element(),
            ),
        };
        let chat = chatting.then(|| {
            v_flex()
                .id("assistant-chat")
                .flex_1()
                .justify_end()
                .gap_3()
                .overflow_y_scroll()
                .children(self.turns.iter().map(bubble))
                .children(pending)
        });

        let bars = (0..3usize).map(|bar| {
            div()
                .w(px(2.))
                .h(px(10.))
                .rounded_full()
                .bg(listen)
                .with_animation(
                    ("assistant-bar", bar),
                    Animation::new(Duration::from_millis(700 + bar as u64 * 140))
                        .repeat()
                        .with_easing(ease_in_out),
                    |this, delta| this.h(px(4. + 9. * (1. - (delta * 2. - 1.).abs()))),
                )
        });
        let mic = if self.listening {
            h_flex()
                .id("assistant-mic")
                .role(Role::Button)
                .aria_label("Stop dictating")
                .flex_none()
                .size(px(28.))
                .justify_center()
                .gap(px(2.))
                .rounded(radius)
                .bg(listen.opacity(0.14))
                .border_1()
                .border_color(listen.opacity(0.4))
                .cursor_pointer()
                .on_click(cx.listener(|this, _, window, cx| this.toggle_mic(window, cx)))
                .children(bars)
                .into_any_element()
        } else {
            Button::new("assistant-mic")
                .ghost()
                .small()
                .icon(IconName::Mic)
                .tooltip("Dictate")
                .accessibility_label("Dictate")
                .disabled(busy)
                .on_click(cx.listener(|this, _, window, cx| this.toggle_mic(window, cx)))
                .into_any_element()
        };

        v_flex()
            .track_focus(&self.focus)
            .relative()
            .w_full()
            .max_w(px(380.))
            .h(px(420.))
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .line_height(relative(1.5))
            .overflow_hidden()
            .shadow(resting_shadow(cx))
            .child(glow)
            .child(
                h_flex()
                    .gap_2()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(line)
                    .bg(fill)
                    .child(
                        v_flex()
                            .flex_1()
                            .child(
                                h_flex()
                                    .gap_1p5()
                                    .child(Icon::new(IconName::Star).xsmall().text_color(accent))
                                    .child(div().font_semibold().child(self.name.clone())),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(quiet)
                                    .child(self.tagline.clone()),
                            ),
                    )
                    .when(chatting, |this| {
                        this.child(
                            Button::new("assistant-new")
                                .ghost()
                                .small()
                                .icon(IconName::Plus)
                                .tooltip("New chat")
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.new_chat(window, cx)),
                                ),
                        )
                    }),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .justify_end()
                    .px_4()
                    .pb_3()
                    .children(welcome)
                    .children(chat),
            )
            .child(
                h_flex()
                    .items_end()
                    .gap_1()
                    .px_3()
                    .py_2()
                    .border_t_1()
                    .border_color(line)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Textarea::new(&self.field).appearance(false)),
                    )
                    .child(mic)
                    .when(typed && !self.listening, |this| {
                        this.child(
                            Button::new("assistant-send")
                                .primary()
                                .small()
                                .icon(IconName::ArrowUp)
                                .tooltip("Send")
                                .disabled(busy)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.send_field(window, cx)),
                                ),
                        )
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies_stream_word_by_word() {
        assert_eq!(streamed("Thanks for the note", 0), "");
        assert_eq!(streamed("Thanks for the note", 2), "Thanks for ");
        assert_eq!(streamed("Thanks for the note", 9), "Thanks for the note");
    }

    #[test]
    fn the_sample_writer_reads_the_ask() {
        assert!(sample_reply("Make it shorter").starts_with("Here's a tighter"));
        assert!(sample_reply("Fix the tone").starts_with("A warmer"));
        assert!(sample_reply("Draft a reply").contains("reply you can send"));
        assert!(sample_reply("A thank-you note").contains("A thank-you note"));
    }
}
