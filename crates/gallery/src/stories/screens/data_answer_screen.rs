// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! A question about the numbers answered with sources, a chart and a table
//! the reader can sort, then follow-up questions and the composer.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, StyledExt as _, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    div, px,
};
use isonomic_ui_extra::chart_card::{ChartCard, ChartKind, ChartSeries};
use isonomic_ui_extra::prompt_composer::PromptComposer;
use isonomic_ui_extra::prompt_suggestions::{PromptSuggestions, Suggestion};
use isonomic_ui_extra::sortable_table::{SortableTable, TableColumn, TableRow};
use isonomic_ui_extra::sourced_answer::{AnswerSegment, AnswerSource, SourcedAnswer};

use super::window_frame;

const QUESTION: &str = "Why did repeat orders fall last quarter, and where?";

fn regions() -> Vec<TableRow> {
    vec![
        TableRow::new("coast", vec!["Coast", "1,420", "71%", "-9 pts", "3,012"]),
        TableRow::new("north", vec!["North", "980", "64%", "-14 pts", "1,874"]),
        TableRow::new("city", vec!["City", "2,310", "78%", "-1 pt", "5,407"]),
        TableRow::new("valley", vec!["Valley", "640", "69%", "-3 pts", "1,325"]),
        TableRow::new("islands", vec!["Islands", "210", "58%", "-11 pts", "366"]),
    ]
}

/// A coffee roaster asking about its subscriptions.
pub struct DataAnswerScreenStory {
    composer: Entity<PromptComposer>,
    sort: Option<(usize, bool)>,
    selected: Vec<SharedString>,
    opened: Option<SharedString>,
}

impl DataAnswerScreenStory {
    /// Create the screen.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let composer =
                cx.new(|cx| PromptComposer::new("Ask about your numbers\u{2026}", window, cx));
            composer.update(cx, |composer, cx| {
                composer.set_attach(false, cx);
                composer.set_dictation(false, cx);
            });
            Self {
                composer,
                sort: Some((3, true)),
                selected: vec!["north".into(), "islands".into()],
                opened: None,
            }
        })
    }

    fn answer(&self, cx: &mut Context<Self>) -> SourcedAnswer {
        let opened = cx.entity();
        let answer = SourcedAnswer::new("repeat-answer").pages_read(7);
        let answer = match self.opened.clone() {
            Some(id) => answer.default_open(id),
            None => answer,
        };
        answer
            .source(
                AnswerSource::new("orders", "Order export")
                    .page_title("Repeat orders by region, July to September")
                    .site("shop.example")
                    .date("Oct 2, 2026")
                    .quote("Repeat orders fell from 2,494 in the second quarter to 2,184.")
                    .mark("2,184"),
            )
            .source(
                AnswerSource::new("courier", "Courier notice")
                    .page_title("Service change for northern routes")
                    .site("parcels.example")
                    .date("Jun 24, 2026")
                    .quote(
                        "From 1 July, deliveries north of the river and to the islands move \
                         from next day to two to three working days.",
                    ),
            )
            .source(
                AnswerSource::new("survey", "Cancellation survey")
                    .page_title("Why people paused, third quarter")
                    .site("forms.example")
                    .quote("41 of 96 people who paused said the coffee arrived stale or late.")
                    .mark("arrived stale or late"),
            )
            .segment(
                AnswerSegment::new(
                    "Repeat orders fell 12% last quarter, and almost all of the drop came from \
                     the North and the Islands.",
                )
                .cite("orders"),
            )
            .segment(
                AnswerSegment::new(
                    " Both are served by the courier routes that slowed from next day to two or \
                     three days in July.",
                )
                .cite("courier"),
            )
            .segment(
                AnswerSegment::new(
                    "Slower parcels meant older coffee on arrival, the most common reason given \
                     for pausing a subscription.",
                )
                .new_paragraph()
                .cite("survey"),
            )
            .segment(AnswerSegment::new(
                " The City held steady, so this is a delivery problem rather than a taste one.",
            ))
            .on_open_source(move |id, _, cx| {
                let id = id.clone();
                opened.update(cx, |this, cx| {
                    this.opened = Some(id);
                    cx.notify();
                })
            })
    }

    fn table(&self, cx: &mut Context<Self>) -> SortableTable {
        let (sort, select) = (cx.entity(), cx.entity());
        SortableTable::new(
            "repeat-regions",
            vec![
                TableColumn::text("Region"),
                TableColumn::number("Subscribers"),
                TableColumn::number("Repeat rate"),
                TableColumn::number("Change"),
                TableColumn::number("Orders").totalled(),
            ],
            regions(),
        )
        .sort(self.sort)
        .selected(self.selected.clone())
        .on_sort(move |col, _, cx| {
            sort.update(cx, |this, cx| {
                this.sort = match this.sort {
                    Some((c, asc)) if c == col => Some((col, !asc)),
                    _ => Some((col, true)),
                };
                cx.notify();
            })
        })
        .on_select(move |ids, _, cx| {
            select.update(cx, |this, cx| {
                this.selected = ids;
                cx.notify();
            })
        })
    }
}

