// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use chrono::{NaiveTime, Timelike as _};
use gpui::{
    AnyElement, App, AppContext as _, Bounds, ClickEvent, Context, ElementId, Empty, Entity,
    EventEmitter, FocusHandle, Focusable, InteractiveElement as _, IntoElement, KeyBinding,
    MouseButton, MouseUpEvent, ParentElement as _, Pixels, Render, RenderOnce, Role, ScrollHandle,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Subscription, Window,
    actions, deferred, div, prelude::FluentBuilder as _, px,
};
use rust_i18n::t;

use crate::ThemeStyled as _;
use crate::{
    ActiveTheme as _, Disableable, IconName, Sizable, Size, StyleSized as _, StyledExt as _,
    actions::Cancel,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::input_style,
};

use isonomic_ui_base::{ElementExt as _, PressableExt as _, TestSupportExt as _};

use super::time_field::{
    HourCycle, TimeFieldEvent, TimeFieldState, TimePrecision, styled_base_field, tabular_figures,
};

const CONTEXT: &str = "TimePicker";

actions!(time_picker, [TogglePopup]);

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", Cancel, Some(CONTEXT)),
        KeyBinding::new("alt-down", TogglePopup, Some(CONTEXT)),
    ])
}

/// How many rows of options a column shows before it scrolls. Odd, so the
/// chosen option can sit in the middle.
const VISIBLE_ROWS: usize = 7;
/// The space between two options.
const ROW_GAP: Pixels = px(2.);
/// The space around a column's options.
const COLUMN_PADDING: Pixels = px(4.);

/// The height of one option and the width of a column at `size`.
fn row_metrics(size: Size) -> (Pixels, Pixels) {
    match size {
        Size::XSmall | Size::Small => (px(28.), px(44.)),
        Size::Large => (px(40.), px(60.)),
        _ => (px(32.), px(52.)),
    }
}

/// A semantic notification from a [`TimePickerState`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimePickerEvent {
    /// Typing or choosing an option changed the time.
    Change(NaiveTime),
}

/// One column of options in the picker's popup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Column {
    Hour,
    Minute,
    Second,
    Period,
}

impl Column {
    fn id(self) -> &'static str {
        match self {
            Self::Hour => "hours",
            Self::Minute => "minutes",
            Self::Second => "seconds",
            Self::Period => "period",
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// The pure rules for the popup's columns, kept apart from the entity so
/// they can be tested without a window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Options {
    precision: TimePrecision,
    hour_cycle: HourCycle,
    minute_step: u32,
    second_step: u32,
}

impl Options {
    fn columns(&self) -> Vec<Column> {
        let mut columns = vec![Column::Hour, Column::Minute];
        if self.precision == TimePrecision::Second {
            columns.push(Column::Second);
        }
        if self.hour_cycle == HourCycle::H12 {
            columns.push(Column::Period);
        }
        columns
    }

    /// The values a column offers, in display order. A 12-hour clock lists
    /// 12 first, as a clock face does.
    fn values(&self, column: Column) -> Vec<u32> {
        match (column, self.hour_cycle) {
            (Column::Hour, HourCycle::H23) => (0..24).collect(),
            (Column::Hour, HourCycle::H12) => std::iter::once(12).chain(1..12).collect(),
            (Column::Minute, _) => (0..60).step_by(self.minute_step as usize).collect(),
            (Column::Second, _) => (0..60).step_by(self.second_step as usize).collect(),
            (Column::Period, _) => vec![0, 1],
        }
    }

    /// The value a column shows for `time`: the hour on the configured
    /// clock, the minute or second, or `0` for AM and `1` for PM.
    fn value(&self, column: Column, time: NaiveTime) -> u32 {
        match (column, self.hour_cycle) {
            (Column::Hour, HourCycle::H23) => time.hour(),
            (Column::Hour, HourCycle::H12) => (time.hour() + 11) % 12 + 1,
            (Column::Minute, _) => time.minute(),
            (Column::Second, _) => time.second(),
            (Column::Period, _) => time.hour() / 12,
        }
    }

