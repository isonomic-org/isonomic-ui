// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Form select: a form field that picks one value from a list split into named groups, with a placeholder, a required mark, an error line and a way to clear.
//!
//! The field shows the picked option, or the placeholder in a quieter ink
//! until something is picked. Its menu lists each group under its name,
//! ticks the picked option, and greys out options that can't be picked.
//! An optional "Clear" entry at the end empties the field.

use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::Button,
    h_flex,
    menu::{DropdownMenu as _, PopupMenuItem},
    v_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, px,
};

/// One option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectOption {
    /// The value reported when picked.
    pub value: SharedString,
    /// What shows.
    pub label: SharedString,
    /// Whether it can be picked.
    pub enabled: bool,
}

impl SelectOption {
    /// Create an option that can be picked.
    pub fn new(value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            enabled: true,
        }
    }

    /// Grey it out.
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

/// Options under a group name. An empty name means no heading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectGroup {
    /// The heading.
    pub name: SharedString,
    /// The options under it.
    pub options: Vec<SelectOption>,
}

impl SelectGroup {
    /// Create a group.
    pub fn new(name: impl Into<SharedString>, options: Vec<SelectOption>) -> Self {
        Self {
            name: name.into(),
            options,
        }
    }
}

/// The label of the option holding `value`, searching every group.
pub fn label_for<'a>(groups: &'a [SelectGroup], value: &str) -> Option<&'a SharedString> {
    groups
        .iter()
        .flat_map(|g| &g.options)
        .find(|o| o.value == value)
        .map(|o| &o.label)
}

type PickHandler = Rc<dyn Fn(Option<SharedString>, &mut Window, &mut App)>;

/// A field that picks one value.
#[derive(IntoElement)]
pub struct FormSelect {
    id: ElementId,
    style: StyleRefinement,
    label: SharedString,
    groups: Vec<SelectGroup>,
    value: Option<SharedString>,
    placeholder: SharedString,
    required: bool,
    clearable: bool,
    error: Option<SharedString>,
    on_pick: Option<PickHandler>,
}

impl FormSelect {
    /// Create a field named `label` choosing among `groups`.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        groups: Vec<SelectGroup>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            label: label.into(),
            groups,
            value: None,
            placeholder: "Choose\u{2026}".into(),
            required: false,
            clearable: false,
            error: None,
            on_pick: None,
        }
    }

    /// Set the picked value.
    pub fn value(mut self, value: Option<SharedString>) -> Self {
        self.value = value;
        self
    }

    /// Show this until something is picked.
    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// Mark the field as required.
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// Offer "Clear" at the end of the menu.
    pub fn clearable(mut self) -> Self {
        self.clearable = true;
        self
    }

    /// Show what's wrong under the field.
    pub fn error(mut self, error: Option<SharedString>) -> Self {
        self.error = error;
        self
    }

    /// Run with the value picked, or `None` when cleared.
    pub fn on_pick(
        mut self,
        handler: impl Fn(Option<SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_pick = Some(Rc::new(handler));
        self
    }
}

impl Styled for FormSelect {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for FormSelect {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let danger = theme.danger;
        let picked = self
            .value
            .as_ref()
            .and_then(|v| label_for(&self.groups, v))
            .cloned();
        let shown = picked.clone().unwrap_or_else(|| self.placeholder.clone());
        let spoken = format!(
            "{}{}: {}",
            self.label,
            if self.required { ", required" } else { "" },
            picked.as_deref().unwrap_or("nothing picked")
        );
        let groups = self.groups.clone();
        let value = self.value.clone();
        let pick = self.on_pick.clone();
        let clearable = self.clearable && self.value.is_some();
        let empty = picked.is_none();

        v_flex()
            .id(self.id.clone())
            .gap_1()
            .child(
                h_flex()
                    .gap_0p5()
                    .text_label()
                    .child(self.label.clone())
                    .when(self.required, |this| {
                        this.child(div().text_color(danger).child("*"))
                    }),
            )
            .child(
                Button::new(ElementId::NamedChild(
                    self.id.clone().into(),
                    "field".into(),
                ))
                .outline()
                .w_full()
                .when(self.error.is_some(), |this| this.border_color(danger))
                .child(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .truncate()
                                .when(empty, |this| this.text_color(quiet))
                                .child(shown),
                        )
                        .child(
                            Icon::new(IconName::ChevronsUpDown)
                                .xsmall()
                                .text_color(quiet),
                        ),
                )
                .accessibility_label(spoken)
                .dropdown_menu(move |mut menu, _, _| {
                    for (ix, group) in groups.iter().enumerate() {
                        if ix > 0 {
                            menu = menu.separator();
                        }
                        if !group.name.is_empty() {
                            menu = menu.label(group.name.clone());
                        }
                        for option in &group.options {
                            let pick = pick.clone();
                            let chosen = option.value.clone();
                            menu = menu.item(
                                PopupMenuItem::new(option.label.clone())
                                    .checked(value.as_ref() == Some(&option.value))
                                    .disabled(!option.enabled)
                                    .on_click(move |_, window, cx| {
                                        if let Some(pick) = &pick {
                                            pick(Some(chosen.clone()), window, cx);
                                        }
                                    }),
                            );
                        }
                    }
                    if clearable {
                        let pick = pick.clone();
                        menu = menu.separator().item(PopupMenuItem::new("Clear").on_click(
                            move |_, window, cx| {
                                if let Some(pick) = &pick {
                                    pick(None, window, cx);
                                }
                            },
                        ));
                    }
                    menu.min_w(px(240.)).max_h(px(320.)).scrollable(true)
                }),
            )
            .when_some(self.error, |this, error| {
                this.child(
                    h_flex()
                        .gap_1()
                        .text_caption()
                        .text_color(danger)
                        .child(Icon::new(IconName::CircleAlert).xsmall())
                        .child(error),
                )
            })
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_found_across_groups() {
        let groups = vec![
            SelectGroup::new("Income", vec![SelectOption::new("4000", "Sales")]),
            SelectGroup::new(
                "Expenses",
                vec![
                    SelectOption::new("6100", "Fuel"),
                    SelectOption::new("6900", "Old").disabled(),
                ],
            ),
        ];
        assert_eq!(label_for(&groups, "6100").map(|s| s.as_ref()), Some("Fuel"));
        assert_eq!(label_for(&groups, "9999"), None);
        assert!(!groups[1].options[1].enabled);
    }
}
