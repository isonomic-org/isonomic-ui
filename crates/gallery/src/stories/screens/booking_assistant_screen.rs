// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! A booking assistant: the conversation with a question still open, and
//! beside it the day, the time and the booking waiting to be confirmed.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, StyledExt as _, h_flex, v_flex};
use isonomic_ui::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Subscription, Window,
    div, px,
};
use isonomic_ui_extra::chat_thread::{ChatThread, Speaker, ThreadMessage};
use isonomic_ui_extra::confirmation_card::{
    ConfirmationCard, ConfirmationItem, ConfirmationStatus,
};
use isonomic_ui_extra::due_date_picker::{Day, DueDatePicker};
use isonomic_ui_extra::prompt_composer::PromptComposer;
use isonomic_ui_extra::question_card::{
    Question, QuestionAnswer, QuestionCard, QuestionCardEvent, QuestionLayout, QuestionOption,
};
use isonomic_ui_extra::time_slot_picker::{TimeSlotPicker, write_span};

use super::window_frame;

/// Monday 16 November 2026, 08:40 UTC: the made-up present of this screen.
const NOW: i64 = 1_794_818_400;
const MINUTE: i64 = 60;
const TODAY: Day = Day {
    year: 2026,
    month: 11,
    day: 16,
};
/// How long a first visit takes, in minutes.
const VISIT: u32 = 45;

fn visit_question() -> Question {
    Question::new("visit", "Which kind of visit would suit you?")
        .task("Knee pain after running")
        .layout(QuestionLayout::Cards)
        .option(
            QuestionOption::new("sports", "Sports physio")
                .summary("45 min, gait check on the treadmill")
                .reason("Best for pain that started on a run"),
        )
        .option(
            QuestionOption::new("general", "General physio")
                .summary("45 min, assessment and exercises"),
        )
        .option(
            QuestionOption::new("video", "Video call").summary("30 min, advice and a home plan"),
        )
}

/// A runner booking a physiotherapy visit at a small clinic.
pub struct BookingAssistantScreenStory {
    question: Entity<QuestionCard>,
    composer: Entity<PromptComposer>,
    day: Option<Day>,
    cursor: Option<Day>,
    start: Option<u32>,
    kind: &'static str,
    status: ConfirmationStatus,
    _events: Subscription,
}

