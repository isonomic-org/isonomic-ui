// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Form card: a short form an assistant hands over in the middle of a
//! conversation.
//!
//! Fields are text, email, number, a longer note, a select, a set of choices,
//! a checkbox or a switch. A field shows its problem once it has been left or
//! a submit was tried. A good submit emits the answers and waits; the host
//! then marks the form submitted, and it folds to a summary with a way back.

use std::collections::{HashMap, HashSet};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IndexPath, Sizable as _, TextRole as _,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    form::{field, v_form},
    h_flex,
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    radio::RadioGroup,
    select::{SearchableVec, Select, SelectEvent, SelectState},
    switch::Switch,
    tag::Tag,
};
use isonomic_ui::{
    AppContext as _, Context, Entity, EventEmitter, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Role, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, div, prelude::FluentBuilder as _, px,
};

use crate::common::{panel, shell, shell_header};

/// What a field collects.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldKind {
    /// One line of text.
    Text,
    /// An email address.
    Email,
    /// A number, with optional bounds.
    Number {
        /// The least allowed.
        min: Option<f64>,
        /// The most allowed.
        max: Option<f64>,
    },
    /// Several lines of text.
    Note,
    /// One of a list, in a menu.
    Select(Vec<SharedString>),
    /// One of a few, all on show.
    Choice(Vec<SharedString>),
    /// A box to tick.
    Checkbox,
    /// A switch.
    Switch,
}

/// One field of the form.
#[derive(Debug, Clone)]
pub struct FormField {
    name: SharedString,
    label: SharedString,
    kind: FieldKind,
    required: bool,
    description: Option<SharedString>,
    placeholder: Option<SharedString>,
}

impl FormField {
    /// A field with the name its answer is reported under.
    pub fn new(
        name: impl Into<SharedString>,
        label: impl Into<SharedString>,
        kind: FieldKind,
    ) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            kind,
            required: false,
            description: None,
            placeholder: None,
        }
    }

    /// It must be answered (or, for a box or switch, be on).
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// A line of help under the field.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Greyed example text for an empty field.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }
}

/// One answer.
#[derive(Debug, Clone, PartialEq)]
pub enum FormValue {
    /// Text, a number as typed, or the chosen option.
    Text(SharedString),
    /// A box or switch.
    Flag(bool),
}

impl FormValue {
    fn is_empty(&self) -> bool {
        match self {
            Self::Text(text) => text.trim().is_empty(),
            Self::Flag(on) => !on,
        }
    }
}

/// The problem with an answer, if it has one.
pub fn problem(field: &FormField, value: &FormValue) -> Option<String> {
    if value.is_empty() {
        return field
            .required
            .then(|| format!("{} is required", field.label));
    }
    let FormValue::Text(text) = value else {
        return None;
    };
    let text = text.trim();
    match &field.kind {
        FieldKind::Email if !looks_like_email(text) => {
            Some("Enter a valid email address".to_string())
        }
        FieldKind::Number { min, max } => match text.parse::<f64>() {
            Err(_) => Some("Enter a number".to_string()),
            Ok(number) if number.is_nan() => Some("Enter a number".to_string()),
            Ok(number) if min.is_some_and(|min| number < min) => {
                Some(format!("Must be at least {}", plain(min.unwrap_or(0.))))
            }
            Ok(number) if max.is_some_and(|max| number > max) => {
                Some(format!("Must be at most {}", plain(max.unwrap_or(0.))))
            }
            Ok(_) => None,
        },
        _ => None,
    }
}

fn plain(number: f64) -> String {
    if number.fract() == 0. {
        format!("{}", number as i64)
    } else {
        number.to_string()
    }
}

/// Something, an at sign, something, a dot, something; no spaces.
fn looks_like_email(text: &str) -> bool {
    let Some((name, host)) = text.split_once('@') else {
        return false;
    };
    let dotted = host
        .rsplit_once('.')
        .is_some_and(|(before, after)| !before.is_empty() && !after.is_empty());
    !name.is_empty() && dotted && !text.contains(char::is_whitespace) && !host.contains('@')
}

/// An answer as the summary shows it.
pub fn shown(value: &FormValue) -> SharedString {
    match value {
        FormValue::Flag(true) => "Yes".into(),
        FormValue::Flag(false) => "No".into(),
        FormValue::Text(text) if text.trim().is_empty() => "—".into(),
        FormValue::Text(text) => text.clone(),
    }
}

