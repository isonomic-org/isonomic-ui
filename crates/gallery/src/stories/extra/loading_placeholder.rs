// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the loading placeholder.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div, px,
};
use isonomic_ui_extra::loading_placeholder::{LoadingPlaceholder, PlaceholderShape};

/// Each shape the placeholder takes while a bookkeeping dashboard loads.
pub struct LoadingPlaceholderStory;

impl LoadingPlaceholderStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self)
    }
}

impl Story for LoadingPlaceholderStory {
    fn title() -> &'static str {
        "Loading placeholder"
    }

    fn description() -> &'static str {
        "Grey shapes that stand in for content while it loads, shaped like the lines, rows, cards or table it will become."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for LoadingPlaceholderStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let caption = |text: &'static str| div().text_xs().text_color(quiet).child(text);
        v_flex()
            .gap_6()
            .child(
                h_flex()
                    .items_start()
                    .gap_8()
                    .child(
                        v_flex().w(px(300.)).gap_2().child(caption("Notes")).child(
                            LoadingPlaceholder::new("notes", PlaceholderShape::Lines(4))
                                .label("Loading notes"),
                        ),
                    )
                    .child(
                        v_flex()
                            .w(px(300.))
                            .gap_2()
                            .child(caption("Clients"))
                            .child(
                                LoadingPlaceholder::new("clients", PlaceholderShape::MediaRow)
                                    .repeat(3)
                                    .label("Loading clients"),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .items_start()
                    .gap_8()
                    .child(
                        v_flex()
                            .w(px(240.))
                            .gap_2()
                            .child(caption("Report card"))
                            .child(
                                LoadingPlaceholder::new("report", PlaceholderShape::Card)
                                    .label("Loading report"),
                            ),
                    )
                    .child(
                        v_flex()
                            .w(px(420.))
                            .gap_2()
                            .child(caption("Invoices table"))
                            .child(
                                LoadingPlaceholder::new(
                                    "invoices",
                                    PlaceholderShape::Table {
                                        rows: 4,
                                        columns: 4,
                                    },
                                )
                                .label("Loading invoices"),
                            ),
                    ),
            )
    }
}
