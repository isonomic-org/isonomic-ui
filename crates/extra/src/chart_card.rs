// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Chart card: one set of series shown as bars, lines, an area or a donut.
//!
//! A switch in the header picks the chart type. The legend lists every series
//! with its total; pressing one hides it or brings it back. When the data
//! changes, or a series is hidden, the values glide to their new heights; a
//! new chart type grows in from zero. Under reduced motion every change is
//! instant. Pointing at the chart shows a tooltip; from the keyboard the
//! arrow keys step through the months (or the slices of the donut) and the
//! line under the chart reads the picked one aloud.

use std::{
    collections::BTreeSet,
    rc::Rc,
    time::{Duration, Instant},
};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::TestSupportExt as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, Sizable as _, StyledExt as _, TextRole as _,
    button::{Button, ButtonVariants as _},
    chart::{AreaChart, PieChart},
    h_flex,
    plot::{
        AxisLabelSide, AxisText, Grid, IntoPlot, Plot, PlotAxis, TooltipState,
        tooltip::{CrossLine, Tooltip},
    },
    tab::{Tab, TabBar},
    v_flex,
};
use isonomic_ui::{
    AnyElement, App, Bounds, Corners, ElementId, Hsla, InteractiveElement as _, IntoElement,
    KeyDownEvent, ParentElement as _, Pixels, Point, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, TextAlign, Window, div, fill, point,
    prelude::FluentBuilder as _, px, size, transparent_black,
};

use crate::common::{panel, shell, shell_header};

/// Values closer to zero than this count as gone.
const GONE: f64 = 1e-6;
/// Room left of the bars for the value labels, and under them for the months.
const VALUE_GUTTER: f32 = 40.;
const LABEL_GUTTER: f32 = 22.;

/// How the card draws its series.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ChartKind {
    /// Stacked columns, one per category.
    #[default]
    Bar,
    /// One line per series.
    Line,
    /// One filled area per series.
    Area,
    /// One slice per series, sized by its total.
    Donut,
}

impl ChartKind {
    /// Every kind, in switch order.
    pub const ALL: [ChartKind; 4] = [
        ChartKind::Bar,
        ChartKind::Line,
        ChartKind::Area,
        ChartKind::Donut,
    ];

    /// Its name, as the switch and a screen reader say it.
    pub fn label(self) -> &'static str {
        match self {
            ChartKind::Bar => "Bar chart",
            ChartKind::Line => "Line chart",
            ChartKind::Area => "Area chart",
            ChartKind::Donut => "Donut chart",
        }
    }

    fn icon(self) -> CatalogIcon {
        match self {
            ChartKind::Bar => CatalogIcon::ChartColumn,
            ChartKind::Line => CatalogIcon::ChartLine,
            ChartKind::Area => CatalogIcon::ChartArea,
            ChartKind::Donut => CatalogIcon::ChartRing,
        }
    }
}

/// One named row of values, one value per category.
#[derive(Debug, Clone, PartialEq)]
pub struct ChartSeries {
    /// What the series counts.
    pub name: SharedString,
    /// One value per category, all zero or more.
    pub values: Vec<f64>,
}

impl ChartSeries {
    /// A series with a name and its values.
    pub fn new(name: impl Into<SharedString>, values: impl Into<Vec<f64>>) -> Self {
        Self {
            name: name.into(),
            values: values.into(),
        }
    }
}

/// The values every series is heading for: its own, or zero while hidden.
pub fn targets(
    series: &[ChartSeries],
    hidden: &BTreeSet<usize>,
    categories: usize,
) -> Vec<Vec<f64>> {
    series
        .iter()
        .enumerate()
        .map(|(ix, series)| {
            (0..categories)
                .map(|at| {
                    if hidden.contains(&ix) {
                        0.
                    } else {
                        series.values.get(at).copied().unwrap_or(0.).max(0.)
                    }
                })
                .collect()
        })
        .collect()
}

/// The ease-out curve value changes glide on.
pub fn ease_out(t: f32) -> f32 {
    1. - (1. - t.clamp(0., 1.)).powi(3)
}

