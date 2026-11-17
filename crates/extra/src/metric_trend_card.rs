// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Metric trend card: one headline number and how its parts moved.
//!
//! The card shows a title, a value that is already formatted, and the change
//! against the period before. Under them a small plot draws one line per
//! series over the same points. Pointing at the plot, or stepping through it
//! with the arrow keys, lists every series' value at that point.

use std::rc::Rc;

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, StyledExt as _, ThemeStyled as _, h_flex, v_flex,
};
use isonomic_ui::{
    App, Bounds, ElementId, FocusHandle, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, PathBuilder, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, canvas, div, fill, point,
    prelude::FluentBuilder as _, px, relative, size,
};

use crate::common::{lifted_shadow, resting_shadow};

/// How tall the plot is.
const PLOT_HEIGHT: f32 = 128.;
/// The room kept for the axis labels in front of the plot.
const AXIS_WIDTH: f32 = 40.;

/// One line of the plot.
#[derive(Debug, Clone)]
pub struct TrendSeries {
    label: SharedString,
    values: Vec<f64>,
    color: Option<Hsla>,
}

impl TrendSeries {
    /// Create a series with one value per point.
    pub fn new(label: impl Into<SharedString>, values: impl IntoIterator<Item = f64>) -> Self {
        Self {
            label: label.into(),
            values: values.into_iter().collect(),
            color: None,
        }
    }

    /// Set the colour of the line. Without one, series take the theme's chart
    /// colours in order.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }
}

type Format = Rc<dyn Fn(f64) -> String>;

/// A headline number over a small multi-line plot.
#[derive(IntoElement)]
pub struct MetricTrendCard {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    value: SharedString,
    delta: Option<f64>,
    labels: Vec<SharedString>,
    long_labels: Vec<SharedString>,
    series: Vec<TrendSeries>,
    format: Format,
    footer: Option<SharedString>,
    default_point: Option<usize>,
    empty_text: SharedString,
}

impl MetricTrendCard {
    /// Create a card with a title and a headline value that is already
    /// formatted.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        value: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            value: value.into(),
            delta: None,
            labels: Vec::new(),
            long_labels: Vec::new(),
            series: Vec::new(),
            format: Rc::new(compact),
            footer: None,
            default_point: None,
            empty_text: "Nothing to plot yet".into(),
        }
    }

    /// Set the change against the period before. It is shown with a sign and
    /// an arrow.
    pub fn delta(mut self, delta: f64) -> Self {
        self.delta = Some(delta);
        self
    }

    /// Name the points, oldest first. The first, middle and last names are
    /// shown under the plot.
    pub fn labels(mut self, labels: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.labels = labels.into_iter().map(Into::into).collect();
        self
    }

    /// Give the points longer names for the readout, such as the weekday with
    /// the date.
    pub fn long_labels(
        mut self,
        labels: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.long_labels = labels.into_iter().map(Into::into).collect();
        self
    }

    /// Add a line. Up to six read well.
    pub fn series(mut self, series: TrendSeries) -> Self {
        self.series.push(series);
        self
    }

    /// Set how the axis, the readout and the change are written. Short forms
    /// such as "1.2k" by default.
    pub fn format(mut self, format: impl Fn(f64) -> String + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }

    /// Set a line under the plot.
    pub fn footer(mut self, footer: impl Into<SharedString>) -> Self {
        self.footer = Some(footer.into());
        self
    }

    /// Keep the readout open at this point while nothing else is pointed at.
    pub fn default_point(mut self, point: usize) -> Self {
        self.default_point = Some(point);
        self
    }

    /// Set the text shown when there are no values, or only zeros.
    pub fn empty_text(mut self, text: impl Into<SharedString>) -> Self {
        self.empty_text = text.into();
        self
    }
}