    fn label(&self, column: Column, value: u32) -> SharedString {
        match column {
            Column::Period if value == 0 => "AM".into(),
            Column::Period => "PM".into(),
            _ => format!("{value:02}").into(),
        }
    }

    /// `time` with one column set to `value`. Picking an hour on a 12-hour
    /// clock keeps the period, so 9 in the afternoon stays PM.
    fn with_value(&self, time: NaiveTime, column: Column, value: u32) -> NaiveTime {
        let pm_offset = time.hour() / 12 * 12;
        match (column, self.hour_cycle) {
            (Column::Hour, HourCycle::H23) => time.with_hour(value),
            (Column::Hour, HourCycle::H12) => time.with_hour(value % 12 + pm_offset),
            (Column::Minute, _) => time.with_minute(value),
            (Column::Second, _) => time.with_second(value),
            (Column::Period, _) => time.with_hour(time.hour() % 12 + value * 12),
        }
        .unwrap_or(time)
    }
}

/// The value and behaviour of a [`TimePicker`].
///
/// The picker is a [`TimeField`](super::time_field::TimeField) with a button
/// that opens columns of hours, minutes, seconds and AM/PM to pick from. The
/// field keeps the keyboard model: type or step the time there, and the open
/// columns follow. Alt-Down opens and closes the columns; Escape, a press
/// outside them, or the button closes them.
///
/// The owner supplies the value with [`TimePickerState::set_time`], which
/// does not emit; typing and picking emit [`TimePickerEvent::Change`].
pub struct TimePickerState {
    field: Entity<TimeFieldState>,
    time: NaiveTime,
    options: Options,
    open: bool,
    /// Whether the configuration has reached the field. Builders cannot
    /// reach the field, so the first render applies them.
    pushed: bool,
    bounds: Bounds<Pixels>,
    columns: [ScrollHandle; 4],
    /// Whether the columns should scroll to the chosen options when they
    /// next render.
    scroll_pending: bool,
    _subscriptions: Vec<Subscription>,
}

impl TimePickerState {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let field = cx.new(|cx| TimeFieldState::new(window, cx));
        let _subscriptions = vec![cx.subscribe(&field, |this, _, event, cx| match event {
            TimeFieldEvent::Change(time) => {
                this.time = *time;
                this.scroll_to_selection();
                cx.emit(TimePickerEvent::Change(*time));
                cx.notify();
            }
        })];

        Self {
            field,
            time: NaiveTime::MIN,
            options: Options {
                precision: TimePrecision::default(),
                hour_cycle: HourCycle::default(),
                minute_step: 1,
                second_step: 1,
            },
            open: false,
            pushed: false,
            bounds: Bounds::default(),
            columns: Default::default(),
            scroll_pending: false,
            _subscriptions,
        }
    }

    /// Set the precision, default: [`TimePrecision::Minute`].
    pub fn precision(mut self, precision: TimePrecision) -> Self {
        self.options.precision = precision;
        self.time = precision.truncate(self.time);
        self.pushed = false;
        self
    }

    /// Set the hour cycle, default: [`HourCycle::H23`].
    pub fn hour_cycle(mut self, hour_cycle: HourCycle) -> Self {
        self.options.hour_cycle = hour_cycle;
        self.pushed = false;
        self
    }

    /// List every `step` minutes in the popup, e.g. `15` for 00, 15, 30 and
    /// 45. Typing still sets any minute. Default: `1`.
    pub fn minute_step(mut self, step: u32) -> Self {
        self.options.minute_step = step.clamp(1, 30);
        self
    }

    /// List every `step` seconds in the popup. Default: `1`.
    pub fn second_step(mut self, step: u32) -> Self {
        self.options.second_step = step.clamp(1, 30);
        self
    }

    pub fn time(&self) -> NaiveTime {
        self.time
    }

