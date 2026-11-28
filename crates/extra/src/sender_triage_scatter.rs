// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Sender triage scatter: email senders by how much they send and how much of
//! it gets read, with a zone that picks the ones to act on.
//!
//! Each dot is a sender: further right sends more, higher up gets opened
//! more. A drop zone sits over the plot; senders inside it are selected and
//! counted below, with buttons to act on them all at once. Drag the zone to
//! move it, drag a corner to resize it, or drag on empty space to draw a new
//! one. From the keyboard, the arrow keys move the zone and Shift with the
//! arrow keys resizes it; Escape puts it back.

use std::{cell::Cell, f32::consts::TAU, rc::Rc};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::TestSupportExt as _;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, Sizable as _, StyledExt as _, TextRole as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    tag::Tag,
};
use isonomic_ui::{
    App, Bounds, ElementId, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, ParentElement as _, PathBuilder, Pixels, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, canvas, div,
    point, prelude::FluentBuilder as _, px, relative,
};

use crate::common::{panel, shell, shell_header};

/// Height of the plot, the room left of it for the open-rate axis and the
/// room under it for the volume axis.
const PLOT_H: f32 = 248.;
const GUTTER: f32 = 36.;
const AXIS: f32 = 22.;
/// Radius of a sender's dot.
const DOT: f32 = 4.;
/// How near a corner the pointer must be, in pixels, to grab it.
const HANDLE: f32 = 9.;
/// How far one arrow key moves or resizes the zone, as a share of the plot.
pub const KEY_STEP: f32 = 0.05;
/// The smallest the zone gets, as a share of the plot.
pub const MIN_SIZE: f32 = 0.06;
/// How many selected senders are named before the rest are counted.
const NAMED: usize = 5;

/// One sender on the plot.
#[derive(Debug, Clone, PartialEq)]
pub struct Sender {
    /// Stable id, reported back with actions.
    pub id: SharedString,
    /// What the reader calls it.
    pub name: SharedString,
    /// Emails a month.
    pub volume: f64,
    /// Share of its emails opened, from 0 to 1.
    pub open_rate: f64,
}

impl Sender {
    /// A sender with its monthly volume and open rate (0 to 1).
    pub fn new(
        id: impl Into<SharedString>,
        name: impl Into<SharedString>,
        volume: f64,
        open_rate: f64,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            volume: volume.max(0.),
            open_rate: open_rate.clamp(0., 1.),
        }
    }
}

/// The drop zone, as shares of the plot: `left` and `right` across the
/// volume axis, `bottom` and `top` up the open-rate axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Zone {
    /// Left edge, 0 at no volume.
    pub left: f32,
    /// Right edge, 1 at the end of the volume axis.
    pub right: f32,
    /// Lower edge, 0 at no opens.
    pub bottom: f32,
    /// Upper edge, 1 at every email opened.
    pub top: f32,
}

impl Zone {
    /// The rectangle with two opposite corners at `a` and `b`, kept inside
    /// the plot and at least [`MIN_SIZE`] each way.
    pub fn spanning(a: (f32, f32), b: (f32, f32)) -> Self {
        let span = |from: f32, to: f32| {
            let (low, high) = (from.min(to).clamp(0., 1.), from.max(to).clamp(0., 1.));
            if high - low >= MIN_SIZE {
                (low, high)
            } else if to >= from {
                let low = low.min(1. - MIN_SIZE);
                (low, low + MIN_SIZE)
            } else {
                let high = high.max(MIN_SIZE);
                (high - MIN_SIZE, high)
            }
        };
        let (left, right) = span(a.0, b.0);
        let (bottom, top) = span(a.1, b.1);
        Self {
            left,
            right,
            bottom,
            top,
        }
    }

    /// The zone moved by `dx` and `dy`, stopped at the plot's edges.
    pub fn moved(self, dx: f32, dy: f32) -> Self {
        let width = self.right - self.left;
        let height = self.top - self.bottom;
        let left = (self.left + dx).clamp(0., 1. - width);
        let bottom = (self.bottom + dy).clamp(0., 1. - height);
        Self {
            left,
            right: left + width,
            bottom,
            top: bottom + height,
        }
    }

