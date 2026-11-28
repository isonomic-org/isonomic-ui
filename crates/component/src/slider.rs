// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{rc::Rc, sync::Arc};

use crate::{ActiveTheme, AxisExt, StyledExt, TextRole as _, ThemeStyled as _};
pub use isonomic_ui_base::slider::{SliderEvent, SliderScale, SliderState, SliderValue};
use isonomic_ui_base::{
    LogicalStyledExt as _, Slider as BaseSlider, SliderIndicator, SliderThumb, SliderTrack, spring,
};

use gpui::{
    App, Axis, Background, Corners, DefiniteLength, ElementId, Entity, EntityId, Hsla,
    InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, Pixels, RenderOnce,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

/// Width of the translucent ring that grows outside a hovered thumb.
const THUMB_RING_WIDTH: Pixels = px(3.);
/// Opacity of the fully grown thumb ring.
const THUMB_RING_OPACITY: f32 = 0.12;
/// The animated hover ring of one thumb.
struct ThumbRing {
    interaction: Entity<ThumbInteraction>,
    width: Pixels,
    color: Hsla,
    /// How far the ring has grown, 0 to 1: also how strongly a value label
    /// shown only while the thumb is active shows.
    progress: f32,
}

/// Pointer state of one thumb, written by its own listeners and read on the
/// next frame to size the ring.
#[derive(Default)]
struct ThumbInteraction {
    hovered: bool,
    pressed: bool,
}

impl ThumbInteraction {
    /// The ring shows while the pointer is over the thumb, and stays while the
    /// thumb is dragged: dragging moves the thumb under the pointer, so hover
    /// alone drops out for a frame on every move and the ring would flicker.
    fn is_active(&self) -> bool {
        self.hovered || self.pressed
    }
}

impl ThumbRing {
    /// Samples the ring for the `start` or end thumb of the slider `id`. The ring
    /// also shows while the thumb is `focused`: the one the keys move.
    fn new(
        id: EntityId,
        start: bool,
        focused: bool,
        color: Hsla,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let channel = if start { "start" } else { "end" };
        let interaction = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(("slider-thumb-ring", id).into()), channel.into()),
            cx,
            |_, _| ThumbInteraction::default(),
        );
        let progress = spring(
            (("slider-thumb-ring", id), channel),
            if focused || interaction.read(cx).is_active() {
                1.
            } else {
                0.
            },
            cx.theme().motion_tokens().spring_control,
            window,
            cx,
        );

        Self {
            interaction,
            width: THUMB_RING_WIDTH * progress,
            color: color.alpha(THUMB_RING_OPACITY * progress),
            progress,
        }
    }

    /// Returns a listener that records whether the thumb is being pressed.
    fn press_listener<E>(&self, pressed: bool) -> impl Fn(&E, &mut Window, &mut App) + use<E> {
        let interaction = self.interaction.clone();
        move |_, _, cx| {
            interaction.update(cx, |interaction, cx| {
                if interaction.pressed != pressed {
                    interaction.pressed = pressed;
                    cx.notify();
                }
            });
        }
    }
}

/// A labelled point on a slider's track, such as "Low" at 0 or "50%".
#[derive(Debug, Clone, PartialEq)]
pub struct SliderMark {
    value: f32,
    label: Option<SharedString>,
}

impl SliderMark {
    /// A mark at `value`, drawn as a dot on the track.
    pub fn new(value: f32) -> Self {
        Self { value, label: None }
    }

    /// Writes `label` under the mark (beside it on a vertical slider).
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// When a slider shows its value over the thumb.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SliderValueLabel {
    /// Never; the default.
    #[default]
    Hidden,
    /// Always, with room kept above the track for it.
    Always,
    /// While the thumb is hovered, dragged or focused, fading with its ring.
    WhileActive,
}

/// The most step dots a track draws; a finer step draws none.
pub const MAX_STEP_MARKS: usize = 40;

