// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Tick gauge: a level drawn as ticks around an arc, with the gap to a target.
//!
//! Filled ticks are the value. With a target, a dashed marker crosses the arc
//! there and the ticks between the value and the target turn to the danger
//! colour, so a shortfall reads at a glance. The figure in the middle rolls
//! when it changes. Ticks fill in order when the gauge first appears.

use std::{f32::consts::PI, time::Duration};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::{Presence, Transition, transition};
use isonomic_ui::component::{
    ActiveTheme as _, Icon, StyledExt as _, TextRole as _, rolling_number::RollingNumber,
};
use isonomic_ui::{
    AnyElement, App, Bounds, ElementId, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, PathBuilder, Pixels, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, canvas, div, point,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::{panel, shell, shell_header};

/// The arc spans this many degrees and opens at the bottom.
const SWEEP: f32 = 220.;
/// The drawing is laid out on a 240 by 196 box and scaled to its width.
const BOX_W: f32 = 240.;
const BOX_H: f32 = 196.;
const CX: f32 = 120.;
const CY: f32 = 112.;
const OUTER: f32 = 96.;
const INNER: f32 = 78.;
const TICK_WIDTH: f32 = 3.5;
/// The widest the dial is drawn.
const MAX_WIDTH: f32 = 288.;
/// The delay between one tick filling and the next as the gauge appears.
const TICK_STAGGER: Duration = Duration::from_millis(12);
/// How long one tick takes to change colour.
const TICK_FADE: Duration = Duration::from_millis(240);

/// A point at `degrees` clockwise from the top, `radius` from the pivot, in
/// the 240 by 196 box.
pub fn polar(degrees: f32, radius: f32) -> (f32, f32) {
    let angle = (degrees - 90.) * PI / 180.;
    (CX + radius * angle.cos(), CY + radius * angle.sin())
}

/// The angle of a position along the arc, where 0 is its start and 1 its end.
pub fn arc_angle(along: f32) -> f32 {
    -SWEEP / 2. + SWEEP * along
}

/// What a tick shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickState {
    /// Within the value.
    Filled,
    /// Past the value but short of the target.
    Short,
    /// Past both.
    Empty,
}

/// The state of tick `ix` of `count` for a value and an optional target, both
/// between 0 and 1.
pub fn tick_state(ix: usize, count: usize, value: f32, target: Option<f32>) -> TickState {
    let along = if count > 1 {
        ix as f32 / (count - 1) as f32
    } else {
        0.
    };
    if along <= value + 1e-6 {
        TickState::Filled
    } else if target.is_some_and(|target| along <= target + 1e-6) {
        TickState::Short
    } else {
        TickState::Empty
    }
}

/// A share as a percent with one decimal: 0.931 is "93.1%".
pub fn percent_label(share: f32) -> String {
    format!("{:.1}%", share * 100.)
}

/// The outline of a line from `a` to `b` with round ends, `width` thick.
fn capsule(a: (f32, f32), b: (f32, f32), width: f32) -> Vec<(f32, f32)> {
    let half = width / 2.;
    let along = (b.1 - a.1).atan2(b.0 - a.0);
    let mut points = Vec::with_capacity(18);
    for (centre, turn) in [(b, -PI / 2.), (a, PI / 2.)] {
        for step in 0..=8 {
            let angle = along + turn + PI * step as f32 / 8.;
            points.push((centre.0 + half * angle.cos(), centre.1 + half * angle.sin()));
        }
    }
    points
}