    /// The zone with its right and top edges pushed out by `dx` and `dy`,
    /// never smaller than [`MIN_SIZE`] nor past the plot.
    pub fn resized(self, dx: f32, dy: f32) -> Self {
        Self {
            right: (self.right + dx).clamp(self.left + MIN_SIZE, 1.),
            top: (self.top + dy).clamp(self.bottom + MIN_SIZE, 1.),
            ..self
        }
    }

    /// Whether a point, in shares of the plot, lies inside.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        (self.left..=self.right).contains(&x) && (self.bottom..=self.top).contains(&y)
    }

    /// The zone's four corners: bottom left, bottom right, top right, top
    /// left.
    pub fn corners(&self) -> [(f32, f32); 4] {
        [
            (self.left, self.bottom),
            (self.right, self.bottom),
            (self.right, self.top),
            (self.left, self.top),
        ]
    }
}

/// What a press on the plot takes hold of.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Hit {
    /// One of the zone's corners, by its index in [`Zone::corners`].
    Corner(usize),
    /// The body of the zone.
    Inside,
    /// Empty plot.
    Outside,
}

/// What lies under a point of the plot, in shares of it. A corner wins when
/// the point is within `reach_x` and `reach_y` of it.
pub fn hit_at(zone: &Zone, x: f32, y: f32, reach_x: f32, reach_y: f32) -> Hit {
    let corner = zone
        .corners()
        .iter()
        .position(|(cx, cy)| (x - cx).abs() <= reach_x && (y - cy).abs() <= reach_y);
    match corner {
        Some(ix) => Hit::Corner(ix),
        None if zone.contains(x, y) => Hit::Inside,
        None => Hit::Outside,
    }
}

/// The zone after an arrow key, with Shift resizing instead of moving.
pub fn key_zone(zone: Zone, key: &str, shift: bool) -> Option<Zone> {
    let (dx, dy) = match key {
        "left" => (-KEY_STEP, 0.),
        "right" => (KEY_STEP, 0.),
        "up" => (0., KEY_STEP),
        "down" => (0., -KEY_STEP),
        _ => return None,
    };
    Some(if shift {
        zone.resized(dx, dy)
    } else {
        zone.moved(dx, dy)
    })
}

/// A round axis end at or above `value`: 1, 2, 2.5 or 5 times a power of ten.
pub fn nice_ceiling(value: f64) -> f64 {
    if value <= 0. {
        return 1.;
    }
    let power = 10f64.powf(value.log10().floor());
    [1., 2., 2.5, 5., 10.]
        .iter()
        .map(|step| step * power)
        .find(|end| *end >= value - 1e-9)
        .unwrap_or(10. * power)
}

/// Where a sender sits, in shares of the plot.
pub fn place(sender: &Sender, x_max: f64) -> (f32, f32) {
    (
        (sender.volume / x_max.max(1e-9)).clamp(0., 1.) as f32,
        sender.open_rate as f32,
    )
}

/// The indices of the senders inside the zone, busiest first.
pub fn selected(senders: &[Sender], zone: &Zone, x_max: f64) -> Vec<usize> {
    let mut inside: Vec<usize> = (0..senders.len())
        .filter(|ix| {
            let (x, y) = place(&senders[*ix], x_max);
            zone.contains(x, y)
        })
        .collect();
    inside.sort_by(|a, b| senders[*b].volume.total_cmp(&senders[*a].volume));
    inside
}

