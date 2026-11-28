// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! A ready-made row for a select or a searchable select: a label with an
//! optional leading icon, a trailing hint and a second line.

use gpui::{
    App, IntoElement, ParentElement as _, SharedString, Styled as _, Window, div,
    prelude::FluentBuilder as _,
};

use crate::{Icon, Sizable as _, TextRole as _, h_flex, v_flex};

use super::SearchableListItem;

/// An option in a select: a value and the label it reads as, with an
/// optional icon before the label, a short hint after it ("Owner", "12",
/// "Default") and a description under it.
///
/// Search matches the label, the hint and the description, so a person can
/// find "Lisbon" by typing its time zone if the zone is its hint.
#[derive(Clone)]
pub struct SelectOption<V: Clone + PartialEq> {
    value: V,
    label: SharedString,
    icon: Option<Icon>,
    hint: Option<SharedString>,
    description: Option<SharedString>,
    disabled: bool,
}

impl<V: Clone + PartialEq> SelectOption<V> {
    /// Create an option for `value` that reads as `label`.
    pub fn new(value: V, label: impl Into<SharedString>) -> Self {
        Self {
            value,
            label: label.into(),
            icon: None,
            hint: None,
            description: None,
            disabled: false,
        }
    }

    /// Lead the label with an icon.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Show a short hint at the end of the row.
    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Show a second line under the label.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Show the option but do not let it be chosen.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// Whether an option with this label, hint and description matches a search
/// query, ignoring case.
pub(crate) fn option_matches(
    label: &str,
    hint: Option<&str>,
    description: Option<&str>,
    query: &str,
) -> bool {
    let query = query.to_lowercase();
    [Some(label), hint, description]
        .into_iter()
        .flatten()
        .any(|text| text.to_lowercase().contains(&query))
}

impl<V: Clone + PartialEq + 'static> SearchableListItem for SelectOption<V> {
    type Value = V;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let subtle = crate::look::ink::subtle(cx);
        h_flex()
            .w_full()
            .min_w_0()
            .gap_2()
            .items_center()
            .when_some(self.icon.clone(), |this, icon| {
                this.child(icon.small().text_color(crate::look::ink::icon(cx)))
            })
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().truncate().child(self.label.clone()))
                    .when_some(self.description.clone(), |this, description| {
                        this.child(
                            div()
                                .text_caption()
                                .text_color(subtle)
                                .truncate()
                                .child(description),
                        )
                    }),
            )
            .when_some(self.hint.clone(), |this, hint| {
                this.child(
                    div()
                        .flex_none()
                        .text_caption()
                        .text_color(subtle)
                        .child(hint),
                )
            })
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }

    fn matches(&self, query: &str) -> bool {
        option_matches(
            &self.label,
            self.hint.as_deref(),
            self.description.as_deref(),
            query,
        )
    }

    fn disabled(&self) -> bool {
        self.disabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_reaches_the_hint_and_description() {
        assert!(option_matches("Lisbon", Some("UTC+0"), None, "lis"));
        assert!(option_matches("Lisbon", Some("UTC+0"), None, "utc"));
        assert!(option_matches("Lisbon", None, Some("Portugal"), "PORT"));
        assert!(!option_matches("Lisbon", Some("UTC+0"), None, "tokyo"));
    }

    #[test]
    fn the_label_is_the_title_and_value_is_kept() {
        let option = SelectOption::new(7_u32, "Seven").hint("odd").disabled(true);
        assert_eq!(option.title(), "Seven");
        assert_eq!(*option.value(), 7);
        assert!(SearchableListItem::disabled(&option));
    }
}
