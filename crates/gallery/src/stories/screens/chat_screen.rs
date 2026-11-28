// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! A chat app: the conversations down the side, a thread with a reasoned
//! answer and its branches, the model in the header and a composer with ways
//! of working and a context tray.

use std::time::Duration;

use crate::Story;
use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::component::{ActiveTheme as _, IconName, StyledExt as _, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div, px,
};
use isonomic_ui_extra::chat_thread::{ChatThread, Speaker, ThreadMessage};
use isonomic_ui_extra::conversation_list::{Conversation, ConversationList};
use isonomic_ui_extra::message_action_bar::MessageActionBar;
use isonomic_ui_extra::model_picker::{ModelOption, ModelPicker, PickerSetting};
use isonomic_ui_extra::prompt_composer::{
    ComposerMode, ContextItem, PromptComposer, PromptComposerEvent,
};
use isonomic_ui_extra::streamed_answer::{AnswerSource, StreamedAnswer};

use super::window_frame;

/// Saturday 14 November 2026, 10:30 UTC: the made-up present of this screen.
const NOW: i64 = 1_794_652_200;
const MINUTE: i64 = 60;
const HOUR: i64 = 3600;
const DAY: i64 = 24 * HOUR;

/// Three takes on the first answer, for its branches.
const TAKES: [&str; 3] = [
    "After cabinets (7,400), the counter (3,100) and about 3,900 for fitting, plumbing and \
     electrics, you have roughly 3,600 left for appliances. That covers an oven, a hob and an \
     extractor at mid-range prices, with a little over for a sink and tap.",
    "About 3,600 is left for appliances once fitting, plumbing and electrics (around 3,900) are \
     paid for.",
    "Roughly 3,600. Labour usually runs to a fifth of a kitchen budget, so I counted 3,900 for \
     fitting and trades; ask the fitter to confirm before you order anything.",
];

const REASONING: &str = "The dishwasher works and is eight years old, so keeping it saves the \
                         whole price of a new one, about 650.

An induction hob needs its own 32 A circuit. The old cooker point may already have one; if \
                         not, the electrician adds about 180.";

const ANSWER: &str = "**Yes, and it pays for most of the upgrade.** Keeping the dishwasher frees \
                      about 650 [1]. A good four-zone induction hob costs 450 to 700 more than \
                      a gas one [2], so the swap lands near even.

- **Check the circuit:** induction needs a 32 A supply; adding one is about 180 [3].
- **Check the pans:** a magnet should stick to the base of each pan.

If the circuit is already there, you come out about 100 ahead.";

fn chats() -> Vec<Conversation> {
    vec![
        Conversation::new("kitchen", "Kitchen renovation budget", NOW - 2 * MINUTE).pinned(true),
        Conversation::new("tiles", "Splashback tiles that hide grease", NOW - 3 * HOUR),
        Conversation::new("boiler", "Is the boiler worth servicing?", NOW - 20 * HOUR),
        Conversation::new("move", "Packing list for the move", NOW - 2 * DAY),
        Conversation::new("garden", "Raised beds for a shady garden", NOW - 3 * DAY),
        Conversation::new(
            "insurance",
            "Home insurance excess explained",
            NOW - 6 * DAY,
        ),
        Conversation::new("paint", "Paint for a bathroom ceiling", NOW - 9 * DAY),
        Conversation::new("lease", "Notice period on the old flat", NOW - 24 * DAY),
        Conversation::new("sofa", "Sofa that fits up narrow stairs", NOW - 40 * DAY),
    ]
}

fn modes() -> Vec<ComposerMode> {
    vec![
        ComposerMode::new("chat", "Chat")
            .icon(CatalogIcon::Chat)
            .hint("Talk it through"),
        ComposerMode::new("research", "Research")
            .icon(CatalogIcon::Search)
            .hint("Reads prices and guides first"),
        ComposerMode::new("plan", "Plan")
            .icon(CatalogIcon::Map)
            .hint("Writes a step-by-step plan"),
    ]
}

fn context() -> Vec<ContextItem> {
    vec![
        ContextItem::new("quotes", "kitchen-quotes.pdf", 9_800),
        ContextItem::new("plan", "floor-plan.png", 1_600),
        ContextItem::new("budget", "Budget sheet", 2_400).pinned(true),
    ]
}

/// A person planning a kitchen with an assistant.
pub struct ChatScreenStory {
    list: Entity<ConversationList>,
    answer: Entity<StreamedAnswer>,
    composer: Entity<PromptComposer>,
    take: usize,
    _events: Subscription,
}

