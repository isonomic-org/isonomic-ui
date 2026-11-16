//! Step form wizard: a long form split into short steps, ending in a review.
//!
//! A row of thin bars on top shows where people are: reached steps fill in,
//! finished ones get a check and can be clicked to go back. Each step holds
//! a few fields: text fields (with an optional fixed prefix), chip fields
//! that collect several entries, and choice cards. Continue checks the step
//! and shows what is missing under each field. The last step is a review
//! built from everything entered, with Edit links, and finishing shows a
//! success panel.

use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

type Check = Rc<dyn Fn(&str) -> Option<String>>;

/// One card in a choice field.
#[derive(Clone)]
pub struct WizardChoice {
    value: SharedString,
    title: SharedString,
    aside: Option<SharedString>,
    description: Option<SharedString>,
    badge: Option<SharedString>,
}

impl WizardChoice {
    /// Create a choice.
    pub fn new(value: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            title: title.into(),
            aside: None,
            description: None,
            badge: None,
        }
    }

    /// Show a short figure under the title, such as a price.
    pub fn aside(mut self, aside: impl Into<SharedString>) -> Self {
        self.aside = Some(aside.into());
        self
    }

    /// Describe the choice in a line.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Mark it with a small badge.
    pub fn badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.badge = Some(badge.into());
        self
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Text,
    Tags,
    Choice,
}

/// A field in a step.
#[derive(Clone)]
pub struct WizardField {
    kind: Kind,
    name: SharedString,
    label: SharedString,
    hint: Option<SharedString>,
    placeholder: Option<SharedString>,
    prefix: Option<SharedString>,
    default: Option<SharedString>,
    optional: bool,
    check: Option<Check>,
    slug_of: Option<SharedString>,
    max: Option<usize>,
    review_empty: Option<SharedString>,
    choices: Vec<WizardChoice>,
}

impl WizardField {
    fn new(kind: Kind, name: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            kind,
            name: name.into(),
            label: label.into(),
            hint: None,
            placeholder: None,
            prefix: None,
            default: None,
            optional: false,
            check: None,
            slug_of: None,
            max: None,
            review_empty: None,
            choices: Vec::new(),
        }
    }

    /// A one-line text field.
    pub fn text(name: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self::new(Kind::Text, name, label)
    }

    /// A field that turns entries into chips, checked as email addresses
    /// unless [`WizardField::check`] says otherwise.
    pub fn tags(name: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self::new(Kind::Tags, name, label).check(|entry| {
            (!looks_like_email(entry)).then(|| format!("{entry} isn't an email address"))
        })
    }

    /// A set of cards to pick one from.
    pub fn choice(name: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self::new(Kind::Choice, name, label)
    }

    /// Show help under the field.
    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Show a placeholder in an empty text or chip field.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Show fixed text before a text field's value, such as an address.
    pub fn prefix(mut self, prefix: impl Into<SharedString>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// Start with a value: text for text fields, the value of a choice.
    pub fn default(mut self, value: impl Into<SharedString>) -> Self {
        self.default = Some(value.into());
        self
    }

    /// Let the field stay empty.
    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    /// Check a text value, or each chip entry, returning a message when wrong.
    pub fn check(mut self, check: impl Fn(&str) -> Option<String> + 'static) -> Self {
        self.check = Some(Rc::new(check));
        self
    }

    /// Fill this text field with a web-safe form of another field's text,
    /// until someone edits it by hand.
    pub fn slug_of(mut self, other: impl Into<SharedString>) -> Self {
        self.slug_of = Some(other.into());
        self
    }

    /// Cap how many chips a chip field takes.
    pub fn max(mut self, max: usize) -> Self {
        self.max = Some(max);
        self
    }

    /// What the review shows when the field was left empty.
    pub fn review_empty(mut self, text: impl Into<SharedString>) -> Self {
        self.review_empty = Some(text.into());
        self
    }

    /// Add a card to a choice field.
    pub fn option(mut self, choice: WizardChoice) -> Self {
        self.choices.push(choice);
        self
    }
}

/// One step of the wizard.
#[derive(Clone)]
pub struct WizardStep {
    title: SharedString,
    heading: Option<SharedString>,
    description: Option<SharedString>,
    fields: Vec<WizardField>,
}

