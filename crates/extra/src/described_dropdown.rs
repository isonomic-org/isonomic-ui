// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Described dropdown: a button whose menu explains each choice in a second line, marks the current one, and keeps destructive choices apart.
//!
//! Each choice has a title and a short description, so "Weekly" can say
//! "Every Monday at 9 am" right in the menu. The current choice is ticked
//! and named on the button. Destructive choices sit below a line, in the
//! danger colour.

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
    App, ElementId, IntoElement, ParentElement as _, RenderOnce, SharedString, Styled as _, Window,
    div, px,
};

/// One choice.
#[derive(Debug, Clone, PartialEq)]
pub struct DescribedChoice {
    /// Identifies it.
    pub id: SharedString,
    /// Its title.
    pub title: SharedString,
    /// What it means, in a few words.
    pub description: SharedString,
    /// Whether it destroys something.
    pub destructive: bool,
}

impl DescribedChoice {
    /// Create a choice.
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        description: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: description.into(),
            destructive: false,
        }
    }

    /// Mark it as destructive.
    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }
}

/// The choices split into ordinary and destructive, each in order.
pub fn split_choices(
    choices: &[DescribedChoice],
) -> (Vec<&DescribedChoice>, Vec<&DescribedChoice>) {
    choices.iter().partition(|c| !c.destructive)
}

type PickHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// A dropdown whose choices are described.
#[derive(IntoElement)]
pub struct DescribedDropdown {
    id: ElementId,
    label: SharedString,
    choices: Vec<DescribedChoice>,
    current: Option<SharedString>,
    on_pick: Option<PickHandler>,
}

impl DescribedDropdown {
    /// Create a dropdown with a label for the button.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        choices: Vec<DescribedChoice>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            choices,
            current: None,
            on_pick: None,
        }
    }

    /// The current choice.
    pub fn current(mut self, id: Option<SharedString>) -> Self {
        self.current = id;
        self
    }

    /// Run with the choice picked.
    pub fn on_pick(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_pick = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for DescribedDropdown {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let current_title = self
            .current
            .as_ref()
            .and_then(|id| self.choices.iter().find(|c| &c.id == id))
            .map(|c| c.title.clone());
        let button_label = match &current_title {
            Some(title) => format!("{}: {title}", self.label),
            None => self.label.to_string(),
        };
        let choices = Rc::new(self.choices.clone());
        let current = self.current.clone();
        let pick = self.on_pick.clone();
        Button::new(self.id.clone())
            .small()
            .outline()
            .label(button_label.clone())
            .dropdown_caret(true)
            .accessibility_label(button_label)
            .dropdown_menu(move |menu, _, _| {
                let (plain, risky) = split_choices(&choices);
                let entry = |menu: isonomic_ui::component::menu::PopupMenu,
                             choice: &DescribedChoice| {
                    let pick = pick.clone();
                    let id = choice.id.clone();
                    let (title, description, destructive) = (
                        choice.title.clone(),
                        choice.description.clone(),
                        choice.destructive,
                    );
                    let ticked = current.as_ref() == Some(&choice.id);
                    menu.item(
                        PopupMenuItem::element(move |_, cx| {
                            let theme = cx.theme();
                            h_flex()
                                .gap_2()
                                .min_w(px(220.))
                                .py_0p5()
                                .child(div().w(px(14.)).child(if ticked {
                                    Icon::new(IconName::Check)
                                        .xsmall()
                                        .text_color(theme.primary)
                                        .into_any_element()
                                } else {
                                    div().into_any_element()
                                }))
                                .child(
                                    v_flex()
                                        .child(
                                            div()
                                                .font_medium()
                                                .text_color(if destructive {
                                                    theme.danger
                                                } else {
                                                    theme.foreground
                                                })
                                                .child(title.clone()),
                                        )
                                        .child(
                                            div()
                                                .text_caption()
                                                .text_color(theme.muted_foreground)
                                                .child(description.clone()),
                                        ),
                                )
                        })
                        .on_click(move |_, window, cx| {
                            if let Some(pick) = &pick {
                                pick(&id, window, cx);
                            }
                        }),
                    )
                };
                let mut menu = plain.into_iter().fold(menu, entry);
                if !risky.is_empty() {
                    menu = menu.separator();
                    menu = risky.into_iter().fold(menu, entry);
                }
                menu
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destructive_choices_are_kept_apart() {
        let choices = vec![
            DescribedChoice::new("w", "Weekly", "Every Monday"),
            DescribedChoice::new("off", "Turn off", "No more reports").destructive(),
            DescribedChoice::new("m", "Monthly", "On the 1st"),
        ];
        let (plain, risky) = split_choices(&choices);
        assert_eq!(plain.len(), 2);
        assert_eq!(risky[0].id.as_ref(), "off");
    }
}