/// The values a slider from `min` to `max` stops at every `step`, for
/// drawing a dot at each: empty when there would be more than `limit`, or
/// the step is not a positive number.
pub fn step_mark_values(min: f32, max: f32, step: f32, limit: usize) -> Vec<f32> {
    if !step.is_finite() || step <= 0. || max.partial_cmp(&min) != Some(std::cmp::Ordering::Greater)
    {
        return Vec::new();
    }
    let count = ((max - min) / step + 1e-4).floor() as usize + 1;
    if count > limit {
        return Vec::new();
    }
    (0..count).map(|ix| min + step * ix as f32).collect()
}

/// A value written as a slider's step suggests: whole numbers for a whole
/// step, otherwise as many decimals as the step has (at most four).
pub fn format_slider_value(value: f32, step: f32) -> String {
    let mut decimals = 0;
    let mut scaled = step.abs();
    while decimals < 4
        && scaled.is_finite()
        && scaled > 0.
        && (scaled - scaled.round()).abs() > 1e-4
    {
        scaled *= 10.;
        decimals += 1;
    }
    format!("{value:.decimals$}")
}

/// Whether a mark at `fraction` of the track lies on the filled part.
pub fn mark_is_filled(fraction: f32, filled: std::ops::Range<f32>) -> bool {
    fraction >= filled.start - 1e-4 && fraction <= filled.end + 1e-4
}

type ValueFormatter = Rc<dyn Fn(f32) -> SharedString>;

/// A Slider element.
#[derive(IntoElement)]
pub struct Slider {
    state: Entity<SliderState>,
    axis: Axis,
    style: StyleRefinement,
    disabled: bool,
    accessibility_label: Option<SharedString>,
    reverse: bool,
    tab_stop: bool,
    tab_index: isize,
    marks: Vec<SliderMark>,
    step_marks: bool,
    value_label: SliderValueLabel,
    format_value: Option<ValueFormatter>,
}

impl Slider {
    /// Create a new [`Slider`] element bind to the [`SliderState`].
    pub fn new(state: &Entity<SliderState>) -> Self {
        Self {
            axis: Axis::Horizontal,
            state: state.clone(),
            style: StyleRefinement::default(),
            disabled: false,
            accessibility_label: None,
            reverse: false,
            tab_stop: true,
            tab_index: 0,
            marks: Vec::new(),
            step_marks: false,
            value_label: SliderValueLabel::default(),
            format_value: None,
        }
    }

    /// Draws a dot on the track at each mark, and its label beneath.
    pub fn marks(mut self, marks: impl IntoIterator<Item = SliderMark>) -> Self {
        self.marks = marks.into_iter().collect();
        self
    }

    /// Draws a dot at every step the slider stops at, when there are at most
    /// [`MAX_STEP_MARKS`] of them. Off by default.
    pub fn step_marks(mut self, step_marks: bool) -> Self {
        self.step_marks = step_marks;
        self
    }

    /// When to show the value over the thumb, default: never.
    pub fn value_label(mut self, value_label: SliderValueLabel) -> Self {
        self.value_label = value_label;
        self
    }

    /// How the value label writes a value, such as `"40%"`. By default it
    /// shows as many decimals as the step has.
    pub fn format_value(mut self, format: impl Fn(f32) -> SharedString + 'static) -> Self {
        self.format_value = Some(Rc::new(format));
        self
    }

    /// As a horizontal slider.
    pub fn horizontal(mut self) -> Self {
        self.axis = Axis::Horizontal;
        self
    }

    /// As a vertical slider.
    pub fn vertical(mut self) -> Self {
        self.axis = Axis::Vertical;
        self
    }