impl WizardStep {
    /// Create a step. The title shows in the step bar and the review.
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            heading: None,
            description: None,
            fields: Vec::new(),
        }
    }

    /// Set the heading over the fields. The title is used otherwise.
    pub fn heading(mut self, heading: impl Into<SharedString>) -> Self {
        self.heading = Some(heading.into());
        self
    }

    /// Explain the step in a line.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add a field.
    pub fn field(mut self, field: WizardField) -> Self {
        self.fields.push(field);
        self
    }
}

/// What a field holds.
#[derive(Debug, Clone, PartialEq)]
pub enum WizardValue {
    /// A text field's value.
    Text(String),
    /// A chip field's entries.
    Tags(Vec<String>),
    /// The picked choice's value.
    Choice(Option<SharedString>),
}

/// What happened in the wizard.
#[derive(Debug, Clone)]
pub enum StepFormEvent {
    /// The step changed. The review is the step after the last one given.
    StepChanged(usize),
    /// Finish was pressed with every value. Call
    /// [`StepFormWizard::complete`] when the work is done.
    Submit(Vec<(SharedString, WizardValue)>),
    /// The wizard was reset from the success panel.
    Reset,
}

/// Turn text into lowercase letters, digits and single dashes.
fn slugify(text: &str) -> String {
    let mut out = String::new();
    for c in text.trim().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

/// A loose email check: something, an @, a domain with a dot.
fn looks_like_email(text: &str) -> bool {
    let Some((name, domain)) = text.split_once('@') else {
        return false;
    };
    !name.is_empty()
        && !text.contains(char::is_whitespace)
        && domain.split('.').count() >= 2
        && domain.split('.').all(|part| !part.is_empty())
}

/// Split typed or pasted text into chip entries.
fn entries(text: &str) -> Vec<String> {
    text.split([',', ';', '\n', ' '])
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string)
        .collect()
}

#[derive(Clone, PartialEq)]
enum Finish {
    Editing,
    Submitting,
    Failed(SharedString),
    Done,
}

/// A form split into steps, with checks, a review and a success panel.
pub struct StepFormWizard {
    steps: Vec<WizardStep>,
    review_heading: SharedString,
    review_description: SharedString,
    submit_label: SharedString,
    submitting_label: SharedString,
    success_heading: SharedString,
    success_line: SharedString,
    at: usize,
    reached: usize,
    finish: Finish,
    inputs: HashMap<SharedString, Entity<InputState>>,
    tags: HashMap<SharedString, Vec<String>>,
    picks: HashMap<SharedString, SharedString>,
    choice_focus: HashMap<SharedString, FocusHandle>,
    errors: HashMap<SharedString, String>,
    hand_edited: HashSet<SharedString>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<StepFormEvent> for StepFormWizard {}

impl Focusable for StepFormWizard {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl StepFormWizard {
    /// Create an empty wizard.
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            steps: Vec::new(),
            review_heading: "Review and finish".into(),
            review_description: "Check everything before it's created.".into(),
            submit_label: "Finish".into(),
            submitting_label: "Creating\u{2026}".into(),
            success_heading: "All done".into(),
            success_line: "Everything was created.".into(),
            at: 0,
            reached: 0,
            finish: Finish::Editing,
            inputs: HashMap::new(),
            tags: HashMap::new(),
            picks: HashMap::new(),
            choice_focus: HashMap::new(),
            errors: HashMap::new(),
            hand_edited: HashSet::new(),
            focus: cx.focus_handle(),
            _subscriptions: Vec::new(),
        }
    }

    /// Add a step. A review step follows the last one on its own.
    pub fn step(mut self, step: WizardStep) -> Self {
        for field in &step.fields {
            if field.kind == Kind::Choice
                && let Some(default) = &field.default
            {
                self.picks.insert(field.name.clone(), default.clone());
            }
        }
        self.steps.push(step);
        self
    }

    /// Set the review step's heading and line.
    pub fn review(
        mut self,
        heading: impl Into<SharedString>,
        description: impl Into<SharedString>,
    ) -> Self {
        self.review_heading = heading.into();
        self.review_description = description.into();
        self
    }

    /// Set the finish button's label, and the label while finishing.
    pub fn submit_label(
        mut self,
        label: impl Into<SharedString>,
        submitting: impl Into<SharedString>,
    ) -> Self {
        self.submit_label = label.into();
        self.submitting_label = submitting.into();
        self
    }

    /// Set the success panel's heading and line.
    pub fn success(
        mut self,
        heading: impl Into<SharedString>,
        line: impl Into<SharedString>,
    ) -> Self {
        self.success_heading = heading.into();
        self.success_line = line.into();
        self
    }

