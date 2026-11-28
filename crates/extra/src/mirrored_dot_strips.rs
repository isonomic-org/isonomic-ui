// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Mirrored dot strips: two series stacked either side of a line, row by row.
//!
//! Every row is one round, week or release on the same columns. One series
//! stacks its dots above the row's line, the other below it, so where
//! activity clusters and how it shrinks from row to row reads at a glance.
//! Columns can be pinned with a label. Pointing at a column, or stepping with
//! the arrow keys, lists both counts for every row.

use std::{cell::Cell, f32::consts::TAU, rc::Rc, time::Duration};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::{Presence, TestSupportExt as _, Transition, transition};
use isonomic_ui::component::{
    ActiveTheme as _, Icon, StyledExt as _, TextRole as _, h_flex, rolling_number::group_thousands,
};
use isonomic_ui::{
    App, Div, ElementId, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, PathBuilder, Pixels, Point, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, canvas, div, point,
    prelude::FluentBuilder as _, px,
};

use crate::common::{panel, shell, shell_header};

/// Room left of the plot for row names, and right of it for row totals.
const LEFT: f32 = 64.;
const RIGHT: f32 = 44.;
/// The most dots one side of a row stacks before a dot stands for several.
const STACK: u32 = 5;
/// The widest the gap between two dots of a stack grows.
const STEP: f32 = 6.;
const ROW_GAP: f32 = 14.;
/// Room above the rows for pinned labels, and under them for column labels.
const TOP: f32 = 22.;
const AXIS: f32 = 22.;
/// The plot's width before it has been laid out once.
const FIRST_WIDTH: f32 = 720.;

/// One row of the chart: a count per column for each side of its line.
#[derive(Debug, Clone, PartialEq)]
pub struct StripRow {
    label: SharedString,
    sublabel: Option<SharedString>,
    up: Vec<u32>,
    down: Vec<u32>,
    current: bool,
}

impl StripRow {
    /// A row with one count per column above the line and one below it.
    pub fn new(
        label: impl Into<SharedString>,
        up: impl IntoIterator<Item = u32>,
        down: impl IntoIterator<Item = u32>,
    ) -> Self {
        Self {
            label: label.into(),
            sublabel: None,
            up: up.into_iter().collect(),
            down: down.into_iter().collect(),
            current: false,
        }
    }

    /// A quieter line under the row's name.
    pub fn sublabel(mut self, sublabel: impl Into<SharedString>) -> Self {
        self.sublabel = Some(sublabel.into());
        self
    }

    /// Draw the row with emphasis, such as the latest round.
    pub fn current(mut self) -> Self {
        self.current = true;
        self
    }

    /// Everything in the row, both sides.
    pub fn total(&self) -> u64 {
        self.up
            .iter()
            .chain(&self.down)
            .map(|count| u64::from(*count))
            .sum()
    }

    fn pair(&self, column: usize) -> (u32, u32) {
        (
            self.up.get(column).copied().unwrap_or(0),
            self.down.get(column).copied().unwrap_or(0),
        )
    }
}

/// A column pinned with a label above the chart.
#[derive(Debug, Clone, PartialEq)]
pub struct StripMark {
    column: usize,
    label: SharedString,
    note: Option<SharedString>,
}

impl StripMark {
    /// Pin a column with a short label.
    pub fn new(column: usize, label: impl Into<SharedString>) -> Self {
        Self {
            column,
            label: label.into(),
            note: None,
        }
    }

    /// Longer words for the details shown when the column is picked.
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }
}

/// How many samples one dot stands for, so the tallest stack fits.
pub fn samples_per_dot(rows: &[StripRow]) -> u32 {
    let peak = rows
        .iter()
        .flat_map(|row| row.up.iter().chain(&row.down))
        .copied()
        .max()
        .unwrap_or(0);
    peak.div_ceil(STACK).max(1)
}

