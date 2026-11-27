// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Diverging bars: stacked columns, with one series hanging below zero.
//!
//! Series stack upward as separate rounded segments. One more series can hang
//! under the zero line on its own, smaller scale, so a small count that
//! matters stays readable next to a big one. Each series can be switched off
//! from the legend. Pointing at a column, or stepping with the arrow keys,
//! lists its values.

use std::{cell::Cell, rc::Rc, time::Duration};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::{Presence, TestSupportExt as _, Toggle, Transition, transition};
use isonomic_ui::component::{
    ActiveTheme as _, Icon, StyledExt as _, TextRole as _, h_flex, rolling_number::RollingNumber,
};
use isonomic_ui::{
    AnyElement, App, Bounds, Div, ElementId, Hsla, InteractiveElement as _, IntoElement,
    KeyDownEvent, ParentElement as _, PathBuilder, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, canvas, div, fill, point,
    prelude::FluentBuilder as _, px, size,
};

use crate::common::{panel, shell, shell_header};

/// Room above the plot, right of it for the scale, and under it for labels.
const TOP: f32 = 10.;
const RIGHT: f32 = 34.;
const AXIS: f32 = 26.;
/// The height of the stacks, and of the series hanging below.
const UP: f32 = 150.;
const DOWN: f32 = 40.;
/// Space between the zero line and the first segment on either side.
const ZERO_GAP: f32 = 3.;
/// The plot's width before it has been laid out once.
const FIRST_WIDTH: f32 = 628.;

/// One series of values, one per column.
#[derive(Debug, Clone, PartialEq)]
pub struct BarSeries {
    name: SharedString,
    values: Vec<f64>,
    color: Option<Hsla>,
}

impl BarSeries {
    /// A series with one value per column. Values below zero count as zero.
    pub fn new(name: impl Into<SharedString>, values: impl IntoIterator<Item = f64>) -> Self {
        Self {
            name: name.into(),
            values: values.into_iter().map(|value| value.max(0.)).collect(),
            color: None,
        }
    }

    /// Its colour. By default series take the data colours in order.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }

    fn at(&self, column: usize) -> f64 {
        self.values.get(column).copied().unwrap_or(0.)
    }
}

/// A round upper bound, so the scale ends on a readable number.
pub fn round_ceiling(value: f64) -> f64 {
    if value <= 0. {
        return 1.;
    }
    let half = 10_f64.powi(value.log10().floor() as i32) / 2.;
    (value / half).ceil() * half
}

/// A value in short form: 940, 1.2k, 3.4M.
pub fn short_number(value: f64) -> String {
    let trim = |scaled: f64, unit: &str| {
        let text = format!("{scaled:.1}");
        format!("{}{unit}", text.trim_end_matches('0').trim_end_matches('.'))
    };
    if value.abs() >= 1_000_000. {
        trim(value / 1_000_000., "M")
    } else if value.abs() >= 1000. {
        trim(value / 1000., "k")
    } else {
        format!("{}", value.round())
    }
}

/// The segments of one column, bottom to top, for the values of its series
/// on a scale that ends at `top`. Each is how far its upper edge sits above
/// the zero line and how tall it is drawn; every segment gives up `gap` at
/// its upper end, and a tiny one keeps one pixel. `None` for an empty value.
pub fn column_segments(values: &[f64], top: f64, gap: f32) -> Vec<Option<(f32, f32)>> {
    let mut reach = ZERO_GAP;
    values
        .iter()
        .map(|value| {
            let height = (value / top.max(f64::MIN_POSITIVE)) as f32 * (UP - ZERO_GAP);
            if height <= 0. {
                return None;
            }
            reach += height;
            let drawn = (height - gap).max(1.);
            Some((reach - (height - drawn), drawn))
        })
        .collect()
}

/// How many columns apart labels sit when the plot is `inner` wide: one
/// about every 56 pixels.
pub fn label_spacing(count: usize, inner: f32) -> usize {
    let room = ((inner / 56.).floor() as usize).max(1);
    count.div_ceil(room).max(1)
}