    /// Replace the time without emitting [`TimePickerEvent::Change`].
    pub fn set_time(&mut self, time: NaiveTime, window: &mut Window, cx: &mut Context<Self>) {
        self.time = self.options.precision.truncate(time);
        let time = self.time;
        self.field
            .update(cx, |field, cx| field.set_time(time, window, cx));
        self.scroll_to_selection();
        cx.notify();
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Open or close the popup.
    pub fn set_open(&mut self, open: bool, _: &mut Window, cx: &mut Context<Self>) {
        if self.open != open {
            self.open = open;
            if open {
                self.scroll_to_selection();
            }
            cx.notify();
        }
    }

    /// Bring each column's chosen option to the middle of the column when it
    /// next renders, so the values on either side show too.
    fn scroll_to_selection(&mut self) {
        self.scroll_pending = true;
    }

    fn apply_scroll(&mut self, size: Size) {
        if !std::mem::take(&mut self.scroll_pending) {
            return;
        }
        let (row_height, _) = row_metrics(size);
        for column in self.options.columns() {
            let selected = self.options.value(column, self.time);
            let values = self.options.values(column);
            let ix = values
                .iter()
                .position(|value| *value == selected)
                .unwrap_or(0);
            // Near either end the column stops scrolling rather than leave a gap.
            let rows_above = ix
                .saturating_sub(VISIBLE_ROWS / 2)
                .min(values.len().saturating_sub(VISIBLE_ROWS)) as f32;
            self.columns[column.index()]
                .set_offset(gpui::point(px(0.), -(row_height + ROW_GAP) * rows_above));
        }
    }

    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pushed {
            return;
        }
        let (precision, hour_cycle, time) =
            (self.options.precision, self.options.hour_cycle, self.time);
        self.field.update(cx, |field, cx| {
            field.set_precision(precision, window, cx);
            field.set_hour_cycle(hour_cycle, window, cx);
            field.set_time(time, window, cx);
        });
        self.pushed = true;
    }

    fn select(&mut self, column: Column, value: u32, window: &mut Window, cx: &mut Context<Self>) {
        let time = self.options.with_value(self.time, column, value);
        if time != self.time {
            self.time = time;
            self.field
                .update(cx, |field, cx| field.set_time(time, window, cx));
            cx.emit(TimePickerEvent::Change(time));
        }
        cx.notify();
    }

    fn toggle(&mut self, _: &TogglePopup, window: &mut Window, cx: &mut Context<Self>) {
        self.set_open(!self.open, window, cx);
    }

    fn on_cancel(&mut self, _: &Cancel, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            self.set_open(false, window, cx);
        } else {
            cx.propagate();
        }
    }
}

impl Focusable for TimePickerState {
    /// The field's focus: the picker is one Tab stop for typing, plus its
    /// button.
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.field.focus_handle(cx)
    }
}
impl EventEmitter<TimePickerEvent> for TimePickerState {}
impl Render for TimePickerState {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// A time field with a popup of hours, minutes, seconds and AM/PM.
///
/// The value lives in [`TimePickerState`]; see it for the behaviour.
#[derive(IntoElement)]
pub struct TimePicker {
    id: ElementId,
    state: Entity<TimePickerState>,
    size: Size,
    style: StyleRefinement,
    aria_label: Option<SharedString>,
    disabled: bool,
    invalid: bool,
}

impl TimePicker {
    pub fn new(state: &Entity<TimePickerState>) -> Self {
        Self {
            id: ("time-picker", state.entity_id()).into(),
            state: state.clone(),
            size: Size::default(),
            style: StyleRefinement::default(),
            aria_label: None,
            disabled: false,
            invalid: false,
        }
    }

    /// Name the time field for assistive technology, e.g. "Start time", when
    /// no visible label is associated with it.
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    /// Display the caller's validation result. This does not reject edits.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    fn render_column(
        &self,
        column: Column,
        options: Options,
        time: NaiveTime,
        scroll: &ScrollHandle,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let selected = options.value(column, time);
        let (row_height, width) = row_metrics(self.size);
        let rows = VISIBLE_ROWS as f32;
        let radius = match self.size {
            Size::XSmall | Size::Small => cx.theme().radius / 2.,
            Size::Large => cx.theme().radius * 2.,
            _ => cx.theme().radius,
        };
        let label = match column {
            Column::Hour => t!("TimePicker.hours"),
            Column::Minute => t!("TimePicker.minutes"),
            Column::Second => t!("TimePicker.seconds"),
            Column::Period => t!("TimePicker.period"),
        };