/// How many fields need attention, as a sentence.
pub fn attention(count: usize) -> String {
    format!(
        "{count} {} attention",
        if count == 1 {
            "field needs"
        } else {
            "fields need"
        }
    )
}

/// Where the form stands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FormStatus {
    /// Being filled in.
    #[default]
    Editing,
    /// Sent, waiting for the host.
    Submitting,
    /// Accepted: shows the summary.
    Submitted,
}

/// What the form reports.
#[derive(Debug, Clone, PartialEq)]
pub enum FormCardEvent {
    /// A good submit, with every answer by field name, in field order.
    Submit(Vec<(SharedString, FormValue)>),
}

enum Control {
    Line(Entity<InputState>),
    Note(Entity<TextareaState>),
    Menu(Entity<SelectState<SearchableVec<SharedString>>>),
    Choice(Option<usize>),
    Flag(bool),
}

/// A short form inside a conversation.
pub struct FormCard {
    title: SharedString,
    description: Option<SharedString>,
    fields: Vec<FormField>,
    controls: HashMap<SharedString, Control>,
    touched: HashSet<SharedString>,
    attempted: bool,
    status: FormStatus,
    submit_label: SharedString,
    _events: Vec<Subscription>,
}

impl EventEmitter<FormCardEvent> for FormCard {}

impl FormCard {
    /// Create an empty form.
    pub fn new(
        title: impl Into<SharedString>,
        fields: Vec<FormField>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut controls = HashMap::new();
        let mut events = Vec::new();
        for one in &fields {
            let name = one.name.clone();
            let control = match &one.kind {
                FieldKind::Text | FieldKind::Email | FieldKind::Number { .. } => {
                    let placeholder = one.placeholder.clone().unwrap_or_default();
                    let state = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
                    events.push(cx.subscribe(&state, {
                        let name = name.clone();
                        move |this: &mut Self, _, event: &InputEvent, cx| {
                            this.on_input(&name, event, cx)
                        }
                    }));
                    Control::Line(state)
                }
                FieldKind::Note => {
                    let placeholder = one.placeholder.clone().unwrap_or_default();
                    let state = cx.new(|cx| {
                        TextareaState::new(window, cx)
                            .auto_grow(3, 8)
                            .placeholder(placeholder)
                    });
                    events.push(cx.subscribe(&state, {
                        let name = name.clone();
                        move |this: &mut Self, _, event: &InputEvent, cx| {
                            this.on_input(&name, event, cx)
                        }
                    }));
                    Control::Note(state)
                }
                FieldKind::Select(options) => {
                    let items = SearchableVec::new(options.clone());
                    let state = cx.new(|cx| SelectState::new(items, None::<IndexPath>, window, cx));
                    events.push(cx.subscribe(&state, {
                        let name = name.clone();
                        move |this: &mut Self,
                              _,
                              _: &SelectEvent<SearchableVec<SharedString>>,
                              cx| {
                            this.touched.insert(name.clone());
                            cx.notify();
                        }
                    }));
                    Control::Menu(state)
                }
                FieldKind::Choice(_) => Control::Choice(None),
                FieldKind::Checkbox | FieldKind::Switch => Control::Flag(false),
            };
            controls.insert(name, control);
        }
        Self {
            title: title.into(),
            description: None,
            fields,
            controls,
            touched: HashSet::new(),
            attempted: false,
            status: FormStatus::Editing,
            submit_label: "Submit".into(),
            _events: events,
        }
    }

    /// A line under the title.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// The submit button's words. "Submit" by default.
    pub fn submit_label(mut self, label: impl Into<SharedString>) -> Self {
        self.submit_label = label.into();
        self
    }

    /// Where the form stands.
    pub fn status(&self) -> FormStatus {
        self.status
    }

    /// Move the form on: the host calls this with `Submitted` once the
    /// answers are accepted, or `Editing` if sending failed.
    pub fn set_status(&mut self, status: FormStatus, cx: &mut Context<Self>) {
        self.status = status;
        if status == FormStatus::Editing {
            self.attempted = false;
        }
        cx.notify();
    }

    fn on_input(&mut self, name: &SharedString, event: &InputEvent, cx: &mut Context<Self>) {
        match event {
            InputEvent::Blur => {
                self.touched.insert(name.clone());
                cx.notify();
            }
            InputEvent::Change => cx.notify(),
            InputEvent::PressEnter { .. } | InputEvent::Focus => {}
        }
    }

