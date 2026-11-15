//! Question card: an assistant's question, answered in one keystroke.
//!
//! The card sits above a chat composer and holds the questions that wait for
//! an answer. Each one offers numbered rows, with at most one recommended, or
//! Approve and Decline, and a last row for an answer in your own words. A
//! click or a number key answers at once and brings up the next question;
//! "Not now" folds the card to one line.

use isonomic_ui::component::{
    ActiveTheme as _, IconName, Sizable as _, StyledExt as _, ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
};
use isonomic_ui::{
    AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable as _,
    InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton, ParentElement as _, Render,
    Role, SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// One of the answers a question offers.
#[derive(Debug, Clone)]
pub struct QuestionOption {
    id: SharedString,
    title: SharedString,
    summary: Option<SharedString>,
    recommended: bool,
}

impl QuestionOption {
    /// Create an option.
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            summary: None,
            recommended: false,
        }
    }

    /// One line on what choosing this means.
    pub fn summary(mut self, summary: impl Into<SharedString>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    /// Mark this as the suggested answer. Mark at most one.
    pub fn recommended(mut self, recommended: bool) -> Self {
        self.recommended = recommended;
        self
    }
}

/// A question that waits for an answer.
#[derive(Debug, Clone)]
pub struct Question {
    id: SharedString,
    text: SharedString,
    task: Option<SharedString>,
    approval: bool,
    options: Vec<QuestionOption>,
    facts: Vec<(SharedString, SharedString)>,
}

impl Question {
    /// Create a question answered by picking one of its options.
    pub fn new(id: impl Into<SharedString>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            task: None,
            approval: false,
            options: Vec::new(),
            facts: Vec::new(),
        }
    }

    /// Create a question answered with Approve or Decline.
    pub fn approval(id: impl Into<SharedString>, text: impl Into<SharedString>) -> Self {
        Self {
            approval: true,
            ..Self::new(id, text)
        }
    }

    /// Name the task the question belongs to.
    pub fn task(mut self, task: impl Into<SharedString>) -> Self {
        self.task = Some(task.into());
        self
    }

    /// Add an option. Three to five read best; an approval takes none.
    pub fn option(mut self, option: QuestionOption) -> Self {
        self.options.push(option);
        self
    }

    /// Add a labelled fact to weigh, such as who a message goes to.
    pub fn fact(mut self, label: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        self.facts.push((label.into(), value.into()));
        self
    }
}

/// How a question was answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestionAnswer {
    /// One of the options, by id.
    Option(SharedString),
    /// An answer in the person's own words.
    Text(SharedString),
    /// An approval was given.
    Approve,
    /// An approval was refused.
    Decline,
}

/// What the card tells its owner.
#[derive(Debug, Clone)]
pub enum QuestionCardEvent {
    /// A question was answered and has left the card.
    Answered {
        /// The id of the question.
        question: SharedString,
        /// The answer given.
        answer: QuestionAnswer,
    },
    /// The card was folded away with this question showing.
    Later(SharedString),
}

/// A numbered row of the question that is showing.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Row {
    answer: QuestionAnswer,
    title: SharedString,
    summary: Option<SharedString>,
    recommended: bool,
}

/// The rows a question offers, in number-key order.
fn rows(question: &Question) -> Vec<Row> {
    if question.approval {
        let row = |answer, title: &'static str| Row {
            answer,
            title: title.into(),
            summary: None,
            recommended: false,
        };
        return vec![
            row(QuestionAnswer::Approve, "Approve"),
            row(QuestionAnswer::Decline, "Decline"),
        ];
    }
    question
        .options
        .iter()
        .map(|option| Row {
            answer: QuestionAnswer::Option(option.id.clone()),
            title: option.title.clone(),
            summary: option.summary.clone(),
            recommended: option.recommended,
        })
        .collect()
}

/// Where a key press lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Row(usize),
    /// The field for an answer in your own words.
    Field,
}

/// What a number key answers with, among `rows` rows: the row with that
/// number, or the field for the number after the last row.
fn digit_target(rows: usize, key: &str) -> Option<Target> {
    let number: usize = key.parse().ok()?;
    match number {
        0 => None,
        n if n <= rows => Some(Target::Row(n - 1)),
        n if n == rows + 1 => Some(Target::Field),
        _ => None,
    }
}

