// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Writing assistant card: a small helper for rewriting and drafting text.
//!
//! The card opens on a welcome line with suggestions over a faint glow.
//! Sending a suggestion or a typed message turns it into a short chat: the
//! message shows on the right, a "Writing…" line holds the place, then the
//! reply streams in word by word. The mic dictates into the field and sends
//! the finished sentence on its own.
//!
//! Opt-in tone rewrite: the field holds the text to rewrite, and above it
//! sit a model switch, a length switch (shorter, same, longer) and a
//! two-axis tone pad, casual to formal across and direct to warm up. Drag
//! or click the pad, or use the arrow keys on it (Home recentres). Rewrite
//! sends the request; the result streams in like a reply, with Use this to
//! carry it back into the field for another pass.

use std::{cell::Cell, rc::Rc, time::Duration};

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{InputEvent, Textarea, TextareaState},
    tab::{Tab, TabBar},
    v_flex,
};
use isonomic_ui::{
    Animation, AnimationExt as _, App, AppContext as _, Bounds, Context, Entity, EventEmitter,
    FocusHandle, Focusable, InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, PathBuilder, Pixels, Point,
    Render, Role, SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Task,
    Window, canvas, div, ease_in_out, linear_color_stop, linear_gradient, point,
    prelude::FluentBuilder as _, px, relative,
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
type Rewriter = Rc<dyn Fn(&ToneRequest) -> String>;

/// How long a rewrite should come out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RewriteLength {
    /// Tighter than the text.
    Shorter,
    /// About as long.
    #[default]
    Same,
    /// Fuller than the text.
    Longer,
}

impl RewriteLength {
    /// All three, in switch order.
    pub const EVERY: [RewriteLength; 3] = [
        RewriteLength::Shorter,
        RewriteLength::Same,
        RewriteLength::Longer,
    ];

    /// Its name on the switch.
    pub fn words(self) -> &'static str {
        match self {
            RewriteLength::Shorter => "Shorter",
            RewriteLength::Same => "Same length",
            RewriteLength::Longer => "Longer",
        }
    }
}

/// A point on the tone pad: `formality` runs from casual (-1) to formal
/// (1), `warmth` from direct (-1) to warm (1).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Tone {
    /// Casual (-1) to formal (1).
    pub formality: f32,
    /// Direct (-1) to warm (1).
    pub warmth: f32,
}

impl Tone {
    /// Create a tone, each axis kept within -1 to 1.
    pub fn new(formality: f32, warmth: f32) -> Self {
        Self {
            formality: formality.clamp(-1., 1.),
            warmth: warmth.clamp(-1., 1.),
        }
    }
}

/// What a rewrite is asked for.
#[derive(Debug, Clone, PartialEq)]
pub struct ToneRequest {
    /// The text to rewrite.
    pub text: SharedString,
    /// The model picked to write it.
    pub model: SharedString,
    /// How long it should come out.
    pub length: RewriteLength,
    /// Where it should sit on the tone pad.
    pub tone: Tone,
}

/// The tone in words, as the pad's spoken value and the request summary:
/// "neutral", "formal and warm", "slightly casual".
pub fn tone_words(tone: Tone) -> String {
    fn axis(value: f32, low: &str, high: &str) -> Option<String> {
        let word = if value < 0. { low } else { high };
        match value.abs() {
            v if v < 0.15 => None,
            v if v < 0.55 => Some(format!("slightly {word}")),
            _ => Some(word.to_string()),
        }
    }
    match (
        axis(tone.formality, "casual", "formal"),
        axis(tone.warmth, "direct", "warm"),
    ) {
        (None, None) => "neutral".into(),
        (Some(a), None) | (None, Some(a)) => a,
        (Some(a), Some(b)) => format!("{a} and {b}"),
    }
}