        div()
            .id(column.id())
            .test_support()
            .role(Role::ListBox)
            .aria_label(SharedString::from(label))
            .flex()
            .flex_col()
            .gap(ROW_GAP)
            .p(COLUMN_PADDING)
            .w(width)
            .h(row_height * rows + ROW_GAP * (rows - 1.) + COLUMN_PADDING * 2.)
            .overflow_y_scroll()
            .track_scroll(scroll)
            .when(column != Column::Hour, |this| {
                this.border_l_1().border_color(cx.theme().border)
            })
            .children(options.values(column).into_iter().map(|value| {
                let active = value == selected;
                let label = options.label(column, value);
                div()
                    .id((column.id(), value as usize))
                    .test_support()
                    .role(Role::ListBoxOption)
                    .aria_label(label.clone())
                    .aria_selected(active)
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(row_height)
                    .rounded(radius)
                    .map(|this| match self.size {
                        Size::XSmall | Size::Small => this.text_xs(),
                        _ => this.text_sm(),
                    })
                    .pressable(true)
                    .when(!active, |this| {
                        this.hover(|this| {
                            this.bg(cx.theme().tokens.secondary_hover)
                                .text_color(cx.theme().foreground)
                        })
                    })
                    .when(active, |this| {
                        this.bg(cx.theme().tokens.primary)
                            .text_color(cx.theme().primary_foreground)
                    })
                    .on_click(window.listener_for(
                        &self.state,
                        move |state, _: &ClickEvent, window, cx| {
                            state.select(column, value, window, cx);
                        },
                    ))
                    .child(label)
            }))
            .into_any_element()
    }
}

impl Sizable for TimePicker {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl Disableable for TimePicker {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Styled for TimePicker {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for TimePicker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let size = self.size;
        self.state.update(cx, |state, cx| {
            state.sync(window, cx);
            if state.open {
                state.apply_scroll(size);
            }
        });
        let state = self.state.read(cx);
        let open = state.open && !self.disabled;
        let field = state.field.clone();
        let (options, time, bounds) = (state.options, state.time, state.bounds);
        let scroll = state.columns.clone();
        let focused = field.focus_handle(cx).is_focused(window);
        let (bg, fg) = input_style(self.disabled, cx);
        let toggle_label = SharedString::from(t!("TimePicker.toggle"));
        let columns = open.then(|| {
            options
                .columns()
                .into_iter()
                .map(|column| {
                    let scroll = &scroll[column.index()];
                    self.render_column(column, options, time, scroll, window, cx)
                })
                .collect::<Vec<_>>()
        });

