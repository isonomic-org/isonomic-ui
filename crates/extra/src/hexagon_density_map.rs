// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Hexagon density map: many points binned into hexagons, split by a line.
//!
//! Points fall into a grid of hexagons; the more points, the deeper the
//! colour. A threshold line (any slope) splits the plot: hexagons whose
//! centre lies above it take one colour ramp, those below another, so two
//! measures read at once: where points crowd, and which side of the line
//! they are on. The legend shows both ramps and how many points fall on each
//! side. Pointing at a hexagon, or stepping with the arrow keys, outlines it
//! and reads its place, count and side.

use std::{cell::Cell, rc::Rc, time::Duration};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::{Presence, TestSupportExt as _, Transition};
use isonomic_ui::component::{ActiveTheme as _, Icon, StyledExt as _, TextRole as _, h_flex, look};
use isonomic_ui::{
    App, ElementId, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent, MouseMoveEvent,
    ParentElement as _, PathBuilder, Pixels, Point, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, canvas, div, point,
    prelude::FluentBuilder as _, px,
};

use crate::common::{panel, shell, shell_header};

const SQRT3: f32 = 1.732_050_8;
/// Room left of the plot for the value labels, and under it for the others.
const LEFT: f32 = 40.;
const BOTTOM: f32 = 34.;

/// Which side of the threshold line a hexagon's centre is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    /// On or above the line.
    Above,
    /// Below it.
    Below,
}

/// One hexagon of the grid with the points in it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HexCell {
    /// Its axial column.
    pub q: i32,
    /// Its axial row.
    pub r: i32,
    /// Its centre in plot pixels.
    pub center: Point<f32>,
    /// How many points fall in it.
    pub count: usize,
    /// Which side of the line its centre is on.
    pub side: Side,
}

/// The centre of hexagon (`q`, `r`) of a pointy-top grid with this radius.
pub fn hex_center(q: i32, r: i32, radius: f32) -> Point<f32> {
    point(
        radius * SQRT3 * (q as f32 + r as f32 / 2.),
        radius * 1.5 * r as f32,
    )
}

/// The hexagon a pixel falls in.
pub fn hex_at(x: f32, y: f32, radius: f32) -> (i32, i32) {
    let q = (SQRT3 / 3. * x - y / 3.) / radius;
    let r = (2. / 3. * y) / radius;
    // Round in cube coordinates so the nearest centre wins.
    let s = -q - r;
    let (mut rq, mut rr, rs) = (q.round(), r.round(), s.round());
    let (dq, dr, ds) = ((rq - q).abs(), (rr - r).abs(), (rs - s).abs());
    if dq > dr && dq > ds {
        rq = -rr - rs;
    } else if dr > ds {
        rr = -rq - rs;
    }
    (rq as i32, rr as i32)
}

/// The six corners of a pointy-top hexagon.
pub fn hex_corners(center: Point<f32>, radius: f32) -> [Point<f32>; 6] {
    std::array::from_fn(|k| {
        let angle = (60. * k as f32 - 30.).to_radians();
        point(
            center.x + radius * angle.cos(),
            center.y + radius * angle.sin(),
        )
    })
}

/// The plot's value ranges and pixel size, to map between the two.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    /// The x range, left to right.
    pub x: (f64, f64),
    /// The y range, bottom to top.
    pub y: (f64, f64),
    /// The plot's size in pixels.
    pub width: f32,
    pub height: f32,
}

impl Frame {
    /// Where a value pair lands, in plot pixels from the top left.
    pub fn to_pixel(&self, x: f64, y: f64) -> Point<f32> {
        let span = |(a, b): (f64, f64)| if b > a { b - a } else { 1. };
        point(
            ((x - self.x.0) / span(self.x)) as f32 * self.width,
            (1. - (y - self.y.0) / span(self.y)) as f32 * self.height,
        )
    }

    /// The value pair a plot pixel stands for.
    pub fn to_value(&self, p: Point<f32>) -> (f64, f64) {
        let span = |(a, b): (f64, f64)| if b > a { b - a } else { 1. };
        (
            self.x.0 + f64::from(p.x / self.width.max(1.)) * span(self.x),
            self.y.0 + f64::from(1. - p.y / self.height.max(1.)) * span(self.y),
        )
    }
}

