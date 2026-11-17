// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the wordmark.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div, px,
};
use isonomic_ui_extra::wordmark::Wordmark;

/// The name alone and with two products, at the default size and larger.
pub struct WordmarkStory;

impl WordmarkStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self)
    }
}

impl Story for WordmarkStory {
    fn title() -> &'static str {
        "Wordmark"
    }

    fn description() -> &'static str {
        "The name Isonomic in the sans at regular weight, with a product's name after it in light."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for WordmarkStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let heading = |text: &'static str| div().text_xs().text_color(quiet).child(text);
        v_flex()
            .gap_6()
            .child(
                v_flex()
                    .gap_2()
                    .child(heading("The name alone, and with a product"))
                    .child(Wordmark::new())
                    .child(Wordmark::product("Chat"))
                    .child(Wordmark::product("Docs")),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(heading("Larger"))
                    .child(Wordmark::new().size(px(40.)))
                    .child(Wordmark::product("Chat").size(px(40.)))
                    .child(Wordmark::product("Docs").size(px(40.))),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(heading("In another colour"))
                    .child(Wordmark::product("Chat").size(px(24.)).color(quiet)),
            )
    }
}
