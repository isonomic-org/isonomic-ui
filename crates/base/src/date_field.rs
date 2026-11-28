// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::TestSupportExt as _;
use std::rc::Rc;

use chrono::{Datelike as _, Local, NaiveDate};
use gpui::{
    AnyElement, App, Context, ElementId, Empty, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyBinding, KeyDownEvent, MouseButton, MouseDownEvent,
    ParentElement, Render, RenderOnce, Role, SharedString, StatefulInteractiveElement,
    StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _,
};

use crate::{
    Decrement, Increment, StyledExt as _,
    a11y_disabled::A11yDisabledExt as _,
    actions::{SelectLeft, SelectNextColumn, SelectPrevColumn, SelectRight},
    input::Delete,
};

const CONTEXT: &str = "DateField";

/// The earliest and latest years a [`DateField`] edits.
const YEAR_RANGE: (u32, u32) = (1, 9999);

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", Increment, Some(CONTEXT)),
        KeyBinding::new("down", Decrement, Some(CONTEXT)),
        KeyBinding::new("left", SelectLeft, Some(CONTEXT)),
        KeyBinding::new("right", SelectRight, Some(CONTEXT)),
        KeyBinding::new("tab", SelectNextColumn, Some(CONTEXT)),
        KeyBinding::new("shift-tab", SelectPrevColumn, Some(CONTEXT)),
        KeyBinding::new("backspace", Delete, Some(CONTEXT)),
        KeyBinding::new("delete", Delete, Some(CONTEXT)),
    ]);
}

/// The reading order of a [`DateField`]'s segments.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DateOrder {
    /// Year, month, day, e.g. `2026/11/14`.
    #[default]
    YearMonthDay,
    /// Month, day, year, e.g. `11/14/2026`.
    MonthDayYear,
    /// Day, month, year, e.g. `14/11/2026`.
    DayMonthYear,
}

impl DateOrder {
    /// The segments in reading order.
    pub fn segments(self) -> &'static [DateSegment; 3] {
        use DateSegment::*;
        match self {
            Self::YearMonthDay => &[Year, Month, Day],
            Self::MonthDayYear => &[Month, Day, Year],
            Self::DayMonthYear => &[Day, Month, Year],
        }
    }

    /// The `chrono` format that displays a date in this order with `separator`.
    pub fn format(self, separator: &str) -> String {
        self.segments()
            .iter()
            .map(|segment| match segment {
                DateSegment::Year => "%Y",
                DateSegment::Month => "%m",
                DateSegment::Day => "%d",
            })
            .collect::<Vec<_>>()
            .join(separator)
    }
}

/// One editable component of a [`DateField`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DateSegment {
    Year,
    Month,
    Day,
}

impl DateSegment {
    fn name(self) -> &'static str {
        match self {
            Self::Year => "year",
            Self::Month => "month",
            Self::Day => "day",
        }
    }

    /// How many digits the segment shows and takes.
    fn digits(self) -> u32 {
        match self {
            Self::Year => 4,
            Self::Month | Self::Day => 2,
        }
    }
}

/// A semantic notification from a [`DateFieldState`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateFieldEvent {
    /// Keyboard editing changed the date.
    Change(NaiveDate),
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .and_then(|first| first.pred_opt())
        .map(|last| last.day())
        .unwrap_or(31)
}

/// Build a date, moving the day back to the month's last day when the month
/// is shorter, so January 31 stepped to February becomes February 28 or 29.
fn clamped_date(year: i32, month: u32, day: u32) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(year, month, day.min(days_in_month(year, month)))
}

/// The pure editing rules of a [`DateFieldState`], kept apart from focus so
/// they can be tested without a window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DateEditor {
    date: NaiveDate,
    order: DateOrder,
    segment: DateSegment,
    /// The digits typed into the selected segment so far, and how many.
    pending: Option<(u32, u32)>,
    /// The day last chosen. A shorter month shows its last day instead, and a
    /// longer month brings the chosen day back, so stepping the month from
    /// January 31 reads February 28, then March 31.
    day: u32,
    /// The year Backspace returns the year segment to.
    reference_year: i32,
}

