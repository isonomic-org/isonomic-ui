// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::Arc;

use gpui::{
    App, ElementId, Entity, Focusable as _, FontFeatures, InteractiveElement as _, IntoElement,
    ParentElement as _, RenderOnce, SharedString, StatefulInteractiveElement as _, StyleRefinement,
    Styled, Window, div, prelude::FluentBuilder as _, px,
};

use crate::{
    ActiveTheme as _, Disableable, Sizable, Size, StyleSized as _, StyledExt as _,
    input::input_style,
};

use isonomic_ui_base::TimeField as BaseTimeField;
pub use isonomic_ui_base::{HourCycle, TimeFieldEvent, TimeFieldState, TimePrecision, TimeSegment};

/// Digits of equal width (OpenType `tnum`), so a value that changes while it
/// is edited keeps its width instead of shifting with each digit.
pub(crate) fn tabular_figures() -> FontFeatures {
    FontFeatures(Arc::new(vec![("tnum".into(), 1)]))
}

/// Both period labels stacked in one grid cell, the inactive one transparent,
/// so the segment is as wide as the wider label whichever is shown.
fn period_label(pm: bool) -> impl IntoElement {
    let label = |text: &'static str, active: bool| {
        div()
            .col_start(1)
            .row_start(1)
            .when(!active, |this| this.text_color(gpui::transparent_black()))
            .child(text)
    };
    div()
        .grid()
        .grid_cols(1)
        .child(label("AM", !pm))
        .child(label("PM", pm))
}

/// The base field with the library's segment styling: the selected segment
/// highlighted, and the AM/PM segment sized for the wider label. Shared by
/// [`TimeField`] and the time picker, which frame it differently.
pub(crate) fn styled_base_field(
    id: impl Into<ElementId>,
    state: &Entity<TimeFieldState>,
    disabled: bool,
    cx: &App,
) -> BaseTimeField {
    let segment_radius = cx.theme().radius / 2.;
    BaseTimeField::new(id, state)
        .disabled(disabled)
        .flex()
        .items_center()
        .render_segment(move |segment, state, _, cx| {
            segment
                .px_0p5()
                .when(state.segment() == TimeSegment::Period, |this| {
                    this.ml_1()
                        .clear_children()
                        .child(period_label(state.value() == 1))
                })
                .rounded(segment_radius)
                .when(state.is_selected(), |this| this.bg(cx.theme().selection))
                .into_any_element()
        })
}

/// A segmented time editor, e.g. `09:30`, `09:30:15` or `09:30 PM`.
///
/// The value lives in [`TimeFieldState`]; see it for the keyboard model.
#[derive(IntoElement)]
pub struct TimeField {
    id: ElementId,
    state: Entity<TimeFieldState>,
    size: Size,
    style: StyleRefinement,
    aria_label: Option<SharedString>,
    disabled: bool,
    invalid: bool,
}

impl TimeField {
    pub fn new(state: &Entity<TimeFieldState>) -> Self {
        Self {
            id: ("time-field", state.entity_id()).into(),
            state: state.clone(),
            size: Size::default(),
            style: StyleRefinement::default(),
            aria_label: None,
            disabled: false,
            invalid: false,
        }
    }

    /// Name the field for assistive technology, e.g. "Start time", when no
    /// visible label is associated with it.
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    pub(crate) fn with_id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    /// Display the caller's validation result. This does not reject edits.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }
}

impl Sizable for TimeField {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl Disableable for TimeField {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Styled for TimeField {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for TimeField {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // The frame is a field: a hairline ring that firms under the pointer
        // and steps to a full pixel with the caret in it. Fields show no halo.
        let (fade, on_hover) = crate::look::tracked_hover(
            ("time-field-frame", self.state.entity_id()),
            !self.disabled,
            window,
            cx,
        );
        let focused = self.state.read(cx).focus_handle(cx).is_focused(window);
        let (bg, fg) = input_style(self.disabled, cx);
        let frame_shadow = crate::look::surface::field_for(
            fade,
            focused && !self.disabled,
            self.invalid,
            self.disabled,
            cx,
        );

        div()
            .id(("time-field-frame", self.state.entity_id()))
            .flex()
            .items_center()
            .flex_none()
            .font_features(tabular_figures())
            .bg(bg)
            .text_color(fg)
            .rounded(if cx.theme().radius > px(0.) {
                cx.theme().radius + px(1.)
            } else {
                px(0.)
            })
            .shadow(frame_shadow)
            .on_hover(on_hover)
            .input_text_size(self.size)
            .input_h(self.size)
            .child(
                // The base field fills the frame, so a press anywhere inside it lands on the
                // field rather than on the frame around it.
                styled_base_field(self.id, &self.state, self.disabled, cx)
                    .when_some(self.aria_label, |this, label| this.aria_label(label))
                    .size_full()
                    // Segments carry 2px of their own: 8 + 2 is a field's 10px inset.
                    .px_2(),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use gpui::{AppContext as _, px};

    use super::*;

    #[gpui::test]
    fn test_time_field_builder(cx: &mut gpui::TestAppContext) {
        let window = cx.add_empty_window();
        let state = window.update(|window, cx| {
            cx.new(|cx| {
                TimeFieldState::new(window, cx)
                    .precision(TimePrecision::Second)
                    .hour_cycle(HourCycle::H12)
            })
        });
        let field = TimeField::new(&state)
            .with_id("start")
            .large()
            .disabled(true)
            .invalid(true)
            .w(px(120.));

        assert_eq!(field.id, "start".into());
        assert_eq!(field.size, Size::Large);
        assert!(field.disabled);
        assert!(field.invalid);
    }

    #[cfg(feature = "test-support")]
    struct FieldHarness {
        state: Entity<TimeFieldState>,
    }

    #[cfg(feature = "test-support")]
    impl gpui::Render for FieldHarness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            div().child(TimeField::new(&self.state).with_id("time").w(px(200.)))
        }
    }

    #[cfg(feature = "test-support")]
    fn click_at(cx: &mut gpui::VisualTestContext, position: gpui::Point<gpui::Pixels>) {
        cx.simulate_click(position, gpui::Modifiers::default());
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    #[cfg(feature = "test-support")]
    #[gpui::test]
    fn pressing_outside_the_segments_focuses_the_first_segment(cx: &mut gpui::TestAppContext) {
        cx.update(crate::init);
        let (view, cx) = cx.add_window_view(|window, cx| FieldHarness {
            state: cx.new(|cx| TimeFieldState::new(window, cx)),
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let state = cx.update(|_, cx| view.read(cx).state.clone());
        let segment = |cx: &mut gpui::VisualTestContext, id: &'static str| {
            cx.update(|window, _| {
                isonomic_ui_base::test_support::find(window, &[], &ElementId::from(id))
                    .expect("segment is painted")
                    .bounds()
                    .center()
            })
        };
        let blank = gpui::point(px(180.), segment(cx, "hour").y);

        click_at(cx, blank);
        cx.update(|window, cx| {
            assert!(state.read(cx).focus_handle(cx).is_focused(window));
        });
        assert_eq!(
            cx.update(|_, cx| state.read(cx).selected_segment()),
            TimeSegment::Hour
        );

        // A press on a segment still selects that segment, not the first one.
        let minute = segment(cx, "minute");
        click_at(cx, minute);
        assert_eq!(
            cx.update(|_, cx| state.read(cx).selected_segment()),
            TimeSegment::Minute
        );

        click_at(cx, blank);
        assert_eq!(
            cx.update(|_, cx| state.read(cx).selected_segment()),
            TimeSegment::Hour
        );
    }
}