    /// Say how finishing went: the success panel shows, or the message shows
    /// next to the button.
    pub fn complete(&mut self, result: Result<(), SharedString>, cx: &mut Context<Self>) {
        self.finish = match result {
            Ok(()) => Finish::Done,
            Err(message) => Finish::Failed(message),
        };
        cx.notify();
    }

    /// Every field's value.
    pub fn values(&self, cx: &App) -> Vec<(SharedString, WizardValue)> {
        self.steps
            .iter()
            .flat_map(|step| &step.fields)
            .map(|field| (field.name.clone(), self.value(field, cx)))
            .collect()
    }

    fn value(&self, field: &WizardField, cx: &App) -> WizardValue {
        match field.kind {
            Kind::Text => WizardValue::Text(
                self.inputs
                    .get(&field.name)
                    .map(|input| input.read(cx).value().trim().to_string())
                    .unwrap_or_default(),
            ),
            Kind::Tags => {
                WizardValue::Tags(self.tags.get(&field.name).cloned().unwrap_or_default())
            }
            Kind::Choice => WizardValue::Choice(self.picks.get(&field.name).cloned()),
        }
    }

    fn review_at(&self) -> usize {
        self.steps.len()
    }

    /// Make the text fields once, with their events.
    fn make_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let fields: Vec<WizardField> = self
            .steps
            .iter()
            .flat_map(|step| step.fields.clone())
            .filter(|field| field.kind != Kind::Choice)
            .collect();
        let mut made = false;
        for field in fields {
            if self.inputs.contains_key(&field.name) {
                continue;
            }
            made = true;
            let input = cx.new(|cx| {
                let mut input = InputState::new(window, cx)
                    .placeholder(field.placeholder.clone().unwrap_or_default());
                if field.kind == Kind::Text
                    && let Some(start) = &field.default
                {
                    input.set_value(start.clone(), window, cx);
                }
                input
            });
            let name = field.name.clone();
            let kind = field.kind;
            let events =
                cx.subscribe_in(
                    &input,
                    window,
                    move |this, input, event, window, cx| match event {
                        InputEvent::Change => {
                            this.errors.remove(&name);
                            if kind == Kind::Tags {
                                let typed = input.read(cx).value().to_string();
                                if typed.contains([',', ';', '\n']) {
                                    this.add_tags(&name, &typed, window, cx);
                                }
                            } else {
                                if input.read(cx).focus_handle(cx).is_focused(window) {
                                    this.hand_edited.insert(name.clone());
                                }
                                this.follow_slugs(&name, window, cx);
                            }
                            cx.notify();
                        }
                        InputEvent::PressEnter { .. } => {
                            let typed = input.read(cx).value().to_string();
                            if kind == Kind::Tags && !typed.trim().is_empty() {
                                this.add_tags(&name, &typed, window, cx);
                            } else {
                                this.next(window, cx);
                            }
                        }
                        _ => {}
                    },
                );
            self.inputs.insert(field.name.clone(), input);
            self._subscriptions.push(events);
        }
        if made {
            let sources: Vec<SharedString> = self
                .steps
                .iter()
                .flat_map(|step| &step.fields)
                .filter_map(|field| field.slug_of.clone())
                .collect();
            for source in sources {
                self.follow_slugs(&source, window, cx);
            }
        }
        for field in self.steps.iter().flat_map(|step| &step.fields) {
            if field.kind == Kind::Choice && !self.choice_focus.contains_key(&field.name) {
                self.choice_focus
                    .insert(field.name.clone(), cx.focus_handle());
            }
        }
    }

    /// Fill fields that follow `source`, unless they were edited by hand.
    fn follow_slugs(&mut self, source: &SharedString, window: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = self
            .inputs
            .get(source)
            .map(|input| input.read(cx).value().to_string())
        else {
            return;
        };
        let followers: Vec<SharedString> = self
            .steps
            .iter()
            .flat_map(|step| &step.fields)
            .filter(|field| field.slug_of.as_ref() == Some(source))
            .map(|field| field.name.clone())
            .filter(|name| !self.hand_edited.contains(name))
            .collect();
        for name in followers {
            if let Some(input) = self.inputs.get(&name) {
                let slug = slugify(&text);
                input.update(cx, |input, cx| input.set_value(slug, window, cx));
                self.errors.remove(&name);
            }
        }
    }

    fn field(&self, name: &str) -> Option<&WizardField> {
        self.steps
            .iter()
            .flat_map(|step| &step.fields)
            .find(|field| field.name == name)
    }

    fn add_tags(
        &mut self,
        name: &SharedString,
        typed: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(field) = self.field(name).cloned() else {
            return;
        };
        let list = self.tags.entry(name.clone()).or_default();
        let mut rejected = Vec::new();
        for entry in entries(typed) {
            if field.max.is_some_and(|max| list.len() >= max) {
                self.errors.insert(
                    name.clone(),
                    format!("Up to {} entries", field.max.unwrap_or(0)),
                );
                break;
            }
            if let Some(message) = field.check.as_ref().and_then(|check| check(&entry)) {
                self.errors.insert(name.clone(), message);
                rejected.push(entry);
            } else if !list.contains(&entry) {
                list.push(entry);
            }
        }
        // Wrong entries stay in the field so they can be fixed.
        let left = rejected.join(", ");
        if let Some(input) = self.inputs.get(name) {
            input.update(cx, |input, cx| input.set_value(left, window, cx));
        }
        cx.notify();
    }

    fn remove_tag(&mut self, name: &SharedString, ix: usize, cx: &mut Context<Self>) {
        if let Some(list) = self.tags.get_mut(name)
            && ix < list.len()
        {
            list.remove(ix);
            cx.notify();
        }
    }

    /// The step's problems, by field.
    fn problems(&self, step: usize, cx: &App) -> Vec<(SharedString, String)> {
        let Some(step) = self.steps.get(step) else {
            return Vec::new();
        };
        step.fields
            .iter()
            .filter_map(|field| {
                let message = match (field.kind, self.value(field, cx)) {
                    (Kind::Text, WizardValue::Text(text)) if text.is_empty() => {
                        (!field.optional).then(|| format!("{} is needed", field.label))
                    }
                    (Kind::Text, WizardValue::Text(text)) => {
                        field.check.as_ref().and_then(|check| check(&text))
                    }
                    (Kind::Tags, WizardValue::Tags(list)) if list.is_empty() => {
                        (!field.optional).then(|| "Add at least one".to_string())
                    }
                    (Kind::Choice, WizardValue::Choice(None)) => {
                        (!field.optional).then(|| "Pick one".to_string())
                    }
                    _ => None,
                };
                message
                    .or_else(|| self.errors.get(&field.name).cloned())
                    .map(|message| (field.name.clone(), message))
            })
            .collect()
    }

    fn go(&mut self, to: usize, window: &mut Window, cx: &mut Context<Self>) {
        if to > self.reached || to > self.review_at() || to == self.at {
            return;
        }
        self.at = to;
        if self.finish != Finish::Submitting {
            self.finish = Finish::Editing;
        }
        cx.emit(StepFormEvent::StepChanged(to));
        self.focus_first(window, cx);
        cx.notify();
    }

    fn focus_first(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(step) = self.steps.get(self.at) else {
            window.focus(&self.focus, cx);
            return;
        };
        let first = step.fields.first().map(|field| field.name.clone());
        self.focus_field(first.as_ref(), window, cx);
    }

    fn focus_field(
        &mut self,
        name: Option<&SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(name) = name else {
            return;
        };
        if let Some(input) = self.inputs.get(name) {
            input.update(cx, |input, cx| input.focus(window, cx));
        } else if let Some(focus) = self.choice_focus.get(name) {
            window.focus(focus, cx);
        }
    }

    fn next(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.at == self.review_at() {
            self.submit(cx);
            return;
        }
        // Text left in a chip field is added before checking.
        let pending: Vec<(SharedString, String)> = self.steps[self.at]
            .fields
            .iter()
            .filter(|field| field.kind == Kind::Tags)
            .filter_map(|field| {
                let typed = self.inputs.get(&field.name)?.read(cx).value().to_string();
                (!typed.trim().is_empty()).then(|| (field.name.clone(), typed))
            })
            .collect();
        for (name, typed) in pending {
            self.add_tags(&name, &typed, window, cx);
        }
        let problems = self.problems(self.at, cx);
        if let Some((first, _)) = problems.first().cloned() {
            for (name, message) in problems {
                self.errors.insert(name, message);
            }
            self.focus_field(Some(&first), window, cx);
            cx.notify();
            return;
        }
        self.reached = self.reached.max(self.at + 1);
        self.go(self.at + 1, window, cx);
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if self.finish == Finish::Submitting {
            return;
        }
        self.finish = Finish::Submitting;
        cx.emit(StepFormEvent::Submit(self.values(cx)));
        cx.notify();
    }

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.at = 0;
        self.reached = 0;
        self.finish = Finish::Editing;
        self.errors.clear();
        self.hand_edited.clear();
        self.tags.clear();
        self.picks.clear();
        for field in self.steps.iter().flat_map(|step| &step.fields) {
            match field.kind {
                Kind::Choice => {
                    if let Some(default) = &field.default {
                        self.picks.insert(field.name.clone(), default.clone());
                    }
                }
                _ => {
                    let start = match field.kind {
                        Kind::Text => field.default.clone().unwrap_or_default(),
                        _ => SharedString::default(),
                    };
                    if let Some(input) = self.inputs.get(&field.name) {
                        input.update(cx, |input, cx| input.set_value(start, window, cx));
                    }
                }
            }
        }
        let sources: Vec<SharedString> = self
            .steps
            .iter()
            .flat_map(|step| &step.fields)
            .filter_map(|field| field.slug_of.clone())
            .collect();
        for source in sources {
            self.follow_slugs(&source, window, cx);
        }
        cx.emit(StepFormEvent::Reset);
        self.focus_first(window, cx);
        cx.notify();
    }

    fn pick(&mut self, name: &SharedString, value: SharedString, cx: &mut Context<Self>) {
        self.picks.insert(name.clone(), value);
        self.errors.remove(name);
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        // Backspace in an empty chip field takes the last chip off.
        if key == "backspace" {
            let empty_tags = self
                .steps
                .get(self.at)
                .into_iter()
                .flat_map(|step| &step.fields)
                .filter(|field| field.kind == Kind::Tags)
                .find(|field| {
                    self.inputs.get(&field.name).is_some_and(|input| {
                        let input = input.read(cx);
                        input.focus_handle(cx).is_focused(window) && input.value().is_empty()
                    })
                })
                .map(|field| field.name.clone());
            if let Some(name) = empty_tags {
                let last = self.tags.get(&name).map_or(0, Vec::len);
                if last > 0 {
                    self.remove_tag(&name, last - 1, cx);
                    cx.stop_propagation();
                }
            }
            return;
        }
        // Arrows move through choice cards; Enter continues from them.
        let focused_choice = self
            .steps
            .get(self.at)
            .into_iter()
            .flat_map(|step| &step.fields)
            .filter(|field| field.kind == Kind::Choice)
            .find(|field| {
                self.choice_focus
                    .get(&field.name)
                    .is_some_and(|focus| focus.is_focused(window))
            })
            .cloned();
        if let Some(field) = focused_choice {
            let at = self
                .picks
                .get(&field.name)
                .and_then(|picked| field.choices.iter().position(|c| &c.value == picked));
            let last = field.choices.len().saturating_sub(1);
            let to = match (key, at) {
                ("left" | "up", Some(at)) => Some(at.saturating_sub(1)),
                ("right" | "down", Some(at)) => Some((at + 1).min(last)),
                ("left" | "up" | "right" | "down", None) => Some(0),
                ("enter", _) => {
                    self.next(window, cx);
                    cx.stop_propagation();
                    return;
                }
                _ => None,
            };
            if let Some(to) = to
                && let Some(choice) = field.choices.get(to)
            {
                self.pick(&field.name, choice.value.clone(), cx);
                cx.stop_propagation();
            }
            return;
        }
        if key == "enter" && self.focus.is_focused(window) && self.finish != Finish::Done {
            self.next(window, cx);
            cx.stop_propagation();
        }
    }

    fn review_text(&self, field: &WizardField, cx: &App) -> Option<String> {
        let text = match self.value(field, cx) {
            WizardValue::Text(text) if text.is_empty() => None,
            WizardValue::Text(text) => Some(format!(
                "{}{}",
                field.prefix.clone().unwrap_or_default(),
                text
            )),
            WizardValue::Tags(list) if list.is_empty() => None,
            WizardValue::Tags(list) => Some(list.join(", ")),
            WizardValue::Choice(picked) => picked.and_then(|picked| {
                field
                    .choices
                    .iter()
                    .find(|choice| choice.value == picked)
                    .map(|choice| match &choice.aside {
                        Some(aside) => format!("{} \u{00b7} {}", choice.title, aside),
                        None => choice.title.to_string(),
                    })
            }),
        };
        text.or_else(|| field.review_empty.as_ref().map(ToString::to_string))
    }
}