impl DateEditor {
    fn new(today: NaiveDate) -> Self {
        let order = DateOrder::default();
        Self {
            date: today,
            order,
            segment: order.segments()[0],
            pending: None,
            day: today.day(),
            reference_year: today.year(),
        }
    }

    fn segments(&self) -> &'static [DateSegment; 3] {
        self.order.segments()
    }

    /// The inclusive range a segment takes. The day's range follows the month.
    fn bounds(&self, segment: DateSegment) -> (u32, u32) {
        match segment {
            DateSegment::Year => YEAR_RANGE,
            DateSegment::Month => (1, 12),
            DateSegment::Day => (1, days_in_month(self.date.year(), self.date.month())),
        }
    }

    fn value(&self, segment: DateSegment) -> u32 {
        match segment {
            DateSegment::Year => self.date.year().max(0) as u32,
            DateSegment::Month => self.date.month(),
            DateSegment::Day => self.date.day(),
        }
    }

    fn label(&self, segment: DateSegment) -> SharedString {
        let width = segment.digits() as usize;
        format!("{:0width$}", self.value(segment)).into()
    }

    fn with_value(&self, segment: DateSegment, value: u32) -> NaiveDate {
        let (year, month) = (self.date.year(), self.date.month());
        match segment {
            DateSegment::Year => clamped_date(value as i32, month, self.day),
            DateSegment::Month => clamped_date(year, value, self.day),
            DateSegment::Day => clamped_date(year, month, value),
        }
        .unwrap_or(self.date)
    }

    fn set_order(&mut self, order: DateOrder) {
        self.order = order;
        self.segment = order.segments()[0];
        self.pending = None;
    }

    fn set_date(&mut self, date: NaiveDate) -> bool {
        if self.date == date {
            // Keep a half-typed segment when the owner echoes the same value back.
            return false;
        }
        self.date = date;
        self.day = date.day();
        self.pending = None;
        true
    }

    fn select_segment(&mut self, segment: DateSegment) -> bool {
        self.segment = segment;
        self.pending = None;
        true
    }

    /// Move the selected segment by `offset`; returns `false` at either end.
    fn move_segment(&mut self, offset: isize) -> bool {
        let segments = self.segments();
        let ix = segments
            .iter()
            .position(|segment| *segment == self.segment)
            .unwrap_or(0);
        let Some(next) = ix
            .checked_add_signed(offset)
            .and_then(|ix| segments.get(ix))
        else {
            return false;
        };
        self.select_segment(*next)
    }

    /// Step the selected segment by `delta`. The month and day wrap within
    /// their range without carrying into the next unit; the year stops at its
    /// ends.
    fn step(&mut self, delta: i32) -> bool {
        self.pending = None;
        let (min, max) = self.bounds(self.segment);
        let value = self.value(self.segment) as i32;
        let next = match self.segment {
            DateSegment::Year => (value + delta).clamp(min as i32, max as i32),
            _ => {
                let span = (max - min + 1) as i32;
                (value - min as i32 + delta).rem_euclid(span) + min as i32
            }
        };
        self.replace_segment(next as u32)
    }

    /// Type a digit. The month and day take up to two digits and advance as
    /// soon as no further digit could fit; the year takes four. The day takes
    /// any value up to 31, since the month may be typed after it, and shows
    /// the month's last day until a long enough month is chosen.
    fn input_digit(&mut self, digit: u32) -> bool {
        let segment = self.segment;
        let (min, max) = match segment {
            DateSegment::Day => (1, 31),
            _ => self.bounds(segment),
        };
        let (value, count) = match self.pending.take() {
            Some((typed, count)) if segment == DateSegment::Year => (typed * 10 + digit, count + 1),
            Some((first, _)) if (min..=max).contains(&(first * 10 + digit)) => {
                (first * 10 + digit, 2)
            }
            // The two digits cannot form a valid value, so the new digit starts over.
            _ => (digit, 1),
        };
        let complete = count >= segment.digits() || value * 10 > max;
        if !complete {
            self.pending = Some((value, count));
        }
        // A leading zero waits for the next digit without changing the date.
        let changed = (min..=max).contains(&value) && self.replace_segment(value);
        if complete {
            self.move_segment(1);
        }
        changed
    }

    /// Reset the selected segment: the month and day to `01`, the year to the
    /// reference year.
    fn clear_segment(&mut self) -> bool {
        self.pending = None;
        let value = match self.segment {
            DateSegment::Year => self.reference_year.max(1) as u32,
            DateSegment::Month | DateSegment::Day => 1,
        };
        self.replace_segment(value)
    }

    fn replace_segment(&mut self, value: u32) -> bool {
        let date = self.with_value(self.segment, value);
        if self.segment == DateSegment::Day {
            self.day = value;
        }
        let changed = date != self.date;
        self.date = date;
        changed
    }
}