fn paint_tick(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    scale: f32,
    a: (f32, f32),
    b: (f32, f32),
    color: Hsla,
) {
    let outline: Vec<_> = capsule(a, b, TICK_WIDTH)
        .into_iter()
        .map(|(x, y)| point(bounds.left() + px(x * scale), bounds.top() + px(y * scale)))
        .collect();
    let mut path = PathBuilder::fill();
    path.add_polygon(&outline, true);
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

/// A level as ticks around an arc.
#[derive(IntoElement)]
pub struct TickGauge {
    id: ElementId,
    style: StyleRefinement,
    value: f32,
    title: SharedString,
    subtitle: Option<SharedString>,
    target: Option<f32>,
    target_label: Option<SharedString>,
    caption: Option<SharedString>,
    figure: Option<SharedString>,
    ticks: usize,
    aside: Option<AnyElement>,
}

impl TickGauge {
    /// Create a gauge for a value between 0 and 1.
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>, value: f32) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            value: value.clamp(0., 1.),
            title: title.into(),
            subtitle: None,
            target: None,
            target_label: None,
            caption: None,
            figure: None,
            ticks: 44,
            aside: None,
        }
    }

    /// A quieter line under the title.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// The goal, between 0 and 1, with the label shown under the arc's end.
    pub fn target(mut self, target: f32, label: impl Into<SharedString>) -> Self {
        self.target = Some(target.clamp(0., 1.));
        self.target_label = Some(label.into());
        self
    }

    /// A line under the figure, such as "412 of 5,980 failed".
    pub fn caption(mut self, caption: impl Into<SharedString>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    /// Write the figure yourself. Without it, a percent with one decimal.
    pub fn figure(mut self, figure: impl Into<SharedString>) -> Self {
        self.figure = Some(figure.into());
        self
    }

    /// How many ticks the arc has. 44 by default.
    pub fn ticks(mut self, ticks: usize) -> Self {
        self.ticks = ticks.max(2);
        self
    }

    /// Something for the far end of the header, such as a status tag.
    pub fn aside(mut self, aside: impl IntoElement) -> Self {
        self.aside = Some(aside.into_any_element());
        self
    }

    fn spoken(&self, figure: &str) -> String {
        let mut spoken = format!("{}: {figure}", self.title);
        if let Some(caption) = &self.caption {
            spoken.push_str(&format!(", {caption}"));
        }
        if let Some(label) = &self.target_label {
            spoken.push_str(&format!(", target {label}"));
        }
        spoken
    }
}

impl Styled for TickGauge {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// A label centred under a point of the arc.
fn arc_label(along: f32, text: SharedString, color: Hsla) -> impl IntoElement {
    let (x, y) = polar(arc_angle(along), OUTER);
    div()
        .absolute()
        .left(relative(x / BOX_W))
        .top(relative((y + 10.) / BOX_H))
        .w(px(0.))
        .flex()
        .justify_center()
        .child(
            div()
                .flex_none()
                .whitespace_nowrap()
                .text_size(px(11.))
                .line_height(px(16.))
                .text_color(color)
                .child(text),
        )
}

impl RenderOnce for TickGauge {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let part = |name: &'static str| ElementId::NamedChild(self.id.clone().into(), name.into());
        let figure: SharedString = self
            .figure
            .clone()
            .unwrap_or_else(|| percent_label(self.value).into());
        let spoken = self.spoken(&figure);
        let theme = cx.theme();
        let fill = theme.chart_1;
        let short = theme.danger;
        let ink = theme.foreground;
        let track = ink.opacity(0.1);
        let quiet = theme.muted_foreground;
        let ticks = self.ticks;
        let target = self.target;

        // Ticks fill in order as the gauge appears; later the value sweeps.
        let appear_time = TICK_STAGGER * ticks as u32 + TICK_FADE;
        let appeared = Presence::new(part("appear"), true)
            .transition(Transition::new(appear_time).ease(|time| time))
            .sample(window, cx)
            .progress;
        let value = transition(
            part("value"),
            self.value,
            Transition::new(TICK_FADE),
            window,
            cx,
        );
        let elapsed = appeared * appear_time.as_secs_f32();
        let settled = appeared >= 1.;