impl Render for StepFormWizard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.make_inputs(window, cx);

        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.blue;
        let danger = theme.danger;
        let success = theme.success;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;

        let card = v_flex()
            .track_focus(&self.focus)
            .capture_key_down(cx.listener(Self::key_down))
            .w_full()
            .max_w(px(520.))
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .line_height(relative(1.45))
            .shadow(resting_shadow(cx));

        if self.finish == Finish::Done {
            return card
                .items_center()
                .gap_2()
                .px_6()
                .py_10()
                .child(
                    div()
                        .size(px(44.))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(success.opacity(0.14))
                        .child(Icon::new(IconName::Check).text_color(success)),
                )
                .child(
                    div()
                        .pt_1()
                        .text_base()
                        .font_semibold()
                        .child(self.success_heading.clone()),
                )
                .child(div().text_color(quiet).child(self.success_line.clone()))
                .child(
                    div().pt_2().child(
                        Button::new("wizard-again")
                            .outline()
                            .small()
                            .label("Start over")
                            .on_click(cx.listener(|this, _, window, cx| this.reset(window, cx))),
                    ),
                );
        }

        let total = self.steps.len() + 1;
        let titles: Vec<SharedString> = self
            .steps
            .iter()
            .map(|step| step.title.clone())
            .chain([SharedString::from("Review")])
            .collect();
        let bar =
            h_flex()
                .gap_1p5()
                .px_5()
                .pt_4()
                .children((0..total).map(|ix| {
                    let done = ix < self.at || (ix < self.reached && ix != self.at);
                    let current = ix == self.at;
                    let reached = ix <= self.reached;
                    v_flex()
                        .id(("wizard-step", ix))
                        .flex_1()
                        .min_w_0()
                        .gap_1p5()
                        .when(reached && !current, |this| {
                            this.cursor_pointer().on_click(
                                cx.listener(move |this, _, window, cx| this.go(ix, window, cx)),
                            )
                        })
                        .child(div().h(px(2.)).rounded_full().bg(
                            if ix <= self.at.max(self.reached) {
                                accent
                            } else {
                                line
                            },
                        ))
                        .child(
                            h_flex()
                                .gap_1p5()
                                .text_xs()
                                .font_medium()
                                .text_color(if current { ink } else { quiet })
                                .child(if done {
                                    Icon::new(IconName::Check)
                                        .xsmall()
                                        .text_color(accent)
                                        .into_any_element()
                                } else {
                                    div()
                                        .text_color(if current { accent } else { quiet })
                                        .child((ix + 1).to_string())
                                        .into_any_element()
                                })
                                .child(div().truncate().child(titles[ix].to_uppercase())),
                        )
                }));

        let label_row = |field: &WizardField| {
            h_flex()
                .justify_between()
                .child(div().font_medium().child(field.label.clone()))
                .when(field.optional, |this| {
                    this.child(div().text_xs().text_color(quiet).child("Optional"))
                })
        };
        let under = |field: &WizardField, error: Option<&String>| match (error, &field.hint) {
            (Some(error), _) => Some(
                h_flex()
                    .gap_1()
                    .text_xs()
                    .text_color(danger)
                    .child(Icon::new(IconName::CircleAlert).xsmall())
                    .child(error.clone()),
            ),
            (None, Some(hint)) => Some(h_flex().text_xs().text_color(quiet).child(hint.clone())),
            _ => None,
        };

        let body = if self.at == self.review_at() {
            let rows =
                self.steps.iter().enumerate().map(|(ix, step)| {
                    let texts: Vec<String> = step
                        .fields
                        .iter()
                        .filter_map(|field| self.review_text(field, cx))
                        .collect();
                    h_flex()
                        .items_start()
                        .gap_3()
                        .px_3()
                        .py_2p5()
                        .when(ix > 0, |this| this.border_t_1().border_color(line))
                        .child(
                            div()
                                .w(px(96.))
                                .flex_none()
                                .text_xs()
                                .text_color(quiet)
                                .pt_0p5()
                                .child(step.title.clone()),
                        )
                        .child(v_flex().flex_1().min_w_0().children(
                            texts.into_iter().enumerate().map(|(line_ix, text)| {
                                div()
                                    .when(line_ix == 0, |this| this.font_medium())
                                    .when(line_ix > 0, |this| this.text_xs().text_color(quiet))
                                    .child(text)
                            }),
                        ))
                        .child(
                            Button::new(("wizard-edit", ix))
                                .ghost()
                                .xsmall()
                                .label("Edit")
                                .on_click(
                                    cx.listener(move |this, _, window, cx| this.go(ix, window, cx)),
                                ),
                        )
                });
            v_flex()
                .gap_3()
                .child(
                    v_flex()
                        .child(
                            div()
                                .text_base()
                                .font_semibold()
                                .child(self.review_heading.clone()),
                        )
                        .child(
                            div()
                                .text_color(quiet)
                                .child(self.review_description.clone()),
                        ),
                )
                .child(
                    v_flex()
                        .rounded(radius)
                        .border_1()
                        .border_color(line)
                        .children(rows),
                )
                .into_any_element()
        } else {
            let step = self.steps[self.at].clone();
            let fields = step.fields.iter().map(|field| {
                let error = self.errors.get(&field.name);
                let wrong = error.is_some();
                let control = match field.kind {
                    Kind::Text => {
                        self.inputs.get(&field.name).map(|input| {
                            let prefixed = field.prefix.is_some();
                            // The row draws the field's edge, so it shows the focus too.
                            let focused = input.read(cx).focus_handle(cx).is_focused(window);
                            h_flex()
                                .h(px(34.))
                                .px_3()
                                .rounded(radius)
                                .border_1()
                                .border_color(if wrong { danger } else { line })
                                .when(focused, |this| this.focus_ring_style(window, cx))
                                .children(field.prefix.clone().map(|prefix| {
                                    div().flex_none().text_color(quiet).child(prefix)
                                }))
                                .child(
                                    div().flex_1().min_w_0().child(
                                        // The row carries the padding, so the text
                                        // lines up with the prefix and other fields.
                                        Input::new(input)
                                            .appearance(false)
                                            .px_0()
                                            .when(prefixed, |this| this.pl_px()),
                                    ),
                                )
                                .into_any_element()
                        })
                    }
                    Kind::Tags => {
                        self.inputs.get(&field.name).map(|input| {
                            let name = field.name.clone();
                            let list = self.tags.get(&field.name).cloned().unwrap_or_default();
                            let focused = input.read(cx).focus_handle(cx).is_focused(window);
                            h_flex()
                                .flex_wrap()
                                .gap_1()
                                .min_h(px(34.))
                                .px_1p5()
                                .py_1()
                                .rounded(radius)
                                .border_1()
                                .border_color(if wrong { danger } else { line })
                                .when(focused, |this| this.focus_ring_style(window, cx))
                                .children(list.into_iter().enumerate().map(|(ix, entry)| {
                                    let name = name.clone();
                                    h_flex()
                                        .gap_0p5()
                                        .pl_2()
                                        .pr_0p5()
                                        .rounded(px(4.))
                                        .bg(chip)
                                        .text_xs()
                                        .child(entry.clone())
                                        .child(
                                            Button::new(SharedString::from(format!(
                                                "wizard-tag-{name}-{ix}"
                                            )))
                                            .ghost()
                                            .xsmall()
                                            .icon(IconName::Close)
                                            .accessibility_label(format!("Remove {entry}"))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.remove_tag(&name, ix, cx)
                                            })),
                                        )
                                }))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(140.))
                                        .child(Input::new(input).appearance(false).small()),
                                )
                                .into_any_element()
                        })
                    }
                    Kind::Choice => self.choice_focus.get(&field.name).map(|focus| {
                        let picked = self.picks.get(&field.name).cloned();
                        h_flex()
                            .track_focus(focus)
                            .items_stretch()
                            .gap_2()
                            .children(field.choices.iter().enumerate().map(|(ix, choice)| {
                                let on = picked.as_ref() == Some(&choice.value);
                                let name = field.name.clone();
                                let value = choice.value.clone();
                                let group = focus.clone();
                                v_flex()
                                    .id(SharedString::from(format!(
                                        "wizard-choice-{}-{ix}",
                                        field.name
                                    )))
                                    .flex_1()
                                    .min_w_0()
                                    .gap_0p5()
                                    .p_2p5()
                                    .rounded(radius)
                                    .border_1()
                                    .border_color(if on {
                                        accent.opacity(0.6)
                                    } else if wrong {
                                        danger
                                    } else {
                                        line
                                    })
                                    .when(on, |this| this.bg(accent.opacity(0.06)))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        window.focus(&group, cx);
                                        this.pick(&name, value.clone(), cx)
                                    }))
                                    .child(
                                        h_flex()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .font_semibold()
                                                    .child(choice.title.clone()),
                                            )
                                            .child(
                                                div()
                                                    .size(px(14.))
                                                    .rounded_full()
                                                    .border_1()
                                                    .border_color(if on { accent } else { line })
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .when(on, |this| {
                                                        this.child(
                                                            div()
                                                                .size(px(6.))
                                                                .rounded_full()
                                                                .bg(accent),
                                                        )
                                                    }),
                                            ),
                                    )
                                    .children(choice.badge.clone().map(|badge| {
                                        h_flex().child(
                                            div()
                                                .px_1p5()
                                                .rounded(px(4.))
                                                .bg(accent.opacity(0.12))
                                                .text_color(accent)
                                                .text_xs()
                                                .child(badge),
                                        )
                                    }))
                                    .children(choice.aside.clone().map(|aside| div().child(aside)))
                                    .children(choice.description.clone().map(|description| {
                                        div().text_xs().text_color(quiet).child(description)
                                    }))
                            }))
                            .into_any_element()
                    }),
                };
                v_flex()
                    .gap_1()
                    .child(label_row(field))
                    .children(control)
                    .children(under(field, error))
            });
            v_flex()
                .gap_3()
                .child(
                    v_flex()
                        .child(
                            div()
                                .text_base()
                                .font_semibold()
                                .child(step.heading.clone().unwrap_or(step.title.clone())),
                        )
                        .children(
                            step.description
                                .clone()
                                .map(|description| div().text_color(quiet).child(description)),
                        ),
                )
                .children(fields)
                .into_any_element()
        };

        let on_review = self.at == self.review_at();
        let blocked = !on_review && !self.problems(self.at, cx).is_empty();
        let submitting = self.finish == Finish::Submitting;
        let failed = match &self.finish {
            Finish::Failed(message) => Some(message.clone()),
            _ => None,
        };
        let footer = h_flex()
            .gap_2()
            .px_5()
            .py_3()
            .border_t_1()
            .border_color(line)
            .when(self.at > 0, |this| {
                this.child(
                    Button::new("wizard-back")
                        .ghost()
                        .small()
                        .icon(IconName::ChevronLeft)
                        .label("Back")
                        .on_click(cx.listener(|this, _, window, cx| {
                            let back = this.at.saturating_sub(1);
                            this.go(back, window, cx)
                        })),
                )
            })
            .child(div().flex_1())
            .children(failed.map(|message| {
                h_flex()
                    .gap_1()
                    .text_xs()
                    .text_color(danger)
                    .child(Icon::new(IconName::CircleAlert).xsmall())
                    .child(message)
            }))
            .child(
                Button::new("wizard-next")
                    .primary()
                    .small()
                    .loading(submitting)
                    .label(if on_review {
                        if submitting {
                            self.submitting_label.clone()
                        } else {
                            self.submit_label.clone()
                        }
                    } else {
                        "Continue".into()
                    })
                    // Still answers when dimmed, so it can say what is missing.
                    .when(blocked, |this| this.opacity(0.55))
                    .on_click(cx.listener(|this, _, window, cx| this.next(window, cx))),
            );

        card.child(bar)
            .child(v_flex().px_5().py_4().child(body))
            .child(footer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_are_web_safe() {
        assert_eq!(slugify("  Harbour & Pine Books "), "harbour-pine-books");
        assert_eq!(slugify("Q3--Close!"), "q3-close");
        assert_eq!(slugify("***"), "");
    }

    #[test]
    fn emails_are_checked_loosely() {
        assert!(looks_like_email("mara@ledger.example"));
        assert!(!looks_like_email("mara@ledger"));
        assert!(!looks_like_email("@ledger.example"));
        assert!(!looks_like_email("mara ledger@x.io"));
        assert!(!looks_like_email("mara@.example"));
    }

    #[test]
    fn pasted_lists_split_into_entries() {
        assert_eq!(
            entries("a@x.io, b@y.io;c@z.io\n  "),
            vec!["a@x.io", "b@y.io", "c@z.io"]
        );
        assert!(entries(" , ").is_empty());
    }
}
