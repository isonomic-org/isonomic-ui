// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Isometric stacked bars: rows of boxes seen from above at an angle, each box
//! split into layers, one per series.
//!
//! Every layer has three visible faces: a light top, the series colour on the
//! left, and a deeper shade on the right, all mixed from the series colour so
//! they hold in both themes. Bars rise into place when first shown (at once
//! under reduced motion). Pointing at a bar, or stepping with the arrow keys
//! (left and right along a row, up and down between rows), picks it: the
//! others dim and the line under the drawing reads its layers and total.

use std::{cell::Cell, rc::Rc, time::Duration};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::{Presence, TestSupportExt as _, Transition};
use isonomic_ui::component::{ActiveTheme as _, Icon, StyledExt as _, TextRole as _, h_flex, look};
use isonomic_ui::{
    App, ElementId, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent, MouseMoveEvent,
    ParentElement as _, PathBuilder, Pixels, Point, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, canvas, div, hsla, point, px,
};

use crate::common::{panel, shell, shell_header};

/// The cosine and sine of the 30 degree isometric angle.
const COS: f32 = 0.866_025_4;
const SIN: f32 = 0.5;
/// Room above the tallest bar, right of the rows for their names, and under
/// the front row for the column labels.
const HEAD: f32 = 12.;
const NAME_ROOM: f32 = 64.;
const FOOT: f32 = 28.;

/// The three shades a layer of a box is painted in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IsoFaces {
    /// The face looking up: the lightest.
    pub top: Hsla,
    /// The face turned left toward the reader: the series colour itself.
    pub left: Hsla,
    /// The face turned right, away from the light: the deepest.
    pub right: Hsla,
}

/// The faces of a series colour, mixed with the theme's mixing helper: the
/// top toward white, the right side toward black.
pub fn iso_faces(color: Hsla) -> IsoFaces {
    IsoFaces {
        top: look::mix(color, tone(color, 1.), 0.32),
        left: color,
        right: look::mix(color, tone(color, 0.), 0.26),
    }
}

/// One bar: a label and one value per series, stacked from the first.
#[derive(Debug, Clone, PartialEq)]
pub struct IsoBar {
    /// What the bar stands for, such as a day.
    pub label: SharedString,
    /// One value per series, all zero or more.
    pub values: Vec<f64>,
}

impl IsoBar {
    /// A bar with its label and values.
    pub fn new(label: impl Into<SharedString>, values: impl Into<Vec<f64>>) -> Self {
        Self {
            label: label.into(),
            values: values.into(),
        }
    }

    /// Its values added up.
    pub fn total(&self) -> f64 {
        self.values.iter().map(|v| v.max(0.)).sum()
    }
}

/// A named row of bars.
#[derive(Debug, Clone, PartialEq)]
pub struct IsoRow {
    /// The row's name, drawn at its left end.
    pub name: SharedString,
    /// Its bars, front to back along the row.
    pub bars: Vec<IsoBar>,
}

impl IsoRow {
    /// A row with a name and its bars.
    pub fn new(name: impl Into<SharedString>, bars: Vec<IsoBar>) -> Self {
        Self {
            name: name.into(),
            bars,
        }
    }
}

/// Where the ground point (`x` along a row, `y` across rows, both in cells)
/// at height `z` pixels lands on screen, `cell` pixels to a cell, before the
/// drawing is shifted into its box.
pub fn project(x: f32, y: f32, z: f32, cell: f32) -> Point<f32> {
    point((x - y) * COS * cell, (x + y) * SIN * cell - z)
}

/// The drawing's layout for `rows` rows of at most `columns` bars.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IsoLayout {
    /// Pixels to one cell of ground.
    pub cell: f32,
    /// The tallest bar's height in pixels.
    pub rise: f32,
    /// How far right and down the projected origin moves to fit the box.
    pub shift: Point<f32>,
    /// The box's size.
    pub width: f32,
    pub height: f32,
}

/// A bar's footprint is this share of its cell; the rest is the gap.
const FOOTPRINT: f32 = 0.62;
/// The colour's own hue and saturation at lightness `l`, so mixing toward
/// it changes only how light the colour is.
fn tone(color: Hsla, l: f32) -> Hsla {
    hsla(color.h, color.s, l, color.a)
}