/// The height of one row when its dots sit `step` apart.
pub fn row_height(step: f32) -> f32 {
    STACK as f32 * step * 2. + step
}

/// Where the line of row `ix` runs, from the top of the chart.
pub fn row_line(ix: usize, step: f32) -> f32 {
    TOP + ix as f32 * (row_height(step) + ROW_GAP) + row_height(step) / 2.
}

/// The height of the whole chart for a number of rows.
pub fn chart_height(rows: usize, step: f32) -> f32 {
    TOP + rows as f32 * (row_height(step) + ROW_GAP) - ROW_GAP + AXIS
}

/// Whether column `ix` of `count` gets a label when the plot is `plot` wide:
/// one about every 48 pixels, and the last one unless it would crowd.
pub fn column_is_labelled(ix: usize, count: usize, plot: f32) -> bool {
    let room = ((plot / 48.).floor() as usize).max(1);
    let every = count.div_ceil(room).max(1);
    ix % every == 0 || (ix + 1 == count && (ix % every) * 2 >= every)
}

/// Where a key moves the picked column. `None` when the key means nothing
/// here; `Some(None)` when it clears the pick.
pub fn step_column(now: Option<usize>, key: &str, count: usize) -> Option<Option<usize>> {
    if count == 0 {
        return None;
    }
    Some(match key {
        "right" => Some(now.map_or(0, |at| (at + 1) % count)),
        "left" => Some(now.map_or(count - 1, |at| (at + count - 1) % count)),
        "home" => Some(0),
        "end" => Some(count - 1),
        "escape" => None,
        _ => return None,
    })
}

/// One column read aloud: both counts of every row.
pub fn column_reading(column: &str, up: &str, down: &str, rows: &[StripRow], ix: usize) -> String {
    let each: Vec<String> = rows
        .iter()
        .map(|row| {
            let (above, below) = row.pair(ix);
            format!("{} {up} {above}, {down} {below}", row.label)
        })
        .collect();
    format!("{column}: {}", each.join("; "))
}

