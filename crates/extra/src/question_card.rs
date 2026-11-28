// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Question card: an assistant's question, answered in one keystroke.
//!
//! The card sits above a chat composer and holds the questions that wait for
//! an answer. Each one offers numbered rows, with at most one recommended, or
//! Approve and Decline, and a last row for an answer in your own words. A
//! click or a number key answers at once and brings up the next question;
//! "Not now" folds the card to one line.
//!
//! A question can lay its options out as cards in a grid instead of rows,
//! where the arrows move across and down. The recommended option can say why
//! it is recommended. A question can take several answers: then a click, a
//! number key or Space ticks an option, and Enter or Send answers with every
//! option ticked.

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
    ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex,
    input::{Input, InputEvent, InputState},
    look, v_flex,
};
use isonomic_ui::{
    AppContext as _, BoxShadow, Context, Entity, EventEmitter, FocusHandle, Focusable as _,
    InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton, ParentElement as _, Render,
    Role, SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div,
    hsla, point, prelude::FluentBuilder as _, px,
};

use crate::common::resting_shadow;

/// One of the answers a question offers.
#[derive(Debug, Clone)]
pub struct QuestionOption {
    id: SharedString,
    title: SharedString,
    summary: Option<SharedString>,
    recommended: bool,
    reason: Option<SharedString>,
}

impl QuestionOption {
    /// Create an option.
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            summary: None,
            recommended: false,
            reason: None,
        }
    }

    /// Say why this is the suggested answer. It shows under the option and
    /// marks it as recommended.
    pub fn reason(mut self, reason: impl Into<SharedString>) -> Self {
        self.reason = Some(reason.into());
        self.recommended = true;
        self
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
    layout: QuestionLayout,
    multiple: bool,
}

/// How a question lays out its options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuestionLayout {
    /// One numbered row each, top to bottom. The default.
    #[default]
    Rows,
    /// Cards in a grid of two columns, for options that need a few words
    /// each to weigh.
    Cards,
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
            layout: QuestionLayout::Rows,
            multiple: false,
        }
    }

    /// Lay the options out as rows (the default) or as cards in a grid.
    pub fn layout(mut self, layout: QuestionLayout) -> Self {
        self.layout = layout;
        self
    }

    /// Let several options be picked: a click, a number key or Space ticks
    /// an option, and Enter or Send answers with every one ticked.
    pub fn multiple(mut self, multiple: bool) -> Self {
        self.multiple = multiple;
        self
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
    /// Several options, by id, in the order they are offered.
    Options(Vec<SharedString>),
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
    reason: Option<SharedString>,
}

/// The rows a question offers, in number-key order.
fn rows(question: &Question) -> Vec<Row> {
    if question.approval {
        let row = |answer, title: &'static str| Row {
            answer,
            title: title.into(),
            summary: None,
            recommended: false,
            reason: None,
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
            reason: option.reason.clone(),
        })
        .collect()
}

/// How many columns the cards layout has.
const CARD_COLUMNS: usize = 2;

/// Where an arrow key moves to among cards laid out `columns` across:
/// Left and Right step one, Up and Down a whole line, and Down from the
/// last line goes to the field.
fn card_target(selected: usize, rows: usize, columns: usize, key: &str) -> Option<Target> {
    let columns = columns.max(1);
    match key {
        "right" if selected + 1 < rows => Some(Target::Row(selected + 1)),
        "right" => Some(Target::Row(selected)),
        "left" => Some(Target::Row(selected.saturating_sub(1))),
        "down" if selected + columns < rows => Some(Target::Row(selected + columns)),
        // A last line that is not full: go down to its last card first.
        "down" if selected / columns < (rows.saturating_sub(1)) / columns => {
            Some(Target::Row(rows - 1))
        }
        "down" => Some(Target::Field),
        "up" => Some(Target::Row(selected.saturating_sub(columns))),
        "home" => Some(Target::Row(0)),
        "end" => Some(Target::Row(rows.saturating_sub(1))),
        _ => None,
    }
}

