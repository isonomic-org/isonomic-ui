// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Stat grid: a few headline figures side by side, each with how it moved.
//!
//! A card carries a label, a figure that rolls when it changes, a small trend
//! line with "now" marked, the change against the last period as a toned
//! pill, a hint, and optional progress toward a goal. Whether a rise is good
//! is up to each stat, so falling latency reads as good and falling revenue
//! as bad.

use std::{rc::Rc, time::Duration};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::{Presence, Transition};
use isonomic_ui::component::{
    ActiveTheme as _, Icon, Sizable as _, StyledExt as _, TextRole as _, h_flex,
    progress::Progress, rolling_number::RollingNumber, tag::Tag, v_flex,
};
use isonomic_ui::{
    Animation, AnimationExt as _, AnyElement, App, Bounds, ElementId, Hsla,
    InteractiveElement as _, IntoElement, ParentElement as _, PathBuilder, Pixels, RenderOnce,
    Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, canvas,
    div, point, prelude::FluentBuilder as _, px,
};

use crate::common::{panel, shell, shell_header};

const SPARK_W: f32 = 72.;
const SPARK_H: f32 = 28.;
/// How long a card takes to appear, and the delay between neighbours.
const APPEAR: Duration = Duration::from_millis(240);
const STAGGER: Duration = Duration::from_millis(50);
/// How long a trend line takes to draw itself.
const DRAW: Duration = Duration::from_millis(700);
/// A change smaller than this many percent reads as flat.
const FLAT_BELOW: f32 = 0.05;

/// How a change reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Movement {
    /// No change worth the name.
    Flat,
    /// Moved the good way.
    Better,
    /// Moved the bad way.
    Worse,
}

/// Read a change in percent, given whether a rise is good.
pub fn movement(delta: Option<f32>, higher_is_better: bool) -> Movement {
    match delta {
        Some(delta) if delta.abs() >= FLAT_BELOW => {
            if (delta > 0.) == higher_is_better {
                Movement::Better
            } else {
                Movement::Worse
            }
        }
        _ => Movement::Flat,
    }
}

/// A change as the pill shows it: "+4.2%", "-1.0%", "0.0%".
pub fn delta_label(delta: f32) -> String {
    if delta.abs() < FLAT_BELOW {
        "0.0%".to_string()
    } else {
        format!("{delta:+.1}%")
    }
}

/// Where a trend's points fall in a `width` by `height` box, oldest first,
/// with the lowest value near the bottom and the highest near the top.
pub fn trend_points(values: &[f32], width: f32, height: f32) -> Vec<(f32, f32)> {
    if values.len() < 2 {
        return Vec::new();
    }
    let low = values.iter().copied().fold(f32::MAX, f32::min);
    let high = values.iter().copied().fold(f32::MIN, f32::max);
    let span = if high > low { high - low } else { 1. };
    let last = (values.len() - 1) as f32;
    values
        .iter()
        .enumerate()
        .map(|(ix, value)| {
            (
                ix as f32 / last * (width - 4.) + 2.,
                height - 3. - (value - low) / span * (height - 6.),
            )
        })
        .collect()
}

/// Progress toward a goal.
#[derive(Clone)]
struct Goal {
    share: f32,
    label: SharedString,
}

/// One headline figure.
#[derive(Clone)]
pub struct Stat {
    id: SharedString,
    label: SharedString,
    value: SharedString,
    delta: Option<f32>,
    higher_is_better: bool,
    trend: Vec<f32>,
    goal: Option<Goal>,
    hint: Option<SharedString>,
}

impl Stat {
    /// A figure, already written out ("48.2k", "1,204", "312 ms").
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        value: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            value: value.into(),
            delta: None,
            higher_is_better: true,
            trend: Vec::new(),
            goal: None,
            hint: None,
        }
    }

    /// The change against the last period, in percent.
    pub fn delta(mut self, delta: f32) -> Self {
        self.delta = Some(delta);
        self
    }

    /// A rise is bad for this one: waiting time, returns, cost.
    pub fn lower_is_better(mut self) -> Self {
        self.higher_is_better = false;
        self
    }

    /// Recent values, oldest first, for the trend line.
    pub fn trend(mut self, values: impl IntoIterator<Item = f32>) -> Self {
        self.trend = values.into_iter().collect();
        self
    }

    /// Progress toward a goal, as a share from 0 up, with its label.
    pub fn goal(mut self, share: f32, label: impl Into<SharedString>) -> Self {
        self.goal = Some(Goal {
            share: share.max(0.),
            label: label.into(),
        });
        self
    }

    /// A short note beside the change, such as "vs last week".
    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// What a screen reader hears for the whole card.
    pub fn spoken(&self) -> String {
        let mut spoken = format!("{}: {}", self.label, self.value);
        if let Some(delta) = self.delta {
            spoken.push_str(&match movement(Some(delta), self.higher_is_better) {
                Movement::Flat => ", flat".to_string(),
                Movement::Better => format!(", {}, better", delta_label(delta)),
                Movement::Worse => format!(", {}, worse", delta_label(delta)),
            });
        }
        if let Some(hint) = &self.hint {
            spoken.push_str(&format!(" {hint}"));
        }
        if let Some(goal) = &self.goal {
            spoken.push_str(&format!(
                ", {}% of {}",
                (goal.share * 100.).round(),
                goal.label
            ));
        }
        spoken
    }
}