/// A whole number with thousands separated by commas.
pub fn whole(value: f64) -> String {
    let digits = format!("{}", value.round().max(0.) as u64);
    let mut out = String::new();
    for (ix, digit) in digits.chars().enumerate() {
        if ix > 0 && (digits.len() - ix) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// The zone in words, such as "40 to 120 emails a month, opened 0% to 20%".
pub fn zone_words(zone: &Zone, x_max: f64) -> String {
    format!(
        "{} to {} emails a month, opened {}% to {}%",
        whole(f64::from(zone.left) * x_max),
        whole(f64::from(zone.right) * x_max),
        (zone.bottom * 100.).round(),
        (zone.top * 100.).round()
    )
}

/// The selection in words, such as "3 senders, 410 emails a month".
pub fn selection_words(count: usize, emails: f64) -> String {
    match count {
        0 => "No senders selected".into(),
        1 => format!("1 sender, {} emails a month", whole(emails)),
        _ => format!("{count} senders, {} emails a month", whole(emails)),
    }
}

/// Something the reader can do to every selected sender.
#[derive(Debug, Clone, PartialEq)]
pub struct TriageAction {
    /// Stable id, reported back when it runs.
    pub id: SharedString,
    /// Button label.
    pub label: SharedString,
    /// Button icon.
    pub icon: CatalogIcon,
    /// It cannot be undone, so it is drawn in the danger colour.
    pub danger: bool,
}

impl TriageAction {
    /// An action with a label and icon.
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        icon: CatalogIcon,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon,
            danger: false,
        }
    }

    /// Draw it in the danger colour.
    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }
}

/// What the plot is holding on to while the pointer is down.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Grab {
    /// Moving the zone from where it was when the press began.
    Move { from: (f32, f32), start: Zone },
    /// Spanning a zone from a fixed corner to the pointer.
    Span { anchor: (f32, f32) },
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TriageState {
    zone: Zone,
    grab: Option<Grab>,
    hover: Option<usize>,
    over_zone: bool,
}

type ZoneHandler = Rc<dyn Fn(&Zone, &mut Window, &mut App)>;
type ActionHandler = Rc<dyn Fn(&SharedString, &[SharedString], &mut Window, &mut App)>;

/// Senders by volume and open rate, with a zone that selects some to act on.
#[derive(IntoElement)]
pub struct SenderTriageScatter {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    subtitle: Option<SharedString>,
    senders: Vec<Sender>,
    zone: Zone,
    actions: Vec<TriageAction>,
    on_zone_change: Option<ZoneHandler>,
    on_action: Option<ActionHandler>,
}

impl SenderTriageScatter {
    /// Create a scatter of senders. The zone starts over busy senders that
    /// are rarely opened.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        senders: Vec<Sender>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            subtitle: None,
            senders,
            zone: Zone {
                left: 0.5,
                right: 1.,
                bottom: 0.,
                top: 0.25,
            },
            actions: vec![
                TriageAction::new("archive", "Archive all", CatalogIcon::Archive),
                TriageAction::new("unsubscribe", "Unsubscribe", CatalogIcon::Email).danger(),
            ],
            on_zone_change: None,
            on_action: None,
        }
    }

    /// A quieter line under the title.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Where the zone starts, as shares of the plot. Used the first time the
    /// scatter is drawn; after that the reader's zone is kept.
    pub fn zone(mut self, zone: Zone) -> Self {
        self.zone = Zone::spanning((zone.left, zone.bottom), (zone.right, zone.top));
        self
    }

    /// Replace the actions offered for the selection.
    pub fn actions(mut self, actions: Vec<TriageAction>) -> Self {
        self.actions = actions;
        self
    }

    /// Run when the zone is moved or resized, once the drag or key press ends.
    pub fn on_zone_change(
        mut self,
        handler: impl Fn(&Zone, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_zone_change = Some(Rc::new(handler));
        self
    }

    /// Run with an action's id and the ids of the selected senders.
    pub fn on_action(
        mut self,
        handler: impl Fn(&SharedString, &[SharedString], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(handler));
        self
    }
}

impl Styled for SenderTriageScatter {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

fn paint_dot(window: &mut Window, x: Pixels, y: Pixels, r: f32, color: Hsla) {
    let ring: Vec<_> = (0..14)
        .map(|step| {
            let angle = step as f32 / 14. * TAU;
            point(x + px(r * angle.cos()), y + px(r * angle.sin()))
        })
        .collect();
    let mut dot = PathBuilder::fill();
    dot.add_polygon(&ring, true);
    if let Ok(dot) = dot.build() {
        window.paint_path(dot, color);
    }
}

/// A pointer position as shares of the plot, the open-rate axis pointing up.
fn share_of(bounds: Bounds<Pixels>, position: isonomic_ui::Point<Pixels>) -> (f32, f32) {
    let width = f32::from(bounds.size.width).max(1.);
    let height = f32::from(bounds.size.height).max(1.);
    (
        (f32::from(position.x - bounds.left()) / width).clamp(0., 1.),
        (1. - f32::from(position.y - bounds.top()) / height).clamp(0., 1.),
    )
}

impl RenderOnce for SenderTriageScatter {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let part = |name: &str| ElementId::NamedChild(self.id.clone().into(), name.into());
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let pick = theme.chart_1;
        let surface = theme.background;
        let raised = theme.popover;
        let ring = theme.ring;
        let mono = theme.mono_font_family.clone();
        let grid = ink.opacity(0.08);
        let border = theme.border;