/// The ticked options after ticking or unticking `ix`, kept in order.
fn toggle_ticked(ticked: &[usize], ix: usize) -> Vec<usize> {
    let mut next: Vec<usize> = ticked.iter().copied().filter(|t| *t != ix).collect();
    if next.len() == ticked.len() {
        next.push(ix);
        next.sort_unstable();
    }
    next
}

/// The answer a multiple-choice question gives with these options ticked.
fn ticked_answer(rows: &[Row], ticked: &[usize]) -> Option<QuestionAnswer> {
    let ids: Vec<SharedString> = ticked
        .iter()
        .filter_map(|ix| match &rows.get(*ix)?.answer {
            QuestionAnswer::Option(id) => Some(id.clone()),
            _ => None,
        })
        .collect();
    (!ids.is_empty()).then_some(QuestionAnswer::Options(ids))
}

/// What a screen reader hears for an option: its number and title, whether
/// it is suggested and why, whether it is ticked, and its summary.
fn row_words(ix: usize, row: &Row, ticked: Option<bool>) -> String {
    let mut words = format!("{}. {}", ix + 1, row.title);
    if row.recommended {
        words.push_str(", recommended");
        if let Some(reason) = &row.reason {
            words.push_str(&format!(" because {reason}"));
        }
    }
    match ticked {
        Some(true) => words.push_str(", ticked"),
        Some(false) => words.push_str(", not ticked"),
        None => {}
    }
    if let Some(summary) = &row.summary {
        words.push_str(&format!(". {summary}"));
    }
    words
}

