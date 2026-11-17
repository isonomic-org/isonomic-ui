// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the run schedule board.

use crate::Story;
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, Render, SharedString, Window,
};
use isonomic_ui_extra::run_schedule_board::{RunScheduleBoard, ScheduledSession, auto_place};

const DAYS: [&str; 5] = ["Mon", "Tue", "Wed", "Thu", "Fri"];
const HOURS: (u32, u32) = (8, 18);
const CAP: u32 = 5;

/// A week of bookkeeping sessions with a five-hour daily cap. The story
/// moves sessions where they're dropped and places the rest on Auto.
pub struct RunScheduleBoardStory {
    sessions: Vec<ScheduledSession>,
}

impl RunScheduleBoardStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            sessions: vec![
                ScheduledSession::new("recon", "Reconcile last week", 2).at(0, 9),
                ScheduledSession::new("pay", "Build the supplier run", 2).at(3, 10),
                ScheduledSession::new("payroll", "Payroll check", 1).at(4, 9),
                ScheduledSession::new("chase", "Chase overdue invoices", 1),
                ScheduledSession::new("receipts", "File receipts", 2),
                ScheduledSession::new("close", "Draft the month-end close", 3),
                ScheduledSession::new("vendors", "Refresh vendor balances", 1),
            ],
        })
    }
}

impl Story for RunScheduleBoardStory {
    fn title() -> &'static str {
        "Run schedule board"
    }

    fn description() -> &'static str {
        "A week of hour slots where limited assistant sessions are placed by dragging, or by an auto mode."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for RunScheduleBoardStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let place = cx.entity();
        let auto = cx.entity();
        RunScheduleBoard::new(
            "week",
            DAYS.iter().map(|d| SharedString::from(*d)).collect(),
            HOURS,
            CAP,
        )
        .sessions(self.sessions.clone())
        .on_place(move |id, slot, _, cx| {
            place.update(cx, |this, cx| {
                if let Some(session) = this.sessions.iter_mut().find(|s| &s.id == id) {
                    session.slot = slot;
                }
                cx.notify();
            })
        })
        .on_auto(move |_, cx| {
            auto.update(cx, |this, cx| {
                auto_place(&mut this.sessions, DAYS.len(), HOURS, CAP);
                cx.notify();
            })
        })
    }
}