/// The ranges that hold every point with a little room around them.
pub fn fit_ranges(points: &[(f64, f64)]) -> ((f64, f64), (f64, f64)) {
    if points.is_empty() {
        return ((0., 1.), (0., 1.));
    }
    let fold = |pick: fn(&(f64, f64)) -> f64| {
        points
            .iter()
            .map(pick)
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                (lo.min(v), hi.max(v))
            })
    };
    let pad = |(lo, hi): (f64, f64)| {
        let room = if hi > lo { (hi - lo) * 0.04 } else { 1. };
        (lo - room, hi + room)
    };
    (pad(fold(|p| p.0)), pad(fold(|p| p.1)))
}

/// The side of the line `y = slope * x + intercept` a value pair is on.
pub fn side_of(x: f64, y: f64, slope: f64, intercept: f64) -> Side {
    if y >= slope * x + intercept {
        Side::Above
    } else {
        Side::Below
    }
}

/// Bin points into hexagons of `radius` pixels inside `frame`, each sided by
/// its centre against the line. Cells come out row by row, left to right.
pub fn bin(points: &[(f64, f64)], frame: &Frame, radius: f32, line: (f64, f64)) -> Vec<HexCell> {
    let mut counts: std::collections::BTreeMap<(i32, i32), usize> = Default::default();
    for &(x, y) in points {
        let p = frame.to_pixel(x, y);
        let (q, r) = hex_at(p.x, p.y, radius);
        *counts.entry((r, q)).or_default() += 1;
    }
    counts
        .into_iter()
        .map(|((r, q), count)| {
            let center = hex_center(q, r, radius);
            let (x, y) = frame.to_value(center);
            HexCell {
                q,
                r,
                center,
                count,
                side: side_of(x, y, line.0, line.1),
            }
        })
        .collect()
}

/// How deep a cell's colour is, 0.2 to 1, by its count against the busiest.
pub fn depth(count: usize, peak: usize) -> f32 {
    if peak == 0 {
        return 0.2;
    }
    0.2 + 0.8 * (count as f32 / peak as f32).sqrt().min(1.)
}

/// Where a key moves the pick among `cells`: `Some(next)` for a key the map
/// takes (`next` is `None` to clear it), `None` for any other key. Left and
/// right walk the cells in reading order; up and down jump to the nearest
/// cell in the row above or below.
pub fn step_hex(cells: &[HexCell], current: Option<usize>, key: &str) -> Option<Option<usize>> {
    if cells.is_empty() {
        return None;
    }
    let last = cells.len() - 1;
    let Some(at) = current else {
        return match key {
            "left" | "right" | "up" | "down" | "home" | "end" => Some(Some(0)),
            _ => None,
        };
    };
    let vertical = |down: bool| {
        let here = cells[at];
        cells
            .iter()
            .enumerate()
            .filter(|(_, cell)| {
                if down {
                    cell.r > here.r
                } else {
                    cell.r < here.r
                }
            })
            .min_by(|(_, a), (_, b)| {
                let cost = |cell: &HexCell| {
                    (cell.r - here.r).abs() as f32 * 1000. + (cell.center.x - here.center.x).abs()
                };
                cost(a).total_cmp(&cost(b))
            })
            .map_or(at, |(ix, _)| ix)
    };
    Some(Some(match key {
        "right" => (at + 1).min(last),
        "left" => at.saturating_sub(1),
        "down" => vertical(true),
        "up" => vertical(false),
        "home" => 0,
        "end" => last,
        "escape" => return Some(None),
        _ => return None,
    }))
}

type Format = Rc<dyn Fn(f64) -> String>;
type SelectHandler = Rc<dyn Fn(&HexCell, &mut Window, &mut App)>;

/// Points binned into hexagons, coloured by density on either side of a line.
#[derive(IntoElement)]
pub struct HexagonDensityMap {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    subtitle: Option<SharedString>,
    points: Vec<(f64, f64)>,
    x_name: SharedString,
    y_name: SharedString,
    line: (f64, f64),
    line_label: SharedString,
    above: SharedString,
    below: SharedString,
    radius: f32,
    height: f32,
    format: Format,
    on_select: Option<SelectHandler>,
}