/// Retained editing behavior for a segmented date field.
///
/// The field is one Tab stop. Inside it, one segment is selected at a time:
/// Up/Down step the selected segment (the month and day wrap within their
/// range, the year stops at its ends), Left/Right and Tab/Shift-Tab move
/// between segments, digits are typed into the segment and advance to the
/// next one once no further digit could fit, and Backspace/Delete reset the
/// segment. A day beyond the end of a shorter month moves back to its last
/// day.
///
/// The owner supplies the value with [`DateFieldState::set_date`], which does
/// not emit; user edits emit [`DateFieldEvent::Change`]. A range set with
/// [`DateFieldState::min_date`] and [`DateFieldState::max_date`] is reported
/// by [`DateFieldState::is_in_range`] rather than enforced, so a year can be
/// typed digit by digit without being pushed back into range.
pub struct DateFieldState {
    focus_handle: FocusHandle,
    editor: DateEditor,
    min: Option<NaiveDate>,
    max: Option<NaiveDate>,
}

impl DateFieldState {
    /// A field showing today's date.
    pub fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle().tab_stop(true),
            editor: DateEditor::new(Local::now().date_naive()),
            min: None,
            max: None,
        }
    }

    /// Set the order of the segments, default: [`DateOrder::YearMonthDay`].
    pub fn order(mut self, order: DateOrder) -> Self {
        self.editor.set_order(order);
        self
    }

    pub fn set_order(&mut self, order: DateOrder, _: &mut Window, cx: &mut Context<Self>) {
        if self.editor.order != order {
            self.editor.set_order(order);
            cx.notify();
        }
    }

    /// The earliest date [`Self::is_in_range`] accepts.
    pub fn min_date(mut self, date: NaiveDate) -> Self {
        self.min = Some(date);
        self
    }

    /// The latest date [`Self::is_in_range`] accepts.
    pub fn max_date(mut self, date: NaiveDate) -> Self {
        self.max = Some(date);
        self
    }

    pub fn set_range(
        &mut self,
        min: Option<NaiveDate>,
        max: Option<NaiveDate>,
        cx: &mut Context<Self>,
    ) {
        self.min = min;
        self.max = max;
        cx.notify();
    }

    /// Whether the date lies within the range set with [`Self::min_date`] and
    /// [`Self::max_date`]. Always `true` without a range.
    pub fn is_in_range(&self) -> bool {
        let date = self.editor.date;
        self.min.is_none_or(|min| date >= min) && self.max.is_none_or(|max| date <= max)
    }

    pub fn date(&self) -> NaiveDate {
        self.editor.date
    }

    /// Replace the date without emitting [`DateFieldEvent::Change`].
    pub fn set_date(&mut self, date: NaiveDate, _: &mut Window, cx: &mut Context<Self>) {
        if self.editor.set_date(date) {
            cx.notify();
        }
    }

    /// The segment that keyboard editing applies to.
    pub fn selected_segment(&self) -> DateSegment {
        self.editor.segment
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.focus_handle.focus(window, cx);
    }

    fn edit(&mut self, cx: &mut Context<Self>, edit: impl FnOnce(&mut DateEditor) -> bool) {
        if edit(&mut self.editor) {
            cx.emit(DateFieldEvent::Change(self.editor.date));
        }
        cx.notify();
    }

    fn on_increment(&mut self, _: &Increment, _: &mut Window, cx: &mut Context<Self>) {
        self.edit(cx, |editor| editor.step(1));
    }

    fn on_decrement(&mut self, _: &Decrement, _: &mut Window, cx: &mut Context<Self>) {
        self.edit(cx, |editor| editor.step(-1));
    }

    fn on_delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        self.edit(cx, DateEditor::clear_segment);
    }

    fn on_select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.move_segment(-1);
        cx.notify();
    }

    fn on_select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.move_segment(1);
        cx.notify();
    }

    // Tab walks the segments first and leaves the field only from the last one.
    fn on_next_column(&mut self, _: &SelectNextColumn, _: &mut Window, cx: &mut Context<Self>) {
        if self.editor.move_segment(1) {
            cx.notify();
        } else {
            cx.propagate();
        }
    }

    fn on_prev_column(&mut self, _: &SelectPrevColumn, _: &mut Window, cx: &mut Context<Self>) {
        if self.editor.move_segment(-1) {
            cx.notify();
        } else {
            cx.propagate();
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        if keystroke.modifiers.modified() || keystroke.key.chars().count() != 1 {
            return;
        }
        let Some(digit) = keystroke.key.chars().next().and_then(|ch| ch.to_digit(10)) else {
            return;
        };
        window.prevent_default();
        cx.stop_propagation();
        self.edit(cx, |editor| editor.input_digit(digit));
    }

    fn select_segment(&mut self, segment: DateSegment, cx: &mut Context<Self>) {
        self.editor.select_segment(segment);
        cx.notify();
    }
}