/// Move the tone with a key on the pad: arrows step a quarter, Shift with
/// an arrow a tenth, Home recentres. `None` for any other key.
pub fn tone_step(tone: Tone, key: &str, fine: bool) -> Option<Tone> {
    let step = if fine { 0.1 } else { 0.25 };
    let snap = |v: f32| (v / step).round() * step;
    Some(match key {
        "left" => Tone::new(snap(tone.formality - step), tone.warmth),
        "right" => Tone::new(snap(tone.formality + step), tone.warmth),
        "up" => Tone::new(tone.formality, snap(tone.warmth + step)),
        "down" => Tone::new(tone.formality, snap(tone.warmth - step)),
        "home" => Tone::default(),
        _ => return None,
    })
}

/// The tone at a point inside the pad: left edge casual, top edge warm.
pub fn tone_at(x: f32, y: f32, width: f32, height: f32) -> Tone {
    let fx = (x / width.max(1.)).clamp(0., 1.);
    let fy = (y / height.max(1.)).clamp(0., 1.);
    Tone::new(fx * 2. - 1., 1. - fy * 2.)
}

/// The line that stands for a rewrite request in the chat.
pub fn request_words(request: &ToneRequest) -> String {
    let length = match request.length {
        RewriteLength::Shorter => ", shorter",
        RewriteLength::Same => "",
        RewriteLength::Longer => ", longer",
    };
    format!(
        "Rewrite {}{length}, with {}",
        tone_words(request.tone),
        request.model
    )
}

/// A rewrite for when the owner gives no writer: it reworks the text's
/// greeting and close to fit the tone and trims or pads it for length.
fn sample_rewrite(request: &ToneRequest) -> String {
    let text = request.text.trim().trim_end_matches('.');
    let (open, close) = match (request.tone.formality >= 0., request.tone.warmth >= 0.) {
        (true, true) => (
            "Thank you for your patience.",
            "I would be glad to help further.",
        ),
        (true, false) => ("Please note:", "Kind regards."),
        (false, true) => ("Thanks so much!", "Shout if you need anything!"),
        (false, false) => ("Quick note:", "Thanks."),
    };
    let body = match request.length {
        RewriteLength::Shorter => text
            .split_inclusive([',', ';'])
            .next()
            .unwrap_or(text)
            .trim_end_matches([',', ';'])
            .to_string(),
        RewriteLength::Same => text.to_string(),
        RewriteLength::Longer => format!("{text}, and I have set out the details below"),
    };
    format!("{open} {body}. {close}")
}

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

/// The tone rewrite controls, once switched on.
struct ToneControls {
    models: Vec<SharedString>,
    model: usize,
    length: RewriteLength,
    tone: Tone,
    rewrite: Rewriter,
    pad: Rc<Cell<Option<Bounds<Pixels>>>>,
    pad_focus: FocusHandle,
    dragging: bool,
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
    tone: Option<ToneControls>,
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
            tone: None,
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

    /// Turn on tone rewrite with these models to pick from: the field holds
    /// the text, and Rewrite sends it with the model, length and tone.
    pub fn tone_rewrite(
        mut self,
        models: impl IntoIterator<Item = impl Into<SharedString>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut models: Vec<SharedString> = models.into_iter().map(Into::into).collect();
        if models.is_empty() {
            models.push("Default".into());
        }
        self.tone = Some(ToneControls {
            models,
            model: 0,
            length: RewriteLength::default(),
            tone: Tone::default(),
            rewrite: Rc::new(sample_rewrite),
            pad: Rc::new(Cell::new(None)),
            pad_focus: cx.focus_handle().tab_stop(true),
            dragging: false,
        });
        self.field.update(cx, |field, cx| {
            field.set_placeholder("Paste or type the text to rewrite\u{2026}", window, cx)
        });
        self
    }

    /// Rewrite your way. A sample rewriter answers otherwise.
    pub fn rewrite_with(mut self, rewrite: impl Fn(&ToneRequest) -> String + 'static) -> Self {
        if let Some(tone) = &mut self.tone {
            tone.rewrite = Rc::new(rewrite);
        }
        self
    }

    /// Set where the tone pad starts.
    pub fn tone(mut self, tone: Tone) -> Self {
        if let Some(controls) = &mut self.tone {
            controls.tone = tone;
        }
        self
    }