/// A face colour faded `k` of the way into a page of lightness `page`,
/// staying opaque so the boxes behind never show through.
pub fn recede(color: Hsla, page: f32, k: f32) -> Hsla {
    let k = k.clamp(0., 1.);
    hsla(
        color.h,
        color.s * (1. - 0.8 * k),
        color.l + (page - color.l) * 0.75 * k,
        color.a,
    )
}

/// Rows sit this many cells apart, so an aisle runs between them.
const ROW_PITCH: f32 = 1.7;

/// Where row `r` starts across the ground, in cells.
pub fn row_y(r: usize) -> f32 {
    r as f32 * ROW_PITCH
}

impl IsoLayout {
    /// Lay out `rows` rows of `columns` cells with bars up to `rise` pixels.
    pub fn new(rows: usize, columns: usize, cell: f32, rise: f32) -> Self {
        let rows = row_y(rows.max(1) - 1) + 1.;
        let columns = columns.max(1) as f32;
        // The ground's corners: the far left one is (0, rows), the right one
        // (columns, 0); the near one (columns, rows); the far one (0, 0).
        let left = project(0., rows, 0., cell).x;
        let right = project(columns, 0., 0., cell).x;
        let bottom = project(columns, rows, 0., cell).y;
        Self {
            cell,
            rise,
            shift: point(8. - left, HEAD + rise),
            width: right - left + NAME_ROOM + 8.,
            height: HEAD + rise + bottom + FOOT,
        }
    }

    /// A ground point at a height, in box coordinates.
    pub fn at(&self, x: f32, y: f32, z: f32) -> Point<f32> {
        let p = project(x, y, z, self.cell);
        point(p.x + self.shift.x, p.y + self.shift.y)
    }

    /// The outline of a box on cell (`column`, `row`) from height `z0` to
    /// `z1`: the six corners of its silhouette, clockwise from the top left.
    pub fn outline(&self, column: usize, row: usize, z0: f32, z1: f32) -> [Point<f32>; 6] {
        let pad = (1. - FOOTPRINT) / 2.;
        let x0 = column as f32 + pad;
        let y0 = row_y(row) + pad;
        let (x1, y1) = (x0 + FOOTPRINT, y0 + FOOTPRINT);
        [
            self.at(x0, y1, z1),
            self.at(x0, y0, z1),
            self.at(x1, y0, z1),
            self.at(x1, y0, z0),
            self.at(x1, y1, z0),
            self.at(x0, y1, z0),
        ]
    }

    /// The three faces of the same box: top, left, right.
    pub fn faces(&self, column: usize, row: usize, z0: f32, z1: f32) -> [[Point<f32>; 4]; 3] {
        let pad = (1. - FOOTPRINT) / 2.;
        let x0 = column as f32 + pad;
        let y0 = row_y(row) + pad;
        let (x1, y1) = (x0 + FOOTPRINT, y0 + FOOTPRINT);
        [
            [
                self.at(x0, y0, z1),
                self.at(x1, y0, z1),
                self.at(x1, y1, z1),
                self.at(x0, y1, z1),
            ],
            [
                self.at(x0, y1, z0),
                self.at(x1, y1, z0),
                self.at(x1, y1, z1),
                self.at(x0, y1, z1),
            ],
            [
                self.at(x1, y0, z0),
                self.at(x1, y1, z0),
                self.at(x1, y1, z1),
                self.at(x1, y0, z1),
            ],
        ]
    }
}

