// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The inbox where people pick up conversations an assistant handed over:
//! the queue with each one's state and wait, the chosen conversation, and
//! the customer beside it with what can be done.

use crate::Story;
use isonomic_ui::component::{
    ActiveTheme as _, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    description_list::DescriptionList,
    h_flex, v_flex,
};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use isonomic_ui_extra::chat_thread::{ChatThread, Density, Speaker, ThreadMessage};
use isonomic_ui_extra::common::Tone;
use isonomic_ui_extra::identity_avatar::{Identity, IdentityAvatar, Presence};
use isonomic_ui_extra::media_list_row::{MediaListRow, RowMedia};
use isonomic_ui_extra::prompt_composer::PromptComposer;
use isonomic_ui_extra::status_badge::StatusBadge;

/// Tuesday 17 November 2026, 14:20 UTC: the made-up present of this screen.
const NOW: i64 = 1_794_925_200;
const MINUTE: i64 = 60;

/// Where a handed-over conversation stands.
#[derive(Clone, Copy)]
enum State {
    Urgent,
    Waiting,
    Yours,
}

impl State {
    fn badge(self, id: (&'static str, usize)) -> StatusBadge {
        match self {
            State::Urgent => StatusBadge::new(id, "Urgent").tone(Tone::Danger).live(true),
            State::Waiting => StatusBadge::new(id, "Waiting").tone(Tone::Warning),
            State::Yours => StatusBadge::new(id, "Yours").tone(Tone::Info),
        }
    }
}

/// One conversation in the queue.
struct Escalation {
    name: &'static str,
    reason: &'static str,
    waited: &'static str,
    state: State,
    plan: &'static str,
    since: &'static str,
    orders: &'static str,
    spend: &'static str,
    summary: &'static str,
    turns: [(Speaker, &'static str, i64); 4],
}

fn queue() -> Vec<Escalation> {
    vec![
        Escalation {
            name: "Ruth Adebayo",
            reason: "Charged twice for one order",
            waited: "9 min",
            state: State::Urgent,
            plan: "Plant club, monthly",
            since: "March 2024",
            orders: "31",
            spend: "\u{20ac}1,284",
            summary: "Two charges of \u{20ac}64.00 for order 58213 on 14 Nov. The bank shows \
                      both as settled. She asked for a person after the refund form failed.",
            turns: [
                (
                    Speaker::Person,
                    "I was charged twice for the same order and the refund form says error.",
                    14,
                ),
                (
                    Speaker::Assistant,
                    "I can see two payments of \u{20ac}64.00 for order 58213. The refund form \
                     cannot reverse a settled charge, so a person needs to do it.",
                    13,
                ),
                (
                    Speaker::Person,
                    "Please just fix it, that money was for rent.",
                    11,
                ),
                (
                    Speaker::Assistant,
                    "I have passed this to the team with both payment references. Someone \
                     will be with you shortly.",
                    9,
                ),
            ],
        },
        Escalation {
            name: "Tomasz Wilk",
            reason: "Plant arrived broken",
            waited: "6 min",
            state: State::Waiting,
            plan: "Pay as you go",
            since: "October 2026",
            orders: "2",
            spend: "\u{20ac}118",
            summary: "Fiddle-leaf fig arrived with a snapped stem. Photo attached. Wants a \
                      replacement, not a refund.",
            turns: [
                (Speaker::Person, "The fig arrived snapped in half.", 9),
                (
                    Speaker::Assistant,
                    "I am sorry. Could you send a photo of the plant and the box?",
                    8,
                ),
                (Speaker::Person, "Sent. I would like a new one please.", 7),
                (
                    Speaker::Assistant,
                    "Thank you. A person will arrange the replacement.",
                    6,
                ),
            ],
        },
        Escalation {
            name: "Hannah Becker",
            reason: "Delivery to an island",
            waited: "4 min",
            state: State::Waiting,
            plan: "Pay as you go",
            since: "May 2025",
            orders: "5",
            spend: "\u{20ac}240",
            summary: "Asks whether large trees can go to an island address. Not covered by \
                      the delivery rules the assistant has.",
            turns: [
                (Speaker::Person, "Do you deliver olive trees to Juist?", 6),
                (
                    Speaker::Assistant,
                    "Small plants go by post. For trees over 1.5 m I need to check with the team.",
                    5,
                ),
                (Speaker::Person, "It is 1.8 m.", 5),
                (
                    Speaker::Assistant,
                    "I have asked a colleague to confirm.",
                    4,
                ),
            ],
        },
        Escalation {
            name: "Kofi Mensah",
            reason: "Wholesale price request",
            waited: "2 min",
            state: State::Yours,
            plan: "Trade account",
            since: "January 2023",
            orders: "88",
            spend: "\u{20ac}14,920",
            summary: "Café owner asking for a price on 40 hanging planters for a new shop.",
            turns: [
                (
                    Speaker::Person,
                    "Can I get a price for 40 hanging planters?",
                    4,
                ),
                (
                    Speaker::Assistant,
                    "Orders over 25 get a trade price. Let me bring in the trade team.",
                    3,
                ),
                (Speaker::Person, "Great, by Friday if possible.", 3),
                (Speaker::Assistant, "Noted. Handing you over now.", 2),
            ],
        },
        Escalation {
            name: "Lena Vogt",
            reason: "Wants to cancel the club",
            waited: "1 min",
            state: State::Waiting,
            plan: "Plant club, yearly",
            since: "June 2022",
            orders: "54",
            spend: "\u{20ac}2,610",
            summary: "Moving abroad and wants to cancel. Yearly plan renewed 3 weeks ago.",
            turns: [
                (
                    Speaker::Person,
                    "I need to cancel, I am moving to Lisbon.",
                    2,
                ),
                (
                    Speaker::Assistant,
                    "Your yearly plan renewed on 27 October. A person can talk through a \
                     partial refund.",
                    2,
                ),
                (Speaker::Person, "Ok.", 1),
                (
                    Speaker::Assistant,
                    "Someone will be with you in a moment.",
                    1,
                ),
            ],
        },
    ]
}

/// A plant shop's support team working through handed-over conversations.
pub struct HandoffInboxScreenStory {
    queue: Vec<Escalation>,
    picked: usize,
    composer: Entity<PromptComposer>,
}

impl HandoffInboxScreenStory {
    /// Create the screen.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let composer = cx.new(|cx| PromptComposer::new("Reply as Dana\u{2026}", window, cx));
            composer.update(cx, |composer, cx| composer.set_dictation(false, cx));
            Self {
                queue: queue(),
                picked: 0,
                composer,
            }
        })
    }
}