/// `t` of the way from one set of values to the next. A value missing on
/// either side counts as zero, so series and categories can come and go.
pub fn blend(from: &[Vec<f64>], to: &[Vec<f64>], t: f32) -> Vec<Vec<f64>> {
    let at = |rows: &[Vec<f64>], s: usize, i: usize| {
        rows.get(s)
            .and_then(|row| row.get(i))
            .copied()
            .unwrap_or(0.)
    };
    let rows = from.len().max(to.len());
    (0..rows)
        .map(|s| {
            let width = from
                .get(s)
                .map_or(0, Vec::len)
                .max(to.get(s).map_or(0, Vec::len));
            (0..width)
                .map(|i| {
                    let a = at(from, s, i);
                    a + (at(to, s, i) - a) * f64::from(t)
                })
                .collect()
        })
        .collect()
}

/// The series still on the chart: shown ones, and hidden ones still sinking.
pub fn drawn(hidden: &BTreeSet<usize>, current: &[Vec<f64>]) -> Vec<usize> {
    current
        .iter()
        .enumerate()
        .filter(|(ix, row)| !hidden.contains(ix) || row.iter().any(|v| v.abs() > GONE))
        .map(|(ix, _)| ix)
        .collect()
}

/// The smallest round number (1, 2, 2.5 or 5 times a power of ten) at or above
/// `value`, so the value axis ends on a tidy line.
pub fn nice_ceiling(value: f64) -> f64 {
    if value <= 0. || !value.is_finite() {
        return 1.;
    }
    let power = 10f64.powf(value.log10().floor());
    let scaled = value / power;
    let step = [1., 2., 2.5, 5., 10.]
        .into_iter()
        .find(|step| scaled <= *step + 1e-9)
        .unwrap_or(10.);
    step * power
}

/// A number written short: "950", "1.2k", "3.4M".
pub fn compact(value: f64) -> String {
    let trim = |text: String| {
        if text.contains('.') {
            text.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            text
        }
    };
    let size = value.abs();
    if size >= 1e6 {
        format!("{}M", trim(format!("{:.1}", value / 1e6)))
    } else if size >= 1e3 {
        format!("{}k", trim(format!("{:.1}", value / 1e3)))
    } else {
        trim(format!("{value:.1}"))
    }
}

/// Where a key moves the pick among `len` items: `Some(next)` for a key the
/// chart takes (`next` is `None` to clear it), `None` for any other key.
pub fn step_pick(current: Option<usize>, key: &str, len: usize) -> Option<Option<usize>> {
    if len == 0 {
        return None;
    }
    let last = len - 1;
    Some(match key {
        "right" | "down" => Some(current.map_or(0, |at| (at + 1).min(last))),
        "left" | "up" => Some(current.map_or(last, |at| at.saturating_sub(1))),
        "home" => Some(0),
        "end" => Some(last),
        "escape" => None,
        _ => return None,
    })
}

/// A share of a total as a whole percentage.
pub fn percent(part: f64, total: f64) -> String {
    if total <= 0. {
        return "0%".into();
    }
    format!("{:.0}%", part / total * 100.)
}

type Format = Rc<dyn Fn(f64) -> String>;
type KindHandler = Rc<dyn Fn(ChartKind, &mut Window, &mut App)>;
type ToggleHandler = Rc<dyn Fn(usize, bool, &mut Window, &mut App)>;

/// The words under the chart for the picked category (or slice).
fn readout(
    kind: ChartKind,
    categories: &[SharedString],
    series: &[ChartSeries],
    hidden: &BTreeSet<usize>,
    pick: usize,
    format: &Format,
) -> String {
    let shown: Vec<usize> = (0..series.len())
        .filter(|ix| !hidden.contains(ix))
        .collect();
    if kind == ChartKind::Donut {
        let totals: Vec<f64> = series.iter().map(|s| s.values.iter().sum()).collect();
        let all: f64 = shown.iter().map(|ix| totals[*ix]).sum();
        return match shown.get(pick) {
            Some(&ix) => format!(
                "{}: {}, {} of the total",
                series[ix].name,
                format(totals[ix]),
                percent(totals[ix], all)
            ),
            None => String::new(),
        };
    }
    let Some(category) = categories.get(pick) else {
        return String::new();
    };
    let mut total = 0.;
    let mut parts = Vec::new();
    for ix in &shown {
        let value = series[*ix].values.get(pick).copied().unwrap_or(0.);
        total += value;
        parts.push(format!("{} {}", series[*ix].name, format(value)));
    }
    if shown.len() > 1 {
        parts.push(format!("total {}", format(total)));
    }
    format!("{category}: {}", parts.join(", "))
}