        let peak = self.senders.iter().map(|one| one.volume).fold(0., f64::max);
        let x_max = nice_ceiling(peak);
        let initial = self.zone;
        let state = window.use_keyed_state(part("state"), cx, move |_, _| TriageState {
            zone: initial,
            grab: None,
            hover: None,
            over_zone: false,
        });
        let now = *state.read(cx);
        let zone = now.zone;
        let plot_bounds = window
            .use_keyed_state(part("bounds"), cx, |_, _| {
                Rc::new(Cell::new(Bounds::default()))
            })
            .read(cx)
            .clone();
        let focus = window
            .use_keyed_state(part("focus"), cx, |_, cx| cx.focus_handle().tab_stop(true))
            .read(cx)
            .clone();
        let focused = focus.is_focused(window);

        let picked = selected(&self.senders, &zone, x_max);
        let emails: f64 = picked.iter().map(|ix| self.senders[*ix].volume).sum();
        let places: Vec<(f32, f32)> = self.senders.iter().map(|one| place(one, x_max)).collect();
        let inside: Vec<bool> = (0..self.senders.len())
            .map(|ix| picked.contains(&ix))
            .collect();

        let seen = plot_bounds.clone();
        let paint_places = places.clone();
        let hover = now.hover;
        let dragging = now.grab.is_some();
        let plot = canvas(
            move |bounds, _, _| seen.set(bounds),
            move |bounds, _, window, _| {
                let w = f32::from(bounds.size.width);
                let h = f32::from(bounds.size.height);
                let at = |x: f32, y: f32| {
                    point(bounds.left() + px(x * w), bounds.top() + px((1. - y) * h))
                };
                let stroke = |window: &mut Window,
                              from: (f32, f32),
                              to: (f32, f32),
                              wide: f32,
                              dash: Option<[f32; 2]>,
                              color| {
                    let mut path = PathBuilder::stroke(px(wide));
                    if let Some([on, off]) = dash {
                        path = path.dash_array(&[px(on), px(off)]);
                    }
                    path.move_to(at(from.0, from.1));
                    path.line_to(at(to.0, to.1));
                    if let Ok(path) = path.build() {
                        window.paint_path(path, color);
                    }
                };
                for quarter in [0., 0.25, 0.5, 0.75, 1.] {
                    stroke(
                        window,
                        (0., quarter),
                        (1., quarter),
                        1.,
                        Some([2., 4.]),
                        grid,
                    );
                    stroke(
                        window,
                        (quarter, 0.),
                        (quarter, 1.),
                        1.,
                        Some([2., 4.]),
                        grid,
                    );
                }

                // The zone: a soft fill, a dashed edge and a handle per corner.
                let mut fill = PathBuilder::fill();
                let corners: Vec<_> = zone.corners().iter().map(|(x, y)| at(*x, *y)).collect();
                fill.add_polygon(&corners, true);
                if let Ok(fill) = fill.build() {
                    window.paint_path(fill, pick.opacity(if dragging { 0.14 } else { 0.09 }));
                }
                let edge = zone.corners();
                for side in 0..4 {
                    stroke(
                        window,
                        edge[side],
                        edge[(side + 1) % 4],
                        1.5,
                        Some([4., 3.]),
                        pick.opacity(0.9),
                    );
                }

                for (ix, (x, y)) in paint_places.iter().enumerate() {
                    let spot = at(*x, *y);
                    let color = if inside[ix] { pick } else { ink.opacity(0.28) };
                    if hover == Some(ix) {
                        paint_dot(window, spot.x, spot.y, DOT + 3., color.opacity(0.25));
                    }
                    paint_dot(window, spot.x, spot.y, DOT, color);
                }

                for corner in edge {
                    let spot = at(corner.0, corner.1);
                    paint_dot(window, spot.x, spot.y, 5., pick);
                    paint_dot(window, spot.x, spot.y, 3., surface);
                }
            },
        )
        .absolute()
        .inset_0()
        .size_full();