/// Whether `p` lies inside the polygon `shape`.
pub fn inside(p: Point<f32>, shape: &[Point<f32>]) -> bool {
    let mut hit = false;
    let mut j = shape.len().wrapping_sub(1);
    for i in 0..shape.len() {
        let (a, b) = (shape[i], shape[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            hit = !hit;
        }
        j = i;
    }
    hit
}

/// The order bars are painted in, far to near, as (row, column).
pub fn paint_order(rows: &[IsoRow]) -> Vec<(usize, usize)> {
    let mut order: Vec<(usize, usize)> = rows
        .iter()
        .enumerate()
        .flat_map(|(r, row)| (0..row.bars.len()).map(move |c| (r, c)))
        .collect();
    order.sort_by_key(|(r, c)| (r + c, *r));
    order
}

/// Where a key moves the pick: `Some(next)` for a key the drawing takes
/// (`next` is `None` to clear it), `None` for any other key.
pub fn step_pick(
    current: Option<(usize, usize)>,
    key: &str,
    rows: &[IsoRow],
) -> Option<Option<(usize, usize)>> {
    let len = |r: usize| rows.get(r).map_or(0, |row| row.bars.len());
    let first = (0..rows.len()).find(|r| len(*r) > 0)?;
    let Some((r, c)) = current else {
        return match key {
            "right" | "left" | "up" | "down" | "home" | "end" => Some(Some((first, 0))),
            _ => None,
        };
    };
    let next = match key {
        "right" => (r, (c + 1).min(len(r) - 1)),
        "left" => (r, c.saturating_sub(1)),
        "down" | "up" => {
            let rows_with: Vec<usize> = (0..rows.len()).filter(|r| len(*r) > 0).collect();
            let at = rows_with.iter().position(|x| *x == r).unwrap_or(0);
            let to = if key == "down" {
                rows_with[(at + 1).min(rows_with.len() - 1)]
            } else {
                rows_with[at.saturating_sub(1)]
            };
            (to, c.min(len(to) - 1))
        }
        "home" => (r, 0),
        "end" => (r, len(r) - 1),
        "escape" => return Some(None),
        _ => return None,
    };
    Some(Some(next))
}

/// A value with at most one decimal.
fn plain(value: f64) -> String {
    let text = format!("{value:.1}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The words for a picked bar: "North, Wed: Bread 40, Rolls 22, total 62".
pub fn bar_readout(row: &IsoRow, bar: &IsoBar, series: &[SharedString], unit: &str) -> String {
    let parts: Vec<String> = series
        .iter()
        .enumerate()
        .map(|(ix, name)| {
            format!(
                "{name} {}",
                plain(bar.values.get(ix).copied().unwrap_or(0.))
            )
        })
        .collect();
    let unit = if unit.is_empty() {
        String::new()
    } else {
        format!(" {unit}")
    };
    format!(
        "{}, {}: {}, total {}{unit}",
        row.name,
        bar.label,
        parts.join(", "),
        plain(bar.total())
    )
}

type SelectHandler = Rc<dyn Fn(usize, usize, &mut Window, &mut App)>;

/// Rows of stacked bars drawn as boxes in an isometric view.
#[derive(IntoElement)]
pub struct IsometricStackedBars {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    subtitle: Option<SharedString>,
    series: Vec<SharedString>,
    rows: Vec<IsoRow>,
    unit: SharedString,
    cell: f32,
    rise: f32,
    on_select: Option<SelectHandler>,
}

impl IsometricStackedBars {
    /// A drawing with a title and no rows yet.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        series: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            subtitle: None,
            series: series.into_iter().map(Into::into).collect(),
            rows: Vec::new(),
            unit: SharedString::default(),
            cell: 34.,
            rise: 132.,
            on_select: None,
        }
    }

    /// A quieter line under the title.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Add a row. Rows go back to front.
    pub fn row(mut self, row: IsoRow) -> Self {
        self.rows.push(row);
        self
    }

    /// What the values count, said after a total, such as "kg".
    pub fn unit(mut self, unit: impl Into<SharedString>) -> Self {
        self.unit = unit.into();
        self
    }

    /// Pixels to one cell of ground. 34 by default.
    pub fn cell(mut self, cell: impl Into<Pixels>) -> Self {
        self.cell = f32::from(cell.into()).max(12.);
        self
    }

    /// The tallest bar's height. 132px by default.
    pub fn rise(mut self, rise: impl Into<Pixels>) -> Self {
        self.rise = f32::from(rise.into()).max(24.);
        self
    }

    /// Run with the row and column of the bar the reader picks.
    pub fn on_select(
        mut self,
        handler: impl Fn(usize, usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Styled for IsometricStackedBars {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for IsometricStackedBars {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let part = |name: &str| ElementId::NamedChild(self.id.clone().into(), name.into());
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let ink = theme.foreground;
        let mono = theme.mono_font_family.clone();
        let palette = [
            theme.chart_1,
            theme.chart_2,
            theme.chart_3,
            theme.chart_4,
            theme.chart_5,
        ];
        let faces: Vec<IsoFaces> = (0..self.series.len())
            .map(|ix| iso_faces(palette[ix % palette.len()]))
            .collect();
        let ground = look::ink::wash_strong(cx);
        let page = cx.theme().background.l;

        let columns = self
            .rows
            .iter()
            .map(|row| row.bars.len())
            .max()
            .unwrap_or(0);
        let layout = IsoLayout::new(self.rows.len(), columns, self.cell, self.rise);
        let peak = self
            .rows
            .iter()
            .flat_map(|row| row.bars.iter().map(IsoBar::total))
            .fold(0., f64::max)
            .max(f64::MIN_POSITIVE);

        let pick_state = window.use_keyed_state(part("pick"), cx, |_, _| None::<(usize, usize)>);
        let pick = *pick_state.read(cx);
        let origin = window
            .use_keyed_state(part("origin"), cx, |_, _| {
                Rc::new(Cell::new(point(px(0.), px(0.))))
            })
            .read(cx)
            .clone();

        let grown = Presence::new(part("rise"), true)
            .transition(Transition::new(Duration::from_millis(520)))
            .sample(window, cx)
            .progress;

        // Picking eases in: the drawing dims as a whole while the picked bar
        // keeps its full colour, both on the hover fade.
        let dimmed = look::hover_fade(part("dim"), pick.is_some(), window, cx);
        let focus: Vec<Vec<f32>> = self
            .rows
            .iter()
            .enumerate()
            .map(|(r, row)| {
                (0..row.bars.len())
                    .map(|c| {
                        look::hover_fade(
                            part(&format!("bar-{r}-{c}")),
                            pick == Some((r, c)),
                            window,
                            cx,
                        )
                    })
                    .collect()
            })
            .collect();
        let rows = self.rows.clone();
        let order = paint_order(&rows);
        let paint_rows = rows.clone();
        let paint_order_list = order.clone();
        let paint_origin = origin.clone();
        let paint_faces = faces.clone();
        let drawing = canvas(
            move |bounds, _, _| paint_origin.set(bounds.origin),
            move |bounds, _, window, _| {
                let to = |p: Point<f32>| point(bounds.left() + px(p.x), bounds.top() + px(p.y));
                let poly = |window: &mut Window, corners: &[Point<f32>], color: Hsla| {
                    let mut path = PathBuilder::fill();
                    let shape: Vec<_> = corners.iter().map(|p| to(*p)).collect();
                    path.add_polygon(&shape, true);
                    if let Ok(path) = path.build() {
                        window.paint_path(path, color);
                    }
                };
                // A soft ground plate under each row.
                for (r, row) in paint_rows.iter().enumerate() {
                    let n = row.bars.len() as f32;
                    let plate = [
                        layout.at(0.05, row_y(r) + 0.05, 0.),
                        layout.at(n - 0.05, row_y(r) + 0.05, 0.),
                        layout.at(n - 0.05, row_y(r) + 0.95, 0.),
                        layout.at(0.05, row_y(r) + 0.95, 0.),
                    ];
                    poly(window, &plate, ground);
                }
                for &(r, c) in &paint_order_list {
                    let bar = &paint_rows[r].bars[c];
                    // Bars rise one after another along the drawing.
                    let delay = (r + c) as f32 * 0.04;
                    let shown = ((grown - delay) / (1. - delay).max(0.2)).clamp(0., 1.);
                    let shown = 1. - (1. - shown).powi(3);
                    let dim = 0.7 * dimmed * (1. - focus[r][c]);
                    let mut base = 0.;
                    for (s, value) in bar.values.iter().enumerate() {
                        let value = value.max(0.);
                        if value <= 0. {
                            continue;
                        }
                        let z0 = (base / peak) as f32 * layout.rise * shown;
                        base += value;
                        let z1 = (base / peak) as f32 * layout.rise * shown;
                        let shade = paint_faces[s % paint_faces.len().max(1)];
                        let [top, left, right] = layout.faces(c, r, z0, z1);
                        poly(window, &left, recede(shade.left, page, dim));
                        poly(window, &right, recede(shade.right, page, dim));
                        poly(window, &top, recede(shade.top, page, dim));
                    }
                }
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .w(px(layout.width))
        .h(px(layout.height));

        // Row names at the far end of each row; column labels along the front.
        let mut labels = Vec::new();
        for (r, row) in rows.iter().enumerate() {
            // The row's far end, with its name just past it, clear of the bars.
            let at = layout.at(row.bars.len() as f32, row_y(r) + 0.5, 0.);
            labels.push(
                div()
                    .absolute()
                    .top(px(at.y - 8.))
                    .left(px(at.x + 6.))
                    .w(px(NAME_ROOM))
                    .text_label_small()
                    .text_color(ink)
                    .truncate()
                    .child(row.name.clone())
                    .into_any_element(),
            );
        }
        if let Some(front) = rows.len().checked_sub(1) {
            for (c, bar) in rows[front].bars.iter().enumerate() {
                let at = layout.at(c as f32 + 0.5, row_y(front) + 1., 0.);
                labels.push(
                    div()
                        .absolute()
                        .top(px(at.y + 2.))
                        .left(px(at.x - 30.))
                        .w(px(60.))
                        .flex()
                        .justify_center()
                        .text_size(px(10.))
                        .line_height(px(16.))
                        .text_color(quiet)
                        .child(bar.label.clone())
                        .into_any_element(),
                );
            }
        }

        let readout = pick
            .and_then(|(r, c)| Some((rows.get(r)?, rows.get(r)?.bars.get(c)?)))
            .map(|(row, bar)| bar_readout(row, bar, &self.series, &self.unit));

        let hover_state = pick_state.clone();
        let hover_select = self.on_select.clone();
        let hit_rows = rows.clone();
        let hit_order = order;
        let on_move = move |event: &MouseMoveEvent, window: &mut Window, cx: &mut App| {
            let o = origin.get();
            let p = point(
                f32::from(event.position.x - o.x),
                f32::from(event.position.y - o.y),
            );
            let hit = hit_order.iter().rev().copied().find(|&(r, c)| {
                let total = hit_rows[r].bars[c].total();
                let z1 = (total / peak) as f32 * layout.rise;
                inside(p, &layout.outline(c, r, 0., z1.max(2.)))
            });
            if *hover_state.read(cx) != hit {
                hover_state.update(cx, |state, cx| {
                    *state = hit;
                    cx.notify();
                });
                if let (Some(select), Some((r, c))) = (&hover_select, hit) {
                    select(r, c, window, cx);
                }
            }
        };

        let key_state = pick_state.clone();
        let key_rows = rows.clone();
        let key_select = self.on_select.clone();
        let spoken = format!(
            "{}: {} rows of bars, layers {}{}",
            self.title,
            rows.len(),
            self.series
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(", "),
            readout
                .as_ref()
                .map(|words| format!(". Picked {words}"))
                .unwrap_or_else(|| ". Use the arrow keys to read a bar".into())
        );

        let legend = h_flex()
            .flex_wrap()
            .gap_x_4()
            .gap_y_1()
            .text_caption()
            .text_color(quiet)
            .children(self.series.iter().zip(&faces).map(|(name, shade)| {
                h_flex()
                    .gap(px(6.))
                    .child(
                        h_flex()
                            .child(div().w(px(5.)).h(px(10.)).bg(shade.left))
                            .child(div().w(px(5.)).h(px(10.)).bg(shade.right)),
                    )
                    .child(name.clone())
            }));

        let footer = readout.clone().unwrap_or_else(|| {
            let all: f64 = rows
                .iter()
                .flat_map(|row| row.bars.iter().map(IsoBar::total))
                .sum();
            format!(
                "Total {}{}",
                plain(all),
                if self.unit.is_empty() {
                    String::new()
                } else {
                    format!(" {}", self.unit)
                }
            )
        });

        let leave_state = pick_state;
        shell(cx)
            .id(self.id.clone())
            .max_w(px(640.))
            .child(shell_header(
                Some(Icon::new(CatalogIcon::Chart3d)),
                self.title.clone(),
                self.subtitle.clone().map(|line| line.into_any_element()),
                None,
                cx,
            ))
            .child(
                panel(cx)
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(legend)
                    .child(
                        div()
                            .id(part("plot"))
                            .test_support()
                            .w_full()
                            .flex()
                            .justify_center()
                            .overflow_hidden()
                            .rounded(px(8.))
                            .role(Role::Group)
                            .aria_label(spoken)
                            .tab_index(0)
                            .focus_visible(|this| this.bg(look::ink::wash(cx)))
                            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                                let now = *key_state.read(cx);
                                let Some(next) =
                                    step_pick(now, event.keystroke.key.as_str(), &key_rows)
                                else {
                                    return;
                                };
                                key_state.update(cx, |state, cx| {
                                    *state = next;
                                    cx.notify();
                                });
                                if let (Some(select), Some((r, c))) = (&key_select, next) {
                                    select(r, c, window, cx);
                                }
                                cx.stop_propagation();
                            })
                            .child(
                                div()
                                    .id(part("drawing"))
                                    .relative()
                                    .flex_none()
                                    .w(px(layout.width))
                                    .h(px(layout.height))
                                    .on_mouse_move(on_move)
                                    .on_hover(move |over, _, cx| {
                                        if !*over && leave_state.read(cx).is_some() {
                                            leave_state.update(cx, |state, cx| {
                                                *state = None;
                                                cx.notify();
                                            });
                                        }
                                    })
                                    .child(drawing)
                                    .children(labels),
                            ),
                    )
                    .child(
                        div()
                            .min_h(px(16.))
                            .text_caption()
                            .font_family(mono)
                            .text_color(if readout.is_some() { ink } else { quiet })
                            .child(footer),
                    ),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows() -> Vec<IsoRow> {
        vec![
            IsoRow::new(
                "Back",
                vec![
                    IsoBar::new("a", vec![1., 2.]),
                    IsoBar::new("b", vec![3., 0.]),
                ],
            ),
            IsoRow::new("Front", vec![IsoBar::new("a", vec![4., 1.])]),
        ]
    }

    #[test]
    fn faces_mix_lighter_on_top_and_deeper_on_the_right() {
        let color = Hsla {
            h: 0.6,
            s: 0.8,
            l: 0.5,
            a: 1.,
        };
        let faces = iso_faces(color);
        assert_eq!(faces.left, color);
        assert!(faces.top.l > color.l);
        assert!(faces.right.l < color.l);
        assert_eq!(recede(color, 1., 0.), color);
        let faded = recede(color, 1., 1.);
        assert!(faded.l > color.l && faded.s < color.s && faded.a == 1.);
    }

    #[test]
    fn the_projection_tilts_thirty_degrees() {
        let p = project(1., 0., 0., 10.);
        assert!((p.x - 8.660_254).abs() < 1e-4 && (p.y - 5.).abs() < 1e-4);
        let q = project(0., 1., 20., 10.);
        assert!((q.x + 8.660_254).abs() < 1e-4 && (q.y + 15.).abs() < 1e-4);
    }

    #[test]
    fn the_layout_fits_the_ground_and_the_tallest_bar() {
        let layout = IsoLayout::new(2, 5, 10., 50.);
        let far_left = layout.at(0., row_y(1) + 1., 0.);
        assert!((far_left.x - 8.).abs() < 1e-3);
        let top = layout.at(0., 0., 50.);
        assert!((top.y - HEAD).abs() < 1e-3);
        let near = layout.at(5., row_y(1) + 1., 0.);
        assert!((layout.height - near.y - FOOT).abs() < 1e-3);
    }

    #[test]
    fn a_point_on_a_bar_hits_it_and_one_beside_it_does_not() {
        let layout = IsoLayout::new(1, 1, 40., 60.);
        let shape = layout.outline(0, 0, 0., 30.);
        let middle = layout.at(0.5, 0.5, 15.);
        assert!(inside(middle, &shape));
        assert!(!inside(point(middle.x + 200., middle.y), &shape));
    }

    #[test]
    fn far_bars_paint_first() {
        assert_eq!(paint_order(&rows()), vec![(0, 0), (0, 1), (1, 0)]);
    }

    #[test]
    fn keys_walk_rows_and_columns() {
        let rows = rows();
        assert_eq!(step_pick(None, "right", &rows), Some(Some((0, 0))));
        assert_eq!(step_pick(Some((0, 0)), "right", &rows), Some(Some((0, 1))));
        assert_eq!(step_pick(Some((0, 1)), "right", &rows), Some(Some((0, 1))));
        // The front row is shorter, so stepping down clamps the column.
        assert_eq!(step_pick(Some((0, 1)), "down", &rows), Some(Some((1, 0))));
        assert_eq!(step_pick(Some((1, 0)), "up", &rows), Some(Some((0, 0))));
        assert_eq!(step_pick(Some((0, 0)), "end", &rows), Some(Some((0, 1))));
        assert_eq!(step_pick(Some((0, 1)), "escape", &rows), Some(None));
        assert_eq!(step_pick(Some((0, 1)), "x", &rows), None);
        assert_eq!(step_pick(None, "right", &[]), None);
    }

    #[test]
    fn the_readout_lists_layers_and_the_total() {
        let rows = rows();
        let series: Vec<SharedString> = vec!["Oats".into(), "Rye".into()];
        assert_eq!(
            bar_readout(&rows[0], &rows[0].bars[0], &series, "kg"),
            "Back, a: Oats 1, Rye 2, total 3 kg"
        );
        assert_eq!(
            bar_readout(&rows[1], &rows[1].bars[0], &series, ""),
            "Front, a: Oats 4, Rye 1, total 5"
        );
    }
}
