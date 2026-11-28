// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Sortable table: rows under headers that sort when clicked, with row selection, a totals row and a quiet empty state.
//!
//! Clicking a header sorts by it; clicking again flips the order, and the
//! header says which way it runs. Numbers sort as numbers and line up on
//! the right. A box at the start of each row selects it, and the header's
//! box selects all. Numeric columns can be totalled in a footer row, over
//! the selection when there is one.
//!
//! Opt-in data table parts: a filter field over the rows that matches any
//! cell, pages of a set size with a pager underneath, and Copy as CSV,
//! which copies the selected rows (or every row the filter leaves) with
//! their headings. The header's box then selects what the filter leaves.

use std::{cmp::Ordering, rc::Rc};

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    checkbox::Checkbox,
    clipboard::Clipboard,
    h_flex,
    input::{Input, InputState},
    look,
    pagination::Pagination,
    v_flex,
};
use isonomic_ui::{
    App, AppContext as _, ElementId, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, RenderOnce, Role, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, TestSupportExt as _, Window, div, prelude::FluentBuilder as _, px,
};

use crate::common::resting_shadow;

/// A column.
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumn {
    /// Its heading.
    pub title: SharedString,
    /// Whether its cells are numbers.
    pub numeric: bool,
    /// Whether the footer totals it.
    pub total: bool,
    /// Its width in pixels; `None` shares what's left.
    pub width: Option<f32>,
}

impl TableColumn {
    /// A text column that shares the space.
    pub fn text(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            numeric: false,
            total: false,
            width: None,
        }
    }

    /// A number column, 110 pixels wide.
    pub fn number(title: impl Into<SharedString>) -> Self {
        Self {
            numeric: true,
            width: Some(110.),
            ..Self::text(title)
        }
    }

    /// Total it in the footer.
    pub fn totalled(mut self) -> Self {
        self.total = true;
        self
    }

    /// Set its width.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
}

/// A row: its id, the text shown in each cell, and each cell's number for sorting and totals.
#[derive(Debug, Clone, PartialEq)]
pub struct TableRow {
    /// Identifies it.
    pub id: SharedString,
    /// Text per cell.
    pub cells: Vec<SharedString>,
    /// Number per cell, where the cell is a number.
    pub values: Vec<Option<f64>>,
}

impl TableRow {
    /// Create a row from its cells; cells that read as numbers (with $ , and %) get values.
    pub fn new(id: impl Into<SharedString>, cells: Vec<&str>) -> Self {
        let values = cells.iter().map(|c| read_number(c)).collect();
        Self {
            id: id.into(),
            cells: cells
                .into_iter()
                .map(|c| SharedString::from(c.to_string()))
                .collect(),
            values,
        }
    }
}

/// Read a number from text such as "$1,240.50", "-12%" or "(80)".
pub fn read_number(text: &str) -> Option<f64> {
    let negative = text.trim().starts_with('(') && text.trim().ends_with(')');
    let cleaned: String = text
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    let value: f64 = cleaned.parse().ok()?;
    Some(if negative { -value } else { value })
}

