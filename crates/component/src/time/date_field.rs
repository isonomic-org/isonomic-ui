// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

use gpui::{
    App, ElementId, Entity, Focusable as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _,
};

use crate::{
    ActiveTheme as _, Disableable, Sizable, Size, StyleSized as _, StyledExt as _,
    ThemeStyled as _, input::input_style,
};

use super::time_field::tabular_figures;
use isonomic_ui_base::DateField as BaseDateField;
pub use isonomic_ui_base::{DateFieldEvent, DateFieldState, DateOrder, DateSegment};

/// A segmented date editor, e.g. `2026/11/14`, `11/14/2026` or `14.11.2026`.
///
/// The value lives in [`DateFieldState`]; see it for the keyboard model. The
/// field shows the invalid style when the date falls outside the state's
/// range, as well as when [`DateField::invalid`] says so.
#[derive(IntoElement)]
pub struct DateField {
    id: ElementId,
    state: Entity<DateFieldState>,
    size: Size,
    style: StyleRefinement,
    separator: Option<SharedString>,
    aria_label: Option<SharedString>,
    disabled: bool,
    invalid: bool,
}

impl DateField {
    pub fn new(state: &Entity<DateFieldState>) -> Self {
        Self {
            id: ("date-field", state.entity_id()).into(),
            state: state.clone(),
            size: Size::default(),
            style: StyleRefinement::default(),
            separator: None,
            aria_label: None,
            disabled: false,
            invalid: false,
        }
    }

    /// What stands between the segments, default: `/`.
    pub fn separator(mut self, separator: impl Into<SharedString>) -> Self {
        self.separator = Some(separator.into());
        self
    }

    /// Name the field for assistive technology, e.g. "Start date", when no
    /// visible label is associated with it.
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    /// Display the caller's validation result. This does not reject edits.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }
}

impl Sizable for DateField {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl Disableable for DateField {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Styled for DateField {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for DateField {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let focused = state.focus_handle(cx).is_focused(window);
        let invalid = self.invalid || !state.is_in_range();
        let (bg, fg) = input_style(self.disabled, cx);
        let segment_radius = cx.theme().radius / 2.;

        div()
            .flex()
            .items_center()
            .flex_none()
            .font_features(tabular_figures())
            .bg(bg)
            .text_color(fg)
            .border_1()
            .border_color(cx.theme().input)
            .rounded(cx.theme().radius)
            .input_text_size(self.size)
            .input_h(self.size)
            .when(self.disabled, |this| this.opacity(0.5))
            .when(focused && !self.disabled, |this| {
                this.focus_ring_style(window, cx)
            })
            .when(invalid, |this| this.border_color(cx.theme().danger))
            .child(
                BaseDateField::new(self.id, &self.state)
                    .disabled(self.disabled)
                    .when_some(self.separator, |this, separator| this.separator(separator))
                    .when_some(self.aria_label, |this, label| this.aria_label(label))
                    // The base field fills the frame, so a press anywhere inside it lands on the
                    // field rather than on the frame around it.
                    .flex()
                    .size_full()
                    .px_1()
                    .items_center()
                    .render_segment(move |segment, state, _, cx| {
                        segment
                            .px_0p5()
                            .rounded(segment_radius)
                            .when(state.is_selected(), |this| this.bg(cx.theme().selection))
                            .into_any_element()
                    }),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use gpui::{AppContext as _, px};

    use super::*;

    #[gpui::test]
    fn test_date_field_builder(cx: &mut gpui::TestAppContext) {
        let window = cx.add_empty_window();
        let state = window.update(|window, cx| {
            cx.new(|cx| DateFieldState::new(window, cx).order(DateOrder::DayMonthYear))
        });
        let field = DateField::new(&state)
            .separator(".")
            .large()
            .disabled(true)
            .invalid(true)
            .w(px(140.));

        assert_eq!(field.id, ("date-field", state.entity_id()).into());
        assert_eq!(field.separator.as_ref().map(|s| s.as_ref()), Some("."));
        assert_eq!(field.size, Size::Large);
        assert!(field.disabled);
        assert!(field.invalid);
    }

    #[cfg(feature = "test-support")]
    struct FieldHarness {
        state: Entity<DateFieldState>,
    }

    #[cfg(feature = "test-support")]
    impl gpui::Render for FieldHarness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            div().child(
                DateField::new(&self.state)
                    .aria_label("Start date")
                    .w(px(200.)),
            )
        }
    }

    #[cfg(feature = "test-support")]
    fn click_at(cx: &mut gpui::VisualTestContext, position: gpui::Point<gpui::Pixels>) {
        cx.simulate_click(position, gpui::Modifiers::default());
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    #[cfg(feature = "test-support")]
    #[gpui::test]
    fn pressing_selects_a_segment_and_typing_edits_it(cx: &mut gpui::TestAppContext) {
        cx.update(crate::init);
        let (view, cx) = cx.add_window_view(|window, cx| FieldHarness {
            state: cx.new(|cx| {
                let mut state = DateFieldState::new(window, cx);
                state.set_date(NaiveDate::from_ymd_opt(2026, 11, 14).unwrap(), window, cx);
                state
            }),
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

        cx.update(|window, _| {
            let id = ElementId::from(("date-field", state.entity_id()));
            let field = isonomic_ui_base::test_support::find(window, &[], &id).unwrap();
            assert_eq!(field.role(), Some(gpui::Role::DateInput));
            assert_eq!(field.label(), Some("Start date"));
            assert_eq!(field.value(), Some("2026/11/14"));
        });

        let day = segment(cx, "day");
        click_at(cx, day);
        cx.update(|window, cx| {
            assert!(state.read(cx).focus_handle(cx).is_focused(window));
        });
        assert_eq!(
            cx.update(|_, cx| state.read(cx).selected_segment()),
            DateSegment::Day
        );

        cx.simulate_keystrokes("0 9");
        assert_eq!(
            cx.update(|_, cx| state.read(cx).date()),
            NaiveDate::from_ymd_opt(2026, 11, 9).unwrap()
        );
        cx.simulate_keystrokes("shift-tab up");
        assert_eq!(
            cx.update(|_, cx| state.read(cx).date()),
            NaiveDate::from_ymd_opt(2026, 12, 9).unwrap()
        );

        // A press outside the segments selects the first one.
        click_at(cx, gpui::point(px(190.), day.y));
        assert_eq!(
            cx.update(|_, cx| state.read(cx).selected_segment()),
            DateSegment::Year
        );
    }
}
