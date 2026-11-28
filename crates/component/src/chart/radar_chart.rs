// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    f32::consts::{PI, TAU},
    hash::Hash,
    rc::Rc,
};

use gpui::{
    AnyElement, App, AvailableSpace, Background, Bounds, ElementId, Hsla, IntoElement, PathBuilder,
    Pixels, Point, SharedString, TextAlign, Window, point, px,
};
use isonomic_ui_macros::IntoPlot;
use num_traits::Zero;

use crate::{
    ActiveTheme,
    plot::{
        Plot, PlotAppear,
        label::{PlotLabel, TEXT_SIZE, Text},
        polygon,
        scale::{PlotValue, Scale, ScaleLinear},
        shape::RadialLine,
        tooltip::{Dot, Tooltip, TooltipState},
    },
};

use super::{ChartAppear, HOVER_DOT_SIZE, HOVER_HALO_SIZE, TooltipContent, caller_id};

const HALF_PI: f32 = PI / 2.;

/// The default extra gap (in pixels) between the outer grid ring and the labels.
const DEFAULT_LABEL_GAP: f32 = 10.;

/// The default number of concentric grid rings.
const DEFAULT_GRID_LEVELS: usize = 4;

/// The label of one radar dimension, returned by [`RadarChart::label`].
pub enum RadarLabel {
    /// Plain text, drawn by the plot's own text layer: honors
    /// [`RadarChart::label_color`] and supplies the tooltip title.
    Text(SharedString),
    /// A custom element, measured at its natural size and anchored like a text
    /// label. [`RadarChart::label_color`] does not apply, and it supplies no
    /// tooltip title.
    Element(AnyElement),
}

impl From<&'static str> for RadarLabel {
    fn from(text: &'static str) -> Self {
        Self::Text(text.into())
    }
}

impl From<String> for RadarLabel {
    fn from(text: String) -> Self {
        Self::Text(text.into())
    }
}

impl From<SharedString> for RadarLabel {
    fn from(text: SharedString) -> Self {
        Self::Text(text)
    }
}

impl From<AnyElement> for RadarLabel {
    fn from(element: AnyElement) -> Self {
        Self::Element(element)
    }
}

/// Where a value sits on spoke `ix` of `n`, `radius` from the center, with
/// 12 o'clock first and the spokes running clockwise.
pub fn spoke_point(center: Point<f32>, radius: f32, ix: usize, n: usize) -> Point<f32> {
    let angle = ix as f32 * TAU / n.max(1) as f32 - HALF_PI;
    point(
        center.x + radius * angle.cos(),
        center.y + radius * angle.sin(),
    )
}

/// How far to nudge each of a spoke's member dots across the spoke so that
/// dots closer than `gap` along it sit side by side instead of on top of
/// each other. Dots on their own stay on the spoke (0); a cluster spreads
/// evenly either side of it.
pub fn dot_nudges(radii: &[f32], gap: f32) -> Vec<f32> {
    let mut order: Vec<usize> = (0..radii.len()).collect();
    order.sort_by(|a, b| radii[*a].total_cmp(&radii[*b]));
    let mut nudges = vec![0.; radii.len()];
    let mut start = 0;
    while start < order.len() {
        let mut end = start + 1;
        while end < order.len() && radii[order[end]] - radii[order[end - 1]] < gap {
            end += 1;
        }
        let count = end - start;
        for (k, ix) in order[start..end].iter().enumerate() {
            nudges[*ix] = (k as f32 - (count - 1) as f32 / 2.) * gap;
        }
        start = end;
    }
    nudges
}

