// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Negotiation range card: where an offer sits against the market and your
//! own three numbers.
//!
//! The market is a spread of comparable offers, one dot each, stacked in
//! columns by band. Three marked lines carry your walk-away, your target and
//! your ask, and a bold line carries the offer on the table. Under the plot,
//! a bar splits the range into what each stretch means, and a readout says in
//! words where the offer lands: how far from target and walk-away, and how
//! much of the market it beats. Point at a band, or step with the arrow
//! keys, to read its range and count.

use std::{cell::Cell, f32::consts::TAU, rc::Rc};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::{Presence, TestSupportExt as _, Transition};
use isonomic_ui::component::{
    ActiveTheme as _, Icon, Sizable as _, StyledExt as _, TextRole as _, h_flex, look, tag::Tag,
    v_flex,
};
use isonomic_ui::{
    App, Bounds, ElementId, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, PathBuilder, Pixels, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, canvas, div, point,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::{panel, shell, shell_header};

/// Distance between stacked dots and their radius.
const STEP: f32 = 9.;
const RADIUS: f32 = 3.;
/// The tallest a column grows before one dot stands for several offers.
const MAX_ROWS: usize = 12;
/// The lane above the plot for the offer's flag, and the lane under it for
/// the three marker labels.
const FLAG: f32 = 28.;
const LABELS: f32 = 38.;
/// How wide a marker label is taken to be when keeping labels apart.
const LABEL_W: f32 = 84.;

/// Which way is good for you.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Side {
    /// You are selling or being paid: higher is better.
    #[default]
    Selling,
    /// You are buying or paying: lower is better.
    Buying,
}

/// Where the offer lands against your three numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// Worse than your walk-away.
    PastWalkAway,
    /// Acceptable, but short of your target.
    ShortOfTarget,
    /// At or better than your target, short of your ask.
    AtTarget,
    /// At or better than your ask.
    AtAsk,
}

impl Standing {
    /// A short headline for the readout.
    pub fn headline(self) -> &'static str {
        match self {
            Standing::PastWalkAway => "Past your walk-away",
            Standing::ShortOfTarget => "Workable, short of target",
            Standing::AtTarget => "At or beyond target",
            Standing::AtAsk => "At or beyond your ask",
        }
    }

    /// Where it sits, in words for a screen reader.
    pub fn spoken(self) -> &'static str {
        match self {
            Standing::PastWalkAway => "worse than your walk-away",
            Standing::ShortOfTarget => "between walk-away and target",
            Standing::AtTarget => "between target and ask",
            Standing::AtAsk => "at or beyond your ask",
        }
    }
}

/// Where `offer` lands. On the selling side bigger is better; buying flips it.
pub fn standing(offer: f64, walk_away: f64, target: f64, ask: f64, side: Side) -> Standing {
    let good = |value: f64| match side {
        Side::Selling => value,
        Side::Buying => -value,
    };
    let offer = good(offer);
    if offer >= good(ask) {
        Standing::AtAsk
    } else if offer >= good(target) {
        Standing::AtTarget
    } else if offer >= good(walk_away) {
        Standing::ShortOfTarget
    } else {
        Standing::PastWalkAway
    }
}

/// The first band's start and how many bands of `width` cover `low` to
/// `high`.
pub fn band_edges(low: f64, high: f64, width: f64) -> (f64, usize) {
    let width = width.max(1e-9);
    let start = (low / width).floor() * width;
    let count = ((high - start) / width).floor() as usize + 1;
    (start, count.max(1))
}

/// How many values fall in each band.
pub fn band_counts(values: &[f64], start: f64, width: f64, count: usize) -> Vec<usize> {
    let mut counts = vec![0; count.max(1)];
    for value in values {
        let at = ((value - start) / width).floor().max(0.) as usize;
        let last = counts.len() - 1;
        counts[at.min(last)] += 1;
    }
    counts
}