/// Where an arrow key moves to from the selected row.
fn arrow_target(selected: usize, rows: usize, key: &str) -> Option<Target> {
    match key {
        "down" if selected + 1 < rows => Some(Target::Row(selected + 1)),
        "down" => Some(Target::Field),
        "up" => Some(Target::Row(selected.saturating_sub(1))),
        "home" => Some(Target::Row(0)),
        "end" => Some(Target::Row(rows.saturating_sub(1))),
        _ => None,
    }
}

/// The folded line: how many questions wait.
fn waiting_label(count: usize) -> String {
    match count {
        1 => "1 question waiting".into(),
        n => format!("{n} questions waiting"),
    }
}

/// The questions that wait for an answer, one showing at a time.
pub struct QuestionCard {
    questions: Vec<Question>,
    current: usize,
    /// The row that Enter answers with.
    selected: usize,
    folded: bool,
    input: Entity<InputState>,
    focus: FocusHandle,
    _input_events: Subscription,
}

impl EventEmitter<QuestionCardEvent> for QuestionCard {}

impl QuestionCard {
    /// Create an empty card. It draws nothing until it has a question.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Or write your own…"));
        let input_events = cx.subscribe_in(&input, window, |this, input, event, window, cx| {
            if let InputEvent::PressEnter { .. } = event {
                let text = input.read(cx).value().trim().to_string();
                if !text.is_empty() {
                    this.answer(QuestionAnswer::Text(text.into()), window, cx);
                }
            }
        });

        Self {
            questions: Vec::new(),
            current: 0,
            selected: 0,
            folded: false,
            input,
            // A Tab stop of its own, so the keys below work without a mouse.
            focus: cx.focus_handle().tab_stop(true),
            _input_events: input_events,
        }
    }

    /// Start with these questions, oldest first.
    pub fn questions(mut self, questions: impl IntoIterator<Item = Question>) -> Self {
        self.questions = questions.into_iter().collect();
        self
    }

    /// Start folded to one line.
    pub fn folded(mut self, folded: bool) -> Self {
        self.folded = folded;
        self
    }

    /// The questions still waiting, oldest first.
    pub fn open(&self) -> &[Question] {
        &self.questions
    }

    /// Add a question behind the ones already waiting.
    pub fn push(&mut self, question: Question, cx: &mut Context<Self>) {
        self.questions.push(question);
        cx.notify();
    }

    /// Take a question off the card without an answer.
    pub fn dismiss(&mut self, id: &str, cx: &mut Context<Self>) {
        self.questions.retain(|question| question.id.as_ref() != id);
        self.show(self.current, cx);
    }

    /// Unfold the card and put the keyboard on its selected row.
    pub fn reveal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.folded = false;
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn show(&mut self, ix: usize, cx: &mut Context<Self>) {
        self.current = ix.min(self.questions.len().saturating_sub(1));
        self.selected = 0;
        cx.notify();
    }

    fn answer(&mut self, answer: QuestionAnswer, window: &mut Window, cx: &mut Context<Self>) {
        if self.current >= self.questions.len() {
            return;
        }
        let question = self.questions.remove(self.current);
        self.input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.show(self.current, cx);
        // Keep the keyboard on the card for the next question.
        if !self.questions.is_empty() {
            self.focus.focus(window, cx);
        }
        cx.emit(QuestionCardEvent::Answered {
            question: question.id,
            answer,
        });
    }

    fn later(&mut self, cx: &mut Context<Self>) {
        let Some(question) = self.questions.get(self.current) else {
            return;
        };
        let id = question.id.clone();
        self.folded = true;
        cx.emit(QuestionCardEvent::Later(id));
        cx.notify();
    }

    fn go(&mut self, target: Target, window: &mut Window, cx: &mut Context<Self>) {
        match target {
            Target::Row(ix) => {
                self.selected = ix;
                self.focus.focus(window, cx);
            }
            Target::Field => self.input.read(cx).focus_handle(cx).focus(window, cx),
        }
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.folded || event.keystroke.modifiers.modified() {
            return;
        }
        let Some(question) = self.questions.get(self.current) else {
            return;
        };
        let rows = rows(question);
        let key = event.keystroke.key.as_str();

        if key == "escape" {
            self.later(cx);
            cx.stop_propagation();
            return;
        }
        if self.input.read(cx).focus_handle(cx).is_focused(window) {
            // Up from an empty field goes back to the rows; anything else is typing.
            if key == "up" && self.input.read(cx).value().is_empty() && !rows.is_empty() {
                self.go(Target::Row(rows.len() - 1), window, cx);
                cx.stop_propagation();
            }
            return;
        }
        // Number keys and arrows work from anywhere in the card; Enter and
        // Space only on the card itself, since a focused button takes them.
        let on_card = self.focus.is_focused(window);
        if !on_card && !self.focus.contains_focused(window, cx) {
            return;
        }

        let selected = self.selected.min(rows.len().saturating_sub(1));
        if let Some(target) = digit_target(rows.len(), key) {
            match target {
                Target::Row(ix) => self.answer(rows[ix].answer.clone(), window, cx),
                Target::Field => self.go(Target::Field, window, cx),
            }
        } else if on_card && matches!(key, "enter" | "space") {
            match rows.get(selected) {
                Some(row) => self.answer(row.answer.clone(), window, cx),
                None => self.go(Target::Field, window, cx),
            }
        } else if let Some(target) = arrow_target(selected, rows.len(), key) {
            self.go(target, window, cx);
        } else {
            return;
        }
        cx.stop_propagation();
    }
}