        // Pointer handling: press to grab, move to drag or point, release to let go.
        let down_state = state.clone();
        let down_bounds = plot_bounds.clone();
        let down_focus = focus.clone();
        let move_state = state.clone();
        let move_bounds = plot_bounds;
        let move_places = places.clone();
        let up_state = state.clone();
        let up_change = self.on_zone_change.clone();
        let out_state = state.clone();
        let out_change = self.on_zone_change.clone();
        let leave_state = state.clone();
        let key_state = state;
        let key_change = self.on_zone_change.clone();
        let release = |state: &isonomic_ui::Entity<TriageState>,
                       change: &Option<ZoneHandler>,
                       window: &mut Window,
                       cx: &mut App| {
            if state.read(cx).grab.is_none() {
                return;
            }
            state.update(cx, |state, cx| {
                state.grab = None;
                cx.notify();
            });
            let zone = state.read(cx).zone;
            if let Some(change) = change {
                change(&zone, window, cx);
            }
        };

        let tip = now.hover.filter(|_| !dragging).and_then(|ix| {
            let sender = self.senders.get(ix)?;
            let (x, y) = places[ix];
            Some(
                div()
                    .absolute()
                    .left(relative(x.min(0.7)))
                    .top(relative((1. - y).clamp(0., 0.78)))
                    .ml(px(12.))
                    .mt(px(6.))
                    .px_3()
                    .py_2()
                    .rounded(px(12.))
                    .bg(raised)
                    .shadow_md()
                    .text_caption()
                    .child(div().text_label_small().child(sender.name.clone()))
                    .child(
                        div()
                            .font_family(mono.clone())
                            .text_color(quiet)
                            .child(format!(
                                "{} a month · {}% opened",
                                whole(sender.volume),
                                (sender.open_rate * 100.).round()
                            )),
                    ),
            )
        });

        let spoken = format!(
            "{}: drop zone covers {}. {}. Arrow keys move the zone, Shift with arrow keys resizes it, Escape resets it.",
            self.title,
            zone_words(&zone, x_max),
            selection_words(picked.len(), emails)
        );

