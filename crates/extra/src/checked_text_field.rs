// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Checked text field: a single-line field that checks what's typed against simple rules and says what's wrong, without nagging before the person has finished.
//!
//! Rules cover required, shortest and longest length, and the shape of an
//! email address, web address or digits. Checking starts when the person
//! first leaves the field; after that it follows each change, so a fixed
//! mistake clears at once. A good value shows a check; a bad one shows the
//! first rule it breaks, in red, under the field.

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement as _,
    Render, SharedString, Styled as _, Subscription, Window, div, prelude::FluentBuilder as _,
};

/// The shape a value must have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueShape {
    /// Anything.
    Any,
    /// An email address.
    Email,
    /// A web address starting with http:// or https://.
    WebAddress,
    /// Digits only.
    Digits,
}

/// The rules a value must follow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldRules {
    /// Must not be empty.
    pub required: bool,
    /// Fewest characters, when not empty.
    pub min_len: Option<usize>,
    /// Most characters.
    pub max_len: Option<usize>,
    /// The shape the value must have.
    pub shape: ValueShape,
}

impl Default for FieldRules {
    fn default() -> Self {
        Self {
            required: false,
            min_len: None,
            max_len: None,
            shape: ValueShape::Any,
        }
    }
}

fn looks_like_email(value: &str) -> bool {
    let Some((name, domain)) = value.split_once('@') else {
        return false;
    };
    !name.is_empty()
        && !domain.contains('@')
        && domain.split('.').count() >= 2
        && domain.split('.').all(|part| !part.is_empty())
        && !value.contains(char::is_whitespace)
}

/// The first rule `value` breaks, as a message, or `None` when it follows them all.
pub fn check_value(value: &str, rules: &FieldRules) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return rules.required.then(|| "Fill this in.".to_string());
    }
    let len = value.chars().count();
    if let Some(min) = rules.min_len
        && len < min
    {
        return Some(format!("Use at least {min} characters."));
    }
    if let Some(max) = rules.max_len
        && len > max
    {
        return Some(format!("Use {max} characters or fewer."));
    }
    match rules.shape {
        ValueShape::Any => None,
        ValueShape::Email => (!looks_like_email(value))
            .then(|| "Enter an email address, like name@example.com.".into()),
        ValueShape::WebAddress => {
            let rest = value
                .strip_prefix("https://")
                .or_else(|| value.strip_prefix("http://"));
            match rest {
                Some(rest) if rest.contains('.') && !rest.contains(char::is_whitespace) => None,
                _ => Some("Enter a web address starting with https://.".into()),
            }
        }
        ValueShape::Digits => {
            (!value.chars().all(|c| c.is_ascii_digit())).then(|| "Use digits only.".into())
        }
    }
}

/// A field that checks itself.
pub struct CheckedTextField {
    field: Entity<InputState>,
    label: SharedString,
    hint: Option<SharedString>,
    rules: FieldRules,
    touched: bool,
    _events: Subscription,
}

impl Focusable for CheckedTextField {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.field.focus_handle(cx)
    }
}

impl CheckedTextField {
    /// Create a field with a label above it and the rules it checks.
    pub fn new(
        label: impl Into<SharedString>,
        rules: FieldRules,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let field = cx.new(|cx| InputState::new(window, cx));
        let events = cx.subscribe(
            &field,
            |this: &mut Self, _, event: &InputEvent, cx| match event {
                InputEvent::Blur => {
                    this.touched = true;
                    cx.notify();
                }
                InputEvent::Change => cx.notify(),
                _ => {}
            },
        );
        Self {
            field,
            label: label.into(),
            hint: None,
            rules,
            touched: false,
            _events: events,
        }
    }

    /// Show a hint under the field while nothing is wrong.
    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Show placeholder text in the empty field.
    pub fn placeholder(
        self,
        text: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let text = text.into();
        self.field
            .update(cx, |field, cx| field.set_placeholder(text, window, cx));
        self
    }

    /// The text in the field.
    pub fn value(&self, cx: &App) -> SharedString {
        self.field.read(cx).value()
    }

    /// Whether the value follows every rule.
    pub fn is_valid(&self, cx: &App) -> bool {
        check_value(&self.value(cx), &self.rules).is_none()
    }

    /// Check now, as on submitting a form, and show any message.
    pub fn check_now(&mut self, cx: &mut Context<Self>) -> bool {
        self.touched = true;
        cx.notify();
        self.is_valid(cx)
    }
}

impl Render for CheckedTextField {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let danger = theme.danger;
        let success = theme.success;
        let value = self.value(cx);
        let problem = if self.touched {
            check_value(&value, &self.rules)
        } else {
            None
        };
        let good = self.touched && problem.is_none() && !value.trim().is_empty();
        let count = self
            .rules
            .max_len
            .map(|max| format!("{}/{max}", value.chars().count()));
        let label = if self.rules.required {
            format!("{} (required)", self.label)
        } else {
            self.label.to_string()
        };
        v_flex()
            .gap_1()
            .child(
                h_flex()
                    .justify_between()
                    .child(
                        div()
                            .text_sm()
                            .font_medium()
                            .child(self.label.clone())
                            .when(self.rules.required, |this| {
                                this.child(div().text_color(danger).child(" *"))
                            }),
                    )
                    .children(count.map(|c| div().text_xs().text_color(quiet).child(c))),
            )
            .child(
                Input::new(&self.field)
                    .aria_label(label)
                    .when(problem.is_some(), |this| this.border_color(danger))
                    .when(good, |this| {
                        this.suffix(Icon::new(IconName::Check).small().text_color(success))
                    }),
            )
            .map(|this| match (problem, self.hint.clone()) {
                (Some(problem), _) => this.child(
                    h_flex()
                        .gap_1()
                        .text_xs()
                        .text_color(danger)
                        .child(Icon::new(IconName::CircleAlert).xsmall())
                        .child(problem),
                ),
                (None, Some(hint)) => this.child(div().text_xs().text_color(quiet).child(hint)),
                (None, None) => this,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_report_the_first_problem() {
        let email = FieldRules {
            required: true,
            shape: ValueShape::Email,
            ..FieldRules::default()
        };
        assert_eq!(check_value("  ", &email).as_deref(), Some("Fill this in."));
        assert!(check_value("ana@harbor.dental", &email).is_none());
        assert!(check_value("ana@harbor", &email).is_some());
        assert!(check_value("ana harbor@x.com", &email).is_some());
        let site = FieldRules {
            shape: ValueShape::WebAddress,
            ..FieldRules::default()
        };
        assert!(check_value("", &site).is_none());
        assert!(check_value("https://harbor.dental", &site).is_none());
        assert!(check_value("harbor.dental", &site).is_some());
        let tax = FieldRules {
            min_len: Some(9),
            max_len: Some(9),
            shape: ValueShape::Digits,
            ..FieldRules::default()
        };
        assert_eq!(
            check_value("1234", &tax).as_deref(),
            Some("Use at least 9 characters.")
        );
        assert_eq!(
            check_value("1234567890", &tax).as_deref(),
            Some("Use 9 characters or fewer.")
        );
        assert_eq!(
            check_value("12345678a", &tax).as_deref(),
            Some("Use digits only.")
        );
        assert!(check_value("123456789", &tax).is_none());
    }
}