        div()
            .id(self.id.clone())
            .key_context(CONTEXT)
            .when(!self.disabled, |this| {
                this.on_action(window.listener_for(&self.state, TimePickerState::toggle))
                    .on_action(window.listener_for(&self.state, TimePickerState::on_cancel))
            })
            .relative()
            .flex_none()
            .child(
                div()
                    .flex()
                    .items_center()
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
                    .when(self.invalid, |this| this.border_color(cx.theme().danger))
                    .on_prepaint({
                        let state = self.state.clone();
                        move |bounds, _, cx| state.update(cx, |state, _| state.bounds = bounds)
                    })
                    .child(
                        // The field fills the frame up to the button, so a press beside the
                        // segments still lands on the field.
                        styled_base_field(
                            ("field", self.state.entity_id()),
                            &field,
                            self.disabled,
                            cx,
                        )
                        .when_some(self.aria_label, |this, label| this.aria_label(label))
                        .h_full()
                        .flex_1()
                        .pl_1(),
                    )
                    .child(
                        Button::new("toggle")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Time)
                            .tooltip(toggle_label.clone())
                            .accessibility_label(toggle_label)
                            .disabled(self.disabled)
                            .mr_0p5()
                            .on_click(window.listener_for(
                                &self.state,
                                |state, _: &ClickEvent, window, cx| {
                                    state.toggle(&TogglePopup, window, cx);
                                },
                            )),
                    ),
            )
            .when_some(columns, |this, columns| {
                this.child(
                    deferred(crate::popover::dropdown_popup(
                        ("time-picker-popup", self.state.entity_id()),
                        bounds,
                        h_flex()
                            .occlude()
                            .p_1()
                            .popover_style(cx)
                            .font_features(tabular_figures())
                            // A press outside the popup closes it, except one on the field or its
                            // button: the field keeps it open while typing, and the button
                            // toggles it itself.
                            .on_mouse_up_out(
                                MouseButton::Left,
                                window.listener_for(
                                    &self.state,
                                    |state, event: &MouseUpEvent, window, cx| {
                                        if !state.bounds.contains(&event.position) {
                                            state.set_open(false, window, cx);
                                        }
                                    },
                                ),
                            )
                            .children(columns),
                        cx,
                    ))
                    .with_priority(isonomic_ui_base::POPUP_PRIORITY),
                )
            })
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hm(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    fn options(precision: TimePrecision, hour_cycle: HourCycle) -> Options {
        Options {
            precision,
            hour_cycle,
            minute_step: 1,
            second_step: 1,
        }
    }

    #[test]
    fn columns_follow_the_precision_and_clock() {
        use Column::*;
        let columns = |p, h| options(p, h).columns();
        assert_eq!(
            columns(TimePrecision::Minute, HourCycle::H23),
            [Hour, Minute]
        );
        assert_eq!(
            columns(TimePrecision::Second, HourCycle::H12),
            [Hour, Minute, Second, Period]
        );
    }

    #[test]
    fn a_twelve_hour_clock_lists_twelve_first() {
        let options = options(TimePrecision::Minute, HourCycle::H12);
        let hours = options.values(Column::Hour);
        assert_eq!(hours.len(), 12);
        assert_eq!(&hours[..3], &[12, 1, 2]);
        assert_eq!(options.values(Column::Period), [0, 1]);
        assert_eq!(options.label(Column::Period, 1), "PM");
        assert_eq!(options.label(Column::Hour, 9), "09");
    }

    #[test]
    fn steps_thin_the_minutes() {
        let mut options = options(TimePrecision::Minute, HourCycle::H23);
        options.minute_step = 15;
        assert_eq!(options.values(Column::Minute), [0, 15, 30, 45]);
        assert_eq!(options.values(Column::Hour).len(), 24);
    }

    #[test]
    fn picking_an_hour_keeps_the_period() {
        let options = options(TimePrecision::Minute, HourCycle::H12);
        assert_eq!(options.with_value(hm(14, 30), Column::Hour, 9), hm(21, 30));
        assert_eq!(options.with_value(hm(14, 30), Column::Hour, 12), hm(12, 30));
        assert_eq!(options.with_value(hm(2, 30), Column::Hour, 12), hm(0, 30));
        assert_eq!(options.with_value(hm(9, 30), Column::Period, 1), hm(21, 30));
        assert_eq!(options.with_value(hm(21, 30), Column::Period, 0), hm(9, 30));
        assert_eq!(options.value(Column::Hour, hm(0, 0)), 12);
        assert_eq!(options.value(Column::Period, hm(12, 0)), 1);
    }

    #[test]
    fn picking_a_minute_or_second_leaves_the_hour() {
        let options = options(TimePrecision::Second, HourCycle::H23);
        let time = NaiveTime::from_hms_opt(9, 30, 15).unwrap();
        assert_eq!(
            options.with_value(time, Column::Minute, 45),
            NaiveTime::from_hms_opt(9, 45, 15).unwrap()
        );
        assert_eq!(
            options.with_value(time, Column::Second, 0),
            NaiveTime::from_hms_opt(9, 30, 0).unwrap()
        );
    }

    #[cfg(feature = "test-support")]
    struct PickerHarness {
        state: Entity<TimePickerState>,
        changes: Vec<NaiveTime>,
        _subscription: Subscription,
    }

    #[cfg(feature = "test-support")]
    impl Render for PickerHarness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .p_4()
                .child(TimePicker::new(&self.state).aria_label("Start time"))
        }
    }