/// What a screen reader hears for the chart as a whole.
fn summary(
    title: &str,
    kind: ChartKind,
    categories: &[SharedString],
    series: &[ChartSeries],
    hidden: &BTreeSet<usize>,
    format: &Format,
) -> String {
    let shown: Vec<String> = series
        .iter()
        .enumerate()
        .filter(|(ix, _)| !hidden.contains(ix))
        .map(|(_, s)| format!("{} {}", s.name, format(s.values.iter().sum())))
        .collect();
    let across = match (categories.first(), categories.last()) {
        (Some(first), Some(last)) if categories.len() > 1 => format!(", {first} to {last}"),
        _ => String::new(),
    };
    if shown.is_empty() {
        return format!("{title}, {}{across}: every series hidden", kind.label());
    }
    format!(
        "{title}, {}{across}. Totals: {}",
        kind.label(),
        shown.join(", ")
    )
}

/// The values on screen and where they are heading.
#[derive(Clone)]
struct Tween {
    from: Vec<Vec<f64>>,
    to: Vec<Vec<f64>>,
    started: Instant,
    kind: ChartKind,
}

impl Tween {
    fn at(&self, now: Instant, duration: Duration) -> f32 {
        if duration.is_zero() {
            return 1.;
        }
        ease_out(now.saturating_duration_since(self.started).as_secs_f32() / duration.as_secs_f32())
    }
}

/// The kind the caller gave last, and the one on screen.
#[derive(Clone, Copy)]
struct KindState {
    given: ChartKind,
    shown: ChartKind,
}

/// One category, with every series' value at it this frame.
#[derive(Clone)]
struct Row {
    label: SharedString,
    values: Rc<Vec<f64>>,
}

/// Series of named values, switchable between bars, lines, an area and a donut.
#[derive(IntoElement)]
pub struct ChartCard {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    subtitle: Option<SharedString>,
    categories: Vec<SharedString>,
    series: Vec<ChartSeries>,
    kind: ChartKind,
    kinds: Vec<ChartKind>,
    hidden: BTreeSet<usize>,
    height: Pixels,
    format: Format,
    on_kind_change: Option<KindHandler>,
    on_toggle: Option<ToggleHandler>,
}