impl BookingAssistantScreenStory {
    /// Create the screen.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let question = cx.new(|cx| QuestionCard::new(window, cx).questions([visit_question()]));
            let events = cx.subscribe(&question, |this: &mut Self, _, event, cx| {
                if let QuestionCardEvent::Answered {
                    answer: QuestionAnswer::Option(option),
                    ..
                } = event
                {
                    this.kind = match option.as_ref() {
                        "general" => "General physio",
                        "video" => "Video call",
                        _ => "Sports physio",
                    };
                    cx.notify();
                }
            });
            let composer = cx.new(|cx| PromptComposer::new("Reply\u{2026}", window, cx));
            composer.update(cx, |composer, cx| {
                composer.set_dictation(false, cx);
            });
            Self {
                question,
                composer,
                day: Some(TODAY.add_days(3)),
                cursor: None,
                start: Some(17 * 60 + 30),
                kind: "Sports physio",
                status: ConfirmationStatus::Review,
                _events: events,
            }
        })
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        self.status = ConfirmationStatus::Confirming;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1200))
                .await;
            this.update(cx, |this, cx| {
                this.status = ConfirmationStatus::Done;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn messages(&self) -> Vec<ThreadMessage> {
        vec![
            ThreadMessage::new(
                "ask",
                Speaker::Person,
                "Hi, my left knee has hurt since Saturday's long run, on the outside, mostly \
                 going downstairs. Can I see someone this week? Evenings are best.",
                NOW - 6 * MINUTE,
            ),
            ThreadMessage::new(
                "openings",
                Speaker::Assistant,
                "Sorry to hear that. Pain on the outside of the knee after a long run is \
                 common and usually settles with the right exercises. Andrea has evening \
                 openings on Wednesday and Thursday, and your plan covers 40 of the visit.",
                NOW - 5 * MINUTE,
            ),
            ThreadMessage::new(
                "thursday",
                Speaker::Person,
                "Thursday after work would be ideal.",
                NOW - 3 * MINUTE,
            ),
            ThreadMessage::new(
                "question",
                Speaker::Assistant,
                "Which kind of visit?",
                NOW - 2 * MINUTE,
            )
            .content(
                v_flex()
                    .gap_2()
                    .child(div().text_sm().child(
                        "Thursday at 17:30 is free. I have held it for ten minutes. One \
                         question first:",
                    ))
                    .child(self.question.clone()),
            ),
        ]
    }

    fn booking(&self, cx: &mut Context<Self>) -> AnyElement {
        let (pick, cursor, slot) = (cx.entity(), cx.entity(), cx.entity());
        let (confirm, undo) = (cx.entity(), cx.entity());
        let when = match (self.day, self.start) {
            (Some(day), Some(start)) => {
                format!("{}, {}", day.words(), write_span(start, VISIT, false))
            }
            _ => "Pick a day and a time".to_string(),
        };

        v_flex()
            .gap_3()
            .child(
                v_flex().gap_2().child(
                    DueDatePicker::new("booking-day", TODAY)
                        .picked(self.day)
                        .cursor(self.cursor)
                        .earliest(TODAY)
                        .weekends(false)
                        .on_pick(move |day, _, cx| {
                            pick.update(cx, |this, cx| {
                                this.day = Some(day);
                                this.cursor = Some(day);
                                cx.notify();
                            })
                        })
                        .on_cursor(move |day, _, cx| {
                            cursor.update(cx, |this, cx| {
                                this.cursor = Some(day);
                                cx.notify();
                            })
                        }),
                ),
            )
            .child(
                h_flex()
                    .gap_3()
                    .justify_between()
                    .child(div().text_sm().font_medium().child("Time"))
                    .child(
                        TimeSlotPicker::new("booking-time", self.start)
                            .hours(16 * 60, 19 * 60 + 30, 30)
                            .length(VISIT)
                            .booked([16 * 60, 17 * 60, 18 * 60 + 30])
                            .twenty_four_hour()
                            .label("Visit start")
                            .on_pick(move |start, _, cx| {
                                slot.update(cx, |this, cx| {
                                    this.start = Some(start);
                                    cx.notify();
                                })
                            }),
                    ),
            )
            .child(
                ConfirmationCard::new("booking-confirm", "Book with Andrea Lindqvist")
                    .subtitle(when)
                    .done_note("Riverside Physio, room 2. A reminder comes the day before.")
                    .items(vec![
                        ConfirmationItem::new("visit", self.kind, 72.)
                            .detail("First visit, 45 minutes"),
                    ])
                    .fee("Covered by your plan", -40.)
                    .currency("\u{20ac}")
                    .warning("Free to cancel until 24 hours before.")
                    .confirm_label("Book visit")
                    .done_label("Booked")
                    .status(self.status)
                    .on_confirm(move |_, cx| confirm.update(cx, |this, cx| this.confirm(cx)))
                    .on_undo(move |_, cx| {
                        undo.update(cx, |this, cx| {
                            this.status = ConfirmationStatus::Review;
                            cx.notify();
                        })
                    }),
            )
            .into_any_element()
    }
}

impl Story for BookingAssistantScreenStory {
    fn title() -> &'static str {
        "Booking assistant screen"
    }

    fn description() -> &'static str {
        "A booking assistant built from the library: a chat with a question to answer, and a day, a time and a booking to confirm."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for BookingAssistantScreenStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let booking = self.booking(cx);
        let messages = self.messages();
        let theme = cx.theme();
        let line = theme.border;
        let quiet = theme.muted_foreground;

        let header = h_flex()
            .flex_none()
            .h(px(52.))
            .px_4()
            .gap_3()
            .border_b_1()
            .border_color(line)
            .child(
                v_flex()
                    .child(div().text_sm().font_medium().child("Riverside Physio"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(quiet)
                            .child("Booking assistant, replies in seconds"),
                    ),
            );

        let chat = v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(
                div().flex_1().min_h_0().child(
                    ChatThread::new("booking-thread", messages)
                        .bare()
                        .assistant_name("Riverside")
                        .person_name("Jonas")
                        .now(NOW, 0)
                        .twenty_four_hour(true),
                ),
            )
            .child(
                div()
                    .flex_none()
                    .px_4()
                    .pb_4()
                    .pt_2()
                    .child(self.composer.clone()),
            );

        let side = div()
            .id("booking-side")
            .flex_none()
            .w(px(360.))
            .h_full()
            .overflow_y_scroll()
            .p_4()
            .border_l_1()
            .border_color(line)
            .child(booking);

        window_frame(cx)
            .child(header)
            .child(h_flex().flex_1().min_h_0().w_full().child(chat).child(side))
    }
}