/// What Send says on a multiple-choice question.
fn send_label(ticked: usize) -> String {
    match ticked {
        0 => "Send".into(),
        1 => "Send 1 choice".into(),
        n => format!("Send {n} choices"),
    }
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
    /// The options ticked on a question that takes several answers.
    ticked: Vec<usize>,
    hovered: Option<usize>,
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
            ticked: Vec::new(),
            hovered: None,
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
        self.ticked.clear();
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

    fn tick(&mut self, ix: usize, cx: &mut Context<Self>) {
        self.ticked = toggle_ticked(&self.ticked, ix);
        self.selected = ix;
        cx.notify();
    }

    fn send_ticked(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(question) = self.questions.get(self.current) else {
            return;
        };
        if let Some(answer) = ticked_answer(&rows(question), &self.ticked) {
            self.answer(answer, window, cx);
        }
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
        let multiple = question.multiple && !question.approval;
        let arrows = match question.layout {
            QuestionLayout::Rows => arrow_target(selected, rows.len(), key),
            QuestionLayout::Cards => card_target(selected, rows.len(), CARD_COLUMNS, key),
        };
        if let Some(target) = digit_target(rows.len(), key) {
            match target {
                Target::Row(ix) if multiple => self.tick(ix, cx),
                Target::Row(ix) => self.answer(rows[ix].answer.clone(), window, cx),
                Target::Field => self.go(Target::Field, window, cx),
            }
        } else if on_card && multiple && key == "space" && !rows.is_empty() {
            self.tick(selected, cx);
        } else if on_card && multiple && key == "enter" {
            if self.ticked.is_empty() {
                self.go(Target::Field, window, cx);
            } else {
                self.send_ticked(window, cx);
            }
        } else if on_card && matches!(key, "enter" | "space") {
            match rows.get(selected) {
                Some(row) => self.answer(row.answer.clone(), window, cx),
                None => self.go(Target::Field, window, cx),
            }
        } else if let Some(target) = arrows {
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
        let chip = isonomic_ui::component::look::ink::wash_strong(cx);
        let subtle = isonomic_ui::component::look::ink::subtle(cx);
        let warning = theme.warning;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;
        let accent = theme.info;
        let line = theme.border;

        let count = self.questions.len();
        let Some(question) = self.questions.get(self.current) else {
            // Nothing waits: the card takes no room.
            return div().into_any_element();
        };
        let card = v_flex()
            .id("question-card")
            .w_full()
            .max_w(px(600.))
            .rounded(if radius_lg > px(0.) {
                radius_lg + px(6.)
            } else {
                px(0.)
            })
            .bg(fill)
            .text_color(ink)
            .text_body()
            // A question waits above the page: the raised edge plus a long,
            // soft drop.
            .shadow({
                let mut shadow = resting_shadow(cx);
                shadow.push(BoxShadow {
                    color: hsla(0., 0., 0., if theme.mode.is_dark() { 0.5 } else { 0.16 }),
                    offset: point(px(0.), px(10.)),
                    blur_radius: px(14.),
                    spread_radius: px(-12.),
                    inset: false,
                });
                shadow
            });

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
                .size(px(22.))
                .rounded(px(6.))
                .bg(chip)
                .text_caption()
                .font_medium()
                .text_color(subtle)
                .child(n.to_string())
        };

        let multiple = question.multiple && !question.approval;
        let cards = question.layout == QuestionLayout::Cards;
        let fades: Vec<f32> = (0..rows.len())
            .map(|ix| look::hover_fade(("row", ix), self.hovered == Some(ix), window, cx))
            .collect();
        let mut row_elements = Vec::new();
        for (ix, row) in rows.iter().enumerate() {
            let answer = row.answer.clone();
            let ticked = self.ticked.contains(&ix);
            let focused = ix == selected && on_rows;
            // The tick box stands in for the number on a question that takes
            // several answers; the card holds the keyboard, not the box.
            let lead = if multiple {
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(22.))
                    .child(
                        Checkbox::new(("tick", ix))
                            .checked(ticked)
                            .tab_stop(false)
                            .accessibility_label(row.title.clone())
                            .on_click(cx.listener(move |this, _, _, cx| this.tick(ix, cx))),
                    )
                    .into_any_element()
            } else {
                number(ix + 1).into_any_element()
            };
            let reason = row.reason.clone().map(|reason| {
                h_flex()
                    .gap_1()
                    .items_start()
                    .text_caption()
                    .text_color(accent)
                    .child(
                        div()
                            .flex_none()
                            .pt(px(2.))
                            .child(Icon::new(IconName::Star).xsmall()),
                    )
                    .child(div().min_w_0().child(reason))
            });
            let badge = row.recommended.then(|| {
                div()
                    .flex_none()
                    .px_1p5()
                    .rounded(px(5.))
                    .bg(chip)
                    .text_color(subtle)
                    .text_badge()
                    .line_height(px(18.))
                    .child("Recommended")
            });
            let base = div()
                .id(("row", ix))
                .role(Role::Button)
                .aria_label(row_words(ix, row, multiple.then_some(ticked)))
                .cursor_pointer()
                .rounded(radius + px(2.))
                .bg(look::mix(chip.opacity(0.), chip, fades[ix]))
                .when(focused, |this| this.bg(chip).focus_ring_style(window, cx))
                .on_hover(cx.listener(move |this, over: &bool, _, cx| {
                    let next = if *over {
                        Some(ix)
                    } else if this.hovered == Some(ix) {
                        None
                    } else {
                        this.hovered
                    };
                    if next != this.hovered {
                        this.hovered = next;
                        cx.notify();
                    }
                }))
                .on_click(cx.listener(move |this, _, window, cx| {
                    if multiple {
                        this.tick(ix, cx);
                    } else {
                        this.answer(answer.clone(), window, cx);
                    }
                }));
            let element = if cards {
                base.flex()
                    .flex_col()
                    .gap_1()
                    .p_2p5()
                    .border_1()
                    .border_color(if row.recommended || ticked {
                        accent
                    } else {
                        line
                    })
                    .child(
                        h_flex()
                            .gap_2()
                            .child(lead)
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .font_medium()
                                    .child(row.title.clone()),
                            )
                            .children(badge),
                    )
                    .when_some(row.summary.clone(), |this, summary| {
                        this.child(div().text_caption().text_color(subtle).child(summary))
                    })
                    .children(reason)
                    .into_any_element()
            } else {
                base.flex()
                    .items_center()
                    .gap_3()
                    .min_h(px(44.))
                    .pl(px(6.))
                    .pr_2()
                    .py_1()
                    .child(lead)
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().font_medium().child(row.title.clone()))
                            .when_some(row.summary.clone(), |this, summary| {
                                this.child(
                                    div()
                                        .truncate()
                                        .text_caption()
                                        .text_color(subtle)
                                        .child(summary),
                                )
                            })
                            .children(reason),
                    )
                    .children(badge)
                    .into_any_element()
            };
            row_elements.push(element);
        }

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
                        .text_caption()
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
            .aria_description(if multiple {
                "Press a number or Space to tick an answer, then Enter to send."
            } else {
                "Press a number to answer, or the arrows then Enter."
            })
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
                            .bg(warning.opacity(0.16))
                            .text_color(warning)
                            .text_badge()
                            .line_height(px(18.))
                            .child("Your call"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_caption()
                            .line_height(px(20.))
                            .text_color(subtle)
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
            .child(
                div()
                    .text_size(px(14.))
                    .line_height(px(20.))
                    .font_medium()
                    .child(question.text.clone()),
            )
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
                v_flex()
                    .gap_1()
                    .map(|this| {
                        if cards {
                            this.child(
                                div()
                                    .grid()
                                    .grid_cols(CARD_COLUMNS as u16)
                                    .gap_2()
                                    .children(row_elements),
                            )
                        } else {
                            this.child(v_flex().children(row_elements))
                        }
                    })
                    .child(
                        h_flex()
                            .gap_1()
                            .pl_2()
                            .child(number(rows.len() + 1))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(Input::new(&self.input).appearance(false)),
                            )
                            .when(multiple, |this| {
                                this.child(
                                    Button::new("send-ticked")
                                        .primary()
                                        .small()
                                        .label(send_label(self.ticked.len()))
                                        .disabled(self.ticked.is_empty())
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.send_ticked(window, cx)
                                        })),
                                )
                            }),
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

    #[test]
    fn arrows_move_across_and_down_the_cards() {
        // Five cards, two across: 0 1 / 2 3 / 4.
        assert_eq!(card_target(0, 5, 2, "right"), Some(Target::Row(1)));
        assert_eq!(card_target(4, 5, 2, "right"), Some(Target::Row(4)));
        assert_eq!(card_target(1, 5, 2, "left"), Some(Target::Row(0)));
        assert_eq!(card_target(1, 5, 2, "down"), Some(Target::Row(3)));
        // The last line is short: down from the card above its gap.
        assert_eq!(card_target(3, 5, 2, "down"), Some(Target::Row(4)));
        assert_eq!(card_target(4, 5, 2, "down"), Some(Target::Field));
        assert_eq!(card_target(3, 5, 2, "up"), Some(Target::Row(1)));
        assert_eq!(card_target(1, 5, 2, "up"), Some(Target::Row(0)));
        assert_eq!(card_target(2, 5, 2, "x"), None);
    }

    #[test]
    fn ticking_keeps_the_options_in_order() {
        assert_eq!(toggle_ticked(&[], 2), [2]);
        assert_eq!(toggle_ticked(&[2], 0), [0, 2]);
        assert_eq!(toggle_ticked(&[0, 2], 2), [0]);
    }

    #[test]
    fn several_ticked_options_answer_together() {
        let question = Question::new("which", "Which?")
            .multiple(true)
            .option(QuestionOption::new("a", "A"))
            .option(QuestionOption::new("b", "B"))
            .option(QuestionOption::new("c", "C"));
        let rows = rows(&question);
        assert_eq!(ticked_answer(&rows, &[]), None);
        assert_eq!(
            ticked_answer(&rows, &[0, 2]),
            Some(QuestionAnswer::Options(vec!["a".into(), "c".into()]))
        );
        assert_eq!(send_label(0), "Send");
        assert_eq!(send_label(1), "Send 1 choice");
        assert_eq!(send_label(3), "Send 3 choices");
    }

    #[test]
    fn a_suggestion_says_why_and_whether_it_is_ticked() {
        let question = Question::new("where", "Where to?").option(
            QuestionOption::new("a", "Bank")
                .summary("Same day")
                .reason("it matches last month"),
        );
        let row = &rows(&question)[0];
        assert!(row.recommended);
        assert_eq!(
            row_words(0, row, None),
            "1. Bank, recommended because it matches last month. Same day"
        );
        assert_eq!(
            row_words(0, row, Some(true)),
            "1. Bank, recommended because it matches last month, ticked. Same day"
        );
    }
}