impl ChartCard {
    /// A card over these categories, such as months, with no series yet.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        categories: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            subtitle: None,
            categories: categories.into_iter().map(Into::into).collect(),
            series: Vec::new(),
            kind: ChartKind::Bar,
            kinds: ChartKind::ALL.to_vec(),
            hidden: BTreeSet::new(),
            height: px(220.),
            format: Rc::new(compact),
            on_kind_change: None,
            on_toggle: None,
        }
    }

    /// A quieter line under the title.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Add a series. Series take the theme's data colours in order.
    pub fn series(mut self, series: ChartSeries) -> Self {
        self.series.push(series);
        self
    }

    /// The chart type to show. Changing it later switches the card to it.
    pub fn kind(mut self, kind: ChartKind) -> Self {
        self.kind = kind;
        self
    }

    /// Which types the switch offers, in order. All four by default; with one
    /// the switch is left out.
    pub fn kinds(mut self, kinds: impl IntoIterator<Item = ChartKind>) -> Self {
        let kinds: Vec<ChartKind> = kinds.into_iter().collect();
        if !kinds.is_empty() {
            self.kinds = kinds;
        }
        self
    }

    /// Start with the series at `index` hidden.
    pub fn hidden(mut self, index: usize) -> Self {
        self.hidden.insert(index);
        self
    }

    /// The height of the plot. 220px by default.
    pub fn height(mut self, height: impl Into<Pixels>) -> Self {
        self.height = height.into();
        self
    }

    /// How a value is written. Short numbers ("1.2k") by default.
    pub fn format(mut self, format: impl Fn(f64) -> String + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }

    /// Run when the reader picks another chart type.
    pub fn on_kind_change(
        mut self,
        handler: impl Fn(ChartKind, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_kind_change = Some(Rc::new(handler));
        self
    }

    /// Run when the reader hides or shows a series, with its index and whether
    /// it is now shown.
    pub fn on_series_toggle(
        mut self,
        handler: impl Fn(usize, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }
}

impl Styled for ChartCard {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// The theme's data colour for series `ix`.
fn series_color(ix: usize, cx: &App) -> Hsla {
    let theme = cx.theme();
    [
        theme.chart_1,
        theme.chart_2,
        theme.chart_3,
        theme.chart_4,
        theme.chart_5,
    ][ix % 5]
}

impl RenderOnce for ChartCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let part = |name: &str| ElementId::NamedChild(self.id.clone().into(), name.into());
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let mono = theme.mono_font_family.clone();
        let duration = if cx.reduce_motion() {
            Duration::ZERO
        } else {
            theme.motion.duration_slow
        };
        let colors: Vec<Hsla> = (0..self.series.len())
            .map(|ix| series_color(ix, cx))
            .collect();
        let format = self.format.clone();

        let kinds = self.kinds.clone();
        let given = if kinds.contains(&self.kind) {
            self.kind
        } else {
            kinds[0]
        };
        let kind_state = window.use_keyed_state(part("kind"), cx, |_, _| KindState {
            given,
            shown: given,
        });
        if kind_state.read(cx).given != given {
            kind_state.update(cx, |state, _| {
                *state = KindState {
                    given,
                    shown: given,
                }
            });
        }
        let kind = kind_state.read(cx).shown;

        let start_hidden = self.hidden.clone();
        let hidden_state = window.use_keyed_state(part("hidden"), cx, |_, _| start_hidden);
        let hidden = hidden_state.read(cx).clone();
        let pick_state = window.use_keyed_state(part("pick"), cx, |_, _| None::<usize>);

        // Glide toward the target values; a new chart type grows from zero.
        let target = targets(&self.series, &hidden, self.categories.len());
        let now = cx.background_executor().now();
        let tween = window.use_keyed_state(part("tween"), cx, |_, _| Tween {
            from: Vec::new(),
            to: target.clone(),
            started: now,
            kind,
        });
        let before = tween.read(cx).clone();
        if before.to != target || before.kind != kind {
            let progress = before.at(now, duration);
            let from = if before.kind == kind {
                blend(&before.from, &before.to, progress)
            } else {
                Vec::new()
            };
            tween.update(cx, |state, _| {
                *state = Tween {
                    from,
                    to: target.clone(),
                    started: now,
                    kind,
                }
            });
        }
        let tween = tween.read(cx).clone();
        let progress = tween.at(now, duration);
        if progress < 1. {
            window.request_animation_frame();
        }
        let current = blend(&tween.from, &tween.to, progress);
        let on_chart = drawn(&hidden, &current);

        // The axis ends on a round number above the tallest target, so it holds
        // still while the values glide.
        let top = match kind {
            ChartKind::Bar => (0..self.categories.len())
                .map(|at| target.iter().map(|row| row[at]).sum::<f64>())
                .fold(0., f64::max),
            _ => target.iter().flatten().copied().fold(0., f64::max),
        };
        let top = nice_ceiling(top);

        let rows: Vec<Row> = self
            .categories
            .iter()
            .enumerate()
            .map(|(at, label)| Row {
                label: label.clone(),
                values: Rc::new(current.iter().map(|row| row[at]).collect()),
            })
            .collect();

        let names: Vec<SharedString> = self.series.iter().map(|s| s.name.clone()).collect();
        let pick = *pick_state.read(cx);
        let shown_count = self.series.len() - hidden.len().min(self.series.len());
        let pick_len = if kind == ChartKind::Donut {
            shown_count
        } else {
            self.categories.len()
        };
        let pick = pick.filter(|at| *at < pick_len);

        let chart: AnyElement = if on_chart.is_empty() {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_caption()
                .text_color(quiet)
                .child("Every series is hidden. Pick one in the legend to show it.")
                .into_any_element()
        } else {
            match kind {
                ChartKind::Bar => StackedColumns {
                    id: (self.id.clone(), "bars").into(),
                    rows,
                    series: on_chart,
                    names,
                    colors: colors.clone(),
                    top,
                    pick,
                    format: format.clone(),
                }
                .into_any_element(),
                ChartKind::Line | ChartKind::Area => {
                    let area = kind == ChartKind::Area;
                    let mut chart = AreaChart::new(rows)
                        .id((self.id.clone(), if area { "area" } else { "line" }))
                        .appear(false)
                        .x(|row: &Row| row.label.clone())
                        .y_domain(0., top)
                        .y_axis(true)
                        .y_tick_count(5)
                        .y_padding(0., 0.)
                        .y_tick_format({
                            let format = format.clone();
                            move |value| format(value)
                        })
                        .tooltip_value({
                            let format = format.clone();
                            move |_, _, value| format(value).into()
                        });
                    for ix in &on_chart {
                        let ix = *ix;
                        let color = colors[ix];
                        chart = chart
                            .y(move |row: &Row| row.values[ix])
                            .stroke(color)
                            .fill(if area {
                                color.opacity(0.16)
                            } else {
                                transparent_black()
                            })
                            .name(names[ix].clone());
                    }
                    chart.into_any_element()
                }
                ChartKind::Donut => {
                    let slices: Vec<(SharedString, f32, Hsla)> = on_chart
                        .iter()
                        .map(|ix| {
                            (
                                names[*ix].clone(),
                                current[*ix].iter().sum::<f64>() as f32,
                                colors[*ix],
                            )
                        })
                        .collect();
                    let total: f64 = target.iter().flatten().sum();
                    let donut_format = format.clone();
                    div()
                        .relative()
                        .size_full()
                        .child(
                            PieChart::new(slices)
                                .id((self.id.clone(), "donut"))
                                .appear(false)
                                .inner_radius_ratio(0.66)
                                .pad_angle(0.02)
                                .value(|slice: &(SharedString, f32, Hsla)| slice.1)
                                .color(|slice: &(SharedString, f32, Hsla)| slice.2)
                                .tooltip_name(|slice: &(SharedString, f32, Hsla)| slice.0.clone())
                                .tooltip_value(move |_, value, share| {
                                    format!("{} · {share:.0}%", donut_format(f64::from(value)))
                                        .into()
                                }),
                        )
                        .child(
                            v_flex()
                                .absolute()
                                .inset_0()
                                .items_center()
                                .justify_center()
                                .child(div().text_caption().text_color(quiet).child("Total"))
                                .child(
                                    div()
                                        .text_title()
                                        .font_family(mono.clone())
                                        .child(format(total)),
                                ),
                        )
                        .into_any_element()
                }
            }
        };

        // The type switch: one segmented tab per kind, icon only.
        let switch = (kinds.len() > 1).then(|| {
            let selected = kinds.iter().position(|k| *k == kind).unwrap_or(0);
            let state = kind_state.clone();
            let handler = self.on_kind_change.clone();
            let pick_reset = pick_state.clone();
            let list = kinds.clone();
            TabBar::new(part("kinds"))
                .segmented()
                .small()
                .selected_index(selected)
                .children(kinds.iter().map(|kind| {
                    Tab::new()
                        .icon(Icon::new(kind.icon()))
                        .aria_label(kind.label())
                }))
                .on_click(move |ix: &usize, window, cx| {
                    let Some(next) = list.get(*ix).copied() else {
                        return;
                    };
                    state.update(cx, |state, cx| {
                        state.shown = next;
                        cx.notify();
                    });
                    pick_reset.update(cx, |pick, _| *pick = None);
                    if let Some(handler) = &handler {
                        handler(next, window, cx);
                    }
                })
        });

        let legend = h_flex()
            .flex_wrap()
            .gap_1()
            .children(self.series.iter().enumerate().map(|(ix, series)| {
                let shown = !hidden.contains(&ix);
                let color = colors[ix];
                let state = hidden_state.clone();
                let handler = self.on_toggle.clone();
                let total = format(series.values.iter().sum());
                Button::new(part(&format!("series-{ix}")))
                    .ghost()
                    .small()
                    .toggled(shown)
                    .accessibility_label(format!(
                        "{}, {}, total {total}",
                        series.name,
                        if shown { "shown" } else { "hidden" }
                    ))
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .child(
                                div()
                                    .size(px(8.))
                                    .rounded(px(2.))
                                    .border_1()
                                    .border_color(color)
                                    .when(shown, |this| this.bg(color)),
                            )
                            .child(
                                div()
                                    .text_label_small()
                                    .when(!shown, |this| this.text_color(quiet).line_through())
                                    .child(series.name.clone()),
                            )
                            .child(
                                div()
                                    .text_caption()
                                    .font_family(mono.clone())
                                    .text_color(quiet)
                                    .child(total),
                            ),
                    )
                    .on_click(move |_, window, cx| {
                        state.update(cx, |hidden, cx| {
                            if !hidden.remove(&ix) {
                                hidden.insert(ix);
                            }
                            cx.notify();
                        });
                        if let Some(handler) = &handler {
                            handler(ix, !shown, window, cx);
                        }
                    })
            }));

        let words =
            pick.map(|at| readout(kind, &self.categories, &self.series, &hidden, at, &format));
        let spoken = format!(
            "{}{}",
            summary(
                &self.title,
                kind,
                &self.categories,
                &self.series,
                &hidden,
                &format
            ),
            words
                .as_ref()
                .map(|words| format!(". Picked {words}"))
                .unwrap_or_default()
        );
        let footer = words.clone().unwrap_or_else(|| {
            let total: f64 = target.iter().flatten().sum();
            format!(
                "Total {} across {} {}",
                format(total),
                self.categories.len(),
                if self.categories.len() == 1 {
                    "category"
                } else {
                    "categories"
                }
            )
        });

        let key_pick = pick_state;
        shell(cx)
            .id(self.id.clone())
            .max_w(px(640.))
            .child(shell_header(
                Some(Icon::new(CatalogIcon::Analytics)),
                self.title.clone(),
                self.subtitle.clone().map(|line| line.into_any_element()),
                switch.map(|switch| switch.into_any_element()),
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
                            .h(self.height)
                            .rounded(px(8.))
                            .role(Role::Group)
                            .aria_label(spoken)
                            .tab_index(0)
                            .focus_visible(|this| this.border_2().border_color(cx.theme().ring))
                            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                                let now = *key_pick.read(cx);
                                let Some(next) =
                                    step_pick(now, event.keystroke.key.as_str(), pick_len)
                                else {
                                    return;
                                };
                                key_pick.update(cx, |pick, cx| {
                                    *pick = next;
                                    cx.notify();
                                });
                                window.refresh();
                                cx.stop_propagation();
                            })
                            .child(chart),
                    )
                    .child(
                        div()
                            .min_h(px(16.))
                            .text_caption()
                            .text_color(if words.is_some() {
                                cx.theme().foreground
                            } else {
                                quiet
                            })
                            .child(footer),
                    ),
            )
            .refine_style(&self.style)
    }
}

