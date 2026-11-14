//! Gallery story for the question card.

use gpui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div,
};
use gpui_component::{ActiveTheme as _, button::Button, v_flex};
use isonomic_ui_extra::question_card::{
    Question, QuestionAnswer, QuestionCard, QuestionCardEvent, QuestionOption,
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
                    .recommended(true),
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

/// Shows a card with an approval and a choice waiting, and reports each answer.
pub struct QuestionCardStory {
    card: Entity<QuestionCard>,
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
                    QuestionCardEvent::Answered { question, answer } => match answer {
                        QuestionAnswer::Approve => format!("{question}: approved"),
                        QuestionAnswer::Decline => format!("{question}: declined"),
                        QuestionAnswer::Option(option) => format!("{question}: {option}"),
                        QuestionAnswer::Text(text) => format!("{question}: \"{text}\""),
                    },
                }
                .into();
                cx.notify();
            });

            Self {
                card,
                last: "Answer with a click or a number key".into(),
                _events: events,
            }
        })
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
    }
}