/// A radar (spider) chart.
///
/// Each datum is one dimension (a spoke), placed clockwise around the center
/// starting at 12 o'clock. Add one series per [`RadarChart::value`] call; each
/// series is drawn as a closed polygon connecting its values on every spoke.
///
/// A [`target`](Self::target) draws a dashed outline of where each dimension
/// should reach, and [`member_dots`](Self::member_dots) puts one dot per
/// member on every spoke, such as each person behind a team's average.
#[derive(IntoPlot)]
pub struct RadarChart<T, Y>
where
    T: 'static,
    Y: PlotValue,
{
    data: Vec<T>,
    values: Vec<Rc<dyn Fn(&T) -> Y>>,
    strokes: Vec<Hsla>,
    fills: Vec<Background>,
    names: Vec<SharedString>,
    tooltip_content: TooltipContent<T>,
    label: Option<Rc<dyn Fn(&T) -> RadarLabel + 'static>>,
    /// The text of each dimension's label, resolved once per frame in `prepaint`;
    /// element labels leave `None`. Read by `paint` (to draw them) and `tooltip`
    /// (as the title), so the label closure runs once per dimension per frame.
    label_texts: Vec<Option<SharedString>>,
    label_color: Option<Hsla>,
    label_gap: f32,
    max_value: Option<Y>,
    outer_radius: f32,
    grid: bool,
    grid_levels: usize,
    dot: bool,
    id: ElementId,
    interactive: bool,
    appear: ChartAppear,
    target: Option<Rc<dyn Fn(&T) -> Y>>,
    target_stroke: Option<Hsla>,
    members: Option<Rc<dyn Fn(&T) -> Vec<Y>>>,
    member_colors: Vec<Hsla>,
}

