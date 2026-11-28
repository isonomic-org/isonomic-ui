// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Labelled field: any control with its label, a required or optional mark, a hint, an error and a character count, laid out the same way everywhere.
//!
//! The label sits above the control with "Required" or "Optional" beside
//! it, and an optional description under the label. Under the control, an
//! error replaces the hint while there is one, in the danger colour, and a
//! count shows when there's a limit, turning red past it. The field passes its
//! label, hint and error to screen readers as one name.
//!
//! A text field or text area handed over with [`LabelledField::input`] or
//! [`LabelledField::textarea`] is wired up too: it takes the label as its own
//! name, the description, hint or error as its spoken description, and the
//! danger ring while there is an error.

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex,
    input::{Input, Textarea},
    look, v_flex,
};
use isonomic_ui::{
    AnyElement, App, ElementId, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};

use crate::common::Tone;

/// How a count reads against a limit: the words and whether it's over.
pub fn count_words(used: usize, limit: usize) -> (String, bool) {
    (format!("{used} / {limit}"), used > limit)
}

/// The name a screen reader hears.
pub fn field_name(
    label: &str,
    required: Option<bool>,
    hint: Option<&str>,
    error: Option<&str>,
) -> String {
    let mut out = control_name(label, required);
    if let Some(error) = error {
        out.push_str(&format!(". Error: {error}"));
    } else if let Some(hint) = hint {
        out.push_str(&format!(". {hint}"));
    }
    out
}

/// The name a wired text field takes: the label and whether it's required.
pub fn control_name(label: &str, required: Option<bool>) -> String {
    let mut out = label.to_string();
    match required {
        Some(true) => out.push_str(", required"),
        Some(false) => out.push_str(", optional"),
        None => {}
    }
    out
}

/// What a wired text field reads after its name: the error while there is
/// one, otherwise the description and the hint, one sentence each.
pub fn control_description(
    description: Option<&str>,
    hint: Option<&str>,
    error: Option<&str>,
) -> Option<String> {
    if let Some(error) = error {
        return Some(format!("Error: {error}"));
    }
    let parts: Vec<&str> = [description, hint]
        .into_iter()
        .flatten()
        .map(|text| text.trim_end_matches('.'))
        .filter(|text| !text.is_empty())
        .collect();
    (!parts.is_empty()).then(|| parts.join(". "))
}

/// A text control the field wires its words into.
enum Control {
    Input(Input),
    Textarea(Textarea),
}

/// A control with its label, hint, error and count.
#[derive(IntoElement)]
pub struct LabelledField {
    id: ElementId,
    style: StyleRefinement,
    label: SharedString,
    required: Option<bool>,
    description: Option<SharedString>,
    hint: Option<SharedString>,
    error: Option<SharedString>,
    count: Option<(usize, usize)>,
    control: Option<Control>,
    children: Vec<AnyElement>,
}

impl LabelledField {
    /// Create a field with its label.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            label: label.into(),
            required: None,
            description: None,
            hint: None,
            error: None,
            count: None,
            control: None,
            children: Vec::new(),
        }
    }

    /// Mark it required (`true`) or optional (`false`).
    pub fn required(mut self, required: bool) -> Self {
        self.required = Some(required);
        self
    }

    /// Add a description under the label, above the control.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Hand over the text field this labels. It takes the label as its name,
    /// the description, hint or error as its spoken description, and the
    /// danger ring while there is an error. It goes before any children.
    pub fn input(mut self, input: Input) -> Self {
        self.control = Some(Control::Input(input));
        self
    }

    /// Hand over the text area this labels, wired like [`Self::input`].
    pub fn textarea(mut self, textarea: Textarea) -> Self {
        self.control = Some(Control::Textarea(textarea));
        self
    }

    /// Add a hint under the control.
    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Show an error in place of the hint.
    pub fn error(mut self, error: Option<SharedString>) -> Self {
        self.error = error;
        self
    }

    /// Show how many characters are used of a limit.
    pub fn count(mut self, used: usize, limit: usize) -> Self {
        self.count = Some((used, limit));
        self
    }
}

impl ParentElement for LabelledField {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for LabelledField {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for LabelledField {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let danger = Tone::Danger.color(cx);
        let name = field_name(
            &self.label,
            self.required,
            self.hint.as_deref(),
            self.error.as_deref(),
        );
        let count = self.count.map(|(used, limit)| count_words(used, limit));
        let invalid = self.error.is_some();
        let spoken_name: SharedString = control_name(&self.label, self.required).into();
        let spoken_description: Option<SharedString> = control_description(
            self.description.as_deref(),
            self.hint.as_deref(),
            self.error.as_deref(),
        )
        .map(Into::into);
        let control = self.control.map(|control| match control {
            Control::Input(input) => input
                .aria_label(spoken_name)
                .invalid(invalid)
                .when_some(spoken_description, |this, text| this.description(text))
                .into_any_element(),
            Control::Textarea(textarea) => textarea
                .aria_label(spoken_name)
                .invalid(invalid)
                .when_some(spoken_description, |this, text| this.description(text))
                .into_any_element(),
        });
        v_flex()
            .id(self.id.clone())
            .w_full()
            .gap_1p5()
            .text_body()
            .role(Role::Group)
            .aria_label(name)
            .child(
                h_flex()
                    .gap_2()
                    .child(div().font_medium().child(self.label.clone()))
                    .children(self.required.map(|required| {
                        div()
                            .text_caption()
                            .text_color(if required { danger } else { quiet })
                            .child(if required { "Required" } else { "Optional" })
                    })),
            )
            .children(self.description.clone().map(|description| {
                div()
                    .mt(-px(2.))
                    .text_caption()
                    .text_color(look::ink::subtle(cx))
                    .child(description)
            }))
            .children(control)
            .children(self.children)
            .when(
                self.error.is_some() || self.hint.is_some() || count.is_some(),
                |this| {
                    this.child(
                        h_flex()
                            .gap_2()
                            .text_caption()
                            .child(match &self.error {
                                Some(error) => h_flex()
                                    .flex_1()
                                    .gap_1()
                                    .text_color(danger)
                                    .child(Icon::new(IconName::CircleAlert).xsmall())
                                    .child(error.clone())
                                    .into_any_element(),
                                None => div()
                                    .flex_1()
                                    .text_color(quiet)
                                    .children(self.hint.clone())
                                    .into_any_element(),
                            })
                            .children(count.map(|(words, over)| {
                                div()
                                    .text_color(if over { danger } else { quiet })
                                    .child(words)
                            })),
                    )
                },
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_counts_read_naturally() {
        assert_eq!(
            field_name("Email", Some(true), Some("For receipts"), None),
            "Email, required. For receipts"
        );
        assert_eq!(
            field_name(
                "Email",
                Some(true),
                Some("For receipts"),
                Some("Not an address")
            ),
            "Email, required. Error: Not an address"
        );
        assert_eq!(count_words(141, 140), ("141 / 140".to_string(), true));
    }

    #[test]
    fn wired_controls_hear_their_label_and_the_words_around_it() {
        assert_eq!(control_name("Email", Some(false)), "Email, optional");
        assert_eq!(control_name("Email", None), "Email");
        assert_eq!(
            control_description(Some("Used for sign in."), Some("Never shared"), None),
            Some("Used for sign in. Never shared".to_string())
        );
        assert_eq!(
            control_description(
                Some("Used for sign in"),
                Some("Never shared"),
                Some("Taken")
            ),
            Some("Error: Taken".to_string())
        );
        assert_eq!(control_description(None, Some(""), None), None);
        assert_eq!(control_description(None, None, None), None);
    }
}
