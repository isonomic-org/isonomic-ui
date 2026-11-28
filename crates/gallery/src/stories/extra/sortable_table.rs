// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the sortable table.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div, px,
};
use isonomic_ui_extra::sortable_table::{SortableTable, TableColumn, TableRow};

/// Unpaid bills, sortable and selectable, totalled over the selection.
/// Under it, a year of payments as a data table: a filter, eight rows a
/// page and Copy as CSV, which copies the selection or what the filter
/// leaves.
pub struct SortableTableStory {
    sort: Option<(usize, bool)>,
    selected: Vec<SharedString>,
    ledger_sort: Option<(usize, bool)>,
    ledger_selected: Vec<SharedString>,
    copied: SharedString,
}

/// Payments received: client, method, month and amount, made up.
fn ledger() -> Vec<TableRow> {
    const CLIENTS: [&str; 6] = [
        "Harbour Supply",
        "Northline Fuel",
        "Kestrel Print",
        "Bayview Rentals",
        "Oak & Iron",
        "Juniper Cafe",
    ];
    const METHODS: [&str; 3] = ["Bank transfer", "Card", "Cheque"];
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    (0..23)
        .map(|n: usize| {
            let amount = format!("${}.{:02}", 120 + (n * 397) % 3_900, (n * 37) % 100);
            TableRow::new(
                format!("p{n}"),
                vec![
                    CLIENTS[(n * 5) % CLIENTS.len()],
                    METHODS[(n * 7) % METHODS.len()],
                    MONTHS[n % MONTHS.len()],
                    &amount,
                ],
            )
        })
        .collect()
}

fn rows() -> Vec<TableRow> {
    vec![
        TableRow::new(
            "b1",
            vec!["Harbour Supply", "Invoice 2291", "Mar 2", "$1,240.50"],
        ),
        TableRow::new("b2", vec!["Northline Fuel", "March", "Mar 31", "$612.08"]),
        TableRow::new(
            "b3",
            vec!["Kestrel Print", "400 flyers", "Apr 4", "$289.00"],
        ),
        TableRow::new(
            "b4",
            vec!["Bayview Rentals", "April rent", "Apr 1", "$4,200.00"],
        ),
        TableRow::new(
            "b5",
            vec!["Oak & Iron", "Shelving, disputed", "Mar 18", "$980.00"],
        ),
        TableRow::new(
            "b6",
            vec!["Juniper Cafe", "Team lunch", "Mar 28", "$146.40"],
        ),
    ]
}

impl SortableTableStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            sort: Some((3, false)),
            selected: vec!["b1".into(), "b4".into()],
            ledger_sort: None,
            ledger_selected: Vec::new(),
            copied: "Nothing copied yet".into(),
        })
    }
}

impl Story for SortableTableStory {
    fn title() -> &'static str {
        "Sortable table"
    }

    fn description() -> &'static str {
        "Rows under headers that sort when clicked, with row selection, a totals row and a quiet empty state."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for SortableTableStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (sort, select) = (cx.entity(), cx.entity());
        let (ledger_sort, ledger_select, copied) = (cx.entity(), cx.entity(), cx.entity());
        v_flex()
            .gap_3()
            .child(
                SortableTable::new(
                    "bills",
                    vec![
                        TableColumn::text("Supplier"),
                        TableColumn::text("For"),
                        TableColumn::text("Due").width(90.),
                        TableColumn::number("Amount").totalled(),
                    ],
                    rows(),
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
                }),
            )
            .child(div().h(px(12.)))
            .child(
                SortableTable::new(
                    "payments",
                    vec![
                        TableColumn::text("Client"),
                        TableColumn::text("Method"),
                        TableColumn::text("Month").width(90.),
                        TableColumn::number("Amount").totalled(),
                    ],
                    ledger(),
                )
                .filterable("Filter payments")
                .page_size(8)
                .sort(self.ledger_sort)
                .selected(self.ledger_selected.clone())
                .on_sort(move |col, _, cx| {
                    ledger_sort.update(cx, |this, cx| {
                        this.ledger_sort = match this.ledger_sort {
                            Some((c, asc)) if c == col => Some((col, !asc)),
                            _ => Some((col, true)),
                        };
                        cx.notify();
                    })
                })
                .on_select(move |ids, _, cx| {
                    ledger_select.update(cx, |this, cx| {
                        this.ledger_selected = ids;
                        cx.notify();
                    })
                })
                .on_copy(move |text, _, cx| {
                    let lines = text.lines().count().saturating_sub(1);
                    copied.update(cx, |this, cx| {
                        this.copied = format!("Copied {lines} rows as CSV").into();
                        cx.notify();
                    })
                }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.copied.clone()),
            )
    }
}
