// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Dot histogram: a spread of samples where every dot is a share of them.
//!
//! Samples fall into columns; each column stacks its dots outward from the
//! middle, so the shape reads like a violin. The median is a solid line. With
//! a limit, a dashed line marks it and the columns at or past it take the
//! danger colour. Pointing at a column, or stepping with the arrow keys,
//! shows its range, count and share.

use std::rc::Rc;

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::{Presence, TestSupportExt as _, Transition};
use isonomic_ui::component::{ActiveTheme as _, Icon, StyledExt as _, TextRole as _, h_flex, look};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    PathBuilder, RenderOnce, Role, SharedString, StatefulInteractiveElement as _, StyleRefinement,
    Styled, Window, canvas, div, point, prelude::FluentBuilder as _, px, relative,
};

use crate::common::{panel, shell, shell_header};

/// Vertical distance between dots, and their radius.
const STEP: f32 = 7.;
const RADIUS: f32 = 2.4;
/// The tallest a column grows before one dot stands for several samples.
const MAX_ROWS: usize = 18;
/// Room under the plot for the axis labels.
const AXIS: f32 = 22.;

/// One column of the histogram.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bin {
    /// Where its range starts.
    pub from: f64,
    /// Where its range ends.
    pub to: f64,
    /// How many samples fall in it.
    pub count: usize,
    /// It sits at or past the limit.
    pub flagged: bool,
}

/// The value below which the share `q` (0 to 1) of sorted samples fall.
pub fn quantile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.;
    }
    let at = (sorted.len() - 1) as f64 * q;
    let low = at.floor() as usize;
    let high = at.ceil() as usize;
    sorted[low] + (sorted[high] - sorted[low]) * (at - low as f64)
}

/// Sort samples into `count` columns. With a limit, a column edge falls
/// exactly on it, so no column straddles the line.
pub fn bins(values: &[f64], count: usize, limit: Option<f64>) -> Vec<Bin> {
    let count = count.max(1);
    let highest = values.iter().copied().fold(0., f64::max);
    let top = match limit {
        Some(limit) => highest.max(limit * 1.04),
        None => highest,
    };
    let top = if top > 0. { top } else { 1. };
    let size = match limit {
        Some(limit) if limit < top => limit / (count as f64 * limit / top).round().max(1.),
        _ => top / count as f64,
    };
    let mut bins: Vec<Bin> = (0..count)
        .map(|ix| Bin {
            from: ix as f64 * size,
            to: (ix + 1) as f64 * size,
            count: 0,
            flagged: limit.is_some_and(|limit| ix as f64 * size >= limit - size / 1000.),
        })
        .collect();
    for value in values {
        let at = ((value / size).floor().max(0.) as usize).min(count - 1);
        bins[at].count += 1;
    }
    bins
}

/// How many samples one dot stands for, so the tallest column fits.
pub fn samples_per_dot(bins: &[Bin]) -> usize {
    let peak = bins.iter().map(|bin| bin.count).max().unwrap_or(0).max(1);
    peak.div_ceil(MAX_ROWS).max(1)
}

/// Where dot `k` of a column sits, in steps from the middle: 0, 1 down, 1 up,
/// 2 down, 2 up, and so on.
pub fn dot_offset(k: usize) -> f32 {
    let side = if k % 2 == 1 { 1. } else { -1. };
    side * k.div_ceil(2) as f32
}

/// A value with at most two decimals and no trailing zeros.
pub fn plain_number(value: f64) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

type Format = Rc<dyn Fn(f64) -> String>;
type SelectHandler = Rc<dyn Fn(&Bin, &mut Window, &mut App)>;

/// A spread of samples as columns of dots.
#[derive(IntoElement)]
pub struct DotHistogram {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    subtitle: Option<SharedString>,
    values: Vec<f64>,
    limit: Option<f64>,
    limit_label: SharedString,
    columns: usize,
    unit: SharedString,
    format: Format,
    on_select: Option<SelectHandler>,
}