        let dial = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let scale = f32::from(bounds.size.width) / BOX_W;
                for ix in 0..ticks {
                    let along = ix as f32 / (ticks - 1) as f32;
                    let angle = arc_angle(along);
                    // How far this tick is into its own colour change.
                    let lit = ((elapsed - TICK_STAGGER.as_secs_f32() * ix as f32)
                        / TICK_FADE.as_secs_f32())
                    .clamp(0., 1.);
                    let color = match tick_state(ix, ticks, value, target) {
                        TickState::Filled => track.blend(fill.opacity(lit)),
                        TickState::Short if settled => short,
                        _ => track,
                    };
                    paint_tick(
                        window,
                        bounds,
                        scale,
                        polar(angle, INNER),
                        polar(angle, OUTER),
                        color,
                    );
                }
                if let Some(target) = target {
                    let angle = arc_angle(target);
                    let (x1, y1) = polar(angle, INNER - 8.);
                    let (x2, y2) = polar(angle, OUTER + 6.);
                    let mut path = PathBuilder::stroke(px(1.5 * scale))
                        .dash_array(&[px(2. * scale), px(3. * scale)]);
                    path.move_to(point(
                        bounds.left() + px(x1 * scale),
                        bounds.top() + px(y1 * scale),
                    ));
                    path.line_to(point(
                        bounds.left() + px(x2 * scale),
                        bounds.top() + px(y2 * scale),
                    ));
                    if let Ok(path) = path.build() {
                        window.paint_path(path, ink);
                    }
                }
            },
        )
        .w_full()
        .aspect_ratio(BOX_W / BOX_H);

        // The figure is centred on the arc's pivot, not on the box.
        let has_caption = self.caption.is_some();
        let block_height = if has_caption { 56. } else { 36. };
        let middle = div()
            .absolute()
            .left_0()
            .right_0()
            .top(relative((CY - 14.) / BOX_H))
            .mt(px(-block_height / 2.))
            .v_flex()
            .items_center()
            .child(
                RollingNumber::new(part("figure"), figure)
                    .text_size(px(30.))
                    .line_height(px(36.))
                    .font_medium(),
            )
            .when_some(self.caption.clone(), |this, caption| {
                this.child(div().mt_1().text_caption().text_color(quiet).child(caption))
            });

        shell(cx)
            .id(self.id.clone())
            .max_w(px(384.))
            .child(shell_header(
                Some(Icon::new(CatalogIcon::Meter)),
                self.title.clone(),
                self.subtitle.clone().map(|line| line.into_any_element()),
                self.aside,
                cx,
            ))
            .child(
                panel(cx).px_4().pt_5().pb_4().child(
                    div()
                        .id(part("dial"))
                        .relative()
                        .mx_auto()
                        .w_full()
                        .max_w(px(MAX_WIDTH))
                        .role(Role::Meter)
                        .aria_label(spoken)
                        .aria_min_numeric_value(0.)
                        .aria_max_numeric_value(1.)
                        .aria_numeric_value(f64::from(self.value))
                        .child(dial)
                        .child(arc_label(0., "0%".into(), quiet))
                        .when_some(self.target_label.clone(), |this, label| {
                            this.child(arc_label(1., label, quiet))
                        })
                        .child(middle),
                ),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arc_opens_at_the_bottom_and_is_symmetric() {
        assert_eq!(arc_angle(0.), -110.);
        assert_eq!(arc_angle(0.5), 0.);
        assert_eq!(arc_angle(1.), 110.);
        // Straight up from the pivot at the middle of the arc.
        let (x, y) = polar(0., OUTER);
        assert!((x - CX).abs() < 0.001 && (y - (CY - OUTER)).abs() < 0.001);
        // The two ends mirror each other and sit below the pivot.
        let (left, left_y) = polar(arc_angle(0.), OUTER);
        let (right, right_y) = polar(arc_angle(1.), OUTER);
        assert!((left + right - 2. * CX).abs() < 0.001);
        assert!((left_y - right_y).abs() < 0.001 && left_y > CY);
    }

    #[test]
    fn ticks_fill_to_the_value_and_show_the_gap_to_the_target() {
        // Eleven ticks sit at 0, 0.1 ... 1.
        assert_eq!(tick_state(0, 11, 0., None), TickState::Filled);
        assert_eq!(tick_state(7, 11, 0.7, Some(0.9)), TickState::Filled);
        assert_eq!(tick_state(8, 11, 0.7, Some(0.9)), TickState::Short);
        assert_eq!(tick_state(9, 11, 0.7, Some(0.9)), TickState::Short);
        assert_eq!(tick_state(10, 11, 0.7, Some(0.9)), TickState::Empty);
        assert_eq!(tick_state(8, 11, 0.7, None), TickState::Empty);
        // A value past the target leaves nothing short.
        assert_eq!(tick_state(9, 11, 0.95, Some(0.9)), TickState::Filled);
    }

    #[test]
    fn the_figure_and_the_spoken_line() {
        assert_eq!(percent_label(0.931), "93.1%");
        assert_eq!(percent_label(1.), "100.0%");
        let gauge = TickGauge::new("g", "Orders sent on time", 0.931)
            .caption("412 of 5,980 late")
            .target(0.99, "Goal 99%");
        assert_eq!(
            gauge.spoken("93.1%"),
            "Orders sent on time: 93.1%, 412 of 5,980 late, target Goal 99%"
        );
        assert_eq!(TickGauge::new("g", "Level", 3.).value, 1.);
    }

    #[test]
    fn a_tick_outline_is_as_wide_as_its_stroke() {
        let outline = capsule((0., 0.), (10., 0.), 4.);
        let top = outline.iter().map(|p| p.1).fold(f32::MAX, f32::min);
        let bottom = outline.iter().map(|p| p.1).fold(f32::MIN, f32::max);
        let left = outline.iter().map(|p| p.0).fold(f32::MAX, f32::min);
        let right = outline.iter().map(|p| p.0).fold(f32::MIN, f32::max);
        assert!((bottom - top - 4.).abs() < 0.01);
        // Round ends reach half a width past each end.
        assert!((left + 2.).abs() < 0.01 && (right - 12.).abs() < 0.01);
    }
}
