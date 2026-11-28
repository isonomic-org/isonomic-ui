// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fit field: candidates placed round a centre by how well they fit.
//!
//! The field is a disc cut into one sector per category. Each candidate sits
//! in its category's sector, nearer the middle the higher its overall score,
//! so the best fits gather at the centre. Dashed rings mark the score
//! thresholds (such as strong and possible). Pick a candidate on the field,
//! in the ranked list, or step through them best first with the arrow keys,
//! to read how each criterion adds up to the score.

use std::{cell::Cell, f32::consts::TAU, rc::Rc};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::{Presence, TestSupportExt as _, Transition};
use isonomic_ui::component::{
    ActiveTheme as _, Icon, Sizable as _, StyledExt as _, TextRole as _, h_flex, look,
    progress::Progress, tag::Tag, v_flex,
};
use isonomic_ui::{
    App, Bounds, ElementId, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, ParentElement as _, PathBuilder, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, canvas, div,
    point, prelude::FluentBuilder as _, px, relative,
};

use crate::common::{panel, shell, shell_header};

/// The field is laid out on a 340 box (300 plus room for sector labels) and
/// scaled to its width.
const BOX: f32 = 340.;
const PAD: f32 = 20.;
const CENTRE: f32 = 150.;
/// A perfect score sits this far out; a zero score sits on the rim.
const CORE: f32 = 10.;
const RIM: f32 = 140.;
/// How near the pointer must be to a candidate, in field units.
const REACH: f32 = 9.;
/// How many candidates the ranked list shows.
const LISTED: usize = 5;

/// One thing candidates are scored on.
#[derive(Debug, Clone, PartialEq)]
pub struct Criterion {
    /// What it is called.
    pub name: SharedString,
    /// How much it counts towards the overall score.
    pub weight: f64,
}

impl Criterion {
    /// A criterion and its weight.
    pub fn new(name: impl Into<SharedString>, weight: f64) -> Self {
        Self {
            name: name.into(),
            weight: weight.max(0.),
        }
    }
}

/// One candidate on the field.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// Stable id, reported back when picked.
    pub id: SharedString,
    /// What the reader calls them.
    pub name: SharedString,
    /// Index of their category.
    pub category: usize,
    /// One score per criterion, 0 to 100.
    pub scores: Vec<f64>,
}

impl Candidate {
    /// A candidate in a category, with one score (0 to 100) per criterion.
    pub fn new(
        id: impl Into<SharedString>,
        name: impl Into<SharedString>,
        category: usize,
        scores: impl IntoIterator<Item = f64>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            category,
            scores: scores
                .into_iter()
                .map(|score| score.clamp(0., 100.))
                .collect(),
        }
    }
}

/// A score line drawn as a ring.
#[derive(Debug, Clone, PartialEq)]
pub struct Threshold {
    /// The least score inside it.
    pub score: f64,
    /// What a score at or above it is called, such as "Strong fit".
    pub label: SharedString,
}

impl Threshold {
    /// A threshold and its name.
    pub fn new(score: f64, label: impl Into<SharedString>) -> Self {
        Self {
            score: score.clamp(0., 100.),
            label: label.into(),
        }
    }
}

/// The weighted mean of `scores`, 0 to 100. Missing scores count as zero.
pub fn overall(scores: &[f64], criteria: &[Criterion]) -> f64 {
    let total: f64 = criteria.iter().map(|one| one.weight).sum();
    if total <= 0. {
        return 0.;
    }
    criteria
        .iter()
        .enumerate()
        .map(|(ix, one)| scores.get(ix).copied().unwrap_or(0.) * one.weight)
        .sum::<f64>()
        / total
}

/// How far from the middle a score sits on the 300 field.
pub fn radius_for(score: f64) -> f32 {
    CORE + (RIM - CORE) * (1. - (score / 100.).clamp(0., 1.)) as f32
}

/// The index of the highest threshold the score reaches, given thresholds
/// sorted from high to low.
pub fn tier_of(score: f64, thresholds: &[Threshold]) -> Option<usize> {
    thresholds.iter().position(|one| score >= one.score)
}

/// Where the `k`-th of `m` candidates in sector `sector` of `sectors` sits,
/// as a turn (0 at the top, clockwise), kept clear of the sector's edges.
pub fn sector_turn(sector: usize, sectors: usize, k: usize, m: usize) -> f32 {
    let span = 1. / sectors.max(1) as f32;
    let inner = span * 0.8;
    let start = sector as f32 * span + span * 0.1;
    start + inner * (k as f32 + 0.5) / m.max(1) as f32
}

