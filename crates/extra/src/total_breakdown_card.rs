//! Total breakdown card: one total and the parts it is made of.
//!
//! A small card for a dashboard: the total with its change, one bar split by
//! part, and the parts ranked by size with their share. Pointing at a part
//! puts its own figure in place of the total. The bar keeps the parts in the
//! order they were given, so a part keeps its place and colour from one
//! period to the next while the list re-ranks.

use std::rc::Rc;

use isonomic_ui::component::{ActiveTheme as _, Icon, IconName, StyledExt as _, h_flex, v_flex};
use isonomic_ui::{
    App, ElementId, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// One part of the total.
#[derive(Debug, Clone)]
pub struct BreakdownPart {
    label: SharedString,
    value: f64,
    color: Option<Hsla>,
}

impl BreakdownPart {
    /// Create a part.
    pub fn new(label: impl Into<SharedString>, value: f64) -> Self {
        Self {
            label: label.into(),
            value,
            color: None,
        }
    }

    /// Set the colour. Without one, parts get lighter tints of the accent and
    /// then grey, in the order they were given.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }
}

/// The last line of the card.
enum Footer {
    /// Name the biggest part.
    Biggest,
    Text(SharedString),
    Hidden,
}

type Format = Rc<dyn Fn(f64) -> String>;
type PeriodHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// A total, a bar split by part, and the parts ranked by size.
#[derive(IntoElement)]
pub struct TotalBreakdownCard {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    parts: Vec<BreakdownPart>,
    periods: Vec<(SharedString, SharedString)>,
    period: Option<SharedString>,
    change: Option<f32>,
    format: Format,
    total_label: SharedString,
    footer: Footer,
    on_period_change: Option<PeriodHandler>,
}

impl TotalBreakdownCard {
    /// Create a card with a heading and no parts.
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            parts: Vec::new(),
            periods: Vec::new(),
            period: None,
            change: None,
            format: Rc::new(grouped),
            total_label: "Total".into(),
            footer: Footer::Biggest,
            on_period_change: None,
        }
    }

    /// Add a part. Keep the same order across periods; up to seven read well.
    pub fn part(mut self, part: BreakdownPart) -> Self {
        self.parts.push(part);
        self
    }

    /// Offer a period to look at, by id and label. Pass the parts and the
    /// change of the chosen period along with `selected_period`.
    pub fn period(mut self, id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        self.periods.push((id.into(), label.into()));
        self
    }

    /// Mark the period the numbers belong to. The first one otherwise.
    pub fn selected_period(mut self, id: impl Into<SharedString>) -> Self {
        self.period = Some(id.into());
        self
    }

    /// Set the change against the period before, in percent: 12.4 or -3.
    pub fn change(mut self, percent: f32) -> Self {
        self.change = Some(percent);
        self
    }

    /// Set how the total and the parts are written. Whole numbers with
    /// grouped thousands by default.
    pub fn format(mut self, format: impl Fn(f64) -> String + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }

    /// Set the line under the total. It gives way to a part's name while that
    /// part is pointed at.
    pub fn total_label(mut self, label: impl Into<SharedString>) -> Self {
        self.total_label = label.into();
        self
    }

    /// Replace the last line, which names the biggest part by default.
    pub fn footer(mut self, footer: impl Into<SharedString>) -> Self {
        self.footer = Footer::Text(footer.into());
        self
    }

    /// Leave the last line out.
    pub fn without_footer(mut self) -> Self {
        self.footer = Footer::Hidden;
        self
    }

    /// Run when another period is picked, with its id.
    pub fn on_period_change(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_period_change = Some(Rc::new(handler));
        self
    }
}