    /// Set the disabled state of the slider, default: false
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Sets the label exposed to accessibility clients.
    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }

    /// Set the tab stop for the slider, default is true.
    pub fn tab_stop(mut self, tab_stop: bool) -> Self {
        self.tab_stop = tab_stop;
        self
    }

    /// Set the tab index for the slider, default is 0.
    pub fn tab_index(mut self, tab_index: isize) -> Self {
        self.tab_index = tab_index;
        self
    }

    /// Reverse the filled (highlighted) side of the track, default: false.
    ///
    /// By default the track is filled from the min end to the thumb. With
    /// `reverse`, the fill goes from the thumb to the max end instead — useful
    /// when the slider represents a remaining amount (e.g. time left).
    ///
    /// This only changes the visual fill; values, events and interactions are
    /// unaffected. It applies to single-value sliders and is ignored for
    /// range sliders.
    pub fn reverse(mut self) -> Self {
        self.reverse = true;
        self
    }
}

impl Styled for Slider {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Slider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let axis = self.axis;
        let state = self.state.read(cx);
        let is_range = state.value().is_range();
        let percentage = state.percentage();
        let (bar_start, bar_end) = if self.reverse && !is_range {
            // Fill from the thumb to the max end (remaining side).
            (relative(percentage.end), relative(0.))
        } else {
            (relative(percentage.start), relative(1. - percentage.end))
        };
        let rem_size = window.rem_size();
        let (min, max, step) = (state.min_value(), state.max_value(), state.step_value());
        let value = state.value();
        let mut dots: Vec<(f32, Option<SharedString>)> = Vec::new();
        if self.step_marks {
            dots.extend(
                step_mark_values(min, max, step, MAX_STEP_MARKS)
                    .into_iter()
                    .map(|value| (state.fraction_of(value), None)),
            );
        }
        for mark in &self.marks {
            let fraction = state.fraction_of(mark.value);
            // A labelled mark on a step replaces the step's bare dot.
            dots.retain(|(f, label)| label.is_some() || (f - fraction).abs() > 1e-4);
            dots.push((fraction, mark.label.clone()));
        }
        let has_mark_labels = dots.iter().any(|(_, label)| label.is_some());
        let format = self.format_value.clone();
        let step_for_format = step;
        let label_text = move |value: f32| -> SharedString {
            match &format {
                Some(format) => format(value),
                None => format_slider_value(value, step_for_format).into(),
            }
        };
        let (start_text, end_text) = (
            value.is_range().then(|| label_text(value.start())),
            label_text(value.end()),
        );
        let filled = if self.reverse && !is_range {
            percentage.end..1.
        } else {
            percentage.clone()
        };

        let bar_color = self
            .style
            .background
            .clone()
            .and_then(|bg| bg.color())
            .unwrap_or(cx.theme().tokens.slider_bar.into());
        let thumb_bg: Background = self
            .style
            .text
            .color
            .map(Into::into)
            // The knob stays a lit object over the track in both modes, never a
            // dark hole in it.
            .unwrap_or_else(|| {
                if cx.theme().mode.is_dark() {
                    cx.theme().foreground.into()
                } else {
                    cx.theme().tokens.switch_thumb.into()
                }
            });
        let corner_radii = self.style.corner_radii.clone();
        // The track is a pill by default, and square when the theme squares its
        // corners. A caller's own corner radii still win.
        let default_radius = cx.theme().radius_full();
        let radius = Corners {
            top_left: corner_radii
                .top_left
                .map(|v| v.to_pixels(rem_size))
                .unwrap_or(default_radius),
            top_right: corner_radii
                .top_right
                .map(|v| v.to_pixels(rem_size))
                .unwrap_or(default_radius),
            bottom_left: corner_radii
                .bottom_left
                .map(|v| v.to_pixels(rem_size))
                .unwrap_or(default_radius),
            bottom_right: corner_radii
                .bottom_right
                .map(|v| v.to_pixels(rem_size))
                .unwrap_or(default_radius),
        };