/// The legend after series `ix` is switched: a hidden one comes back, a
/// shown one hides, unless it is the last stacked series still shown. The
/// first `stacked` entries are the stack; one more may follow for the series
/// below the line.
pub fn switch_series(hidden: &[bool], ix: usize, stacked: usize) -> Vec<bool> {
    let mut next = hidden.to_vec();
    let Some(now) = hidden.get(ix).copied() else {
        return next;
    };
    let shown = hidden.iter().take(stacked).filter(|off| !**off).count();
    if now || ix >= stacked || shown > 1 {
        next[ix] = !now;
    }
    next
}

/// One column read aloud: its total, then each value, largest first.
pub fn column_reading(label: &str, total: &str, rows: &[(SharedString, String)]) -> String {
    let each: Vec<String> = rows
        .iter()
        .map(|(name, value)| format!("{name} {value}"))
        .collect();
    format!("{label}: total {total}, {}", each.join(", "))
}

fn step_column(now: Option<usize>, key: &str, count: usize) -> Option<Option<usize>> {
    if count == 0 {
        return None;
    }
    Some(match key {
        "right" => Some(now.map_or(0, |at| (at + 1) % count)),
        "left" => Some(now.map_or(count - 1, |at| (at + count - 1) % count)),
        "home" => Some(0),
        "end" => Some(count - 1),
        "escape" => None,
        _ => return None,
    })
}

type Format = Rc<dyn Fn(f64) -> String>;
type SelectHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// Stacked columns with one series below the zero line.
#[derive(IntoElement)]
pub struct DivergingBars {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    subtitle: Option<SharedString>,
    labels: Vec<SharedString>,
    series: Vec<BarSeries>,
    below: Option<BarSeries>,
    headline: Option<(SharedString, Option<SharedString>)>,
    aside: Option<AnyElement>,
    format: Format,
    on_select: Option<SelectHandler>,
}