/// Columns stacked from the first drawn series up, on the base plot pieces.
#[derive(IntoPlot)]
struct StackedColumns {
    id: ElementId,
    rows: Vec<Row>,
    series: Vec<usize>,
    names: Vec<SharedString>,
    colors: Vec<Hsla>,
    top: f64,
    pick: Option<usize>,
    format: Format,
}

impl StackedColumns {
    /// The left edge and width of every column, and the step between them.
    fn columns(&self, width: f32) -> (Vec<f32>, f32, f32) {
        let count = self.rows.len().max(1);
        let step = (width - VALUE_GUTTER) / count as f32;
        let wide = (step * 0.56).clamp(4., 40.);
        let lefts = (0..self.rows.len())
            .map(|ix| VALUE_GUTTER + step * ix as f32 + (step - wide) / 2.)
            .collect();
        (lefts, wide, step)
    }

    fn y(&self, value: f64, plot_h: f32) -> f32 {
        plot_h - (value / self.top) as f32 * (plot_h - 8.)
    }
}

impl Plot for StackedColumns {
    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let theme = cx.theme();
        let width = f32::from(bounds.size.width);
        let plot_h = f32::from(bounds.size.height) - LABEL_GUTTER;
        let (lefts, wide, step) = self.columns(width);
        let ticks: Vec<f64> = (0..=4).map(|ix| self.top * ix as f64 / 4.).collect();