impl DotHistogram {
    /// Create a histogram of samples, all zero or more.
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>, values: Vec<f64>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            subtitle: None,
            values,
            limit: None,
            limit_label: "over limit".into(),
            columns: 48,
            unit: "samples".into(),
            format: Rc::new(plain_number),
            on_select: None,
        }
    }

    /// A quieter line under the title.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// A limit: samples at or past it are marked, with this label.
    pub fn limit(mut self, limit: f64, label: impl Into<SharedString>) -> Self {
        self.limit = Some(limit);
        self.limit_label = label.into();
        self
    }

    /// How many columns. 48 by default.
    pub fn columns(mut self, columns: usize) -> Self {
        self.columns = columns.clamp(4, 120);
        self
    }

    /// What one sample is called, in the plural. "samples" by default.
    pub fn unit(mut self, unit: impl Into<SharedString>) -> Self {
        self.unit = unit.into();
        self
    }

    /// How a value is written, such as with a unit after it.
    pub fn format(mut self, format: impl Fn(f64) -> String + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }

    /// Run with the column the reader points at or steps to.
    pub fn on_select(mut self, handler: impl Fn(&Bin, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Styled for DotHistogram {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for DotHistogram {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let part = |name: String| ElementId::NamedChild(self.id.clone().into(), name.into());
        let format = self.format.clone();
        let mut sorted = self.values.clone();
        sorted.sort_by(f64::total_cmp);
        let median = quantile(&sorted, 0.5);
        let p95 = quantile(&sorted, 0.95);
        let all = bins(&self.values, self.columns, self.limit);
        let per = samples_per_dot(&all);
        let peak = all.iter().map(|bin| bin.count).max().unwrap_or(0).max(1);
        let rows = peak.div_ceil(per);
        let plot_h = rows.max(6) as f32 * STEP + STEP;
        let span = all.last().map(|bin| bin.to).unwrap_or(1.);
        let total = self.values.len();
        let flagged = self
            .limit
            .map(|limit| self.values.iter().filter(|value| **value >= limit).count());
        let share = |count: usize| format!("{:.1}%", count as f64 / total.max(1) as f64 * 100.);

        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let ink = theme.foreground;
        let fill = theme.chart_1;
        let danger = theme.danger;
        let raised = theme.popover;
        let mono = theme.mono_font_family.clone();
        let grid = ink.opacity(0.1);
        let wash = look::ink::wash_strong(cx);

        // Which column is picked, and whether the keyboard picked it.
        let active = window.use_keyed_state(part("active".into()), cx, |_, _| None::<usize>);
        let picked = *active.read(cx);
        let appear_time = std::time::Duration::from_millis(300 + 8 * all.len() as u64);
        let appeared = Presence::new(part("appear".into()), true)
            .transition(Transition::new(appear_time).ease(|time| time))
            .sample(window, cx)
            .progress;
        let elapsed = appeared * appear_time.as_secs_f32();

        let columns = all.clone();
        let median_at = (median / span) as f32;
        let limit_at = self.limit.map(|limit| (limit / span) as f32);
        let plot = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let width = f32::from(bounds.size.width);
                let col = width / columns.len() as f32;
                let at = |x: f32, y: f32| point(bounds.left() + px(x), bounds.top() + px(y));
                let line = |window: &mut Window,
                            x: f32,
                            from: f32,
                            to: f32,
                            wide: f32,
                            dash: Option<[f32; 2]>,
                            color| {
                    let mut path = PathBuilder::stroke(px(wide));
                    if let Some([on, off]) = dash {
                        path = path.dash_array(&[px(on), px(off)]);
                    }
                    path.move_to(at(x, from));
                    path.line_to(at(x, to));
                    if let Ok(path) = path.build() {
                        window.paint_path(path, color);
                    }
                };
                for quarter in [0.25, 0.5, 0.75] {
                    line(
                        window,
                        width * quarter,
                        0.,
                        plot_h,
                        1.,
                        Some([2., 4.]),
                        grid,
                    );
                }
                let radius = RADIUS.min(col * 0.38);
                let mid = plot_h / 2.;
                for (ix, bin) in columns.iter().enumerate() {
                    if bin.count == 0 {
                        continue;
                    }
                    // Columns fade in from the low end, 8ms apart.
                    let shown = ((elapsed - 0.008 * ix as f32) / 0.3).clamp(0., 1.);
                    let dim = if picked.is_some_and(|at| at != ix) {
                        0.45
                    } else {
                        1.
                    };
                    let color = if bin.flagged { danger } else { fill }.opacity(shown * dim);
                    let x = (ix as f32 + 0.5) * col;
                    for k in 0..bin.count.div_ceil(per) {
                        let y = mid + dot_offset(k) * STEP;
                        let ring: Vec<_> = (0..12)
                            .map(|step| {
                                let angle = step as f32 / 12. * std::f32::consts::TAU;
                                at(x + radius * angle.cos(), y + radius * angle.sin())
                            })
                            .collect();
                        let mut dot = PathBuilder::fill();
                        dot.add_polygon(&ring, true);
                        if let Ok(dot) = dot.build() {
                            window.paint_path(dot, color);
                        }
                    }
                }
                line(
                    window,
                    width * median_at,
                    2.,
                    plot_h - 2.,
                    2.,
                    None,
                    ink.opacity(0.7),
                );
                if let Some(limit_at) = limit_at {
                    line(
                        window,
                        width * limit_at,
                        0.,
                        plot_h,
                        1.5,
                        Some([3., 3.]),
                        danger,
                    );
                }
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .w_full()
        .h(px(plot_h));

        // One invisible strip per column takes the pointer.
        let strips = all.iter().enumerate().map(|(ix, bin)| {
            let active = active.clone();
            let select = self.on_select.clone();
            let bin = *bin;
            div()
                .id(part(format!("column-{ix}")))
                .test_support()
                .flex_1()
                .h_full()
                .when(picked == Some(ix), |this| this.rounded(px(6.)).bg(wash))
                .on_hover(move |over, window, cx| {
                    let next = (*over && bin.count > 0).then_some(ix);
                    if *over || *active.read(cx) == Some(ix) {
                        active.update(cx, |state, cx| {
                            *state = next;
                            cx.notify();
                        });
                    }
                    if let (Some(select), true) = (&select, next.is_some()) {
                        select(&bin, window, cx);
                    }
                })
        });

        let axis_label = |along: f32, text: String, color, anchor_end: bool| {
            div()
                .absolute()
                .top(px(plot_h + 4.))
                .left(relative(along))
                .w(px(0.))
                .flex()
                .when(!anchor_end, |this| this.justify_center())
                .when(anchor_end, |this| this.justify_end())
                .child(
                    div()
                        .flex_none()
                        .whitespace_nowrap()
                        .text_size(px(10.))
                        .line_height(px(16.))
                        .text_color(color)
                        .child(text),
                )
        };

        let tip = picked
            .and_then(|ix| all.get(ix).map(|bin| (ix, *bin)))
            .map(|(ix, bin)| {
                let along = (ix as f32 + 0.5) / all.len() as f32;
                div()
                    .absolute()
                    .top_0()
                    .left(relative(along.min(0.72)))
                    .ml(px(10.))
                    .px_3()
                    .py_2()
                    .rounded(px(12.))
                    .bg(raised)
                    .shadow_md()
                    .text_caption()
                    .child(div().text_color(quiet).child(if bin.flagged {
                        self.limit_label.to_string()
                    } else {
                        format!("{}–{}", format(bin.from), format(bin.to))
                    }))
                    .child(div().font_family(mono.clone()).child(format!(
                        "{} {} · {}",
                        bin.count,
                        self.unit,
                        share(bin.count)
                    )))
            });

        let filled: Vec<usize> = all
            .iter()
            .enumerate()
            .filter(|(_, bin)| bin.count > 0)
            .map(|(ix, _)| ix)
            .collect();
        let key_state = active.clone();
        let key_select = self.on_select.clone();
        let key_bins = all.clone();
        let spoken = format!(
            "{}: {total} {}, median {}, p95 {}{}",
            self.title,
            self.unit,
            format(median),
            format(p95),
            match (flagged, picked.and_then(|ix| all.get(ix))) {
                (_, Some(bin)) => format!(
                    ". {} to {}: {} {}",
                    format(bin.from),
                    format(bin.to),
                    bin.count,
                    self.unit
                ),
                (Some(flagged), None) => format!(", {flagged} {}", self.limit_label),
                (None, None) => String::new(),
            }
        );

        let legend = h_flex()
            .mt_3()
            .flex_wrap()
            .gap_x_4()
            .gap_y_1()
            .text_caption()
            .text_color(quiet)
            .child(
                h_flex()
                    .gap(px(6.))
                    .child(
                        div()
                            .w(px(2.))
                            .h(px(12.))
                            .rounded_full()
                            .bg(ink.opacity(0.7)),
                    )
                    .child("Median")
                    .child(
                        div()
                            .font_family(mono.clone())
                            .text_color(ink)
                            .child(format(median)),
                    ),
            )
            .child(
                h_flex().gap(px(6.)).child("p95").child(
                    div()
                        .font_family(mono.clone())
                        .text_color(ink)
                        .child(format(p95)),
                ),
            )
            .when_some(flagged, |this, flagged| {
                this.child(
                    h_flex()
                        .gap(px(6.))
                        .child(div().size(px(8.)).rounded_full().bg(danger))
                        .child(self.limit_label.clone())
                        .child(
                            div()
                                .font_family(mono.clone())
                                .text_color(ink)
                                .child(format!("{flagged} · {}", share(flagged))),
                        ),
                )
            })
            .when(per > 1, |this| {
                this.child(
                    div()
                        .ml_auto()
                        .child(format!("1 dot = {per} {}", self.unit)),
                )
            });

        shell(cx)
            .id(self.id.clone())
            .max_w(px(672.))
            .child(shell_header(
                Some(Icon::new(CatalogIcon::ChartScatter)),
                self.title.clone(),
                self.subtitle.clone().map(|line| line.into_any_element()),
                Some(
                    div()
                        .font_family(mono)
                        .text_caption()
                        .text_color(quiet)
                        .child(format!("{total} {}", self.unit))
                        .into_any_element(),
                ),
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
                            .h(px(plot_h + AXIS))
                            .rounded(px(12.))
                            .role(Role::Group)
                            .aria_label(spoken)
                            .tab_index(0)
                            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                                if filled.is_empty() {
                                    return;
                                }
                                let now = *key_state.read(cx);
                                let at =
                                    now.and_then(|ix| filled.iter().position(|&bin| bin == ix));
                                let last = filled.len() - 1;
                                let next = match event.keystroke.key.as_str() {
                                    "right" => Some(at.map_or(0, |at| (at + 1) % filled.len())),
                                    "left" => {
                                        Some(at.map_or(last, |at| (at + last) % filled.len()))
                                    }
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
                                    select(&key_bins[ix], window, cx);
                                }
                                cx.stop_propagation();
                            })
                            .child(plot)
                            .child(
                                h_flex()
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .w_full()
                                    .h(px(plot_h))
                                    .children(strips),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .top(px(plot_h + 4.))
                                    .left_0()
                                    .text_size(px(10.))
                                    .line_height(px(16.))
                                    .text_color(quiet)
                                    .child(format(0.)),
                            )
                            .child(axis_label(median_at, format(median), ink, false))
                            .when_some(self.limit, |this, limit| {
                                this.child(axis_label(
                                    (limit / span) as f32,
                                    format(limit),
                                    danger,
                                    true,
                                ))
                            })
                            .children(tip),
                    )
                    .child(legend),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantiles_interpolate_between_samples() {
        let sorted = [1., 2., 3., 4.];
        assert_eq!(quantile(&sorted, 0.5), 2.5);
        assert_eq!(quantile(&sorted, 0.), 1.);
        assert_eq!(quantile(&sorted, 1.), 4.);
        assert_eq!(quantile(&[], 0.5), 0.);
    }

    #[test]
    fn a_column_edge_falls_on_the_limit() {
        let values = [10., 20., 30., 90., 95., 120.];
        let all = bins(&values, 12, Some(100.));
        // Twelve columns of 10: the limit is the start of the eleventh.
        assert_eq!(all[10].from, 100.);
        assert!(!all[9].flagged && all[10].flagged && all[11].flagged);
        assert_eq!(all.iter().map(|bin| bin.count).sum::<usize>(), values.len());
        // The highest sample lands in the last column, not past it.
        assert_eq!(all[11].count, 1);
        // Without a limit the range ends at the highest sample.
        let plain = bins(&values, 12, None);
        assert_eq!(plain[11].to, 120.);
        assert!(plain.iter().all(|bin| !bin.flagged));
        assert_eq!(bins(&[], 4, None)[3].to, 1.);
    }

    #[test]
    fn tall_columns_share_dots() {
        let short = [Bin {
            from: 0.,
            to: 1.,
            count: 18,
            flagged: false,
        }];
        assert_eq!(samples_per_dot(&short), 1);
        let tall = [Bin {
            from: 0.,
            to: 1.,
            count: 19,
            flagged: false,
        }];
        assert_eq!(samples_per_dot(&tall), 2);
        assert_eq!(samples_per_dot(&[]), 1);
    }

    #[test]
    fn dots_stack_outward_from_the_middle() {
        let offsets: Vec<f32> = (0..5).map(dot_offset).collect();
        assert_eq!(offsets, [0., 1., -1., 2., -2.]);
        assert_eq!(plain_number(2.50), "2.5");
        assert_eq!(plain_number(300.), "300");
        assert_eq!(plain_number(0.126), "0.13");
    }
}
