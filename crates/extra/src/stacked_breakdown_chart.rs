// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Stacked breakdown chart: one set of records, split the way the reader asks.
//!
//! The owner hands over flat records, the dimensions they can be grouped by
//! and the measures that can be added up. The reader picks what to measure,
//! which dimension runs along the bars and which one stacks them. A table
//! under the bars repeats every number, so nothing depends on colour or on
//! pointing.

use std::rc::Rc;

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::Button,
    h_flex,
    menu::{DropdownMenu as _, PopupMenuItem},
    v_flex,
};
use isonomic_ui::{
    App, ElementId, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::{lifted_shadow, resting_shadow};

/// How tall the bars' area is.
const PLOT_HEIGHT: f32 = 180.;
/// The width of a number column in the table.
const NUMBER_COLUMN: f32 = 64.;

/// One thing that is counted: a task, a ticket, an order.
#[derive(Debug, Clone, Default)]
pub struct BreakdownRecord {
    fields: Vec<(SharedString, SharedString)>,
    numbers: Vec<(SharedString, f64)>,
}

impl BreakdownRecord {
    /// Create a record with no fields.
    pub fn new() -> Self {
        Self::default()
    }

    /// Say which value of a dimension the record has, by the dimension's key.
    pub fn field(mut self, key: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        self.fields.push((key.into(), value.into()));
        self
    }

    /// Give the record a number a measure can add up, such as hours.
    pub fn number(mut self, key: impl Into<SharedString>, value: f64) -> Self {
        self.numbers.push((key.into(), value));
        self
    }
}

/// A way to group the records, with its values in the order to show them.
#[derive(Debug, Clone)]
pub struct BreakdownDimension {
    key: SharedString,
    label: SharedString,
    values: Vec<(SharedString, Option<Hsla>)>,
}

impl BreakdownDimension {
    /// Create a dimension. Records point at it with `key`.
    pub fn new(key: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            values: Vec::new(),
        }
    }

    /// Add a value. Values without a colour take the theme's chart colours.
    pub fn value(mut self, name: impl Into<SharedString>) -> Self {
        self.values.push((name.into(), None));
        self
    }

    /// Add a value with its own colour.
    pub fn colored_value(mut self, name: impl Into<SharedString>, color: impl Into<Hsla>) -> Self {
        self.values.push((name.into(), Some(color.into())));
        self
    }

    /// Where the record's value sits among this dimension's values.
    fn position_of(&self, record: &BreakdownRecord) -> Option<usize> {
        let (_, value) = record.fields.iter().find(|(key, _)| *key == self.key)?;
        self.values.iter().position(|(name, _)| name == value)
    }
}

/// Something to add up per group.
#[derive(Debug, Clone)]
pub struct BreakdownMeasure {
    label: SharedString,
    unit: SharedString,
    field: Option<SharedString>,
}

impl BreakdownMeasure {
    /// Count the records.
    pub fn count(label: impl Into<SharedString>, unit: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            unit: unit.into(),
            field: None,
        }
    }

    /// Add up one of the records' numbers.
    pub fn sum(
        label: impl Into<SharedString>,
        unit: impl Into<SharedString>,
        field: impl Into<SharedString>,
    ) -> Self {
        Self {
            label: label.into(),
            unit: unit.into(),
            field: Some(field.into()),
        }
    }
}

/// Stacked bars with pickers for what they show, and a table of the numbers.
#[derive(IntoElement)]
pub struct StackedBreakdownChart {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    records: Vec<BreakdownRecord>,
    dimensions: Vec<BreakdownDimension>,
    measures: Vec<BreakdownMeasure>,
    default_measure: usize,
    default_slice: usize,
    default_segment: Option<usize>,
}