        Grid::new()
            .y(ticks.iter().skip(1).map(|tick| px(self.y(*tick, plot_h))))
            .stroke(theme.border)
            .dash_array(&[px(4.), px(2.)])
            .paint(&bounds, window);

        if let Some(pick) = self.pick {
            let left = VALUE_GUTTER + step * pick as f32;
            window.paint_quad(
                fill(
                    Bounds::new(
                        bounds.origin + point(px(left), px(0.)),
                        size(px(step), px(plot_h)),
                    ),
                    theme.foreground.opacity(0.05),
                )
                .corner_radii(px(6.)),
            );
        }

        for (at, row) in self.rows.iter().enumerate() {
            let mut base = 0.;
            let last = self
                .series
                .iter()
                .rposition(|ix| row.values[*ix] > GONE)
                .unwrap_or(0);
            for (place, ix) in self.series.iter().enumerate() {
                let value = row.values[*ix];
                if value <= GONE {
                    continue;
                }
                let y0 = self.y(base, plot_h);
                let y1 = self.y(base + value, plot_h);
                base += value;
                // A hairline gap between segments; the top one is rounded.
                let height = (y0 - y1 - 1.).max(0.5);
                let radius = if place == last { px(3.) } else { px(0.) };
                window.paint_quad(
                    fill(
                        Bounds::new(
                            bounds.origin + point(px(lefts[at]), px(y1)),
                            size(px(wide), px(height)),
                        ),
                        self.colors[*ix],
                    )
                    .corner_radii(Corners {
                        top_left: radius,
                        top_right: radius,
                        bottom_left: px(0.),
                        bottom_right: px(0.),
                    }),
                );
            }
        }

