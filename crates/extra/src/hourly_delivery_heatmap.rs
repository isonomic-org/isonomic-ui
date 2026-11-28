// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Hourly delivery heatmap: how much each endpoint handled in every hour, and
//! where it went wrong.
//!
//! One row per endpoint, one column per hour. A cell is a slot filled as high
//! as its volume, in six steps; a failing hour turns to the danger colour. A
//! column of dots over the grid counts each hour across all rows, and a thin
//! bar at the end of a row compares the rows. Pointing at a cell, or moving
//! to it with the arrow keys, names it and its counts above the grid.

use std::{cell::Cell, rc::Rc, time::Duration};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::{Presence, TestSupportExt as _, Transition, transition};
use isonomic_ui::component::{
    ActiveTheme as _, Icon, Sizable as _, StyledExt as _, TextRole as _, h_flex,
    rolling_number::{RollingNumber, group_thousands},
    tab::{Tab, TabBar},
    v_flex,
};
use isonomic_ui::{
    App, ElementId, FocusHandle, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, RenderOnce, Role, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Window, canvas, div, prelude::FluentBuilder as _, px, relative,
};

use crate::common::{panel, shell, shell_header};

/// How high a cell fills at each volume level; level 0 is an empty slot.
const LEVELS: [f32; 6] = [0., 0.14, 0.32, 0.52, 0.75, 1.];
/// Dots stacked over one column, per lane.
const DOT_ROWS: usize = 9;
const DOT: f32 = 5.;
const GAP: f32 = 3.;
const ROW: f32 = 26.;
const ROW_GAP: f32 = 6.;
/// Width of the row names, and of the bars after the rows.
const NAMES: f32 = 120.;
const SHARES: f32 = 60.;
/// Steps between axis labels, smallest that fits first.
const STEPS: [usize; 9] = [1, 2, 3, 4, 6, 8, 12, 24, 48];

/// What one endpoint handled in one hour.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HourBucket {
    attempts: u32,
    failed: u32,
    status: Option<SharedString>,
    failing: Option<bool>,
}

impl HourBucket {
    /// A bucket with its attempts, retries included, and how many failed.
    pub fn new(attempts: u32, failed: u32) -> Self {
        Self {
            attempts,
            failed: failed.min(attempts),
            status: None,
            failing: None,
        }
    }

    /// What the failures answered with, such as a status code.
    pub fn status(mut self, status: impl Into<SharedString>) -> Self {
        self.status = Some(status.into());
        self
    }

    /// Mark the bucket as failing or not, whatever its share of failures.
    pub fn failing(mut self, failing: bool) -> Self {
        self.failing = Some(failing);
        self
    }

    /// The bucket reads as failing: marked so, or its share of failed
    /// attempts reaches `threshold` (0 to 1).
    pub fn is_failing(&self, threshold: f64) -> bool {
        self.failing.unwrap_or(
            self.attempts > 0 && f64::from(self.failed) / f64::from(self.attempts) >= threshold,
        )
    }
}

/// One endpoint and its buckets, one per column.
#[derive(Debug, Clone)]
pub struct HeatmapRow {
    label: SharedString,
    buckets: Vec<HourBucket>,
}

impl HeatmapRow {
    /// A row by name, with one bucket per column.
    pub fn new(label: impl Into<SharedString>, buckets: Vec<HourBucket>) -> Self {
        Self {
            label: label.into(),
            buckets,
        }
    }
}

/// The fill level of a cell, 0 to 5, for its attempts against the busiest
/// cell.
pub fn volume_level(attempts: u32, peak: u32) -> usize {
    if attempts == 0 {
        return 0;
    }
    let share = f64::from(attempts) / f64::from(peak.max(1));
    ((share * 5.).ceil() as usize).min(5)
}

