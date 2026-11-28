// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the page navigator.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div,
};
use isonomic_ui_extra::page_navigator::PageNavigator;

/// A ledger of 312 entries paged twenty at a time, and a short list on one page.
pub struct PageNavigatorStory {
    page: usize,
    per_page: usize,
}

impl PageNavigatorStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            page: 5,
            per_page: 20,
        })
    }
}

impl Story for PageNavigatorStory {
    fn title() -> &'static str {
        "Page navigator"
    }

    fn description() -> &'static str {
        "Moves through a long list a page at a time, showing which rows are on screen and the pages around the current one."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for PageNavigatorStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let on_page = cx.entity();
        let on_size = cx.entity();
        v_flex()
            .gap_6()
            .max_w_full()
            .child(
                div()
                    .text_sm()
                    .child(format!("Ledger entries, page {}", self.page)),
            )
            .child(
                PageNavigator::new("ledger-pages", self.page, 312)
                    .per_page(self.per_page)
                    .sizes([10, 20, 50, 100])
                    .on_page(move |page, _, cx| {
                        on_page.update(cx, |this, cx| {
                            this.page = page;
                            cx.notify();
                        })
                    })
                    .on_per_page(move |size, _, cx| {
                        on_size.update(cx, |this, cx| {
                            this.per_page = size;
                            this.page = 1;
                            cx.notify();
                        })
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(quiet)
                    .child("A short list keeps both arrows off."),
            )
            .child(PageNavigator::new("receipts-pages", 1, 7))
    }
}
