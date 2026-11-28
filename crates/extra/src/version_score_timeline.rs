// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Version score timeline: each shipped version at the evaluation score it
//! shipped with, laid out over time.
//!
//! A step line holds each score until the next version ships. A drop past the
//! tolerance is a regression: its dot and the drop take the danger colour. A
//! version that restored an earlier one is a rollback: a hollow ring, with a
//! dashed arc back to the version it brought back. An optional target is a
//! dashed line across the plot. Pointing at a version, or stepping with the
//! arrow keys, reads its score, its change from the version before and its
//! note underneath; with nothing picked, the readout shows the latest.

use std::{cell::Cell, f32::consts::TAU, rc::Rc};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::{Presence, TestSupportExt as _, Transition, transition};
use isonomic_ui::component::{
    ActiveTheme as _, Icon, Sizable as _, StyledExt as _, TextRole as _, h_flex, look, tag::Tag,
};
use isonomic_ui::{
    App, Bounds, ElementId, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent,
    MouseMoveEvent, ParentElement as _, PathBuilder, Pixels, Point, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, canvas, div, point,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::{panel, shell, shell_header};

/// Height of the plot, and the room kept free above and below the line.
const PLOT_H: f32 = 184.;
const ROOM: f32 = 14.;
/// Room left of the plot for the score labels, and under it for the names.
const GUTTER: f32 = 36.;
const AXIS: f32 = 22.;
/// Room at each end so the first and last dots are not cut.
const EDGE: f32 = 14.;
/// The narrowest gap between two version names on the axis.
const NAME_GAP: f32 = 44.;

/// One shipped version.
#[derive(Debug, Clone, PartialEq)]
pub struct ShippedVersion {
    /// Its name, such as "v2.4".
    pub name: SharedString,
    /// When it shipped, on any steady scale (days, hours): only the spacing
    /// matters.
    pub at: f64,
    /// When it shipped, as written in the readout.
    pub date: SharedString,
    /// The evaluation score it shipped with.
    pub score: f64,
    /// What changed, or why it was rolled back.
    pub note: Option<SharedString>,
    /// The name of the earlier version this one restored, when it was a
    /// rollback.
    pub restores: Option<SharedString>,
}

impl ShippedVersion {
    /// A version that shipped at `at` (written `date`) with `score`.
    pub fn new(
        name: impl Into<SharedString>,
        at: f64,
        date: impl Into<SharedString>,
        score: f64,
    ) -> Self {
        Self {
            name: name.into(),
            at,
            date: date.into(),
            score,
            note: None,
            restores: None,
        }
    }

    /// What changed in it.
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// Mark it as a rollback that restored the version named `name`.
    pub fn restores(mut self, name: impl Into<SharedString>) -> Self {
        self.restores = Some(name.into());
        self
    }
}

/// What a version's dot says about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionMark {
    /// An ordinary release.
    Release,
    /// It scored worse than the version before by more than the tolerance.
    Regression,
    /// It restored an earlier version.
    Rollback,
}

/// The change in score from the version before each one; none for the first.
pub fn deltas(versions: &[ShippedVersion]) -> Vec<Option<f64>> {
    (0..versions.len())
        .map(|ix| {
            ix.checked_sub(1)
                .map(|before| versions[ix].score - versions[before].score)
        })
        .collect()
}

/// What version `ix` is marked as. A rollback wins over a regression.
pub fn mark(
    versions: &[ShippedVersion],
    ix: usize,
    tolerance: f64,
    higher_is_better: bool,
) -> VersionMark {
    let Some(version) = versions.get(ix) else {
        return VersionMark::Release;
    };
    if version.restores.is_some() {
        return VersionMark::Rollback;
    }
    let worse = ix.checked_sub(1).is_some_and(|before| {
        let change = version.score - versions[before].score;
        let change = if higher_is_better { -change } else { change };
        change > tolerance
    });
    if worse {
        VersionMark::Regression
    } else {
        VersionMark::Release
    }
}

/// A round step near `raw`: 1, 2 or 5 times a power of ten.
pub fn nice_step(raw: f64) -> f64 {
    if !raw.is_finite() || raw <= 0. {
        return 1.;
    }
    let power = 10f64.powf(raw.log10().floor());
    let scaled = raw / power;
    let nice = if scaled <= 1. {
        1.
    } else if scaled <= 2. {
        2.
    } else if scaled <= 5. {
        5.
    } else {
        10.
    };
    nice * power
}