/// A point `turn` of the way round, `radius` from the middle of the 300 field.
pub fn field_point(turn: f32, radius: f32) -> (f32, f32) {
    let angle = turn * TAU - TAU / 4.;
    (CENTRE + radius * angle.cos(), CENTRE + radius * angle.sin())
}

/// Every candidate's place on the field. Within a sector candidates fan out
/// in order of score, so neighbours with close scores do not overlap.
pub fn places(candidates: &[Candidate], sectors: usize, totals: &[f64]) -> Vec<(f32, f32)> {
    let mut out = vec![(CENTRE, CENTRE); candidates.len()];
    for sector in 0..sectors {
        let mut members: Vec<usize> = (0..candidates.len())
            .filter(|ix| candidates[*ix].category == sector)
            .collect();
        members.sort_by(|a, b| totals[*b].total_cmp(&totals[*a]));
        let m = members.len();
        // Alternate sides of the sector's middle so the best sit centrally.
        let mut slots: Vec<usize> = (0..m).collect();
        slots.sort_by_key(|slot| {
            let middle = (m as f32 - 1.) / 2.;
            ((*slot as f32 - middle).abs() * 2.) as usize
        });
        for (rank, ix) in members.iter().enumerate() {
            let turn = sector_turn(sector, sectors, slots[rank], m);
            out[*ix] = field_point(turn, radius_for(totals[*ix]));
        }
    }
    out
}

/// The candidate nearest a point of the field, if within reach.
pub fn nearest(places: &[(f32, f32)], x: f32, y: f32, reach: f32) -> Option<usize> {
    places
        .iter()
        .enumerate()
        .map(|(ix, (px_, py))| (ix, (px_ - x).powi(2) + (py - y).powi(2)))
        .filter(|(_, distance)| *distance <= reach * reach)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(ix, _)| ix)
}

/// Candidate indices best first.
pub fn ranked(totals: &[f64]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..totals.len()).collect();
    order.sort_by(|a, b| totals[*b].total_cmp(&totals[*a]));
    order
}

/// A candidate and their breakdown, in words.
pub fn spoken(
    candidate: &Candidate,
    category: &str,
    total: f64,
    tier: Option<&str>,
    criteria: &[Criterion],
) -> String {
    let parts: Vec<String> = criteria
        .iter()
        .enumerate()
        .map(|(ix, one)| {
            format!(
                "{} {}",
                one.name,
                candidate.scores.get(ix).copied().unwrap_or(0.).round()
            )
        })
        .collect();
    format!(
        "{}, {category}, score {}, {}. {}",
        candidate.name,
        total.round(),
        tier.map_or("below every threshold".to_string(), |tier| tier
            .to_lowercase()),
        parts.join(", ")
    )
}

type SelectHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct FieldState {
    picked: Option<usize>,
    hover: Option<usize>,
    row_hover: Option<usize>,
}

/// Candidates round a centre by score, in sectors by category.
#[derive(IntoElement)]
pub struct FitField {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    subtitle: Option<SharedString>,
    categories: Vec<SharedString>,
    criteria: Vec<Criterion>,
    candidates: Vec<Candidate>,
    thresholds: Vec<Threshold>,
    on_select: Option<SelectHandler>,
}