    #[cfg(feature = "test-support")]
    fn center_of(
        cx: &mut gpui::VisualTestContext,
        id: impl Into<ElementId>,
    ) -> gpui::Point<Pixels> {
        let id = id.into();
        cx.update(|window, _| {
            isonomic_ui_base::test_support::find(window, &[], &id)
                .unwrap_or_else(|| panic!("{id:?} is painted"))
                .bounds()
                .center()
        })
    }

    #[cfg(feature = "test-support")]
    fn click(cx: &mut gpui::VisualTestContext, position: gpui::Point<Pixels>) {
        cx.simulate_click(position, gpui::Modifiers::default());
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    #[cfg(feature = "test-support")]
    #[gpui::test]
    fn picking_typing_and_dismissing(cx: &mut gpui::TestAppContext) {
        cx.update(crate::init);
        let (view, cx) = cx.add_window_view(|window, cx| {
            let state = cx.new(|cx| {
                let mut state = TimePickerState::new(window, cx);
                state.set_time(hm(9, 30), window, cx);
                state
            });
            let _subscription = cx.subscribe(&state, |this: &mut PickerHarness, _, event, _| {
                let TimePickerEvent::Change(time) = event;
                this.changes.push(*time);
            });
            PickerHarness {
                state,
                changes: vec![],
                _subscription,
            }
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let state = cx.update(|_, cx| view.read(cx).state.clone());
        let is_open =
            |cx: &mut gpui::VisualTestContext| cx.update(|_, cx| state.read(cx).is_open());
        let time = |cx: &mut gpui::VisualTestContext| cx.update(|_, cx| state.read(cx).time());

        cx.update(|window, _| {
            let id = ElementId::from(("field", state.entity_id()));
            let field = isonomic_ui_base::test_support::find(window, &[], &id).unwrap();
            assert_eq!(field.role(), Some(Role::TimeInput));
            assert_eq!(field.label(), Some("Start time"));
            let toggle =
                isonomic_ui_base::test_support::find(window, &[], &"toggle".into()).unwrap();
            assert_eq!(toggle.label(), Some("Choose time"));
        });

        // The button opens the columns, with the current values selected.
        let toggle = center_of(cx, "toggle");
        click(cx, toggle);
        assert!(is_open(cx));
        cx.update(|window, _| {
            let hour = isonomic_ui_base::test_support::find(window, &[], &("hours", 9usize).into())
                .expect("the hour column is open");
            assert_eq!(hour.selected(), Some(true));
            assert_eq!(hour.role(), Some(Role::ListBoxOption));
            assert_eq!(hour.label(), Some("09"));
        });

        // Picking options sets the time and stays open for the next column. The
        // columns open scrolled to the chosen values, so nearby options are in view.
        let noon = center_of(cx, ("hours", 12usize));
        click(cx, noon);
        let thirty_three = center_of(cx, ("minutes", 33usize));
        click(cx, thirty_three);
        assert_eq!(time(cx), hm(12, 33));
        assert!(is_open(cx));

        // Typing in the field keeps the columns open and they follow.
        let hour_segment = center_of(cx, "hour");
        click(cx, hour_segment);
        assert!(is_open(cx), "a press on the field keeps the columns open");
        cx.simulate_keystrokes("1 0");
        assert_eq!(time(cx), hm(10, 33));
        cx.update(|window, _| {
            let hour =
                isonomic_ui_base::test_support::find(window, &[], &("hours", 10usize).into())
                    .unwrap();
            assert_eq!(hour.selected(), Some(true));
        });

        // Escape closes; Alt-Down reopens; a press outside closes again.
        cx.simulate_keystrokes("escape");
        assert!(!is_open(cx));
        cx.simulate_keystrokes("alt-down");
        assert!(is_open(cx));
        click(cx, gpui::point(px(600.), px(400.)));
        assert!(!is_open(cx));

        // Each keystroke reports the time it leaves, as the field does: "1" is
        // 01:33 until "0" makes it 10:33.
        assert_eq!(
            cx.update(|_, cx| view.read(cx).changes.clone()),
            [hm(12, 30), hm(12, 33), hm(1, 33), hm(10, 33)]
        );
    }
}