/// The score range to plot, padded out to whole steps, and the step: about
/// four gridlines whatever the scores, and room above and below them.
pub fn score_domain(scores: &[f64], target: Option<f64>) -> (f64, f64, f64) {
    let mut low = scores.iter().copied().fold(f64::INFINITY, f64::min);
    let mut high = scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if let Some(target) = target {
        low = low.min(target);
        high = high.max(target);
    }
    if !low.is_finite() || !high.is_finite() {
        return (0., 1., 0.25);
    }
    if (high - low).abs() < f64::EPSILON {
        low -= 1.;
        high += 1.;
    }
    let step = nice_step((high - low) / 4.);
    let floor = (low / step).floor() * step;
    let ceil = (high / step).ceil() * step;
    let floor = if floor == low { floor - step } else { floor };
    let ceil = if ceil == high { ceil + step } else { ceil };
    (floor, ceil, step)
}

/// The gridline values from `low` to `high`, `step` apart.
pub fn grid_values(low: f64, high: f64, step: f64) -> Vec<f64> {
    if step <= 0. || high < low {
        return Vec::new();
    }
    let count = ((high - low) / step).round() as usize;
    (0..=count).map(|ix| low + ix as f64 * step).collect()
}

/// How far along the time axis each version sits, 0 to 1.
pub fn time_fractions(versions: &[ShippedVersion]) -> Vec<f32> {
    let first = versions.first().map_or(0., |version| version.at);
    let last = versions.last().map_or(0., |version| version.at);
    let span = last - first;
    versions
        .iter()
        .map(|version| {
            if span <= 0. {
                if versions.len() == 1 { 0.5 } else { 0. }
            } else {
                ((version.at - first) / span) as f32
            }
        })
        .collect()
}

/// The pixel `x` of a fraction along a plot `width` wide, inside the edges.
pub fn x_at(fraction: f32, width: f32) -> f32 {
    EDGE + fraction * (width - 2. * EDGE).max(0.)
}

/// The pixel `y` of `score` in a plot `height` tall showing `low` to `high`.
pub fn y_at(score: f64, low: f64, high: f64, height: f32) -> f32 {
    let share = if high > low {
        ((score - low) / (high - low)) as f32
    } else {
        0.5
    };
    ROOM + (1. - share) * (height - 2. * ROOM)
}

/// The version nearest a pointer at pixel `x` of a plot `width` wide.
pub fn nearest(fractions: &[f32], x: f32, width: f32) -> Option<usize> {
    fractions
        .iter()
        .enumerate()
        .min_by(|a, b| {
            (x_at(*a.1, width) - x)
                .abs()
                .total_cmp(&(x_at(*b.1, width) - x).abs())
        })
        .map(|(ix, _)| ix)
}

/// Which versions get their name on the axis of a plot `width` wide, so
/// names keep at least [`NAME_GAP`] apart. The last and the picked version
/// always do; the rest are taken from the left where they fit.
pub fn named(fractions: &[f32], width: f32, picked: Option<usize>) -> Vec<bool> {
    let count = fractions.len();
    let xs: Vec<f32> = fractions.iter().map(|f| x_at(*f, width)).collect();
    let mut keep = vec![false; count];
    let forced: Vec<usize> = count
        .checked_sub(1)
        .into_iter()
        .chain(picked.filter(|ix| *ix < count))
        .collect();
    for &ix in &forced {
        keep[ix] = true;
    }
    let mut last: Option<f32> = None;
    for ix in 0..count {
        if keep[ix] {
            last = Some(xs[ix]);
            continue;
        }
        let clear_left = last.is_none_or(|x| xs[ix] - x >= NAME_GAP);
        let clear_forced = forced.iter().all(|&at| (xs[at] - xs[ix]).abs() >= NAME_GAP);
        if clear_left && clear_forced {
            keep[ix] = true;
            last = Some(xs[ix]);
        }
    }
    keep
}

/// A change with its sign: "+2.4", "−3.1", "0.0".
pub fn signed(delta: f64, decimals: usize) -> String {
    let text = format!("{:.*}", decimals, delta.abs());
    let zero = text.chars().all(|c| c == '0' || c == '.');
    if zero {
        text
    } else if delta > 0. {
        format!("+{text}")
    } else {
        format!("\u{2212}{text}")
    }
}