impl Focusable for DateFieldState {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
impl EventEmitter<DateFieldEvent> for DateFieldState {}
impl Render for DateFieldState {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// State exposed to a [`DateField`] segment slot for decoration.
#[derive(Clone, Copy, Debug)]
pub struct DateFieldSegmentState {
    segment: DateSegment,
    value: u32,
    selected: bool,
    disabled: bool,
}

impl DateFieldSegmentState {
    pub fn segment(&self) -> DateSegment {
        self.segment
    }

    /// The displayed value: the year, the month from 1, or the day from 1.
    pub fn value(&self) -> u32 {
        self.value
    }

    /// Whether keyboard editing applies to this segment: it is the selected
    /// segment and the field has focus.
    pub fn is_selected(&self) -> bool {
        self.selected
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }
}

type SegmentRenderer =
    Rc<dyn Fn(DateFieldSegment, DateFieldSegmentState, &mut Window, &mut App) -> AnyElement>;

/// An unstyled, pre-wired segment passed to the [`DateField`] segment slot.
#[derive(IntoElement)]
pub struct DateFieldSegment {
    base: crate::ObservedElement<gpui::Stateful<gpui::Div>>,
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl DateFieldSegment {
    /// Remove the default label so a styled facade can provide custom content.
    pub fn clear_children(mut self) -> Self {
        self.children.clear();
        self
    }
}

impl ParentElement for DateFieldSegment {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}
impl Styled for DateFieldSegment {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
impl InteractiveElement for DateFieldSegment {
    fn interactivity(&mut self) -> &mut gpui::Interactivity {
        self.base.interactivity()
    }
}
impl StatefulInteractiveElement for DateFieldSegment {}
impl RenderOnce for DateFieldSegment {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.base.children(self.children).refine_style(&self.style)
    }
}

/// Unstyled segmented date editor.
///
/// Base owns focus, the keyboard model and pointer segment selection; the
/// presentation lays out and decorates the field with `Styled` and each
/// segment through [`DateField::render_segment`]. A `/` separates the
/// segments unless [`DateField::separator`] says otherwise.
#[derive(IntoElement)]
pub struct DateField {
    id: ElementId,
    state: Entity<DateFieldState>,
    disabled: bool,
    style: StyleRefinement,
    segment: SegmentRenderer,
    separator: SharedString,
    aria_label: Option<SharedString>,
}

impl DateField {
    pub fn new(id: impl Into<ElementId>, state: &Entity<DateFieldState>) -> Self {
        Self {
            id: id.into(),
            state: state.clone(),
            disabled: false,
            style: StyleRefinement::default(),
            segment: Rc::new(|segment, _, _, _| segment.into_any_element()),
            separator: SharedString::new_static("/"),
            aria_label: None,
        }
    }

