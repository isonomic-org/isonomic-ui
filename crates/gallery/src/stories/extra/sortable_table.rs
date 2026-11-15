//! Gallery story for the sortable table.

use crate::Story;
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, Render, SharedString, Window,
};
use isonomic_ui_extra::sortable_table::{SortableTable, TableColumn, TableRow};

/// Unpaid bills, sortable and selectable, totalled over the selection.
pub struct SortableTableStory {
    sort: Option<(usize, bool)>,
    selected: Vec<SharedString>,
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
        })
    }
}