fn paint_trend(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    points: &[(f32, f32)],
    drawn: f32,
    color: Hsla,
) {
    let Some(&(first_x, _)) = points.first() else {
        return;
    };
    let at = |(x, y): (f32, f32)| point(bounds.left() + px(x), bounds.top() + px(y));
    // The line draws itself from the oldest point: `drawn` runs 0 to 1.
    let reach = drawn * (points.len() - 1) as f32;
    let whole = reach.floor() as usize;
    let mut shown: Vec<(f32, f32)> = points.iter().copied().take(whole + 1).collect();
    if whole + 1 < points.len() {
        let (from, to) = (points[whole], points[whole + 1]);
        let part = reach - whole as f32;
        shown.push((
            from.0 + (to.0 - from.0) * part,
            from.1 + (to.1 - from.1) * part,
        ));
    }
    let Some(&(last_x, last_y)) = shown.last() else {
        return;
    };

    let mut area = PathBuilder::fill();
    area.move_to(at((first_x, SPARK_H)));
    for &spot in &shown {
        area.line_to(at(spot));
    }
    area.line_to(at((last_x, SPARK_H)));
    area.close();
    if let Ok(area) = area.build() {
        window.paint_path(area, color.opacity(0.1));
    }

    if shown.len() > 1 {
        let mut line = PathBuilder::stroke(px(1.75));
        line.move_to(at(shown[0]));
        for &spot in &shown[1..] {
            line.line_to(at(spot));
        }
        if let Ok(line) = line.build() {
            window.paint_path(line, color);
        }
    }

    // "Now" is marked with a dot.
    let mut dot = PathBuilder::fill();
    let ring: Vec<_> = (0..16)
        .map(|step| {
            let angle = step as f32 / 16. * std::f32::consts::TAU;
            at((last_x + 2.5 * angle.cos(), last_y + 2.5 * angle.sin()))
        })
        .collect();
    dot.add_polygon(&ring, true);
    if let Ok(dot) = dot.build() {
        window.paint_path(dot, color);
    }
}

type SelectHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// Headline figures side by side.
#[derive(IntoElement)]
pub struct StatGrid {
    id: ElementId,
    style: StyleRefinement,
    stats: Vec<Stat>,
    title: Option<SharedString>,
    subtitle: Option<SharedString>,
    columns: u16,
    trends: bool,
    on_select: Option<SelectHandler>,
}

