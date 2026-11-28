// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the toggle button set.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, IconName, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div,
};
use isonomic_ui_extra::toggle_button_set::{PressMode, ToggleButtonSet, ToggleItem};

/// Columns to show in a ledger (any number), a sort direction (at most
/// one), and a view that must always be picked (exactly one).
pub struct ToggleButtonSetStory {
    columns: Vec<bool>,
    sort: Vec<bool>,
    view: Vec<bool>,
}

impl ToggleButtonSetStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            columns: vec![true, true, false, false],
            sort: vec![false, true],
            view: vec![true, false, false],
        })
    }
}

impl Story for ToggleButtonSetStory {
    fn title() -> &'static str {
        "Toggle button set"
    }

    fn description() -> &'static str {
        "A row of buttons that each stay pressed or not, where any number, at most one or exactly one can be pressed."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ToggleButtonSetStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let caption = |text: &'static str| div().text_xs().text_color(quiet).child(text);
        let columns = cx.entity();
        let sort = cx.entity();
        let view = cx.entity();
        v_flex()
            .gap_3()
            .child(caption("Columns, any number"))
            .child(
                ToggleButtonSet::new(
                    "columns",
                    vec![
                        ToggleItem::text("Date"),
                        ToggleItem::text("Payee"),
                        ToggleItem::text("Category"),
                        ToggleItem::text("Memo"),
                    ],
                    self.columns.clone(),
                )
                .label("Ledger columns")
                .on_change(move |next, _, cx| {
                    columns.update(cx, |this, cx| {
                        this.columns = next;
                        cx.notify();
                    })
                }),
            )
            .child(caption("Sort by amount, at most one"))
            .child(
                ToggleButtonSet::new(
                    "sort",
                    vec![
                        ToggleItem::icon(IconName::SortAscending, "Smallest first"),
                        ToggleItem::icon(IconName::SortDescending, "Largest first"),
                    ],
                    self.sort.clone(),
                )
                .mode(PressMode::One)
                .label("Sort by amount")
                .on_change(move |next, _, cx| {
                    sort.update(cx, |this, cx| {
                        this.sort = next;
                        cx.notify();
                    })
                }),
            )
            .child(caption("View, exactly one"))
            .child(
                ToggleButtonSet::new(
                    "view",
                    vec![
                        ToggleItem::icon(IconName::Menu, "List"),
                        ToggleItem::icon(IconName::LayoutDashboard, "Board"),
                        ToggleItem::icon(IconName::Calendar, "Calendar"),
                    ],
                    self.view.clone(),
                )
                .mode(PressMode::ExactlyOne)
                .label("View")
                .on_change(move |next, _, cx| {
                    view.update(cx, |this, cx| {
                        this.view = next;
                        cx.notify();
                    })
                }),
            )
    }
}
