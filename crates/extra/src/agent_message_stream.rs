//! Agent message stream: a live log of what assistants say to each other, with a composer that can mention them.
//!
//! Each message shows who sent it, who it was for, when, and what it said,
//! with mentions picked out. Status lines from the system sit quietly
//! between them. A filter narrows the log to one participant. The composer
//! at the foot sends a note into the stream; typing @ opens a picker of
//! participants, and Up, Down and Enter choose one.

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{InputEvent, Textarea, TextareaState},
    v_flex,
};
use isonomic_ui::{
    App, AppContext as _, Context, ElementId, Entity, EventEmitter, FocusHandle, Focusable,
    HighlightStyle, InteractiveElement as _, IntoElement, ParentElement as _, Render, Role,
    SharedString, StatefulInteractiveElement as _, Styled as _, StyledText, Subscription, Window,
    div, prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// Who a participant is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticipantKind {
    /// An assistant.
    Agent,
    /// A person.
    Person,
}

/// Someone in the stream.
#[derive(Debug, Clone, PartialEq)]
pub struct Participant {
    /// Identifies them.
    pub id: SharedString,
    /// Their name as mentioned, without spaces, such as "Reconciler".
    pub handle: SharedString,
    /// What they are.
    pub kind: ParticipantKind,
}

impl Participant {
    /// Create a participant.
    pub fn new(
        id: impl Into<SharedString>,
        handle: impl Into<SharedString>,
        kind: ParticipantKind,
    ) -> Self {
        Self {
            id: id.into(),
            handle: handle.into(),
            kind,
        }
    }
}

/// One line of the stream.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamMessage {
    /// Identifies it.
    pub id: SharedString,
    /// Who sent it; `None` for a system status line.
    pub from: Option<SharedString>,
    /// Who it was for, if anyone in particular.
    pub to: Option<SharedString>,
    /// What it said.
    pub text: SharedString,
    /// When, as written.
    pub when: SharedString,
}

impl StreamMessage {
    /// Create a message from a participant.
    pub fn new(
        id: impl Into<SharedString>,
        from: impl Into<SharedString>,
        text: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            from: Some(from.into()),
            to: None,
            text: text.into(),
            when: SharedString::default(),
        }
    }

    /// Create a system status line.
    pub fn status(id: impl Into<SharedString>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            from: None,
            to: None,
            text: text.into(),
            when: SharedString::default(),
        }
    }

    /// Say who it was for.
    pub fn to(mut self, to: impl Into<SharedString>) -> Self {
        self.to = Some(to.into());
        self
    }

    /// Say when.
    pub fn when(mut self, when: impl Into<SharedString>) -> Self {
        self.when = when.into();
        self
    }
}

/// What the stream reports.
#[derive(Debug, Clone, PartialEq)]
pub enum MessageStreamEvent {
    /// A note was sent, with the participants it mentions.
    Send {
        /// The text.
        text: SharedString,
        /// Ids of the participants mentioned.
        mentions: Vec<SharedString>,
    },
    /// The filter changed; `None` shows everyone.
    Filter(Option<SharedString>),
}

/// The partial mention being typed at the end of `text`, if any: the
/// letters after the last `@`, with no space since.
pub fn typing_mention(text: &str) -> Option<&str> {
    let at = text.rfind('@')?;
    let before_ok = at == 0 || text[..at].ends_with(char::is_whitespace);
    let after = &text[at + 1..];
    (before_ok && !after.contains(char::is_whitespace)).then_some(after)
}

/// The participants whose handle starts with what's typed, ignoring case.
pub fn mention_matches<'a>(people: &'a [Participant], typed: &str) -> Vec<&'a Participant> {
    let typed = typed.to_lowercase();
    people
        .iter()
        .filter(|p| p.handle.to_lowercase().starts_with(&typed))
        .collect()
}