impl DivergingBars {
    /// Create a chart with one label per column.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        labels: Vec<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            subtitle: None,
            labels,
            series: Vec::new(),
            below: None,
            headline: None,
            aside: None,
            format: Rc::new(short_number),
            on_select: None,
        }
    }

    /// A quieter line under the title.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// The series stacked upward, the first at the bottom.
    pub fn series(mut self, series: impl IntoIterator<Item = BarSeries>) -> Self {
        self.series = series.into_iter().collect();
        self
    }

    /// A series hanging below the zero line on its own scale. It takes the
    /// danger colour unless it has one.
    pub fn below(mut self, below: BarSeries) -> Self {
        self.below = Some(below);
        self
    }

    /// A large figure above the chart. It rolls when it changes.
    pub fn headline(mut self, value: impl Into<SharedString>) -> Self {
        self.headline = Some((value.into(), None));
        self
    }

    /// A few words beside the headline.
    pub fn caption(mut self, caption: impl Into<SharedString>) -> Self {
        if let Some(headline) = &mut self.headline {
            headline.1 = Some(caption.into());
        }
        self
    }

    /// Whatever belongs at the far end of the headline row.
    pub fn aside(mut self, aside: impl IntoElement) -> Self {
        self.aside = Some(aside.into_any_element());
        self
    }

    /// How a value is written on the scale and in the details.
    pub fn format(mut self, format: impl Fn(f64) -> String + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }

    /// Run with the column the reader points at, clicks or steps to.
    pub fn on_select(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Styled for DivergingBars {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for DivergingBars {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let part = |name: String| ElementId::NamedChild(self.id.clone().into(), name.into());
        let count = self.labels.len();
        let stacked = self.series.len();
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let raised = theme.popover;
        let mono = theme.mono_font_family.clone();
        let palette = [
            theme.chart_1,
            theme.chart_2,
            theme.chart_5,
            theme.chart_3,
            theme.chart_4,
        ];
        let danger = theme.danger;
        let format = self.format.clone();

        // Every series in legend order: the stack, then the one below.
        let all: Vec<(&BarSeries, Hsla)> = self
            .series
            .iter()
            .enumerate()
            .map(|(ix, one)| (one, one.color.unwrap_or(palette[ix % palette.len()])))
            .chain(
                self.below
                    .iter()
                    .map(|one| (one, one.color.unwrap_or(danger))),
            )
            .collect();
        let has_below = self.below.is_some();

        let hidden_state = window.use_keyed_state(part("hidden".into()), cx, |_, _| Vec::new());
        let mut hidden: Vec<bool> = hidden_state.read(cx).clone();
        hidden.resize(all.len(), false);
        let below_on = has_below && !hidden[stacked];
        let active = window.use_keyed_state(part("active".into()), cx, |_, _| None::<usize>);
        let picked = (*active.read(cx)).filter(|ix| *ix < count);

        let seen = window
            .use_keyed_state(part("width".into()), cx, |_, _| {
                Rc::new(Cell::new(FIRST_WIDTH))
            })
            .read(cx)
            .clone();
        let width = seen.get();
        let inner = (width - RIGHT).max(40.);
        let band = inner / count.max(1) as f32;
        let bar = (band * 0.64).min(22.);
        let gap = if bar < 8. { 1.5 } else { 2. };
        let corner = (bar / 3.).min(3.);
        let x = move |ix: usize| band * (ix as f32 + 0.5);
        let zero = TOP + UP;
        let reach = if has_below { ZERO_GAP + DOWN } else { 0. };
        let height = zero + reach + AXIS;

        let totals: Vec<f64> = (0..count)
            .map(|ix| {
                (0..stacked)
                    .filter(|series| !hidden[*series])
                    .map(|series| self.series[series].at(ix))
                    .sum()
            })
            .collect();
        let up_top = round_ceiling(totals.iter().copied().fold(0., f64::max));
        let down_top = self.below.as_ref().map_or(1., |one| {
            round_ceiling(one.values.iter().copied().fold(0., f64::max))
        });

        // Switching a series tweens its share and the scale together.
        let slow = Transition::new(Duration::from_millis(450));
        let shares: Vec<f32> = hidden
            .iter()
            .enumerate()
            .map(|(ix, off)| {
                transition(
                    part(format!("share-{ix}")),
                    if *off { 0_f32 } else { 1. },
                    slow.clone(),
                    window,
                    cx,
                )
            })
            .collect();
        let scale = transition(part("scale".into()), up_top as f32, slow, window, cx);
        let dimming = transition(
            part("dim".into()),
            if picked.is_some() { 1_f32 } else { 0. },
            Transition::new(Duration::from_millis(150)),
            window,
            cx,
        );
        let stagger = (0.6 / count.max(1) as f32).min(0.02);
        let appear_time = Duration::from_secs_f32(0.45 + stagger * count as f32);
        let appeared = Presence::new(part("appear".into()), true)
            .transition(Transition::new(appear_time).ease(|time| time))
            .sample(window, cx)
            .progress;
        let elapsed = appeared * appear_time.as_secs_f32();

        let columns: Vec<(Vec<f64>, f64)> = (0..count)
            .map(|ix| {
                let stack = (0..stacked)
                    .map(|series| self.series[series].at(ix) * f64::from(shares[series]))
                    .collect();
                let hang = self
                    .below
                    .as_ref()
                    .map_or(0., |one| one.at(ix) * f64::from(shares[stacked]));
                (stack, hang)
            })
            .collect();
        let colors: Vec<Hsla> = all.iter().map(|(_, color)| *color).collect();
        let below_color = colors.get(stacked).copied().unwrap_or(danger);
        let grid = quiet.opacity(0.1);
        let bars = canvas(
            move |bounds, window, _| {
                let now = f32::from(bounds.size.width);
                if (now - seen.get()).abs() > 0.5 {
                    seen.set(now);
                    window.request_animation_frame();
                }
            },
            move |bounds, _, window, _| {
                let at = |x: f32, y: f32| point(bounds.left() + px(x), bounds.top() + px(y));
                for share in [0.5, 1.] {
                    let y = zero - share * (UP - ZERO_GAP);
                    let mut line = PathBuilder::stroke(px(1.)).dash_array(&[px(2.), px(4.)]);
                    line.move_to(at(0., y));
                    line.line_to(at(inner, y));
                    if let Ok(line) = line.build() {
                        window.paint_path(line, grid);
                    }
                }
                window.paint_quad(fill(
                    Bounds::new(at(0., zero - 0.5), size(px(inner), px(1.))),
                    quiet.opacity(0.22),
                ));
                for (ix, (stack, hang)) in columns.iter().enumerate() {
                    // Columns rise from the line, left to right.
                    let linear = ((elapsed - stagger * ix as f32) / 0.45).clamp(0., 1.);
                    let grown = 1. - (1. - linear).powi(3);
                    let strength = if picked.is_some_and(|at| at != ix) {
                        1. - 0.6 * dimming
                    } else {
                        1.
                    };
                    let left = x(ix) - bar / 2.;
                    let segments = column_segments(stack, f64::from(scale), gap);
                    for (series, segment) in segments.iter().enumerate() {
                        let Some((rise, tall)) = segment else {
                            continue;
                        };
                        let rise = ZERO_GAP + (rise - ZERO_GAP) * grown;
                        let color = colors[series];
                        window.paint_quad(
                            fill(
                                Bounds::new(at(left, zero - rise), size(px(bar), px(tall * grown))),
                                color.opacity(color.a * strength),
                            )
                            .corner_radii(px(corner.min(tall * grown / 2.))),
                        );
                    }
                    if *hang > 0. {
                        let tall = ((hang / down_top) as f32 * DOWN).max(1.) * grown;
                        let color = colors[stacked];
                        window.paint_quad(
                            fill(
                                Bounds::new(at(left, zero + ZERO_GAP), size(px(bar), px(tall))),
                                color.opacity(color.a * strength),
                            )
                            .corner_radii(px(corner.min(tall / 2.))),
                        );
                    }
                }
            },
        )
        .absolute()
        .inset_0();

        // The scale down the right: the stack's, zero, the one below's.
        let scale_label = |y: f32, text: String, color: Hsla| {
            div()
                .absolute()
                .right(px(4.))
                .top(px(y - 7.))
                .text_size(px(10.))
                .line_height(px(14.))
                .text_color(color)
                .child(text)
        };
        let scale_labels = [0.5, 1.]
            .into_iter()
            .map(|share| {
                scale_label(
                    zero - share as f32 * (UP - ZERO_GAP),
                    format(up_top * share),
                    quiet,
                )
            })
            .chain([scale_label(zero, format(0.), quiet)])
            .chain(
                has_below
                    .then(|| scale_label(zero + ZERO_GAP + DOWN, format(down_top), below_color)),
            );

        let band_w = band.min(bar + 8.);
        let highlight = picked.map(|ix| {
            div()
                .absolute()
                .left(px(x(ix) - band_w / 2.))
                .top(px(TOP - 6.))
                .w(px(band_w))
                .h(px(UP + reach + 12.))
                .rounded(px((bar / 2. + 4.).min(8.)))
                .bg(ink.opacity(0.06))
        });

        let every = label_spacing(count, inner);
        let axis = self
            .labels
            .iter()
            .enumerate()
            .filter(|(ix, _)| ix % every == 0)
            .map(|(ix, label)| {
                let text = div()
                    .flex_none()
                    .whitespace_nowrap()
                    .text_size(px(10.))
                    .line_height(px(14.))
                    .text_color(if picked == Some(ix) { ink } else { quiet })
                    .child(label.clone());
                // The first label starts at its bar's edge; the rest are centred.
                let place = div().absolute().top(px(height - 18.));
                if ix == 0 {
                    place.left(px((x(0) - bar / 2.).max(0.))).child(text)
                } else {
                    place
                        .left(px(x(ix)))
                        .w(px(0.))
                        .flex()
                        .justify_center()
                        .child(text)
                }
            });

        // One invisible strip per column takes the pointer, both sides of the line.
        let hits = (0..count).map(|ix| {
            let active = active.clone();
            let hover_select = self.on_select.clone();
            let click_state = active.clone();
            let click_select = self.on_select.clone();
            div()
                .id(part(format!("column-{ix}")))
                .test_support()
                .flex_1()
                .h_full()
                .on_hover(move |over, window, cx| {
                    if *over || *active.read(cx) == Some(ix) {
                        active.update(cx, |state, cx| {
                            *state = over.then_some(ix);
                            cx.notify();
                        });
                    }
                    if let (Some(select), true) = (&hover_select, *over) {
                        select(ix, window, cx);
                    }
                })
                .on_click(move |_, window, cx| {
                    click_state.update(cx, |state, cx| {
                        *state = Some(ix);
                        cx.notify();
                    });
                    if let Some(select) = &click_select {
                        select(ix, window, cx);
                    }
                })
        });

        // The picked column's values, largest first.
        let rows = |ix: usize| {
            let mut rows: Vec<(SharedString, Hsla, f64)> = all
                .iter()
                .enumerate()
                .filter(|(series, _)| !hidden[*series])
                .map(|(_, (one, color))| (one.name.clone(), *color, one.at(ix)))
                .collect();
            rows.sort_by(|a, b| b.2.total_cmp(&a.2));
            rows
        };
        let tip = picked.map(|ix| {
            let from_right = x(ix) > width - 190.;
            div()
                .absolute()
                .top_0()
                .map(|this| {
                    if from_right {
                        this.right(px((width - x(ix) + band / 2. + 6.).max(0.)))
                    } else {
                        this.left(px((x(ix) + band / 2. + 6.).max(0.)))
                    }
                })
                .min_w(px(160.))
                .px_3()
                .py_2()
                .rounded(px(12.))
                .bg(raised)
                .shadow_md()
                .text_caption()
                .child(
                    h_flex()
                        .mb_1()
                        .justify_between()
                        .gap_4()
                        .font_medium()
                        .child(self.labels[ix].clone())
                        .child(div().font_family(mono.clone()).child(format(totals[ix]))),
                )
                .children(rows(ix).into_iter().map(|(name, color, value)| {
                    h_flex()
                        .gap_2()
                        .text_color(quiet)
                        .child(div().flex_none().size(px(8.)).rounded(px(2.)).bg(color))
                        .child(div().flex_1().child(name))
                        .child(
                            div()
                                .font_family(mono.clone())
                                .text_color(ink)
                                .child(format(value)),
                        )
                }))
        });

        let spoken = match picked {
            Some(ix) => column_reading(
                &self.labels[ix],
                &format(totals[ix]),
                &rows(ix)
                    .into_iter()
                    .map(|(name, _, value)| (name, format(value)))
                    .collect::<Vec<_>>(),
            ),
            None => format!(
                "{}: stacked columns of {}{}, {} to {}, {} in total. Arrow keys read each column.",
                self.title,
                all.iter()
                    .take(stacked)
                    .enumerate()
                    .filter(|(ix, _)| !hidden[*ix])
                    .map(|(_, (one, _))| one.name.to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
                match (&self.below, below_on) {
                    (Some(one), true) => format!(", with {} below the line", one.name),
                    _ => String::new(),
                },
                self.labels.first().cloned().unwrap_or_default(),
                self.labels.last().cloned().unwrap_or_default(),
                format(totals.iter().sum()),
            ),
        };

        let legend = h_flex()
            .id(part("legend".into()))
            .role(Role::Group)
            .aria_label("Series")
            .mb_3()
            .flex_wrap()
            .gap(px(6.))
            .children(all.iter().enumerate().map(|(ix, (one, color))| {
                let on = !hidden[ix];
                let state = hidden_state.clone();
                let now = hidden.clone();
                let swatch: Div = div().flex_none().size(px(10.)).rounded(px(3.));
                Toggle::new(part(format!("series-{ix}")))
                    .pressed(on)
                    .accessibility_label(if on {
                        format!("{}, shown", one.name)
                    } else {
                        format!("{}, hidden", one.name)
                    })
                    .on_change(move |_, _, _, cx| {
                        let next = switch_series(&now, ix, stacked);
                        state.update(cx, |state, cx| {
                            *state = next;
                            cx.notify();
                        });
                    })
                    .h(px(28.))
                    .gap(px(6.))
                    .px(px(10.))
                    .rounded_full()
                    .cursor_pointer()
                    .text_caption()
                    .when(has_below && ix == stacked, |this| this.ml_auto())
                    .map(|this| {
                        if on {
                            this.bg(ink.opacity(0.06))
                                .text_color(ink)
                                .child(swatch.bg(*color))
                        } else {
                            this.text_color(quiet)
                                .opacity(0.6)
                                .hover(|this| this.opacity(1.))
                                .child(swatch.border(px(1.5)).border_color(*color))
                        }
                    })
                    .child(one.name.clone())
            }));

        let top_row = (self.headline.is_some() || self.aside.is_some()).then(|| {
            h_flex()
                .mb_3()
                .flex_wrap()
                .items_baseline()
                .justify_between()
                .gap_x_4()
                .gap_y_1()
                .when_some(self.headline.clone(), |this, (value, caption)| {
                    this.child(
                        h_flex()
                            .items_baseline()
                            .gap_x_2()
                            .child(
                                div().text_size(px(24.)).font_medium().child(
                                    RollingNumber::new(part("headline".into()), value)
                                        .line_height(px(32.)),
                                ),
                            )
                            .when_some(caption, |this, caption| {
                                this.child(
                                    div()
                                        .text_size(px(14.))
                                        .line_height(px(20.))
                                        .text_color(quiet)
                                        .child(caption),
                                )
                            }),
                    )
                })
                .when_some(self.aside, |this, aside| {
                    this.child(
                        div()
                            .text_size(px(14.))
                            .line_height(px(20.))
                            .text_color(quiet)
                            .child(aside),
                    )
                })
        });

        let key_state = active.clone();
        let key_select = self.on_select.clone();
        shell(cx)
            .id(self.id.clone())
            .max_w(px(672.))
            .child(shell_header(
                Some(Icon::new(CatalogIcon::ChartColumn)),
                self.title.clone(),
                self.subtitle.clone().map(|line| line.into_any_element()),
                None,
                cx,
            ))
            .child(
                panel(cx).p_4().children(top_row).child(legend).child(
                    div()
                        .id(part("plot".into()))
                        .test_support()
                        .relative()
                        .w_full()
                        .h(px(height))
                        .rounded(px(12.))
                        .role(Role::Group)
                        .aria_label(spoken)
                        .tab_index(0)
                        .on_key_down(move |event: &KeyDownEvent, window, cx| {
                            let now = *key_state.read(cx);
                            let Some(next) = step_column(now, event.keystroke.key.as_str(), count)
                            else {
                                return;
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
                        .children(highlight)
                        .child(bars)
                        .children(scale_labels)
                        .children(axis)
                        .child(
                            h_flex()
                                .absolute()
                                .left_0()
                                .top(px(TOP - 6.))
                                .w(px(inner))
                                .h(px(UP + reach + 12.))
                                .children(hits),
                        )
                        .children(tip),
                ),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_ends_on_a_round_number() {
        assert_eq!(round_ceiling(0.), 1.);
        assert_eq!(round_ceiling(7.), 7.);
        assert_eq!(round_ceiling(7.2), 7.5);
        assert_eq!(round_ceiling(83.), 85.);
        assert_eq!(round_ceiling(100.), 100.);
        assert_eq!(round_ceiling(1_340.), 1_500.);
    }

    #[test]
    fn values_are_written_short() {
        assert_eq!(short_number(0.), "0");
        assert_eq!(short_number(940.4), "940");
        assert_eq!(short_number(1_000.), "1k");
        assert_eq!(short_number(1_240.), "1.2k");
        assert_eq!(short_number(3_400_000.), "3.4M");
    }

    #[test]
    fn segments_stack_with_a_gap_between_them() {
        // Half and half of the scale: each 73.5 tall before its gap.
        let segments = column_segments(&[50., 0., 50.], 100., 2.);
        assert_eq!(segments[0], Some((3. + 73.5 - 2., 71.5)));
        assert_eq!(segments[1], None);
        // The last one ends exactly at the top of the plot, less its gap.
        assert_eq!(segments[2], Some((150. - 2., 71.5)));
        // A sliver keeps one pixel.
        let sliver = column_segments(&[0.5], 100., 2.);
        assert_eq!(sliver[0].map(|(_, tall)| tall), Some(1.));
    }

    #[test]
    fn the_last_stacked_series_stays_on() {
        // Two stacked and one below, all shown.
        let all_on = [false, false, false];
        assert_eq!(switch_series(&all_on, 0, 2), [true, false, false]);
        // The only stacked series left cannot be hidden.
        assert_eq!(
            switch_series(&[true, false, false], 1, 2),
            [true, false, false]
        );
        // The one below always can, and a hidden one always comes back.
        assert_eq!(
            switch_series(&[true, false, false], 2, 2),
            [true, false, true]
        );
        assert_eq!(
            switch_series(&[true, false, true], 0, 2),
            [false, false, true]
        );
        assert_eq!(switch_series(&all_on, 9, 2), all_on);
    }

    #[test]
    fn labels_thin_out_and_a_column_reads_its_values() {
        assert_eq!(label_spacing(30, 594.), 3);
        assert_eq!(label_spacing(7, 594.), 1);
        assert_eq!(label_spacing(40, 100.), 40);
        assert_eq!(
            column_reading(
                "Tue 4",
                "1.2k",
                &[
                    ("Parcels".into(), "900".into()),
                    ("Letters".into(), "300".into())
                ]
            ),
            "Tue 4: total 1.2k, Parcels 900, Letters 300"
        );
    }
}