        let ring_color = cx.theme().ring;
        let entity_id = self.state.entity_id();
        let focus_handle = window
            .use_keyed_state(("slider-focus", entity_id), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        let is_focused = focus_handle.is_focused(window);
        let start_ring =
            is_range.then(|| ThumbRing::new(entity_id, true, false, ring_color, window, cx));
        // The keys move the end thumb, so it carries the focus ring.
        let end_ring = ThumbRing::new(entity_id, false, is_focused, ring_color, window, cx);

        let value_label = self.value_label;
        let label_bg = cx.theme().foreground;
        let label_fg = cx.theme().background;
        let thumb = |position: DefiniteLength, start: bool, ring: ThumbRing, text: SharedString| {
            let label_strength = match value_label {
                SliderValueLabel::Hidden => 0.,
                SliderValueLabel::Always => 1.,
                SliderValueLabel::WhileActive => ring.progress.clamp(0., 1.),
            };
            // Centred over the thumb: a zero-wide box at the thumb's centre
            // lets the tag overflow evenly to both sides.
            let tag = (label_strength > 0.).then(|| {
                div()
                    .absolute()
                    .when(axis.is_horizontal(), |this| {
                        this.left(px(8.))
                            .bottom(px(22.))
                            .w(px(0.))
                            .flex()
                            .justify_center()
                    })
                    .when(axis.is_vertical(), |this| {
                        this.right(px(22.))
                            .top(px(8.))
                            .h(px(0.))
                            .flex()
                            .items_center()
                    })
                    .child(
                        div()
                            .flex_none()
                            .whitespace_nowrap()
                            .px(px(6.))
                            .rounded(cx.theme().radius)
                            .bg(label_bg)
                            .text_color(label_fg)
                            .text_badge()
                            .opacity(label_strength)
                            .child(text),
                    )
            });
            SliderThumb::new(&self.state)
                .axis(axis)
                .start(start)
                .disabled(self.disabled)
                .when(!self.disabled, |this| {
                    this.absolute()
                        .when(axis.is_horizontal(), |this| {
                            this.top(px(-6.)).inset_s(position, cx).ms(-px(8.), cx)
                        })
                        .when(axis.is_vertical(), |this| {
                            this.bottom(position).left(px(-6.)).mb(-px(8.))
                        })
                        .flex()
                        .items_center()
                        .justify_center()
                        .flex_shrink_0()
                        .rounded_full_style(cx)
                        // A white knob lifted by a soft shadow, as on a switch.
                        .shadow(crate::look::surface::knob(cx))
                        .size_4()
                        .on_hover({
                            let interaction = ring.interaction.clone();
                            move |entered, _, cx| {
                                let entered = *entered;
                                interaction.update(cx, |interaction, cx| {
                                    if interaction.hovered != entered {
                                        interaction.hovered = entered;
                                        cx.notify();
                                    }
                                });
                            }
                        })
                        // The base thumb stops propagation on mouse down to own
                        // the drag, so the press is read in the capture phase.
                        .capture_any_mouse_down(ring.press_listener(true))
                        .capture_any_mouse_up(ring.press_listener(false))
                        .on_mouse_up_out(MouseButton::Left, ring.press_listener(false))
                        // The ring grows outward from the thumb's edge: inset by
                        // its own width so its inner edge sits flush against it.
                        .child(
                            div()
                                .flex_none()
                                .absolute()
                                .top(-ring.width)
                                .left(-ring.width)
                                .right(-ring.width)
                                .bottom(-ring.width)
                                .rounded_full_style(cx)
                                .border(ring.width)
                                .border_color(ring.color),
                        )
                        .child(
                            div()
                                .flex_shrink_0()
                                .size_full()
                                .rounded_full_style(cx)
                                .bg(thumb_bg),
                        )
                        .children(tag)
                })
        };
        let dot_filled = if cx.theme().mode.is_dark() {
            cx.theme().background.opacity(0.55)
        } else {
            cx.theme().background.opacity(0.7)
        };
        let dot_filled: Background = dot_filled.into();
        let dot_empty = bar_color.opacity(0.4);
        let mark_label_color = crate::look::ink::subtle(cx);
        let mark_elements = dots
            .iter()
            .map(|(fraction, _)| {
                let on = mark_is_filled(*fraction, filled.clone());
                div()
                    .absolute()
                    .size(px(4.))
                    .rounded_full_style(cx)
                    .bg(if on { dot_filled } else { dot_empty })
                    .when(axis.is_horizontal(), |this| {
                        this.top(px(0.))
                            .inset_s(relative(*fraction), cx)
                            .ms(-px(2.), cx)
                    })
                    .when(axis.is_vertical(), |this| {
                        this.left(px(0.)).bottom(relative(*fraction)).mb(-px(2.))
                    })
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        let mark_labels = dots
            .into_iter()
            .filter_map(|(fraction, label)| label.map(|label| (fraction, label)))
            .map(|(fraction, label)| {
                div()
                    .absolute()
                    .when(axis.is_horizontal(), |this| {
                        this.top(px(12.))
                            .inset_s(relative(fraction), cx)
                            .w(px(0.))
                            .flex()
                            .justify_center()
                    })
                    .when(axis.is_vertical(), |this| {
                        this.left(px(14.))
                            .bottom(relative(fraction))
                            .h(px(0.))
                            .flex()
                            .items_center()
                    })
                    .child(
                        div()
                            .flex_none()
                            .whitespace_nowrap()
                            .text_caption()
                            .text_color(mark_label_color)
                            .child(label),
                    )
                    .into_any_element()
            })
            .collect::<Vec<_>>();

        BaseSlider::new(&self.state)
            .axis(axis)
            .disabled(self.disabled)
            // Keep the current focus in the dispatch tree when the slider is
            // disabled while focused, so Tab can still leave in either direction.
            .when(!self.disabled || is_focused, |this| {
                this.track_focus(
                    &focus_handle
                        .tab_index(self.tab_index)
                        .tab_stop(self.tab_stop && !self.disabled),
                )
            })
            .when_some(self.accessibility_label, |this, label| {
                this.accessibility_label(label)
            })
            .flex()
            .flex_1()
            .items_center()
            .justify_center()
            .when(axis.is_vertical(), |this| this.h(px(120.)))
            .when(axis.is_horizontal(), |this| this.w_full())
            // Room for what sits outside the track: labels under it and a
            // value tag that always shows over it.
            .when(has_mark_labels && axis.is_horizontal(), |this| {
                this.pb(px(16.))
            })
            .when(
                value_label == SliderValueLabel::Always && axis.is_horizontal(),
                |this| this.pt(px(22.)),
            )
            .refine_style(&self.style)
            .bg(cx.theme().transparent)
            .text_color(cx.theme().foreground)
            .child(
                SliderTrack::new(&self.state)
                    .axis(axis)
                    .disabled(self.disabled)
                    .flex()
                    .when(axis.is_horizontal(), |this| {
                        this.items_center().h_6().w_full()
                    })
                    .when(axis.is_vertical(), |this| {
                        this.justify_center().w_6().h_full()
                    })
                    .flex_shrink_0()
                    .child(
                        SliderIndicator::new(&self.state)
                            .relative()
                            .when(axis.is_horizontal(), |this| this.w_full().h_1())
                            .when(axis.is_vertical(), |this| this.h_full().w_1())
                            .bg(bar_color.opacity(0.12))
                            .active(|this| this.bg(bar_color.opacity(0.18)))
                            .corner_radii(radius)
                            .child(
                                div()
                                    .absolute()
                                    .when(axis.is_horizontal(), |this| {
                                        // The fill runs from the start edge: the right in RTL.
                                        this.h_full().inset_s(bar_start, cx).inset_e(bar_end, cx)
                                    })
                                    .when(axis.is_vertical(), |this| {
                                        this.w_full().bottom(bar_start).top(bar_end)
                                    })
                                    .bg(bar_color)
                                    .rounded_full_style(cx),
                            )
                            .children(mark_elements)
                            .children(mark_labels)
                            .when_some(start_ring, |this, ring| {
                                this.child(thumb(
                                    relative(percentage.start),
                                    true,
                                    ring,
                                    start_text.unwrap_or_default(),
                                ))
                            })
                            .child(thumb(relative(percentage.end), false, end_ring, end_text)),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use gpui::{AppContext as _, Context, Modifiers, Render, TestAppContext, point};

    use super::*;

    #[test]
    fn step_marks_cover_the_range_and_give_up_when_too_fine() {
        assert_eq!(step_mark_values(0., 10., 5., 40), vec![0., 5., 10.]);
        assert_eq!(
            step_mark_values(0., 1., 0.25, 40),
            vec![0., 0.25, 0.5, 0.75, 1.]
        );
        // A step that does not divide the range stops short of the end.
        assert_eq!(step_mark_values(0., 10., 4., 40), vec![0., 4., 8.]);
        assert!(step_mark_values(0., 100., 1., 40).is_empty());
        assert!(step_mark_values(0., 10., 0., 40).is_empty());
        assert!(step_mark_values(5., 5., 1., 40).is_empty());
    }

    #[test]
    fn values_take_the_steps_decimals() {
        assert_eq!(format_slider_value(42., 1.), "42");
        assert_eq!(format_slider_value(42.25, 0.25), "42.25");
        assert_eq!(format_slider_value(0.5, 0.1), "0.5");
        assert_eq!(format_slider_value(1.0, 0.05), "1.00");
        assert_eq!(format_slider_value(3.0, 15.), "3");
    }

    #[test]
    fn marks_inside_the_fill_are_filled() {
        assert!(mark_is_filled(0.2, 0.0..0.5));
        assert!(mark_is_filled(0.5, 0.0..0.5));
        assert!(!mark_is_filled(0.6, 0.0..0.5));
        assert!(!mark_is_filled(0.1, 0.2..0.8));
    }

    struct Harness {
        state: Entity<SliderState>,
        disabled: bool,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .w(px(100.))
                .h(px(24.))
                .child(Slider::new(&self.state).disabled(self.disabled))
        }
    }

    fn harness(
        cx: &mut TestAppContext,
        disabled: bool,
    ) -> (&mut gpui::VisualTestContext, Entity<SliderState>) {
        cx.update(crate::theme::init);
        let state = cx.new(|_| SliderState::new());
        let result = state.clone();
        let (_, cx) = cx.add_window_view(move |_, _| Harness { state, disabled });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        (cx, result)
    }

    #[gpui::test]
    fn pointer_updates_the_migrated_state(cx: &mut TestAppContext) {
        let (cx, state) = harness(cx, false);
        cx.simulate_click(point(px(50.), px(12.)), Modifiers::default());
        cx.update(|_, cx| assert!((state.read(cx).value().end() - 50.).abs() < 1.));
    }

    /// Moves focus as Tab does, then presses `keys`.
    fn tab_then_press(cx: &mut gpui::VisualTestContext, keys: &str) {
        cx.update(|window, cx| {
            window.focus_next(cx);
            window.draw(cx).clear(cx);
        });
        cx.simulate_keystrokes(keys);
    }

    #[gpui::test]
    fn tab_reaches_the_slider_and_keys_step_it(cx: &mut TestAppContext) {
        let (cx, state) = harness(cx, false);
        tab_then_press(cx, "right right");
        cx.update(|_, cx| assert_eq!(state.read(cx).value(), SliderValue::Single(2.)));
        cx.simulate_keystrokes("end");
        cx.update(|_, cx| assert_eq!(state.read(cx).value(), SliderValue::Single(100.)));
    }

    #[gpui::test]
    fn tab_skips_a_disabled_slider(cx: &mut TestAppContext) {
        let (cx, state) = harness(cx, true);
        tab_then_press(cx, "right end");
        cx.update(|_, cx| assert_eq!(state.read(cx).value(), SliderValue::Single(0.)));
    }

    #[gpui::test]
    fn disabled_slider_is_inert(cx: &mut TestAppContext) {
        let (cx, state) = harness(cx, true);
        cx.simulate_click(point(px(50.), px(12.)), Modifiers::default());
        cx.update(|_, cx| assert_eq!(state.read(cx).value(), SliderValue::Single(0.)));
    }
}