/// What a screen reader hears for one version.
pub fn spoken_version(
    versions: &[ShippedVersion],
    ix: usize,
    mark: VersionMark,
    format: &dyn Fn(f64) -> String,
) -> String {
    let Some(version) = versions.get(ix) else {
        return String::new();
    };
    let mut words = format!(
        "{}, shipped {}, score {}",
        version.name,
        version.date,
        format(version.score)
    );
    if let Some(before) = ix.checked_sub(1).and_then(|before| versions.get(before)) {
        let change = version.score - before.score;
        let way = if change > 0. {
            "up"
        } else if change < 0. {
            "down"
        } else {
            "unchanged"
        };
        if change == 0. {
            words.push_str(&format!(", unchanged from {}", before.name));
        } else {
            words.push_str(&format!(
                ", {way} {} from {}",
                format(change.abs()),
                before.name
            ));
        }
    }
    match (mark, &version.restores) {
        (VersionMark::Rollback, Some(name)) => {
            words.push_str(&format!(", rollback to {name}"));
        }
        (VersionMark::Regression, _) => words.push_str(", regression"),
        _ => {}
    }
    if let Some(note) = &version.note {
        words.push_str(&format!(". {note}"));
    }
    words
}

type Format = Rc<dyn Fn(f64) -> String>;
type SelectHandler = Rc<dyn Fn(&ShippedVersion, &mut Window, &mut App)>;

/// Versions over time at the score they shipped with.
#[derive(IntoElement)]
pub struct VersionScoreTimeline {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    subtitle: Option<SharedString>,
    versions: Vec<ShippedVersion>,
    target: Option<(f64, SharedString)>,
    tolerance: f64,
    higher_is_better: bool,
    format: Format,
    on_select: Option<SelectHandler>,
}