impl Render for QuestionCard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.info;
        let warning = theme.warning;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;

        let count = self.questions.len();
        let Some(question) = self.questions.get(self.current) else {
            // Nothing waits: the card takes no room.
            return div().into_any_element();
        };
        let card = v_flex()
            .id("question-card")
            .w_full()
            .max_w(px(600.))
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .line_height(relative(1.45))
            .shadow(resting_shadow(cx));

        if self.folded {
            return card
                .child(
                    h_flex()
                        .gap_2()
                        .h(px(40.))
                        .pl_3()
                        .pr_2()
                        .child(div().size(px(6.)).rounded_full_style(cx).bg(warning))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_color(quiet)
                                .child(waiting_label(count)),
                        )
                        .child(
                            Button::new("show").ghost().xsmall().label("Show").on_click(
                                cx.listener(|this, _, window, cx| this.reveal(window, cx)),
                            ),
                        ),
                )
                .into_any_element();
        }

        let rows = rows(question);
        let selected = self.selected.min(rows.len().saturating_sub(1));
        let on_rows = self.focus.is_focused(window);
        let number = |n: usize| {
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(20.))
                .rounded(px(5.))
                .border_1()
                .border_color(line)
                .text_xs()
                .text_color(quiet)
                .child(n.to_string())
        };

        let row_elements = rows.iter().enumerate().map(|(ix, row)| {
            let answer = row.answer.clone();
            h_flex()
                .id(("row", ix))
                .role(Role::Button)
                .aria_label(match (&row.summary, row.recommended) {
                    (Some(summary), true) => {
                        format!("{}. {}, recommended. {summary}", ix + 1, row.title)
                    }
                    (Some(summary), false) => format!("{}. {}. {summary}", ix + 1, row.title),
                    (None, true) => format!("{}. {}, recommended", ix + 1, row.title),
                    (None, false) => format!("{}. {}", ix + 1, row.title),
                })
                .gap_3()
                .min_h(px(36.))
                .px_2()
                .py_1p5()
                .rounded(radius)
                .cursor_pointer()
                .hover(|this| this.bg(chip))
                .when(ix == selected && on_rows, |this| {
                    this.bg(chip).focus_ring_style(window, cx)
                })
                .on_click(
                    cx.listener(move |this, _, window, cx| this.answer(answer.clone(), window, cx)),
                )
                .child(number(ix + 1))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(div().font_medium().child(row.title.clone()))
                        .when_some(row.summary.clone(), |this, summary| {
                            this.child(div().truncate().text_xs().text_color(quiet).child(summary))
                        }),
                )
                .when(row.recommended, |this| {
                    this.child(
                        div()
                            .flex_none()
                            .px_1p5()
                            .rounded(px(5.))
                            .bg(accent.opacity(0.14))
                            .text_xs()
                            .child("Recommended"),
                    )
                })
        });
        let row_elements: Vec<_> = row_elements.collect();

        let pager = (count > 1).then(|| {
            let current = self.current;
            h_flex()
                .flex_none()
                .gap_0p5()
                .child(
                    Button::new("previous")
                        .ghost()
                        .xsmall()
                        .icon(IconName::ChevronLeft)
                        .accessibility_label("Previous question")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.show(current.saturating_sub(1), cx)
                        })),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(quiet)
                        .child(format!("{} of {count}", current + 1)),
                )
                .child(
                    Button::new("next")
                        .ghost()
                        .xsmall()
                        .icon(IconName::ChevronRight)
                        .accessibility_label("Next question")
                        .on_click(cx.listener(move |this, _, _, cx| this.show(current + 1, cx))),
                )
        });

        card.track_focus(&self.focus)
            .role(Role::Group)
            .aria_label(format!("Question: {}", question.text))
            .aria_description("Press a number to answer, or Up and Down then Enter.")
            .on_key_down(cx.listener(Self::key_down))
            // A click anywhere but in the field puts the keyboard on the card.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    if !this.input.read(cx).focus_handle(cx).is_focused(window) {
                        this.focus.focus(window, cx);
                    }
                }),
            )
            .gap_2()
            .p_3()
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_none()
                            .px_1p5()
                            .rounded(px(5.))
                            .bg(warning.opacity(0.18))
                            .text_xs()
                            .font_medium()
                            .child("Your call"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_xs()
                            .text_color(quiet)
                            .children(question.task.clone()),
                    )
                    .children(pager)
                    .child(
                        Button::new("later")
                            .ghost()
                            .xsmall()
                            .label("Not now")
                            .on_click(cx.listener(|this, _, _, cx| this.later(cx))),
                    ),
            )
            .child(div().text_base().font_medium().child(question.text.clone()))
            .when(!question.facts.is_empty(), |this| {
                this.child(v_flex().gap_0p5().children(question.facts.iter().map(
                    |(label, value)| {
                        h_flex()
                            .items_start()
                            .gap_3()
                            .child(
                                div()
                                    .flex_none()
                                    .w(px(72.))
                                    .truncate()
                                    .text_color(quiet)
                                    .child(label.clone()),
                            )
                            .child(div().flex_1().min_w_0().truncate().child(value.clone()))
                    },
                )))
            })
            .child(
                v_flex().children(row_elements).child(
                    h_flex().gap_1().pl_2().child(number(rows.len() + 1)).child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Input::new(&self.input).appearance(false)),
                    ),
                ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_approval_offers_approve_and_decline() {
        let question = Question::approval("send", "Send it?")
            .option(QuestionOption::new("ignored", "Ignored"));
        let answers: Vec<_> = rows(&question).into_iter().map(|row| row.answer).collect();
        assert_eq!(answers, [QuestionAnswer::Approve, QuestionAnswer::Decline]);
    }

    #[test]
    fn a_choice_offers_its_options_in_order() {
        let question = Question::new("where", "Where to?")
            .option(QuestionOption::new("a", "First").recommended(true))
            .option(QuestionOption::new("b", "Second").summary("The slow way"));
        let rows = rows(&question);
        assert_eq!(rows[0].answer, QuestionAnswer::Option("a".into()));
        assert!(rows[0].recommended);
        assert_eq!(rows[1].summary.as_deref(), Some("The slow way"));
    }

    #[test]
    fn number_keys_pick_a_row_or_the_field() {
        assert_eq!(digit_target(3, "1"), Some(Target::Row(0)));
        assert_eq!(digit_target(3, "3"), Some(Target::Row(2)));
        // The number after the last row belongs to the field.
        assert_eq!(digit_target(3, "4"), Some(Target::Field));
        assert_eq!(digit_target(3, "5"), None);
        assert_eq!(digit_target(3, "0"), None);
        assert_eq!(digit_target(3, "a"), None);
    }

    #[test]
    fn arrows_move_between_rows_and_into_the_field() {
        assert_eq!(arrow_target(0, 3, "down"), Some(Target::Row(1)));
        assert_eq!(arrow_target(2, 3, "down"), Some(Target::Field));
        assert_eq!(arrow_target(0, 3, "up"), Some(Target::Row(0)));
        assert_eq!(arrow_target(1, 3, "end"), Some(Target::Row(2)));
        assert_eq!(arrow_target(2, 3, "home"), Some(Target::Row(0)));
        assert_eq!(arrow_target(0, 3, "left"), None);
    }

    #[test]
    fn the_folded_line_counts_what_waits() {
        assert_eq!(waiting_label(1), "1 question waiting");
        assert_eq!(waiting_label(3), "3 questions waiting");
    }
}