impl Story for DataAnswerScreenStory {
    fn title() -> &'static str {
        "Data answer screen"
    }

    fn description() -> &'static str {
        "A question about the numbers answered with sources, a trend chart and a sortable table, with follow-ups and the composer."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for DataAnswerScreenStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let answer = self.answer(cx);
        let table = self.table(cx);
        let theme = cx.theme();
        let line = theme.border;
        let quiet = theme.muted_foreground;

        let chart = ChartCard::new(
            "repeat-trend",
            "Repeat orders a month",
            ["Apr", "May", "Jun", "Jul", "Aug", "Sep"],
        )
        .subtitle("People on their second box or later")
        .kind(ChartKind::Line)
        .series(ChartSeries::new(
            "City",
            vec![440., 452., 447., 451., 438., 446.],
        ))
        .series(ChartSeries::new(
            "Coast",
            vec![210., 215., 212., 196., 184., 179.],
        ))
        .series(ChartSeries::new(
            "North",
            vec![128., 131., 126., 104., 92., 84.],
        ))
        .series(ChartSeries::new(
            "Islands",
            vec![61., 63., 60., 42., 37., 33.],
        ))
        .height(px(140.))
        .format(|orders| format!("{orders:.0}"));

        let header = v_flex()
            .flex_none()
            .px_5()
            .py_3()
            .gap_0p5()
            .border_b_1()
            .border_color(line)
            .child(div().font_medium().child(QUESTION))
            .child(
                div()
                    .text_xs()
                    .text_color(quiet)
                    .child("Roastery insights \u{b7} Subscriptions, third quarter"),
            );

        let body = v_flex()
            .gap_4()
            .px_5()
            .py_4()
            .child(
                h_flex()
                    .w_full()
                    .items_start()
                    .gap_5()
                    .child(div().flex_1().min_w_0().child(answer))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w(px(300.))
                            .gap_4()
                            .child(chart)
                            .child(
                                PromptSuggestions::new(
                                    "repeat-follow-ups",
                                    vec![
                                        Suggestion::new(
                                            "cost",
                                            "What would next-day courier cost us?",
                                        ),
                                        Suggestion::new("email", "Draft a note to North"),
                                        Suggestion::new("trend", "Show twelve months"),
                                    ],
                                )
                                .chips()
                                .heading("Follow up"),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_medium()
                            .child("Repeat rate by region, July to September"),
                    )
                    .child(table),
            );

        window_frame(cx)
            .child(header)
            .child(
                div()
                    .id("data-answer-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(body),
            )
            .child(
                div()
                    .flex_none()
                    .px_5()
                    .pb_3()
                    .pt_2()
                    .border_t_1()
                    .border_color(line)
                    .child(self.composer.clone()),
            )
    }
}