impl StackedBreakdownChart {
    /// Create a chart with a heading. It starts with the first measure, the
    /// first dimension along the bars and the second one stacking them.
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            records: Vec::new(),
            dimensions: Vec::new(),
            measures: Vec::new(),
            default_measure: 0,
            default_slice: 0,
            default_segment: Some(1),
        }
    }

    /// Set the records.
    pub fn records(mut self, records: impl IntoIterator<Item = BreakdownRecord>) -> Self {
        self.records = records.into_iter().collect();
        self
    }

    /// Add a dimension the records can be grouped by.
    pub fn dimension(mut self, dimension: BreakdownDimension) -> Self {
        self.dimensions.push(dimension);
        self
    }

    /// Add a measure. Without any, the records are counted.
    pub fn measure(mut self, measure: BreakdownMeasure) -> Self {
        self.measures.push(measure);
        self
    }

    /// Start with this measure, by position.
    pub fn default_measure(mut self, measure: usize) -> Self {
        self.default_measure = measure;
        self
    }

    /// Start with this dimension along the bars, by position.
    pub fn default_slice(mut self, dimension: usize) -> Self {
        self.default_slice = dimension;
        self
    }

    /// Start with this dimension stacking the bars, by position, or with
    /// plain bars.
    pub fn default_segment(mut self, dimension: Option<usize>) -> Self {
        self.default_segment = dimension;
        self
    }
}

impl Styled for StackedBreakdownChart {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// The measure per slice value and segment value. Without a segment
/// dimension every slice has one column. A record that names a value its
/// dimension does not list is left out.
fn tally(
    records: &[BreakdownRecord],
    slice: &BreakdownDimension,
    segment: Option<&BreakdownDimension>,
    measure: &BreakdownMeasure,
) -> Vec<Vec<f64>> {
    let columns = segment.map_or(1, |segment| segment.values.len());
    let mut table = vec![vec![0.; columns]; slice.values.len()];
    for record in records {
        let Some(row) = slice.position_of(record) else {
            continue;
        };
        let column = match segment {
            Some(segment) => match segment.position_of(record) {
                Some(column) => column,
                None => continue,
            },
            None => 0,
        };
        table[row][column] += match &measure.field {
            Some(field) => record
                .numbers
                .iter()
                .find(|(key, _)| key == field)
                .map_or(0., |(_, value)| *value),
            None => 1.,
        };
    }
    table
}

/// The top of the axis: a round number at or above `peak` that splits into
/// four neat steps.
fn axis_top(peak: f64) -> f64 {
    if peak <= 0. {
        return 4.;
    }
    let magnitude = 10f64.powf(peak.log10().floor());
    [1., 2., 4., 6., 8., 10.]
        .into_iter()
        .map(|step| step * magnitude)
        .find(|top| *top >= peak)
        .unwrap_or(10. * magnitude)
}

/// An amount with grouped thousands and at most one decimal.
fn amount_words(value: f64) -> String {
    let tenths = (value.abs() * 10.).round() as u64;
    let digits = (tenths / 10).to_string();
    let mut words = String::new();
    if value < 0. && tenths > 0 {
        words.push('-');
    }
    for (n, digit) in digits.chars().enumerate() {
        if n > 0 && (digits.len() - n).is_multiple_of(3) {
            words.push(',');
        }
        words.push(digit);
    }
    if !tenths.is_multiple_of(10) {
        words.push('.');
        words.push_str(&(tenths % 10).to_string());
    }
    words
}

/// An axis label in a few characters: "800", "2.5k", "40k", "1.2M".
fn axis_words(value: f64) -> String {
    let (scaled, suffix) = if value >= 1e6 {
        (value / 1e6, "M")
    } else if value >= 1e3 {
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

/// The columns from biggest to smallest, leaving out empty and hidden ones.
fn ranked_columns(row: &[f64], hidden: &[usize]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..row.len())
        .filter(|column| row[*column] > 0. && !hidden.contains(column))
        .collect();
    order.sort_by(|a, b| row[*b].total_cmp(&row[*a]));
    order
}

struct ChartState {
    measure: usize,
    slice: usize,
    segment: Option<usize>,
    /// Segment values switched off in the legend.
    hidden: Vec<usize>,
    /// The bar the pointer is on.
    hovered: Option<usize>,
}

type Pick = Rc<dyn Fn(usize, &mut App)>;

impl RenderOnce for StackedBreakdownChart {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let surface = theme.popover;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.info;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;
        let palette = [
            theme.chart_1,
            theme.chart_2,
            theme.chart_3,
            theme.chart_4,
            theme.chart_5,
        ];

        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| ChartState {
            measure: self.default_measure,
            slice: self.default_slice,
            segment: self.default_segment,
            hidden: Vec::new(),
            hovered: None,
        });
        let (measure_ix, slice_ix, segment_ix, hidden, hovered) = {
            let state = state.read(cx);
            (
                state.measure,
                state.slice,
                state.segment,
                state.hidden.clone(),
                state.hovered,
            )
        };

        let counted = [BreakdownMeasure::count("Count", "records")];
        let measures: &[BreakdownMeasure] = if self.measures.is_empty() {
            &counted
        } else {
            &self.measures
        };
        let measure_ix = measure_ix.min(measures.len() - 1);
        let measure = &measures[measure_ix];
        let card = v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(560.))
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .line_height(relative(1.45))
            .shadow(resting_shadow(cx));
        if self.dimensions.is_empty() {
            return card
                .p_4()
                .gap_1()
                .child(div().font_medium().child(self.title))
                .child(
                    div()
                        .text_color(quiet)
                        .child("Nothing to group the records by"),
                )
                .refine_style(&self.style);
        }
        let slice_ix = slice_ix.min(self.dimensions.len() - 1);
        let slice = &self.dimensions[slice_ix];
        // A dimension cannot both run along the bars and stack them.
        let segment_ix = segment_ix.filter(|ix| *ix != slice_ix && *ix < self.dimensions.len());
        let segment = segment_ix.map(|ix| &self.dimensions[ix]);
        let hidden: Vec<usize> = if segment.is_some() {
            hidden
        } else {
            Vec::new()
        };