impl StatGrid {
    /// Create a grid in two columns.
    pub fn new(id: impl Into<ElementId>, stats: Vec<Stat>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            stats,
            title: None,
            subtitle: None,
            columns: 2,
            trends: true,
            on_select: None,
        }
    }

    /// A title above the cards.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// A quieter line under the title.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// How many columns, from one to four. Use one below about 320px.
    pub fn columns(mut self, columns: u16) -> Self {
        self.columns = columns.clamp(1, 4);
        self
    }

    /// Leave the trend lines out.
    pub fn without_trends(mut self) -> Self {
        self.trends = false;
        self
    }

    /// Make each card a button that reports its stat's id.
    pub fn on_select(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Styled for StatGrid {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for StatGrid {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let part = |name: String| ElementId::NamedChild(self.id.clone().into(), name.into());
        let still = cx.reduce_motion();
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let good = theme.success;
        let bad = theme.danger;
        let data = theme.chart_1;
        let ring = theme.ring;
        let count = self.stats.len();
        let total = APPEAR + STAGGER * count.saturating_sub(1) as u32;

        let mut cards: Vec<AnyElement> = Vec::with_capacity(count);
        for (ix, stat) in self.stats.iter().enumerate() {
            let moved = movement(stat.delta, stat.higher_is_better);
            let tone = match moved {
                Movement::Flat => quiet,
                Movement::Better => good,
                Movement::Worse => bad,
            };
            let points = trend_points(&stat.trend, SPARK_W, SPARK_H);
            let drawn = Presence::new(part(format!("draw-{}", stat.id)), true)
                .transition(Transition::new(DRAW))
                .sample(window, cx)
                .progress;
            let trend = (self.trends && !points.is_empty()).then(|| {
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| paint_trend(window, bounds, &points, drawn, tone),
                )
                .w(px(SPARK_W))
                .h(px(SPARK_H))
                .flex_none()
            });
            let pill = stat.delta.map(|delta| {
                let icon = match moved {
                    Movement::Flat => CatalogIcon::Subtract,
                    _ if delta > 0. => CatalogIcon::ArrowUpRight,
                    _ => CatalogIcon::ArrowDownRight,
                };
                match moved {
                    Movement::Flat => Tag::secondary(),
                    Movement::Better => Tag::success(),
                    Movement::Worse => Tag::danger(),
                }
                .small()
                .rounded_full()
                .gap_1()
                .child(Icon::new(icon).size(px(12.)))
                .child(
                    RollingNumber::new(part(format!("delta-{}", stat.id)), delta_label(delta))
                        .line_height(px(16.)),
                )
            });
            let goal = stat.goal.clone().map(|goal| {
                v_flex()
                    .mt_1()
                    .gap(px(6.))
                    .child(
                        Progress::new(part(format!("goal-{}", stat.id)))
                            .small()
                            .striped(true)
                            .value((goal.share * 100.).min(100.))
                            .color(if goal.share >= 1. { good } else { data })
                            .accessibility_label(format!("{}: {}", stat.label, goal.label)),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .text_caption()
                            .text_color(quiet)
                            .child(goal.label.clone())
                            .child(format!("{}%", (goal.share * 100.).round())),
                    )
            });

            let card = panel(cx)
                .id(part(format!("card-{}", stat.id)))
                .h_full()
                .v_flex()
                .gap_2()
                .p_4()
                .role(if self.on_select.is_some() {
                    Role::Button
                } else {
                    Role::Group
                })
                .aria_label(stat.spoken())
                .child(
                    div()
                        .text_body()
                        .text_color(quiet)
                        .child(stat.label.clone()),
                )
                .child(
                    h_flex()
                        .items_end()
                        .justify_between()
                        .gap_3()
                        .child(
                            RollingNumber::new(
                                part(format!("value-{}", stat.id)),
                                stat.value.clone(),
                            )
                            .text_size(px(26.))
                            .line_height(px(26.))
                            .font_medium(),
                        )
                        .children(trend),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .text_caption()
                        .text_color(quiet)
                        .children(pill)
                        .when_some(stat.hint.clone(), |this, hint| {
                            this.child(div().min_w_0().truncate().child(hint))
                        }),
                )
                .children(goal)
                .when_some(self.on_select.clone(), |this, pick| {
                    let id = stat.id.clone();
                    this.cursor_pointer()
                        .tab_index(0)
                        .focus_visible(move |style| style.border_2().border_color(ring))
                        .on_click(move |_, window, cx| pick(&id, window, cx))
                });

            let slot = div().relative().h_full().child(card);
            cards.push(if still {
                slot.into_any_element()
            } else {
                slot.with_animation(
                    part(format!("appear-{}", stat.id)),
                    Animation::new(total),
                    move |this, time| {
                        let start = STAGGER.as_secs_f32() * ix as f32;
                        let linear = ((time * total.as_secs_f32() - start) / APPEAR.as_secs_f32())
                            .clamp(0., 1.);
                        let shown = 1. - (1. - linear).powi(3);
                        this.opacity(shown).top(px(6. * (1. - shown)))
                    },
                )
                .into_any_element()
            });
        }

        shell(cx)
            .id(self.id.clone())
            .max_w(px(672.))
            .when_some(self.title.clone(), |this, title| {
                this.child(shell_header(
                    None,
                    title,
                    self.subtitle.clone().map(|line| line.into_any_element()),
                    None,
                    cx,
                ))
            })
            .child(
                div()
                    .grid()
                    .grid_cols(self.columns)
                    .gap(px(6.))
                    .children(cards),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_reads_by_which_way_is_good() {
        assert_eq!(movement(None, true), Movement::Flat);
        assert_eq!(movement(Some(0.04), true), Movement::Flat);
        assert_eq!(movement(Some(4.2), true), Movement::Better);
        assert_eq!(movement(Some(-4.2), true), Movement::Worse);
        assert_eq!(movement(Some(4.2), false), Movement::Worse);
        assert_eq!(movement(Some(-4.2), false), Movement::Better);
    }

    #[test]
    fn a_change_is_written_with_its_sign() {
        assert_eq!(delta_label(4.24), "+4.2%");
        assert_eq!(delta_label(-1.), "-1.0%");
        assert_eq!(delta_label(0.01), "0.0%");
    }

    #[test]
    fn a_trend_spans_the_box_with_room_for_the_dot() {
        let points = trend_points(&[10., 30., 20.], 72., 28.);
        assert_eq!(points.len(), 3);
        assert_eq!(points[0], (2., 25.));
        assert_eq!(points[1], (36., 3.));
        assert_eq!(points[2].0, 70.);
        assert_eq!(points[2].1, 14.);
        // One value is no trend; equal values sit on the bottom line.
        assert!(trend_points(&[5.], 72., 28.).is_empty());
        assert_eq!(trend_points(&[5., 5.], 72., 28.)[1], (70., 25.));
    }

    #[test]
    fn a_card_is_read_as_one_line() {
        let stat = Stat::new("wait", "Wait at the till", "3.4 min")
            .delta(-12.)
            .lower_is_better()
            .hint("vs last week")
            .goal(0.8, "Goal 3 min");
        assert_eq!(
            stat.spoken(),
            "Wait at the till: 3.4 min, -12.0%, better vs last week, 80% of Goal 3 min"
        );
        assert_eq!(
            Stat::new("a", "Visits", "120").delta(0.).spoken(),
            "Visits: 120, flat"
        );
    }
}
