// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the chat thread.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{
    ActiveTheme as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, ScrollHandle,
    Styled as _, Window, div, point, px,
};
use isonomic_ui_extra::chat_thread::{ChatThread, Density, Speaker, ThreadMessage};

/// Thursday 12 March 2026, 09:00 UTC: the made-up present of this story.
const NOW: i64 = 1_773_306_000;
const MINUTE: i64 = 60;
const DAY: i64 = 86_400;
/// How often the reply being written gains a word.
const TICK: Duration = Duration::from_millis(140);

const REPLY: &str = "The rear wheel is trued and the new chain is on. The front brake pads are \
                     worn to the line, so I would swap them while the bike is on the stand: \
                     twelve euros for the pair, ten minutes of work.";

/// Turns that arrive when the story's button is pressed.
const LATER: [(Speaker, &str); 3] = [
    (Speaker::Person, "Go ahead with the pads."),
    (
        Speaker::Assistant,
        "Done. They are bedding in on the stand now.",
    ),
    (
        Speaker::Assistant,
        "Ready from four o'clock. Bring the lock key.",
    ),
];

/// A bike workshop's thread: two days, an unread mark, a reply being written.
pub struct ChatThreadStory {
    handle: ScrollHandle,
    written: usize,
    arrived: usize,
}

impl ChatThreadStory {
    /// Create the story view and start writing the reply.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(TICK).await;
                    let going = this
                        .update(cx, |this: &mut Self, cx| {
                            if this.written >= REPLY.split(' ').count() {
                                return false;
                            }
                            this.written += 1;
                            cx.notify();
                            true
                        })
                        .unwrap_or(false);
                    if !going {
                        break;
                    }
                }
            })
            .detach();
            Self {
                handle: ScrollHandle::new(),
                written: 6,
                arrived: 0,
            }
        })
    }

    fn messages(&self) -> Vec<ThreadMessage> {
        let words: Vec<&str> = REPLY.split(' ').collect();
        let writing = self.written < words.len();
        let reply = words[..self.written.min(words.len())].join(" ");
        let yesterday = NOW - DAY - 40 * MINUTE;
        let mut messages = vec![
            ThreadMessage::new(
                "drop",
                Speaker::Person,
                "Dropping the green tourer off after work. The rear wheel wobbles and the chain \
                 skips under load.",
                yesterday,
            ),
            ThreadMessage::new(
                "booked",
                Speaker::Assistant,
                "Booked in for tomorrow morning. Leave it at the side door if the shop is shut.",
                yesterday + MINUTE,
            ),
            ThreadMessage::new(
                "key",
                Speaker::Person,
                "It is locked to the rack.\nThe key is under the saddle bag.",
                yesterday + 2 * MINUTE,
            ),
            ThreadMessage::new("thanks", Speaker::Person, "Thanks!", yesterday + 3 * MINUTE),
            ThreadMessage::new(
                "start",
                Speaker::Assistant,
                "Morning. The tourer is on the stand.",
                NOW - 50 * MINUTE,
            ),
            ThreadMessage::new(
                "found",
                Speaker::Assistant,
                "Two loose spokes and a stretched chain, nothing bent.",
                NOW - 48 * MINUTE,
            ),
            ThreadMessage::new(
                "cost",
                Speaker::Person,
                "What will it cost?",
                NOW - 20 * MINUTE,
            )
            .author("Ines"),
            ThreadMessage::new(
                "quote",
                Speaker::Assistant,
                "Thirty-eight euros with the chain.",
                NOW - 19 * MINUTE,
            ),
            ThreadMessage::new("reply", Speaker::Assistant, reply, NOW - 2 * MINUTE)
                .streaming(writing),
        ];
        for (ix, (speaker, text)) in LATER.iter().take(self.arrived).enumerate() {
            messages.push(ThreadMessage::new(
                format!("later-{ix}"),
                *speaker,
                *text,
                NOW + ix as i64 * MINUTE,
            ));
        }
        messages
    }
}

impl Story for ChatThreadStory {
    fn title() -> &'static str {
        "Chat thread"
    }

    fn description() -> &'static str {
        "A conversation's turns grouped by speaker, with day and unread marks, a reply being written, and a way back to the latest turn."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ChatThreadStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let handle = self.handle.clone();
        let all_arrived = self.arrived >= LATER.len();

        v_flex()
            .gap_6()
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("scroll-up")
                            .secondary()
                            .label("Scroll to the start")
                            .on_click(cx.listener(move |_, _, _, cx| {
                                handle.set_offset(point(px(0.), px(0.)));
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("arrive")
                            .secondary()
                            .label("A turn arrives")
                            .when_done(all_arrived)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.arrived = (this.arrived + 1).min(LATER.len());
                                cx.notify();
                            })),
                    ),
            )
            .child(
                ChatThread::new("workshop", self.messages())
                    .unread_from("quote")
                    .assistant_name("Workshop")
                    .now(NOW, 60)
                    .height(px(380.))
                    .track_scroll(&self.handle),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(quiet)
                            .child("Dense, on a 24-hour clock, with a note after the last turn"),
                    )
                    .child(
                        ChatThread::new(
                            "counter",
                            vec![
                                ThreadMessage::new(
                                    "ask",
                                    Speaker::Person,
                                    "Do you stock 28 mm tubes with a long valve?",
                                    NOW - 9 * DAY,
                                ),
                                ThreadMessage::new(
                                    "have",
                                    Speaker::Assistant,
                                    "Four on the shelf, sixty millimetre valves.",
                                    NOW - 9 * DAY + 2 * MINUTE,
                                ),
                                ThreadMessage::new(
                                    "hold",
                                    Speaker::Person,
                                    "Hold two for me, please.",
                                    NOW - 30 * MINUTE,
                                ),
                                ThreadMessage::new(
                                    "held",
                                    Speaker::Assistant,
                                    "Two are behind the counter under your name.",
                                    NOW - 29 * MINUTE,
                                ),
                            ],
                        )
                        .density(Density::Dense)
                        .assistant_name("Counter")
                        .now(NOW, 60)
                        .twenty_four_hour(true)
                        .height(px(230.))
                        .footer(
                            div()
                                .pt_2()
                                .text_xs()
                                .text_color(quiet)
                                .child("Held until Saturday noon."),
                        ),
                    ),
            )
    }
}

/// Lets the story switch its button off once every turn has arrived.
trait WhenDone {
    fn when_done(self, done: bool) -> Self;
}

impl WhenDone for Button {
    fn when_done(self, done: bool) -> Self {
        use isonomic_ui::component::Disableable as _;
        self.disabled(done)
    }
}
