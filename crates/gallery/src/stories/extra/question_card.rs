// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the question card.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, button::Button, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div,
};
use isonomic_ui_extra::question_card::{
    Question, QuestionAnswer, QuestionCard, QuestionCardEvent, QuestionLayout, QuestionOption,
};

fn samples() -> Vec<Question> {
    vec![
        Question::approval("reminder", "Send the payment reminder to Harbour Supply?")
            .task("Overdue invoices")
            .fact("To", "accounts@harbour.example")
            .fact("Invoice", "No. 2291, 14 days overdue")
            .fact("Tone", "Friendly, second reminder"),
        Question::new("accrual", "How should the unbilled March hours be booked?")
            .task("Month-end close")
            .option(
                QuestionOption::new("accrue", "Accrue them in March")
                    .summary("Revenue lands in the month the work was done")
                    .reason("You accrued the same way in February"),
            )
            .option(
                QuestionOption::new("defer", "Leave them for April")
                    .summary("Simpler, but March looks weaker than it was"),
            )
            .option(
                QuestionOption::new("split", "Split by project")
                    .summary("Accrue fixed-fee work, defer hourly work"),
            ),
    ]
}

/// Questions for the second card: choice cards with a marked suggestion,
/// then one that takes several answers.
fn agent_samples() -> Vec<Question> {
    vec![
        Question::new("layout", "Which report layout should the board pack use?")
            .task("Quarterly board pack")
            .layout(QuestionLayout::Cards)
            .option(
                QuestionOption::new("one-page", "One page")
                    .summary("Headline numbers and three charts")
                    .reason("The board asked for shorter packs last quarter"),
            )
            .option(
                QuestionOption::new("detailed", "Detailed")
                    .summary("Every account, with notes on each change"),
            )
            .option(
                QuestionOption::new("by-team", "By team").summary("One section per budget holder"),
            )
            .option(
                QuestionOption::new("compare", "Year on year")
                    .summary("This quarter beside the same quarter last year"),
            ),
        Question::new("checks", "Which checks should run before the close?")
            .task("Month-end close")
            .multiple(true)
            .option(
                QuestionOption::new("bank", "Match the bank feed")
                    .summary("214 lines since the last match")
                    .reason("Three payments are still unmatched"),
            )
            .option(QuestionOption::new("payroll", "Check payroll totals"))
            .option(
                QuestionOption::new("fx", "Revalue foreign balances")
                    .summary("Two accounts in other currencies"),
            ),
    ]
}

fn answer_words(question: &SharedString, answer: &QuestionAnswer) -> String {
    match answer {
        QuestionAnswer::Approve => format!("{question}: approved"),
        QuestionAnswer::Decline => format!("{question}: declined"),
        QuestionAnswer::Option(option) => format!("{question}: {option}"),
        QuestionAnswer::Options(options) => format!("{question}: {}", options.join(", ")),
        QuestionAnswer::Text(text) => format!("{question}: \"{text}\""),
    }
}

/// Shows a card with an approval and a choice waiting, and reports each answer.
pub struct QuestionCardStory {
    card: Entity<QuestionCard>,
    /// Choice cards and a question with several answers.
    agent_card: Entity<QuestionCard>,
    agent_last: SharedString,
    _agent_events: Subscription,
    last: SharedString,
    _events: Subscription,
}

impl QuestionCardStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let card = cx.new(|cx| QuestionCard::new(window, cx).questions(samples()));
            let events = cx.subscribe(&card, |this: &mut Self, _, event, cx| {
                this.last = match event {
                    QuestionCardEvent::Later(question) => format!("{question}: later"),
                    QuestionCardEvent::Answered { question, answer } => {
                        answer_words(question, answer)
                    }
                }
                .into();
                cx.notify();
            });

            let agent_card = cx.new(|cx| QuestionCard::new(window, cx).questions(agent_samples()));
            let agent_events = cx.subscribe(&agent_card, |this: &mut Self, _, event, cx| {
                this.agent_last = match event {
                    QuestionCardEvent::Later(question) => format!("{question}: later"),
                    QuestionCardEvent::Answered { question, answer } => {
                        answer_words(question, answer)
                    }
                }
                .into();
                cx.notify();
            });

            Self {
                card,
                agent_card,
                agent_last: "Arrows move across the cards; Space ticks, Enter sends".into(),
                _agent_events: agent_events,
                last: "Answer with a click or a number key".into(),
                _events: events,
            }
        })
    }
}

impl Story for QuestionCardStory {
    fn title() -> &'static str {
        "Question card"
    }

    fn description() -> &'static str {
        "An assistant's question, answered in one keystroke."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for QuestionCardStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let card = self.card.clone();

        v_flex()
            .gap_3()
            .child(self.card.clone())
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.last.clone()),
            )
            .child(
                div().child(
                    Button::new("ask-again")
                        .outline()
                        .label("Ask both again")
                        .on_click(move |_, window, cx| {
                            card.update(cx, |card, cx| {
                                for question in samples() {
                                    card.push(question, cx);
                                }
                                card.reveal(window, cx);
                            })
                        }),
                ),
            )
            .child(
                div()
                    .pt_4()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Choice cards with a marked suggestion, then several answers at once"),
            )
            .child(self.agent_card.clone())
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.agent_last.clone()),
            )
    }
}