/// The rows' order for a sort by column `col`, ascending or not.
pub fn sorted_order(rows: &[TableRow], col: usize, ascending: bool) -> Vec<usize> {
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by(|a, b| {
        let (ra, rb) = (&rows[*a], &rows[*b]);
        let by = match (
            ra.values.get(col).copied().flatten(),
            rb.values.get(col).copied().flatten(),
        ) {
            (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
            _ => ra
                .cells
                .get(col)
                .map(|c| c.to_lowercase())
                .cmp(&rb.cells.get(col).map(|c| c.to_lowercase())),
        };
        if ascending { by } else { by.reverse() }
    });
    order
}

/// Whether a row has a cell containing every word of the filter, ignoring
/// case.
pub fn row_matches(row: &TableRow, query: &str) -> bool {
    let query = query.to_lowercase();
    query.split_whitespace().all(|word| {
        row.cells
            .iter()
            .any(|cell| cell.to_lowercase().contains(word))
    })
}

/// The rows on a page: the start and end of the slice, and how many pages
/// there are. `page` counts from 0 and is kept within the pages.
pub fn page_slice(rows: usize, size: usize, page: usize) -> (usize, usize, usize) {
    let size = size.max(1);
    let pages = rows.div_ceil(size).max(1);
    let page = page.min(pages - 1);
    let start = page * size;
    (start, (start + size).min(rows), pages)
}

/// Write rows as comma-separated values under their headings. A cell with a
/// comma, quote or line break is quoted, with quotes doubled.
pub fn to_csv(columns: &[TableColumn], rows: &[&TableRow]) -> String {
    fn field(text: &str) -> String {
        if text.contains([',', '"', '\n', '\r']) {
            format!("\"{}\"", text.replace('"', "\"\""))
        } else {
            text.to_string()
        }
    }
    let mut out = columns
        .iter()
        .map(|c| field(&c.title))
        .collect::<Vec<_>>()
        .join(",");
    for row in rows {
        out.push('\n');
        out.push_str(
            &row.cells
                .iter()
                .map(|c| field(c))
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    out
}

/// The words under the table: which rows show of how many.
pub fn range_words(start: usize, end: usize, shown: usize, total: usize) -> String {
    let of = if shown == total {
        format!("{total}")
    } else {
        format!("{shown} matching ({total} in all)")
    };
    if shown == 0 {
        format!("No rows of {total}")
    } else {
        format!("Rows {}\u{2013}{end} of {of}", start + 1)
    }
}

type CopyHandler = Rc<dyn Fn(&str, &mut Window, &mut App)>;
type SortHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type SelectHandler = Rc<dyn Fn(Vec<SharedString>, &mut Window, &mut App)>;

/// A table that sorts, selects and totals.
#[derive(IntoElement)]
pub struct SortableTable {
    id: ElementId,
    style: StyleRefinement,
    columns: Vec<TableColumn>,
    rows: Vec<TableRow>,
    sort: Option<(usize, bool)>,
    selected: Vec<SharedString>,
    empty: SharedString,
    on_sort: Option<SortHandler>,
    on_select: Option<SelectHandler>,
    filter: Option<SharedString>,
    page_size: Option<usize>,
    copy: bool,
    on_copy: Option<CopyHandler>,
}

#[derive(Default)]
struct TableState {
    page: usize,
    query: String,
}

impl SortableTable {
    /// Create a table.
    pub fn new(id: impl Into<ElementId>, columns: Vec<TableColumn>, rows: Vec<TableRow>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            columns,
            rows,
            sort: None,
            selected: Vec::new(),
            empty: "Nothing to show.".into(),
            on_sort: None,
            on_select: None,
            filter: None,
            page_size: None,
            copy: false,
            on_copy: None,
        }
    }

    /// Show a filter field over the rows, with this placeholder.
    pub fn filterable(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.filter = Some(placeholder.into());
        self
    }

    /// Show this many rows a page, with a pager underneath.
    pub fn page_size(mut self, rows: usize) -> Self {
        self.page_size = Some(rows.max(1));
        self
    }

    /// Offer Copy as CSV: the selected rows, or every row the filter leaves.
    pub fn copyable(mut self) -> Self {
        self.copy = true;
        self
    }

    /// Run with the text copied as CSV.
    pub fn on_copy(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.copy = true;
        self.on_copy = Some(Rc::new(handler));
        self
    }

    /// Sort by a column, ascending or not.
    pub fn sort(mut self, sort: Option<(usize, bool)>) -> Self {
        self.sort = sort;
        self
    }

    /// The selected rows.
    pub fn selected(mut self, ids: Vec<SharedString>) -> Self {
        self.selected = ids;
        self
    }

    /// Words for an empty table.
    pub fn empty(mut self, text: impl Into<SharedString>) -> Self {
        self.empty = text.into();
        self
    }

    /// Run with the column whose header was clicked.
    pub fn on_sort(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_sort = Some(Rc::new(handler));
        self
    }

    /// Run with the new selection.
    pub fn on_select(
        mut self,
        handler: impl Fn(Vec<SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Styled for SortableTable {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

fn money(value: f64) -> String {
    let cents = (value * 100.).round() as i64;
    let (sign, cents) = if cents < 0 {
        ("-", -cents)
    } else {
        ("", cents)
    };
    let whole = (cents / 100).to_string();
    let mut grouped = String::new();
    for (ix, c) in whole.chars().enumerate() {
        if ix > 0 && (whole.len() - ix) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    format!("{sign}{grouped}.{:02}", cents % 100)
}

impl RenderOnce for SortableTable {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let radius_lg = theme.radius_lg;
        let part = |name: &str| ElementId::NamedChild(self.id.clone().into(), name.into());
        let state = window.use_keyed_state(part("table-state"), cx, |_, _| TableState::default());
        let search: Option<Entity<InputState>> = self.filter.clone().map(|placeholder| {
            window
                .use_keyed_state(part("filter"), cx, move |window, cx| {
                    cx.new(|cx| InputState::new(window, cx).placeholder(placeholder.clone()))
                })
                .read(cx)
                .clone()
        });
        let query = search
            .as_ref()
            .map(|s| s.read(cx).value().trim().to_string())
            .unwrap_or_default();
        if state.read(cx).query != query {
            // A new filter starts from the first page.
            state.update(cx, |state, _| {
                state.query = query.clone();
                state.page = 0;
            });
        }
        let order: Vec<usize> = match self.sort {
            Some((col, asc)) => sorted_order(&self.rows, col, asc),
            None => (0..self.rows.len()).collect(),
        }
        .into_iter()
        .filter(|ix| row_matches(&self.rows[*ix], &query))
        .collect();
        let shown = order.len();
        let (start, end, pages) = match self.page_size {
            Some(size) => page_slice(shown, size, state.read(cx).page),
            None => (0, shown, 1),
        };
        let visible_ids: Vec<SharedString> =
            order.iter().map(|ix| self.rows[*ix].id.clone()).collect();
        let all = !visible_ids.is_empty() && visible_ids.iter().all(|r| self.selected.contains(r));
        let over_selection = !self.selected.is_empty();
        let columns = self.columns.clone();
        let cell = move |col: usize, text: SharedString, bold: bool| {
            let c = &columns[col];
            div()
                .px_3()
                .truncate()
                .when_some(c.width, |this, w| this.w(px(w)).flex_none())
                .when(c.width.is_none(), |this| this.flex_1().min_w(px(80.)))
                .when(c.numeric, |this| this.text_right())
                .when(bold, |this| this.font_medium())
                .child(text)
        };

        let header = {
            let select_all = self.on_select.clone();
            let ids = visible_ids;
            h_flex()
                .h(px(34.))
                .border_b_1()
                .border_color(line)
                .bg(chip.opacity(0.4))
                .text_label_small()
                .text_color(quiet)
                .child(
                    div().w(px(36.)).flex_none().pl_3().child(
                        Checkbox::new(ElementId::NamedChild(self.id.clone().into(), "all".into()))
                            .checked(all)
                            .accessibility_label("Select every row")
                            .on_click(move |checked, window, cx| {
                                if let Some(select) = &select_all {
                                    select(
                                        if *checked { ids.clone() } else { Vec::new() },
                                        window,
                                        cx,
                                    );
                                }
                            }),
                    ),
                )
                .children(self.columns.iter().enumerate().map(|(col, c)| {
                    let sort = self.on_sort.clone();
                    let here = self.sort.filter(|(s, _)| *s == col).map(|(_, asc)| asc);
                    let focus = window
                        .use_keyed_state(
                            ElementId::NamedChild(
                                self.id.clone().into(),
                                format!("head-focus-{col}").into(),
                            ),
                            cx,
                            |_, cx| cx.focus_handle().tab_stop(true),
                        )
                        .read(cx)
                        .clone();
                    h_flex()
                        .id(ElementId::NamedChild(
                            self.id.clone().into(),
                            format!("head-{col}").into(),
                        ))
                        .test_support()
                        .gap_1()
                        .px_3()
                        .h_full()
                        .when_some(c.width, |this, w| this.w(px(w)).flex_none())
                        .when(c.width.is_none(), |this| this.flex_1().min_w(px(80.)))
                        .when(c.numeric, |this| this.justify_end())
                        .cursor_pointer()
                        .map(|this| {
                            let hovered = window.use_keyed_state(
                                ElementId::NamedChild(
                                    self.id.clone().into(),
                                    format!("head-hover-{col}").into(),
                                ),
                                cx,
                                |_, _| false,
                            );
                            let fade = look::hover_fade(
                                ElementId::NamedChild(
                                    self.id.clone().into(),
                                    format!("head-fade-{col}").into(),
                                ),
                                *hovered.read(cx),
                                window,
                                cx,
                            );
                            this.text_color(if here.is_some() {
                                ink
                            } else {
                                look::mix(quiet, ink, fade)
                            })
                            .on_hover(move |over, _, cx| {
                                hovered.update(cx, |state, cx| {
                                    *state = *over;
                                    cx.notify();
                                })
                            })
                        })
                        // A Tab stop: Enter or Space on the focused header is a click, so it sorts.
                        .when(self.on_sort.is_some(), |this| {
                            this.track_focus(&focus)
                                .focus_visible(|s| s.text_color(ink).bg(chip))
                        })
                        .role(Role::ColumnHeader)
                        .aria_label(format!(
                            "{}, {}",
                            c.title,
                            match here {
                                Some(true) => "sorted up",
                                Some(false) => "sorted down",
                                None => "not sorted",
                            }
                        ))
                        .on_click(move |_, window, cx| {
                            if let Some(sort) = &sort {
                                sort(col, window, cx);
                            }
                        })
                        .child(c.title.clone())
                        .children(here.map(|asc| {
                            Icon::new(if asc {
                                IconName::ArrowUp
                            } else {
                                IconName::ArrowDown
                            })
                            .xsmall()
                        }))
                }))
        };

        let rows: Vec<_> = order[start..end]
            .iter()
            .map(|ix| {
                let row = &self.rows[*ix];
                let picked = self.selected.contains(&row.id);
                let hovered = window.use_keyed_state(
                    ElementId::NamedChild(
                        self.id.clone().into(),
                        format!("row-hover-{}", row.id).into(),
                    ),
                    cx,
                    |_, _| false,
                );
                let fade = look::hover_fade(
                    ElementId::NamedChild(
                        self.id.clone().into(),
                        format!("row-fade-{}", row.id).into(),
                    ),
                    *hovered.read(cx),
                    window,
                    cx,
                );
                let select = self.on_select.clone();
                let current = self.selected.clone();
                let id = row.id.clone();
                h_flex()
                    .id(ElementId::NamedChild(
                        self.id.clone().into(),
                        format!("row-{}", row.id).into(),
                    ))
                    .h(px(34.))
                    .border_b_1()
                    .border_color(line.opacity(0.6))
                    .bg(if picked {
                        chip.opacity(0.6)
                    } else {
                        look::mix(chip.opacity(0.), chip.opacity(0.4), fade)
                    })
                    .on_hover(move |over, _, cx| {
                        hovered.update(cx, |state, cx| {
                            *state = *over;
                            cx.notify();
                        })
                    })
                    .aria_selected(picked)
                    .child(
                        div().w(px(36.)).flex_none().pl_3().child(
                            Checkbox::new(ElementId::NamedChild(
                                self.id.clone().into(),
                                format!("pick-{}", row.id).into(),
                            ))
                            .checked(picked)
                            .accessibility_label(format!(
                                "Select {}",
                                row.cells.first().cloned().unwrap_or_default()
                            ))
                            .on_click(move |checked, window, cx| {
                                let mut next = current.clone();
                                next.retain(|r| r != &id);
                                if *checked {
                                    next.push(id.clone());
                                }
                                if let Some(select) = &select {
                                    select(next, window, cx);
                                }
                            }),
                        ),
                    )
                    .children(
                        row.cells
                            .iter()
                            .enumerate()
                            .map(|(col, text)| cell(col, text.clone(), false)),
                    )
            })
            .collect();

        let totals = self.columns.iter().any(|c| c.total).then(|| {
            let counted: Vec<&TableRow> = self
                .rows
                .iter()
                .filter(|r| !over_selection || self.selected.contains(&r.id))
                .collect();
            h_flex()
                .h(px(34.))
                .bg(chip.opacity(0.4))
                .child(div().w(px(36.)).flex_none())
                .children(self.columns.iter().enumerate().map(|(col, c)| {
                    let text: SharedString = if c.total {
                        money(
                            counted
                                .iter()
                                .filter_map(|r| r.values.get(col).copied().flatten())
                                .sum(),
                        )
                        .into()
                    } else if col == 0 {
                        (if over_selection {
                            format!("Total of {} selected", counted.len())
                        } else {
                            "Total".to_string()
                        })
                        .into()
                    } else {
                        SharedString::default()
                    };
                    cell(col, text, true)
                }))
        });

        let csv_rows: Vec<&TableRow> = if over_selection {
            order
                .iter()
                .map(|ix| &self.rows[*ix])
                .filter(|r| self.selected.contains(&r.id))
                .collect()
        } else {
            order.iter().map(|ix| &self.rows[*ix]).collect()
        };
        let csv_count = csv_rows.len();
        let csv: SharedString = to_csv(&self.columns, &csv_rows).into();
        let toolbar = (search.is_some() || self.copy).then(|| {
            let on_copy = self.on_copy.clone();
            h_flex()
                .gap_2()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(line)
                .children(search.as_ref().map(|search| {
                    div().flex_1().max_w(px(280.)).child(
                        Input::new(search)
                            .small()
                            .cleanable(true)
                            .prefix(Icon::new(IconName::Search).xsmall().text_color(quiet)),
                    )
                }))
                .child(div().flex_1())
                .when(over_selection, |this| {
                    this.child(
                        div()
                            .text_caption()
                            .text_color(quiet)
                            .child(format!("{} selected", self.selected.len())),
                    )
                })
                .when(self.copy, |this| {
                    this.child(
                        Clipboard::new(part("copy-csv"))
                            .label(if over_selection {
                                "Copy selected as CSV"
                            } else {
                                "Copy as CSV"
                            })
                            .accessibility_label(format!("Copy {csv_count} rows as CSV"))
                            .value(csv.clone())
                            .on_copied(move |text, window, cx| {
                                if let Some(on_copy) = &on_copy {
                                    on_copy(&text, window, cx);
                                }
                            }),
                    )
                })
        });
        let pager = self.page_size.map(|_| {
            let page_state = state.clone();
            h_flex()
                .gap_2()
                .px_3()
                .py_1p5()
                .border_t_1()
                .border_color(line)
                .child(
                    div()
                        .id(part("range"))
                        .flex_1()
                        .text_caption()
                        .text_color(quiet)
                        .role(Role::Status)
                        .aria_label(range_words(start, end, shown, self.rows.len()))
                        .child(range_words(start, end, shown, self.rows.len())),
                )
                .child(
                    Pagination::new(part("pager"))
                        .small()
                        .current_page(state.read(cx).page.min(pages - 1) + 1)
                        .total_pages(pages)
                        .on_click(move |page, _, cx| {
                            page_state.update(cx, |state, cx| {
                                state.page = page.saturating_sub(1);
                                cx.notify();
                            })
                        }),
                )
        });
        let filtered_out = self.rows.len() > 0 && shown == 0;

        v_flex()
            .id(self.id.clone())
            .w_full()
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .overflow_hidden()
            .shadow(resting_shadow(cx))
            .children(toolbar)
            .child(header)
            .children(rows)
            .when(filtered_out, |this| {
                this.child(
                    div()
                        .px_4()
                        .py_6()
                        .text_center()
                        .text_color(quiet)
                        .child(format!("No rows match \u{201c}{query}\u{201d}.")),
                )
            })
            .when(self.rows.is_empty(), |this| {
                this.child(
                    div()
                        .px_4()
                        .py_6()
                        .text_center()
                        .text_color(quiet)
                        .child(self.empty.clone()),
                )
            })
            .children(totals)
            .children(pager)
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_read_and_sort_as_numbers() {
        assert_eq!(read_number("$1,240.50"), Some(1240.5));
        assert_eq!(read_number("(80)"), Some(-80.));
        assert_eq!(read_number("Harbour"), None);
        let rows = vec![
            TableRow::new("a", vec!["Kestrel", "$900.00"]),
            TableRow::new("b", vec!["harbour", "$1,240.50"]),
            TableRow::new("c", vec!["Bayview", "$80.00"]),
        ];
        assert_eq!(sorted_order(&rows, 1, true), [2, 0, 1]);
        assert_eq!(sorted_order(&rows, 0, true), [2, 1, 0]);
        assert_eq!(sorted_order(&rows, 0, false), [0, 1, 2]);
        assert_eq!(money(14822.1), "14,822.10");
    }

    #[test]
    fn the_filter_matches_every_word_in_any_cell() {
        let row = TableRow::new("a", vec!["Harbour Books", "Paid", "$90.00"]);
        assert!(row_matches(&row, ""));
        assert!(row_matches(&row, "harbour paid"));
        assert!(row_matches(&row, "90"));
        assert!(!row_matches(&row, "harbour overdue"));
    }

    #[test]
    fn pages_slice_the_rows() {
        assert_eq!(page_slice(23, 10, 0), (0, 10, 3));
        assert_eq!(page_slice(23, 10, 2), (20, 23, 3));
        assert_eq!(
            page_slice(23, 10, 9),
            (20, 23, 3),
            "a page past the end is the last"
        );
        assert_eq!(page_slice(0, 10, 0), (0, 0, 1));
        assert_eq!(range_words(20, 23, 23, 23), "Rows 21\u{2013}23 of 23");
        assert_eq!(
            range_words(0, 4, 4, 23),
            "Rows 1\u{2013}4 of 4 matching (23 in all)"
        );
        assert_eq!(range_words(0, 0, 0, 23), "No rows of 23");
    }

    #[test]
    fn csv_quotes_what_needs_it() {
        let columns = vec![TableColumn::text("Client"), TableColumn::number("Amount")];
        let rows = [
            TableRow::new("a", vec!["Kestrel, Ltd", "$1,240.50"]),
            TableRow::new("b", vec!["The \"Bay\"", "80"]),
        ];
        let refs: Vec<&TableRow> = rows.iter().collect();
        assert_eq!(
            to_csv(&columns, &refs),
            "Client,Amount\n\"Kestrel, Ltd\",\"$1,240.50\"\n\"The \"\"Bay\"\"\",80"
        );
    }
}