/// The ids of everyone mentioned in `text`.
pub fn mentions_in(people: &[Participant], text: &str) -> Vec<SharedString> {
    people
        .iter()
        .filter(|p| {
            let tag = format!("@{}", p.handle);
            text.match_indices(&tag).any(|(at, _)| {
                let end = at + tag.len();
                text[end..]
                    .chars()
                    .next()
                    .is_none_or(|c| !c.is_alphanumeric())
            })
        })
        .map(|p| p.id.clone())
        .collect()
}

/// Byte ranges of mentions in `text`, for highlighting.
pub fn mention_ranges(people: &[Participant], text: &str) -> Vec<std::ops::Range<usize>> {
    let mut ranges: Vec<_> = people
        .iter()
        .flat_map(|p| {
            let tag = format!("@{}", p.handle);
            text.match_indices(&tag)
                .map(|(at, _)| at..at + tag.len())
                .collect::<Vec<_>>()
        })
        .collect();
    ranges.sort_by_key(|r| r.start);
    ranges.dedup_by(|b, a| b.start < a.end);
    ranges
}

/// A live log of messages between assistants and people.
pub struct AgentMessageStream {
    people: Vec<Participant>,
    messages: Vec<StreamMessage>,
    filter: Option<SharedString>,
    field: Entity<TextareaState>,
    picked: usize,
    _field_events: Subscription,
}

impl EventEmitter<MessageStreamEvent> for AgentMessageStream {}

impl Focusable for AgentMessageStream {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.field.focus_handle(cx)
    }
}

impl AgentMessageStream {
    /// Create a stream for these participants.
    pub fn new(people: Vec<Participant>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let field = cx.new(|cx| {
            let mut field = TextareaState::new(window, cx)
                .auto_grow(1, 4)
                .placeholder("Write to the stream, @ to mention");
            field.set_submit_on_enter(true, cx);
            field
        });
        let field_events =
            cx.subscribe_in(&field, window, |this, _, event, window, cx| match event {
                InputEvent::PressEnter { shift: false, .. } => {
                    if !this.complete_mention(window, cx) {
                        this.send(window, cx);
                    }
                }
                InputEvent::Change => {
                    this.picked = 0;
                    cx.notify();
                }
                _ => {}
            });
        Self {
            people,
            messages: Vec::new(),
            filter: None,
            field,
            picked: 0,
            _field_events: field_events,
        }
    }

    /// Show these messages, oldest first.
    pub fn set_messages(&mut self, messages: Vec<StreamMessage>, cx: &mut Context<Self>) {
        self.messages = messages;
        cx.notify();
    }

    /// Add one message at the end.
    pub fn push(&mut self, message: StreamMessage, cx: &mut Context<Self>) {
        self.messages.push(message);
        cx.notify();
    }

    /// Show only messages from or to this participant.
    pub fn set_filter(&mut self, filter: Option<SharedString>, cx: &mut Context<Self>) {
        self.filter = filter;
        cx.notify();
    }

