// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Radial dot cycle: a daily or weekly rhythm as rings of dots round a dial.
//!
//! The cycle runs clockwise from the top, one spoke per slot. Each spoke fills
//! outward with dots, coloured by series, so the busiest slots reach the rim.
//! The middle shows a headline until a slot is picked with the pointer or the
//! arrow keys; then it shows that slot's label and total.

use std::{cell::Cell, f32::consts::TAU, rc::Rc};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::{Presence, TestSupportExt as _, Transition};
use isonomic_ui::component::{ActiveTheme as _, Icon, StyledExt as _, TextRole as _, h_flex};
use isonomic_ui::{
    App, Bounds, ElementId, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent,
    MouseMoveEvent, ParentElement as _, PathBuilder, Pixels, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, canvas, div, point,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::{panel, shell, shell_header};

/// The dial is laid out on a 340 box (300 plus room for labels) and scaled.
const BOX: f32 = 340.;
const PAD: f32 = 20.;
const CENTRE: f32 = 150.;
const INNER: f32 = 70.;
const RINGS: usize = 8;
const RING: f32 = 8.;
const LANES: usize = 3;

/// One series of values round the cycle.
#[derive(Clone)]
pub struct CycleSeries {
    name: SharedString,
    values: Vec<f64>,
    color: Option<Hsla>,
}

impl CycleSeries {
    /// A series with one value per slot.
    pub fn new(name: impl Into<SharedString>, values: impl IntoIterator<Item = f64>) -> Self {
        Self {
            name: name.into(),
            values: values.into_iter().map(|value| value.max(0.)).collect(),
            color: None,
        }
    }

    /// Its colour. By default the first series takes the data colour, the
    /// second the ink, the third a quieter ink.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }
}

/// A point `turn` of the way round (0 at the top, clockwise), `radius` from
/// the middle of the 300 dial.
pub fn dial_point(turn: f32, radius: f32) -> (f32, f32) {
    let angle = turn * TAU - TAU / 4.;
    (CENTRE + radius * angle.cos(), CENTRE + radius * angle.sin())
}

/// The total of every series in each slot.
pub fn slot_totals(slots: usize, series: &[CycleSeries]) -> Vec<f64> {
    (0..slots)
        .map(|ix| {
            series
                .iter()
                .map(|one| one.values.get(ix).copied().unwrap_or(0.))
                .sum()
        })
        .collect()
}

/// Which series fills each of a slot's 24 dot places, innermost first, when
/// one dot stands for `per`.
pub fn slot_dots(slot: usize, series: &[CycleSeries], per: f64) -> Vec<Option<usize>> {
    let mut places = vec![None; RINGS * LANES];
    let mut at = 0;
    for (ix, one) in series.iter().enumerate() {
        let dots = (one.values.get(slot).copied().unwrap_or(0.) / per).round() as usize;
        for _ in 0..dots {
            if at >= places.len() {
                return places;
            }
            places[at] = Some(ix);
            at += 1;
        }
    }
    places
}

/// The slot under a point of the 300 dial, if it lies in the ring of dots.
pub fn slot_at(x: f32, y: f32, slots: usize) -> Option<usize> {
    let (dx, dy) = (x - CENTRE, y - CENTRE);
    let distance = (dx * dx + dy * dy).sqrt();
    if slots == 0 || !(INNER..=CENTRE).contains(&distance) {
        return None;
    }
    let turn = (dy.atan2(dx) + TAU / 4.).rem_euclid(TAU) / TAU;
    Some(((turn * slots as f32) as usize).min(slots - 1))
}

type Format = Rc<dyn Fn(f64) -> String>;
type SelectHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// A cycle as rings of dots.
#[derive(IntoElement)]
pub struct RadialDotCycle {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    subtitle: Option<SharedString>,
    labels: Vec<SharedString>,
    series: Vec<CycleSeries>,
    headline: SharedString,
    caption: Option<SharedString>,
    axis: Option<Vec<usize>>,
    format: Format,
    on_select: Option<SelectHandler>,
}

impl RadialDotCycle {
    /// Create a dial with one label per slot and the headline for its middle.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        labels: Vec<SharedString>,
        headline: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            subtitle: None,
            labels,
            series: Vec::new(),
            headline: headline.into(),
            caption: None,
            axis: None,
            format: Rc::new(|value| format!("{}", value.round() as i64)),
            on_select: None,
        }
    }

    /// A quieter line under the title.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Add a series.
    pub fn series(mut self, series: CycleSeries) -> Self {
        self.series.push(series);
        self
    }

    /// A line under the headline.
    pub fn caption(mut self, caption: impl Into<SharedString>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    /// Which slots get a label on the rim. By default the four quarters.
    pub fn axis(mut self, slots: impl IntoIterator<Item = usize>) -> Self {
        self.axis = Some(slots.into_iter().collect());
        self
    }

    /// How a slot's total is written.
    pub fn format(mut self, format: impl Fn(f64) -> String + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }

    /// Run with the slot the reader points at or steps to.
    pub fn on_select(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Styled for RadialDotCycle {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

fn paint_dot(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    scale: f32,
    at: (f32, f32),
    r: f32,
    color: Hsla,
) {
    let ring: Vec<_> = (0..10)
        .map(|step| {
            let angle = step as f32 / 10. * TAU;
            point(
                bounds.left() + px((PAD + at.0 + r * angle.cos()) * scale),
                bounds.top() + px((PAD + at.1 + r * angle.sin()) * scale),
            )
        })
        .collect();
    let mut dot = PathBuilder::fill();
    dot.add_polygon(&ring, true);
    if let Ok(dot) = dot.build() {
        window.paint_path(dot, color);
    }
}

impl RenderOnce for RadialDotCycle {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let part = |name: &'static str| ElementId::NamedChild(self.id.clone().into(), name.into());
        let slots = self.labels.len();
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let mono = theme.mono_font_family.clone();
        let palette = [theme.chart_1, ink, ink.opacity(0.45)];
        let colors: Vec<Hsla> = self
            .series
            .iter()
            .enumerate()
            .map(|(ix, one)| one.color.unwrap_or(palette[ix % palette.len()]))
            .collect();
        let totals = slot_totals(slots, &self.series);
        let peak = totals.iter().copied().fold(1., f64::max);
        let per = peak / (RINGS * LANES) as f64;
        let axis: Vec<usize> = self.axis.clone().unwrap_or_else(|| {
            (0..4)
                .filter(|quarter| slots > 0 && (quarter * slots) % 4 == 0)
                .map(|quarter| quarter * slots / 4)
                .collect()
        });

        let active = window.use_keyed_state(part("active"), cx, |_, _| None::<usize>);
        let picked = (*active.read(cx)).filter(|ix| *ix < slots);
        let dial_bounds = window
            .use_keyed_state(part("bounds"), cx, |_, _| {
                Rc::new(Cell::new(Bounds::default()))
            })
            .read(cx)
            .clone();
        let appear = std::time::Duration::from_millis(300 + 20 * slots as u64);
        let appeared = Presence::new(part("appear"), true)
            .transition(Transition::new(appear).ease(|time| time))
            .sample(window, cx)
            .progress;
        let elapsed = appeared * appear.as_secs_f32();

        let places: Vec<Vec<Option<usize>>> = (0..slots)
            .map(|ix| slot_dots(ix, &self.series, per))
            .collect();
        let paint_colors = colors.clone();
        let paint_axis = axis.clone();
        let seen = dial_bounds.clone();
        let dial = canvas(
            move |bounds, _, _| seen.set(bounds),
            move |bounds, _, window, _| {
                let scale = f32::from(bounds.size.width) / BOX;
                let at = |(x, y): (f32, f32)| {
                    point(
                        bounds.left() + px((PAD + x) * scale),
                        bounds.top() + px((PAD + y) * scale),
                    )
                };
                for ix in 0..slots {
                    let turn = ix as f32 / slots as f32;
                    let mut tick = PathBuilder::stroke(px(1.25 * scale));
                    tick.move_to(at(dial_point(turn, CENTRE - 2.)));
                    tick.line_to(at(dial_point(turn, CENTRE + 4.)));
                    if let Ok(tick) = tick.build() {
                        let strength = if paint_axis.contains(&ix) { 0.7 } else { 0.3 };
                        window.paint_path(tick, ink.opacity(strength));
                    }
                    // Spokes fade in clockwise, 20ms apart.
                    let shown = ((elapsed - 0.02 * ix as f32) / 0.3).clamp(0., 1.);
                    let dim = if picked.is_some_and(|at| at != ix) {
                        0.4
                    } else {
                        1.
                    };
                    for (place, series) in places[ix].iter().enumerate() {
                        let lane = (place % LANES) as f32;
                        let ring = (place / LANES) as f32;
                        let spot = dial_point(
                            (ix as f32 + (lane + 0.5) / LANES as f32) / slots as f32,
                            INNER + 6. + ring * RING,
                        );
                        match series {
                            Some(series) => paint_dot(
                                window,
                                bounds,
                                scale,
                                spot,
                                2.6,
                                paint_colors[*series]
                                    .opacity(paint_colors[*series].a * shown * dim),
                            ),
                            None => paint_dot(
                                window,
                                bounds,
                                scale,
                                spot,
                                1.1,
                                ink.opacity(0.15 * shown * dim),
                            ),
                        }
                    }
                }
            },
        )
        .w_full()
        .aspect_ratio(1.);

        let rim_labels = axis.iter().filter_map(|&ix| {
            let label = self.labels.get(ix)?.clone();
            let (x, y) = dial_point(ix as f32 / slots as f32, CENTRE + 16.);
            Some(
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
                            .font_family(mono.clone())
                            .text_size(px(12.))
                            .child(label),
                    ),
            )
        });

        let format = self.format.clone();
        let middle = div()
            .absolute()
            .inset_0()
            .v_flex()
            .items_center()
            .justify_center()
            .map(|this| match picked {
                None => this
                    .child(
                        div()
                            .text_size(px(24.))
                            .line_height(px(32.))
                            .font_medium()
                            .child(self.headline.clone()),
                    )
                    .when_some(self.caption.clone(), |this, caption| {
                        this.child(div().text_caption().text_color(quiet).child(caption))
                    }),
                Some(ix) => this
                    .child(
                        div()
                            .font_family(mono.clone())
                            .text_caption()
                            .text_color(quiet)
                            .child(self.labels[ix].clone()),
                    )
                    .child(
                        div()
                            .text_size(px(20.))
                            .line_height(px(28.))
                            .font_medium()
                            .child(format(totals[ix])),
                    ),
            });

        let busiest = totals
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(ix, total)| (self.labels[ix].clone(), format(*total)));
        let spoken = match (picked, busiest) {
            (Some(ix), _) => format!(
                "{}: {}, {}",
                self.title,
                self.labels[ix],
                format(totals[ix])
            ),
            (None, Some((label, total))) => {
                format!(
                    "{}: busiest {label} with {total}. Arrow keys read each slot.",
                    self.title
                )
            }
            (None, None) => self.title.to_string(),
        };

        let hover_state = active.clone();
        let hover_select = self.on_select.clone();
        let leave_state = active.clone();
        let key_state = active;
        let key_select = self.on_select.clone();
        let legend = (self.series.len() > 1).then(|| {
            h_flex()
                .mt_3()
                .flex_wrap()
                .justify_center()
                .gap_x_4()
                .gap_y_1()
                .text_caption()
                .text_color(quiet)
                .children(self.series.iter().zip(&colors).map(|(one, color)| {
                    h_flex()
                        .gap(px(6.))
                        .child(div().size(px(8.)).rounded_full().bg(*color))
                        .child(one.name.clone())
                }))
        });

        shell(cx)
            .id(self.id.clone())
            .max_w(px(384.))
            .child(shell_header(
                Some(Icon::new(CatalogIcon::ChartRadial)),
                self.title.clone(),
                self.subtitle.clone().map(|line| line.into_any_element()),
                None,
                cx,
            ))
            .child(
                panel(cx)
                    .p_4()
                    .child(
                        div()
                            .id(part("dial"))
                            .test_support()
                            .relative()
                            .mx_auto()
                            .w_full()
                            .max_w(px(288.))
                            .role(Role::Group)
                            .aria_label(spoken)
                            .tab_index(0)
                            .on_mouse_move(move |event: &MouseMoveEvent, window, cx| {
                                let bounds = dial_bounds.get();
                                let scale = f32::from(bounds.size.width) / BOX;
                                if scale <= 0. {
                                    return;
                                }
                                let x = f32::from(event.position.x - bounds.left()) / scale - PAD;
                                let y = f32::from(event.position.y - bounds.top()) / scale - PAD;
                                let next = slot_at(x, y, slots);
                                if *hover_state.read(cx) != next {
                                    hover_state.update(cx, |state, cx| {
                                        *state = next;
                                        cx.notify();
                                    });
                                    if let (Some(select), Some(ix)) = (&hover_select, next) {
                                        select(ix, window, cx);
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
                                if slots == 0 {
                                    return;
                                }
                                let now = *key_state.read(cx);
                                let next = match event.keystroke.key.as_str() {
                                    "right" | "down" => Some(now.map_or(0, |at| (at + 1) % slots)),
                                    "left" | "up" => {
                                        Some(now.map_or(slots - 1, |at| (at + slots - 1) % slots))
                                    }
                                    "home" => Some(0),
                                    "end" => Some(slots - 1),
                                    "escape" => None,
                                    _ => return,
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
                            .child(dial)
                            .children(rim_labels)
                            .child(middle),
                    )
                    .children(legend),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn week() -> Vec<CycleSeries> {
        vec![
            CycleSeries::new("Walk-ins", [12., 0., 24., 6.]),
            CycleSeries::new("Bookings", [12., 6., 0., 0.]),
        ]
    }

    #[test]
    fn the_cycle_starts_at_the_top_and_runs_clockwise() {
        let (x, y) = dial_point(0., 100.);
        assert!((x - 150.).abs() < 0.01 && (y - 50.).abs() < 0.01);
        let (x, y) = dial_point(0.25, 100.);
        assert!((x - 250.).abs() < 0.01 && (y - 150.).abs() < 0.01);
    }

    #[test]
    fn dots_fill_outward_series_by_series() {
        let series = week();
        assert_eq!(slot_totals(4, &series), [24., 6., 24., 6.]);
        // The peak is 24 over 24 places: one dot each.
        let first = slot_dots(0, &series, 1.);
        assert_eq!(first.iter().filter(|dot| **dot == Some(0)).count(), 12);
        assert_eq!(first[12], Some(1));
        assert!(first.iter().all(Option::is_some));
        let second = slot_dots(1, &series, 1.);
        assert_eq!(second[5], Some(1));
        assert_eq!(second[6], None);
        // More than fits is cut at the rim.
        assert_eq!(slot_dots(0, &series, 0.5).len(), 24);
    }

    #[test]
    fn a_point_in_the_ring_names_its_slot() {
        // Just right of the top, in the ring: the first of four slots.
        assert_eq!(slot_at(160., 50., 4), Some(0));
        // Right of the middle, below the axis: the second.
        assert_eq!(slot_at(250., 160., 4), Some(1));
        // Left of the top: the last.
        assert_eq!(slot_at(140., 50., 4), Some(3));
        // The middle and beyond the rim belong to no slot.
        assert_eq!(slot_at(150., 150., 4), None);
        assert_eq!(slot_at(150., -20., 4), None);
        assert_eq!(slot_at(160., 50., 0), None);
    }
}