        let quiet = theme.muted_foreground;
        PlotAxis::new()
            .x(plot_h)
            .x_label(self.rows.iter().enumerate().map(|(at, row)| {
                AxisText::new(row.label.clone(), px(lefts[at] + wide / 2.), quiet)
                    .align(TextAlign::Center)
            }))
            .stroke(theme.border)
            .paint(&bounds, window, cx);
        PlotAxis::new()
            .y(px(VALUE_GUTTER - 8.))
            .y_axis(false)
            .y_label_side(AxisLabelSide::Start)
            .y_label(ticks.iter().map(|tick| {
                AxisText::new((self.format)(*tick), px(self.y(*tick, plot_h) - 6.), quiet)
                    .align(TextAlign::Right)
            }))
            .paint(&bounds, window, cx);
    }

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn tooltip_state(
        &self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        _: &App,
    ) -> Option<TooltipState> {
        let width = f32::from(bounds.size.width);
        let plot_h = f32::from(bounds.size.height) - LABEL_GUTTER;
        let x = f32::from(position.x);
        if f32::from(position.y) > plot_h || x < VALUE_GUTTER || self.rows.is_empty() {
            return None;
        }
        let (lefts, wide, step) = self.columns(width);
        let at = (((x - VALUE_GUTTER) / step).floor() as usize).min(self.rows.len() - 1);
        Some(TooltipState::new(
            at,
            point(px(lefts[at] + wide / 2.), position.y),
            vec![],
        ))
    }

    fn tooltip(
        &self,
        state: &TooltipState,
        cursor: Point<Pixels>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<AnyElement> {
        let row = self.rows.get(state.index)?;
        let plot_h = f32::from(bounds.size.height) - LABEL_GUTTER;
        let (_, wide, _) = self.columns(f32::from(bounds.size.width));
        let mut tooltip = Tooltip::new(cursor, bounds.size)
            .gap(px(8.))
            .cross_line(
                CrossLine::new(state.cross_line)
                    .height(plot_h)
                    .band(px(wide)),
            )
            .title(row.label.clone());
        let mut total = 0.;
        for ix in self.series.iter().rev() {
            let value = row.values[*ix];
            total += value;
            tooltip = tooltip.row(
                self.colors[*ix],
                self.names[*ix].clone(),
                (self.format)(value),
            );
        }
        if self.series.len() > 1 {
            tooltip = tooltip.plain_row("Total", (self.format)(total));
        }
        Some(tooltip.into_any_element())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two() -> Vec<ChartSeries> {
        vec![
            ChartSeries::new("Seeds", vec![1., 2., 3.]),
            ChartSeries::new("Bulbs", vec![4., 5.]),
        ]
    }

    #[test]
    fn hidden_series_head_for_zero_and_short_ones_pad() {
        let hidden = BTreeSet::from([1]);
        assert_eq!(
            targets(&two(), &hidden, 3),
            vec![vec![1., 2., 3.], vec![0., 0., 0.]]
        );
        let none = BTreeSet::new();
        assert_eq!(targets(&two(), &none, 3)[1], vec![4., 5., 0.]);
    }

    #[test]
    fn blending_glides_and_fills_missing_values_with_zero() {
        let from = vec![vec![0., 10.]];
        let to = vec![vec![10., 0.], vec![4.]];
        assert_eq!(blend(&from, &to, 0.5), vec![vec![5., 5.], vec![2.]]);
        assert_eq!(blend(&from, &to, 1.), to);
        assert_eq!(blend(&[], &to, 0.), vec![vec![0., 0.], vec![0.]]);
        assert_eq!(ease_out(0.), 0.);
        assert_eq!(ease_out(1.), 1.);
        assert_eq!(ease_out(2.), 1.);
        assert!(ease_out(0.5) > 0.5);
    }

    #[test]
    fn a_hidden_series_stays_drawn_until_it_reaches_zero() {
        let hidden = BTreeSet::from([0]);
        assert_eq!(drawn(&hidden, &[vec![0.2], vec![1.]]), vec![0, 1]);
        assert_eq!(drawn(&hidden, &[vec![0.], vec![1.]]), vec![1]);
    }

    #[test]
    fn the_axis_ends_on_a_round_number() {
        assert_eq!(nice_ceiling(0.), 1.);
        assert_eq!(nice_ceiling(7.), 10.);
        assert_eq!(nice_ceiling(18.), 20.);
        assert_eq!(nice_ceiling(21.), 25.);
        assert_eq!(nice_ceiling(420.), 500.);
        assert_eq!(nice_ceiling(1000.), 1000.);
    }

    #[test]
    fn numbers_write_short() {
        assert_eq!(compact(950.), "950");
        assert_eq!(compact(1200.), "1.2k");
        assert_eq!(compact(40_000.), "40k");
        assert_eq!(compact(3_450_000.), "3.5M");
        assert_eq!(compact(2.5), "2.5");
        assert_eq!(percent(1., 4.), "25%");
        assert_eq!(percent(1., 0.), "0%");
    }

    #[test]
    fn arrow_keys_step_and_stop_at_the_ends() {
        assert_eq!(step_pick(None, "right", 3), Some(Some(0)));
        assert_eq!(step_pick(None, "left", 3), Some(Some(2)));
        assert_eq!(step_pick(Some(2), "right", 3), Some(Some(2)));
        assert_eq!(step_pick(Some(0), "left", 3), Some(Some(0)));
        assert_eq!(step_pick(Some(1), "end", 3), Some(Some(2)));
        assert_eq!(step_pick(Some(1), "home", 3), Some(Some(0)));
        assert_eq!(step_pick(Some(1), "escape", 3), Some(None));
        assert_eq!(step_pick(Some(1), "a", 3), None);
        assert_eq!(step_pick(None, "right", 0), None);
    }

    #[test]
    fn the_readout_says_every_shown_series() {
        let format: Format = Rc::new(compact);
        let months: Vec<SharedString> = vec!["Jan".into(), "Feb".into(), "Mar".into()];
        let none = BTreeSet::new();
        assert_eq!(
            readout(ChartKind::Bar, &months, &two(), &none, 1, &format),
            "Feb: Seeds 2, Bulbs 5, total 7"
        );
        let hidden = BTreeSet::from([1]);
        assert_eq!(
            readout(ChartKind::Line, &months, &two(), &hidden, 2, &format),
            "Mar: Seeds 3"
        );
        assert_eq!(
            readout(ChartKind::Donut, &months, &two(), &none, 1, &format),
            "Bulbs: 9, 60% of the total"
        );
        assert_eq!(
            summary("Plants", ChartKind::Area, &months, &two(), &none, &format),
            "Plants, Area chart, Jan to Mar. Totals: Seeds 6, Bulbs 9"
        );
        let all = BTreeSet::from([0, 1]);
        assert_eq!(
            summary("Plants", ChartKind::Bar, &months, &two(), &all, &format),
            "Plants, Bar chart, Jan to Mar: every series hidden"
        );
    }
}