/// The share of `values` the offer beats, counting ties as half. On the
/// buying side, beating means being lower.
pub fn share_beaten(values: &[f64], offer: f64, side: Side) -> f64 {
    if values.is_empty() {
        return 0.;
    }
    let score: f64 = values
        .iter()
        .map(|value| {
            let better = match side {
                Side::Selling => offer > *value,
                Side::Buying => offer < *value,
            };
            if (offer - value).abs() < 1e-9 {
                0.5
            } else if better {
                1.
            } else {
                0.
            }
        })
        .sum();
    score / values.len() as f64
}

/// Label centres pushed apart to at least `gap`, kept between `low` and
/// `high`, in the order given (which must be sorted).
pub fn spread_labels(centres: &[f32], gap: f32, low: f32, high: f32) -> Vec<f32> {
    let mut out: Vec<f32> = centres.to_vec();
    for ix in 0..out.len() {
        out[ix] = out[ix].max(low);
        if ix > 0 {
            out[ix] = out[ix].max(out[ix - 1] + gap);
        }
    }
    for ix in (0..out.len()).rev() {
        out[ix] = out[ix].min(high);
        if ix + 1 < out.len() {
            out[ix] = out[ix].min(out[ix + 1] - gap);
        }
    }
    out
}

/// A compact amount: 118000 as "118k", 1250000 as "1.25M", 640 as "640".
pub fn compact(value: f64) -> String {
    let trim = |text: String| {
        if text.contains('.') {
            text.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            text
        }
    };
    let size = value.abs();
    let sign = if value < 0. { "-" } else { "" };
    if size >= 1_000_000. {
        format!("{sign}{}M", trim(format!("{:.2}", size / 1_000_000.)))
    } else if size >= 1_000. {
        format!("{sign}{}k", trim(format!("{:.1}", size / 1_000.)))
    } else {
        format!("{sign}{}", trim(format!("{size:.1}")))
    }
}

/// How far the offer is from a mark, such as "7k below target".
pub fn gap_words(offer: f64, mark: f64, name: &str, format: &dyn Fn(f64) -> String) -> String {
    let gap = offer - mark;
    if gap.abs() < 1e-9 {
        format!("right on {name}")
    } else if gap > 0. {
        format!("{} above {name}", format(gap))
    } else {
        format!("{} below {name}", format(-gap))
    }
}

type Format = Rc<dyn Fn(f64) -> String>;
type SelectHandler = Rc<dyn Fn(f64, f64, usize, &mut Window, &mut App)>;

/// The market as columns of dots with walk-away, target, ask and the offer.
#[derive(IntoElement)]
pub struct NegotiationRangeCard {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    subtitle: Option<SharedString>,
    market: Vec<f64>,
    walk_away: f64,
    target: f64,
    ask: f64,
    offer: f64,
    band: Option<f64>,
    side: Side,
    unit: SharedString,
    format: Format,
    on_select: Option<SelectHandler>,
}

