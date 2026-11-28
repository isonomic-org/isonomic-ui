// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the result carousel.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::result_carousel::{ResultCarousel, ResultItem};

/// Places to stay near a made-up harbour town.
pub struct ResultCarouselStory {
    last: SharedString,
}

impl ResultCarouselStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            last: "Nothing picked yet".into(),
        })
    }
}

impl Story for ResultCarouselStory {
    fn title() -> &'static str {
        "Result carousel"
    }

    fn description() -> &'static str {
        "Results as cards in a row that scrolls sideways, one card per arrow press."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ResultCarouselStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let stays = vec![
            ResultItem::new("loft", "Harbour loft")
                .meta("Sleeps 2 · 300 m to the ferry")
                .rating(4.8)
                .price("€140 a night")
                .tint(theme.chart_1)
                .badge("Best match")
                .action("Hold dates"),
            ResultItem::new("garden", "Garden room")
                .meta("Sleeps 2 · quiet street")
                .rating(4.6)
                .price("€95 a night")
                .tint(theme.chart_2),
            ResultItem::new("boat", "Moored sailboat")
                .meta("Sleeps 3 · in the marina")
                .rating(4.4)
                .price("€120 a night")
                .tint(theme.chart_3)
                .badge("Unusual"),
            ResultItem::new("mill", "Old mill house")
                .meta("Sleeps 6 · 4 km inland")
                .rating(4.9)
                .price("€210 a night")
                .tint(theme.chart_4),
            ResultItem::new("attic", "Bakery attic")
                .meta("Sleeps 1 · over the bakery")
                .rating(4.2)
                .price("€60 a night")
                .tint(theme.chart_5),
        ];
        let quiet = theme.muted_foreground;
        let on_action = cx.entity();
        let on_open = cx.entity();
        v_flex()
            .gap_6()
            .child(
                ResultCarousel::new("stays", "Stays for 12 to 15 June", stays.clone())
                    .on_action(move |id, _, cx| {
                        on_action.update(cx, |this, cx| {
                            this.last = format!("Picked {id}").into();
                            cx.notify();
                        })
                    })
                    .on_open(move |id, _, cx| {
                        on_open.update(cx, |this, cx| {
                            this.last = format!("Opened {id}").into();
                            cx.notify();
                        })
                    }),
            )
            .child(
                ResultCarousel::new("stays-compact", "Cheaper nearby", stays[1..4].to_vec())
                    .subtitle("Within 5 km")
                    .compact(),
            )
            .child(div().text_sm().text_color(quiet).child(self.last.clone()))
    }
}