impl ChatScreenStory {
    /// Create the screen.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let list = cx.new(|cx| {
                ConversationList::new("screen-chats", chats(), NOW, window, cx).active("kitchen")
            });
            let answer = cx.new(|cx| {
                StreamedAnswer::new("Can we keep the old dishwasher for an induction hob?", cx)
                    .animate(false)
                    .reasoning(REASONING)
                    .reasoning_took(Duration::from_secs(6))
                    .answer(ANSWER)
                    .source(
                        AnswerSource::new("Appliance price list, November", "Price list")
                            .domain("homegoods.example")
                            .icon(IconName::FileText),
                    )
                    .source(
                        AnswerSource::new("Induction or gas: running costs", "Guide")
                            .domain("cookingguide.example"),
                    )
                    .source(
                        AnswerSource::new("Electrician's quote, page 2", "Quote")
                            .domain("kitchen-quotes.pdf")
                            .icon(IconName::File),
                    )
                    .follow_up("Which hobs fit a 60 cm cut-out?")
                    .follow_up("Draft a question for the electrician")
            });
            answer.update(cx, |answer, cx| answer.start(cx));
            let composer =
                cx.new(|cx| PromptComposer::new("Ask about the kitchen\u{2026}", window, cx));
            composer.update(cx, |composer, cx| {
                composer.set_dictation(false, cx);
                composer.set_modes(modes(), Some("chat".into()), cx);
                composer.set_context(context(), Some(32_000), cx);
            });
            let events = cx.subscribe(&composer, |_: &mut Self, composer, event, cx| {
                if let PromptComposerEvent::SelectMode(id) = event {
                    let id = id.clone();
                    composer.update(cx, |composer, cx| composer.set_modes(modes(), Some(id), cx));
                }
            });
            Self {
                list,
                answer,
                composer,
                take: 0,
                _events: events,
            }
        })
    }

    fn messages(&self, cx: &mut Context<Self>) -> Vec<ThreadMessage> {
        let story = cx.entity();
        let take: SharedString = TAKES[self.take].into();
        vec![
            ThreadMessage::new(
                "budget",
                Speaker::Person,
                "We have 18,000 for the kitchen. Cabinets are quoted at 7,400 and the counter at \
                 3,100. What is left for appliances once labour is in?",
                NOW - 12 * MINUTE,
            ),
            ThreadMessage::new("left", Speaker::Assistant, take.clone(), NOW - 11 * MINUTE)
                .content(
                    v_flex()
                        .gap_2()
                        .child(div().text_sm().child(take.clone()))
                        .child(
                            MessageActionBar::new("left-actions")
                                .text(take)
                                .quiet(false)
                                .versions(TAKES.len())
                                .version(self.take)
                                .branch_map(true)
                                .branch_labels(["First answer", "Shorter", "With the sums"])
                                .reasons(["Wrong numbers", "Too long", "Missed the question"])
                                .on_version_change(move |take, _, cx| {
                                    story.update(cx, |this, cx| {
                                        this.take = take;
                                        cx.notify();
                                    })
                                }),
                        ),
                ),
            ThreadMessage::new(
                "dishwasher",
                Speaker::Person,
                "Can we keep the old dishwasher and put the difference into an induction hob?",
                NOW - 3 * MINUTE,
            ),
            ThreadMessage::new("hob", Speaker::Assistant, ANSWER, NOW - 2 * MINUTE)
                .content(self.answer.clone()),
        ]
    }
}

impl Story for ChatScreenStory {
    fn title() -> &'static str {
        "Chat screen"
    }

    fn description() -> &'static str {
        "A chat app built from the library: conversations, a thread with a reasoned answer and branches, the model, and a composer with modes and context."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ChatScreenStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let messages = self.messages(cx);
        let theme = cx.theme();
        let line = theme.border;
        let quiet = theme.muted_foreground;
        let sidebar = theme.sidebar;

        let header = h_flex()
            .flex_none()
            .h(px(52.))
            .px_4()
            .gap_3()
            .justify_between()
            .border_b_1()
            .border_color(line)
            .child(
                v_flex()
                    .min_w_0()
                    .child(
                        div()
                            .text_sm()
                            .font_medium()
                            .child("Kitchen renovation budget"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(quiet)
                            .child("Four turns, three files in context"),
                    ),
            )
            .child(
                ModelPicker::new("screen-model")
                    .columns(["Speed", "Depth"])
                    .model(
                        ModelOption::new("steady", "Steady")
                            .tagline("Everyday questions")
                            .ratings([4, 3]),
                    )
                    .model(
                        ModelOption::new("careful", "Careful")
                            .tagline("Sums and long plans")
                            .ratings([2, 5]),
                    )
                    .model(
                        ModelOption::new("brisk", "Brisk")
                            .tagline("Quick lookups")
                            .ratings([5, 2]),
                    )
                    .default_value("careful")
                    .setting(
                        PickerSetting::new("reason", "Reason it through")
                            .icon(IconName::Cpu)
                            .default_on(true),
                    ),
            );

        let conversation = v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(header)
            .child(
                div().flex_1().min_h_0().child(
                    ChatThread::new("screen-thread", messages)
                        .bare()
                        .assistant_name("Assistant")
                        .person_name("Maya")
                        .now(NOW, 0),
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

        window_frame(cx).child(
            h_flex()
                .size_full()
                .child(
                    v_flex()
                        .flex_none()
                        .w(px(268.))
                        .h_full()
                        .p_2()
                        .bg(sidebar)
                        .border_r_1()
                        .border_color(line)
                        .child(self.list.clone()),
                )
                .child(conversation),
        )
    }
}
