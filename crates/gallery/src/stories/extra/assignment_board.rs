// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the assignment board.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::assignment_board::{
    AssigneeKind, AssigneeLane, AssignmentBoard, Availability, WorkItem, WorkPriority,
};

/// Shows a month-end close split between two assistants and two people.
/// The story moves cards when the board reports a hand-over.
pub struct AssignmentBoardStory {
    items: Vec<WorkItem>,
    selected: Option<SharedString>,
    last: Option<String>,
}

fn lanes() -> Vec<AssigneeLane> {
    vec![
        AssigneeLane::new("open", "Not picked up", AssigneeKind::Unassigned),
        AssigneeLane::new("recon", "Reconciler", AssigneeKind::Agent)
            .capacity(8)
            .availability(Availability::Busy("Matching bank lines".into())),
        AssigneeLane::new("payables", "Payables assistant", AssigneeKind::Agent)
            .capacity(5)
            .availability(Availability::Free),
        AssigneeLane::new("maya", "Maya Okafor", AssigneeKind::Person)
            .capacity(4)
            .availability(Availability::Free),
        AssigneeLane::new("tom", "Tom Brandt", AssigneeKind::Person)
            .capacity(3)
            .availability(Availability::Away("Back Monday".into())),
    ]
}

fn items() -> Vec<WorkItem> {
    vec![
        WorkItem::new("w1", "Review the March accrual for rent", "open")
            .points(2)
            .tag("Close"),
        WorkItem::new("w2", "Chase the missing Harbour Supply receipt", "open").tag("Payables"),
        WorkItem::new("w3", "Match 214 card payouts", "recon")
            .points(5)
            .tag("Bank"),
        WorkItem::new("w4", "Flag duplicate fuel charges", "recon")
            .points(2)
            .priority(WorkPriority::Low),
        WorkItem::new("w5", "Schedule Friday's supplier run", "payables")
            .points(3)
            .priority(WorkPriority::Urgent),
        WorkItem::new("w6", "Approve the write-off for invoice 2240", "maya")
            .points(2)
            .priority(WorkPriority::Urgent)
            .blocked_by("the customer's reply"),
        WorkItem::new("w7", "Sign off the payroll journal", "maya").points(2),
        WorkItem::new("w8", "Update the fixed asset register", "tom").points(3),
    ]
}

impl AssignmentBoardStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            items: items(),
            selected: None,
            last: None,
        })
    }
}

impl Story for AssignmentBoardStory {
    fn title() -> &'static str {
        "Assignment board"
    }

    fn description() -> &'static str {
        "Work items in one column per assignee, handed over by dragging."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for AssignmentBoardStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let assign = cx.entity();
        let select = cx.entity();
        let open = cx.entity();
        let lanes = lanes();
        let names: Vec<(SharedString, SharedString)> = lanes
            .iter()
            .map(|l| (l.id.clone(), l.name.clone()))
            .collect();
        v_flex()
            .w_full()
            .gap_3()
            .child(
                AssignmentBoard::new("close-board", lanes, self.items.clone())
                    .selected(self.selected.clone())
                    .on_select(move |id, _, cx| {
                        select.update(cx, |this, cx| {
                            this.selected = Some(id.clone());
                            cx.notify();
                        })
                    })
                    .on_open(move |id, _, cx| {
                        open.update(cx, |this, cx| {
                            let title = this
                                .items
                                .iter()
                                .find(|i| &i.id == id)
                                .map(|i| i.title.to_string())
                                .unwrap_or_default();
                            this.last = Some(format!("Opened \u{201c}{title}\u{201d}"));
                            cx.notify();
                        })
                    })
                    .on_assign(move |id, lane, _, cx| {
                        let to = names
                            .iter()
                            .find(|(lane_id, _)| lane_id == lane)
                            .map(|(_, name)| name.to_string())
                            .unwrap_or_default();
                        assign.update(cx, |this, cx| {
                            if let Some(item) = this.items.iter_mut().find(|i| &i.id == id) {
                                item.lane = lane.clone();
                                this.last =
                                    Some(format!("Handed \u{201c}{}\u{201d} to {to}", item.title));
                            }
                            this.selected = Some(id.clone());
                            cx.notify();
                        })
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.last.clone().unwrap_or_else(|| {
                    "Drag a card to another column, or select one and press Alt with Left or Right."
                        .into()
                })),
            )
    }
}