impl Styled for TotalBreakdownCard {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// A whole number with grouped thousands: "57,600".
fn grouped(value: f64) -> String {
    let whole = value.abs().round() as u64;
    let digits = whole.to_string();
    let mut words = String::new();
    if value < 0. && whole > 0 {
        words.push('-');
    }
    for (n, digit) in digits.chars().enumerate() {
        if n > 0 && (digits.len() - n).is_multiple_of(3) {
            words.push(',');
        }
        words.push(digit);
    }
    words
}

/// The positions of the parts from biggest to smallest. Equal parts keep the
/// order they were given.
fn ranked(values: &[f64]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_by(|a, b| values[*b].total_cmp(&values[*a]));
    order
}

/// A part as a whole percentage of the total.
fn share(part: f64, total: f64) -> u32 {
    if total <= 0. {
        return 0;
    }
    (part / total * 100.).round().clamp(0., 100.) as u32
}

/// The change without its sign, with one decimal unless it is whole.
fn change_words(percent: f32) -> String {
    let tenths = (percent.abs() * 10.).round() as u32;
    if tenths.is_multiple_of(10) {
        format!("{}%", tenths / 10)
    } else {
        format!("{}.{}%", tenths / 10, tenths % 10)
    }
}

/// How strong the accent is for the part at `ix`. Later parts are grey.
fn part_strength(ix: usize) -> Option<f32> {
    [1., 0.72, 0.5, 0.32, 0.2].get(ix).copied()
}

struct CardState {
    /// The part the pointer is on, by the order the parts were given in.
    hovered: Option<usize>,
    period: Option<SharedString>,
}

impl RenderOnce for TotalBreakdownCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.info;
        let success = theme.success;
        let danger = theme.danger;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;

        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| CardState {
            hovered: None,
            period: None,
        });
        let (hovered, kept_period) = {
            let state = state.read(cx);
            (state.hovered, state.period.clone())
        };
        let hovered = hovered.filter(|ix| *ix < self.parts.len());
        let period = self
            .period
            .clone()
            .or(kept_period)
            .or_else(|| self.periods.first().map(|(id, _)| id.clone()));

        let values: Vec<f64> = self.parts.iter().map(|part| part.value.max(0.)).collect();
        let total: f64 = values.iter().sum();
        let order = ranked(&values);
        let colors: Vec<Hsla> = self
            .parts
            .iter()
            .enumerate()
            .map(|(ix, part)| {
                part.color.unwrap_or_else(|| match part_strength(ix) {
                    Some(strength) => accent.opacity(strength),
                    None => quiet.opacity(0.4),
                })
            })
            .collect();
        let format = self.format.clone();
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

        let switch = (!self.periods.is_empty()).then(|| {
            h_flex()
                .flex_none()
                .gap_0p5()
                .children(self.periods.iter().enumerate().map(|(ix, (id, label))| {
                    let id = id.clone();
                    let is_selected = period.as_ref() == Some(&id);
                    div()
                        .id(("period", ix))
                        .flex()
                        .items_center()
                        .h(px(24.))
                        .px_2()
                        .rounded(radius)
                        .text_xs()
                        .cursor_pointer()
                        .map(|this| {
                            if is_selected {
                                this.bg(chip).font_medium()
                            } else {
                                this.text_color(quiet)
                            }
                        })
                        .on_click({
                            let state = state.clone();
                            let on_period_change = self.on_period_change.clone();
                            move |_, window, cx| {
                                if is_selected {
                                    return;
                                }
                                state.update(cx, |state, cx| {
                                    state.period = Some(id.clone());
                                    cx.notify();
                                });
                                if let Some(handler) = &on_period_change {
                                    handler(&id, window, cx);
                                }
                            }
                        })
                        .child(label.clone())
                }))
        });

        let change = self.change.map(|percent| {
            let (color, icon) = if percent > 0. {
                (success, Some(IconName::ArrowUp))
            } else if percent < 0. {
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
                .child(change_words(percent))
        });
        // The headline is the total, or the part being pointed at.
        let (figure, caption) = match hovered {
            Some(ix) => (format(values[ix]), self.parts[ix].label.clone()),
            None => (format(total), self.total_label.clone()),
        };

        let bar =
            h_flex()
                .w_full()
                .h(px(6.))
                .gap(px(2.))
                .children(self.parts.iter().enumerate().map(|(ix, _)| {
                    let width = if total > 0. {
                        (values[ix] / total) as f32
                    } else {
                        0.
                    };
                    let dimmed = hovered.is_some_and(|hovered| hovered != ix);
                    div()
                        .id(("part", ix))
                        .h_full()
                        .w(relative(width))
                        .rounded(px(2.))
                        .bg(colors[ix].opacity(if dimmed { 0.35 } else { 1. }))
                        .on_hover(hover(ix))
                }));

        let rows = order.iter().map(|&ix| {
            let part = &self.parts[ix];
            h_flex()
                .id(("row", ix))
                .gap_2()
                .h(px(30.))
                .px_1()
                .rounded(radius)
                .when(hovered == Some(ix), |this| this.bg(chip))
                .on_hover(hover(ix))
                .child(div().flex_none().size_2().rounded(px(2.)).bg(colors[ix]))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(part.label.clone()),
                )
                .child(div().flex_none().font_medium().child(format(values[ix])))
                .child(
                    div()
                        .flex_none()
                        .w(px(36.))
                        .text_right()
                        .text_color(quiet)
                        .child(format!("{}%", share(values[ix], total))),
                )
        });
        let rows: Vec<_> = rows.collect();

        let footer = match self.footer {
            Footer::Hidden => None,
            Footer::Text(text) => Some(div().child(text)),
            Footer::Biggest => order.first().filter(|_| total > 0.).map(|&ix| {
                h_flex().gap_1().child("Biggest share:").child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(ink)
                        .child(self.parts[ix].label.clone()),
                )
            }),
        };

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(360.))
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .line_height(relative(1.45))
            .shadow(resting_shadow(cx))
            .child(
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
                            .children(switch),
                    )
                    .child(
                        v_flex()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_2xl()
                                            .font_semibold()
                                            .line_height(relative(1.2))
                                            .child(figure),
                                    )
                                    .when(hovered.is_none(), |this| this.children(change)),
                            )
                            .child(div().truncate().text_xs().text_color(quiet).child(caption)),
                    )
                    .child(bar)
                    .child(v_flex().children(rows)),
            )
            .children(footer.map(|footer| {
                footer
                    .px_4()
                    .py_2p5()
                    .border_t_1()
                    .border_color(line)
                    .text_xs()
                    .text_color(quiet)
            }))
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_group_thousands() {
        assert_eq!(grouped(0.), "0");
        assert_eq!(grouped(999.4), "999");
        assert_eq!(grouped(57_600.), "57,600");
        assert_eq!(grouped(1_234_567.), "1,234,567");
        assert_eq!(grouped(-4200.), "-4,200");
    }

    #[test]
    fn parts_rank_by_size_and_ties_keep_their_order() {
        assert_eq!(ranked(&[10., 30., 20.]), [1, 2, 0]);
        assert_eq!(ranked(&[5., 5., 9.]), [2, 0, 1]);
        assert!(ranked(&[]).is_empty());
    }

    #[test]
    fn shares_round_and_survive_an_empty_total() {
        assert_eq!(share(18_400., 57_600.), 32);
        assert_eq!(share(1., 3.), 33);
        assert_eq!(share(4., 0.), 0);
    }

    #[test]
    fn the_change_drops_its_sign_and_a_whole_decimal() {
        assert_eq!(change_words(8.1), "8.1%");
        assert_eq!(change_words(-3.), "3%");
        assert_eq!(change_words(0.), "0%");
        assert_eq!(change_words(12.96), "13%");
    }

    #[test]
    fn later_parts_fade_and_then_turn_grey() {
        assert_eq!(part_strength(0), Some(1.));
        assert!(part_strength(4) < part_strength(3));
        assert_eq!(part_strength(5), None);
    }
}