    fn value(&self, one: &FormField, cx: &Context<Self>) -> FormValue {
        match self.controls.get(&one.name) {
            Some(Control::Line(state)) => FormValue::Text(state.read(cx).value()),
            Some(Control::Note(state)) => FormValue::Text(state.read(cx).value()),
            Some(Control::Menu(state)) => {
                FormValue::Text(state.read(cx).selected_value().cloned().unwrap_or_default())
            }
            Some(Control::Choice(picked)) => {
                let text = match (&one.kind, picked) {
                    (FieldKind::Choice(options), Some(ix)) => {
                        options.get(*ix).cloned().unwrap_or_default()
                    }
                    _ => SharedString::default(),
                };
                FormValue::Text(text)
            }
            Some(Control::Flag(on)) => FormValue::Flag(*on),
            None => FormValue::Text(SharedString::default()),
        }
    }

    /// Every answer by field name, in field order.
    pub fn values(&self, cx: &Context<Self>) -> Vec<(SharedString, FormValue)> {
        self.fields
            .iter()
            .map(|one| (one.name.clone(), self.value(one, cx)))
            .collect()
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        self.attempted = true;
        let wrong = self
            .fields
            .iter()
            .filter(|one| problem(one, &self.value(one, cx)).is_some())
            .count();
        if wrong == 0 {
            self.status = FormStatus::Submitting;
            cx.emit(FormCardEvent::Submit(self.values(cx)));
        }
        cx.notify();
    }
}

impl Render for FormCard {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let danger = theme.danger;
        let submitted = self.status == FormStatus::Submitted;
        let busy = self.status == FormStatus::Submitting;

