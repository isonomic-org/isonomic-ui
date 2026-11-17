// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the explained tooltip.

use crate::Story;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div,
};
use isonomic_ui_extra::explained_tooltip::ExplainedTooltip;

/// A ledger toolbar: hover each control for what it does, its shortcut, or
/// why it's off.
pub struct ExplainedTooltipStory;

impl ExplainedTooltipStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self)
    }
}

impl Story for ExplainedTooltipStory {
    fn title() -> &'static str {
        "Explained tooltip"
    }

    fn description() -> &'static str {
        "A hover note with a title, a line of explanation and its shortcut, or the reason a control is off."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ExplainedTooltipStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        ExplainedTooltip::new(
                            "tip-search",
                            Button::new("search").small().ghost().icon(IconName::Search),
                            "Search entries",
                        )
                        .shortcut("mod-k"),
                    )
                    .child(
                        ExplainedTooltip::new(
                            "tip-export",
                            Button::new("export").small().outline().label("Export"),
                            "Export to a spreadsheet",
                        )
                        .body(
                            "Writes the entries you can see, with your filters, to an .xlsx file.",
                        )
                        .shortcut("mod-shift-e"),
                    )
                    .child(
                        ExplainedTooltip::new(
                            "tip-close",
                            Button::new("close-month")
                                .small()
                                .primary()
                                .label("Close November")
                                .disabled(true),
                            "Close the month",
                        )
                        .body("Locks every entry dated before 1 December.")
                        .off_because("Two bank feeds still have unmatched transactions."),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(quiet)
                    .child("Hover each control."),
            )
    }
}
