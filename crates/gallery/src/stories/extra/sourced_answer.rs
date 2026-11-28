// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the sourced answer.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::sourced_answer::{AnswerSegment, AnswerSource, SourcedAnswer};

/// Shows an answer with three sources, one of them open at the start, and the
/// same answer with hover previews and the panel of all sources open.
pub struct SourcedAnswerStory {
    last: SharedString,
}

impl SourcedAnswerStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            last: "Nothing pressed yet".into(),
        })
    }
}

impl Story for SourcedAnswerStory {
    fn title() -> &'static str {
        "Sourced answer"
    }

    fn description() -> &'static str {
        "A finished answer wired to the sources behind it."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

/// The answer both examples show, without its handlers.
fn margin_answer(id: &'static str) -> SourcedAnswer {
    SourcedAnswer::new(id)
        .pages_read(9)
        .source(
            AnswerSource::new("ledger", "March ledger")
                .page_title("Cost of goods sold, March")
                .site("books.example")
                .date("Apr 2, 2026")
                .quote(
                    "Cost of goods sold came to 61 percent of revenue in March, \
                     up from 55 percent in February.",
                ),
        )
        .source(
            AnswerSource::new("carrier", "Carrier contract")
                .page_title("Rate schedule, amendment 3")
                .site("contracts.example")
                .date("Feb 20, 2026")
                .url("https://contracts.example/carrier/amendment-3")
                .quote(
                    "From 1 March the rate per pallet rises by 12 percent on all \
                     northbound routes.",
                ),
        )
        .source(
            AnswerSource::new("orders", "Order export")
                .page_title("Orders by region, first quarter")
                .site("shop.example")
                .quote("Northbound orders made up 48 percent of volume in March.")
                .mark("48 percent of volume"),
        )
        .segment(
            AnswerSegment::new(
                "Margin fell in March because goods cost more to deliver, not \
                 because they sold for less.",
            )
            .cite("ledger"),
        )
        .segment(
            AnswerSegment::new(
                " The carrier put its rate per pallet up by 12 percent on \
                 northbound routes from the first of the month.",
            )
            .cite("carrier"),
        )
        .segment(
            AnswerSegment::new(
                "Those routes carried almost half of the month's volume, so the \
                 rise landed on a large share of orders.",
            )
            .new_paragraph()
            .cite("orders")
            .cite("carrier"),
        )
        .segment(AnswerSegment::new(
            " Prices stayed where they were, so the difference came out of margin.",
        ))
}

impl Render for SourcedAnswerStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let opened = cx.entity();
        let followed = opened.clone();
        let peeked = opened.clone();

        v_flex()
            .gap_3()
            .child(
                margin_answer("margin")
                    .default_open("carrier")
                    .on_open_source(move |id, _, cx| {
                        let line = format!("Open source: {id}");
                        opened.update(cx, |this, cx| {
                            this.last = line.into();
                            cx.notify();
                        })
                    })
                    .action("Follow up", move |_, cx| {
                        followed.update(cx, |this, cx| {
                            this.last = "Follow up pressed".into();
                            cx.notify();
                        })
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.last.clone()),
            )
            .child(
                margin_answer("margin-panel")
                    .hover_previews(true)
                    .sources_panel(true)
                    .default_panel_open(true)
                    .on_preview_change(move |id, _, cx| {
                        let line = match id {
                            Some(id) => format!("Passage open: {id}"),
                            None => "Passage closed".to_string(),
                        };
                        peeked.update(cx, |this, cx| {
                            this.last = line.into();
                            cx.notify();
                        })
                    }),
            )
    }
}