impl<T, Y> RadarChart<T, Y>
where
    Y: PlotValue,
{
    #[track_caller]
    pub fn new<I>(data: I) -> Self
    where
        I: IntoIterator<Item = T>,
    {
        Self {
            data: data.into_iter().collect(),
            values: vec![],
            strokes: vec![],
            fills: vec![],
            names: vec![],
            tooltip_content: TooltipContent::default(),
            label: None,
            label_texts: vec![],
            label_color: None,
            label_gap: DEFAULT_LABEL_GAP,
            max_value: None,
            outer_radius: 0.,
            grid: true,
            grid_levels: DEFAULT_GRID_LEVELS,
            dot: false,
            id: caller_id(),
            interactive: true,
            appear: ChartAppear::default(),
            target: None,
            target_stroke: None,
            members: None,
            member_colors: vec![],
        }
    }

    /// Draw a dashed outline through the value each dimension should reach.
    ///
    /// It counts toward the outer ring's default value, has no fill, and is
    /// drawn over the series.
    pub fn target(mut self, target: impl Fn(&T) -> Y + 'static) -> Self {
        self.target = Some(Rc::new(target));
        self
    }

    /// Set the target outline's color; the foreground at 60% by default.
    pub fn target_stroke(mut self, stroke: impl Into<Hsla>) -> Self {
        self.target_stroke = Some(stroke.into());
        self
    }

    /// Put one dot per member on every spoke, at that member's value for the
    /// dimension. Return the members in the same order for every datum, so a
    /// member keeps its color around the chart.
    ///
    /// Dots that would overlap sit side by side across the spoke.
    pub fn member_dots(mut self, members: impl Fn(&T) -> Vec<Y> + 'static) -> Self {
        self.members = Some(Rc::new(members));
        self
    }

    /// Color the member dots by member, in order; the muted foreground for
    /// all by default.
    pub fn member_colors(mut self, colors: impl IntoIterator<Item = impl Into<Hsla>>) -> Self {
        self.member_colors = colors.into_iter().map(Into::into).collect();
        self
    }

    /// Name this chart's [`ElementId`], replacing the default taken from the
    /// construction site.
    ///
    /// Pass one where a single construction site renders several of these
    /// charts as siblings: they share the default id, and with it one hover
    /// state and one path cache. The id must be unique among those siblings.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    /// Turn this chart's interactive layer on or off. On by default.
    ///
    /// The layer is the hitbox under the cursor and what it drives: a dot per
    /// series marks the hovered dimension, and a tooltip shows a row each. Turn
    /// it off for a chart that only decorates, or one an element above it wants
    /// the cursor for: without a hitbox it neither answers the mouse nor takes
    /// the hover from what sits over it.
    pub fn interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        self
    }

    /// Draw the data in the first time this chart is painted. On by default.
    ///
    /// The theme sets how long it takes, and the system's reduced-motion
    /// setting skips it. Turn it off for a chart that is painted again and
    /// again as it scrolls in and out of view, such as one in each row of a
    /// long list, where it would draw in every time.
    pub fn appear(mut self, appear: bool) -> Self {
        self.appear.set_enabled(appear);
        self
    }

    /// Draw the data in again whenever `key` changes, such as the symbol or
    /// period a chart shows.
    ///
    /// Without one the data draws in once, and later data paints in place.
    pub fn appear_key(mut self, key: impl Hash) -> Self {
        self.appear.set_key(key);
        self
    }

    /// Set the name of the most recently added series, shown in its tooltip row.
    ///
    /// Call after the matching [`RadarChart::value`]
    /// (e.g. `.value(..).stroke(..).name("Desktop")`).
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.names.push(name.into());
        self
    }

    /// Set the hover tooltip's title for a datum, instead of its dimension label.
    pub fn tooltip_title(mut self, title: impl Fn(&T) -> SharedString + 'static) -> Self {
        self.tooltip_content.set_title(title);
        self
    }

    /// Set the text of each tooltip row's value; the raw number by default.
    ///
    /// The closure receives the datum, the row's index (the series' index in the order `value`
    /// added them) and the value the row reads.
    pub fn tooltip_value(
        mut self,
        value: impl Fn(&T, usize, f64) -> SharedString + 'static,
    ) -> Self {
        self.tooltip_content.set_value(value);
        self
    }

    /// Color each tooltip row's value, such as green or red by its sign; the
    /// tooltip's text color by default.
    ///
    /// The closure receives the same arguments as
    /// [`tooltip_value`](Self::tooltip_value).
    pub fn tooltip_value_color<H>(mut self, color: impl Fn(&T, usize, f64) -> H + 'static) -> Self
    where
        H: Into<Hsla>,
    {
        self.tooltip_content.set_value_color(color);
        self
    }

    /// Draw the tooltip box's content for a datum yourself, in place of the
    /// title and rows, for a layout they cannot express such as a table.
    ///
    /// The dots and where the box sits stay the chart's, and
    /// [`tooltip_title`](Self::tooltip_title), [`tooltip_value`](Self::tooltip_value)
    /// and [`tooltip_value_color`](Self::tooltip_value_color) no longer apply.
    pub fn tooltip_content<E>(
        mut self,
        content: impl Fn(&T, &mut Window, &mut App) -> E + 'static,
    ) -> Self
    where
        E: IntoElement,
    {
        self.tooltip_content.set_content(content);
        self
    }

    /// Add a series to the radar chart.
    ///
    /// Call multiple times to overlay multiple series, each paired with the
    /// matching [`RadarChart::stroke`] and [`RadarChart::fill`] calls.
    pub fn value(mut self, value: impl Fn(&T) -> Y + 'static) -> Self {
        self.values.push(Rc::new(value));
        self
    }

    /// Set the stroke color of the most recently added series.
    ///
    /// Defaults to the theme chart colors, cycled per series.
    pub fn stroke(mut self, stroke: impl Into<Hsla>) -> Self {
        self.strokes.push(stroke.into());
        self
    }

    /// Set the fill color of the most recently added series.
    ///
    /// Defaults to the series stroke color with 0.3 opacity.
    pub fn fill(mut self, fill: impl Into<Background>) -> Self {
        self.fills.push(fill.into());
        self
    }

    /// Set the label for each dimension, shown outside the outer ring.
    ///
    /// Return a string for a plain text label, or `element.into_any_element()`
    /// for a custom one; see [`RadarLabel`] for how the two differ.
    ///
    /// ```ignore
    /// RadarChart::new(data).label(|d| d.month.clone())
    ///
    /// RadarChart::new(data).label(|d| {
    ///     v_flex()
    ///         .items_center()
    ///         .child(Icon::new(IconName::Star).xsmall())
    ///         .child(d.month.clone())
    ///         .into_any_element()
    /// })
    /// ```
    pub fn label<L>(mut self, label: impl Fn(&T) -> L + 'static) -> Self
    where
        L: Into<RadarLabel> + 'static,
    {
        self.label = Some(Rc::new(move |d| label(d).into()));
        self
    }

    /// Set the text label color (defaults to `cx.theme().muted_foreground`).
    ///
    /// Element labels style themselves; this does not apply to them.
    pub fn label_color(mut self, color: impl Into<Hsla>) -> Self {
        self.label_color = Some(color.into());
        self
    }

    /// Set the extra gap between the outer ring and the labels
    /// (defaults to 10px).
    pub fn label_gap(mut self, gap: f32) -> Self {
        self.label_gap = gap;
        self
    }

    /// Set the value at the outer ring.
    ///
    /// Defaults to the maximum value across all series.
    pub fn max_value(mut self, max_value: Y) -> Self {
        self.max_value = Some(max_value);
        self
    }

    /// Set the outer radius of the radar chart.
    ///
    /// Defaults to 40% of the bounds height.
    pub fn outer_radius(mut self, outer_radius: f32) -> Self {
        self.outer_radius = outer_radius;
        self
    }

    /// Show or hide the grid rings and spokes.
    ///
    /// Default is true.
    pub fn grid(mut self, grid: bool) -> Self {
        self.grid = grid;
        self
    }

    /// Set the number of concentric grid rings (defaults to 4).
    pub fn grid_levels(mut self, grid_levels: usize) -> Self {
        self.grid_levels = grid_levels.max(1);
        self
    }

    /// Show dots on the vertices of each series.
    pub fn dot(mut self) -> Self {
        self.dot = true;
        self
    }

    /// The stroke color of the series at the given index, set or default.
    ///
    /// Defaults to the theme chart colors, cycled per series.
    fn series_stroke(&self, ix: usize, cx: &App) -> Hsla {
        self.series_stroke_from(&Self::palette(cx), ix)
    }

    /// The theme chart colors the series cycle through by default.
    fn palette(cx: &App) -> [Hsla; 5] {
        [
            cx.theme().chart_1,
            cx.theme().chart_2,
            cx.theme().chart_3,
            cx.theme().chart_4,
            cx.theme().chart_5,
        ]
    }

    /// The stroke color of the series at the given index, set or from `palette`.
    fn series_stroke_from(&self, palette: &[Hsla; 5], ix: usize) -> Hsla {
        self.strokes
            .get(ix)
            .copied()
            .unwrap_or(palette[ix % palette.len()])
    }

    /// The resolved outer radius for the given bounds.
    fn resolve_outer_radius(&self, bounds: &Bounds<Pixels>) -> f32 {
        if self.outer_radius.is_zero() {
            bounds.size.height.as_f32() * 0.4
        } else {
            self.outer_radius
        }
    }

    /// Where the label of dimension `ix` attaches, in bounds-relative coordinates,
    /// plus the outward radial direction at that dimension (a unit vector).
    ///
    /// The anchor sits on the label ring at the dimension's angle; callers offset
    /// their own box from it along `direction`. Shared by `prepaint` and `paint`
    /// so element and text labels land in the same place.
    fn label_anchor(
        &self,
        ix: usize,
        outer_radius: f32,
        bounds: &Bounds<Pixels>,
    ) -> (Point<f32>, Point<f32>) {
        let label_radius = outer_radius + self.label_gap;
        let angle = ix as f32 * TAU / self.data.len() as f32 - HALF_PI;
        let direction = point(angle.cos(), angle.sin());

        let anchor = point(
            bounds.size.width.as_f32() / 2. + label_radius * direction.x,
            bounds.size.height.as_f32() / 2. + label_radius * direction.y,
        );

        (anchor, direction)
    }

    /// Build the radius scale from the center to the outer ring.
    ///
    /// The domain includes zero so non-negative data starts at the center.
    /// Shared by `paint` and `tooltip_state` so the two stay in sync.
    fn scale(&self, outer_radius: f32) -> ScaleLinear<Y> {
        let domain = if let Some(max_value) = self.max_value {
            vec![Y::zero(), max_value]
        } else {
            self.data
                .iter()
                .flat_map(|d| {
                    self.values
                        .iter()
                        .map(|value_fn| value_fn(d))
                        .chain(self.target.as_ref().map(|target| target(d)))
                        .chain(self.members.as_ref().map_or(vec![], |members| members(d)))
                })
                .chain(Some(Y::zero()))
                .collect()
        };

        ScaleLinear::new(domain, [0., outer_radius])
    }

    /// Map a cursor position to the nearest spoke index, or `None` when the
    /// cursor is outside the radar.
    fn hovered_index(&self, position: Point<Pixels>, bounds: Bounds<Pixels>) -> Option<usize> {
        let n = self.data.len();
        if n == 0 {
            return None;
        }

        let outer_radius = self.resolve_outer_radius(&bounds);
        let dx = position.x.as_f32() - bounds.size.width.as_f32() / 2.;
        let dy = position.y.as_f32() - bounds.size.height.as_f32() / 2.;
        if dx.hypot(dy) > outer_radius + self.label_gap {
            return None;
        }

        // Screen angle -> chart angle (0 at 12 o'clock, clockwise).
        let angle = (dy.atan2(dx) + HALF_PI).rem_euclid(TAU);
        Some((angle * n as f32 / TAU).round() as usize % n)
    }
}

impl<T, Y> Plot for RadarChart<T, Y>
where
    Y: PlotValue,
{
    /// Resolve every dimension's label, keeping the text ones for `paint` and
    /// laying out the element ones here (measuring is illegal in `paint`).
    fn prepaint(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<AnyElement> {
        self.label_texts.clear();

        // Same guard as `paint`: without a series nothing is drawn at all.
        let n = self.data.len();
        if n == 0 || self.values.is_empty() {
            return vec![];
        }
        let Some(label_fn) = self.label.clone() else {
            return vec![];
        };

        let outer_radius = self.resolve_outer_radius(&bounds);
        let mut texts = Vec::with_capacity(n);
        let mut elements = vec![];

        for (ix, d) in self.data.iter().enumerate() {
            match label_fn(d) {
                RadarLabel::Text(text) => texts.push(Some(text)),
                RadarLabel::Element(mut element) => {
                    texts.push(None);

                    // Only `AvailableSpace::Definite` makes text wrap, so this
                    // measures the label at its natural, unwrapped size.
                    let size = element.layout_as_root(AvailableSpace::min_size(), window, cx);
                    let (anchor, direction) = self.label_anchor(ix, outer_radius, &bounds);

                    // Push the box radially outward until its inner edge meets the
                    // anchor, so a tall label clears the ring instead of straddling
                    // it. Reduces to "centered on the anchor" across that axis when
                    // the dimension is square-on to it.
                    let origin = bounds.origin
                        + point(
                            px(anchor.x + (direction.x - 1.) * size.width.as_f32() / 2.),
                            px(anchor.y + (direction.y - 1.) * size.height.as_f32() / 2.),
                        );

                    element.prepaint_at(origin, window, cx);
                    elements.push(element);
                }
            }
        }

        self.label_texts = texts;

        elements
    }

    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let n = self.data.len();
        if n == 0 || self.values.is_empty() {
            return;
        }

        let outer_radius = self.resolve_outer_radius(&bounds);
        let angle_step = TAU / n as f32;
        let center_x = bounds.size.width.as_f32() / 2.;
        let center_y = bounds.size.height.as_f32() / 2.;
        let scale = self.scale(outer_radius);

        // Draw grid rings and spokes
        if self.grid {
            let stroke = cx.theme().chart_grid;

            for level in 1..=self.grid_levels {
                let radius = outer_radius * level as f32 / self.grid_levels as f32;
                RadialLine::new()
                    .data(0..n)
                    .angle(move |_, i| Some(i as f32 * angle_step))
                    .radius(move |_, _| Some(radius))
                    .closed()
                    .stroke(stroke)
                    .paint(&bounds, window);
            }

            for i in 0..n {
                let angle = i as f32 * angle_step - HALF_PI;
                let points = [
                    point(center_x, center_y),
                    point(
                        center_x + outer_radius * angle.cos(),
                        center_y + outer_radius * angle.sin(),
                    ),
                ];
                if let Some(path) = polygon(&points, &bounds) {
                    window.paint_path(path, stroke);
                }
            }
        }

        // Draw series. They grow out of the center as the chart appears.
        let appear = self.appear.get().progress();
        for (i, value_fn) in self.values.iter().enumerate() {
            let stroke = self.series_stroke(i, cx);
            let fill = self
                .fills
                .get(i)
                .copied()
                .unwrap_or_else(|| stroke.opacity(0.3).into());

            let scale = scale.clone();
            let value_fn = value_fn.clone();
            let mut line = RadialLine::new()
                .data(&self.data)
                .angle(move |_, i| Some(i as f32 * angle_step))
                .radius(move |d, _| scale.tick(&value_fn(d)).map(|r| r * appear))
                .closed()
                .fill(fill)
                .stroke(stroke)
                .stroke_width(2.);
            if self.dot {
                line = line.dot().dot_size(8.).dot_fill(stroke);
            }
            line.paint(&bounds, window);
        }

        // The target outline, dashed and unfilled, over the series.
        let center = point(center_x, center_y);
        if let Some(target) = &self.target {
            let stroke = self
                .target_stroke
                .unwrap_or(cx.theme().foreground.opacity(0.6));
            let points: Vec<Point<f32>> = self
                .data
                .iter()
                .enumerate()
                .filter_map(|(ix, d)| {
                    let radius = scale.tick(&target(d))? * appear;
                    Some(spoke_point(center, radius, ix, n))
                })
                .collect();
            if points.len() == n && n > 1 {
                let at = |p: Point<f32>| bounds.origin + point(px(p.x), px(p.y));
                let mut path = PathBuilder::stroke(px(1.5)).dash_array(&[px(4.), px(3.)]);
                path.move_to(at(points[0]));
                for p in &points[1..] {
                    path.line_to(at(*p));
                }
                path.close();
                if let Ok(path) = path.build() {
                    window.paint_path(path, stroke);
                }
            }
        }

        // One dot per member on every spoke, ringed in the background so they
        // read over the series' fill.
        if let Some(members) = &self.members {
            let ring = cx.theme().background;
            let fallback = cx.theme().muted_foreground;
            const DOT: f32 = 6.;
            for (ix, d) in self.data.iter().enumerate() {
                let radii: Vec<f32> = members(d)
                    .iter()
                    .map(|v| scale.tick(v).unwrap_or(0.) * appear)
                    .collect();
                let nudges = dot_nudges(&radii, DOT + 1.);
                let angle = ix as f32 * angle_step - HALF_PI;
                // Across the spoke: the spoke's direction turned a quarter.
                let across = point(-angle.sin(), angle.cos());
                for (m, (radius, nudge)) in radii.iter().zip(&nudges).enumerate() {
                    let on = spoke_point(center, *radius, ix, n);
                    let c = point(on.x + across.x * nudge, on.y + across.y * nudge);
                    let color = self.member_colors.get(m).copied().unwrap_or(fallback);
                    for (size, fill) in [(DOT + 2., ring), (DOT, color)] {
                        let ring: Vec<Point<Pixels>> = (0..14)
                            .map(|step| {
                                let a = step as f32 / 14. * TAU;
                                bounds.origin
                                    + point(
                                        px(c.x + size / 2. * a.cos()),
                                        px(c.y + size / 2. * a.sin()),
                                    )
                            })
                            .collect();
                        let mut dot = PathBuilder::fill();
                        dot.add_polygon(&ring, true);
                        if let Ok(dot) = dot.build() {
                            window.paint_path(dot, fill);
                        }
                    }
                }
            }
        }

        // Draw the text labels outside the outer ring; `prepaint` resolved them and
        // already placed the element ones.
        let label_color = self.label_color.unwrap_or(cx.theme().muted_foreground);
        let labels = self
            .label_texts
            .iter()
            .enumerate()
            .filter_map(|(ix, text)| {
                let text = text.clone()?;
                let (anchor, direction) = self.label_anchor(ix, outer_radius, &bounds);

                // Labels on the right are left-aligned, on the left right-aligned,
                // and near the vertical axis centered. `direction` is a unit vector,
                // so the epsilon only absorbs float noise at 12 and 6 o'clock.
                let align = if direction.x > 1e-3 {
                    TextAlign::Left
                } else if direction.x < -1e-3 {
                    TextAlign::Right
                } else {
                    TextAlign::Center
                };

                Some(
                    Text::new(
                        text,
                        point(px(anchor.x), px(anchor.y - TEXT_SIZE / 2.)),
                        label_color,
                    )
                    .align(align),
                )
            });

        PlotLabel::new(labels.collect()).paint(&bounds, window, cx);
    }

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn interactive(&self) -> bool {
        self.interactive
    }

    fn appear(&mut self, appear: PlotAppear, _window: &mut Window, _cx: &mut App) {
        self.appear.update(appear);
    }

    fn appear_generation(&self) -> Option<u64> {
        self.appear.generation()
    }

    fn tooltip_state(
        &self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        _cx: &App,
    ) -> Option<TooltipState> {
        if self.values.is_empty() {
            return None;
        }
        let index = self.hovered_index(position, bounds)?;
        let d = self.data.get(index)?;

        let outer_radius = self.resolve_outer_radius(&bounds);
        let scale = self.scale(outer_radius);
        let center_x = bounds.size.width.as_f32() / 2.;
        let center_y = bounds.size.height.as_f32() / 2.;
        let angle = index as f32 * TAU / self.data.len() as f32 - HALF_PI;

        // One dot per series at the hovered dimension's vertex.
        let dots = self
            .values
            .iter()
            .filter_map(|value_fn| {
                let radius = scale.tick(&value_fn(d))?;
                Some(point(
                    px(center_x + radius * angle.cos()),
                    px(center_y + radius * angle.sin()),
                ))
            })
            .collect();

        Some(TooltipState::new(index, position, dots))
    }

    fn tooltip(
        &self,
        state: &TooltipState,
        cursor: Point<Pixels>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        let d = self.data.get(state.index)?;

        let dot_stroke = cx.theme().background;

        // No crosshair: a radar has no cartesian axis to snap to; the dots mark
        // the hovered dimension's vertices instead.
        let tooltip =
            Tooltip::new(cursor, bounds.size)
                .gap(px(8.))
                .dots(state.dots.iter().enumerate().map(|(i, p)| {
                    Dot::new(*p)
                        .size(HOVER_DOT_SIZE)
                        .halo(HOVER_HALO_SIZE)
                        .stroke(dot_stroke)
                        .fill(self.series_stroke(i, cx))
                }));

        let palette = Self::palette(cx);
        let tooltip = self.tooltip_content.apply(
            tooltip,
            d,
            // Filled by `prepaint`, which runs first; element labels leave no title.
            || self.label_texts.get(state.index).cloned().flatten(),
            // One row per series: swatch + label + value.
            || {
                self.values
                    .iter()
                    .enumerate()
                    .map(|(i, value_fn)| {
                        let name = self.names.get(i).cloned().unwrap_or_default();
                        Some((
                            self.series_stroke_from(&palette, i),
                            name,
                            value_fn(d).to_f64()?,
                        ))
                    })
                    .collect::<Option<Vec<_>>>()
            },
            window,
            cx,
        )?;

        Some(tooltip.into_any_element())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct Item {
        subject: SharedString,
        a: f64,
        b: f64,
    }

    #[test]
    fn test_radar_chart_builder() {
        let data = vec![
            Item {
                subject: "Sales".into(),
                a: 80.,
                b: 60.,
            },
            Item {
                subject: "Marketing".into(),
                a: 50.,
                b: 90.,
            },
        ];

        let chart = RadarChart::new(data.clone())
            .label(|d| d.subject.clone())
            .value(|d| d.a)
            .stroke(gpui::red())
            .fill(gpui::red())
            .name("A")
            .value(|d| d.b)
            .max_value(100.)
            .outer_radius(120.)
            .label_gap(8.)
            .grid(false)
            .grid_levels(5)
            .dot()
            .id("radar");

        assert_eq!(chart.data.len(), 2);
        assert_eq!(chart.values.len(), 2);
        assert_eq!(chart.strokes.len(), 1);
        assert_eq!(chart.fills.len(), 1);
        assert_eq!(chart.names.len(), 1);
        assert!(chart.label.is_some());
        assert_eq!(chart.max_value, Some(100.));
        assert_eq!(chart.outer_radius, 120.);
        assert_eq!(chart.label_gap, 8.);
        assert!(!chart.grid);
        assert_eq!(chart.grid_levels, 5);
        assert!(chart.dot);
        assert_eq!(chart.id, gpui::ElementId::Name("radar".into()));

        let values = (chart.values[0](&data[0]), chart.values[1](&data[0]));
        assert_eq!(values, (80., 60.));
    }

    /// Every string form the `label` closure may return lands on the text path,
    /// which is what keeps `label_color` and the tooltip title working.
    #[test]
    fn test_radar_label_from_text() {
        let labels = [
            RadarLabel::from("Sales"),
            RadarLabel::from("Sales".to_string()),
            RadarLabel::from(SharedString::from("Sales")),
        ];

        for label in labels {
            assert!(matches!(label, RadarLabel::Text(text) if text == "Sales"));
        }
    }

    #[test]
    fn spokes_start_at_twelve_and_run_clockwise() {
        let c = point(100., 100.);
        let top = spoke_point(c, 50., 0, 4);
        assert!((top.x - 100.).abs() < 1e-4 && (top.y - 50.).abs() < 1e-4);
        let right = spoke_point(c, 50., 1, 4);
        assert!((right.x - 150.).abs() < 1e-4 && (right.y - 100.).abs() < 1e-4);
    }

    #[test]
    fn member_dots_spread_only_when_they_crowd() {
        assert_eq!(dot_nudges(&[10., 40., 80.], 7.), [0., 0., 0.]);
        let nudges = dot_nudges(&[50., 52., 90.], 7.);
        assert_eq!(nudges, [-3.5, 3.5, 0.]);
        let three = dot_nudges(&[20., 21., 22.], 7.);
        assert_eq!(three, [-7., 0., 7.]);
        assert!(dot_nudges(&[], 7.).is_empty());
    }

    #[test]
    fn target_and_members_widen_the_default_scale() {
        let data = vec![Item {
            subject: "Sales".into(),
            a: 40.,
            b: 0.,
        }];
        let chart = RadarChart::new(data)
            .value(|d| d.a)
            .target(|_| 90.)
            .member_dots(|d| vec![d.a - 10., d.a + 70.]);
        let scale = chart.scale(100.);
        assert_eq!(scale.tick(&110.), Some(100.));
        assert!(chart.members.is_some() && chart.target.is_some());
    }

    #[test]
    fn test_radar_chart_grid_levels_min() {
        let chart: RadarChart<Item, f64> = RadarChart::new(vec![]).grid_levels(0);
        assert_eq!(chart.grid_levels, 1);
    }

    #[test]
    fn test_radar_chart_hovered_index() {
        let data = (0..4)
            .map(|i| Item {
                subject: format!("S{}", i).into(),
                a: 50.,
                b: 50.,
            })
            .collect::<Vec<_>>();

        // Bounds 200x200 => center (100, 100), default outer radius 80,
        // hover region 80 + 10 (label gap) = 90.
        let chart: RadarChart<Item, f64> = RadarChart::new(data).value(|d| d.a);
        let bounds = gpui::Bounds::new(point(px(0.), px(0.)), gpui::size(px(200.), px(200.)));

        // The four spokes point at 12, 3, 6 and 9 o'clock.
        assert_eq!(
            chart.hovered_index(point(px(100.), px(30.)), bounds),
            Some(0)
        );
        assert_eq!(
            chart.hovered_index(point(px(170.), px(100.)), bounds),
            Some(1)
        );
        assert_eq!(
            chart.hovered_index(point(px(100.), px(170.)), bounds),
            Some(2)
        );
        assert_eq!(
            chart.hovered_index(point(px(30.), px(100.)), bounds),
            Some(3)
        );

        // Nearest spoke wins between two spokes.
        assert_eq!(
            chart.hovered_index(point(px(110.), px(40.)), bounds),
            Some(0)
        );
        assert_eq!(
            chart.hovered_index(point(px(160.), px(90.)), bounds),
            Some(1)
        );

        // Outside the radar.
        assert_eq!(chart.hovered_index(point(px(100.), px(5.)), bounds), None);
        assert_eq!(chart.hovered_index(point(px(5.), px(5.)), bounds), None);
    }
}