impl VersionScoreTimeline {
    /// Create a timeline of versions, oldest first.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        versions: Vec<ShippedVersion>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            subtitle: None,
            versions,
            target: None,
            tolerance: 0.,
            higher_is_better: true,
            format: Rc::new(|score| format!("{score:.1}")),
            on_select: None,
        }
    }

    /// A quieter line under the title.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// A score to hold, drawn as a dashed line with this label.
    pub fn target(mut self, score: f64, label: impl Into<SharedString>) -> Self {
        self.target = Some((score, label.into()));
        self
    }

    /// How far a score may slip before it counts as a regression. 0 by
    /// default: any drop is one.
    pub fn tolerance(mut self, tolerance: f64) -> Self {
        self.tolerance = tolerance.max(0.);
        self
    }

    /// Treat a lower score as the better one, such as for an error rate.
    pub fn lower_is_better(mut self) -> Self {
        self.higher_is_better = false;
        self
    }

    /// How a score is written. One decimal by default.
    pub fn format(mut self, format: impl Fn(f64) -> String + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }

    /// Run with the version the reader points at or steps to.
    pub fn on_select(
        mut self,
        handler: impl Fn(&ShippedVersion, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Styled for VersionScoreTimeline {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

fn paint_dot(window: &mut Window, centre: Point<Pixels>, radius: f32, color: Hsla) {
    let ring: Vec<_> = (0..16)
        .map(|step| {
            let angle = step as f32 / 16. * TAU;
            point(
                centre.x + px(radius * angle.cos()),
                centre.y + px(radius * angle.sin()),
            )
        })
        .collect();
    let mut dot = PathBuilder::fill();
    dot.add_polygon(&ring, true);
    if let Ok(dot) = dot.build() {
        window.paint_path(dot, color);
    }
}

fn paint_ring(window: &mut Window, centre: Point<Pixels>, radius: f32, wide: f32, color: Hsla) {
    let mut path = PathBuilder::stroke(px(wide));
    for step in 0..=24 {
        let angle = step as f32 / 24. * TAU;
        let at = point(
            centre.x + px(radius * angle.cos()),
            centre.y + px(radius * angle.sin()),
        );
        if step == 0 {
            path.move_to(at);
        } else {
            path.line_to(at);
        }
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

impl RenderOnce for VersionScoreTimeline {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let part = |name: &str| ElementId::NamedChild(self.id.clone().into(), name.into());
        let count = self.versions.len();
        let format = self.format.clone();
        let scores: Vec<f64> = self.versions.iter().map(|version| version.score).collect();
        let (low, high, step) = score_domain(&scores, self.target.as_ref().map(|(at, _)| *at));
        let fractions = time_fractions(&self.versions);
        let changes = deltas(&self.versions);
        let marks: Vec<VersionMark> = (0..count)
            .map(|ix| mark(&self.versions, ix, self.tolerance, self.higher_is_better))
            .collect();
        let best = scores
            .iter()
            .enumerate()
            .max_by(|a, b| {
                if self.higher_is_better {
                    a.1.total_cmp(b.1)
                } else {
                    b.1.total_cmp(a.1)
                }
            })
            .map(|(ix, _)| ix);
        // Where each rollback's arc lands: the latest earlier version of the
        // name it restored.
        let restored: Vec<Option<usize>> = (0..count)
            .map(|ix| {
                let name = self.versions[ix].restores.as_ref()?;
                (0..ix).rev().find(|&at| &self.versions[at].name == name)
            })
            .collect();

        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let data = theme.chart_1;
        let danger = theme.danger;
        let warning = theme.warning;
        let surface = theme.background;
        let mono = theme.mono_font_family.clone();
        let grid = ink.opacity(0.1);
        let fast = theme.motion.duration_fast;
        let normal = theme.motion.duration_normal;
        let slow = theme.motion.duration_slow;

        let active = window.use_keyed_state(part("active"), cx, |_, _| None::<usize>);
        let picked = (*active.read(cx)).filter(|ix| *ix < count);
        let plot_bounds = window
            .use_keyed_state(part("bounds"), cx, |_, _| {
                Rc::new(Cell::new(Bounds::<Pixels>::default()))
            })
            .read(cx)
            .clone();
        let known_width = f32::from(plot_bounds.get().size.width);
        let shown_names = named(
            &fractions,
            if known_width > 0. { known_width } else { 560. },
            picked,
        );

        let drawn = Presence::new(part("appear"), true)
            .transition(Transition::new(slow * 2))
            .sample(window, cx)
            .progress;
        // The guide slides to the picked version and fades when none is.
        let last_pick = window.use_keyed_state(part("last-pick"), cx, |_, _| 1f32);
        if let Some(ix) = picked {
            last_pick.update(cx, |state, _| *state = fractions[ix]);
        }
        let guide_at = transition(
            part("guide-at"),
            *last_pick.read(cx),
            Transition::new(normal),
            window,
            cx,
        );
        let guide_shown = transition(
            part("guide-shown"),
            if picked.is_some() { 1f32 } else { 0. },
            Transition::new(fast),
            window,
            cx,
        );

        // The picked dot grows and rings in, eased rather than snapped.
        let lit: Vec<f32> = (0..count)
            .map(|ix| look::hover_fade(part(&format!("dot-{ix}")), picked == Some(ix), window, cx))
            .collect();
        let paint_lit = lit.clone();
        let seen = plot_bounds.clone();
        let paint_scores = scores;
        let paint_fractions = fractions.clone();
        let paint_marks = marks.clone();
        let target_score = self.target.as_ref().map(|(at, _)| *at);
        let plot = canvas(
            move |bounds, _, _| seen.set(bounds),
            move |bounds, _, window, _| {
                let width = f32::from(bounds.size.width);
                let at = |x: f32, y: f32| point(bounds.left() + px(x), bounds.top() + px(y));
                let spot = |ix: usize| {
                    (
                        x_at(paint_fractions[ix], width),
                        y_at(paint_scores[ix], low, high, PLOT_H),
                    )
                };
                let stroke = |window: &mut Window,
                              points: &[(f32, f32)],
                              wide: f32,
                              dash: Option<[f32; 2]>,
                              color: Hsla| {
                    if points.len() < 2 {
                        return;
                    }
                    let mut path = PathBuilder::stroke(px(wide));
                    if let Some([on, off]) = dash {
                        path = path.dash_array(&[px(on), px(off)]);
                    }
                    path.move_to(at(points[0].0, points[0].1));
                    for &(x, y) in &points[1..] {
                        path.line_to(at(x, y));
                    }
                    if let Ok(path) = path.build() {
                        window.paint_path(path, color);
                    }
                };
                for value in grid_values(low, high, step) {
                    let y = y_at(value, low, high, PLOT_H);
                    stroke(window, &[(0., y), (width, y)], 1., Some([2., 4.]), grid);
                }
                if let Some(target) = target_score {
                    let y = y_at(target, low, high, PLOT_H);
                    stroke(
                        window,
                        &[(0., y), (width, y)],
                        1.5,
                        Some([5., 4.]),
                        ink.opacity(0.45),
                    );
                }
                if paint_scores.is_empty() {
                    return;
                }
                // The line draws in from the left; nothing past `reach` yet.
                let reach = drawn * width;
                let mut line = vec![spot(0)];
                let mut drops = Vec::new();
                for ix in 1..paint_scores.len() {
                    let (x, y) = spot(ix);
                    let held = line.last().map_or(y, |last| last.1);
                    line.push((x, held));
                    if paint_marks[ix] == VersionMark::Regression {
                        drops.push([(x, held), (x, y)]);
                    }
                    line.push((x, y));
                }
                let clipped: Vec<(f32, f32)> = line
                    .iter()
                    .copied()
                    .take_while(|point| point.0 <= reach + 0.5)
                    .collect();
                // A wash under the line.
                if clipped.len() > 1 {
                    let floor = PLOT_H - ROOM;
                    let mut area = PathBuilder::fill();
                    let mut outline = vec![at(clipped[0].0, floor)];
                    outline.extend(clipped.iter().map(|&(x, y)| at(x, y)));
                    outline.push(at(clipped.last().map_or(0., |last| last.0), floor));
                    area.add_polygon(&outline, true);
                    if let Ok(area) = area.build() {
                        window.paint_path(area, data.opacity(0.07));
                    }
                }
                stroke(window, &clipped, 2., None, data);
                for drop in drops {
                    if drop[0].0 <= reach {
                        stroke(window, &drop, 2., None, danger);
                    }
                }
                // Rollbacks arc back to what they restored.
                for (ix, from) in restored.iter().enumerate() {
                    let Some(from) = *from else { continue };
                    let (x1, y1) = spot(ix);
                    if x1 > reach {
                        continue;
                    }
                    let (x0, y0) = spot(from);
                    let lift = ((x1 - x0) * 0.3).clamp(14., 40.);
                    let top = y0.min(y1) - lift;
                    let arc: Vec<(f32, f32)> = (0..=20)
                        .map(|step| {
                            let t = step as f32 / 20.;
                            let u = 1. - t;
                            let mid = (x0 + x1) / 2.;
                            (
                                u * u * x1 + 2. * u * t * mid + t * t * x0,
                                u * u * y1 + 2. * u * t * top + t * t * y0,
                            )
                        })
                        .collect();
                    stroke(window, &arc, 1.25, Some([3., 3.]), warning);
                }
                // The guide under the picked version.
                if guide_shown > 0. {
                    let x = x_at(guide_at, width);
                    stroke(
                        window,
                        &[(x, 2.), (x, PLOT_H - 2.)],
                        1.,
                        None,
                        ink.opacity(0.35 * guide_shown),
                    );
                }
                for ix in 0..paint_scores.len() {
                    let (x, y) = spot(ix);
                    let shown = ((reach - x + 12.) / 24.).clamp(0., 1.);
                    if shown <= 0. {
                        continue;
                    }
                    let centre = at(x, y);
                    let grow = paint_lit[ix];
                    let radius = 4. + 1.5 * grow;
                    paint_dot(window, centre, radius + 2., surface.opacity(shown));
                    match paint_marks[ix] {
                        VersionMark::Rollback => {
                            paint_ring(window, centre, radius - 0.75, 1.5, warning.opacity(shown));
                        }
                        VersionMark::Regression => {
                            paint_dot(window, centre, radius, danger.opacity(shown));
                        }
                        VersionMark::Release => {
                            paint_dot(window, centre, radius, data.opacity(shown));
                        }
                    }
                    if grow > 0. {
                        paint_ring(window, centre, radius + 3.5, 1.5, ink.opacity(shown * grow));
                    }
                }
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .w_full()
        .h(px(PLOT_H));

        let score_labels = grid_values(low, high, step).into_iter().map(|value| {
            div()
                .absolute()
                .right(px(8.))
                .top(px(y_at(value, low, high, PLOT_H) - 8.))
                .font_family(mono.clone())
                .text_size(px(10.))
                .line_height(px(16.))
                .text_color(quiet)
                .child(format(value))
        });

        let names = self
            .versions
            .iter()
            .enumerate()
            .filter_map(|(ix, version)| {
                shown_names[ix].then(|| {
                    div()
                        .absolute()
                        .top_0()
                        .left(relative(fractions[ix]))
                        .w(px(0.))
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .flex_none()
                                .whitespace_nowrap()
                                .font_family(mono.clone())
                                .text_size(px(10.))
                                .line_height(px(16.))
                                .text_color(look::mix(quiet, ink, lit[ix]))
                                .child(version.name.clone()),
                        )
                })
            });

        // The readout: the picked version, or the latest.
        let shown = picked.or(count.checked_sub(1));
        let readout = shown.map(|ix| {
            let version = &self.versions[ix];
            let change = changes[ix];
            let better = change.map(|change| {
                if self.higher_is_better {
                    change > 0.
                } else {
                    change < 0.
                }
            });
            let pill = change.map(|change| {
                let flat = change == 0.;
                let icon = if flat {
                    CatalogIcon::Subtract
                } else if change > 0. {
                    CatalogIcon::ArrowUpRight
                } else {
                    CatalogIcon::ArrowDownRight
                };
                match (flat, better) {
                    (true, _) => Tag::secondary(),
                    (_, Some(true)) => Tag::success(),
                    _ => Tag::danger(),
                }
                .small()
                .rounded_full()
                .gap_1()
                .child(Icon::new(icon).size(px(12.)))
                .child(div().font_family(mono.clone()).child(signed(change, 1)))
            });
            let flag = match marks[ix] {
                VersionMark::Regression => Some(Tag::danger().small().child("Regression")),
                VersionMark::Rollback => Some(
                    Tag::warning()
                        .small()
                        .gap_1()
                        .child(Icon::new(CatalogIcon::Undo).size(px(12.)))
                        .child(format!(
                            "Rollback to {}",
                            version.restores.clone().unwrap_or_default()
                        )),
                ),
                VersionMark::Release => None,
            };
            let before = ix
                .checked_sub(1)
                .map(|before| format!("from {}", self.versions[before].name));
            div()
                .id(part("readout"))
                .mt_3()
                .px_1()
                .role(Role::Status)
                .aria_label(spoken_version(&self.versions, ix, marks[ix], &*format))
                .child(
                    h_flex()
                        .items_start()
                        .justify_between()
                        .gap_3()
                        .child(
                            div()
                                .min_w_0()
                                .child(
                                    h_flex()
                                        .flex_wrap()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_label()
                                                .font_family(mono.clone())
                                                .child(version.name.clone()),
                                        )
                                        .children(flag)
                                        .when(best == Some(ix) && count > 1, |this| {
                                            this.child(
                                                Tag::success().outline().small().child("Best"),
                                            )
                                        })
                                        .when(picked.is_none(), |this| {
                                            this.child(
                                                div()
                                                    .text_caption()
                                                    .text_color(quiet)
                                                    .child("Latest"),
                                            )
                                        }),
                                )
                                .child(
                                    div()
                                        .text_caption()
                                        .text_color(quiet)
                                        .child(format!("Shipped {}", version.date)),
                                ),
                        )
                        .child(
                            h_flex()
                                .flex_none()
                                .gap_2()
                                .children(pill)
                                .children(before.map(|before| {
                                    div().text_caption().text_color(quiet).child(before)
                                }))
                                .child(
                                    div()
                                        .font_family(mono.clone())
                                        .text_size(px(20.))
                                        .line_height(px(28.))
                                        .font_medium()
                                        .child(format(version.score)),
                                ),
                        ),
                )
                .when_some(version.note.clone(), |this, note| {
                    this.child(div().mt_1().text_body().text_color(quiet).child(note))
                })
        });

        let regressions = marks
            .iter()
            .filter(|mark| **mark == VersionMark::Regression)
            .count();
        let rollbacks = marks
            .iter()
            .filter(|mark| **mark == VersionMark::Rollback)
            .count();
        let spoken = match picked {
            Some(ix) => format!(
                "{}: {}",
                self.title,
                spoken_version(&self.versions, ix, marks[ix], &*format)
            ),
            None => format!(
                "{}: {count} versions{}, {regressions} {}, {rollbacks} {}. Arrow keys read each version.",
                self.title,
                shown
                    .map(|ix| format!(
                        ", latest {} at {}",
                        self.versions[ix].name,
                        format(self.versions[ix].score)
                    ))
                    .unwrap_or_default(),
                if regressions == 1 {
                    "regression"
                } else {
                    "regressions"
                },
                if rollbacks == 1 {
                    "rollback"
                } else {
                    "rollbacks"
                },
            ),
        };

        let legend_mark = |color: Hsla, hollow: bool| {
            div()
                .size(px(8.))
                .rounded_full()
                .when(hollow, |this| this.border_1().border_color(color))
                .when(!hollow, |this| this.bg(color))
        };
        let legend = h_flex()
            .mt_3()
            .px_1()
            .flex_wrap()
            .gap_x_4()
            .gap_y_1()
            .text_caption()
            .text_color(quiet)
            .child(
                h_flex()
                    .gap(px(6.))
                    .child(legend_mark(data, false))
                    .child("Release"),
            )
            .child(
                h_flex()
                    .gap(px(6.))
                    .child(legend_mark(danger, false))
                    .child(format!("Regression · {regressions}")),
            )
            .child(
                h_flex()
                    .gap(px(6.))
                    .child(legend_mark(warning, true))
                    .child(format!("Rollback · {rollbacks}")),
            )
            .when_some(self.target.clone(), |this, (score, label)| {
                this.child(
                    h_flex()
                        .gap(px(6.))
                        .child(
                            div()
                                .w(px(12.))
                                .h(px(0.))
                                .border_t_1()
                                .border_dashed()
                                .border_color(ink.opacity(0.45)),
                        )
                        .child(label)
                        .child(
                            div()
                                .font_family(mono.clone())
                                .text_color(ink)
                                .child(format(score)),
                        ),
                )
            });

        let hover_state = active.clone();
        let hover_select = self.on_select.clone();
        let hover_versions = self.versions.clone();
        let hover_fractions = fractions.clone();
        let hover_bounds = plot_bounds;
        let leave_state = active.clone();
        let key_state = active;
        let key_select = self.on_select.clone();
        let key_versions = self.versions.clone();

        let latest = self.versions.last().map(|_| {
            div()
                .font_family(mono.clone())
                .text_caption()
                .text_color(quiet)
                .child(format!("{count} versions"))
                .into_any_element()
        });

        shell(cx)
            .id(self.id.clone())
            .max_w(px(672.))
            .child(shell_header(
                Some(Icon::new(CatalogIcon::ChartLine)),
                self.title.clone(),
                self.subtitle.clone().map(|line| line.into_any_element()),
                latest,
                cx,
            ))
            .child(
                panel(cx)
                    .p_4()
                    .child(
                        div()
                            .id(part("plot"))
                            .test_support()
                            .relative()
                            .w_full()
                            .h(px(PLOT_H + AXIS))
                            .pl(px(GUTTER))
                            .rounded(px(12.))
                            .role(Role::Group)
                            .aria_label(spoken)
                            .tab_index(0)
                            .on_mouse_move(move |event: &MouseMoveEvent, window, cx| {
                                let bounds = hover_bounds.get();
                                let width = f32::from(bounds.size.width);
                                let x = f32::from(event.position.x - bounds.left());
                                if width <= 0. || x < -EDGE || x > width + EDGE {
                                    return;
                                }
                                let next = nearest(&hover_fractions, x, width);
                                if *hover_state.read(cx) != next {
                                    hover_state.update(cx, |state, cx| {
                                        *state = next;
                                        cx.notify();
                                    });
                                    if let (Some(select), Some(ix)) = (&hover_select, next) {
                                        select(&hover_versions[ix], window, cx);
                                    }
                                }
                            })
                            .on_hover(move |over, _, cx| {
                                if !*over {
                                    leave_state.update(cx, |state, cx| {
                                        *state = None;
                                        cx.notify();
                                    });
                                }
                            })
                            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                                if count == 0 {
                                    return;
                                }
                                let now = *key_state.read(cx);
                                let next = match event.keystroke.key.as_str() {
                                    "right" => Some(now.map_or(0, |at| (at + 1).min(count - 1))),
                                    "left" => {
                                        Some(now.map_or(count - 1, |at| at.saturating_sub(1)))
                                    }
                                    "home" => Some(0),
                                    "end" => Some(count - 1),
                                    "escape" => None,
                                    _ => return,
                                };
                                key_state.update(cx, |state, cx| {
                                    *state = next;
                                    cx.notify();
                                });
                                if let (Some(select), Some(ix)) = (&key_select, next) {
                                    select(&key_versions[ix], window, cx);
                                }
                                cx.stop_propagation();
                            })
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .w(px(GUTTER))
                                    .h(px(PLOT_H))
                                    .children(score_labels),
                            )
                            .child(
                                div().relative().size_full().child(plot).child(
                                    div()
                                        .absolute()
                                        .top(px(PLOT_H + 4.))
                                        .left(px(EDGE))
                                        .right(px(EDGE))
                                        .h(px(16.))
                                        .children(names),
                                ),
                            ),
                    )
                    .children(readout)
                    .child(legend),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn releases() -> Vec<ShippedVersion> {
        vec![
            ShippedVersion::new("v1", 0., "1 Mar", 70.),
            ShippedVersion::new("v2", 5., "6 Mar", 74.),
            ShippedVersion::new("v3", 10., "11 Mar", 71.5),
            ShippedVersion::new("v2", 12., "13 Mar", 74.).restores("v2"),
            ShippedVersion::new("v4", 20., "21 Mar", 73.8),
        ]
    }

    #[test]
    fn changes_are_against_the_version_before() {
        let changes = deltas(&releases());
        assert_eq!(changes[0], None);
        assert_eq!(changes[1], Some(4.));
        assert_eq!(changes[2], Some(-2.5));
        assert!(deltas(&[]).is_empty());
    }

    #[test]
    fn drops_past_the_tolerance_are_regressions_and_rollbacks_win() {
        let all = releases();
        assert_eq!(mark(&all, 0, 0., true), VersionMark::Release);
        assert_eq!(mark(&all, 1, 0., true), VersionMark::Release);
        assert_eq!(mark(&all, 2, 0., true), VersionMark::Regression);
        assert_eq!(mark(&all, 3, 0., true), VersionMark::Rollback);
        // A slip of 0.2 is a regression only without tolerance.
        assert_eq!(mark(&all, 4, 0., true), VersionMark::Regression);
        assert_eq!(mark(&all, 4, 0.5, true), VersionMark::Release);
        // When lower is better, the rise is the regression.
        assert_eq!(mark(&all, 1, 0., false), VersionMark::Regression);
        assert_eq!(mark(&all, 2, 0., false), VersionMark::Release);
        assert_eq!(mark(&all, 9, 0., true), VersionMark::Release);
    }

    #[test]
    fn the_domain_pads_out_to_round_steps() {
        assert_eq!(nice_step(3.), 5.);
        assert_eq!(nice_step(0.12), 0.2);
        assert_eq!(nice_step(10.), 10.);
        assert_eq!(nice_step(0.), 1.);
        let (low, high, step) = score_domain(&[70., 74., 71.5], None);
        assert_eq!(step, 1.);
        assert_eq!((low, high), (69., 75.));
        // The target is always in view.
        let (low, high, _) = score_domain(&[70., 74.], Some(80.));
        assert!(low < 70. && high > 80.);
        // One score still gets a range.
        let (low, high, _) = score_domain(&[50.], None);
        assert!(low < 50. && high > 50.);
        assert_eq!(score_domain(&[], None), (0., 1., 0.25));
        assert_eq!(grid_values(69., 72., 1.), [69., 70., 71., 72.]);
        assert!(grid_values(1., 0., 1.).is_empty());
    }

    #[test]
    fn versions_sit_by_time_and_the_pointer_finds_the_nearest() {
        let fractions = time_fractions(&releases());
        assert_eq!(fractions[0], 0.);
        assert_eq!(fractions[2], 0.5);
        assert_eq!(fractions[4], 1.);
        assert_eq!(time_fractions(&releases()[..1]), [0.5]);
        // 228 wide: the plot runs from 14 to 214.
        assert_eq!(x_at(0., 228.), 14.);
        assert_eq!(x_at(1., 228.), 214.);
        assert_eq!(nearest(&fractions, 0., 228.), Some(0));
        assert_eq!(nearest(&fractions, 118., 228.), Some(2));
        assert_eq!(nearest(&fractions, 300., 228.), Some(4));
        assert_eq!(nearest(&[], 10., 228.), None);
        assert_eq!(y_at(100., 0., 100., 128.), ROOM);
        assert_eq!(y_at(0., 0., 100., 128.), 128. - ROOM);
    }

    #[test]
    fn names_thin_out_when_crowded() {
        // 228 wide: x at 14, 64, 114, 124 and 214.
        let fractions = [0., 0.25, 0.5, 0.55, 1.];
        assert_eq!(
            named(&fractions, 228., None),
            [true, true, true, false, true]
        );
        // A picked name pushes out its close neighbour.
        assert_eq!(
            named(&fractions, 228., Some(3)),
            [true, true, false, true, true]
        );
        assert!(named(&[], 228., None).is_empty());
    }

    #[test]
    fn changes_carry_their_sign_and_versions_read_in_words() {
        assert_eq!(signed(2.44, 1), "+2.4");
        assert_eq!(signed(-3.06, 1), "\u{2212}3.1");
        assert_eq!(signed(0.01, 1), "0.0");
        let all = releases();
        let format = |score: f64| format!("{score:.1}");
        assert_eq!(
            spoken_version(&all, 2, VersionMark::Regression, &format),
            "v3, shipped 11 Mar, score 71.5, down 2.5 from v2, regression"
        );
        assert_eq!(
            spoken_version(&all, 3, VersionMark::Rollback, &format),
            "v2, shipped 13 Mar, score 74.0, up 2.5 from v3, rollback to v2"
        );
        assert_eq!(
            spoken_version(&all, 0, VersionMark::Release, &format),
            "v1, shipped 1 Mar, score 70.0"
        );
    }
}
