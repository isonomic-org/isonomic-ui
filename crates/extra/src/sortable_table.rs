//! Sortable table: rows under headers that sort when clicked, with row selection, a totals row and a quiet empty state.
//!
//! Clicking a header sorts by it; clicking again flips the order, and the
//! header says which way it runs. Numbers sort as numbers and line up on
//! the right. A box at the start of each row selects it, and the header's
//! box selects all. Numeric columns can be totalled in a footer row, over
//! the selection when there is one.

use std::{cmp::Ordering, rc::Rc};

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, checkbox::Checkbox, h_flex,
    v_flex,
};
use isonomic_ui::{
    Role,
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, TestSupportExt as _, Window, div,
    prelude::FluentBuilder as _, px,
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
            cells: cells.into_iter().map(|c| SharedString::from(c.to_string())).collect(),
            values,
        }
    }
}

/// Read a number from text such as "$1,240.50", "-12%" or "(80)".
pub fn read_number(text: &str) -> Option<f64> {
    let negative = text.trim().starts_with('(') && text.trim().ends_with(')');
    let cleaned: String = text.chars().filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
    let value: f64 = cleaned.parse().ok()?;
    Some(if negative { -value } else { value })
}

/// The rows' order for a sort by column `col`, ascending or not.
pub fn sorted_order(rows: &[TableRow], col: usize, ascending: bool) -> Vec<usize> {
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by(|a, b| {
        let (ra, rb) = (&rows[*a], &rows[*b]);
        let by = match (ra.values.get(col).copied().flatten(), rb.values.get(col).copied().flatten()) {
            (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
            _ => ra.cells.get(col).map(|c| c.to_lowercase()).cmp(&rb.cells.get(col).map(|c| c.to_lowercase())),
        };
        if ascending { by } else { by.reverse() }
    });
    order
}

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
        }
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
    pub fn on_select(mut self, handler: impl Fn(Vec<SharedString>, &mut Window, &mut App) + 'static) -> Self {
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
    let (sign, cents) = if cents < 0 { ("-", -cents) } else { ("", cents) };
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
        let order = match self.sort {
            Some((col, asc)) => sorted_order(&self.rows, col, asc),
            None => (0..self.rows.len()).collect(),
        };
        let all = !self.rows.is_empty() && self.rows.iter().all(|r| self.selected.contains(&r.id));
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
                .when(bold, |this| this.font_semibold())
                .child(text)
        };

        let header = {
            let select_all = self.on_select.clone();
            let ids: Vec<SharedString> = self.rows.iter().map(|r| r.id.clone()).collect();
            h_flex()
                .h(px(34.))
                .border_b_1()
                .border_color(line)
                .bg(chip.opacity(0.4))
                .text_xs()
                .font_medium()
                .text_color(quiet)
                .child(
                    div().w(px(36.)).flex_none().pl_3().child(
                        Checkbox::new(ElementId::NamedChild(self.id.clone().into(), "all".into()))
                            .checked(all)
                            .accessibility_label("Select every row")
                            .on_click(move |checked, window, cx| {
                                if let Some(select) = &select_all {
                                    select(if *checked { ids.clone() } else { Vec::new() }, window, cx);
                                }
                            }),
                    ),
                )
                .children(self.columns.iter().enumerate().map(|(col, c)| {
                    let sort = self.on_sort.clone();
                    let here = self.sort.filter(|(s, _)| *s == col).map(|(_, asc)| asc);
                    let focus = window
                        .use_keyed_state(
                            ElementId::NamedChild(self.id.clone().into(), format!("head-focus-{col}").into()),
                            cx,
                            |_, cx| cx.focus_handle().tab_stop(true),
                        )
                        .read(cx)
                        .clone();
                    h_flex()
                        .id(ElementId::NamedChild(self.id.clone().into(), format!("head-{col}").into()))
                        .test_support()
                        .gap_1()
                        .px_3()
                        .h_full()
                        .when_some(c.width, |this, w| this.w(px(w)).flex_none())
                        .when(c.width.is_none(), |this| this.flex_1().min_w(px(80.)))
                        .when(c.numeric, |this| this.justify_end())
                        .cursor_pointer()
                        .hover(|s| s.text_color(ink))
                        .when(here.is_some(), |this| this.text_color(ink))
                        // A Tab stop: Enter or Space on the focused header is a click, so it sorts.
                        .when(self.on_sort.is_some(), |this| {
                            this.track_focus(&focus).focus_visible(|s| s.text_color(ink).bg(chip))
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
                        .children(here.map(|asc| Icon::new(if asc { IconName::ArrowUp } else { IconName::ArrowDown }).xsmall()))
                }))
        };

        let rows: Vec<_> = order
            .iter()
            .map(|ix| {
                let row = &self.rows[*ix];
                let picked = self.selected.contains(&row.id);
                let select = self.on_select.clone();
                let current = self.selected.clone();
                let id = row.id.clone();
                h_flex()
                    .id(ElementId::NamedChild(self.id.clone().into(), format!("row-{}", row.id).into()))
                    .h(px(34.))
                    .border_b_1()
                    .border_color(line.opacity(0.6))
                    .when(picked, |this| this.bg(chip.opacity(0.6)))
                    .hover(|s| s.bg(chip.opacity(0.4)))
                    .aria_selected(picked)
                    .child(
                        div().w(px(36.)).flex_none().pl_3().child(
                            Checkbox::new(ElementId::NamedChild(self.id.clone().into(), format!("pick-{}", row.id).into()))
                                .checked(picked)
                                .accessibility_label(format!("Select {}", row.cells.first().cloned().unwrap_or_default()))
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
                    .children(row.cells.iter().enumerate().map(|(col, text)| cell(col, text.clone(), false)))
            })
            .collect();

        let totals = self.columns.iter().any(|c| c.total).then(|| {
            let counted: Vec<&TableRow> = self.rows.iter().filter(|r| !over_selection || self.selected.contains(&r.id)).collect();
            h_flex()
                .h(px(34.))
                .bg(chip.opacity(0.4))
                .child(div().w(px(36.)).flex_none())
                .children(self.columns.iter().enumerate().map(|(col, c)| {
                    let text: SharedString = if c.total {
                        money(counted.iter().filter_map(|r| r.values.get(col).copied().flatten()).sum()).into()
                    } else if col == 0 {
                        (if over_selection { format!("Total of {} selected", counted.len()) } else { "Total".to_string() }).into()
                    } else {
                        SharedString::default()
                    };
                    cell(col, text, true)
                }))
        });

        v_flex()
            .id(self.id.clone())
            .w_full()
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .overflow_hidden()
            .shadow(resting_shadow(cx))
            .child(header)
            .children(rows)
            .when(self.rows.is_empty(), |this| this.child(div().px_4().py_6().text_center().text_color(quiet).child(self.empty.clone())))
            .children(totals)
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
}