    /// Set the length a rewrite asks for at first.
    pub fn length(mut self, length: RewriteLength) -> Self {
        if let Some(controls) = &mut self.tone {
            controls.length = length;
        }
        self
    }

    /// The current rewrite request for what's in the field, when tone
    /// rewrite is on.
    pub fn tone_request(&self, cx: &App) -> Option<ToneRequest> {
        let tone = self.tone.as_ref()?;
        Some(ToneRequest {
            text: self.field.read(cx).value().trim().to_string().into(),
            model: tone.models[tone.model.min(tone.models.len() - 1)].clone(),
            length: tone.length,
            tone: tone.tone,
        })
    }

    fn set_tone_from(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(controls) = &mut self.tone else {
            return;
        };
        let Some(bounds) = controls.pad.get() else {
            return;
        };
        controls.tone = tone_at(
            f32::from(at.x - bounds.left()),
            f32::from(at.y - bounds.top()),
            f32::from(bounds.size.width),
            f32::from(bounds.size.height),
        );
        cx.notify();
    }

    fn send_rewrite(&mut self, cx: &mut Context<Self>) {
        let Some(request) = self.tone_request(cx) else {
            return;
        };
        if request.text.is_empty() || self.state != State::Idle {
            return;
        }
        let reply = self
            .tone
            .as_ref()
            .map(|t| (t.rewrite)(&request))
            .unwrap_or_default();
        self.start(request_words(&request).into(), reply.into(), cx);
    }

    /// Put text in the field, such as the message to rewrite.
    pub fn set_text(
        &mut self,
        text: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text: SharedString = text.into();
        self.field
            .update(cx, |field, cx| field.set_value(text, window, cx));
        cx.notify();
    }

    fn use_reply(&mut self, text: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        self.field
            .update(cx, |field, cx| field.set_value(text, window, cx));
        cx.notify();
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
        let reply: SharedString = (self.respond)(&message).into();
        self.start(message, reply, cx);
    }

