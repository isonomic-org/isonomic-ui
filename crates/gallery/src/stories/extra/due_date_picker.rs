// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the due date picker.

use crate::Story;
use isonomic_ui::component::h_flex;
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window,
};
use isonomic_ui_extra::due_date_picker::{Day, DayRange, DueDatePicker, next_range};

/// Picking an invoice's due date on 14 April 2026: no weekends, nothing
/// before today. Beside it, a leave request: a range from today to the end
/// of June. The story keeps the picks and the cursors.
pub struct DueDatePickerStory {
    picked: Option<Day>,
    cursor: Option<Day>,
    leave: DayRange,
    leave_cursor: Option<Day>,
}

const TODAY: Day = Day {
    year: 2026,
    month: 4,
    day: 14,
};

impl DueDatePickerStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            picked: Some(TODAY.add_days(30)),
            cursor: None,
            leave: (Some(TODAY.add_days(6)), Some(TODAY.add_days(10))),
            leave_cursor: None,
        })
    }
}

impl Story for DueDatePickerStory {
    fn title() -> &'static str {
        "Due date picker"
    }

    fn description() -> &'static str {
        "A month calendar with one-click presets, a range mode, and an earliest and latest day it keeps to."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for DueDatePickerStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (pick, cursor) = (cx.entity(), cx.entity());
        let (pick_leave, leave_cursor) = (cx.entity(), cx.entity());
        let due = DueDatePicker::new("due", TODAY)
            .picked(self.picked)
            .cursor(self.cursor)
            .earliest(TODAY)
            .weekends(false)
            .on_pick(move |day, _, cx| {
                pick.update(cx, |this, cx| {
                    this.picked = Some(day);
                    this.cursor = Some(day);
                    cx.notify();
                })
            })
            .on_cursor(move |day, _, cx| {
                cursor.update(cx, |this, cx| {
                    this.cursor = Some(day);
                    cx.notify();
                })
            });
        let leave = DueDatePicker::new("leave", TODAY)
            .range(self.leave)
            .cursor(self.leave_cursor)
            .earliest(TODAY)
            .latest(Day::new(2026, 6, 30))
            .on_pick(move |day, _, cx| {
                pick_leave.update(cx, |this, cx| {
                    this.leave = next_range(this.leave, day);
                    this.leave_cursor = Some(day);
                    cx.notify();
                })
            })
            .on_cursor(move |day, _, cx| {
                leave_cursor.update(cx, |this, cx| {
                    this.leave_cursor = Some(day);
                    cx.notify();
                })
            });
        h_flex()
            .items_start()
            .flex_wrap()
            .gap_6()
            .child(due)
            .child(leave)
    }
}
