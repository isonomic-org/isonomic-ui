// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the due date picker.

use crate::Story;
use isonomic_ui::{App, AppContext as _, Context, Entity, IntoElement, Render, Window};
use isonomic_ui_extra::due_date_picker::{Day, DueDatePicker};

/// Picking an invoice's due date on 14 April 2026: no weekends, nothing
/// before today. The story keeps the pick and the cursor.
pub struct DueDatePickerStory {
    picked: Option<Day>,
    cursor: Option<Day>,
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
        })
    }
}

impl Story for DueDatePickerStory {
    fn title() -> &'static str {
        "Due date picker"
    }

    fn description() -> &'static str {
        "A month calendar with one-click presets for the dates people actually pick, and days that can't be picked greyed out."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for DueDatePickerStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (pick, cursor) = (cx.entity(), cx.entity());
        DueDatePicker::new("due", TODAY)
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
            })
    }
}