        let body = if submitted {
            let rows = self.fields.iter().map(|one| {
                h_flex()
                    .items_start()
                    .gap_4()
                    .child(
                        div()
                            .w(px(140.))
                            .flex_none()
                            .text_color(quiet)
                            .child(one.label.clone()),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .truncate()
                            .child(shown(&self.value(one, cx))),
                    )
            });
            div()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(10.))
                        .p_4()
                        .text_body()
                        .children(rows),
                )
                .child(
                    h_flex().justify_end().px_2().pb_2().child(
                        Button::new("edit-answers")
                            .ghost()
                            .small()
                            .icon(Icon::new(CatalogIcon::Edit))
                            .label("Edit answers")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_status(FormStatus::Editing, cx)
                            })),
                    ),
                )
                .into_any_element()
        } else {
            let mut wrong = 0;
            let mut form = v_form();
            for one in &self.fields {
                let trouble = problem(one, &self.value(one, cx));
                if trouble.is_some() {
                    wrong += 1;
                }
                let trouble =
                    trouble.filter(|_| self.attempted || self.touched.contains(&one.name));
                let name = one.name.clone();
                let control = match (self.controls.get(&one.name), &one.kind) {
                    (Some(Control::Line(state)), _) => {
                        Input::new(state).disabled(busy).into_any_element()
                    }
                    (Some(Control::Note(state)), _) => {
                        Textarea::new(state).disabled(busy).into_any_element()
                    }
                    (Some(Control::Menu(state)), _) => Select::new(state)
                        .placeholder(one.placeholder.clone().unwrap_or_else(|| "Choose…".into()))
                        .disabled(busy)
                        .into_any_element(),
                    (Some(Control::Choice(picked)), FieldKind::Choice(options)) => {
                        RadioGroup::vertical(SharedString::from(format!("choice-{name}")))
                            .children(options.clone())
                            .selected_index(*picked)
                            .disabled(busy)
                            .on_click(cx.listener(move |this, ix: &usize, _, cx| {
                                this.controls
                                    .insert(name.clone(), Control::Choice(Some(*ix)));
                                this.touched.insert(name.clone());
                                cx.notify();
                            }))
                            .into_any_element()
                    }
                    (Some(Control::Flag(on)), FieldKind::Switch) => {
                        Switch::new(SharedString::from(format!("switch-{name}")))
                            .checked(*on)
                            .disabled(busy)
                            .on_click(cx.listener(move |this, on: &bool, _, cx| {
                                this.controls.insert(name.clone(), Control::Flag(*on));
                                this.touched.insert(name.clone());
                                cx.notify();
                            }))
                            .into_any_element()
                    }
                    (Some(Control::Flag(on)), _) => {
                        Checkbox::new(SharedString::from(format!("check-{name}")))
                            .checked(*on)
                            .disabled(busy)
                            .on_click(cx.listener(move |this, on: &bool, _, cx| {
                                this.controls.insert(name.clone(), Control::Flag(*on));
                                this.touched.insert(name.clone());
                                cx.notify();
                            }))
                            .into_any_element()
                    }
                    _ => div().into_any_element(),
                };
                let help = one.description.clone();
                form = form.child(
                    field()
                        .label(one.label.clone())
                        .required(one.required)
                        .when(trouble.is_some() || help.is_some(), |this| {
                            this.description_fn(move |_, _| match &trouble {
                                Some(trouble) => div()
                                    .text_color(danger)
                                    .child(trouble.clone())
                                    .into_any_element(),
                                None => div()
                                    .child(help.clone().unwrap_or_default())
                                    .into_any_element(),
                            })
                        })
                        .child(control),
                );
            }
            let note: SharedString = if self.attempted && wrong > 0 {
                attention(wrong).into()
            } else {
                "* Required".into()
            };
            div()
                .flex()
                .flex_col()
                .gap_4()
                .p_4()
                .child(form)
                .child(
                    h_flex()
                        .justify_between()
                        .gap_3()
                        .pt_1()
                        .child(
                            div()
                                .text_caption()
                                .text_color(if self.attempted && wrong > 0 {
                                    danger
                                } else {
                                    quiet
                                })
                                .child(note),
                        )
                        .child(
                            Button::new("submit")
                                .primary()
                                .loading(busy)
                                .label(if busy {
                                    SharedString::from("Sending…")
                                } else {
                                    self.submit_label.clone()
                                })
                                .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
                        ),
                )
                .into_any_element()
        };

        shell(cx)
            .id("form-card")
            .max_w(px(448.))
            .role(Role::Form)
            .aria_label(if submitted {
                format!("{}, submitted", self.title)
            } else {
                self.title.to_string()
            })
            .child(shell_header(
                Some(Icon::new(CatalogIcon::Task)),
                self.title.clone(),
                self.description
                    .clone()
                    .filter(|_| !submitted)
                    .map(|line| line.into_any_element()),
                submitted.then(|| {
                    Tag::success()
                        .rounded_full()
                        .gap(px(6.))
                        .child(Icon::new(CatalogIcon::CheckmarkFilled).size(px(14.)))
                        .child("Submitted")
                        .into_any_element()
                }),
                cx,
            ))
            .child(panel(cx).child(body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(value: &str) -> FormValue {
        FormValue::Text(SharedString::from(value.to_string()))
    }

    #[test]
    fn a_required_field_must_be_answered() {
        let name = FormField::new("name", "Name", FieldKind::Text).required();
        assert_eq!(problem(&name, &text("  ")), Some("Name is required".into()));
        assert_eq!(problem(&name, &text("Ada")), None);
        let terms = FormField::new("terms", "Terms", FieldKind::Checkbox).required();
        assert!(problem(&terms, &FormValue::Flag(false)).is_some());
        assert!(problem(&terms, &FormValue::Flag(true)).is_none());
        let optional = FormField::new("note", "Note", FieldKind::Note);
        assert_eq!(problem(&optional, &text("")), None);
    }

    #[test]
    fn emails_and_numbers_are_checked() {
        let email = FormField::new("email", "Email", FieldKind::Email);
        assert!(problem(&email, &text("ada@example.org")).is_none());
        for wrong in [
            "ada",
            "ada@",
            "ada@example",
            "a da@example.org",
            "@example.org",
            "a@b@c.d",
        ] {
            assert!(problem(&email, &text(wrong)).is_some(), "{wrong}");
        }
        let seats = FormField::new(
            "seats",
            "Seats",
            FieldKind::Number {
                min: Some(1.),
                max: Some(8.),
            },
        );
        assert_eq!(
            problem(&seats, &text("four")),
            Some("Enter a number".into())
        );
        assert_eq!(
            problem(&seats, &text("0")),
            Some("Must be at least 1".into())
        );
        assert_eq!(
            problem(&seats, &text("9")),
            Some("Must be at most 8".into())
        );
        assert_eq!(problem(&seats, &text(" 4 ")), None);
    }

    #[test]
    fn the_summary_and_the_count_read_plainly() {
        assert_eq!(shown(&FormValue::Flag(true)), "Yes");
        assert_eq!(shown(&FormValue::Flag(false)), "No");
        assert_eq!(shown(&text("")), "—");
        assert_eq!(shown(&text("Window")), "Window");
        assert_eq!(attention(1), "1 field needs attention");
        assert_eq!(attention(3), "3 fields need attention");
    }
}