        let plot_area = div()
            .id(part("plot"))
            .test_support()
            .track_focus(&focus)
            .absolute()
            .top_0()
            .left(px(GUTTER))
            .right_0()
            .h(px(PLOT_H))
            .rounded(px(4.))
            .role(Role::Group)
            .aria_label(spoken)
            .map(|this| {
                if dragging || now.over_zone {
                    this.cursor_grab()
                } else {
                    this.cursor_crosshair()
                }
            })
            .on_mouse_down(
                MouseButton::Left,
                move |event: &MouseDownEvent, window, cx| {
                    window.focus(&down_focus, cx);
                    let bounds = down_bounds.get();
                    let (x, y) = share_of(bounds, event.position);
                    let reach_x = HANDLE / f32::from(bounds.size.width).max(1.);
                    let reach_y = HANDLE / f32::from(bounds.size.height).max(1.);
                    down_state.update(cx, |state, cx| {
                        let zone = state.zone;
                        state.grab = Some(match hit_at(&zone, x, y, reach_x, reach_y) {
                            Hit::Corner(ix) => Grab::Span {
                                anchor: zone.corners()[(ix + 2) % 4],
                            },
                            Hit::Inside => Grab::Move {
                                from: (x, y),
                                start: zone,
                            },
                            Hit::Outside => {
                                state.zone = Zone::spanning((x, y), (x, y));
                                Grab::Span { anchor: (x, y) }
                            }
                        });
                        state.hover = None;
                        cx.notify();
                    });
                    cx.stop_propagation();
                },
            )
            .on_mouse_move(move |event: &MouseMoveEvent, window, cx| {
                let bounds = move_bounds.get();
                let (x, y) = share_of(bounds, event.position);
                let now = *move_state.read(cx);
                if now.grab.is_some() && event.pressed_button != Some(MouseButton::Left) {
                    move_state.update(cx, |state, cx| {
                        state.grab = None;
                        cx.notify();
                    });
                    return;
                }
                let next = match now.grab {
                    Some(Grab::Move { from, start }) => TriageState {
                        zone: start.moved(x - from.0, y - from.1),
                        ..now
                    },
                    Some(Grab::Span { anchor }) => TriageState {
                        zone: Zone::spanning(anchor, (x, y)),
                        ..now
                    },
                    None => {
                        let w = f32::from(bounds.size.width).max(1.);
                        let h = f32::from(bounds.size.height).max(1.);
                        let near = move_places
                            .iter()
                            .enumerate()
                            .map(|(ix, (px_, py))| {
                                let dx = (px_ - x) * w;
                                let dy = (py - y) * h;
                                (ix, dx * dx + dy * dy)
                            })
                            .filter(|(_, d)| *d <= (DOT + 4.) * (DOT + 4.))
                            .min_by(|a, b| a.1.total_cmp(&b.1))
                            .map(|(ix, _)| ix);
                        TriageState {
                            hover: near,
                            over_zone: now.zone.contains(x, y),
                            ..now
                        }
                    }
                };
                if next != now {
                    move_state.update(cx, |state, cx| {
                        *state = next;
                        cx.notify();
                    });
                    window.refresh();
                }
            })
            .on_mouse_up(MouseButton::Left, move |_, window, cx| {
                release(&up_state, &up_change, window, cx)
            })
            .on_mouse_up_out(MouseButton::Left, move |_, window, cx| {
                release(&out_state, &out_change, window, cx)
            })
            .on_hover(move |over, _, cx| {
                if !*over {
                    leave_state.update(cx, |state, cx| {
                        state.hover = None;
                        state.over_zone = false;
                        cx.notify();
                    });
                }
            })
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let key = event.keystroke.key.as_str();
                let zone = key_state.read(cx).zone;
                let next = if key == "escape" {
                    initial
                } else {
                    match key_zone(zone, key, event.keystroke.modifiers.shift) {
                        Some(next) => next,
                        None => return,
                    }
                };
                key_state.update(cx, |state, cx| {
                    state.zone = next;
                    cx.notify();
                });
                if let Some(change) = &key_change {
                    change(&next, window, cx);
                }
                cx.stop_propagation();
            })
            .child(plot)
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
            })
            .children(tip);

        let axis_text = |text: String| {
            div()
                .flex_none()
                .whitespace_nowrap()
                .font_family(mono.clone())
                .text_size(px(10.))
                .line_height(px(16.))
                .text_color(quiet)
                .child(text)
        };
        let y_labels = [0., 0.25, 0.5, 0.75, 1.].map(|share: f32| {
            div()
                .absolute()
                .left_0()
                .w(px(GUTTER - 6.))
                .top(px((1. - share) * PLOT_H - 8.))
                .flex()
                .justify_end()
                .child(axis_text(format!("{}%", (share * 100.).round())))
        });
        let x_labels = [0., 0.25, 0.5, 0.75, 1.].map(|share: f32| {
            div()
                .absolute()
                .top(px(PLOT_H + 4.))
                .left(relative(share))
                .w(px(0.))
                .flex()
                .map(|this| match share {
                    s if s <= 0. => this.justify_start(),
                    s if s >= 1. => this.justify_end(),
                    _ => this.justify_center(),
                })
                .child(axis_text(whole(f64::from(share) * x_max)))
        });

        let chart = div()
            .relative()
            .w_full()
            .h(px(PLOT_H + AXIS))
            .children(y_labels)
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left(px(GUTTER))
                    .right_0()
                    .bottom_0()
                    .children(x_labels),
            )
            .child(plot_area);

        let names: Vec<SharedString> = picked
            .iter()
            .take(NAMED)
            .map(|ix| self.senders[*ix].name.clone())
            .collect();
        let more = picked.len().saturating_sub(NAMED);
        let ids: Rc<Vec<SharedString>> = Rc::new(
            picked
                .iter()
                .map(|ix| self.senders[*ix].id.clone())
                .collect(),
        );
        let buttons = self.actions.iter().map(|action| {
            let run = self.on_action.clone();
            let ids = ids.clone();
            let action_id = action.id.clone();
            Button::new(part(&format!("action-{}", action.id)))
                .small()
                .map(|this| {
                    if action.danger {
                        this.danger()
                    } else {
                        this.secondary()
                    }
                })
                .icon(Icon::new(action.icon))
                .label(action.label.clone())
                .disabled(ids.is_empty())
                .on_click(move |_, window, cx| {
                    if let Some(run) = &run {
                        run(&action_id, &ids, window, cx);
                    }
                })
        });

        let footer = div()
            .mt_3()
            .pt_3()
            .border_t_1()
            .border_color(border)
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        div()
                            .id(part("count"))
                            .flex_1()
                            .min_w(px(160.))
                            .role(Role::Status)
                            .aria_label(selection_words(picked.len(), emails))
                            .child(div().text_label().child(match picked.len() {
                                0 => "No senders in the zone".to_string(),
                                1 => "1 sender selected".to_string(),
                                n => format!("{n} senders selected"),
                            }))
                            .child(
                                div()
                                    .text_caption()
                                    .text_color(quiet)
                                    .font_family(mono.clone())
                                    .child(format!("{} emails a month", whole(emails))),
                            ),
                    )
                    .child(h_flex().gap_2().children(buttons)),
            )
            .when(!names.is_empty(), |this| {
                this.child(
                    h_flex()
                        .mt_2()
                        .flex_wrap()
                        .gap_1()
                        .children(
                            names
                                .into_iter()
                                .map(|name| Tag::secondary().small().child(name)),
                        )
                        .when(more > 0, |this| {
                            this.child(
                                div()
                                    .text_caption()
                                    .text_color(quiet)
                                    .pl_1()
                                    .child(format!("+{more} more")),
                            )
                        }),
                )
            });

        let legend = h_flex()
            .mb_2()
            .gap_4()
            .flex_wrap()
            .text_caption()
            .text_color(quiet)
            .child(
                h_flex()
                    .gap(px(6.))
                    .child(div().size(px(8.)).rounded_full().bg(pick))
                    .child("In the zone"),
            )
            .child(
                h_flex()
                    .gap(px(6.))
                    .child(div().size(px(8.)).rounded_full().bg(ink.opacity(0.28)))
                    .child("Other senders"),
            )
            .child(
                div()
                    .ml_auto()
                    .child("Drag the zone, its corners, or draw a new one"),
            );

        shell(cx)
            .id(self.id.clone())
            .max_w(px(672.))
            .child(shell_header(
                Some(Icon::new(CatalogIcon::ChartScatter)),
                self.title.clone(),
                self.subtitle.clone().map(|line| line.into_any_element()),
                Some(
                    div()
                        .font_family(mono.clone())
                        .text_caption()
                        .text_color(quiet)
                        .child(format!("{} senders", self.senders.len()))
                        .into_any_element(),
                ),
                cx,
            ))
            .child(
                panel(cx)
                    .p_4()
                    .child(legend)
                    .child(chart)
                    .child(
                        h_flex()
                            .pl(px(GUTTER))
                            .justify_between()
                            .text_caption()
                            .text_color(quiet)
                            .child("Open rate, up the side")
                            .child("Emails a month, along the bottom"),
                    )
                    .child(footer),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone(left: f32, right: f32, bottom: f32, top: f32) -> Zone {
        Zone {
            left,
            right,
            bottom,
            top,
        }
    }

    fn close(a: Zone, b: Zone) -> bool {
        (a.left - b.left).abs() < 1e-5
            && (a.right - b.right).abs() < 1e-5
            && (a.bottom - b.bottom).abs() < 1e-5
            && (a.top - b.top).abs() < 1e-5
    }

    #[test]
    fn a_zone_spans_two_corners_in_either_order() {
        assert!(close(
            Zone::spanning((0.8, 0.1), (0.2, 0.6)),
            zone(0.2, 0.8, 0.1, 0.6)
        ));
        // Too small grows to the least size, in the direction dragged.
        assert!(close(
            Zone::spanning((0.5, 0.5), (0.5, 0.5)),
            zone(0.5, 0.56, 0.5, 0.56)
        ));
        assert!(close(
            Zone::spanning((0.5, 0.5), (0.48, 0.48)),
            zone(0.44, 0.5, 0.44, 0.5)
        ));
        // At the far edge it stays inside the plot.
        assert!(close(
            Zone::spanning((1., 1.), (1.2, 1.3)),
            zone(0.94, 1., 0.94, 1.)
        ));
    }

    #[test]
    fn moving_stops_at_the_edges_and_keeps_the_size() {
        let start = zone(0.5, 0.9, 0., 0.3);
        assert!(close(start.moved(0.3, -0.2), zone(0.6, 1., 0., 0.3)));
        assert!(close(start.moved(-0.2, 0.1), zone(0.3, 0.7, 0.1, 0.4)));
    }

    #[test]
    fn resizing_keeps_the_least_size() {
        let start = zone(0.5, 0.6, 0., 0.1);
        assert!(close(start.resized(-0.2, -0.2), zone(0.5, 0.56, 0., 0.06)));
        assert!(close(start.resized(0.6, 0.95), zone(0.5, 1., 0., 1.)));
    }

    #[test]
    fn arrow_keys_move_and_shift_resizes() {
        let start = zone(0.5, 0.7, 0.2, 0.4);
        assert!(close(
            key_zone(start, "right", false).unwrap(),
            zone(0.55, 0.75, 0.2, 0.4)
        ));
        assert!(close(
            key_zone(start, "down", false).unwrap(),
            zone(0.5, 0.7, 0.15, 0.35)
        ));
        assert!(close(
            key_zone(start, "right", true).unwrap(),
            zone(0.5, 0.75, 0.2, 0.4)
        ));
        assert!(close(
            key_zone(start, "down", true).unwrap(),
            zone(0.5, 0.7, 0.2, 0.35)
        ));
        assert_eq!(key_zone(start, "a", false), None);
    }

    #[test]
    fn a_press_takes_a_corner_before_the_body() {
        let start = zone(0.4, 0.8, 0.2, 0.6);
        assert_eq!(hit_at(&start, 0.41, 0.21, 0.02, 0.02), Hit::Corner(0));
        assert_eq!(hit_at(&start, 0.79, 0.59, 0.02, 0.02), Hit::Corner(2));
        assert_eq!(hit_at(&start, 0.6, 0.4, 0.02, 0.02), Hit::Inside);
        assert_eq!(hit_at(&start, 0.1, 0.4, 0.02, 0.02), Hit::Outside);
    }

    #[test]
    fn senders_inside_the_zone_are_picked_busiest_first() {
        let senders = vec![
            Sender::new("a", "A", 40., 0.1),
            Sender::new("b", "B", 180., 0.05),
            Sender::new("c", "C", 150., 0.9),
            Sender::new("d", "D", 120., 0.2),
        ];
        let x_max = nice_ceiling(180.);
        assert_eq!(x_max, 200.);
        assert_eq!(selected(&senders, &zone(0.5, 1., 0., 0.25), x_max), [1, 3]);
        assert_eq!(place(&senders[0], x_max), (0.2, 0.1));
    }

    #[test]
    fn axis_ends_are_round() {
        assert_eq!(nice_ceiling(0.), 1.);
        assert_eq!(nice_ceiling(7.), 10.);
        assert_eq!(nice_ceiling(180.), 200.);
        assert_eq!(nice_ceiling(210.), 250.);
        assert_eq!(nice_ceiling(400.), 500.);
        assert_eq!(nice_ceiling(1000.), 1000.);
    }

    #[test]
    fn the_selection_is_said_in_words() {
        assert_eq!(whole(1234.4), "1,234");
        assert_eq!(whole(999.), "999");
        assert_eq!(whole(1_000_000.), "1,000,000");
        assert_eq!(selection_words(0, 0.), "No senders selected");
        assert_eq!(selection_words(1, 30.), "1 sender, 30 emails a month");
        assert_eq!(selection_words(4, 1210.), "4 senders, 1,210 emails a month");
        assert_eq!(
            zone_words(&zone(0.2, 0.6, 0., 0.2), 200.),
            "40 to 120 emails a month, opened 0% to 20%"
        );
    }
}