impl NegotiationRangeCard {
    /// Create a card from comparable market offers and the offer on the
    /// table. Set your three numbers with [`Self::marks`].
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        market: Vec<f64>,
        offer: f64,
    ) -> Self {
        let mut sorted = market.clone();
        sorted.sort_by(f64::total_cmp);
        let at = |q: f64| sorted.get(((sorted.len() as f64 - 1.) * q).round().max(0.) as usize);
        let (walk_away, target, ask) = (
            at(0.25).copied().unwrap_or(offer),
            at(0.6).copied().unwrap_or(offer),
            at(0.85).copied().unwrap_or(offer),
        );
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            subtitle: None,
            market,
            walk_away,
            target,
            ask,
            offer,
            band: None,
            side: Side::Selling,
            unit: "offers".into(),
            format: Rc::new(compact),
            on_select: None,
        }
    }

    /// A quieter line under the title.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Your walk-away, target and ask (the anchor you open with).
    pub fn marks(mut self, walk_away: f64, target: f64, ask: f64) -> Self {
        self.walk_away = walk_away;
        self.target = target;
        self.ask = ask;
        self
    }

    /// How wide one band of the market is. By default the range is cut in
    /// about sixteen.
    pub fn band(mut self, width: f64) -> Self {
        self.band = (width > 0.).then_some(width);
        self
    }

    /// Which way is good for you. Selling by default.
    pub fn side(mut self, side: Side) -> Self {
        self.side = side;
        self
    }

    /// What one market data point is called, in the plural. "offers" by
    /// default.
    pub fn unit(mut self, unit: impl Into<SharedString>) -> Self {
        self.unit = unit.into();
        self
    }

    /// How an amount is written, such as with a currency sign.
    pub fn format(mut self, format: impl Fn(f64) -> String + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }

    /// Run with a band's start, end and count when it is pointed at or
    /// stepped to.
    pub fn on_select(
        mut self,
        handler: impl Fn(f64, f64, usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Styled for NegotiationRangeCard {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

fn paint_dot(window: &mut Window, x: Pixels, y: Pixels, r: f32, color: Hsla) {
    let ring: Vec<_> = (0..12)
        .map(|step| {
            let angle = step as f32 / 12. * TAU;
            point(x + px(r * angle.cos()), y + px(r * angle.sin()))
        })
        .collect();
    let mut dot = PathBuilder::fill();
    dot.add_polygon(&ring, true);
    if let Ok(dot) = dot.build() {
        window.paint_path(dot, color);
    }
}

impl RenderOnce for NegotiationRangeCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let part = |name: String| ElementId::NamedChild(self.id.clone().into(), name.into());
        let format = self.format.clone();
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let data = theme.chart_1;
        let danger = theme.danger;
        let warning = theme.warning;
        let success = theme.success;
        let raised = theme.popover;
        let ring = theme.ring;
        let surface = theme.background;
        let border = theme.border;
        let wash = look::ink::wash_strong(cx);
        let mono = theme.mono_font_family.clone();

        let marks = [self.walk_away, self.target, self.ask, self.offer];
        let low = self
            .market
            .iter()
            .chain(marks.iter())
            .copied()
            .fold(f64::INFINITY, f64::min);
        let high = self
            .market
            .iter()
            .chain(marks.iter())
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        let (low, high) = if low.is_finite() && high > low {
            (low, high)
        } else {
            (0., 1.)
        };
        let width = self.band.unwrap_or((high - low) / 16.);
        let (start, count) = band_edges(low, high, width);
        let end = start + width * count as f64;
        let counts = band_counts(&self.market, start, width, count);
        let peak = counts.iter().copied().max().unwrap_or(0).max(1);
        let per = peak.div_ceil(MAX_ROWS).max(1);
        let rows = peak.div_ceil(per).max(5);
        let plot_h = rows as f32 * STEP + STEP;
        let share = |value: f64| ((value - start) / (end - start)).clamp(0., 1.) as f32;

        let where_ = standing(self.offer, self.walk_away, self.target, self.ask, self.side);
        let beaten = share_beaten(&self.market, self.offer, self.side);
        let (tone_color, tag) = match where_ {
            Standing::PastWalkAway => (danger, Tag::danger()),
            Standing::ShortOfTarget => (warning, Tag::warning()),
            Standing::AtTarget | Standing::AtAsk => (success, Tag::success()),
        };

        let active = window.use_keyed_state(part("active".into()), cx, |_, _| None::<usize>);
        let picked = (*active.read(cx)).filter(|ix| *ix < count);
        let plot_bounds = window
            .use_keyed_state(part("bounds".into()), cx, |_, _| {
                Rc::new(Cell::new(Bounds::default()))
            })
            .read(cx)
            .clone();
        let measured = f32::from(plot_bounds.get().size.width);
        let plot_w = if measured > 0. { measured } else { 600. };
        let appear = std::time::Duration::from_millis(320 + 14 * count as u64);
        let elapsed = Presence::new(part("appear".into()), true)
            .transition(Transition::new(appear).ease(|time| time))
            .sample(window, cx)
            .progress
            * appear.as_secs_f32();

        // Each band's dots take the colour of the stretch its middle is in.
        let side = self.side;
        let tints: Vec<Hsla> = (0..count)
            .map(|ix| {
                let middle = start + width * (ix as f64 + 0.5);
                match standing(middle, self.walk_away, self.target, self.ask, side) {
                    Standing::PastWalkAway => ink.opacity(0.22),
                    Standing::ShortOfTarget => ink.opacity(0.45),
                    _ => data,
                }
            })
            .collect();
        let lines = [
            (share(self.walk_away), danger, Some([3., 3.]), 1.5),
            (share(self.target), success, None, 1.5),
            (share(self.ask), ink.opacity(0.6), Some([3., 3.]), 1.5),
            (share(self.offer), ink, None, 2.5),
        ];
        let paint_counts = counts.clone();
        let seen = plot_bounds;
        let plot = canvas(
            move |bounds, _, _| seen.set(bounds),
            move |bounds, _, window, _| {
                let w = f32::from(bounds.size.width);
                let col = w / count as f32;
                let radius = RADIUS.min(col * 0.36);
                for (ix, band) in paint_counts.iter().enumerate() {
                    let shown = ((elapsed - 0.014 * ix as f32) / 0.32).clamp(0., 1.);
                    let dim = if picked.is_some_and(|at| at != ix) {
                        0.45
                    } else {
                        1.
                    };
                    let x = bounds.left() + px((ix as f32 + 0.5) * col);
                    for k in 0..band.div_ceil(per) {
                        let y = bounds.top() + px(plot_h - STEP * (k as f32 + 1.));
                        let tint = tints[ix];
                        paint_dot(window, x, y, radius, tint.opacity(tint.a * shown * dim));
                    }
                }
                for (along, color, dash, wide) in lines {
                    let mut path = PathBuilder::stroke(px(wide));
                    if let Some([on, off]) = dash {
                        path = path.dash_array(&[px(on), px(off)]);
                    }
                    let x = bounds.left() + px(along * w);
                    path.move_to(point(x, bounds.top()));
                    path.line_to(point(x, bounds.top() + px(plot_h)));
                    if let Ok(path) = path.build() {
                        window.paint_path(path, color);
                    }
                }
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .w_full()
        .h(px(plot_h));

        // One invisible strip per band takes the pointer.
        let fades: Vec<f32> = (0..count)
            .map(|ix| {
                look::hover_fade(
                    part(format!("band-fade-{ix}")),
                    picked == Some(ix),
                    window,
                    cx,
                )
            })
            .collect();
        let strips = counts.iter().enumerate().map(|(ix, n)| {
            let active = active.clone();
            let select = self.on_select.clone();
            let n = *n;
            let from = start + width * ix as f64;
            div()
                .id(part(format!("band-{ix}")))
                .flex_1()
                .h_full()
                .rounded(px(6.))
                .bg(wash.opacity(wash.a * fades[ix]))
                .on_hover(move |over, window, cx| {
                    let next = (*over && n > 0).then_some(ix);
                    if *over || *active.read(cx) == Some(ix) {
                        active.update(cx, |state, cx| {
                            *state = next;
                            cx.notify();
                        });
                    }
                    if let (Some(select), true) = (&select, next.is_some()) {
                        select(from, from + width, n, window, cx);
                    }
                })
        });

        // The offer's flag above the plot.
        let offer_at = share(self.offer);
        let flag = div()
            .absolute()
            .top_0()
            .left(relative(offer_at))
            .w(px(0.))
            .flex()
            .map(|this| {
                if offer_at < 0.12 {
                    this.justify_start()
                } else if offer_at > 0.88 {
                    this.justify_end()
                } else {
                    this.justify_center()
                }
            })
            .child(
                h_flex()
                    .flex_none()
                    .gap_1()
                    .px_2()
                    .h(px(22.))
                    .rounded_full()
                    .bg(ink)
                    .text_color(surface)
                    .text_label_small()
                    .whitespace_nowrap()
                    .child("Offer")
                    .child(div().font_family(mono.clone()).child(format(self.offer))),
            );

        // The three marker labels under the plot, kept apart.
        let mut named = [
            ("Walk-away", self.walk_away, danger),
            ("Target", self.target, success),
            ("Ask", self.ask, ink.opacity(0.75)),
        ];
        named.sort_by(|a, b| a.1.total_cmp(&b.1));
        let half = LABEL_W / 2. / plot_w;
        let centres = spread_labels(
            &named.map(|(_, value, _)| share(value)),
            LABEL_W / plot_w,
            half,
            1. - half,
        );
        let labels = named
            .iter()
            .zip(centres)
            .map(|((name, value, color), along)| {
                div()
                    .absolute()
                    .top(px(plot_h + FLAG + 6.))
                    .left(relative(along))
                    .w(px(0.))
                    .flex()
                    .justify_center()
                    .child(
                        v_flex()
                            .flex_none()
                            .w(px(LABEL_W))
                            .items_center()
                            .child(
                                h_flex()
                                    .gap_1()
                                    .text_caption()
                                    .text_color(quiet)
                                    .child(div().w(px(8.)).h(px(2.)).rounded_full().bg(*color))
                                    .child(*name),
                            )
                            .child(
                                div()
                                    .font_family(mono.clone())
                                    .text_label_small()
                                    .child(format(*value)),
                            ),
                    )
            });

        // The range bar: what each stretch means, with the offer on it.
        let ordered = {
            let mut marks = [self.walk_away, self.target, self.ask];
            marks.sort_by(f64::total_cmp);
            marks
        };
        let segment_colors = match self.side {
            Side::Selling => [danger, warning, success, success],
            Side::Buying => [success, success, warning, danger],
        };
        let edges = [
            0.,
            share(ordered[0]),
            share(ordered[1]),
            share(ordered[2]),
            1.,
        ];
        let segments = (0..4).map(|ix| {
            let strength =
                if ix == 3 && self.side == Side::Selling || ix == 0 && self.side == Side::Buying {
                    0.4
                } else {
                    0.22
                };
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(relative(edges[ix]))
                .w(relative((edges[ix + 1] - edges[ix]).max(0.)))
                .bg(segment_colors[ix].opacity(strength))
        });
        let bar = div()
            .relative()
            .mt_2()
            .h(px(8.))
            .rounded_full()
            .overflow_hidden()
            .bg(wash)
            .children(segments);
        let bar_knob = div()
            .absolute()
            .top(px(-3.))
            .left(relative(offer_at))
            .ml(px(-7.))
            .size(px(14.))
            .rounded_full()
            .bg(surface)
            .border_2()
            .border_color(ink);
        let range = div()
            .relative()
            .child(bar)
            .child(
                div()
                    .absolute()
                    .top(px(8.))
                    .left_0()
                    .right_0()
                    .child(bar_knob),
            )
            .child(
                h_flex()
                    .mt_1()
                    .justify_between()
                    .font_family(mono.clone())
                    .text_size(px(10.))
                    .line_height(px(16.))
                    .text_color(quiet)
                    .child(format(start))
                    .child(format(end)),
            );

        let tip = picked.map(|ix| {
            let from = start + width * ix as f64;
            let along = (ix as f32 + 0.5) / count as f32;
            div()
                .absolute()
                .top(px(FLAG))
                .left(relative(along.min(0.7)))
                .ml(px(10.))
                .px_3()
                .py_2()
                .rounded(px(12.))
                .bg(raised)
                .shadow_md()
                .text_caption()
                .child(div().text_color(quiet).child(format!(
                    "{} to {}",
                    format(from),
                    format(from + width)
                )))
                .child(
                    div()
                        .font_family(mono.clone())
                        .child(format!("{} {}", counts[ix], self.unit)),
                )
        });

        let near_target = gap_words(self.offer, self.target, "target", &*format);
        let near_walk = gap_words(self.offer, self.walk_away, "walk-away", &*format);
        let beats = format!(
            "beats {}% of {} {}",
            (beaten * 100.).round(),
            self.market.len(),
            self.unit
        );
        let filled: Vec<usize> = (0..count).filter(|ix| counts[*ix] > 0).collect();
        let spoken = format!(
            "{}: offer {} is {}. {near_target}, {near_walk}, {beats}.{} Arrow keys read each band.",
            self.title,
            format(self.offer),
            where_.spoken(),
            match picked {
                Some(ix) => format!(
                    " Band {} to {}: {} {}.",
                    format(start + width * ix as f64),
                    format(start + width * (ix as f64 + 1.)),
                    counts[ix],
                    self.unit
                ),
                None => String::new(),
            }
        );

        let key_state = active.clone();
        let key_select = self.on_select.clone();
        let key_counts = counts.clone();
        let focus = window
            .use_keyed_state(part("focus".into()), cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            })
            .read(cx)
            .clone();
        let focused = focus.is_focused(window);

        let chart = div()
            .id(part("plot".into()))
            .test_support()
            .track_focus(&focus)
            .relative()
            .w_full()
            .h(px(FLAG + plot_h + LABELS + 6.))
            .role(Role::Group)
            .aria_label(spoken)
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                if filled.is_empty() {
                    return;
                }
                let now = *key_state.read(cx);
                let at = now.and_then(|ix| filled.iter().position(|&band| band == ix));
                let last = filled.len() - 1;
                let next = match event.keystroke.key.as_str() {
                    "right" => Some(at.map_or(0, |at| (at + 1) % filled.len())),
                    "left" => Some(at.map_or(last, |at| (at + last) % filled.len())),
                    "home" => Some(0),
                    "end" => Some(last),
                    "escape" => None,
                    _ => return,
                };
                let next = next.map(|at| filled[at]);
                key_state.update(cx, |state, cx| {
                    *state = next;
                    cx.notify();
                });
                if let (Some(select), Some(ix)) = (&key_select, next) {
                    let from = start + width * ix as f64;
                    select(from, from + width, key_counts[ix], window, cx);
                }
                cx.stop_propagation();
            })
            .child(flag)
            .child(
                div()
                    .absolute()
                    .top(px(FLAG))
                    .left_0()
                    .w_full()
                    .h(px(plot_h))
                    .child(plot)
                    .child(h_flex().absolute().inset_0().children(strips)),
            )
            .children(labels)
            .children(tip)
            .when(focused, |this| {
                this.child(
                    div()
                        .absolute()
                        .top(px(-4.))
                        .bottom(px(-4.))
                        .left(px(-4.))
                        .right(px(-4.))
                        .rounded(px(8.))
                        .border_2()
                        .border_color(ring),
                )
            });

        let readout = h_flex()
            .mt_4()
            .pt_3()
            .gap_3()
            .items_start()
            .border_t_1()
            .border_color(border)
            .child(div().mt(px(6.)).size(px(10.)).rounded_full().bg(tone_color))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_2()
                            .child(div().text_label().child(where_.headline()))
                            .child(
                                tag.small()
                                    .rounded_full()
                                    .child(format!("Offer {}", format(self.offer))),
                            ),
                    )
                    .child(
                        div()
                            .text_caption()
                            .text_color(quiet)
                            .child(format!("{near_target} · {near_walk} · {beats}")),
                    ),
            );

        shell(cx)
            .id(self.id.clone())
            .max_w(px(672.))
            .child(shell_header(
                Some(Icon::new(CatalogIcon::Scales)),
                self.title.clone(),
                self.subtitle.clone().map(|line| line.into_any_element()),
                Some(
                    div()
                        .font_family(mono.clone())
                        .text_caption()
                        .text_color(quiet)
                        .child(format!("{} {}", self.market.len(), self.unit))
                        .into_any_element(),
                ),
                cx,
            ))
            .child(
                panel(cx)
                    .p_4()
                    .child(chart)
                    .child(range)
                    .child(readout)
                    .when(per > 1, |this| {
                        this.child(
                            div()
                                .mt_2()
                                .text_caption()
                                .text_color(quiet)
                                .child(format!("1 dot = {per} {}", self.unit)),
                        )
                    }),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_offer_lands_against_the_three_marks() {
        let at = |offer| standing(offer, 90., 110., 130., Side::Selling);
        assert_eq!(at(80.), Standing::PastWalkAway);
        assert_eq!(at(90.), Standing::ShortOfTarget);
        assert_eq!(at(115.), Standing::AtTarget);
        assert_eq!(at(130.), Standing::AtAsk);
        // Buying flips which way is good.
        let buy = |offer| standing(offer, 130., 110., 90., Side::Buying);
        assert_eq!(buy(140.), Standing::PastWalkAway);
        assert_eq!(buy(120.), Standing::ShortOfTarget);
        assert_eq!(buy(100.), Standing::AtTarget);
        assert_eq!(buy(85.), Standing::AtAsk);
    }

    #[test]
    fn bands_cover_the_range_on_round_edges() {
        assert_eq!(band_edges(93., 141., 5.), (90., 11));
        assert_eq!(band_edges(90., 140., 5.), (90., 11));
        let counts = band_counts(&[90., 94.9, 95., 140.], 90., 5., 11);
        assert_eq!(counts[0], 2);
        assert_eq!(counts[1], 1);
        assert_eq!(counts[10], 1);
        assert_eq!(counts.iter().sum::<usize>(), 4);
    }

    #[test]
    fn the_share_beaten_counts_ties_as_half() {
        let market = [100., 110., 120., 130.];
        assert_eq!(share_beaten(&market, 115., Side::Selling), 0.5);
        assert_eq!(share_beaten(&market, 110., Side::Selling), 0.375);
        assert_eq!(share_beaten(&market, 105., Side::Buying), 0.75);
        assert_eq!(share_beaten(&[], 1., Side::Selling), 0.);
    }

    #[test]
    fn labels_are_kept_apart_and_inside() {
        let out = spread_labels(&[0.5, 0.52, 0.9], 0.1, 0.05, 0.95);
        assert!((out[0] - 0.5).abs() < 1e-6 && (out[1] - 0.6).abs() < 1e-6);
        assert!((out[2] - 0.9).abs() < 1e-6);
        // Crowded at the right end, they back off to the left.
        let out = spread_labels(&[0.9, 0.92, 0.94], 0.1, 0.05, 0.95);
        assert!((out[2] - 0.95).abs() < 1e-6);
        assert!((out[1] - 0.85).abs() < 1e-6);
        assert!((out[0] - 0.75).abs() < 1e-6);
        // At the left edge they are pushed inside.
        assert!((spread_labels(&[0.], 0.1, 0.05, 0.95)[0] - 0.05).abs() < 1e-6);
    }

    #[test]
    fn amounts_are_compact_and_gaps_are_said() {
        assert_eq!(compact(118_000.), "118k");
        assert_eq!(compact(118_500.), "118.5k");
        assert_eq!(compact(1_250_000.), "1.25M");
        assert_eq!(compact(640.), "640");
        assert_eq!(compact(-7_000.), "-7k");
        let format = |value: f64| compact(value);
        assert_eq!(
            gap_words(118_000., 125_000., "target", &format),
            "7k below target"
        );
        assert_eq!(
            gap_words(118_000., 95_000., "walk-away", &format),
            "23k above walk-away"
        );
        assert_eq!(gap_words(5., 5., "target", &format), "right on target");
    }
}