impl Styled for MetricTrendCard {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// A number in a few characters: "950", "1.2k", "3.4M".
fn compact(value: f64) -> String {
    let size = value.abs();
    let (scaled, suffix) = if size >= 1e9 {
        (value / 1e9, "B")
    } else if size >= 1e6 {
        (value / 1e6, "M")
    } else if size >= 1e3 {
        (value / 1e3, "k")
    } else {
        (value, "")
    };
    let rounded = (scaled * 10.).round() / 10.;
    if rounded.fract() == 0. {
        format!("{rounded:.0}{suffix}")
    } else {
        format!("{rounded:.1}{suffix}")
    }
}

/// The top of the axis: the smallest round number that holds `peak`.
fn axis_top(peak: f64) -> f64 {
    if peak <= 0. {
        return 1.;
    }
    let magnitude = 10f64.powf(peak.log10().floor());
    [1., 2., 3., 4., 5., 6., 8., 10.]
        .into_iter()
        .map(|step| step * magnitude)
        .find(|top| *top >= peak)
        .unwrap_or(10. * magnitude)
}

/// The point an arrow key moves to. It stops at the ends.
fn step_point(current: Option<usize>, points: usize, key: &str) -> Option<usize> {
    let last = points.checked_sub(1)?;
    match (key, current) {
        ("right", None) | ("home", _) => Some(0),
        ("left", None) | ("end", _) => Some(last),
        ("right", Some(ix)) => Some((ix + 1).min(last)),
        ("left", Some(ix)) => Some(ix.saturating_sub(1)),
        _ => None,
    }
}

/// The change with its sign, written with `format`.
fn delta_words(delta: f64, format: &dyn Fn(f64) -> String) -> String {
    if delta > 0. {
        format!("+{}", format(delta))
    } else if delta < 0. {
        format!("\u{2212}{}", format(-delta))
    } else {
        format(0.)
    }
}

struct CardState {
    /// The point the pointer or the arrows are on.
    active: Option<usize>,
    focus: FocusHandle,
}

impl RenderOnce for MetricTrendCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill_color = theme.background;
        let surface = theme.popover;
        let line = theme.border;
        let success = theme.success;
        let danger = theme.danger;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;
        let palette = [
            theme.chart_1,
            theme.chart_2,
            theme.chart_3,
            theme.chart_4,
            theme.chart_5,
        ];

        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| CardState {
            active: None,
            focus: cx.focus_handle().tab_stop(true),
        });
        let (active, focus) = {
            let state = state.read(cx);
            (state.active, state.focus.clone())
        };

        // Every series is drawn over the same points.
        let points = self
            .series
            .iter()
            .map(|series| series.values.len())
            .max()
            .unwrap_or(0);
        let peak = self
            .series
            .iter()
            .flat_map(|series| series.values.iter().copied())
            .fold(0., f64::max);
        let is_empty = points == 0 || peak <= 0.;
        let top = axis_top(peak);
        let shown = active
            .or(self.default_point)
            .filter(|ix| *ix < points && !is_empty);
        let lines: Vec<(Vec<f64>, Hsla)> = self
            .series
            .iter()
            .enumerate()
            .map(|(ix, series)| {
                (
                    series.values.clone(),
                    series.color.unwrap_or(palette[ix % palette.len()]),
                )
            })
            .collect();
        let format = self.format.clone();
        let card_name = match self.delta {
            Some(delta) => format!(
                "{}: {}, {} on the period before",
                self.title,
                self.value,
                delta_words(delta, format.as_ref())
            ),
            None => format!("{}: {}", self.title, self.value),
        };
        let plot_name = if is_empty {
            self.empty_text.to_string()
        } else {
            match shown {
                Some(ix) => {
                    let heading = self
                        .long_labels
                        .get(ix)
                        .or_else(|| self.labels.get(ix))
                        .map(|h| format!("{h}: "))
                        .unwrap_or_default();
                    let values = self
                        .series
                        .iter()
                        .map(|series| match series.values.get(ix) {
                            Some(value) => format!("{} {}", series.label, format(*value)),
                            None => format!("{} no value", series.label),
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("Plot, {heading}{values}")
                }
                None => format!("Plot of {points} points. Use the arrow keys to read each one"),
            }
        };

        let delta = self.delta.map(|delta| {
            let (color, icon) = if delta > 0. {
                (success, Some(IconName::ArrowUp))
            } else if delta < 0. {
                (danger, Some(IconName::ArrowDown))
            } else {
                (quiet, None)
            };
            h_flex()
                .flex_none()
                .gap_0p5()
                .px_1p5()
                .rounded(px(5.))
                .bg(color.opacity(0.14))
                .text_xs()
                .font_medium()
                .children(icon.map(|icon| Icon::new(icon).size_3().text_color(color)))
                .child(delta_words(delta, format.as_ref()))
        });

        let drawing = {
            let lines = lines.clone();
            canvas(
                |_, _, _| {},
                move |bounds: Bounds<isonomic_ui::Pixels>, _, window, _| {
                    let across = |ix: usize| {
                        bounds.left() + bounds.size.width * ((ix as f32 + 0.5) / points as f32)
                    };
                    let up = |value: f64| {
                        let share = (value / top).clamp(0., 1.) as f32;
                        bounds.bottom() - px(1.) - (bounds.size.height - px(1.)) * share
                    };
                    for step in [0., 0.5, 1.] {
                        let y = bounds.bottom() - px(1.) - (bounds.size.height - px(1.)) * step;
                        window.paint_quad(fill(
                            Bounds::new(point(bounds.left(), y), size(bounds.size.width, px(1.))),
                            line,
                        ));
                    }
                    if let Some(ix) = shown {
                        window.paint_quad(fill(
                            Bounds::new(
                                point(across(ix), bounds.top()),
                                size(px(1.), bounds.size.height),
                            ),
                            quiet.opacity(0.5),
                        ));
                    }
                    for (values, color) in &lines {
                        let mut path = PathBuilder::stroke(px(1.5));
                        for (ix, value) in values.iter().enumerate() {
                            let to = point(across(ix), up(*value));
                            if ix == 0 {
                                path.move_to(to);
                            } else {
                                path.line_to(to);
                            }
                        }
                        if values.len() > 1
                            && let Ok(path) = path.build()
                        {
                            window.paint_path(path, *color);
                        }
                        if let Some(ix) = shown
                            && let Some(value) = values.get(ix)
                        {
                            let center = point(across(ix), up(*value));
                            window.paint_quad(
                                fill(Bounds::centered_at(center, size(px(7.), px(7.))), *color)
                                    .corner_radii(px(3.5)),
                            );
                        }
                    }
                },
            )
            .absolute()
            .size_full()
        };

        // One invisible column per point takes the pointer.
        let columns = h_flex()
            .absolute()
            .size_full()
            .children((0..points).map(|ix| {
                let state = state.clone();
                div()
                    .id(("point", ix))
                    .flex_1()
                    .h_full()
                    .on_hover(move |hovered, _, cx| {
                        state.update(cx, |state, cx| {
                            if *hovered {
                                state.active = Some(ix);
                                cx.notify();
                            } else if state.active == Some(ix) {
                                state.active = None;
                                cx.notify();
                            }
                        })
                    })
            }));

        let readout =
            shown.map(|ix| {
                let heading = self
                    .long_labels
                    .get(ix)
                    .or_else(|| self.labels.get(ix))
                    .cloned();
                // The panel sits beside the point, on the side with more room.
                let before = ix * 2 >= points;
                v_flex()
                    .absolute()
                    .top_0()
                    .map(|this| {
                        if before {
                            this.right(relative((points - ix) as f32 / points as f32))
                                .mr_2()
                        } else {
                            this.left(relative((ix + 1) as f32 / points as f32)).ml_2()
                        }
                    })
                    .min_w(px(132.))
                    .gap_1()
                    .p_2()
                    .rounded(radius)
                    .border_1()
                    .border_color(line)
                    .bg(surface)
                    .shadow(lifted_shadow(cx))
                    .text_xs()
                    .children(heading.map(|heading| div().font_medium().child(heading)))
                    .children(self.series.iter().zip(&lines).map(|(series, (_, color))| {
                        h_flex()
                            .gap_2()
                            .child(div().flex_none().size_2().rounded_full_style(cx).bg(*color))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_color(quiet)
                                    .child(series.label.clone()),
                            )
                            .child(div().flex_none().font_medium().child(
                                match series.values.get(ix) {
                                    Some(value) => format(*value),
                                    None => "\u{2013}".to_string(),
                                },
                            ))
                    }))
            });

        let is_focused = focus.is_focused(window);
        let plot = div()
            .id((self.id.clone(), "plot"))
            .role(Role::Figure)
            .aria_label(plot_name)
            .track_focus(&focus)
            .on_key_down({
                move |event, _, cx| {
                    if event.keystroke.modifiers.modified() {
                        return;
                    }
                    let next = match event.keystroke.key.as_str() {
                        "escape" => None,
                        key => match step_point(shown, points, key) {
                            Some(next) => Some(next),
                            None => return,
                        },
                    };
                    state.update(cx, |state, cx| {
                        state.active = next;
                        cx.notify();
                    });
                    cx.stop_propagation();
                }
            })
            .relative()
            .flex_1()
            .h(px(PLOT_HEIGHT))
            .rounded(px(3.))
            .when(is_focused, |this| this.focus_ring_style(window, cx))
            .map(|this| {
                if is_empty {
                    this.flex()
                        .items_center()
                        .justify_center()
                        .text_color(quiet)
                        .child(self.empty_text.clone())
                } else {
                    this.child(drawing).child(columns).children(readout)
                }
            });

        let axis = v_flex()
            .flex_none()
            .w(px(AXIS_WIDTH))
            .h(px(PLOT_HEIGHT))
            .justify_between()
            .text_xs()
            .text_color(quiet)
            .when(!is_empty, |this| {
                this.children(
                    [top, top / 2., 0.]
                        .into_iter()
                        .map(|tick| div().line_height(relative(1.)).child(format(tick))),
                )
            });
        // The names of the first, middle and last points.
        let named = [0, points.saturating_sub(1) / 2, points.saturating_sub(1)];
        let names = (!is_empty && !self.labels.is_empty()).then(|| {
            h_flex()
                .pl(px(AXIS_WIDTH + 8.))
                .justify_between()
                .text_xs()
                .text_color(quiet)
                .children(
                    named
                        .into_iter()
                        .map(|ix| div().children(self.labels.get(ix).cloned())),
                )
        });

        v_flex()
            .id(self.id.clone())
            .role(Role::Group)
            .aria_label(card_name)
            .w_full()
            .max_w(px(460.))
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill_color)
            .text_color(ink)
            .text_sm()
            .line_height(relative(1.45))
            .shadow(resting_shadow(cx))
            .child(
                v_flex()
                    .gap_2()
                    .p_4()
                    .child(
                        v_flex()
                            .gap_0p5()
                            .child(div().font_medium().child(self.title))
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_2xl()
                                            .font_semibold()
                                            .line_height(relative(1.2))
                                            .child(self.value),
                                    )
                                    .children(delta),
                            ),
                    )
                    .child(h_flex().gap_2().child(axis).child(plot))
                    .children(names),
            )
            .children(self.footer.map(|footer| {
                div()
                    .px_4()
                    .py_2p5()
                    .border_t_1()
                    .border_color(line)
                    .text_color(quiet)
                    .child(footer)
            }))
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_shorten_by_thousands() {
        assert_eq!(compact(0.), "0");
        assert_eq!(compact(950.), "950");
        assert_eq!(compact(1200.), "1.2k");
        assert_eq!(compact(6000.), "6k");
        assert_eq!(compact(96_400.), "96.4k");
        assert_eq!(compact(3_400_000.), "3.4M");
        assert_eq!(compact(-2500.), "-2.5k");
    }

    #[test]
    fn the_axis_ends_on_a_round_number() {
        assert_eq!(axis_top(5900.), 6000.);
        assert_eq!(axis_top(6000.), 6000.);
        assert_eq!(axis_top(6100.), 8000.);
        assert!((axis_top(0.7) - 0.8).abs() < 1e-9);
        assert_eq!(axis_top(0.), 1.);
        assert_eq!(axis_top(-4.), 1.);
    }

    #[test]
    fn arrows_step_through_the_points_and_stop_at_the_ends() {
        assert_eq!(step_point(None, 7, "right"), Some(0));
        assert_eq!(step_point(None, 7, "left"), Some(6));
        assert_eq!(step_point(Some(2), 7, "right"), Some(3));
        assert_eq!(step_point(Some(6), 7, "right"), Some(6));
        assert_eq!(step_point(Some(0), 7, "left"), Some(0));
        assert_eq!(step_point(Some(3), 7, "end"), Some(6));
        assert_eq!(step_point(Some(3), 7, "up"), None);
        assert_eq!(step_point(None, 0, "right"), None);
    }

    #[test]
    fn the_change_carries_its_sign() {
        assert_eq!(delta_words(7900., &compact), "+7.9k");
        assert_eq!(delta_words(-1200., &compact), "\u{2212}1.2k");
        assert_eq!(delta_words(0., &compact), "0");
    }
}