impl Story for HandoffInboxScreenStory {
    fn title() -> &'static str {
        "Handoff inbox screen"
    }

    fn description() -> &'static str {
        "A support team's inbox built from the library: conversations an assistant handed over, the chosen transcript, and the customer with actions."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for HandoffInboxScreenStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows: Vec<_> = self
            .queue
            .iter()
            .enumerate()
            .map(|(ix, item)| {
                let pick = cx.entity();
                MediaListRow::new(("handoff-row", ix), item.name)
                    .media(RowMedia::Icon(IconName::User))
                    .secondary(format!("{} \u{b7} {}", item.reason, item.waited))
                    .trailing(item.state.badge(("handoff-state", ix)))
                    .picked(ix == self.picked)
                    .on_click(move |_, _, cx| {
                        pick.update(cx, |this, cx| {
                            this.picked = ix;
                            cx.notify();
                        })
                    })
            })
            .collect();

        let theme = cx.theme();
        let line = theme.border;
        let quiet = theme.muted_foreground;
        let item = &self.queue[self.picked];
        let messages: Vec<ThreadMessage> = item
            .turns
            .iter()
            .enumerate()
            .map(|(ix, (speaker, text, ago))| {
                ThreadMessage::new(format!("turn-{ix}"), *speaker, *text, NOW - ago * MINUTE)
            })
            .collect();

        let header = h_flex()
            .flex_none()
            .h(px(48.))
            .px_4()
            .gap_3()
            .border_b_1()
            .border_color(line)
            .child(div().text_sm().font_medium().child("Handoff inbox"))
            .child(
                StatusBadge::new("inbox-urgent", "Urgent")
                    .tone(Tone::Danger)
                    .count(1, 99),
            )
            .child(
                StatusBadge::new("inbox-waiting", "Waiting")
                    .tone(Tone::Warning)
                    .count(3, 99),
            )
            .child(div().flex_1())
            .child(
                IdentityAvatar::new(
                    "inbox-me",
                    Identity::person("Dana Kowalski").presence(Presence::Online),
                )
                .size(24.),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(quiet)
                    .child("Dana, on shift until 18:00"),
            );

        let list = v_flex()
            .flex_none()
            .w(px(290.))
            .h_full()
            .border_r_1()
            .border_color(line)
            .child(
                div()
                    .px_3()
                    .pt_3()
                    .pb_1()
                    .text_xs()
                    .font_medium()
                    .text_color(quiet)
                    .child("Oldest first"),
            )
            .child(
                div()
                    .id("handoff-queue")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_2()
                    .child(v_flex().gap_1().children(rows)),
            );

        let transcript =
            v_flex()
                .flex_1()
                .min_w_0()
                .h_full()
                .child(
                    h_flex()
                        .flex_none()
                        .px_4()
                        .py_3()
                        .gap_2()
                        .border_b_1()
                        .border_color(line)
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(div().text_sm().font_medium().child(item.name))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(quiet)
                                        .child(format!("{}, waiting {}", item.reason, item.waited)),
                                ),
                        )
                        .child(item.state.badge(("handoff-current", self.picked))),
                )
                .child(
                    div().flex_1().min_h_0().child(
                        ChatThread::new(("handoff-thread", self.picked), messages)
                            .bare()
                            .density(Density::Dense)
                            .assistant_name("Assistant")
                            .person_name(item.name)
                            .now(NOW, 0)
                            .twenty_four_hour(true)
                            .footer(
                                div().pt_2().text_xs().text_color(quiet).child(
                                    "Handed over to the team. The assistant has stepped back.",
                                ),
                            ),
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

        let details = div()
            .id("handoff-details")
            .flex_none()
            .w(px(260.))
            .h_full()
            .overflow_y_scroll()
            .border_l_1()
            .border_color(line)
            .child(
                v_flex()
                    .gap_4()
                    .p_4()
                    .child(
                        h_flex()
                            .gap_3()
                            .child(
                                IdentityAvatar::new(
                                    ("handoff-customer", self.picked),
                                    Identity::person(item.name).presence(Presence::Online),
                                )
                                .size(40.),
                            )
                            .child(
                                v_flex()
                                    .min_w_0()
                                    .child(div().text_sm().font_medium().child(item.name))
                                    .child(div().text_xs().text_color(quiet).child(item.plan)),
                            ),
                    )
                    .child(
                        DescriptionList::vertical()
                            .columns(2)
                            .item("Since", item.since, 1)
                            .item("Orders", item.orders, 1)
                            .item("Spent", item.spend, 1)
                            .item("Waited", item.waited, 1),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_medium()
                                    .text_color(quiet)
                                    .child("Assistant's summary"),
                            )
                            .child(div().text_sm().child(item.summary)),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                Button::new("handoff-take")
                                    .primary()
                                    .small()
                                    .w_full()
                                    .label("Take over"),
                            )
                            .child(
                                Button::new("handoff-refund")
                                    .outline()
                                    .small()
                                    .w_full()
                                    .label("Issue a refund"),
                            )
                            .child(
                                Button::new("handoff-back")
                                    .ghost()
                                    .small()
                                    .w_full()
                                    .label("Send back to the assistant"),
                            ),
                    ),
            );

        super::window_frame(cx).child(header).child(
            h_flex()
                .flex_1()
                .min_h_0()
                .w_full()
                .child(list)
                .child(transcript)
                .child(details),
        )
    }
}