        let table = tally(&self.records, slice, segment, measure);
        let visible = |row: &[f64]| -> f64 {
            row.iter()
                .enumerate()
                .filter(|(column, _)| !hidden.contains(column))
                .map(|(_, value)| *value)
                .sum()
        };
        let totals: Vec<f64> = table.iter().map(|row| visible(row)).collect();
        let grand: f64 = totals.iter().sum();
        let top = axis_top(totals.iter().copied().fold(0., f64::max));
        let hovered = hovered.filter(|ix| *ix < table.len());
        let colors: Vec<Hsla> = match segment {
            Some(segment) => segment
                .values
                .iter()
                .enumerate()
                .map(|(ix, (_, color))| color.unwrap_or(palette[ix % palette.len()]))
                .collect(),
            None => vec![accent],
        };

        let set = |change: fn(&mut ChartState, usize)| -> Pick {
            let state = state.clone();
            Rc::new(move |ix, cx| {
                state.update(cx, |state, cx| {
                    change(state, ix);
                    state.hovered = None;
                    cx.notify();
                })
            })
        };
        let picker = |name: &'static str,
                      label: &'static str,
                      options: Vec<SharedString>,
                      current: usize,
                      pick: Pick| {
            let shown = options.get(current).cloned().unwrap_or_default();
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(div().text_xs().text_color(quiet).child(label))
                .child(
                    Button::new((self.id.clone(), name))
                        .outline()
                        .small()
                        .child(
                            h_flex()
                                .w_full()
                                .gap_2()
                                .child(div().flex_1().min_w_0().truncate().child(shown))
                                .child(Icon::new(IconName::ChevronDown).xsmall().text_color(quiet)),
                        )
                        .dropdown_menu(move |mut menu, _, _| {
                            for (ix, option) in options.iter().enumerate() {
                                let pick = pick.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(option.clone())
                                        .checked(ix == current)
                                        .on_click(move |_, _, cx| pick(ix, cx)),
                                );
                            }
                            menu
                        }),
                )
        };
        let dimension_names: Vec<SharedString> = self
            .dimensions
            .iter()
            .map(|dimension| dimension.label.clone())
            .collect();
        // The last choice of the stacking picker is plain bars.
        let mut stack_names = dimension_names.clone();
        stack_names.push("Nothing".into());
        let plain = self.dimensions.len();
        let pickers = h_flex()
            .gap_2()
            .child(picker(
                "measure",
                "Measure",
                measures
                    .iter()
                    .map(|measure| measure.label.clone())
                    .collect(),
                measure_ix,
                set(|state, ix| state.measure = ix),
            ))
            .child(picker(
                "slice",
                "Along the bars",
                dimension_names,
                slice_ix,
                set(|state, ix| {
                    // Picking the stacking dimension swaps the two.
                    if state.segment == Some(ix) {
                        state.segment = Some(state.slice);
                    }
                    state.slice = ix;
                    state.hidden.clear();
                }),
            ))
            .child(picker(
                "segment",
                "Stacked by",
                stack_names,
                segment_ix.unwrap_or(plain),
                Rc::new({
                    let state = state.clone();
                    move |ix, cx| {
                        state.update(cx, |state, cx| {
                            if ix != state.slice {
                                state.segment = (ix < plain).then_some(ix);
                                state.hidden.clear();
                                state.hovered = None;
                                cx.notify();
                            }
                        })
                    }
                }),
            ));

        let hover = |ix: usize| {
            let state = state.clone();
            move |hovered: &bool, _: &mut Window, cx: &mut App| {
                let next = hovered.then_some(ix);
                state.update(cx, |state, cx| {
                    if *hovered || state.hovered == Some(ix) {
                        state.hovered = next;
                        cx.notify();
                    }
                })
            }
        };
        let swatch = |color: Hsla| div().flex_none().size_2().rounded(px(2.)).bg(color);

        let bars =
            h_flex()
                .absolute()
                .size_full()
                .items_end()
                .children(table.iter().enumerate().map(|(ix, row)| {
                    let dimmed = hovered.is_some_and(|hovered| hovered != ix);
                    div()
                        .id(("bar", ix))
                        .flex()
                        .flex_1()
                        .h_full()
                        .items_end()
                        .justify_center()
                        .when(hovered == Some(ix), |this| this.bg(chip.opacity(0.6)))
                        .on_hover(hover(ix))
                        .child(
                            // The first segment value sits at the bottom.
                            v_flex()
                                .flex_col_reverse()
                                .w(px(14.))
                                .rounded(px(2.))
                                .overflow_hidden()
                                .children(row.iter().enumerate().filter_map(|(column, value)| {
                                    if hidden.contains(&column) || *value <= 0. {
                                        return None;
                                    }
                                    let height = (value / top) as f32 * PLOT_HEIGHT;
                                    Some(
                                        div().w_full().h(px(height)).bg(
                                            colors[column].opacity(if dimmed { 0.35 } else { 1. })
                                        ),
                                    )
                                })),
                        )
                }));
        let readout = hovered.map(|ix| {
            let row = &table[ix];
            // The panel sits beside the bar, on the side with more room.
            let before = ix * 2 >= table.len();
            let bars = table.len() as f32;
            v_flex()
                .absolute()
                .top_0()
                .map(|this| {
                    if before {
                        this.right(relative((table.len() - ix) as f32 / bars))
                    } else {
                        this.left(relative((ix + 1) as f32 / bars))
                    }
                })
                .min_w(px(148.))
                .gap_1()
                .p_2()
                .rounded(radius)
                .border_1()
                .border_color(line)
                .bg(surface)
                .shadow(lifted_shadow(cx))
                .text_xs()
                .child(
                    h_flex()
                        .gap_3()
                        .font_medium()
                        .child(div().flex_1().child(slice.values[ix].0.clone()))
                        .child(amount_words(totals[ix])),
                )
                .when_some(segment, |this, segment| {
                    this.children(ranked_columns(row, &hidden).into_iter().map(|column| {
                        h_flex()
                            .gap_2()
                            .child(swatch(colors[column]))
                            .child(
                                div()
                                    .flex_1()
                                    .text_color(quiet)
                                    .child(segment.values[column].0.clone()),
                            )
                            .child(div().font_medium().child(amount_words(row[column])))
                    }))
                })
        });
        let plot = h_flex()
            .gap_2()
            .child(
                div()
                    .relative()
                    .flex_1()
                    .h(px(PLOT_HEIGHT))
                    .children((0..=4).map(|step| {
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .top(px((PLOT_HEIGHT - 1.) * (1. - step as f32 / 4.)))
                            .h(px(1.))
                            .bg(line)
                    }))
                    .child(bars)
                    .children(readout),
            )
            .child(
                v_flex()
                    .flex_none()
                    .flex_col_reverse()
                    .w(px(36.))
                    .h(px(PLOT_HEIGHT))
                    .justify_between()
                    .text_xs()
                    .text_color(quiet)
                    .children((0..=4).map(|step| {
                        div()
                            .line_height(relative(1.))
                            .whitespace_nowrap()
                            .child(axis_words(top * f64::from(step) / 4.))
                    })),
            );
        let names = h_flex()
            .pr(px(44.))
            .children(slice.values.iter().enumerate().map(|(ix, (name, _))| {
                div()
                    .flex_1()
                    .min_w_0()
                    .px_0p5()
                    .truncate()
                    .text_center()
                    .text_xs()
                    .when(hovered != Some(ix), |this| this.text_color(quiet))
                    .child(name.clone())
            }));

        let number = |words: String| {
            div()
                .flex_none()
                .w(px(NUMBER_COLUMN))
                .text_right()
                .child(words)
        };
        let heading = h_flex()
            .gap_2()
            .h(px(32.))
            .px_4()
            .border_t_1()
            .border_color(line)
            .text_xs()
            .text_color(quiet)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(slice.label.clone()),
            )
            .child(
                div()
                    .flex_none()
                    .w(px(NUMBER_COLUMN))
                    .truncate()
                    .text_right()
                    .child(measure.label.clone()),
            )
            .when_some(segment, |this, segment| {
                this.children(
                    segment
                        .values
                        .iter()
                        .enumerate()
                        .map(|(column, (name, _))| {
                            h_flex()
                                .flex_none()
                                .w(px(NUMBER_COLUMN))
                                .justify_end()
                                .gap_1()
                                .when(hidden.contains(&column), |this| this.opacity(0.4))
                                .child(swatch(colors[column]))
                                .child(div().min_w_0().truncate().child(name.clone()))
                        }),
                )
            });
        let rows = table.iter().enumerate().map(|(ix, row)| {
            h_flex()
                .id(("row", ix))
                .gap_2()
                .h(px(34.))
                .px_4()
                .border_t_1()
                .border_color(line)
                .when(hovered == Some(ix), |this| this.bg(chip))
                .on_hover(hover(ix))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(slice.values[ix].0.clone()),
                )
                .child(number(amount_words(totals[ix])).font_medium())
                .when(segment.is_some(), |this| {
                    this.children(row.iter().enumerate().map(|(column, value)| {
                        number(amount_words(*value))
                            .text_color(quiet)
                            .when(hidden.contains(&column), |this| this.opacity(0.4))
                    }))
                })
        });
        let rows: Vec<_> = rows.collect();

        // Each legend entry switches its segment value off and on.
        let legend = segment.map(|segment| {
            h_flex()
                .flex_wrap()
                .gap_1()
                .px_3()
                .py_2()
                .border_t_1()
                .border_color(line)
                .children(
                    segment
                        .values
                        .iter()
                        .enumerate()
                        .map(|(column, (name, _))| {
                            let state = state.clone();
                            let is_hidden = hidden.contains(&column);
                            h_flex()
                                .id(("legend", column))
                                .gap_1p5()
                                .h(px(26.))
                                .px_1p5()
                                .rounded(radius)
                                .cursor_pointer()
                                .hover(|this| this.bg(chip))
                                .when(is_hidden, |this| this.opacity(0.45).line_through())
                                .on_click(move |_, _, cx| {
                                    state.update(cx, |state, cx| {
                                        match state.hidden.iter().position(|other| *other == column)
                                        {
                                            Some(at) => {
                                                state.hidden.remove(at);
                                            }
                                            None => state.hidden.push(column),
                                        }
                                        cx.notify();
                                    })
                                })
                                .child(swatch(colors[column]))
                                .child(name.clone())
                        }),
                )
        });

        card.child(
            v_flex()
                .gap_3()
                .p_4()
                .child(
                    h_flex()
                        .gap_3()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .font_medium()
                                .child(self.title),
                        )
                        .child(
                            h_flex()
                                .flex_none()
                                .gap_1()
                                .child(div().font_semibold().child(amount_words(grand)))
                                .child(div().text_color(quiet).child(measure.unit.clone())),
                        ),
                )
                .child(pickers)
                .child(v_flex().gap_1p5().child(plot).child(names)),
        )
        .child(heading)
        .children(rows)
        .children(legend)
        .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn records() -> Vec<BreakdownRecord> {
        vec![
            BreakdownRecord::new()
                .field("state", "Open")
                .field("size", "Small")
                .number("hours", 2.),
            BreakdownRecord::new()
                .field("state", "Open")
                .field("size", "Large")
                .number("hours", 8.),
            BreakdownRecord::new()
                .field("state", "Closed")
                .field("size", "Small")
                .number("hours", 1.5),
            // Names a value the dimension does not list.
            BreakdownRecord::new()
                .field("state", "Lost")
                .field("size", "Small"),
        ]
    }

    fn state() -> BreakdownDimension {
        BreakdownDimension::new("state", "State")
            .value("Open")
            .value("Closed")
    }

    fn size() -> BreakdownDimension {
        BreakdownDimension::new("size", "Size")
            .value("Small")
            .value("Large")
    }

    #[test]
    fn records_are_counted_per_slice_and_segment() {
        let count = BreakdownMeasure::count("Count", "records");
        assert_eq!(
            tally(&records(), &state(), Some(&size()), &count),
            [[1., 1.], [1., 0.]]
        );
        assert_eq!(tally(&records(), &state(), None, &count), [[2.], [1.]]);
    }

    #[test]
    fn a_measure_adds_up_its_field() {
        let hours = BreakdownMeasure::sum("Hours", "h", "hours");
        assert_eq!(
            tally(&records(), &size(), Some(&state()), &hours),
            [[2., 1.5], [8., 0.]]
        );
    }

    #[test]
    fn the_axis_splits_into_four_neat_steps() {
        assert_eq!(axis_top(96.), 100.);
        assert_eq!(axis_top(36.), 40.);
        assert_eq!(axis_top(7.), 8.);
        assert_eq!(axis_top(4.5), 6.);
        assert_eq!(axis_top(0.), 4.);
    }

    #[test]
    fn amounts_group_thousands_and_keep_one_decimal() {
        assert_eq!(amount_words(212.), "212");
        assert_eq!(amount_words(12_400.), "12,400");
        assert_eq!(amount_words(1.5), "1.5");
    }

    #[test]
    fn axis_labels_stay_short() {
        assert_eq!(axis_words(80.), "80");
        assert_eq!(axis_words(2500.), "2.5k");
        assert_eq!(axis_words(40_000.), "40k");
        assert_eq!(axis_words(1_200_000.), "1.2M");
    }

    #[test]
    fn the_readout_ranks_what_is_shown() {
        assert_eq!(ranked_columns(&[1., 9., 0., 12.], &[]), [3, 1, 0]);
        assert_eq!(ranked_columns(&[1., 9., 0., 12.], &[3]), [1, 0]);
    }
}
