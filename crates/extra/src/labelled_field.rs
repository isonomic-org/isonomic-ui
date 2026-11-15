//! Labelled field: any control with its label, a required or optional mark, a hint, an error and a character count, laid out the same way everywhere.
//!
//! The label sits above the control with "Required" or "Optional" beside
//! it. Under the control, an error replaces the hint while there is one, in
//! the danger colour, and a count shows when there's a limit, turning red
//! past it. The field passes its label, hint and error to screen readers as
//! one name.

use isonomic_ui::component::{ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex, v_flex};
use isonomic_ui::{
    AnyElement, App, ElementId, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _,
};

use crate::common::Tone;

/// How a count reads against a limit: the words and whether it's over.
pub fn count_words(used: usize, limit: usize) -> (String, bool) {
    (format!("{used} / {limit}"), used > limit)
}

/// The name a screen reader hears.
pub fn field_name(label: &str, required: Option<bool>, hint: Option<&str>, error: Option<&str>) -> String {
    let mut out = label.to_string();
    match required {
        Some(true) => out.push_str(", required"),
        Some(false) => out.push_str(", optional"),
        None => {}
    }
    if let Some(error) = error {
        out.push_str(&format!(". Error: {error}"));
    } else if let Some(hint) = hint {
        out.push_str(&format!(". {hint}"));
    }
    out
}

/// A control with its label, hint, error and count.
#[derive(IntoElement)]
pub struct LabelledField {
    id: ElementId,
    style: StyleRefinement,
    label: SharedString,
    required: Option<bool>,
    hint: Option<SharedString>,
    error: Option<SharedString>,
    count: Option<(usize, usize)>,
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
            hint: None,
            error: None,
            count: None,
            children: Vec::new(),
        }
    }

    /// Mark it required (`true`) or optional (`false`).
    pub fn required(mut self, required: bool) -> Self {
        self.required = Some(required);
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
        let name = field_name(&self.label, self.required, self.hint.as_deref(), self.error.as_deref());
        let count = self.count.map(|(used, limit)| count_words(used, limit));
        v_flex()
            .id(self.id.clone())
            .w_full()
            .gap_1p5()
            .text_sm()
            .aria_label(name)
            .child(
                h_flex()
                    .gap_2()
                    .child(div().font_medium().child(self.label.clone()))
                    .children(self.required.map(|required| {
                        div()
                            .text_xs()
                            .text_color(if required { danger } else { quiet })
                            .child(if required { "Required" } else { "Optional" })
                    })),
            )
            .children(self.children)
            .when(self.error.is_some() || self.hint.is_some() || count.is_some(), |this| {
                this.child(
                    h_flex()
                        .gap_2()
                        .text_xs()
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
                        .children(count.map(|(words, over)| div().text_color(if over { danger } else { quiet }).child(words))),
                )
            })
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_counts_read_naturally() {
        assert_eq!(field_name("Email", Some(true), Some("For receipts"), None), "Email, required. For receipts");
        assert_eq!(field_name("Email", Some(true), Some("For receipts"), Some("Not an address")), "Email, required. Error: Not an address");
        assert_eq!(count_words(141, 140), ("141 / 140".to_string(), true));
    }
}