impl HexagonDensityMap {
    /// A map of `(x, y)` points, split by a flat line at zero until
    /// [`threshold`](Self::threshold) says otherwise.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        points: Vec<(f64, f64)>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            subtitle: None,
            points,
            x_name: "x".into(),
            y_name: "y".into(),
            line: (0., 0.),
            line_label: "threshold".into(),
            above: "Above".into(),
            below: "Below".into(),
            radius: 11.,
            height: 260.,
            format: Rc::new(|value| {
                let text = format!("{value:.1}");
                text.trim_end_matches('0').trim_end_matches('.').to_string()
            }),
            on_select: None,
        }
    }

    /// A quieter line under the title.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// What the two axes measure.
    pub fn axes(mut self, x: impl Into<SharedString>, y: impl Into<SharedString>) -> Self {
        self.x_name = x.into();
        self.y_name = y.into();
        self
    }

    /// The line `y = slope * x + intercept` that splits the plot, and its name.
    pub fn threshold(mut self, slope: f64, intercept: f64, label: impl Into<SharedString>) -> Self {
        self.line = (slope, intercept);
        self.line_label = label.into();
        self
    }

    /// What the hexagons above and below the line are called.
    pub fn sides(mut self, above: impl Into<SharedString>, below: impl Into<SharedString>) -> Self {
        self.above = above.into();
        self.below = below.into();
        self
    }

    /// The hexagons' radius. 11px by default.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = f32::from(radius.into()).clamp(5., 40.);
        self
    }

    /// The plot's height. 260px by default.
    pub fn height(mut self, height: impl Into<Pixels>) -> Self {
        self.height = f32::from(height.into()).max(80.);
        self
    }

    /// How a value is written on the axes and in the readout.
    pub fn format(mut self, format: impl Fn(f64) -> String + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }

    /// Run with the hexagon the reader picks.
    pub fn on_select(
        mut self,
        handler: impl Fn(&HexCell, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Styled for HexagonDensityMap {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// The words for a picked hexagon.
fn readout(
    cell: &HexCell,
    frame: &Frame,
    names: (&str, &str),
    sides: (&str, &str),
    format: &Format,
) -> String {
    let (x, y) = frame.to_value(cell.center);
    format!(
        "{} {}, {} {}: {} {}, {}",
        names.0,
        format(x),
        names.1,
        format(y),
        cell.count,
        if cell.count == 1 { "point" } else { "points" },
        match cell.side {
            Side::Above => sides.0,
            Side::Below => sides.1,
        }
        .to_lowercase()
    )
}

impl RenderOnce for HexagonDensityMap {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let part = |name: &str| ElementId::NamedChild(self.id.clone().into(), name.into());
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let ink = theme.foreground;
        let mono = theme.mono_font_family.clone();
        let above_color = theme.chart_4;
        let below_color = theme.chart_1;
        let grid = theme.border;

        // The plot's width is measured while painting and used next frame.
        let measured = window
            .use_keyed_state(part("width"), cx, |_, _| Rc::new(Cell::new(480_f32)))
            .read(cx)
            .clone();
        let origin = window
            .use_keyed_state(part("origin"), cx, |_, _| {
                Rc::new(Cell::new(point(px(0.), px(0.))))
            })
            .read(cx)
            .clone();
        let (xr, yr) = fit_ranges(&self.points);
        let frame = Frame {
            x: xr,
            y: yr,
            width: measured.get(),
            height: self.height,
        };
        let radius = self.radius;
        let cells = bin(&self.points, &frame, radius, self.line);
        let peak = cells.iter().map(|cell| cell.count).max().unwrap_or(0);
        let count_side = |side: Side| {
            cells
                .iter()
                .filter(|cell| cell.side == side)
                .map(|cell| cell.count)
                .sum::<usize>()
        };
        let (n_above, n_below) = (count_side(Side::Above), count_side(Side::Below));

        let pick_state = window.use_keyed_state(part("pick"), cx, |_, _| None::<usize>);
        let pick = pick_state.read(cx).filter(|ix| *ix < cells.len());
        let shown_pick = window.use_keyed_state(part("shown"), cx, |_, _| None::<usize>);
        if pick.is_some() && *shown_pick.read(cx) != pick {
            shown_pick.update(cx, |state, _| *state = pick);
        }
        // The outline fades with the hover and stays on the last cell while
        // it fades out.
        let outline = look::hover_fade(part("outline"), pick.is_some(), window, cx);
        let outlined = shown_pick.read(cx).filter(|ix| *ix < cells.len());
        let appear = Presence::new(part("appear"), true)
            .transition(Transition::new(Duration::from_millis(400)))
            .sample(window, cx)
            .progress;

        let paint_cells = cells.clone();
        let line = self.line;
        let paint_measured = measured;
        let paint_origin = origin.clone();
        let plot = canvas(
            move |bounds, window, _| {
                let width = f32::from(bounds.size.width);
                if (paint_measured.get() - width).abs() > 0.5 {
                    paint_measured.set(width);
                    window.refresh();
                }
                paint_origin.set(bounds.origin);
            },
            move |bounds, _, window, _| {
                let to = |p: Point<f32>| point(bounds.left() + px(p.x), bounds.top() + px(p.y));
                let stroke = |window: &mut Window,
                              from: Point<f32>,
                              upto: Point<f32>,
                              wide: f32,
                              dash: bool,
                              color: Hsla| {
                    let mut path = PathBuilder::stroke(px(wide));
                    if dash {
                        path = path.dash_array(&[px(5.), px(4.)]);
                    }
                    path.move_to(to(from));
                    path.line_to(to(upto));
                    if let Ok(path) = path.build() {
                        window.paint_path(path, color);
                    }
                };
                let (w, h) = (frame.width, frame.height);
                for k in 0..=4 {
                    let y = h * k as f32 / 4.;
                    stroke(window, point(0., y), point(w, y), 1., false, grid);
                }
                window.with_content_mask(Some(isonomic_ui::ContentMask { bounds }), |window| {
                    for (ix, cell) in paint_cells.iter().enumerate() {
                        let base = match cell.side {
                            Side::Above => above_color,
                            Side::Below => below_color,
                        };
                        let shown = ((appear - 0.3 * ix as f32 / paint_cells.len().max(1) as f32)
                            / 0.7)
                            .clamp(0., 1.);
                        let color = base.opacity(depth(cell.count, peak) * shown);
                        let corners = hex_corners(cell.center, radius - 1.);
                        let shape: Vec<_> = corners.iter().map(|p| to(*p)).collect();
                        let mut path = PathBuilder::fill();
                        path.add_polygon(&shape, true);
                        if let Ok(path) = path.build() {
                            window.paint_path(path, color);
                        }
                    }
                    // The threshold line, across the whole plot.
                    let left = frame.to_pixel(frame.x.0, line.0 * frame.x.0 + line.1);
                    let right = frame.to_pixel(frame.x.1, line.0 * frame.x.1 + line.1);
                    stroke(window, left, right, 1.5, true, ink.opacity(0.8));
                    if let Some(cell) = outlined.and_then(|ix| paint_cells.get(ix)) {
                        let corners = hex_corners(cell.center, radius);
                        for k in 0..6 {
                            stroke(
                                window,
                                corners[k],
                                corners[(k + 1) % 6],
                                2.,
                                false,
                                ink.opacity(outline),
                            );
                        }
                    }
                });
            },
        )
        .size_full();

        let format = self.format.clone();
        let names = (self.x_name.as_ref(), self.y_name.as_ref());
        let sides = (self.above.as_ref(), self.below.as_ref());
        let words = pick
            .and_then(|ix| cells.get(ix))
            .map(|cell| readout(cell, &frame, names, sides, &format));

        let hover_state = pick_state.clone();
        let hover_cells = cells.clone();
        let hover_select = self.on_select.clone();
        let on_move = move |event: &MouseMoveEvent, window: &mut Window, cx: &mut App| {
            let o = origin.get();
            let (q, r) = hex_at(
                f32::from(event.position.x - o.x),
                f32::from(event.position.y - o.y),
                radius,
            );
            let hit = hover_cells
                .iter()
                .position(|cell| cell.q == q && cell.r == r);
            if *hover_state.read(cx) != hit {
                hover_state.update(cx, |state, cx| {
                    *state = hit;
                    cx.notify();
                });
                if let (Some(select), Some(cell)) = (&hover_select, hit.map(|ix| hover_cells[ix])) {
                    select(&cell, window, cx);
                }
            }
        };
        let leave_state = pick_state.clone();
        let key_state = pick_state;
        let key_cells = cells.clone();
        let key_select = self.on_select.clone();

        let ramp = |color: Hsla| {
            h_flex().children((0..5).map(move |k| {
                div()
                    .w(px(12.))
                    .h(px(10.))
                    .bg(color.opacity(0.2 + 0.2 * k as f32))
            }))
        };
        let side_key = |name: SharedString, color: Hsla, count: usize| {
            h_flex().gap(px(6.)).child(ramp(color)).child(name).child(
                div()
                    .font_family(mono.clone())
                    .text_color(ink)
                    .child(count.to_string()),
            )
        };
        let legend = h_flex()
            .flex_wrap()
            .gap_x_4()
            .gap_y_1()
            .text_caption()
            .text_color(quiet)
            .child(side_key(self.above.clone(), above_color, n_above))
            .child(side_key(self.below.clone(), below_color, n_below))
            .child(
                h_flex()
                    .gap(px(6.))
                    .child(
                        h_flex()
                            .gap(px(2.))
                            .children((0..3).map(|_| div().w(px(5.)).h(px(1.5)).bg(ink))),
                    )
                    .child(self.line_label.clone()),
            )
            .child(div().ml_auto().child("Deeper means more points"));

        let spoken = format!(
            "{}: {} points in {} hexagons. {} {n_above}, {} {n_below}, split by {}{}",
            self.title,
            self.points.len(),
            cells.len(),
            self.above,
            self.below,
            self.line_label,
            words
                .as_ref()
                .map(|words| format!(". Picked {words}"))
                .unwrap_or_default()
        );
        let footer = words.clone().unwrap_or_else(|| {
            format!(
                "{} points in {} hexagons, {} of them {}",
                self.points.len(),
                cells.len(),
                n_above,
                self.above.to_lowercase()
            )
        });
        let axis_text = |text: String| {
            div()
                .text_size(px(10.))
                .line_height(px(16.))
                .text_color(quiet)
                .child(text)
        };

        shell(cx)
            .id(self.id.clone())
            .max_w(px(640.))
            .child(shell_header(
                Some(Icon::new(CatalogIcon::HexagonVerticalOutline)),
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
                    .child(div().mb(px(-6.)).child(axis_text(self.y_name.to_string())))
                    .child(
                        div()
                            .id(part("plot"))
                            .test_support()
                            .relative()
                            .w_full()
                            .h(px(self.height + BOTTOM))
                            .pl(px(LEFT))
                            .rounded(px(8.))
                            .role(Role::Group)
                            .aria_label(spoken)
                            .tab_index(0)
                            .focus_visible(|this| this.bg(look::ink::wash(cx)))
                            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                                let now = *key_state.read(cx);
                                let Some(next) =
                                    step_hex(&key_cells, now, event.keystroke.key.as_str())
                                else {
                                    return;
                                };
                                key_state.update(cx, |state, cx| {
                                    *state = next;
                                    cx.notify();
                                });
                                if let (Some(select), Some(cell)) =
                                    (&key_select, next.map(|ix| key_cells[ix]))
                                {
                                    select(&cell, window, cx);
                                }
                                cx.stop_propagation();
                            })
                            .child(
                                div()
                                    .id(part("hexes"))
                                    .w_full()
                                    .h(px(self.height))
                                    .on_mouse_move(on_move)
                                    .on_hover(move |over, _, cx| {
                                        if !*over && leave_state.read(cx).is_some() {
                                            leave_state.update(cx, |state, cx| {
                                                *state = None;
                                                cx.notify();
                                            });
                                        }
                                    })
                                    .child(plot),
                            )
                            // Value labels: the y range on the left, the x range below.
                            .child(
                                div()
                                    .absolute()
                                    .top(px(-6.))
                                    .left_0()
                                    .w(px(LEFT - 6.))
                                    .flex()
                                    .justify_end()
                                    .child(axis_text(format(yr.1))),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .top(px(self.height - 10.))
                                    .left_0()
                                    .w(px(LEFT - 6.))
                                    .flex()
                                    .justify_end()
                                    .child(axis_text(format(yr.0))),
                            )
                            .child(
                                h_flex()
                                    .absolute()
                                    .top(px(self.height + 2.))
                                    .left(px(LEFT))
                                    .right_0()
                                    .justify_between()
                                    .child(axis_text(format(xr.0)))
                                    .child(axis_text(self.x_name.to_string()))
                                    .child(axis_text(format(xr.1))),
                            ),
                    )
                    .child(
                        div()
                            .min_h(px(16.))
                            .text_caption()
                            .when(words.is_some(), |this| this.text_color(ink))
                            .when(words.is_none(), |this| this.text_color(quiet))
                            .child(footer),
                    ),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> Frame {
        Frame {
            x: (0., 10.),
            y: (0., 10.),
            width: 100.,
            height: 100.,
        }
    }

    #[test]
    fn a_centre_falls_in_its_own_hexagon() {
        for (q, r) in [(0, 0), (3, 1), (-2, 4), (5, -3)] {
            let c = hex_center(q, r, 10.);
            assert_eq!(hex_at(c.x, c.y, 10.), (q, r));
            // A point near the edge but inside still lands in it.
            assert_eq!(hex_at(c.x + 7., c.y, 10.), (q, r));
        }
        let corners = hex_corners(point(0., 0.), 10.);
        assert!(corners[2].x.abs() < 1e-4 && (corners[2].y - 10.).abs() < 1e-4);
    }

    #[test]
    fn values_and_pixels_map_both_ways() {
        let f = frame();
        let p = f.to_pixel(2.5, 7.5);
        assert_eq!((p.x, p.y), (25., 25.));
        assert_eq!(f.to_value(p), (2.5, 7.5));
        assert_eq!(fit_ranges(&[]), ((0., 1.), (0., 1.)));
        let ((x0, x1), _) = fit_ranges(&[(0., 0.), (100., 5.)]);
        assert_eq!((x0, x1), (-4., 104.));
    }

    #[test]
    fn the_line_splits_by_the_centre() {
        assert_eq!(side_of(1., 3., 1., 1.), Side::Above);
        assert_eq!(side_of(1., 2., 1., 1.), Side::Above);
        assert_eq!(side_of(1., 1.9, 1., 1.), Side::Below);
        let cells = bin(&[(1., 9.), (1.1, 9.), (9., 1.)], &frame(), 8., (1., 0.));
        assert_eq!(cells.len(), 2);
        assert_eq!(cells.iter().map(|c| c.count).sum::<usize>(), 3);
        // Reading order: the top-left cell first.
        assert_eq!((cells[0].count, cells[0].side), (2, Side::Above));
        assert_eq!(cells[1].side, Side::Below);
    }

    #[test]
    fn busier_cells_are_deeper() {
        assert_eq!(depth(0, 0), 0.2);
        assert_eq!(depth(9, 9), 1.);
        assert!(depth(1, 9) < depth(4, 9));
    }

    #[test]
    fn keys_walk_the_cells() {
        let cell = |q, r, x: f32| HexCell {
            q,
            r,
            center: point(x, r as f32 * 10.),
            count: 1,
            side: Side::Below,
        };
        let cells = [
            cell(0, 0, 0.),
            cell(1, 0, 20.),
            cell(0, 1, 18.),
            cell(3, 1, 60.),
        ];
        assert_eq!(step_hex(&cells, None, "right"), Some(Some(0)));
        assert_eq!(step_hex(&cells, Some(0), "right"), Some(Some(1)));
        assert_eq!(step_hex(&cells, Some(3), "right"), Some(Some(3)));
        assert_eq!(step_hex(&cells, Some(1), "down"), Some(Some(2)));
        assert_eq!(step_hex(&cells, Some(3), "up"), Some(Some(1)));
        assert_eq!(step_hex(&cells, Some(0), "up"), Some(Some(0)));
        assert_eq!(step_hex(&cells, Some(2), "escape"), Some(None));
        assert_eq!(step_hex(&cells, Some(2), "q"), None);
        assert_eq!(step_hex(&[], None, "right"), None);
    }

    #[test]
    fn the_readout_says_place_count_and_side() {
        let f = frame();
        let cell = HexCell {
            q: 0,
            r: 0,
            center: point(50., 50.),
            count: 3,
            side: Side::Above,
        };
        let format: Format = Rc::new(|v| format!("{v:.0}"));
        assert_eq!(
            readout(
                &cell,
                &f,
                ("Distance", "Time"),
                ("Late", "On time"),
                &format
            ),
            "Distance 5, Time 5: 3 points, late"
        );
    }
}