/// How many dots a column gets out of `cap`, and how many of them are failed
/// ones. A failing column always shows at least one failed dot.
pub fn column_dots(
    attempts: u32,
    failed: u32,
    failing: bool,
    peak: u32,
    cap: usize,
) -> (usize, usize) {
    let scale = cap as f64 / f64::from(peak.max(1));
    let dots = ((f64::from(attempts) * scale).round() as usize).min(cap);
    let bad = (f64::from(failed) * scale).round() as usize;
    let bad = bad.max(usize::from(failing));
    (dots.max(bad).min(cap), bad.min(cap))
}

/// The gap between axis labels so they fit a plot `width` pixels wide.
pub fn tick_step(columns: usize, width: f32) -> usize {
    let most = ((width / 60.).floor() as usize).max(2);
    STEPS
        .iter()
        .copied()
        .find(|step| columns.div_ceil(*step) <= most)
        .unwrap_or(columns.max(1))
}

/// Where a key takes the cursor in a grid of `rows` by `columns`. Home and
/// End go to the ends of the row, or of the whole grid with `far`.
pub fn step_cell(
    (row, column): (usize, usize),
    rows: usize,
    columns: usize,
    key: &str,
    far: bool,
) -> Option<(usize, usize)> {
    if rows == 0 || columns == 0 {
        return None;
    }
    let (last_row, last_column) = (rows - 1, columns - 1);
    let next = match key {
        "right" => (row, column + 1),
        "left" => (row, column.saturating_sub(1)),
        "down" => (row + 1, column),
        "up" => (row.saturating_sub(1), column),
        "home" => (if far { 0 } else { row }, 0),
        "end" => (if far { last_row } else { row }, last_column),
        _ => return None,
    };
    Some((next.0.min(last_row), next.1.min(last_column)))
}

/// One cell in words, for a screen reader.
pub fn cell_words(row: &str, column: &str, bucket: &HourBucket, failing: bool) -> String {
    let mut words = format!(
        "{row}, {column}: {} attempts, {} failed",
        group_thousands(u64::from(bucket.attempts)),
        group_thousands(u64::from(bucket.failed))
    );
    if let (Some(status), true) = (&bucket.status, bucket.failed > 0) {
        words.push_str(&format!(", status {status}"));
    }
    if failing {
        words.push_str(", failing");
    }
    words
}

type SelectHandler = Rc<dyn Fn(usize, usize, &mut Window, &mut App)>;
type RangeHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// Volume per endpoint and hour.
#[derive(IntoElement)]
pub struct HourlyDeliveryHeatmap {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    section: Option<SharedString>,
    subtitle: Option<SharedString>,
    heading: SharedString,
    date: Option<SharedString>,
    rows: Vec<HeatmapRow>,
    columns: Vec<SharedString>,
    ticks: Option<Vec<usize>>,
    threshold: f64,
    failing_label: SharedString,
    ranges: Vec<SharedString>,
    range: usize,
    on_range: Option<RangeHandler>,
    on_select: Option<SelectHandler>,
}