    /// Name the field for assistive technology, e.g. "Start date", when no
    /// visible label is associated with it.
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    /// What stands between the field's segments (`/` by default), such as `-`
    /// or `.` for a caller showing a date in its own locale.
    pub fn separator(mut self, separator: impl Into<SharedString>) -> Self {
        self.separator = separator.into();
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Decorate each segment. The segment already carries its label: four
    /// digits for the year, two for the month and day.
    pub fn render_segment(
        mut self,
        render: impl Fn(DateFieldSegment, DateFieldSegmentState, &mut Window, &mut App) -> AnyElement
        + 'static,
    ) -> Self {
        self.segment = Rc::new(render);
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
        let disabled = self.disabled;
        let state = self.state.read(cx);
        let focus_handle = state.focus_handle.clone();
        let focused = focus_handle.is_focused(window);
        let editor = state.editor;
        let value = editor
            .date
            .format(&editor.order.format(&self.separator))
            .to_string();

        let mut root = div()
            .id(self.id.clone())
            .test_support()
            .role(Role::DateInput)
            .when_some(self.aria_label.clone(), |this, label| {
                this.aria_label(label)
            })
            .aria_value(SharedString::from(value))
            .track_focus(&focus_handle.clone().tab_stop(!disabled))
            .when(!disabled, |this| {
                this.key_context(CONTEXT)
                    .on_action(window.listener_for(&self.state, DateFieldState::on_increment))
                    .on_action(window.listener_for(&self.state, DateFieldState::on_decrement))
                    .on_action(window.listener_for(&self.state, DateFieldState::on_delete))
                    .on_action(window.listener_for(&self.state, DateFieldState::on_select_left))
                    .on_action(window.listener_for(&self.state, DateFieldState::on_select_right))
                    .on_action(window.listener_for(&self.state, DateFieldState::on_next_column))
                    .on_action(window.listener_for(&self.state, DateFieldState::on_prev_column))
                    .on_key_down(window.listener_for(&self.state, DateFieldState::on_key_down))
                    // A press on the field outside any segment focuses it and selects the
                    // first segment; a press on a segment stops here and selects that one.
                    .on_mouse_down(MouseButton::Left, {
                        let state = self.state.clone();
                        let focus_handle = focus_handle.clone();
                        move |_: &MouseDownEvent, window, cx| {
                            focus_handle.focus(window, cx);
                            state.update(cx, |state, cx| {
                                let first = state.editor.segments()[0];
                                state.select_segment(first, cx);
                            });
                        }
                    })
            });

        for (ix, segment) in editor.segments().iter().copied().enumerate() {
            if ix > 0 {
                root = root.child(div().child(self.separator.clone()));
            }
            let segment_state = DateFieldSegmentState {
                segment,
                value: editor.value(segment),
                selected: focused && segment == editor.segment,
                disabled,
            };
            let mut item = DateFieldSegment {
                base: div().id(segment.name()).test_support(),
                style: StyleRefinement::default(),
                children: vec![],
            }
            .child(editor.label(segment));
            if !disabled {
                let state = self.state.clone();
                let focus_handle = focus_handle.clone();
                item =
                    item.on_mouse_down(MouseButton::Left, move |_: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        focus_handle.focus(window, cx);
                        state.update(cx, |state, cx| state.select_segment(segment, cx));
                    });
            }
            root = root.child((self.segment)(item, segment_state, window, cx));
        }

        root.refine_style(&self.style).a11y_disabled(disabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn editor_at(order: DateOrder, date: NaiveDate) -> DateEditor {
        let mut editor = DateEditor::new(ymd(2026, 11, 14));
        editor.set_order(order);
        editor.set_date(date);
        editor
    }

    fn type_keys(editor: &mut DateEditor, keys: &str) {
        for ch in keys.chars() {
            editor.input_digit(ch.to_digit(10).unwrap());
        }
    }

    #[test]
    fn typing_fills_segments_in_order_and_advances() {
        let mut editor = editor_at(DateOrder::YearMonthDay, ymd(2000, 1, 1));
        type_keys(&mut editor, "20260915");
        assert_eq!(editor.date, ymd(2026, 9, 15));
        // The last segment stays selected once it is complete.
        assert_eq!(editor.segment, DateSegment::Day);
    }

    #[test]
    fn each_order_reads_and_types_in_its_own_sequence() {
        let mut editor = editor_at(DateOrder::MonthDayYear, ymd(2000, 1, 1));
        assert_eq!(editor.segment, DateSegment::Month);
        type_keys(&mut editor, "09152026");
        assert_eq!(editor.date, ymd(2026, 9, 15));

        let mut editor = editor_at(DateOrder::DayMonthYear, ymd(2000, 1, 1));
        assert_eq!(editor.segment, DateSegment::Day);
        type_keys(&mut editor, "15092026");
        assert_eq!(editor.date, ymd(2026, 9, 15));

        assert_eq!(DateOrder::YearMonthDay.format("/"), "%Y/%m/%d");
        assert_eq!(DateOrder::MonthDayYear.format("/"), "%m/%d/%Y");
        assert_eq!(DateOrder::DayMonthYear.format("."), "%d.%m.%Y");
    }

    #[test]
    fn a_digit_that_cannot_start_two_digits_completes_the_segment() {
        let mut editor = editor_at(DateOrder::YearMonthDay, ymd(2026, 1, 10));
        editor.select_segment(DateSegment::Month);
        // No month starts with 3, so 3 is March and the day is next.
        type_keys(&mut editor, "3");
        assert_eq!(editor.date, ymd(2026, 3, 10));
        assert_eq!(editor.segment, DateSegment::Day);
    }

    #[test]
    fn a_leading_zero_waits_for_the_second_digit() {
        let mut editor = editor_at(DateOrder::YearMonthDay, ymd(2026, 11, 14));
        editor.select_segment(DateSegment::Month);
        assert!(!editor.input_digit(0), "month 00 is not a date");
        assert_eq!(editor.date, ymd(2026, 11, 14));
        assert_eq!(editor.segment, DateSegment::Month);
        type_keys(&mut editor, "4");
        assert_eq!(editor.date, ymd(2026, 4, 14));
        assert_eq!(editor.segment, DateSegment::Day);
    }

    #[test]
    fn a_day_typed_before_its_month_survives_a_short_month() {
        // Day first: February shows its last day until March is typed.
        let mut editor = editor_at(DateOrder::DayMonthYear, ymd(2026, 2, 1));
        type_keys(&mut editor, "31");
        assert_eq!(editor.date, ymd(2026, 2, 28));
        assert_eq!(editor.segment, DateSegment::Month);
        type_keys(&mut editor, "03");
        assert_eq!(editor.date, ymd(2026, 3, 31));
    }

    #[test]
    fn a_day_digit_that_cannot_start_two_digits_completes_the_day() {
        let mut editor = editor_at(DateOrder::YearMonthDay, ymd(2026, 2, 1));
        editor.select_segment(DateSegment::Day);
        // No day starts with 4, so 4 completes the day on its own.
        type_keys(&mut editor, "4");
        assert_eq!(editor.date, ymd(2026, 2, 4));
        let mut editor = editor_at(DateOrder::YearMonthDay, ymd(2026, 1, 1));
        editor.select_segment(DateSegment::Day);
        type_keys(&mut editor, "31");
        assert_eq!(editor.date, ymd(2026, 1, 31));
    }

    #[test]
    fn a_shorter_month_moves_the_day_back_to_its_last_day() {
        let mut editor = editor_at(DateOrder::YearMonthDay, ymd(2026, 1, 31));
        editor.select_segment(DateSegment::Month);
        editor.step(1);
        assert_eq!(editor.date, ymd(2026, 2, 28));
        editor.step(1);
        assert_eq!(editor.date, ymd(2026, 3, 31), "the chosen day comes back");

        let mut editor = editor_at(DateOrder::YearMonthDay, ymd(2024, 2, 29));
        editor.step(1);
        assert_eq!(
            editor.date,
            ymd(2025, 2, 28),
            "a leap day steps to a common year"
        );
    }

    #[test]
    fn stepping_wraps_the_month_and_day_but_not_the_year() {
        let mut editor = editor_at(DateOrder::YearMonthDay, ymd(2026, 12, 31));
        editor.select_segment(DateSegment::Month);
        editor.step(1);
        assert_eq!(editor.date, ymd(2026, 1, 31), "no carry into the year");
        editor.select_segment(DateSegment::Day);
        editor.step(1);
        assert_eq!(editor.date, ymd(2026, 1, 1));
        editor.step(-1);
        assert_eq!(editor.date, ymd(2026, 1, 31));

        let mut editor = editor_at(DateOrder::YearMonthDay, ymd(9999, 6, 1));
        assert!(!editor.step(1), "the year stops at 9999");
        assert_eq!(editor.date, ymd(9999, 6, 1));
    }

    #[test]
    fn the_year_takes_four_digits_before_advancing() {
        let mut editor = editor_at(DateOrder::YearMonthDay, ymd(2026, 11, 14));
        type_keys(&mut editor, "19");
        assert_eq!(editor.segment, DateSegment::Year);
        assert_eq!(editor.label(DateSegment::Year), "0019");
        type_keys(&mut editor, "99");
        assert_eq!(editor.date, ymd(1999, 11, 14));
        assert_eq!(editor.segment, DateSegment::Month);
    }

    #[test]
    fn clearing_resets_only_the_selected_segment() {
        let mut editor = editor_at(DateOrder::YearMonthDay, ymd(1999, 9, 15));
        editor.select_segment(DateSegment::Day);
        assert!(editor.clear_segment());
        assert_eq!(editor.date, ymd(1999, 9, 1));
        editor.select_segment(DateSegment::Year);
        assert!(editor.clear_segment());
        assert_eq!(
            editor.date,
            ymd(2026, 9, 1),
            "the year returns to the reference year"
        );
    }

    #[test]
    fn segment_movement_stops_at_the_ends() {
        let mut editor = editor_at(DateOrder::YearMonthDay, ymd(2026, 11, 14));
        assert!(!editor.move_segment(-1));
        assert!(editor.move_segment(1));
        assert!(editor.move_segment(1));
        assert!(!editor.move_segment(1));
        assert_eq!(editor.segment, DateSegment::Day);
    }

    #[test]
    fn labels_are_zero_padded() {
        let editor = editor_at(DateOrder::YearMonthDay, ymd(987, 3, 4));
        assert_eq!(editor.label(DateSegment::Year), "0987");
        assert_eq!(editor.label(DateSegment::Month), "03");
        assert_eq!(editor.label(DateSegment::Day), "04");
    }

    #[gpui::test]
    fn the_range_is_reported_not_enforced(cx: &mut gpui::TestAppContext) {
        use gpui::AppContext as _;

        let cx = cx.add_empty_window();
        let state = cx.update(|window, cx| {
            cx.new(|cx| {
                DateFieldState::new(window, cx)
                    .min_date(ymd(2026, 1, 1))
                    .max_date(ymd(2026, 12, 31))
            })
        });
        cx.update(|window, cx| {
            state.update(cx, |state, cx| state.set_date(ymd(2026, 6, 1), window, cx));
            assert!(state.read(cx).is_in_range());
            state.update(cx, |state, cx| state.set_date(ymd(2027, 1, 1), window, cx));
            assert!(!state.read(cx).is_in_range());
            assert_eq!(state.read(cx).date(), ymd(2027, 1, 1));
        });
    }

    #[gpui::test]
    fn the_separator_is_a_slash_unless_set(cx: &mut gpui::TestAppContext) {
        use gpui::AppContext as _;

        let cx = cx.add_empty_window();
        let state = cx.update(|window, cx| cx.new(|cx| DateFieldState::new(window, cx)));
        assert_eq!(DateField::new("date", &state).separator.as_ref(), "/");
        assert_eq!(
            DateField::new("date", &state)
                .separator("-")
                .separator
                .as_ref(),
            "-"
        );
    }
}