    fn start(&mut self, message: SharedString, reply: SharedString, cx: &mut Context<Self>) {
        self.turns.push(Turn {
            mine: true,
            text: message.clone(),
        });
        cx.emit(WritingAssistantEvent::Sent(message));
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
        if self.tone.is_some() {
            // The field holds the text to rewrite: it stays for another pass.
            self.send_rewrite(cx);
            return;
        }
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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

        let ring = theme.ring;
        let rewriting = self.tone.is_some();
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
                .child(div().text_base().font_medium().child(self.greeting.clone()))
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

        let last_reply = self
            .turns
            .iter()
            .rposition(|t| !t.mine)
            .filter(|_| rewriting && self.state == State::Idle);
        let bubble = |(ix, turn): (usize, &Turn)| {
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
            } else if last_reply == Some(ix) {
                let text = turn.text.clone();
                h_flex().child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_1()
                        .child(turn.text.clone())
                        .child(
                            h_flex().child(
                                Button::new("assistant-use")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Replace)
                                    .label("Use this")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.use_reply(text.clone(), window, cx)
                                    })),
                            ),
                        ),
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
                .children(self.turns.iter().enumerate().map(bubble))
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

        let controls = self.tone.as_ref().map(|tone| {
            let pad = tone.pad.clone();
            let at = tone.tone;
            let focused = tone.pad_focus.is_focused(window);
            let grid = canvas(
                move |bounds, _, _| pad.set(Some(bounds)),
                move |bounds, _, window, _| {
                    let w = f32::from(bounds.size.width);
                    let h = f32::from(bounds.size.height);
                    let at = |x: f32, y: f32| point(bounds.left() + px(x), bounds.top() + px(y));
                    for (from, to) in [
                        (at(w / 2., 4.), at(w / 2., h - 4.)),
                        (at(4., h / 2.), at(w - 4., h / 2.)),
                    ] {
                        let mut path = PathBuilder::stroke(px(1.)).dash_array(&[px(2.), px(3.)]);
                        path.move_to(from);
                        path.line_to(to);
                        if let Ok(path) = path.build() {
                            window.paint_path(path, line);
                        }
                    }
                },
            )
            .absolute()
            .inset_0();
            let knob_x = (at.formality + 1.) / 2.;
            let knob_y = (1. - at.warmth) / 2.;
            let axis_label = |text: &'static str| {
                div()
                    .absolute()
                    .text_size(px(10.))
                    .line_height(px(16.))
                    .text_color(quiet)
                    .child(text)
            };
            let model_names = tone.models.clone();
            v_flex()
                .gap_2()
                .px_3()
                .pt_2()
                .child(
                    h_flex()
                        .gap_2()
                        .flex_wrap()
                        .child(
                            TabBar::new("assistant-model")
                                .segmented()
                                .small()
                                .selected_index(tone.model)
                                .on_click(cx.listener(|this, ix: &usize, _, cx| {
                                    if let Some(tone) = &mut this.tone {
                                        tone.model = (*ix).min(tone.models.len() - 1);
                                    }
                                    cx.notify();
                                }))
                                .children(model_names.iter().map(|m| Tab::new().label(m.clone()))),
                        )
                        .child(
                            TabBar::new("assistant-length")
                                .segmented()
                                .small()
                                .selected_index(
                                    RewriteLength::EVERY
                                        .iter()
                                        .position(|l| *l == tone.length)
                                        .unwrap_or(1),
                                )
                                .on_click(cx.listener(|this, ix: &usize, _, cx| {
                                    if let Some(tone) = &mut this.tone {
                                        tone.length = RewriteLength::EVERY[(*ix).min(2)];
                                    }
                                    cx.notify();
                                }))
                                .children(
                                    RewriteLength::EVERY
                                        .iter()
                                        .map(|l| Tab::new().label(l.words())),
                                ),
                        ),
                )
                .child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .child(
                            div()
                                .id("assistant-tone")
                                .track_focus(&tone.pad_focus)
                                .relative()
                                .flex_none()
                                .w(px(176.))
                                .h(px(96.))
                                .rounded(radius)
                                .bg(chip)
                                .border_1()
                                .border_color(if focused { ring } else { line })
                                .cursor_crosshair()
                                .role(Role::Slider)
                                .aria_label(format!(
                                    "Tone, {}. Arrows move it, Home recentres",
                                    tone_words(at)
                                ))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, event: &MouseDownEvent, window, cx| {
                                        if let Some(tone) = &mut this.tone {
                                            tone.dragging = true;
                                            tone.pad_focus.focus(window, cx);
                                        }
                                        this.set_tone_from(event.position, cx);
                                    }),
                                )
                                .on_mouse_move(cx.listener(
                                    |this, event: &MouseMoveEvent, _, cx| {
                                        let dragging =
                                            this.tone.as_ref().is_some_and(|t| t.dragging);
                                        if dragging
                                            && event.pressed_button == Some(MouseButton::Left)
                                        {
                                            this.set_tone_from(event.position, cx);
                                        }
                                    },
                                ))
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(|this, _: &MouseUpEvent, _, cx| {
                                        if let Some(tone) = &mut this.tone {
                                            tone.dragging = false;
                                        }
                                        cx.notify();
                                    }),
                                )
                                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                                    let Some(tone) = &mut this.tone else { return };
                                    let fine = event.keystroke.modifiers.shift;
                                    if let Some(next) =
                                        tone_step(tone.tone, event.keystroke.key.as_str(), fine)
                                    {
                                        tone.tone = next;
                                        cx.stop_propagation();
                                        cx.notify();
                                    }
                                }))
                                .child(grid)
                                .child(axis_label("Warm").top(px(1.)).left(px(6.)))
                                .child(axis_label("Direct").bottom(px(1.)).left(px(6.)))
                                .child(
                                    div()
                                        .absolute()
                                        .left(relative(knob_x))
                                        .top(relative(knob_y))
                                        .ml(px(-7.))
                                        .mt(px(-7.))
                                        .size(px(14.))
                                        .rounded_full()
                                        .bg(accent)
                                        .border_2()
                                        .border_color(fill),
                                ),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap_1()
                                .text_caption()
                                .child(
                                    h_flex()
                                        .text_size(px(10.))
                                        .line_height(px(16.))
                                        .text_color(quiet)
                                        .child(div().flex_1().child("\u{2190} Casual"))
                                        .child("Formal \u{2192}"),
                                )
                                .child(div().font_medium().child(tone_words(at)))
                                .child(div().text_color(quiet).child(format!(
                                    "{:+.2} formal, {:+.2} warm",
                                    at.formality, at.warmth
                                ))),
                        ),
                )
        });

        v_flex()
            .track_focus(&self.focus)
            .relative()
            .w_full()
            .max_w(px(380.))
            .h(px(if rewriting { 560. } else { 420. }))
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
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
                                    .child(div().font_medium().child(self.name.clone())),
                            )
                            .child(
                                div()
                                    .text_caption()
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
            .children(controls.map(|c| c.border_t_1().border_color(line).bg(fill)))
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
                    .when(typed && !self.listening && rewriting, |this| {
                        this.child(
                            Button::new("assistant-rewrite")
                                .primary()
                                .small()
                                .icon(IconName::Replace)
                                .label("Rewrite")
                                .disabled(busy)
                                .on_click(cx.listener(|this, _, _, cx| this.send_rewrite(cx))),
                        )
                    })
                    .when(typed && !self.listening && !rewriting, |this| {
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
    fn tone_reads_in_words() {
        assert_eq!(tone_words(Tone::default()), "neutral");
        assert_eq!(tone_words(Tone::new(0.8, 0.9)), "formal and warm");
        assert_eq!(tone_words(Tone::new(-0.3, 0.)), "slightly casual");
        assert_eq!(
            tone_words(Tone::new(0.1, -0.7)),
            "direct",
            "a small lean on one axis is not said"
        );
    }

    #[test]
    fn arrows_step_the_pad_and_stay_inside() {
        let t = tone_step(Tone::default(), "right", false).unwrap();
        assert_eq!(t, Tone::new(0.25, 0.));
        let t = tone_step(t, "up", true).unwrap();
        assert!((t.warmth - 0.1).abs() < 1e-6);
        let edge = tone_step(Tone::new(1., -1.), "right", false).unwrap();
        assert_eq!(edge, Tone::new(1., -1.));
        let edge = tone_step(edge, "down", false).unwrap();
        assert_eq!(edge.warmth, -1.);
        assert_eq!(
            tone_step(Tone::new(0.5, 0.5), "home", false),
            Some(Tone::default())
        );
        assert_eq!(tone_step(Tone::default(), "a", false), None);
    }

    #[test]
    fn a_point_on_the_pad_maps_to_a_tone() {
        assert_eq!(tone_at(0., 0., 200., 100.), Tone::new(-1., 1.));
        assert_eq!(tone_at(100., 50., 200., 100.), Tone::new(0., 0.));
        assert_eq!(tone_at(400., 300., 200., 100.), Tone::new(1., -1.));
    }

    #[test]
    fn requests_summarise_and_the_sample_rewrites() {
        let request = ToneRequest {
            text: "The report is ready, please review it by Friday.".into(),
            model: "Careful".into(),
            length: RewriteLength::Shorter,
            tone: Tone::new(0.9, 0.9),
        };
        assert_eq!(
            request_words(&request),
            "Rewrite formal and warm, shorter, with Careful"
        );
        assert_eq!(
            sample_rewrite(&request),
            "Thank you for your patience. The report is ready. I would be glad to help further."
        );
        let longer = ToneRequest {
            length: RewriteLength::Longer,
            tone: Tone::new(-1., -1.),
            ..request
        };
        assert!(sample_rewrite(&longer).starts_with("Quick note: The report is ready, please"));
    }

    #[test]
    fn the_sample_writer_reads_the_ask() {
        assert!(sample_reply("Make it shorter").starts_with("Here's a tighter"));
        assert!(sample_reply("Fix the tone").starts_with("A warmer"));
        assert!(sample_reply("Draft a reply").contains("reply you can send"));
        assert!(sample_reply("A thank-you note").contains("A thank-you note"));
    }
}