impl HourlyDeliveryHeatmap {
    /// Create the heatmap from its column labels and rows.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        columns: Vec<SharedString>,
        rows: Vec<HeatmapRow>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            section: None,
            subtitle: None,
            heading: "Attempts by endpoint".into(),
            date: None,
            rows,
            columns,
            ticks: None,
            threshold: 0.05,
            failing_label: "Failing hour".into(),
            ranges: Vec::new(),
            range: 0,
            on_range: None,
            on_select: None,
        }
    }

    /// What the title belongs to, shown before it.
    pub fn section(mut self, section: impl Into<SharedString>) -> Self {
        self.section = Some(section.into());
        self
    }

    /// A quieter line under the title.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// The heading over the chart. "Attempts by endpoint" by default.
    pub fn heading(mut self, heading: impl Into<SharedString>) -> Self {
        self.heading = heading.into();
        self
    }

    /// The window shown beside the heading, such as a date.
    pub fn date(mut self, date: impl Into<SharedString>) -> Self {
        self.date = Some(date.into());
        self
    }

    /// Which columns get a label on the axis. By default an even spread that
    /// fits the width.
    pub fn ticks(mut self, columns: impl IntoIterator<Item = usize>) -> Self {
        self.ticks = Some(columns.into_iter().collect());
        self
    }

    /// The share of failed attempts from which a bucket reads as failing, 0
    /// to 1. 0.05 by default.
    pub fn fail_threshold(mut self, share: f64) -> Self {
        self.threshold = share;
        self
    }

    /// The legend's words for a failing bucket. "Failing hour" by default.
    pub fn failing_label(mut self, label: impl Into<SharedString>) -> Self {
        self.failing_label = label.into();
        self
    }

    /// Show a range switch with these choices, `selected` being the one the
    /// data is for. The host swaps the data when it changes.
    pub fn ranges(
        mut self,
        ranges: impl IntoIterator<Item = impl Into<SharedString>>,
        selected: usize,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.ranges = ranges.into_iter().map(Into::into).collect();
        self.range = selected;
        self.on_range = Some(Rc::new(handler));
        self
    }

    /// Run when a cell is clicked or picked with Enter, with its row and
    /// column.
    pub fn on_select(
        mut self,
        handler: impl Fn(usize, usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Styled for HourlyDeliveryHeatmap {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

struct MapState {
    /// The cell named above the grid, and whether the keyboard chose it.
    active: Option<(usize, usize)>,
    by_keys: bool,
    cursor: (usize, usize),
    /// The range has been switched at least once: later fills run quicker.
    switched: bool,
    width: Rc<Cell<f32>>,
    focus: FocusHandle,
}

/// The mark before one of the footer's numbers.
enum Mark {
    /// A small empty slot.
    Slot,
    /// A dash in a colour.
    Dash(Hsla),
}

fn ease_out(time: f32) -> f32 {
    1. - (1. - time.clamp(0., 1.)).powi(3)
}

fn swatch(height: f32, slot: Hsla, fill: Hsla) -> impl IntoElement {
    div()
        .relative()
        .flex_none()
        .w(px(10.))
        .h(px(14.))
        .rounded(px(3.))
        .bg(slot)
        .overflow_hidden()
        .child(
            div()
                .absolute()
                .bottom_0()
                .left_0()
                .w_full()
                .h(relative(height))
                .rounded(px(3.))
                .bg(fill),
        )
}

impl RenderOnce for HourlyDeliveryHeatmap {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let part = |name: String| ElementId::NamedChild(self.id.clone().into(), name.into());
        let state = window.use_keyed_state(part("state".into()), cx, |_, cx| MapState {
            active: None,
            by_keys: false,
            cursor: (0, 0),
            switched: false,
            width: Rc::new(Cell::new(560.)),
            focus: cx.focus_handle().tab_stop(true),
        });
        let n = self.columns.len();
        let row_count = self.rows.len();
        let in_bounds = |(row, column): &(usize, usize)| *row < row_count && *column < n;
        let (active, by_keys, cursor, switched, width, focus) = {
            let state = state.read(cx);
            (
                state.active.filter(in_bounds),
                state.by_keys,
                state.cursor,
                state.switched,
                state.width.clone(),
                state.focus.clone(),
            )
        };
        let still = cx.reduce_motion();
        let threshold = self.threshold;
        let empty = HourBucket::default();
        let bucket = |row: usize, column: usize| -> &HourBucket {
            self.rows[row].buckets.get(column).unwrap_or(&empty)
        };

        let peak = self
            .rows
            .iter()
            .flat_map(|row| row.buckets.iter().map(|bucket| bucket.attempts))
            .max()
            .unwrap_or(0)
            .max(1);
        // Each hour across every row: attempts, failed, any row failing.
        let hours: Vec<(u32, u32, bool)> = (0..n)
            .map(|column| {
                (0..row_count).fold((0, 0, false), |sum, row| {
                    let bucket = bucket(row, column);
                    (
                        sum.0 + bucket.attempts,
                        sum.1 + bucket.failed,
                        sum.2 || bucket.is_failing(threshold),
                    )
                })
            })
            .collect();
        let hour_peak = hours.iter().map(|hour| hour.0).max().unwrap_or(0).max(1);
        let totals: Vec<(u32, u32, bool)> = self
            .rows
            .iter()
            .map(|row| {
                row.buckets.iter().fold((0, 0, false), |sum, bucket| {
                    (
                        sum.0 + bucket.attempts,
                        sum.1 + bucket.failed,
                        sum.2 || bucket.is_failing(threshold),
                    )
                })
            })
            .collect();
        let attempts: u64 = totals.iter().map(|total| u64::from(total.0)).sum();
        let failed: u64 = totals.iter().map(|total| u64::from(total.1)).sum();
        let row_peak = totals.iter().map(|total| total.0).max().unwrap_or(0).max(1);
        // Fewer, wider columns get wider stacks of dots.
        let lanes = if n <= 7 {
            3
        } else if n <= 12 {
            2
        } else {
            1
        };
        let cap = DOT_ROWS * lanes;
        let ticks = self.ticks.clone().unwrap_or_else(|| {
            let step = tick_step(n, width.get());
            (0..n).filter(|column| column % step == 0).collect()
        });

        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.chart_1;
        let danger = theme.danger;
        let raised = theme.popover;
        let ring = theme.ring;
        let mono = theme.mono_font_family.clone();
        let slot = ink.opacity(0.06);

        // Cells fill in from the first column, a little later row by row;
        // after a range switch they do it quicker.
        let (column_lag, row_lag, dot_lag) = if switched {
            (0.01, 0.012, 0.012)
        } else {
            (0.025, 0.03, 0.03)
        };
        let fill_time = 0.5 + column_lag * n as f32 + row_lag * row_count as f32;
        let elapsed = if still {
            fill_time
        } else {
            Presence::new(part(format!("fill-{}", self.range)), true)
                .transition(Transition::new(Duration::from_secs_f32(fill_time)).ease(|time| time))
                .sample(window, cx)
                .progress
                * fill_time
        };

        let measure = {
            canvas(
                move |bounds, window, _| {
                    let now = f32::from(bounds.size.width);
                    if (now - width.get()).abs() > 0.5 {
                        width.set(now);
                        window.refresh();
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left(px(NAMES))
            .right(px(SHARES))
            .h(px(1.))
        };

        let guide = h_flex()
            .absolute()
            .top_0()
            .bottom_0()
            .left(px(NAMES))
            .right(px(SHARES))
            .gap(px(GAP))
            .children((0..n).map(|column| {
                div()
                    .flex_1()
                    .h_full()
                    .rounded(px(6.))
                    .when(active.is_some_and(|at| at.1 == column), |this| {
                        this.bg(ink.opacity(0.05))
                    })
            }));

        let dots =
            h_flex()
                .pt_1()
                .pb(px(10.))
                .child(div().w(px(NAMES)).flex_none())
                .child(h_flex().flex_1().min_w_0().gap(px(GAP)).children(
                    hours.iter().enumerate().map(|(column, hour)| {
                        let (lit, bad) = column_dots(hour.0, hour.1, hour.2, hour_peak, cap);
                        let dim = active.is_some_and(|at| at.1 != column);
                        let shown = ease_out((elapsed - dot_lag * column as f32) / 0.3);
                        // Stacks grow up from the bottom, lane by lane.
                        let stack = (0..DOT_ROWS).rev().map(move |level| {
                            h_flex().gap(px(GAP)).children((0..lanes).map(move |lane| {
                                let place = level * lanes + lane;
                                let color = if place >= lit {
                                    ink.opacity(0.07)
                                } else if place >= lit - bad {
                                    danger.opacity(if dim { 0.6 } else { 1. })
                                } else {
                                    fill.opacity(if dim { 0.6 } else { 1. })
                                };
                                div().size(px(DOT)).rounded_full().bg(color)
                            }))
                        });
                        v_flex()
                            .flex_1()
                            .items_center()
                            .gap(px(GAP))
                            .opacity(shown)
                            .children(stack)
                    }),
                ))
                .child(div().w(px(SHARES)).flex_none());

        let hover = {
            let state = state.clone();
            Rc::new(move |at: Option<(usize, usize)>, cx: &mut App| {
                state.update(cx, |state, cx| {
                    // Leaving a cell only clears what the pointer chose.
                    match at {
                        Some(at) => {
                            state.active = Some(at);
                            state.by_keys = false;
                        }
                        None if !state.by_keys => state.active = None,
                        None => return,
                    }
                    cx.notify();
                })
            })
        };
        let focused = focus.is_focused(window);
        let grid_rows = self.rows.iter().enumerate().map(|(row, one)| {
            let total = totals[row];
            let row_active = active.is_some_and(|at| at.0 == row);
            let cells = (0..n).map(|column| {
                let bucket = bucket(row, column);
                let failing = bucket.is_failing(threshold);
                let on = active == Some((row, column));
                let level = volume_level(bucket.attempts, peak);
                let lag = column_lag * column as f32 + row_lag * row as f32;
                let shown = ease_out((elapsed - lag) / 0.5);
                let back = match (failing, on) {
                    (true, true) => danger.opacity(0.45),
                    (true, false) => danger.opacity(0.25),
                    (false, true) => ink.opacity(0.18),
                    (false, false) => slot,
                };
                let hover = hover.clone();
                let select = self.on_select.clone();
                div()
                    .id(part(format!("cell-{row}-{column}")))
                    .test_support()
                    .relative()
                    .flex_1()
                    .h_full()
                    .rounded(px(4.))
                    .bg(back)
                    .overflow_hidden()
                    .when(focused && cursor == (row, column), |this| {
                        this.border_1().border_color(ring)
                    })
                    .when(select.is_some(), |this| this.cursor_pointer())
                    .role(Role::GridCell)
                    .aria_label(cell_words(
                        &one.label,
                        &self.columns[column],
                        bucket,
                        failing,
                    ))
                    .on_hover(move |over, _, cx| hover(over.then_some((row, column)), cx))
                    .when_some(select, |this, select| {
                        this.on_click(move |_, window, cx| select(row, column, window, cx))
                    })
                    .child(
                        div()
                            .absolute()
                            .bottom_0()
                            .left_0()
                            .w_full()
                            .h(relative(LEVELS[level] * shown))
                            .rounded(px(4.))
                            .bg(if failing { danger } else { fill }),
                    )
            });
            let share = total.0 as f32 / row_peak as f32 * ease_out((elapsed - 0.25) / 0.3);
            h_flex()
                .id(part(format!("row-{row}")))
                .h(px(ROW))
                .items_stretch()
                .role(Role::Row)
                .aria_label(one.label.clone())
                .child(
                    h_flex()
                        .w(px(NAMES))
                        .flex_none()
                        .pr_2()
                        .font_family(mono.clone())
                        .text_caption()
                        .text_color(if row_active { ink } else { quiet })
                        .child(div().truncate().child(one.label.clone())),
                )
                .child(
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .items_stretch()
                        .gap(px(GAP))
                        .children(cells),
                )
                .child(
                    h_flex().w(px(SHARES)).flex_none().pl(px(10.)).child(
                        div()
                            .w_full()
                            .h_1()
                            .rounded_full()
                            .bg(ink.opacity(0.07))
                            .overflow_hidden()
                            .child(
                                div()
                                    .h_full()
                                    .w(relative(share))
                                    .rounded_full()
                                    .bg(if total.2 { danger } else { fill }),
                            ),
                    ),
                )
        });

        let failing_cells: Vec<String> = self
            .rows
            .iter()
            .flat_map(|row| {
                row.buckets
                    .iter()
                    .enumerate()
                    .filter(|(column, bucket)| *column < n && bucket.is_failing(threshold))
                    .map(|(column, _)| format!("{} at {}", row.label, self.columns[column]))
            })
            .collect();
        let summary = format!(
            "{}{}: {row_count} endpoints over {n} columns, {} attempts, {} failed. {} Arrow keys move between cells.",
            self.heading,
            self.date
                .as_ref()
                .map(|date| format!(", {date}"))
                .unwrap_or_default(),
            group_thousands(attempts),
            group_thousands(failed),
            if failing_cells.is_empty() {
                "Nothing failing.".to_string()
            } else {
                format!("Failing: {}.", failing_cells.join(", "))
            }
        );
        let grid = {
            let state = state.clone();
            let select = self.on_select.clone();
            v_flex()
                .id(part("grid".into()))
                .test_support()
                .track_focus(&focus)
                .gap(px(ROW_GAP))
                .role(Role::Grid)
                .aria_label(summary)
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    let (at, shown) = {
                        let state = state.read(cx);
                        (state.cursor, state.active)
                    };
                    let key = event.keystroke.key.as_str();
                    let next = match key {
                        "escape" => None,
                        "enter" | "space" => {
                            if let Some(select) = &select {
                                select(at.0, at.1, window, cx);
                            }
                            Some(at)
                        }
                        // The first arrow shows where the cursor already is.
                        _ if shown.is_none()
                            && step_cell(at, row_count, n, key, false).is_some() =>
                        {
                            Some(at)
                        }
                        _ => {
                            let far = event.keystroke.modifiers.control
                                || event.keystroke.modifiers.platform;
                            match step_cell(at, row_count, n, key, far) {
                                Some(next) => Some(next),
                                None => return,
                            }
                        }
                    };
                    state.update(cx, |state, cx| {
                        state.active = next;
                        state.by_keys = next.is_some();
                        if let Some(next) = next {
                            state.cursor = next;
                        }
                        cx.notify();
                    });
                    cx.stop_propagation();
                })
                .children(grid_rows)
        };

        let axis = h_flex()
            .pt(px(10.))
            .child(div().w(px(NAMES)).flex_none())
            .child(div().relative().flex_1().min_w_0().h_4().children(
                ticks.iter().filter(|column| **column < n).map(|&column| {
                    div()
                        .absolute()
                        .top_0()
                        .left(relative((column as f32 + 0.5) / n as f32))
                        .w(px(0.))
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .flex_none()
                                .whitespace_nowrap()
                                .font_family(mono.clone())
                                .text_size(px(11.))
                                .line_height(px(16.))
                                .text_color(if active.is_some_and(|at| at.1 == column) {
                                    ink
                                } else {
                                    quiet
                                })
                                .child(self.columns[column].clone()),
                        )
                }),
            ))
            .child(div().w(px(SHARES)).flex_none());

        // Over the dots, centred on the column, so no cell is ever hidden.
        let tip =
            active.map(|(row, column)| {
                let bucket = bucket(row, column);
                let failing = bucket.is_failing(threshold);
                let along = (column as f32 + 0.5) / n as f32;
                let along = if still || by_keys {
                    along
                } else {
                    transition(
                        part("tip-along".into()),
                        along,
                        Transition::new(Duration::from_millis(300)),
                        window,
                        cx,
                    )
                };
                let shown = if still || by_keys {
                    1.
                } else {
                    Presence::new(part("tip-in".into()), true)
                        .transition(Transition::new(Duration::from_millis(200)))
                        .sample(window, cx)
                        .progress
                };
                let counts =
                    if bucket.failed > 0 {
                        h_flex()
                            .gap_1()
                            .child(div().when(failing, |this| this.text_color(danger)).child(
                                format!("{} failed", group_thousands(u64::from(bucket.failed))),
                            ))
                            .child(div().text_color(quiet).child(format!(
                                "of {}",
                                group_thousands(u64::from(bucket.attempts))
                            )))
                            .children(bucket.status.clone().map(|status| {
                                div().font_family(mono.clone()).child(format!("· {status}"))
                            }))
                    } else {
                        h_flex().child(format!(
                            "{} delivered",
                            group_thousands(u64::from(bucket.attempts))
                        ))
                    };
                div()
                    .absolute()
                    .top_0()
                    .left(px(NAMES))
                    .right(px(SHARES))
                    .h(px(4. + DOT_ROWS as f32 * (DOT + GAP) - GAP + 10.))
                    .child(
                        div()
                            .absolute()
                            .bottom(px(6.))
                            .left(relative(along))
                            .w(px(0.))
                            .flex()
                            .justify_center()
                            .child(
                                v_flex()
                                    .flex_none()
                                    .gap(px(2.))
                                    .px_3()
                                    .py_2()
                                    .rounded(px(12.))
                                    .bg(raised)
                                    .shadow_md()
                                    .text_caption()
                                    .whitespace_nowrap()
                                    .opacity(shown)
                                    .child(
                                        h_flex()
                                            .gap_1()
                                            .text_color(quiet)
                                            .child(
                                                div()
                                                    .font_family(mono.clone())
                                                    .child(self.rows[row].label.clone()),
                                            )
                                            .child(format!("· {}", self.columns[column])),
                                    )
                                    .child(counts),
                            ),
                    )
            });

        let legend = h_flex()
            .mt_4()
            .flex_wrap()
            .justify_between()
            .gap_x_4()
            .gap_y_2()
            .text_caption()
            .text_color(quiet)
            .child(
                h_flex()
                    .gap_2()
                    .child("Fewer")
                    .child(
                        h_flex()
                            .items_end()
                            .gap(px(3.))
                            .children(LEVELS.iter().map(|height| swatch(*height, slot, fill))),
                    )
                    .child("More"),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(swatch(LEVELS[3], danger.opacity(0.25), danger))
                    .child(self.failing_label.clone()),
            );

        let stat = |name: &'static str, value: u64, mark: Mark| {
            v_flex()
                .min_w_0()
                .child(div().text_caption().text_color(quiet).child(name))
                .child(
                    h_flex()
                        .mt_1()
                        .gap_2()
                        .text_size(px(18.))
                        .font_medium()
                        .child(match mark {
                            Mark::Slot => div()
                                .flex_none()
                                .size(px(10.))
                                .rounded(px(3.))
                                .bg(ink.opacity(0.14)),
                            Mark::Dash(color) => {
                                div().flex_none().w_3().h(px(3.)).rounded_full().bg(color)
                            }
                        })
                        .child(
                            RollingNumber::grouped(part(format!("stat-{name}")), value)
                                .line_height(px(28.)),
                        ),
                )
        };
        let stats = h_flex()
            .mt_5()
            .px_1()
            .gap_x_8()
            .child(stat("Attempts", attempts, Mark::Slot))
            .child(stat("Delivered", attempts - failed, Mark::Dash(fill)))
            .child(stat("Failed", failed, Mark::Dash(danger)));

        let title = h_flex()
            .children(self.section.clone().map(|section| {
                h_flex()
                    .text_color(quiet)
                    .child(section)
                    .child(div().px(px(6.)).opacity(0.5).child("/"))
            }))
            .child(self.title.clone());
        let switch = (!self.ranges.is_empty()).then(|| {
            let state = state.clone();
            let on_range = self.on_range.clone();
            TabBar::new(part("range".into()))
                .segmented()
                .small()
                .selected_index(self.range)
                .on_click(move |ix: &usize, window, cx| {
                    // Other columns come with the new range: drop the pick.
                    state.update(cx, |state, cx| {
                        state.active = None;
                        state.by_keys = false;
                        state.switched = true;
                        cx.notify();
                    });
                    if let Some(handler) = &on_range {
                        handler(*ix, window, cx);
                    }
                })
                .children(
                    self.ranges
                        .iter()
                        .map(|range| Tab::new().label(range.clone())),
                )
                .into_any_element()
        });
        let leave = hover.clone();

        shell(cx)
            .id(self.id.clone())
            .max_w(px(672.))
            .role(Role::Region)
            .aria_label(match &self.section {
                Some(section) => format!("{section}: {}", self.title),
                None => self.title.to_string(),
            })
            .child(shell_header(
                Some(Icon::new(CatalogIcon::Webhook)),
                title,
                self.subtitle.clone().map(|line| line.into_any_element()),
                switch,
                cx,
            ))
            .child(
                panel(cx)
                    .p_5()
                    .child(
                        h_flex()
                            .mb_4()
                            .flex_wrap()
                            .items_baseline()
                            .justify_between()
                            .gap_x_3()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .line_height(px(20.))
                                    .font_medium()
                                    .child(self.heading.clone()),
                            )
                            .children(self.date.clone().map(|date| {
                                div()
                                    .flex_none()
                                    .text_caption()
                                    .text_color(quiet)
                                    .child(date)
                            })),
                    )
                    .child(
                        div()
                            .id(part("chart".into()))
                            .relative()
                            .on_hover(move |over, _, cx| {
                                if !*over {
                                    leave(None, cx);
                                }
                            })
                            .child(measure)
                            .child(guide)
                            .child(dots)
                            .child(grid)
                            .child(axis)
                            .children(tip),
                    )
                    .child(legend)
                    .child(stats),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bucket_fails_from_the_threshold_or_when_marked() {
        assert!(HourBucket::new(100, 5).is_failing(0.05));
        assert!(!HourBucket::new(100, 4).is_failing(0.05));
        assert!(!HourBucket::new(0, 0).is_failing(0.05));
        assert!(HourBucket::new(100, 1).failing(true).is_failing(0.05));
        assert!(!HourBucket::new(100, 60).failing(false).is_failing(0.05));
        // Failures never outnumber attempts.
        assert_eq!(HourBucket::new(3, 9), HourBucket::new(3, 3));
    }

    #[test]
    fn volume_falls_into_six_levels() {
        assert_eq!(volume_level(0, 500), 0);
        assert_eq!(volume_level(1, 500), 1);
        assert_eq!(volume_level(100, 500), 1);
        assert_eq!(volume_level(101, 500), 2);
        assert_eq!(volume_level(499, 500), 5);
        assert_eq!(volume_level(900, 500), 5);
        assert_eq!(volume_level(4, 0), 5);
    }

    #[test]
    fn a_failing_hour_always_shows_a_failed_dot() {
        assert_eq!(column_dots(900, 0, false, 900, 9), (9, 0));
        assert_eq!(column_dots(450, 100, true, 900, 9), (5, 1));
        // Three failures in 900 round away, but the hour is failing.
        assert_eq!(column_dots(300, 3, true, 900, 9), (3, 1));
        assert_eq!(column_dots(10, 10, true, 900, 9), (1, 1));
        assert_eq!(column_dots(0, 0, false, 0, 9), (0, 0));
    }

    #[test]
    fn axis_labels_thin_out_to_fit() {
        assert_eq!(tick_step(24, 480.), 3);
        assert_eq!(tick_step(24, 240.), 6);
        assert_eq!(tick_step(7, 480.), 1);
        assert_eq!(tick_step(60, 480.), 8);
        assert_eq!(tick_step(0, 480.), 1);
    }

    #[test]
    fn keys_move_the_cursor_and_cells_read_aloud() {
        assert_eq!(step_cell((0, 0), 4, 24, "right", false), Some((0, 1)));
        assert_eq!(step_cell((0, 0), 4, 24, "left", false), Some((0, 0)));
        assert_eq!(step_cell((3, 23), 4, 24, "down", false), Some((3, 23)));
        assert_eq!(step_cell((2, 5), 4, 24, "home", false), Some((2, 0)));
        assert_eq!(step_cell((2, 5), 4, 24, "end", true), Some((3, 23)));
        assert_eq!(step_cell((2, 5), 4, 24, "x", false), None);
        assert_eq!(step_cell((0, 0), 0, 24, "right", false), None);
        let bucket = HourBucket::new(1840, 212).status("503");
        assert_eq!(
            cell_words("billing", "14:00", &bucket, true),
            "billing, 14:00: 1,840 attempts, 212 failed, status 503, failing"
        );
        assert_eq!(
            cell_words(
                "billing",
                "15:00",
                &HourBucket::new(12, 0).status("503"),
                false
            ),
            "billing, 15:00: 12 attempts, 0 failed"
        );
    }
}
