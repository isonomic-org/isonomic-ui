// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Row actions menu: one list of actions for a row, opened by right-click on the row or from its "more" button.
//!
//! The kit's context menu opens on right-click only, which leaves keyboard
//! and touch users without it. Here the same actions also sit behind a
//! visible "more" button at the row's end. Actions come in sections; an
//! action that can't run says why in its label; destructive ones are
//! grouped last.

use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenu, PopupMenuItem},
};
use isonomic_ui::{
    App, Context, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
};

/// One action.
#[derive(Debug, Clone, PartialEq)]
pub struct RowAction {
    /// Identifies it.
    pub id: SharedString,
    /// Its label.
    pub label: SharedString,
    /// Its section, such as "Send".
    pub section: SharedString,
    /// Why it can't run now, if it can't.
    pub unavailable: Option<SharedString>,
    /// Whether it destroys something.
    pub destructive: bool,
}

impl RowAction {
    /// Create an action.
    pub fn new(
        id: impl Into<SharedString>,
        section: impl Into<SharedString>,
        label: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            section: section.into(),
            unavailable: None,
            destructive: false,
        }
    }

    /// Say why it can't run now.
    pub fn unavailable(mut self, why: impl Into<SharedString>) -> Self {
        self.unavailable = Some(why.into());
        self
    }

    /// Mark it as destructive.
    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    /// The label as the menu shows it.
    pub fn menu_label(&self) -> String {
        match &self.unavailable {
            Some(why) => format!("{} ({why})", self.label),
            None => self.label.to_string(),
        }
    }
}

/// The actions in menu order: sections in first-seen order, destructive sections last.
pub fn menu_order(actions: &[RowAction]) -> Vec<(SharedString, Vec<RowAction>)> {
    let mut sections: Vec<(SharedString, Vec<RowAction>)> = Vec::new();
    for action in actions {
        match sections.iter_mut().find(|(s, _)| s == &action.section) {
            Some((_, list)) => list.push(action.clone()),
            None => sections.push((action.section.clone(), vec![action.clone()])),
        }
    }
    sections.sort_by_key(|(_, list)| list.iter().all(|a| a.destructive));
    sections
}

type RunHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

fn build_menu(
    mut menu: PopupMenu,
    actions: &[RowAction],
    run: &Option<RunHandler>,
    _: &mut Context<PopupMenu>,
) -> PopupMenu {
    for (ix, (section, list)) in menu_order(actions).into_iter().enumerate() {
        if ix > 0 {
            menu = menu.separator();
        }
        menu = menu.label(section);
        for action in list {
            let run = run.clone();
            let id = action.id.clone();
            menu = menu.item(
                PopupMenuItem::new(action.menu_label())
                    .when_icon(action.destructive)
                    .disabled(action.unavailable.is_some())
                    .on_click(move |_, window, cx| {
                        if let Some(run) = &run {
                            run(&id, window, cx);
                        }
                    }),
            );
        }
    }
    menu
}

trait DestructiveMark {
    fn when_icon(self, destructive: bool) -> Self;
}

impl DestructiveMark for PopupMenuItem {
    fn when_icon(self, destructive: bool) -> Self {
        if destructive {
            self.icon(IconName::Delete)
        } else {
            self
        }
    }
}

/// A row with its actions on right-click and behind a "more" button.
#[derive(IntoElement)]
pub struct RowActionsMenu {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    detail: SharedString,
    actions: Vec<RowAction>,
    on_run: Option<RunHandler>,
}

impl RowActionsMenu {
    /// Create a row with a title and detail line.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        detail: impl Into<SharedString>,
        actions: Vec<RowAction>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            detail: detail.into(),
            actions,
            on_run: None,
        }
    }

    /// Run with the action picked.
    pub fn on_run(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_run = Some(Rc::new(handler));
        self
    }
}

impl Styled for RowActionsMenu {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for RowActionsMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (row_fade, row_hover) = isonomic_ui::component::look::tracked_hover(
            ElementId::NamedChild(self.id.clone().into(), "row-hover".into()),
            true,
            window,
            cx,
        );
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let chip = theme.muted;
        let (a, b) = (Rc::new(self.actions.clone()), Rc::new(self.actions.clone()));
        let (ra, rb) = (self.on_run.clone(), self.on_run.clone());
        h_flex()
            .id(self.id.clone())
            .gap_3()
            .px_3()
            .py_2()
            .rounded(theme.radius)
            .bg(crate::common::faded(chip.opacity(0.6), row_fade))
            .on_hover(row_hover)
            .role(Role::Group)
            .aria_label(format!(
                "{}, {}. Right-click or use More for actions",
                self.title, self.detail
            ))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(div().truncate().child(self.title.clone()))
                    .child(
                        div()
                            .truncate()
                            .text_caption()
                            .text_color(quiet)
                            .child(self.detail.clone()),
                    ),
            )
            .child(
                Button::new(ElementId::NamedChild(self.id.clone().into(), "more".into()))
                    .xsmall()
                    .ghost()
                    .icon(IconName::Ellipsis)
                    .accessibility_label(format!("More actions for {}", self.title))
                    .dropdown_menu(move |menu, _, cx| build_menu(menu, &a, &ra, cx)),
            )
            .refine_style(&self.style)
            .context_menu(move |menu, _, cx| build_menu(menu, &b, &rb, cx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destructive_sections_go_last() {
        let actions = vec![
            RowAction::new("void", "Remove", "Void").destructive(),
            RowAction::new("send", "Send", "Send again"),
            RowAction::new("pdf", "Export", "Download PDF"),
            RowAction::new("remind", "Send", "Send a reminder").unavailable("Not due yet"),
        ];
        let order: Vec<_> = menu_order(&actions)
            .into_iter()
            .map(|(s, l)| (s.to_string(), l.len()))
            .collect();
        assert_eq!(
            order,
            [
                ("Send".into(), 2),
                ("Export".into(), 1),
                ("Remove".into(), 1)
            ]
        );
        assert_eq!(actions[3].menu_label(), "Send a reminder (Not due yet)");
    }
}