impl FitField {
    /// Create a field with its categories, the criteria scored, and the
    /// candidates.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        categories: Vec<SharedString>,
        criteria: Vec<Criterion>,
        candidates: Vec<Candidate>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            subtitle: None,
            categories,
            criteria,
            candidates,
            thresholds: vec![
                Threshold::new(80., "Strong fit"),
                Threshold::new(60., "Possible fit"),
            ],
            on_select: None,
        }
    }

    /// A quieter line under the title.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Replace the threshold rings. Strong fit at 80 and possible fit at 60
    /// by default.
    pub fn thresholds(mut self, mut thresholds: Vec<Threshold>) -> Self {
        thresholds.sort_by(|a, b| b.score.total_cmp(&a.score));
        self.thresholds = thresholds;
        self
    }

    /// Run with a candidate's id when they are picked.
    pub fn on_select(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Styled for FitField {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

fn set_picked(
    state: &isonomic_ui::Entity<FieldState>,
    next: Option<usize>,
    ids: &[SharedString],
    select: &Option<SelectHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    state.update(cx, |state, cx| {
        state.picked = next;
        cx.notify();
    });
    if let (Some(select), Some(ix)) = (select, next) {
        select(&ids[ix], window, cx);
    }
}

impl RenderOnce for FitField {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let part = |name: String| ElementId::NamedChild(self.id.clone().into(), name.into());
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let data = theme.chart_1;
        let surface = theme.background;
        let raised = theme.popover;
        let ring = theme.ring;
        let border = theme.border;
        let mono = theme.mono_font_family.clone();
        let wash = look::ink::wash_strong(cx);
        let sectors = self.categories.len().max(1);

        let totals: Vec<f64> = self
            .candidates
            .iter()
            .map(|one| overall(&one.scores, &self.criteria))
            .collect();
        let spots = places(&self.candidates, sectors, &totals);
        let order = ranked(&totals);
        let tiers: Vec<Option<usize>> = totals
            .iter()
            .map(|total| tier_of(*total, &self.thresholds))
            .collect();
        let ids: Rc<Vec<SharedString>> =
            Rc::new(self.candidates.iter().map(|one| one.id.clone()).collect());
        let count = self.candidates.len();

        let state = window.use_keyed_state(part("state".into()), cx, |_, _| FieldState::default());
        let now = *state.read(cx);
        let picked = now.picked.filter(|ix| *ix < count);
        let field_bounds = window
            .use_keyed_state(part("bounds".into()), cx, |_, _| {
                Rc::new(Cell::new(Bounds::default()))
            })
            .read(cx)
            .clone();
        let focus = window
            .use_keyed_state(part("focus".into()), cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            })
            .read(cx)
            .clone();
        let focused = focus.is_focused(window);
        let appear = std::time::Duration::from_millis(500);
        let drawn = Presence::new(part("appear".into()), true)
            .transition(Transition::new(appear))
            .sample(window, cx)
            .progress;
        let row_fades: Vec<f32> = order
            .iter()
            .take(LISTED)
            .map(|ix| {
                look::hover_fade(
                    part(format!("row-fade-{ix}")),
                    now.row_hover == Some(*ix) || picked == Some(*ix),
                    window,
                    cx,
                )
            })
            .collect();

        let tier_color = |tier: Option<usize>| -> Hsla {
            match tier {
                Some(0) => data,
                Some(_) => ink.opacity(0.6),
                None => ink.opacity(0.25),
            }
        };
        let colors: Vec<Hsla> = tiers.iter().map(|tier| tier_color(*tier)).collect();
        let rings: Vec<f32> = self
            .thresholds
            .iter()
            .map(|one| radius_for(one.score))
            .collect();

        let seen = field_bounds.clone();
        let paint_spots = spots.clone();
        let paint_colors = colors.clone();
        let hover = now.hover;
        let field = canvas(
            move |bounds, _, _| seen.set(bounds),
            move |bounds, _, window, _| {
                let scale = f32::from(bounds.size.width) / BOX;
                let at = |(x, y): (f32, f32)| {
                    point(
                        bounds.left() + px((PAD + x) * scale),
                        bounds.top() + px((PAD + y) * scale),
                    )
                };
                let circle = |window: &mut Window,
                              centre: (f32, f32),
                              r: f32,
                              fill: bool,
                              dash: bool,
                              color: Hsla| {
                    let steps = (r * 0.8).clamp(12., 120.) as usize;
                    let ring: Vec<_> = (0..=steps)
                        .map(|step| {
                            let angle = step as f32 / steps as f32 * TAU;
                            at((centre.0 + r * angle.cos(), centre.1 + r * angle.sin()))
                        })
                        .collect();
                    if fill {
                        let mut path = PathBuilder::fill();
                        path.add_polygon(&ring, true);
                        if let Ok(path) = path.build() {
                            window.paint_path(path, color);
                        }
                    } else {
                        let mut path = PathBuilder::stroke(px(1. * scale));
                        if dash {
                            path = path.dash_array(&[px(3. * scale), px(3. * scale)]);
                        }
                        path.move_to(ring[0]);
                        for spot in &ring[1..] {
                            path.line_to(*spot);
                        }
                        if let Ok(path) = path.build() {
                            window.paint_path(path, color);
                        }
                    }
                };

                // Alternate sectors carry a faint wash; dividers run to the rim.
                for sector in 0..sectors {
                    let from = sector as f32 / sectors as f32;
                    let to = (sector + 1) as f32 / sectors as f32;
                    if sector % 2 == 1 {
                        let steps = 40;
                        let mut wedge = vec![at((CENTRE, CENTRE))];
                        for step in 0..=steps {
                            let turn = from + (to - from) * step as f32 / steps as f32;
                            wedge.push(at(field_point(turn, RIM)));
                        }
                        let mut path = PathBuilder::fill();
                        path.add_polygon(&wedge, true);
                        if let Ok(path) = path.build() {
                            window.paint_path(path, ink.opacity(0.03));
                        }
                    }
                    if sectors > 1 {
                        let mut line = PathBuilder::stroke(px(1. * scale));
                        line.move_to(at(field_point(from, CORE)));
                        line.line_to(at(field_point(from, RIM)));
                        if let Ok(line) = line.build() {
                            window.paint_path(line, ink.opacity(0.1));
                        }
                    }
                }
                circle(
                    window,
                    (CENTRE, CENTRE),
                    RIM,
                    false,
                    false,
                    ink.opacity(0.14),
                );
                for (ix, radius) in rings.iter().enumerate() {
                    let strength = if ix == 0 { 0.5 } else { 0.3 };
                    let color = if ix == 0 { data } else { ink };
                    circle(
                        window,
                        (CENTRE, CENTRE),
                        *radius,
                        false,
                        true,
                        color.opacity(strength),
                    );
                }
                circle(window, (CENTRE, CENTRE), 3., true, false, ink.opacity(0.5));

                for (ix, spot) in paint_spots.iter().enumerate() {
                    // Candidates slide in from the rim as the field appears.
                    let (dx, dy) = (spot.0 - CENTRE, spot.1 - CENTRE);
                    let from_rim = {
                        let distance = (dx * dx + dy * dy).sqrt().max(1e-3);
                        RIM / distance
                    };
                    let reach = 1. + (from_rim - 1.) * (1. - drawn);
                    let place = (CENTRE + dx * reach, CENTRE + dy * reach);
                    let dim = if picked.is_some_and(|at| at != ix) {
                        0.5
                    } else {
                        1.
                    };
                    let color = paint_colors[ix];
                    if picked == Some(ix) || hover == Some(ix) {
                        circle(window, place, 9., true, false, color.opacity(0.22));
                    }
                    circle(window, place, 5., true, false, surface);
                    circle(window, place, 4., true, false, color.opacity(color.a * dim));
                }
            },
        )
        .w_full()
        .aspect_ratio(1.);

        let sector_labels = self.categories.iter().enumerate().map(|(ix, name)| {
            let (x, y) = field_point((ix as f32 + 0.5) / sectors as f32, RIM + 12.);
            div()
                .absolute()
                .left(relative((PAD + x) / BOX))
                .top(relative((PAD + y) / BOX))
                .w(px(0.))
                .h(px(0.))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .flex_none()
                        .whitespace_nowrap()
                        .text_size(px(11.))
                        .line_height(px(16.))
                        .font_medium()
                        .text_color(quiet)
                        .child(name.clone()),
                )
        });
        let ring_labels = self.thresholds.iter().map(|one| {
            let (x, y) = field_point(0., radius_for(one.score));
            div()
                .absolute()
                .left(relative((PAD + x) / BOX))
                .top(relative((PAD + y) / BOX))
                .ml(px(4.))
                .mt(px(-14.))
                .whitespace_nowrap()
                .font_family(mono.clone())
                .text_size(px(10.))
                .line_height(px(12.))
                .text_color(quiet)
                .child(format!("{}", one.score.round()))
        });

        let tip = now.hover.filter(|ix| Some(*ix) != picked).map(|ix| {
            let (x, y) = spots[ix];
            div()
                .absolute()
                .left(relative(((PAD + x) / BOX).min(0.66)))
                .top(relative((PAD + y) / BOX))
                .ml(px(10.))
                .mt(px(8.))
                .px_2()
                .py_1()
                .rounded(px(8.))
                .bg(raised)
                .shadow_md()
                .text_caption()
                .whitespace_nowrap()
                .child(format!(
                    "{} · {}",
                    self.candidates[ix].name,
                    totals[ix].round()
                ))
        });

        let category_name = |ix: usize| {
            self.categories
                .get(self.candidates[ix].category)
                .cloned()
                .unwrap_or_default()
        };
        let tier_label = |ix: usize| tiers[ix].map(|tier| self.thresholds[tier].label.clone());
        let spoken_now = match picked {
            Some(ix) => spoken(
                &self.candidates[ix],
                &category_name(ix),
                totals[ix],
                tier_label(ix).as_deref(),
                &self.criteria,
            ),
            None => format!(
                "{}: {} candidates in {} categories. Arrow keys step through them, best fit first.",
                self.title,
                count,
                self.categories.len()
            ),
        };

        let move_state = state.clone();
        let move_bounds = field_bounds.clone();
        let move_spots = spots.clone();
        let click_state = state.clone();
        let click_spots = spots;
        let click_ids = ids.clone();
        let click_select = self.on_select.clone();
        let click_focus = focus.clone();
        let leave_state = state.clone();
        let key_state = state.clone();
        let key_ids = ids.clone();
        let key_select = self.on_select.clone();
        let key_order = order.clone();
        let to_field = move |bounds: Bounds<isonomic_ui::Pixels>,
                             at: isonomic_ui::Point<isonomic_ui::Pixels>| {
            let scale = f32::from(bounds.size.width) / BOX;
            (scale > 0.).then(|| {
                (
                    f32::from(at.x - bounds.left()) / scale - PAD,
                    f32::from(at.y - bounds.top()) / scale - PAD,
                )
            })
        };

        let field_box = div()
            .id(part("field".into()))
            .test_support()
            .track_focus(&focus)
            .relative()
            .flex_none()
            .w(px(300.))
            .max_w_full()
            .cursor_pointer()
            .role(Role::Group)
            .aria_label(spoken_now)
            .on_mouse_move(move |event: &MouseMoveEvent, _, cx| {
                let Some((x, y)) = to_field(move_bounds.get(), event.position) else {
                    return;
                };
                let next = nearest(&move_spots, x, y, REACH);
                if move_state.read(cx).hover != next {
                    move_state.update(cx, |state, cx| {
                        state.hover = next;
                        cx.notify();
                    });
                }
            })
            .on_hover(move |over, _, cx| {
                if !*over {
                    leave_state.update(cx, |state, cx| {
                        state.hover = None;
                        cx.notify();
                    });
                }
            })
            .on_mouse_down(
                MouseButton::Left,
                move |event: &MouseDownEvent, window, cx| {
                    window.focus(&click_focus, cx);
                    let Some((x, y)) = to_field(field_bounds.get(), event.position) else {
                        return;
                    };
                    let next = nearest(&click_spots, x, y, REACH);
                    set_picked(&click_state, next, &click_ids, &click_select, window, cx);
                },
            )
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                if count == 0 {
                    return;
                }
                let now = key_state.read(cx).picked;
                let at = now.and_then(|ix| key_order.iter().position(|one| *one == ix));
                let last = count - 1;
                let next = match event.keystroke.key.as_str() {
                    "right" | "down" => Some(at.map_or(0, |at| (at + 1) % count)),
                    "left" | "up" => Some(at.map_or(last, |at| (at + last) % count)),
                    "home" => Some(0),
                    "end" => Some(last),
                    "escape" => None,
                    _ => return,
                };
                let next = next.map(|at| key_order[at]);
                set_picked(&key_state, next, &key_ids, &key_select, window, cx);
                cx.stop_propagation();
            })
            .child(field)
            .children(sector_labels)
            .children(ring_labels)
            .children(tip)
            .when(focused, |this| {
                this.child(
                    div()
                        .absolute()
                        .inset_0()
                        .rounded_full()
                        .border_2()
                        .border_color(ring),
                )
            });

        // Beside the field: the picked candidate's breakdown, or the top fits.
        let side = match picked {
            Some(ix) => {
                let candidate = &self.candidates[ix];
                let total_weight: f64 = self.criteria.iter().map(|one| one.weight).sum();
                let bars = self.criteria.iter().enumerate().map(|(cix, one)| {
                    let score = candidate.scores.get(cix).copied().unwrap_or(0.);
                    let share = if total_weight > 0. {
                        one.weight / total_weight * 100.
                    } else {
                        0.
                    };
                    v_flex()
                        .gap(px(4.))
                        .child(
                            h_flex()
                                .justify_between()
                                .text_caption()
                                .child(h_flex().gap_1().child(one.name.clone()).child(
                                    div().text_color(quiet).child(format!("{}%", share.round())),
                                ))
                                .child(
                                    div()
                                        .font_family(mono.clone())
                                        .child(format!("{}", score.round())),
                                ),
                        )
                        .child(
                            Progress::new(part(format!("bar-{cix}")))
                                .small()
                                .value(score as f32)
                                .color(tier_color(tier_of(score, &self.thresholds)))
                                .accessibility_label(format!(
                                    "{}: {} of 100",
                                    one.name,
                                    score.round()
                                )),
                        )
                });
                let tag = match tiers[ix] {
                    Some(0) => Tag::info(),
                    Some(_) => Tag::secondary(),
                    None => Tag::warning(),
                };
                v_flex()
                    .id(part("breakdown".into()))
                    .gap_3()
                    .role(Role::Status)
                    .aria_label(spoken(
                        candidate,
                        &category_name(ix),
                        totals[ix],
                        tier_label(ix).as_deref(),
                        &self.criteria,
                    ))
                    .child(
                        h_flex()
                            .items_start()
                            .justify_between()
                            .gap_2()
                            .child(
                                v_flex()
                                    .min_w_0()
                                    .child(
                                        div().truncate().text_label().child(candidate.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_caption()
                                            .text_color(quiet)
                                            .child(category_name(ix)),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(24.))
                                    .line_height(px(28.))
                                    .font_medium()
                                    .child(format!("{}", totals[ix].round())),
                            ),
                    )
                    .child(
                        h_flex().child(
                            tag.small()
                                .rounded_full()
                                .child(tier_label(ix).unwrap_or_else(|| "Below threshold".into())),
                        ),
                    )
                    .children(bars)
                    .into_any_element()
            }
            None => {
                let rows = order.iter().take(LISTED).enumerate().map(|(rank, ix)| {
                    let ix = *ix;
                    let state = state.clone();
                    let hover_state = state.clone();
                    let ids = ids.clone();
                    let select = self.on_select.clone();
                    h_flex()
                        .id(part(format!("row-{ix}")))
                        .test_support()
                        .gap_2()
                        .px_2()
                        .py(px(6.))
                        .rounded(px(8.))
                        .cursor_pointer()
                        .role(Role::Button)
                        .aria_label(format!(
                            "{}, {}, score {}",
                            self.candidates[ix].name,
                            category_name(ix),
                            totals[ix].round()
                        ))
                        .tab_index(0)
                        .bg(wash.opacity(wash.a * row_fades[rank]))
                        .focus_visible(move |style| style.border_2().border_color(ring))
                        .on_hover(move |over, _, cx| {
                            let next = over.then_some(ix);
                            if *over || hover_state.read(cx).row_hover == Some(ix) {
                                hover_state.update(cx, |state, cx| {
                                    state.row_hover = next;
                                    cx.notify();
                                });
                            }
                        })
                        .on_click(move |_, window, cx| {
                            set_picked(&state, Some(ix), &ids, &select, window, cx)
                        })
                        .child(div().size(px(8.)).rounded_full().bg(colors[ix]))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div()
                                        .truncate()
                                        .text_label()
                                        .child(self.candidates[ix].name.clone()),
                                )
                                .child(
                                    div()
                                        .text_caption()
                                        .text_color(quiet)
                                        .child(category_name(ix)),
                                ),
                        )
                        .child(
                            div()
                                .font_family(mono.clone())
                                .text_label_small()
                                .child(format!("{}", totals[ix].round())),
                        )
                });
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .px_2()
                            .pb_1()
                            .text_eyebrow()
                            .text_color(quiet)
                            .child("TOP FITS"),
                    )
                    .children(rows)
                    .into_any_element()
            }
        };

        let legend = h_flex()
            .mt_3()
            .pt_3()
            .border_t_1()
            .border_color(border)
            .flex_wrap()
            .gap_x_4()
            .gap_y_1()
            .text_caption()
            .text_color(quiet)
            .children(self.thresholds.iter().enumerate().map(|(ix, one)| {
                h_flex()
                    .gap(px(6.))
                    .child(div().size(px(8.)).rounded_full().bg(tier_color(Some(ix))))
                    .child(format!("{} ({}+)", one.label, one.score.round()))
            }))
            .child(
                h_flex()
                    .gap(px(6.))
                    .child(div().size(px(8.)).rounded_full().bg(tier_color(None)))
                    .child("Below"),
            )
            .child(div().ml_auto().child("Nearer the centre fits better"));

        shell(cx)
            .id(self.id.clone())
            .max_w(px(672.))
            .child(shell_header(
                Some(Icon::new(CatalogIcon::Radar)),
                self.title.clone(),
                self.subtitle.clone().map(|line| line.into_any_element()),
                Some(
                    div()
                        .font_family(mono.clone())
                        .text_caption()
                        .text_color(quiet)
                        .child(format!("{count} candidates"))
                        .into_any_element(),
                ),
                cx,
            ))
            .child(
                panel(cx)
                    .p_4()
                    .child(
                        h_flex()
                            .flex_wrap()
                            .items_start()
                            .gap_4()
                            .child(field_box)
                            .child(div().flex_1().min_w(px(220.)).child(side)),
                    )
                    .child(legend),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn criteria() -> Vec<Criterion> {
        vec![
            Criterion::new("Skills", 2.),
            Criterion::new("Experience", 1.),
        ]
    }

    #[test]
    fn the_score_is_a_weighted_mean() {
        assert_eq!(overall(&[90., 60.], &criteria()), 80.);
        assert_eq!(overall(&[90.], &criteria()), 60.);
        assert_eq!(overall(&[90., 60.], &[]), 0.);
    }

    #[test]
    fn better_scores_sit_nearer_the_middle() {
        assert_eq!(radius_for(100.), CORE);
        assert_eq!(radius_for(0.), RIM);
        assert!(radius_for(80.) < radius_for(60.));
        assert_eq!(radius_for(140.), CORE);
    }

    #[test]
    fn tiers_follow_the_thresholds() {
        let thresholds = [
            Threshold::new(80., "Strong"),
            Threshold::new(60., "Possible"),
        ];
        assert_eq!(tier_of(85., &thresholds), Some(0));
        assert_eq!(tier_of(80., &thresholds), Some(0));
        assert_eq!(tier_of(70., &thresholds), Some(1));
        assert_eq!(tier_of(40., &thresholds), None);
    }

    #[test]
    fn candidates_fan_out_inside_their_sector() {
        // Two sectors: the first spans 0 to 0.5, kept 10% clear of each edge.
        assert!((sector_turn(0, 2, 0, 1) - 0.25).abs() < 1e-6);
        assert!((sector_turn(1, 2, 0, 2) - 0.65).abs() < 1e-6);
        assert!((sector_turn(1, 2, 1, 2) - 0.85).abs() < 1e-6);
        let candidates = vec![
            Candidate::new("a", "A", 0, [100., 100.]),
            Candidate::new("b", "B", 1, [0., 0.]),
        ];
        let totals = [100., 0.];
        let spots = places(&candidates, 2, &totals);
        // A perfect score in the first sector sits just right of the middle.
        let (x, y) = field_point(0.25, CORE);
        assert!((spots[0].0 - x).abs() < 1e-3 && (spots[0].1 - y).abs() < 1e-3);
        // A zero score in the second sits on the rim, at its middle (the left).
        assert!((spots[1].0 - (CENTRE - RIM)).abs() < 1e-3);
        assert!((spots[1].1 - CENTRE).abs() < 1e-3);
    }

    #[test]
    fn the_pointer_finds_the_nearest_candidate() {
        let spots = [(100., 100.), (106., 100.), (200., 200.)];
        assert_eq!(nearest(&spots, 104., 100., 9.), Some(1));
        assert_eq!(nearest(&spots, 150., 150., 9.), None);
        assert_eq!(ranked(&[40., 90., 70.]), [1, 2, 0]);
    }

    #[test]
    fn a_candidate_is_said_with_their_breakdown() {
        let one = Candidate::new("a", "Ana", 0, [90., 60.]);
        assert_eq!(
            spoken(&one, "Design", 80., Some("Strong fit"), &criteria()),
            "Ana, Design, score 80, strong fit. Skills 90, Experience 60"
        );
        assert_eq!(
            spoken(&one, "Design", 40., None, &criteria()),
            "Ana, Design, score 40, below every threshold. Skills 90, Experience 60"
        );
    }
}