    fn matches(&self, cx: &App) -> Vec<Participant> {
        let text = self.field.read(cx).value();
        typing_mention(&text)
            .map(|typed| {
                mention_matches(&self.people, typed)
                    .into_iter()
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    fn complete_mention(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let found = self.matches(cx);
        let Some(person) = found.get(self.picked.min(found.len().saturating_sub(1))) else {
            return false;
        };
        let text = self.field.read(cx).value().to_string();
        let Some(at) = text.rfind('@') else {
            return false;
        };
        let next = format!("{}@{} ", &text[..at], person.handle);
        self.field
            .update(cx, |field, cx| field.set_value(next, window, cx));
        self.picked = 0;
        cx.notify();
        true
    }

    fn pick(&mut self, handle: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.field.read(cx).value().to_string();
        if let Some(at) = text.rfind('@') {
            let next = format!("{}@{handle} ", &text[..at]);
            self.field
                .update(cx, |field, cx| field.set_value(next, window, cx));
        }
        self.field.focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.field.read(cx).value();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }
        let mentions = mentions_in(&self.people, trimmed);
        cx.emit(MessageStreamEvent::Send {
            text: trimmed.to_string().into(),
            mentions,
        });
        self.field
            .update(cx, |field, cx| field.set_value("", window, cx));
        cx.notify();
    }

    fn handle(&self, id: &SharedString) -> SharedString {
        self.people
            .iter()
            .find(|p| &p.id == id)
            .map(|p| p.handle.clone())
            .unwrap_or_else(|| id.clone())
    }
}

impl Render for AgentMessageStream {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.primary;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;
        let found = self.matches(cx);
        let picked = self.picked.min(found.len().saturating_sub(1));

        let shown: Vec<&StreamMessage> = self
            .messages
            .iter()
            .filter(|m| {
                self.filter.as_ref().is_none_or(|f| {
                    m.from.as_ref() == Some(f) || m.to.as_ref() == Some(f) || m.from.is_none()
                })
            })
            .collect();

        let chips: Vec<_> = std::iter::once(None)
            .chain(self.people.iter().map(|p| Some(p.clone())))
            .map(|person| {
                let id = person.as_ref().map(|p| p.id.clone());
                let active = self.filter == id;
                let label: SharedString = person
                    .as_ref()
                    .map(|p| p.handle.clone())
                    .unwrap_or_else(|| "Everyone".into());
                h_flex()
                    .id(ElementId::Name(format!("filter-{label}").into()))
                    .gap_1()
                    .h(px(24.))
                    .px_2()
                    .rounded_full()
                    .border_1()
                    .border_color(if active { ink } else { line })
                    .when(active, |this| this.bg(chip))
                    .cursor_pointer()
                    .text_xs()
                    .aria_selected(active)
                    .role(Role::Tab)
                    .aria_label(format!("Show {label}"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.filter = id.clone();
                        cx.emit(MessageStreamEvent::Filter(id.clone()));
                        cx.notify();
                    }))
                    .children(person.map(|p| {
                        Icon::new(if p.kind == ParticipantKind::Agent {
                            IconName::Bot
                        } else {
                            IconName::User
                        })
                        .xsmall()
                        .text_color(quiet)
                    }))
                    .child(label)
            })
            .collect();

        let rows: Vec<_> = shown
            .iter()
            .map(|message| {
                let ranges = mention_ranges(&self.people, &message.text);
                let text = StyledText::new(message.text.clone()).with_highlights(
                    ranges
                        .into_iter()
                        .map(|r| {
                            (
                                r,
                                HighlightStyle {
                                    color: Some(accent),
                                    background_color: Some(accent.opacity(0.1)),
                                    ..Default::default()
                                },
                            )
                        })
                        .collect::<Vec<_>>(),
                );
                match &message.from {
                    None => h_flex()
                        .gap_2()
                        .py_1()
                        .text_xs()
                        .text_color(quiet)
                        .child(div().flex_1().h(px(1.)).bg(line))
                        .child(div().flex_none().child(message.text.clone()))
                        .child(div().flex_1().h(px(1.)).bg(line))
                        .into_any_element(),
                    Some(from) => {
                        let kind = self.people.iter().find(|p| &p.id == from).map(|p| p.kind);
                        h_flex()
                            .items_start()
                            .gap_2()
                            .py_1p5()
                            .child(
                                div()
                                    .size(px(24.))
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(if kind == Some(ParticipantKind::Person) {
                                        px(12.)
                                    } else {
                                        px(6.)
                                    })
                                    .bg(chip)
                                    .child(
                                        Icon::new(if kind == Some(ParticipantKind::Person) {
                                            IconName::User
                                        } else {
                                            IconName::Bot
                                        })
                                        .xsmall()
                                        .text_color(quiet),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        h_flex()
                                            .gap_1p5()
                                            .text_xs()
                                            .child(
                                                div()
                                                    .font_semibold()
                                                    .text_color(ink)
                                                    .child(self.handle(from)),
                                            )
                                            .children(message.to.as_ref().map(|to| {
                                                div()
                                                    .text_color(quiet)
                                                    .child(format!("\u{2192} {}", self.handle(to)))
                                            }))
                                            .child(
                                                div().text_color(quiet).child(message.when.clone()),
                                            ),
                                    )
                                    .child(div().line_height(relative(1.45)).child(text)),
                            )
                            .into_any_element()
                    }
                }
            })
            .collect();

        let picker = (!found.is_empty()).then(|| {
            v_flex()
                .absolute()
                .bottom(px(48.))
                .left(px(8.))
                .w(px(220.))
                .p_1()
                .rounded(radius)
                .border_1()
                .border_color(line)
                .bg(fill)
                .shadow(resting_shadow(cx))
                .children(found.iter().enumerate().map(|(ix, person)| {
                    let handle = person.handle.clone();
                    h_flex()
                        .id(ElementId::Name(format!("mention-{}", person.id).into()))
                        .gap_2()
                        .px_2()
                        .py_1()
                        .rounded(px(4.))
                        .cursor_pointer()
                        .when(ix == picked, |this| this.bg(chip))
                        .aria_selected(ix == picked)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.pick(handle.clone(), window, cx)
                        }))
                        .child(
                            Icon::new(if person.kind == ParticipantKind::Agent {
                                IconName::Bot
                            } else {
                                IconName::User
                            })
                            .xsmall()
                            .text_color(quiet),
                        )
                        .child(format!("@{}", person.handle))
                }))
        });

        let count = found.len();
        v_flex()
            .w_full()
            .max_w(px(620.))
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .gap_1p5()
                    .flex_wrap()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(line)
                    .children(chips),
            )
            .child(
                v_flex()
                    .id("stream-log")
                    .px_3()
                    .py_1()
                    .max_h(px(360.))
                    .overflow_y_scroll()
                    .children(rows)
                    .when(shown.is_empty(), |this| {
                        this.child(div().py_4().text_color(quiet).child("Nothing said yet."))
                    }),
            )
            .child(
                h_flex()
                    .relative()
                    .items_end()
                    .gap_2()
                    .px_2()
                    .py_2()
                    .border_t_1()
                    .border_color(line)
                    .capture_key_down(cx.listener(
                        move |this, event: &isonomic_ui::KeyDownEvent, _, cx| {
                            if count == 0 {
                                return;
                            }
                            match event.keystroke.key.as_str() {
                                "down" => {
                                    this.picked = (this.picked + 1) % count;
                                    cx.stop_propagation();
                                    cx.notify();
                                }
                                "up" => {
                                    this.picked = (this.picked + count - 1) % count;
                                    cx.stop_propagation();
                                    cx.notify();
                                }
                                _ => {}
                            }
                        },
                    ))
                    .child(
                        div().flex_1().min_w_0().child(
                            Textarea::new(&self.field)
                                .appearance(false)
                                .aria_label("Message to the stream"),
                        ),
                    )
                    .child(
                        Button::new("stream-send")
                            .small()
                            .primary()
                            .icon(IconName::ArrowUp)
                            .accessibility_label("Send to the stream")
                            .on_click(cx.listener(|this, _, window, cx| this.send(window, cx))),
                    )
                    .children(picker),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn people() -> Vec<Participant> {
        vec![
            Participant::new("r", "Reconciler", ParticipantKind::Agent),
            Participant::new("p", "Payables", ParticipantKind::Agent),
            Participant::new("m", "Maya", ParticipantKind::Person),
        ]
    }

    #[test]
    fn a_mention_is_typed_only_after_an_at_with_no_space() {
        assert_eq!(typing_mention("hi @Rec"), Some("Rec"));
        assert_eq!(typing_mention("@"), Some(""));
        assert_eq!(typing_mention("hi @Rec there"), None);
        assert_eq!(typing_mention("mail@home"), None);
    }

    #[test]
    fn matches_and_mentions_find_people() {
        let people = people();
        let found: Vec<_> = mention_matches(&people, "pa")
            .iter()
            .map(|p| p.id.to_string())
            .collect();
        assert_eq!(found, ["p"]);
        assert_eq!(
            mentions_in(&people, "@Maya and @Payables, please"),
            ["p", "m"]
        );
        assert!(mentions_in(&people, "@Mayan").is_empty());
        assert_eq!(mention_ranges(&people, "ok @Maya"), [3..8]);
    }
}