fn paint_dots(
    window: &mut Window,
    origin: Point<Pixels>,
    dots: &[(f32, f32)],
    radius: f32,
    color: Hsla,
) {
    if dots.is_empty() || color.a <= 0. {
        return;
    }
    let mut path = PathBuilder::fill();
    for (x, y) in dots {
        let ring: Vec<_> = (0..10)
            .map(|step| {
                let angle = step as f32 / 10. * TAU;
                point(
                    origin.x + px(x + radius * angle.cos()),
                    origin.y + px(y + radius * angle.sin()),
                )
            })
            .collect();
        path.add_polygon(&ring, true);
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

type SelectHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// Rows of dot strips with two series mirrored round a line.
#[derive(IntoElement)]
pub struct MirroredDotStrips {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    subtitle: Option<SharedString>,
    columns: Vec<SharedString>,
    rows: Vec<StripRow>,
    up: (SharedString, Option<Hsla>),
    down: (SharedString, Option<Hsla>),
    marks: Vec<StripMark>,
    unit: Option<SharedString>,
    on_select: Option<SelectHandler>,
}

impl MirroredDotStrips {
    /// Create a chart with one label per column, and the names of the series
    /// above and below each row's line.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        columns: Vec<SharedString>,
        up: impl Into<SharedString>,
        down: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            subtitle: None,
            columns,
            rows: Vec::new(),
            up: (up.into(), None),
            down: (down.into(), None),
            marks: Vec::new(),
            unit: None,
            on_select: None,
        }
    }

    /// A quieter line under the title.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// The rows, top to bottom.
    pub fn rows(mut self, rows: impl IntoIterator<Item = StripRow>) -> Self {
        self.rows = rows.into_iter().collect();
        self
    }

    /// Colours of the series above and below the line. By default the ink
    /// above and the data colour below.
    pub fn colors(mut self, up: impl Into<Hsla>, down: impl Into<Hsla>) -> Self {
        self.up.1 = Some(up.into());
        self.down.1 = Some(down.into());
        self
    }

    /// Columns pinned with a label above the chart.
    pub fn marks(mut self, marks: impl IntoIterator<Item = StripMark>) -> Self {
        self.marks = marks.into_iter().collect();
        self
    }

    /// What one sample is called, in the plural, for the note shown when a
    /// dot stands for several.
    pub fn unit(mut self, unit: impl Into<SharedString>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    /// Run with the column the reader points at or steps to.
    pub fn on_select(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Styled for MirroredDotStrips {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for MirroredDotStrips {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let part = |name: String| ElementId::NamedChild(self.id.clone().into(), name.into());
        let count = self.columns.len();
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let raised = theme.popover;
        let mono = theme.mono_font_family.clone();
        let up_color = self.up.1.unwrap_or(ink);
        let down_color = self.down.1.unwrap_or(theme.chart_1);

        // The chart is drawn at the width it was last laid out at.
        let seen = window
            .use_keyed_state(part("width".into()), cx, |_, _| {
                Rc::new(Cell::new(FIRST_WIDTH))
            })
            .read(cx)
            .clone();
        let width = seen.get();
        let plot = (width - LEFT - RIGHT).max(60.);
        let col = plot / count.max(1) as f32;
        let step = col.min(STEP);
        let radius = step * 0.36;
        let per = samples_per_dot(&self.rows);
        let height = chart_height(self.rows.len().max(1), step);
        let centre = move |ix: usize| LEFT + (ix as f32 + 0.5) * col;

        let active = window.use_keyed_state(part("active".into()), cx, |_, _| None::<usize>);
        let picked = (*active.read(cx)).filter(|ix| *ix < count);
        let marked: Vec<usize> = self.marks.iter().map(|mark| mark.column).collect();
        let dimming = transition(
            part("dim".into()),
            if picked.is_some() { 1_f32 } else { 0. },
            Transition::new(Duration::from_millis(150)),
            window,
            cx,
        );
        let appear_time = Duration::from_millis(300 + 60 * self.rows.len() as u64);
        let appeared = Presence::new(part("appear".into()), true)
            .transition(Transition::new(appear_time).ease(|time| time))
            .sample(window, cx)
            .progress;
        let elapsed = appeared * appear_time.as_secs_f32();
        // Rows fade in from the top, 60ms apart.
        let shown = move |row: usize| {
            let linear = ((elapsed - 0.06 * row as f32) / 0.3).clamp(0., 1.);
            1. - (1. - linear).powi(3)
        };

        let paint_rows = self.rows.clone();
        let paint_marked = marked.clone();
        let strips = canvas(
            move |bounds, window, _| {
                let now = f32::from(bounds.size.width);
                if (now - seen.get()).abs() > 0.5 {
                    seen.set(now);
                    window.request_animation_frame();
                }
            },
            move |bounds, _, window, _| {
                let origin = bounds.origin;
                for (ri, row) in paint_rows.iter().enumerate() {
                    let y = row_line(ri, step);
                    let fade = shown(ri);
                    // Empty places, so the scale of each stack stays readable.
                    let mut places = Vec::with_capacity(count * STACK as usize * 2);
                    let mut stacks: [Vec<(f32, f32)>; 4] = Default::default();
                    for ci in 0..count {
                        let x = centre(ci);
                        for k in 0..STACK {
                            let off = (k as f32 + 0.5) * step;
                            places.push((x, y - off));
                            places.push((x, y + off));
                        }
                        let (above, below) = row.pair(ci);
                        let dim = picked.is_some_and(|at| at != ci) && !paint_marked.contains(&ci);
                        for k in 0..above.div_ceil(per) {
                            stacks[usize::from(dim)].push((x, y - (k as f32 + 0.5) * step));
                        }
                        for k in 0..below.div_ceil(per) {
                            stacks[2 + usize::from(dim)].push((x, y + (k as f32 + 0.5) * step));
                        }
                    }
                    paint_dots(
                        window,
                        origin,
                        &places,
                        radius * 0.5,
                        ink.opacity(0.07 * fade),
                    );
                    for (ix, dots) in stacks.iter().enumerate() {
                        let color = if ix < 2 { up_color } else { down_color };
                        let strength = if ix % 2 == 1 { 1. - 0.5 * dimming } else { 1. };
                        paint_dots(
                            window,
                            origin,
                            dots,
                            radius,
                            color.opacity(color.a * fade * strength),
                        );
                    }
                }
            },
        )
        .absolute()
        .inset_0();

        // A band behind every pinned column and the picked one.
        let mut banded = marked.clone();
        banded.extend(picked.filter(|ix| !marked.contains(ix)));
        let bands = banded.into_iter().filter(|ix| *ix < count).map(|ix| {
            div()
                .absolute()
                .left(px(LEFT + ix as f32 * col - 1.))
                .top(px(TOP - 4.))
                .w(px(col + 2.))
                .h(px(height - TOP - AXIS + 8.))
                .rounded(px((col / 2. + 1.).min(6.)))
                .bg(ink.opacity(if picked == Some(ix) { 0.08 } else { 0.04 }))
        });

        // A label centred on a point of the chart.
        let centred = |x: f32, top: f32, label: Div| {
            div()
                .absolute()
                .left(px(x))
                .top(px(top))
                .w(px(0.))
                .flex()
                .justify_center()
                .child(label.flex_none().whitespace_nowrap())
        };
        let pins = self
            .marks
            .iter()
            .filter(|mark| mark.column < count)
            .map(|mark| {
                centred(
                    centre(mark.column),
                    0.,
                    div()
                        .font_family(mono.clone())
                        .text_size(px(11.))
                        .line_height(px(14.))
                        .text_color(quiet)
                        .child(mark.label.clone()),
                )
            });
        let axis = self
            .columns
            .iter()
            .enumerate()
            .filter(|(ix, _)| column_is_labelled(*ix, count, plot))
            .map(|(ix, label)| {
                centred(
                    centre(ix),
                    height - 18.,
                    div()
                        .font_family(mono.clone())
                        .text_size(px(10.))
                        .line_height(px(14.))
                        .text_color(if picked == Some(ix) { ink } else { quiet })
                        .child(label.clone()),
                )
            });

        let names = self.rows.iter().enumerate().map(|(ri, row)| {
            let y = row_line(ri, step);
            let lines = if row.sublabel.is_some() { 30. } else { 16. };
            div()
                .absolute()
                .left_0()
                .top(px(y - lines / 2.))
                .w(px(LEFT - 6.))
                .opacity(shown(ri))
                .child(
                    div()
                        .truncate()
                        .font_family(mono.clone())
                        .text_caption()
                        .text_color(if row.current { ink } else { quiet })
                        .child(row.label.clone()),
                )
                .when_some(row.sublabel.clone(), |this, sublabel| {
                    this.child(
                        div()
                            .truncate()
                            .text_size(px(11.))
                            .line_height(px(14.))
                            .text_color(if row.current { down_color } else { quiet })
                            .child(sublabel),
                    )
                })
        });
        let totals = self.rows.iter().enumerate().map(|(ri, row)| {
            div()
                .absolute()
                .right_0()
                .top(px(row_line(ri, step) - 8.))
                .opacity(shown(ri))
                .font_family(mono.clone())
                .text_caption()
                .text_color(if row.current { ink } else { quiet })
                .child(group_thousands(row.total()))
        });

        // One invisible strip per column takes the pointer.
        let hits = (0..count).map(|ix| {
            let active = active.clone();
            let select = self.on_select.clone();
            div()
                .id(part(format!("column-{ix}")))
                .test_support()
                .flex_1()
                .h_full()
                .on_hover(move |over, window, cx| {
                    if *over || *active.read(cx) == Some(ix) {
                        active.update(cx, |state, cx| {
                            *state = over.then_some(ix);
                            cx.notify();
                        });
                    }
                    if let (Some(select), true) = (&select, *over) {
                        select(ix, window, cx);
                    }
                })
        });

        let tip = picked.map(|ix| {
            let heading = self
                .marks
                .iter()
                .find(|mark| mark.column == ix)
                .map(|mark| mark.note.clone().unwrap_or_else(|| mark.label.clone()))
                .unwrap_or_else(|| self.columns[ix].clone());
            let x = centre(ix);
            // Opens to the right of the column unless it would leave the chart.
            let flip = x + col + 168. > width;
            div()
                .absolute()
                .top(px(TOP))
                .map(|this| {
                    if flip {
                        this.right(px(width - (x - col - 4.)))
                    } else {
                        this.left(px(x + col + 4.))
                    }
                })
                .min_w(px(160.))
                .px_3()
                .py_2()
                .rounded(px(12.))
                .bg(raised)
                .shadow_md()
                .text_caption()
                .child(div().mb_1().font_medium().child(heading))
                .children(self.rows.iter().map(|row| {
                    let (above, below) = row.pair(ix);
                    h_flex()
                        .gap_3()
                        .text_color(quiet)
                        .child(
                            div()
                                .flex_1()
                                .font_family(mono.clone())
                                .child(row.label.clone()),
                        )
                        .child(
                            div()
                                .font_family(mono.clone())
                                .text_color(ink)
                                .child(format!("↑{above} ↓{below}")),
                        )
                }))
        });

        let spoken = match picked {
            Some(ix) => column_reading(&self.columns[ix], &self.up.0, &self.down.0, &self.rows, ix),
            None => format!(
                "{}: {}. Arrow keys read each column.",
                self.title,
                self.rows
                    .iter()
                    .map(|row| format!("{} {}", row.label, row.total()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };

        let key = |color: Hsla, arrow: &'static str, name: SharedString| {
            h_flex()
                .gap(px(6.))
                .child(div().size(px(8.)).rounded_full().bg(color))
                .child(format!("{arrow} {name}"))
        };
        let legend = h_flex()
            .gap_3()
            .text_caption()
            .text_color(quiet)
            .child(key(up_color, "↑", self.up.0.clone()))
            .child(key(down_color, "↓", self.down.0.clone()));

        let key_state = active.clone();
        let key_select = self.on_select.clone();
        shell(cx)
            .id(self.id.clone())
            .max_w(px(768.))
            .child(shell_header(
                Some(Icon::new(CatalogIcon::Row)),
                self.title.clone(),
                self.subtitle.clone().map(|line| line.into_any_element()),
                Some(legend.into_any_element()),
                cx,
            ))
            .child(
                panel(cx)
                    .p_4()
                    .child(
                        div()
                            .id(part("plot".into()))
                            .test_support()
                            .relative()
                            .w_full()
                            .h(px(height))
                            .rounded(px(12.))
                            .role(Role::Group)
                            .aria_label(spoken)
                            .tab_index(0)
                            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                                let now = *key_state.read(cx);
                                let Some(next) =
                                    step_column(now, event.keystroke.key.as_str(), count)
                                else {
                                    return;
                                };
                                key_state.update(cx, |state, cx| {
                                    *state = next;
                                    cx.notify();
                                });
                                if let (Some(select), Some(ix)) = (&key_select, next) {
                                    select(ix, window, cx);
                                }
                                cx.stop_propagation();
                            })
                            .children(bands)
                            .child(strips)
                            .children(pins)
                            .children(axis)
                            .children(names)
                            .children(totals)
                            .child(
                                h_flex()
                                    .absolute()
                                    .left(px(LEFT))
                                    .top(px(TOP))
                                    .w(px(plot))
                                    .h(px(height - TOP - AXIS))
                                    .children(hits),
                            )
                            .children(tip),
                    )
                    .when(per > 1, |this| {
                        this.child(
                            div()
                                .mt_2()
                                .flex()
                                .justify_end()
                                .text_caption()
                                .text_color(quiet)
                                .child(match &self.unit {
                                    Some(unit) => format!("1 dot = {per} {unit}"),
                                    None => format!("1 dot = {per}"),
                                }),
                        )
                    }),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rounds() -> Vec<StripRow> {
        vec![
            StripRow::new("One", [4, 12, 0], [1, 0, 7]),
            StripRow::new("Two", [2, 3], [0, 1, 1]),
        ]
    }

    #[test]
    fn a_dot_stands_for_several_once_a_stack_is_full() {
        // The tallest stack holds 12: five dots of three reach it.
        assert_eq!(samples_per_dot(&rounds()), 3);
        assert_eq!(samples_per_dot(&[StripRow::new("Low", [5, 1], [2])]), 1);
        assert_eq!(samples_per_dot(&[StripRow::new("Six", [6], [])]), 2);
        assert_eq!(samples_per_dot(&[]), 1);
    }

    #[test]
    fn rows_are_spaced_by_their_height_and_a_gap() {
        assert_eq!(row_height(6.), 66.);
        assert_eq!(row_line(0, 6.), 22. + 33.);
        assert_eq!(row_line(1, 6.) - row_line(0, 6.), 66. + 14.);
        // Three rows, two gaps, and room above and below.
        assert_eq!(chart_height(3, 6.), 22. + 3. * 66. + 2. * 14. + 22.);
        // Narrow columns pull the dots, and so the rows, closer.
        assert_eq!(row_height(4.), 44.);
    }

    #[test]
    fn column_labels_thin_out_to_fit() {
        // 480 wide fits ten labels: every third of thirty.
        let shown: Vec<usize> = (0..30)
            .filter(|ix| column_is_labelled(*ix, 30, 480.))
            .collect();
        assert_eq!(shown, [0, 3, 6, 9, 12, 15, 18, 21, 24, 27, 29]);
        // The last column is labelled only when it sits far enough from the one before.
        assert!(column_is_labelled(31, 32, 480.));
        assert!(!column_is_labelled(28, 29, 480.));
        assert!(!column_is_labelled(28, 30, 480.));
        // A few columns are all labelled.
        assert!((0..6).all(|ix| column_is_labelled(ix, 6, 480.)));
    }

    #[test]
    fn keys_step_and_wrap() {
        assert_eq!(step_column(None, "right", 4), Some(Some(0)));
        assert_eq!(step_column(None, "left", 4), Some(Some(3)));
        assert_eq!(step_column(Some(3), "right", 4), Some(Some(0)));
        assert_eq!(step_column(Some(0), "left", 4), Some(Some(3)));
        assert_eq!(step_column(Some(2), "home", 4), Some(Some(0)));
        assert_eq!(step_column(Some(2), "end", 4), Some(Some(3)));
        assert_eq!(step_column(Some(2), "escape", 4), Some(None));
        assert_eq!(step_column(Some(2), "a", 4), None);
        assert_eq!(step_column(None, "right", 0), None);
    }

    #[test]
    fn a_column_is_read_row_by_row() {
        let rows = rounds();
        assert_eq!(rows[0].total(), 24);
        assert_eq!(
            column_reading("Tue", "opened", "closed", &rows, 1),
            "Tue: One opened 12, closed 0; Two opened 3, closed 1"
        );
        // A row with no count for a column reads as zero.
        assert_eq!(
            column_reading("Wed", "opened", "closed", &rows, 2),
            "Wed: One opened 0, closed 7; Two opened 0, closed 1"
        );
    }
}
